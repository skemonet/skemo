//! A* on the implicit orthogonal visibility graph.
//!
//! Cost (R-05) = length (increased in margins), plus `segment_penalty` × bends, plus
//! `crossing_penalty` × crossings with the connectors already routed. The state carries the
//! direction of arrival (to count bends and forbid U-turns) and the number of waypoints already
//! passed (R-07): waypoints are thus optimised globally, not stretch by stretch. Starts and ends
//! are several (one per allowed side): the best side is chosen by the search itself (R-06).

use std::cmp::Ordering;
use std::collections::BinaryHeap;

use crate::geom::Dir;
use crate::model::Params;
use crate::scene::Scene;

pub(crate) type Node = (usize, usize);

/// A possible start.
#[derive(Clone, Debug)]
pub(crate) struct Source {
    pub node: Node,
    /// Direction of the stub (none for a free point).
    pub dir: Option<Dir>,
    /// Cost already spent (length of the stub).
    pub cost: f64,
}

/// A possible end.
#[derive(Clone, Debug)]
pub(crate) struct Target {
    pub node: Node,
    /// Direction imposed on the last stretch (towards the shape), none for a free point.
    pub arrive: Option<Dir>,
    /// Remaining cost (length of the stub).
    pub cost: f64,
}

pub(crate) struct Query<'a> {
    pub scene: &'a Scene,
    pub active: &'a [bool],
    pub params: &'a Params,
    pub sources: &'a [Source],
    pub targets: &'a [Target],
    pub checkpoints: &'a [Node],
    /// Crossings: number of segments already routed through each node (by identifier),
    /// (horizontal, vertical).
    pub through: &'a [(u32, u32)],
}

pub(crate) struct Found {
    pub nodes: Vec<Node>,
    pub source: usize,
    pub target: usize,
}

pub(crate) enum Outcome {
    Found(Found),
    NoPath,
    Exhausted,
}

/// Item of the heap: priority `f`, sequence number, cost `g`, state key and target (`NONE` for an
/// open state). Everything inline: no indirection when popping.
#[derive(Clone, Copy)]
struct Item {
    f: f64,
    g: f64,
    key: u64,
    seq: u32,
    target: u32,
}
const NONE: u32 = u32::MAX;
impl PartialEq for Item {
    fn eq(&self, o: &Self) -> bool {
        self.cmp(o) == Ordering::Equal
    }
}
impl Eq for Item {}
impl PartialOrd for Item {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for Item {
    // inverted max-heap: smallest f first, then the most recent (depth-first on ties: far fewer
    // expansions on a grid, same optimal cost; deterministic)
    fn cmp(&self, o: &Self) -> Ordering {
        o.f.total_cmp(&self.f).then(self.seq.cmp(&o.seq))
    }
}

#[derive(Clone, Copy)]
struct State {
    node: Node,
    id: u32,
    dir: Option<Dir>,
    stage: usize,
}

/// Working state: cost, parent and start, valid only if `stamp` is the current generation (no
/// reset). Grouped: one memory access per state.
#[derive(Clone, Copy, Default)]
struct Slot {
    g: f64,
    parent: u64,
    src: u32,
    stamp: u32,
}

/// Working arrays reused from one search to the next.
#[derive(Default)]
pub(crate) struct Workspace {
    slots: Vec<Slot>,
    generation: u32,
    heap: BinaryHeap<Item>,
}

impl Workspace {
    fn begin(&mut self, size: usize) {
        if self.slots.len() < size {
            self.slots.resize(size, Slot::default());
        }
        self.generation = self.generation.wrapping_add(1);
        if self.generation == 0 {
            self.slots.iter_mut().for_each(|s| s.stamp = 0);
            self.generation = 1;
        }
    }
    fn get(&self, k: usize) -> Option<(f64, u64, u32)> {
        let s = &self.slots[k];
        (s.stamp == self.generation).then_some((s.g, s.parent, s.src))
    }
    fn set(&mut self, k: usize, g: f64, parent: u64, src: u32) {
        self.slots[k] = Slot {
            g,
            parent,
            src,
            stamp: self.generation,
        };
    }
}

fn dir_code(d: Option<Dir>) -> u64 {
    d.map_or(4, |d| d.index() as u64)
}

fn manhattan(a: (f64, f64), b: (f64, f64)) -> f64 {
    (a.0 - b.0).abs() + (a.1 - b.1).abs()
}

/// Root marker in the chain of parents.
const ROOT: u64 = u64::MAX;

/// Search heap with a sequence number (deterministic tie-breaking).
struct Frontier {
    heap: BinaryHeap<Item>,
    seq: u32,
}

impl Frontier {
    fn push(&mut self, f: f64, g: f64, key: u64, target: u32) {
        self.heap.push(Item {
            f,
            g,
            key,
            seq: self.seq,
            target,
        });
        self.seq = self.seq.wrapping_add(1);
    }
}

/// Immutable data of a search: query, indexing of the states, heuristic.
struct Astar<'a> {
    q: &'a Query<'a>,
    sc: &'a Scene,
    /// Number of nodes of the scene.
    nn: usize,
    /// Nombre de points de passage.
    ncp: usize,
    /// `rest[s]`: lower bound of the remaining cost from waypoint `s`.
    rest: Vec<f64>,
    /// Bend penalty (at least 0).
    bend_cost: f64,
}

impl<'a> Astar<'a> {
    fn new(q: &'a Query<'a>) -> Self {
        let mut a = Astar {
            q,
            sc: q.scene,
            nn: q.scene.n_nodes(),
            ncp: q.checkpoints.len(),
            rest: Vec::new(),
            bend_cost: q.params.segment_penalty.max(0.0),
        };
        a.rest = a.checkpoint_chain_costs();
        a
    }

    fn pos(&self, n: Node) -> (f64, f64) {
        (self.sc.xs[n.0], self.sc.ys[n.1])
    }

    /// Dense key of a state (stage, node, direction of arrival).
    fn key(&self, s: &State) -> u64 {
        ((s.stage * self.nn + s.id as usize) * 5) as u64 + dir_code(s.dir)
    }

    /// Inverse de [`Astar::key`].
    fn state_of(&self, key: u64) -> State {
        let id = ((key / 5) as usize % self.nn) as u32;
        State {
            node: self.sc.ij(id),
            id,
            dir: Dir::from_code(key % 5),
            stage: (key / 5) as usize / self.nn,
        }
    }

    /// Stage after passing through `n` (the waypoints are passed in order).
    fn advance(&self, mut stage: usize, n: Node) -> usize {
        while stage < self.ncp && self.q.checkpoints[stage] == n {
            stage += 1;
        }
        stage
    }

    /// Admissible heuristic: distance to the next waypoint, then the chain of the remaining points,
    /// then the nearest target.
    fn checkpoint_chain_costs(&self) -> Vec<f64> {
        let q = self.q;
        let mut rest = vec![0.0; self.ncp + 1];
        for s in (0..self.ncp).rev() {
            let here = self.pos(q.checkpoints[s]);
            rest[s] = if s + 1 < self.ncp {
                manhattan(here, self.pos(q.checkpoints[s + 1])) + rest[s + 1]
            } else {
                q.targets
                    .iter()
                    .map(|t| manhattan(here, self.pos(t.node)) + t.cost)
                    .fold(f64::INFINITY, f64::min)
            };
        }
        rest
    }

    /// Lower bound of the bends' cost to go from `p` (direction `d`) to `t`: one bend if `t` is not
    /// on the same line, or if it is not straight ahead.
    fn bends(&self, p: (f64, f64), d: Option<Dir>, t: (f64, f64)) -> f64 {
        let (dx, dy) = (t.0 - p.0, t.1 - p.1);
        if dx.abs() < 1e-9 && dy.abs() < 1e-9 {
            return 0.0;
        }
        if dx.abs() >= 1e-9 && dy.abs() >= 1e-9 {
            return self.bend_cost;
        }
        match d {
            None => 0.0,
            Some(d) => {
                let v = d.vector();
                if (v.0 * dx + v.1 * dy) > 0.0 && (v.0 * dy - v.1 * dx).abs() < 1e-9 {
                    0.0
                } else {
                    self.bend_cost
                }
            }
        }
    }

    fn heuristic(&self, n: Node, stage: usize, d: Option<Dir>) -> f64 {
        let p = self.pos(n);
        if stage < self.ncp {
            let c = self.pos(self.q.checkpoints[stage]);
            manhattan(p, c) + self.bends(p, d, c) + self.rest[stage]
        } else {
            self.q
                .targets
                .iter()
                .map(|t| {
                    let tp = self.pos(t.node);
                    manhattan(p, tp) + t.cost + self.bends(p, d, tp)
                })
                .fold(f64::INFINITY, f64::min)
        }
    }

    /// Puts the starts in the heap.
    fn seed(&self, ws: &mut Workspace, frontier: &mut Frontier) {
        for (si, s) in self.q.sources.iter().enumerate() {
            let Some(id) = self.sc.id(s.node.0, s.node.1) else {
                continue;
            };
            let st = State {
                node: s.node,
                id,
                dir: s.dir,
                stage: self.advance(0, s.node),
            };
            let k = self.key(&st);
            if ws.get(k as usize).is_none_or(|b| s.cost < b.0) {
                ws.set(k as usize, s.cost, ROOT, si as u32);
                let f = s.cost + self.heuristic(st.node, st.stage, st.dir);
                frontier.push(f, s.cost, k, NONE);
            }
        }
    }

    /// Path (nodes, start) going back up the chain of parents from `key`.
    fn reconstruct(&self, ws: &Workspace, key: u64) -> (Vec<Node>, usize) {
        let mut nodes = Vec::new();
        let mut k = key;
        let mut source = 0;
        while k != ROOT {
            let id = (k / 5) as usize % self.nn;
            nodes.push(self.sc.ij(id as u32));
            let Some(b) = ws.get(k as usize) else { break };
            source = b.2 as usize;
            k = b.1;
        }
        nodes.reverse();
        nodes.dedup();
        (nodes, source)
    }

    /// If the state is on a target (every stage passed), offers the arrival.
    fn offer_targets(&self, st: &State, g: f64, k: u64, frontier: &mut Frontier) {
        if st.stage != self.ncp {
            return;
        }
        for (ti, t) in self.q.targets.iter().enumerate() {
            if t.node != st.node {
                continue;
            }
            let extra = match (t.arrive, st.dir) {
                (None, _) | (_, None) => 0.0,
                (Some(a), Some(d)) if a == d => 0.0,
                (Some(a), Some(d)) if a == d.reverse() => continue,
                _ => self.q.params.segment_penalty,
            };
            frontier.push(g + extra + t.cost, g, k, ti as u32);
        }
    }

    /// Cost of the move from `st` to neighbour `next_id` in direction `nd`, or `None` if an active
    /// region blocks it.
    fn step_cost(&self, st: &State, nd: Dir, next_id: u32) -> Option<f64> {
        let params = self.q.params;
        let (blocked, in_buffer, in_regions) = self.sc.span(st.id, nd, next_id, self.q.active);
        if blocked {
            return None;
        }
        let len = manhattan(self.pos(st.node), self.pos(self.sc.ij(next_id)));
        let mut cost = len + in_buffer * params.buffer_cost + in_regions * params.region_cost;
        match st.dir {
            Some(d) if d != nd => cost += params.segment_penalty,
            Some(_) => {
                let (hc, vc) = self.q.through[st.id as usize];
                let crossed = if nd.horizontal() { vc } else { hc };
                if crossed > 0 {
                    cost += params.crossing_penalty * crossed as f64;
                }
            }
            None => {}
        }
        Some(cost)
    }

    /// Expands state `st` (cost `g`, key `k`): arrivals then neighbours.
    fn expand(&self, st: &State, g: f64, k: u64, ws: &mut Workspace, frontier: &mut Frontier) {
        self.offer_targets(st, g, k, frontier);
        let src = ws.get(k as usize).map_or(0, |b| b.2);
        for nd in Dir::ALL {
            if st.dir == Some(nd.reverse()) {
                continue;
            }
            let Some(next_id) = self.sc.next(st.id, nd) else {
                continue;
            };
            let Some(cost) = self.step_cost(st, nd, next_id) else {
                continue;
            };
            let next = self.sc.ij(next_id);
            let ns = State {
                node: next,
                id: next_id,
                dir: Some(nd),
                stage: self.advance(st.stage, next),
            };
            let ng = g + cost;
            let nk = self.key(&ns);
            if ws.get(nk as usize).is_none_or(|b| ng < b.0) {
                ws.set(nk as usize, ng, k, src);
                frontier.push(ng + self.heuristic(next, ns.stage, ns.dir), ng, nk, NONE);
            }
        }
    }
}

fn log_stats(message: std::fmt::Arguments) {
    if std::env::var_os("ROUTER_STATS").is_some() {
        eprintln!("{message}");
    }
}

pub(crate) fn search(q: &Query, ws: &mut Workspace) -> Outcome {
    let astar = Astar::new(q);
    ws.begin(astar.nn * 5 * (astar.ncp + 1));
    let mut frontier = Frontier {
        heap: std::mem::take(&mut ws.heap),
        seq: 0,
    };
    frontier.heap.clear();
    astar.seed(ws, &mut frontier);

    let (nx, ny) = (q.scene.xs.len(), q.scene.ys.len());
    let mut expanded = 0usize;
    let outcome = loop {
        let Some(it) = frontier.heap.pop() else {
            log_stats(format_args!(
                "grid {nx}×{ny}, {expanded} expansions (failed)"
            ));
            break Outcome::NoPath;
        };
        if it.target != NONE {
            let (nodes, source) = astar.reconstruct(ws, it.key);
            log_stats(format_args!(
                "grid {nx}×{ny}, {} nodes, {expanded} expansions",
                astar.nn
            ));
            break Outcome::Found(Found {
                nodes,
                source,
                target: it.target as usize,
            });
        }
        let st = astar.state_of(it.key);
        if ws.get(it.key as usize).is_some_and(|b| it.g > b.0) {
            continue;
        }
        expanded += 1;
        if expanded > q.params.max_expansions {
            break Outcome::Exhausted;
        }
        astar.expand(&st, it.g, it.key, ws, &mut frontier);
    };
    ws.heap = frontier.heap;
    outcome
}
