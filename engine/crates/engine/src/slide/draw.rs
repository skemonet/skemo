//! The whole slide drawn on one lattice: each arrow's candidates (its own, carried from the
//! drawing before where room was made away from it, sharing a sibling's trunk, found round the
//! others), its label's places, then the model solved.

use super::grid::{self, Arrow as Search, Grid, Route};
use super::lattice::{Extra, Gaps, Lattice};
use super::room::{inserts, mapped};
use super::solve::{self, Problem};
use super::{Drawing, Engine, Input, labels, pairs, trace};

/// The search's effort: moves for a first drawing, for an attempt.
pub(super) const EFFORT_FIRST: usize = 150_000;
pub(super) const EFFORT_ROOM: usize = 60_000;
/// Candidates kept per arrow, slid variants besides.
const KEEP: usize = 24;
const SLIDES: usize = 8;
/// Routes kept per line that pays, found round the others.
const AWARE_KEEP: usize = 8;
/// Siblings whose routes a fork's or a join's line may share a trunk with.
const SIB_MATES: usize = 4;
/// Each arrow's new routes added to its candidates (a route already there is left out).
fn merge_new(routes: &mut [Vec<Route>], new: Vec<Vec<Route>>) {
    for (k, f) in new.into_iter().enumerate() {
        for r in f {
            if !routes[k].iter().any(|x| x.nodes == r.nodes) {
                routes[k].push(r);
            }
        }
    }
}

/// Each arrow's places for its label, every candidate's together, and the places valid on each
/// candidate (a place is valid on every candidate that runs under it).
fn slots_for(
    cache: &mut labels::SlotCache,
    l: &Lattice,
    routes: &[Vec<Route>],
    inp: &Input,
) -> (Vec<Vec<labels::Slot>>, Vec<Vec<Vec<usize>>>) {
    let mut slots = Vec::new();
    let mut valid = Vec::new();
    for (k, e) in inp.edges.iter().enumerate() {
        let mut uniq: Vec<labels::Slot> = Vec::new();
        let mut per = Vec::new();
        for r in &routes[k] {
            let mut v = Vec::new();
            if e.label.is_some() {
                for s in cache.slots(l, r, k, e.lsize) {
                    let i = if let Some(i) = uniq.iter().position(|u| {
                        (u.bx.x - s.bx.x).abs() < 0.05 && (u.bx.y - s.bx.y).abs() < 0.05
                    }) {
                        if s.along < uniq[i].along {
                            uniq[i] = s;
                        }
                        i
                    } else {
                        uniq.push(s);
                        uniq.len() - 1
                    };
                    v.push(i);
                }
            }
            per.push(v);
        }
        for (ci, r) in routes[k].iter().enumerate() {
            for (i, u) in uniq.iter().enumerate() {
                if !per[ci].contains(&i) && r.holds(&u.seg) {
                    per[ci].push(i);
                }
            }
        }
        slots.push(uniq);
        valid.push(per);
    }
    (slots, valid)
}

impl Engine<'_> {
    /// The whole slide drawn on the lattice of these gutters, starting from `from` (its routes
    /// carried onto this lattice where room was made).
    pub(super) fn draw(
        &mut self,
        gaps: Gaps,
        extra: &Extra,
        from: Option<&Drawing>,
        effort: usize,
    ) -> Drawing {
        let inp = self.inp;
        let l = Lattice::new(inp, &self.tiling, gaps, extra, self.walls);
        trace("lattice");
        let g = Grid::new(inp, &l);
        trace("grid");
        for (k, e) in inp.edges.iter().enumerate() {
            self.arrows[k].face = grid::facing(inp, &l, e);
        }
        let (mut routes, start) = self.own_routes(&g, &l, from);
        trace("own routes");
        self.add_siblings(&g, &l, &mut routes);
        let start = self.written(&g, &l, &mut routes, start);
        trace("routes");
        let mut cache = labels::SlotCache::new(&l);
        let (slots, valid) = slots_for(&mut cache, &l, &routes, inp);
        trace(&format!(
            "candidates {gaps:?} {extra:?}: {}",
            routes.iter().map(Vec::len).sum::<usize>()
        ));
        let gn = (g.w * g.h) as usize;
        let mut p = Problem::new(&self.arrows, routes, slots, valid, (g.w, gn));
        let sol = Self::solve_from(&mut p, &start, from.is_some(), effort);
        let sol = self.aware_round(&g, &l, &mut p, &mut cache, sol, effort);
        let chosen: Vec<Option<Route>> = sol
            .pick
            .iter()
            .enumerate()
            .map(|(k, c)| c.map(|c| p.routes[k][c].clone()))
            .collect();
        trace(&format!("solved: {:.0}", sol.objective));
        let refs: Vec<&Route> = chosen.iter().flatten().collect();
        let crossings = pairs::count_crossings(&refs);
        let labels = sol
            .labels
            .iter()
            .map(|&(k, s)| (k, p.slots[k][s].bx))
            .collect();
        Drawing {
            cands: p.routes.clone(),
            dropped: chosen.iter().filter(|r| r.is_none()).count(),
            ldropped: sol.ldropped.len(),
            l,
            routes: chosen,
            labels,
            crossings,
            objective: sol.objective,
        }
    }

    /// Each arrow's own candidates and the one it starts from (its route in `from`, carried).
    fn own_routes(
        &self,
        g: &Grid,
        l: &Lattice,
        from: Option<&Drawing>,
    ) -> (Vec<Vec<Route>>, Vec<Option<usize>>) {
        let inp = self.inp;
        let keep = (600 / inp.edges.len().max(1)).clamp(6, KEEP);
        let ins = from.map(|d| inserts(&d.l, l));
        let mut routes = Vec::new();
        let mut start = Vec::new();
        for (k, e) in inp.edges.iter().enumerate() {
            let s = Search::of(g, inp, l, e);
            let carry = |r: &Route, ins: &[(usize, i32, i32)]| {
                let nodes = grid::dense(&r.pts.iter().map(|&p| mapped(ins, p)).collect::<Vec<_>>());
                s.route(nodes, r.sside, r.tside)
            };
            // room made away from both its cards changes nothing of an arrow's choices but where
            // they lie: its candidates are carried; otherwise searched again
            let near = match (from, &ins) {
                (Some(d), Some(ins)) => {
                    let (a, b) = (d.l.end(inp, e, true), d.l.end(inp, e, false));
                    ins.iter().any(|&(ax, at, _)| {
                        let (lo, hi) = if ax == 0 {
                            (a.x.min(b.x), (a.x + a.w).max(b.x + b.w))
                        } else {
                            (a.y.min(b.y), (a.y + a.h).max(b.y + b.h))
                        };
                        lo - 4 <= at && at <= hi + 4
                    })
                }
                _ => true,
            };
            let mut cands: Vec<Route> = match (from, &ins) {
                (Some(d), Some(ins)) if !near => {
                    d.cands[k].iter().filter_map(|r| carry(r, ins)).collect()
                }
                _ => {
                    let mut c = s.candidates(3, 2, keep, &|_| 0.0);
                    let slid = s.slides(&c, SLIDES);
                    c.extend(slid);
                    c
                }
            };
            let mut first = None;
            if let (Some(d), Some(ins)) = (from, &ins)
                && let Some(c) = d.routes[k].as_ref().and_then(|r| carry(r, ins))
            {
                first = Some(
                    if let Some(i) = cands.iter().position(|x| x.nodes == c.nodes) {
                        i
                    } else {
                        cands.push(c);
                        cands.len() - 1
                    },
                );
            }
            start.push(first);
            routes.push(cands);
        }
        (routes, start)
    }

    /// The arrows' written ways: an arrow with waypoints (`via`) takes the route through them,
    /// alone; one with sides (`side`) keeps the routes on them, if any. The start routes follow.
    fn written(
        &self,
        g: &Grid,
        l: &Lattice,
        routes: &mut [Vec<Route>],
        start: Vec<Option<usize>>,
    ) -> Vec<Option<usize>> {
        let inp = self.inp;
        let mut out = start;
        for (k, e) in inp.edges.iter().enumerate() {
            if e.via.is_empty() && e.sides == (None, None) {
                continue;
            }
            let was = out[k]
                .and_then(|i| routes[k].get(i))
                .map(|r| r.nodes.clone());
            if !e.via.is_empty()
                && let Some(r) = Search::of(g, inp, l, e).through(&e.via, e.sides)
            {
                routes[k] = vec![r];
            } else {
                let keep: Vec<Route> = routes[k]
                    .iter()
                    .filter(|r| {
                        e.sides.0.is_none_or(|s| s == r.sside)
                            && e.sides.1.is_none_or(|t| t == r.tside)
                    })
                    .cloned()
                    .collect();
                if !keep.is_empty() {
                    routes[k] = keep;
                }
            }
            out[k] = was.and_then(|n| routes[k].iter().position(|r| r.nodes == n));
        }
        out
    }

    /// Routes sharing a trunk with a sibling (a fork or a join drawn alike with one label).
    fn add_siblings(&self, g: &Grid, l: &Lattice, routes: &mut [Vec<Route>]) {
        let inp = self.inp;
        let mut sib: Vec<Vec<Route>> = vec![Vec::new(); inp.edges.len()];
        for (k, e) in inp.edges.iter().enumerate() {
            let s = Search::of(g, inp, l, e);
            for join in [true, false] {
                let mates: Vec<&Route> = inp
                    .edges
                    .iter()
                    .enumerate()
                    .filter(|&(j, f)| {
                        let alike = (e.look == f.look && e.label == f.label)
                            || (e.join.is_some() && e.join == f.join);
                        j != k && alike && if join { e.to == f.to } else { e.from == f.from }
                    })
                    .take(SIB_MATES)
                    .flat_map(|(j, _)| routes[j].iter().take(3))
                    .collect();
                if !mates.is_empty() {
                    sib[k].extend(s.siblings(&mates, join, 3));
                }
            }
        }
        merge_new(routes, sib);
    }

    /// The model solved from `start`, and from a fresh start too when room was made: the
    /// drawing carried from before may hold the lines where the room asks them to move.
    fn solve_from(
        p: &mut Problem,
        start: &[Option<usize>],
        carried: bool,
        effort: usize,
    ) -> solve::Sol {
        let mut sol = solve::Search::new(p, effort).run(start);
        if carried {
            let fresh = solve::Search::new(p, effort / 2).run(&[]);
            if fresh.objective < sol.objective {
                sol = fresh;
            }
        }
        sol
    }

    /// The lines that pay get routes found round the others as they are drawn (a hook, a
    /// crossing, a bend between facing cards, a line or a label left out), then the whole is
    /// solved again from `sol`.
    fn aware_round<'s>(
        &'s self,
        g: &Grid,
        l: &Lattice,
        p: &mut Problem<'s>,
        cache: &mut labels::SlotCache,
        sol: solve::Sol,
        effort: usize,
    ) -> solve::Sol {
        let fresh = self.aware(g, l, p, &sol);
        if fresh.iter().all(Vec::is_empty) {
            return sol;
        }
        let mut routes = std::mem::take(&mut p.routes);
        merge_new(&mut routes, fresh);
        let (slots, valid) = slots_for(cache, l, &routes, self.inp);
        *p = Problem::new(
            &self.arrows,
            routes,
            slots,
            valid,
            (g.w, (g.w * g.h) as usize),
        );
        let again = solve::Search::new(p, effort / 2).run(&sol.pick);
        let sol = if again.objective <= sol.objective {
            again
        } else {
            sol
        };
        trace(&format!("aware: {:.0}", sol.objective));
        sol
    }

    /// Routes found round the other lines as drawn, for each line that pays.
    fn aware(&self, g: &Grid, l: &Lattice, p: &Problem, sol: &solve::Sol) -> Vec<Vec<Route>> {
        let inp = self.inp;
        let n = inp.edges.len();
        let chosen: Vec<Option<&Route>> = sol
            .pick
            .iter()
            .enumerate()
            .map(|(k, c)| c.map(|c| &p.routes[k][c]))
            .collect();
        let mut out = vec![Vec::new(); n];
        for k in 0..n {
            let e = &inp.edges[k];
            // an arrow through waypoints keeps its one route
            if !e.via.is_empty() {
                continue;
            }
            let (s, t) = (l.end(inp, e, true), l.end(inp, e, false));
            let pays = match chosen[k] {
                None => true,
                Some(r) => {
                    grid::away(r.sside, s, t)
                        || grid::away(r.tside, t, s)
                        || (self.arrows[k].face && r.bends > 0)
                        || sol.ldropped.contains(&k)
                        || chosen.iter().enumerate().any(|(j, q)| {
                            j != k && q.is_some_and(|q| pairs::count_crossings(&[r, q]) > 0)
                        })
                }
            };
            if !pays {
                continue;
            }
            let mut pen = vec![0.0f64; g.block.len()];
            for (j, q) in chosen.iter().enumerate() {
                if j == k {
                    continue;
                }
                if let Some(q) = q {
                    for &v in &q.nodes {
                        if g.inside(v) {
                            pen[g.idx(v)] = grid::P_LINE;
                        }
                    }
                }
            }
            for &(j, si) in &sol.labels {
                if j == k {
                    continue;
                }
                for &key in &p.slots[j][si].near {
                    let v = grid::unkey(key);
                    if g.inside(v) {
                        pen[g.idx(v)] = grid::P_LINE;
                    }
                }
            }
            let search = Search::of(g, inp, l, e);
            out[k] = search.candidates(3, 1, AWARE_KEEP, &|m| pen[m]);
        }
        out
    }
}
