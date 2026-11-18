//! Room made where a line pays for the lack of it (P11): the first drawing on even gutters, then
//! one gutter wider or a row's cards lower under their title, kept only when the whole scores
//! better.

use super::draw::{EFFORT_FIRST, EFFORT_ROOM};
use super::grid::Route;
use super::lattice::Gaps;
use super::lattice::{Extra, Lattice, PAD};
use super::{Drawing, Engine, Pt, STEP, grid, pairs, solve, steps};
use std::collections::BTreeMap;

/// Room made in one place: a step (the golden set sets it)…
const ROOM: f64 = 10.0;
/// … and a step for each gutter of its kind left narrower (P11: room is made evenly).
const UNEVEN: f64 = 5.0;
/// Room is made two steps at a time: a track and its air.
const GROW_BY: i32 = 2;
/// A step of room on every gutter, in steps of length.
const GROW: f64 = 50.0;
/// Places to make room tried per line, and attempts in all.
const ROOM_TRIES: usize = 4;
const ATTEMPTS: usize = 24;
/// Links past which the first drawing starts from the widest gutters.
const PROBE_UPTO: usize = 18;
/// Attempts in a row without a kept change before the loop stops.
const STALL: usize = 4;

/// How a first drawing ranks: arrows left out, labels left out, a crossing, the width if none,
/// its score.
type Rank = (usize, usize, bool, usize, i64);

/// ROOM a step of room made, UNEVEN a step for each gutter of its kind left narrower (P11).
pub(super) fn room_cost(l: &Lattice) -> f64 {
    let ex = |m: &BTreeMap<usize, i32>, k: usize| f64::from(m.get(&k).copied().unwrap_or(0));
    let mut kinds: BTreeMap<(u8, bool), Vec<f64>> = BTreeMap::new();
    for k in 0..l.cols.saturating_sub(1) {
        kinds
            .entry((0, l.xout[k]))
            .or_default()
            .push(ex(&l.extra.x, k));
    }
    for k in 0..l.rows.saturating_sub(1) {
        kinds
            .entry((1, l.yout[k]))
            .or_default()
            .push(ex(&l.extra.y, k));
    }
    kinds.insert(
        (2, false),
        (0..l.rows).map(|r| ex(&l.extra.head, r)).collect(),
    );
    kinds
        .values()
        .map(|v| {
            let max = v.iter().copied().fold(0.0, f64::max);
            ROOM * v.iter().sum::<f64>() + UNEVEN * v.iter().map(|u| max - u).sum::<f64>()
        })
        .sum()
}

/// Where room was made between lattice `a` and lattice `b` (b only wider): (axis, at, steps),
/// every coordinate >= at in a moves by steps in b. A gutter grows in its middle; room under a
/// zone's title grows right above the cards, so that the frame and its title stay.
pub(super) fn inserts(a: &Lattice, b: &Lattice) -> Vec<(usize, i32, i32)> {
    let ex = |l: &Lattice, k: usize| l.extra.head.get(&k).copied().unwrap_or(0);
    let mut out = Vec::new();
    for k in 0..a.cols.saturating_sub(1) {
        let d = b.xgap[k] - a.xgap[k];
        if d != 0 {
            out.push((0, a.colx[k] + a.cw + a.xgap[k] / 2, d));
        }
    }
    let dh0 = ex(b, 0) - ex(a, 0);
    if dh0 != 0 {
        out.push((1, a.rowy[0], dh0));
    }
    for k in 0..a.rows.saturating_sub(1) {
        let dh = ex(b, k + 1) - ex(a, k + 1);
        let dg = b.ygap[k] - a.ygap[k] - dh;
        if dh != 0 {
            out.push((1, a.rowy[k + 1], dh));
        }
        if dg != 0 {
            let lo = a.rowy[k] + a.ch;
            let at = if a.yout[k] {
                let ft = a.rowy[k + 1] - a.zone_head - ex(a, k + 1);
                (lo + PAD + ft) / 2
            } else {
                lo + a.ygap[k] / 2
            };
            out.push((1, at, dg));
        }
    }
    out
}

pub(super) fn mapped(ins: &[(usize, i32, i32)], p: Pt) -> Pt {
    let dx: i32 = ins
        .iter()
        .filter(|t| t.0 == 0 && p.0 >= t.1)
        .map(|t| t.2)
        .sum();
    let dy: i32 = ins
        .iter()
        .filter(|t| t.0 == 1 && p.1 >= t.1)
        .map(|t| t.2)
        .sum();
    (p.0 + dx, p.1 + dy)
}

/// A move making room: `X(k)` the gutter after column k, `Y(k)` after row k, `Head(r)` the cards
/// of row r lower under their titles.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Move {
    X(usize),
    Y(usize),
    Head(usize),
}

fn grown(e: &Extra, mv: Move, by: i32) -> Extra {
    let mut e = e.clone();
    let (m, k) = match mv {
        Move::X(k) => (&mut e.x, k),
        Move::Y(k) => (&mut e.y, k),
        Move::Head(k) => (&mut e.head, k),
    };
    *m.entry(k).or_insert(0) += by;
    e
}

impl Engine<'_> {
    /// The gutters the labels ask for: a label between two neighbouring cards sits on the
    /// straight line between them, a step from each card and clear of the head.
    pub(super) fn needs(&self) -> Gaps {
        let base = Gaps {
            gin_x: 4,
            gin_y: 4,
            chan_x: 4,
            chan_y: 4,
        };
        let l = Lattice::new(self.inp, &self.tiling, base, &Extra::default(), self.walls);
        // a label of length `a` between two cards: a step from the card it leaves, the head
        // (18 px) before the card it reaches, centred on a node of the lattice
        let span = |a: f64| STEP * ((STEP + a / 2.0) / STEP).ceil() + a / 2.0 + 18.0;
        let mut need = base;
        let pos = &self.tiling.pos;
        for e in &self.inp.edges {
            if e.label.is_none() {
                continue;
            }
            let ((c1, r1), (c2, r2)) = (pos[e.from], pos[e.to]);
            let (w, h) = e.lsize;
            let same = self.inp.card_panel[e.from] == self.inp.card_panel[e.to];
            if r1 == r2 && c1.abs_diff(c2) == 1 {
                if !l.xout[c1.min(c2)] {
                    need.gin_x = need.gin_x.max(steps(span(w)));
                } else if same {
                    need.chan_x = need.chan_x.max(steps(span(w)) - 2 * PAD);
                } else {
                    need.chan_x = need.chan_x.max(steps(w + 2.0 * STEP));
                }
            }
            if c1 == c2 && r1.abs_diff(r2) == 1 {
                if !l.yout[r1.min(r2)] {
                    need.gin_y = need.gin_y.max(steps(span(h)));
                } else if same {
                    need.chan_y = need.chan_y.max(steps(span(h)) - PAD - l.zone_head);
                } else {
                    need.chan_y = need.chan_y.max(steps(h + 2.0 * STEP));
                }
            }
        }
        need
    }

    /// The first drawing: gutters alike on every gutter of a kind, the narrowest where all
    /// fits ([room-step], P11), wider while each width removes a crossing.
    fn first(&mut self) -> Option<Drawing> {
        let need = self.needs();
        let mut best: Option<(Rank, Drawing)> = None;
        // a dense slide (past PROBE_UPTO links) rarely fits the narrowest: the widest first,
        // then narrower while everything still fits (a narrower one cannot fit if this one did not)
        let wide = self.inp.edges.len() > PROBE_UPTO;
        let order: Vec<usize> = if wide {
            vec![3, 2, 1, 0]
        } else {
            vec![0, 1, 2, 3]
        };
        let mut prev_cr = None;
        for i in order {
            let d = self.draw(need.plus(i as i32), &Extra::default(), None, EFFORT_FIRST);
            let cr = d.crossings;
            let fits = d.dropped == 0 && d.ldropped == 0;
            let key = (
                d.dropped,
                d.ldropped,
                cr > 0,
                if cr == 0 { i } else { 0 },
                (d.objective + GROW * i as f64) as i64,
            );
            let better = best.as_ref().is_none_or(|b| key < b.0);
            if better {
                best = Some((key, d));
            }
            if wide {
                if !fits || !better {
                    break;
                }
            } else if fits && (cr == 0 || prev_cr.is_some_and(|p| cr >= p)) {
                break;
            }
            prev_cr = Some(cr);
        }
        best.map(|b| b.1)
    }

    /// What each line pays for want of room, the most first.
    fn paying(&self, d: &Drawing) -> Vec<usize> {
        let mut pays: Vec<(f64, usize)> = Vec::new();
        let refs: Vec<(usize, &Route)> = d
            .routes
            .iter()
            .enumerate()
            .filter_map(|(k, r)| r.as_ref().map(|r| (k, r)))
            .collect();
        for k in 0..d.routes.len() {
            let Some(r) = &d.routes[k] else {
                pays.push((solve::W_DROP, k));
                continue;
            };
            // what the constitution calls a defect, not a line merely longer than it could be
            let e = &self.inp.edges[k];
            let (s, t) = (d.l.card[e.from], d.l.card[e.to]);
            let mut p = 0.0;
            let cr: usize = refs
                .iter()
                .filter(|(j, _)| *j != k)
                .map(|(_, q)| pairs::count_crossings(&[r, q]))
                .sum();
            p += solve::W_CROSS / 2.0 * cr as f64;
            if grid::away(r.sside, s, t) || grid::away(r.tside, t, s) {
                p += grid::W_HOOK;
            }
            if r.bends >= 3 || (self.arrows[k].face && r.bends > 0) {
                p += grid::BEND * r.bends as f64;
            }
            if r.cost
                > grid::BEND * r.bends as f64 + r.nodes.len() as f64 * 2.0 + grid::W_BAND * 3.0
            {
                p += grid::BEND; // a title band, a foreign group, a line hugging the cards
            }
            if e.label.is_some() && !d.labels.iter().any(|(a, _)| *a == k) {
                p += solve::W_LDROP;
            }
            pays.push((p, k));
        }
        pays.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
        pays.into_iter()
            .filter(|t| t.0 > grid::BEND * 1.5)
            .map(|t| t.1)
            .collect()
    }

    /// Where room would help line k: the title bands its drawing runs along (the cards lowered,
    /// "il suffit de décaler les cartes vers le bas"), then the gutters near the other lines its
    /// drawing touches, then every gutter it spans.
    fn where_room(&self, d: &Drawing, k: usize) -> Vec<Move> {
        let l = &d.l;
        let mut votes: BTreeMap<Move, usize> = BTreeMap::new();
        let gx = |x: i32| l.gutter_x(x).map(Move::X);
        let gy = |y: i32| {
            l.gutter_y(y)
                .map(|(head, r)| if head { Move::Head(r) } else { Move::Y(r) })
        };
        let Some(r) = &d.routes[k] else {
            return Vec::new();
        };
        let band = grid::title_bands(l);
        let mut front = Vec::new();
        for (i, &p) in r.nodes.iter().enumerate() {
            let level = i > 0
                && i + 1 < r.nodes.len()
                && r.nodes[i - 1].1 == p.1
                && r.nodes[i + 1].1 == p.1;
            if level
                && band(p)
                && let Some(m @ Move::Head(_)) = gy(p.1)
                && !front.contains(&m)
            {
                front.push(m);
            }
        }
        for (j, q) in d.routes.iter().enumerate() {
            let Some(q) = q else { continue };
            if j == k {
                continue;
            }
            for &p in &r.nodes {
                let close = [(0, 0), (1, 0), (-1, 0), (0, 1), (0, -1)]
                    .iter()
                    .any(|&(dx, dy)| q.has((p.0 + dx, p.1 + dy)));
                if close {
                    for m in [gx(p.0), gy(p.1)].into_iter().flatten() {
                        *votes.entry(m).or_default() += 1;
                    }
                }
            }
        }
        let mut out = front;
        let mut v: Vec<(usize, Move)> = votes.into_iter().map(|(m, n)| (n, m)).collect();
        v.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        for (_, m) in v {
            if !out.contains(&m) {
                out.push(m);
            }
        }
        let (x0, y0, x1, y1) = r.bbox;
        let spans = (x0..=x1).filter_map(gx).chain((y0..=y1).filter_map(gy));
        for m in spans {
            if !out.contains(&m) {
                out.push(m);
            }
        }
        out.truncate(ROOM_TRIES);
        out
    }

    /// The first drawing, then room made where a line pays for the lack of it, each change kept
    /// only when the whole slide scores better (a ratchet).
    pub(super) fn run(&mut self) -> Option<Drawing> {
        let mut cur = self.first()?;
        let mut tried: Vec<(usize, Move, Extra)> = Vec::new();
        let mut attempts = 0;
        let mut failed = 0;
        // room tried in fewer places on a dense slide: each attempt redraws it whole
        let limit = (240 / self.inp.edges.len().max(1)).clamp(4, ATTEMPTS);
        'outer: while attempts < limit {
            for k in self.paying(&cur) {
                for mv in self.where_room(&cur, k) {
                    let key = (k, mv, cur.l.extra.clone());
                    if tried.contains(&key) {
                        continue;
                    }
                    tried.push(key);
                    // a track and its air; then two, as a label across a gutter may need
                    for by in [GROW_BY, 2 * GROW_BY] {
                        attempts += 1;
                        let extra = grown(&cur.l.extra, mv, by);
                        let d = self.draw(cur.l.gaps, &extra, Some(&cur), EFFORT_ROOM);
                        if d.score() < cur.score() - 0.5 && d.dropped <= cur.dropped {
                            cur = d;
                            failed = 0;
                            continue 'outer;
                        }
                        failed += 1;
                        if attempts >= limit || failed >= STALL {
                            break 'outer;
                        }
                    }
                }
            }
            break;
        }
        Some(cur)
    }
}
