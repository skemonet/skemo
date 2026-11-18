//! One model for the whole slide, solved at once: one route per arrow, one place per label, the
//! hard rules between two lines and between labels and lines as conflicts, the preferences as one
//! objective in the constitution's order (crossings, then bends, then length; ports in the middle;
//! a label near the start of its line).
//!
//! The prototype solved it with CP-SAT; this is a search of our own (pure Rust, deterministic, the
//! same in WebAssembly): every line's cheapest route first, then the best move of one line, or of
//! two lines in conflict, while the whole scores better; then kicks (a few lines around the one
//! that pays most redrawn at random), each kept only when the whole scores better.

use super::grid::Route;
use super::labels::{Slot, too_close};
use super::pairs::{PairRes, Who, pair};
use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};

pub(crate) const W_CROSS: f64 = 1000.0;
/// An arrow left out: the slide cannot hold it (W2), reported.
pub(crate) const W_DROP: f64 = 1_000_000.0;
/// A label left out (W2).
pub(crate) const W_LDROP: f64 = 300_000.0;
/// Each label drawn (one per group of arrows saying the same thing, D-054).
const W_LABEL: f64 = 30.0;
/// A label said twice in one group (L3: carried once, on the shared part).
const W_DUP: f64 = 400.0;
/// Two lines breaking a hard rule together: dearer than leaving one out.
const HARD: f64 = 2.0 * W_DROP;
/// Kicks in a row without a better drawing before the search stops.
const PATIENCE: usize = 12;

/// A small fast hasher for the pair cache (keys are already well spread integers).
#[derive(Default)]
pub(crate) struct Fx(u64);
impl Hasher for Fx {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.write_u64(u64::from(b));
        }
    }
    fn write_u64(&mut self, v: u64) {
        self.0 = (self.0.rotate_left(5) ^ v).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
    }
}
type FxMap<K, V> = HashMap<K, V, BuildHasherDefault<Fx>>;

/// A deterministic random source (xorshift64*).
pub(crate) struct Rng(u64);
impl Rng {
    pub(crate) fn new(seed: u64) -> Rng {
        Rng(seed | 1)
    }
    pub(crate) fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }
    pub(crate) fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

/// An arrow as the model sees it.
pub(crate) struct Arrow {
    pub src: usize,
    pub dst: usize,
    pub look: String,
    pub label: Option<String>,
    /// Two facing cards: a bend on their line costs (H2.23 "neighbours linked straight").
    pub face: bool,
}

pub(crate) struct Problem<'a> {
    pub arrows: &'a [Arrow],
    pub routes: Vec<Vec<Route>>,
    /// Each arrow's places for its label, every candidate's together.
    pub slots: Vec<Vec<Slot>>,
    /// For each arrow and candidate, the places valid on it.
    pub valid: Vec<Vec<Vec<usize>>>,
    /// Groups of arrows drawn alike with one label sharing a card (a fork or a join).
    pub groups: Vec<Vec<usize>>,
    /// Width of the lattice's grid of nodes (a node key to an index).
    pub gw: i32,
    pub gn: usize,
    /// For each arrow's place, the routes of the other arrows that come too near it (arrow,
    /// candidate).
    blockers: Vec<Vec<Vec<(u16, u16)>>>,
    cache: FxMap<(u32, u32), PairRes>,
}

/// Two sorted lists of node keys share one.
fn meet(a: &[u32], b: impl Iterator<Item = u32>) -> bool {
    let mut i = 0;
    for k in b {
        while i < a.len() && a[i] < k {
            i += 1;
        }
        if i < a.len() && a[i] == k {
            return true;
        }
    }
    false
}

fn blockers(routes: &[Vec<Route>], slots: &[Vec<Slot>]) -> Vec<Vec<Vec<(u16, u16)>>> {
    let bb = |keys: &[u32]| {
        let pts = keys.iter().map(|&k| super::grid::unkey(k));
        pts.fold((i32::MAX, i32::MAX, i32::MIN, i32::MIN), |b, p| {
            (b.0.min(p.0), b.1.min(p.1), b.2.max(p.0), b.3.max(p.1))
        })
    };
    slots
        .iter()
        .enumerate()
        .map(|(k, ss)| {
            ss.iter()
                .map(|slot| {
                    let b = bb(&slot.near);
                    let mut out = Vec::new();
                    for (j, rs) in routes.iter().enumerate() {
                        if j == k {
                            continue;
                        }
                        for (d, r) in rs.iter().enumerate() {
                            let o = r.bbox;
                            if o.0 > b.2 || o.2 < b.0 || o.1 > b.3 || o.3 < b.1 {
                                continue;
                            }
                            if meet(&slot.near, r.keys.iter().map(|e| e.0)) {
                                out.push((j as u16, d as u16));
                            }
                        }
                    }
                    out
                })
                .collect()
        })
        .collect()
}

/// Neighbours linked straight (H2.23, rank 2, soft so that a slide where it cannot be is drawn).
const W_NEIGH: f64 = 500.0;

#[derive(Clone, Debug)]
pub(crate) struct Sol {
    pub pick: Vec<Option<usize>>,
    /// (arrow, slot) of each label drawn.
    pub labels: Vec<(usize, usize)>,
    pub ldropped: Vec<usize>,
    pub objective: f64,
}

impl<'a> Problem<'a> {
    pub(crate) fn new(
        arrows: &'a [Arrow],
        routes: Vec<Vec<Route>>,
        slots: Vec<Vec<Slot>>,
        valid: Vec<Vec<Vec<usize>>>,
        (gw, gn): (i32, usize),
    ) -> Problem<'a> {
        let groups = label_groups(arrows);
        let blockers = blockers(&routes, &slots);
        Problem {
            blockers,
            arrows,
            routes,
            slots,
            valid,
            groups,
            gw,
            gn,
            cache: FxMap::default(),
        }
    }

    fn who(&self, k: usize) -> Who<'_> {
        let a = &self.arrows[k];
        Who {
            look: &a.look,
            src: a.src,
            dst: a.dst,
            label: a.label.as_deref(),
        }
    }

    /// A route's own cost in the model.
    pub(crate) fn own(&self, k: usize, c: usize) -> f64 {
        let r = &self.routes[k][c];
        r.cost
            + if self.arrows[k].face && r.bends > 0 {
                W_NEIGH
            } else {
                0.0
            }
    }

    pub(crate) fn pair(&mut self, k: usize, c: usize, j: usize, d: usize) -> PairRes {
        let (a, b) = (((k << 16) | c) as u32, ((j << 16) | d) as u32);
        let key = if a < b { (a, b) } else { (b, a) };
        if let Some(r) = self.cache.get(&key) {
            return *r;
        }
        let r = pair(
            &self.routes[k][c],
            self.who(k),
            &self.routes[j][d],
            self.who(j),
        );
        self.cache.insert(key, r);
        r
    }

    fn pair_cost(&mut self, k: usize, c: usize, j: usize, d: usize) -> f64 {
        let r = self.pair(k, c, j, d);
        if r.bad {
            HARD
        } else {
            W_CROSS * f64::from(r.cross) + r.merge
        }
    }

    fn node(&self, key: u32) -> usize {
        let (x, y) = super::grid::unkey(key);
        (y * self.gw + x) as usize
    }
}

/// `{arrows}`: arrows drawn alike with one label, linked when they share their source (a fork)
/// or their target (a join), closed under both (a union-find).
fn label_groups(arrows: &[Arrow]) -> Vec<Vec<usize>> {
    fn find(par: &mut [usize], mut k: usize) -> usize {
        while par[k] != k {
            par[k] = par[par[k]];
            k = par[k];
        }
        k
    }
    let n = arrows.len();
    let mut par: Vec<usize> = (0..n).collect();
    let mut by: HashMap<(bool, usize, &str, &str), usize> = HashMap::new();
    for (k, a) in arrows.iter().enumerate() {
        let Some(label) = a.label.as_deref() else {
            continue;
        };
        for key in [
            (true, a.src, label, a.look.as_str()),
            (false, a.dst, label, a.look.as_str()),
        ] {
            if let Some(&first) = by.get(&key) {
                let (x, y) = (find(&mut par, k), find(&mut par, first));
                par[x] = y;
            } else {
                by.insert(key, k);
            }
        }
    }
    let mut groups: Vec<Vec<usize>> = Vec::new();
    let mut at: HashMap<usize, usize> = HashMap::new();
    for (k, a) in arrows.iter().enumerate().take(n) {
        if a.label.is_none() {
            continue;
        }
        let r = find(&mut par, k);
        let g = *at.entry(r).or_insert_with(|| {
            groups.push(Vec::new());
            groups.len() - 1
        });
        groups[g].push(k);
    }
    groups
}

/// The search's state: the choice of each line, the nodes each node's lines take, the costs.
struct State {
    pick: Vec<Option<usize>>,
    /// Per node, the arrows whose drawing runs there (bits).
    occ: Vec<u128>,
    route_part: f64,
    label_part: f64,
    labels: Vec<(usize, usize)>,
    ldropped: Vec<usize>,
    /// The nodes near a label drawn (sorted keys): a line moving clear of them, with no label
    /// of its own and none left out, leaves the labels as they are.
    lab_keys: Vec<u32>,
}

impl State {
    fn total(&self) -> f64 {
        self.route_part + self.label_part
    }
}

pub(crate) struct Search<'p, 'a> {
    pub p: &'p mut Problem<'a>,
    rng: Rng,
    /// Moves tried at most (deterministic effort, never the clock: the same drawing every time,
    /// and WebAssembly has no clock).
    budget: usize,
    spent: usize,
    relabels: std::cell::Cell<usize>,
}

impl<'p, 'a> Search<'p, 'a> {
    pub(crate) fn new(p: &'p mut Problem<'a>, budget: usize) -> Search<'p, 'a> {
        Search {
            p,
            rng: Rng::new(0x5eed_51de),
            budget,
            spent: 0,
            relabels: std::cell::Cell::new(0),
        }
    }

    fn set_occ(&self, occ: &mut [u128], k: usize, c: Option<usize>, on: bool) {
        let Some(c) = c else { return };
        let bit = 1u128 << k;
        for e in &self.p.routes[k][c].keys {
            let i = self.p.node(e.0);
            if on {
                occ[i] |= bit;
            } else {
                occ[i] &= !bit;
            }
        }
    }

    fn state(&mut self, pick: Vec<Option<usize>>) -> State {
        let mut occ = vec![0u128; self.p.gn];
        for (k, c) in pick.iter().enumerate() {
            self.set_occ(&mut occ, k, *c, true);
        }
        let mut s = State {
            pick,
            occ,
            route_part: 0.0,
            label_part: 0.0,
            labels: Vec::new(),
            ldropped: Vec::new(),
            lab_keys: Vec::new(),
        };
        s.route_part = self.route_part(&s.pick);
        self.relabel(&mut s);
        s
    }

    fn route_part(&mut self, pick: &[Option<usize>]) -> f64 {
        let mut t = 0.0;
        for k in 0..pick.len() {
            let Some(c) = pick[k] else {
                t += W_DROP;
                continue;
            };
            t += self.p.own(k, c);
            for (j, d) in pick.iter().enumerate().skip(k + 1) {
                if let Some(d) = *d {
                    t += self.p.pair_cost(k, c, j, d);
                }
            }
        }
        t
    }

    /// What changing line k to `c` changes in the routes' part.
    fn delta(&mut self, pick: &[Option<usize>], k: usize, c: Option<usize>) -> f64 {
        let one = |s: &mut Self, c: Option<usize>| -> f64 {
            let Some(c) = c else { return W_DROP };
            let mut t = s.p.own(k, c);
            for (j, d) in pick.iter().enumerate() {
                if j != k
                    && let Some(d) = d
                {
                    t += s.p.pair_cost(k, c, j, *d);
                }
            }
            t
        };
        one(self, c) - one(self, pick[k])
    }

    /// The labels for the routes drawn: group by group, the place that names the most of its
    /// lines at the least cost, clear of every other line and label; a line left unnamed pays.
    fn relabel(&self, s: &mut State) {
        self.relabels.set(self.relabels.get() + 1);
        let p = &*self.p;
        let mut chosen: Vec<(usize, usize)> = Vec::new();
        let mut cost = 0.0;
        let mut ldropped = Vec::new();
        for g in &p.groups {
            let members: Vec<usize> = g.iter().copied().filter(|&k| s.pick[k].is_some()).collect();
            if members.is_empty() {
                continue;
            }
            let mut opts: Vec<(usize, usize, u32, f64)> = Vec::new(); // arrow, slot, covers, cost
            for &k in &members {
                let c = s.pick[k].unwrap_or(0);
                for &si in &p.valid[k][c] {
                    let slot = &p.slots[k][si];
                    let mut cover = 0u32;
                    let mut allowed = 1u128 << k;
                    for (mi, &m) in members.iter().enumerate() {
                        if m == k || p.routes[m][s.pick[m].unwrap_or(0)].holds(&slot.seg) {
                            cover |= 1 << mi;
                            allowed |= 1u128 << m;
                        }
                    }
                    let hit = p.blockers[k][si].iter().any(|&(j, d)| {
                        allowed >> j & 1 == 0 && s.pick[j as usize] == Some(d as usize)
                    });
                    if !hit {
                        opts.push((k, si, cover, W_LABEL + f64::from(slot.along / 2)));
                    }
                }
            }
            let mut covered = 0u32;
            let mut said = 0;
            let full = (1u32 << members.len()) - 1;
            while covered != full {
                let mut best: Option<(f64, usize)> = None;
                for (i, &(k, si, cover, c)) in opts.iter().enumerate() {
                    let new = (cover & !covered).count_ones();
                    if new == 0 {
                        continue;
                    }
                    let bx = p.slots[k][si].bx;
                    if chosen.iter().any(|&(a, b)| too_close(p.slots[a][b].bx, bx)) {
                        continue;
                    }
                    let gain = f64::from(new) * W_LDROP - c - if said > 0 { W_DUP } else { 0.0 };
                    if best.is_none_or(|b| gain > b.0) {
                        best = Some((gain, i));
                    }
                }
                let Some((_, i)) = best else { break };
                let (k, si, cover, c) = opts[i];
                chosen.push((k, si));
                cost += c + if said > 0 { W_DUP } else { 0.0 };
                said += 1;
                covered |= cover;
            }
            for (mi, &m) in members.iter().enumerate() {
                if covered & (1 << mi) == 0 {
                    cost += W_LDROP;
                    ldropped.push(m);
                }
            }
        }
        let mut keys: Vec<u32> = chosen
            .iter()
            .flat_map(|&(k, si)| p.slots[k][si].near.iter().copied())
            .collect();
        keys.sort_unstable();
        keys.dedup();
        s.lab_keys = keys;
        s.labels = chosen;
        s.label_part = cost;
        s.ldropped = ldropped;
    }

    /// Moving line k from `old` to `c` may change the labels.
    fn touches_labels(&self, s: &State, k: usize, old: Option<usize>, c: Option<usize>) -> bool {
        if self.p.arrows[k].label.is_some() || !s.ldropped.is_empty() {
            return true;
        }
        let hit = |r: Option<usize>| {
            let Some(r) = r else { return false };
            let keys = &self.p.routes[k][r].keys;
            let (mut i, mut j) = (0, 0);
            while i < keys.len() && j < s.lab_keys.len() {
                match keys[i].0.cmp(&s.lab_keys[j]) {
                    std::cmp::Ordering::Less => i += 1,
                    std::cmp::Ordering::Greater => j += 1,
                    std::cmp::Ordering::Equal => return true,
                }
            }
            false
        };
        hit(old) || hit(c)
    }

    /// Line k drawn by `c` instead: the state updated (routes, nodes, labels).
    fn apply(&mut self, s: &mut State, k: usize, c: Option<usize>, d_route: f64) {
        let old = s.pick[k];
        self.set_occ(&mut s.occ, k, old, false);
        self.set_occ(&mut s.occ, k, c, true);
        let touch = self.touches_labels(s, k, old, c);
        s.pick[k] = c;
        s.route_part += d_route;
        if touch {
            self.relabel(s);
        }
    }

    /// The whole's score were line k drawn by `c` (the state left as it was).
    fn try_move(&mut self, s: &mut State, k: usize, c: Option<usize>, d_route: f64) -> f64 {
        let old = s.pick[k];
        if self.p.groups.is_empty() || !self.touches_labels(s, k, old, c) {
            return s.route_part + d_route + s.label_part;
        }
        let (rp, lp, labels, ld, lk) = (
            s.route_part,
            s.label_part,
            std::mem::take(&mut s.labels),
            std::mem::take(&mut s.ldropped),
            s.lab_keys.clone(),
        );
        self.apply(s, k, c, d_route);
        let t = s.total();
        self.set_occ(&mut s.occ, k, c, false);
        self.set_occ(&mut s.occ, k, old, true);
        s.pick[k] = old;
        s.route_part = rp;
        s.label_part = lp;
        s.labels = labels;
        s.ldropped = ld;
        s.lab_keys = lk;
        t
    }

    /// What each line pays beyond its cheapest route: its crossings, conflicts, a drop.
    fn pays(&mut self, s: &State) -> Vec<f64> {
        let n = s.pick.len();
        let mut out = vec![0.0; n];
        for (k, o) in out.iter_mut().enumerate() {
            let Some(c) = s.pick[k] else {
                *o = W_DROP;
                continue;
            };
            let best = (0..self.p.routes[k].len())
                .map(|i| self.p.own(k, i))
                .fold(f64::INFINITY, f64::min);
            *o = self.p.own(k, c) - best;
            for j in 0..n {
                if j != k
                    && let Some(d) = s.pick[j]
                {
                    *o += self.p.pair_cost(k, c, j, d) / 2.0;
                }
            }
        }
        for &k in &s.ldropped {
            out[k] += W_LDROP;
        }
        out
    }

    /// The best move of line k (or of k and a line it conflicts with); applied when it gains.
    fn improve(&mut self, s: &mut State, k: usize, pays: f64) -> bool {
        let n = self.p.routes[k].len();
        let mut opts: Vec<(f64, Option<usize>)> = (0..n)
            .map(Some)
            .chain([None])
            .filter(|&c| c != s.pick[k])
            .map(|c| (self.delta(&s.pick, k, c), c))
            .collect();
        self.spent += opts.len();
        opts.sort_by(|a, b| a.0.total_cmp(&b.0));
        let cur = s.total();
        let mut best: Option<(f64, Option<usize>, f64)> = None;
        for &(d, c) in opts.iter().take(4) {
            if d >= HARD / 2.0 {
                break;
            }
            let t = self.try_move(s, k, c, d);
            if t < cur - 1e-6 && best.is_none_or(|b| t < b.0) {
                best = Some((t, c, d));
            }
        }
        if let Some((_, c, d)) = best {
            self.apply(s, k, c, d);
            return true;
        }
        // two lines together only for a line that pays (a crossing, a hook, a detour, a label)
        pays > super::grid::BEND / 2.0 && self.pair_move(s, k)
    }

    /// Line k in conflict with line j: the best pair of routes for the two together.
    fn pair_move(&mut self, s: &mut State, k: usize) -> bool {
        let Some(c0) = s.pick[k] else { return false };
        let n = s.pick.len();
        let mut rivals = Vec::new();
        for j in 0..n {
            if j == k {
                continue;
            }
            if let Some(d) = s.pick[j] {
                let r = self.p.pair(k, c0, j, d);
                let (a, b) = (&self.p.arrows[k], &self.p.arrows[j]);
                // a line in conflict or crossing; or one sharing a card (a fork, a join, a trunk
                // and its label move together)
                if r.bad
                    || r.cross > 0
                    || a.src == b.src
                    || a.dst == b.dst
                    || a.src == b.dst
                    || a.dst == b.src
                {
                    rivals.push(j);
                }
            }
        }
        let cur = s.total();
        for j in rivals {
            let (nk, nj) = (
                self.p.routes[k].len().min(40),
                self.p.routes[j].len().min(40),
            );
            let mut combos: Vec<(f64, usize, usize)> = Vec::new();
            let pick0 = s.pick.clone();
            let Some(j0d) = s.pick[j] else { continue };
            // each line's change alone, then the pair's own term swapped: k at c, j at d
            let dks: Vec<f64> = (0..nk).map(|c| self.delta(&pick0, k, Some(c))).collect();
            let djs: Vec<f64> = (0..nj).map(|d| self.delta(&pick0, j, Some(d))).collect();
            self.spent += nk + nj;
            for (c, &dk) in dks.iter().enumerate() {
                if dk >= HARD / 2.0 {
                    continue;
                }
                for (d, &dj) in djs.iter().enumerate() {
                    let swap = self.p.pair_cost(j, d, k, c)
                        - self.p.pair_cost(j, d, k, c0)
                        - self.p.pair_cost(j, j0d, k, c)
                        + self.p.pair_cost(j, j0d, k, c0);
                    let t = s.route_part + dk + dj + swap;
                    if t < cur + 1000.0 {
                        combos.push((t, c, d));
                    }
                }
            }
            self.spent += nk * nj / 16;
            combos.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
            let (k0, j0) = (s.pick[k], s.pick[j]);
            for &(_, c, d) in combos.iter().take(6) {
                if (Some(c), Some(d)) == (k0, j0) {
                    continue;
                }
                let dk = self.delta(&s.pick, k, Some(c));
                self.apply(s, k, Some(c), dk);
                let dj = self.delta(&s.pick, j, Some(d));
                self.apply(s, j, Some(d), dj);
                if s.total() < cur - 1e-6 {
                    return true;
                }
                let dj = self.delta(&s.pick, j, j0);
                self.apply(s, j, j0, dj);
                let dk = self.delta(&s.pick, k, k0);
                self.apply(s, k, k0, dk);
            }
        }
        false
    }

    fn descend(&mut self, s: &mut State) {
        for _ in 0..40 {
            if self.spent > self.budget {
                return;
            }
            let pays = self.pays(s);
            let mut order: Vec<usize> = (0..s.pick.len()).collect();
            order.sort_by(|&a, &b| pays[b].total_cmp(&pays[a]).then(a.cmp(&b)));
            let mut any = false;
            for k in order {
                any |= self.improve(s, k, pays[k]);
            }
            if !any {
                return;
            }
        }
    }

    /// Lines near line k (their drawings' boxes close to its own).
    fn around(&self, s: &State, k: usize, size: usize) -> Vec<usize> {
        let bb = |j: usize| s.pick[j].map(|c| self.p.routes[j][c].bbox);
        let Some(b) = bb(k) else { return vec![k] };
        let mut near: Vec<(i32, usize)> = (0..s.pick.len())
            .filter(|&j| j != k)
            .filter_map(|j| {
                let o = bb(j)?;
                let dx = (o.0 - b.2).max(b.0 - o.2).max(0);
                let dy = (o.1 - b.3).max(b.1 - o.3).max(0);
                Some((dx + dy, j))
            })
            .collect();
        near.sort_unstable();
        let mut out = vec![k];
        out.extend(near.into_iter().take(size - 1).map(|t| t.1));
        out
    }

    /// The search from `start`: each line's cheapest route where none is given.
    pub(crate) fn run(&mut self, start: &[Option<usize>]) -> Sol {
        let pick: Vec<Option<usize>> = (0..self.p.routes.len())
            .map(|k| {
                start
                    .get(k)
                    .copied()
                    .flatten()
                    .or(if self.p.routes[k].is_empty() {
                        None
                    } else {
                        Some(0)
                    })
            })
            .collect();
        let mut s = self.state(pick);
        self.descend(&mut s);
        let mut best = (s.total(), s.pick.clone());
        let mut since = 0;
        while self.spent < self.budget && since < PATIENCE {
            let pays = self.pays(&s);
            let mut order: Vec<usize> = (0..pays.len()).filter(|&k| pays[k] > 1.0).collect();
            if order.is_empty() {
                break;
            }
            order.sort_by(|&a, &b| pays[b].total_cmp(&pays[a]).then(a.cmp(&b)));
            let target = order[self.rng.below(order.len().min(4))];
            let size = 2 + self.rng.below(5);
            for j in self.around(&s, target, size) {
                let n = self.p.routes[j].len();
                if n == 0 {
                    continue;
                }
                let c = Some(self.rng.below(n.min(8)));
                let d = self.delta(&s.pick, j, c);
                self.apply(&mut s, j, c, d);
            }
            self.descend(&mut s);
            if s.total() < best.0 - 0.5 {
                best = (s.total(), s.pick.clone());
                since = 0;
            } else {
                s = self.state(best.1.clone());
                since += 1;
            }
        }
        let mut s = self.state(best.1);
        self.settle(&mut s);
        super::trace(&format!(
            "search: {} spent, {} relabels, {} pairs",
            self.spent,
            self.relabels.get(),
            self.p.cache.len()
        ));
        if std::env::var("SKEMO_SLIDE_DEBUG").is_ok() {
            self.dump(&s);
        }
        Sol {
            pick: s.pick.clone(),
            labels: s.labels.clone(),
            ldropped: s.ldropped.clone(),
            objective: s.total(),
        }
    }

    /// Each line: its route, what it costs, its cheapest candidate and what holds that one back.
    fn dump(&mut self, s: &State) {
        let n = s.pick.len();
        for k in 0..n {
            let Some(c) = s.pick[k] else {
                eprintln!("  #{k} dropped");
                continue;
            };
            let r = &self.p.routes[k][c];
            eprintln!(
                "  #{k} {}->{} pick {c}/{} own {:.0} {:?}->{:?} bends {}",
                self.p.arrows[k].src,
                self.p.arrows[k].dst,
                self.p.routes[k].len(),
                self.p.own(k, c),
                r.sside,
                r.tside,
                r.bends
            );
            for b in 0..self.p.routes[k].len() {
                if b == c || self.p.own(k, b) >= self.p.own(k, c) {
                    continue;
                }
                let mut why = Vec::new();
                for j in 0..n {
                    if j == k {
                        continue;
                    }
                    if let Some(d) = s.pick[j] {
                        let pr = self.p.pair(k, b, j, d);
                        if pr.bad || pr.cross > 0 {
                            why.push(format!("#{j}{}", if pr.bad { "!" } else { "x" }));
                        }
                    }
                }
                let rb = &self.p.routes[k][b];
                eprintln!(
                    "      cand {b} own {:.0} {:?}->{:?} bends {} held by {}",
                    self.p.own(k, b),
                    rb.sside,
                    rb.tside,
                    rb.bends,
                    why.join(" ")
                );
            }
        }
    }

    /// No hard conflict left: the line in the most conflicts is left out (reported) until none is.
    fn settle(&mut self, s: &mut State) {
        loop {
            let n = s.pick.len();
            let mut count = vec![0; n];
            for k in 0..n {
                for j in k + 1..n {
                    if let (Some(c), Some(d)) = (s.pick[k], s.pick[j])
                        && self.p.pair(k, c, j, d).bad
                    {
                        count[k] += 1;
                        count[j] += 1;
                    }
                }
            }
            let Some(k) = (0..n)
                .filter(|&k| count[k] > 0)
                .max_by_key(|&k| (count[k], std::cmp::Reverse(k)))
            else {
                return;
            };
            let d = self.delta(&s.pick, k, None);
            self.apply(s, k, None, d);
        }
    }
}
