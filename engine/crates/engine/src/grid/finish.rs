//! The rules on finished routes, applied after the router and the shared finishing passes.
//!
//! - **Straight between neighbours**: a Z between two cards facing each other, offset only because
//!   the ports of one side are spread, becomes straight: the end on the side with fewer links moves
//!   in line with the other.
//! - **A tail behind every head**: an arrow's last segment is at least [`TAIL`] long, so that the
//!   line visibly carries on behind its head; a shorter one gets its previous segment moved back,
//!   and the line of another arrow that stands in the way moves on with it.
//! - **An L for a squashed Z**: a Z still too short for a tail leaves its source by the side
//!   facing the target.

use super::{cards, cuts, on};
use crate::labels::LANE;
use crate::layout::Box2;
use crate::route::Points;
use skemo_lang::{Kind, Model};
use std::collections::BTreeMap;

type Routes = BTreeMap<String, Points>;

/// Shortest last segment: the head (8 px) and a visible tail behind it.
pub(super) const TAIL: f64 = 24.0;
/// Least distance of a line from a card it does not touch (the router's margin).
pub(super) const MARGIN: f64 = 14.0;
/// Least distance of a port from a corner, and between two ports of one side.
pub(super) const PORT_ROOM: f64 = 12.0;
/// Two lines of different arrows closer than this, side by side over more than [`CLOSE_LEN`],
/// read as one thick line (the ledger's `close-lines`).
const CLOSE: f64 = 6.0;
const CLOSE_LEN: f64 = 8.0;
/// A line closer than this to a zone's border, over more than [`HUG_LEN`], runs along it (the
/// ledger's `hugging`).
const HUG: f64 = 10.0;
const HUG_LEN: f64 = 20.0;

/// The zones' titles and tag pills: no rule brings a line onto one.
type Titles<'a> = &'a [(String, Box2)];

/// The rules, in order: straight lines first (they remove bends), then the tails, then an L for
/// a Z still too short for a tail.
pub(crate) fn finish(
    model: &Model,
    boxes: &BTreeMap<String, Box2>,
    titles: Titles,
    routes: &mut BTreeMap<String, Points>,
) {
    if on("grid-straighten") {
        straighten(model, boxes, titles, routes);
        crate::trace::routes("grid-straighten", routes);
    }
    if on("grid-tails") {
        tails(model, boxes, titles, routes);
        crate::trace::routes("grid-tails", routes);
    }
    if on("grid-squashed-z") {
        squashed_z(model, boxes, titles, routes);
        crate::trace::routes("grid-squashed-z", routes);
    }
}

/// How many titles the line `pts` runs over: a rule may leave a line on a title the router put
/// it on, never bring one there.
fn on_titles(pts: &[(f64, f64)], titles: Titles) -> usize {
    titles
        .iter()
        .filter(|(_, t)| pts.windows(2).any(|s| cuts(s[0], s[1], t, 0.0)))
        .count()
}

/// A route as its bends: the points that repeat one another and those in line with their
/// neighbours dropped (a port's stub drawn as a segment of its own).
fn bends(pts: &[(f64, f64)]) -> Points {
    let mut out: Points = Vec::new();
    for &p in pts {
        if out
            .last()
            .is_some_and(|l| (l.0 - p.0).abs() + (l.1 - p.1).abs() < 0.5)
        {
            continue;
        }
        if let [.., a, b] = out[..] {
            let upright = (a.0 - b.0).abs() < 0.5 && (b.0 - p.0).abs() < 0.5;
            let level = (a.1 - b.1).abs() < 0.5 && (b.1 - p.1).abs() < 0.5;
            if upright || level {
                out.pop();
            }
        }
        out.push(p);
    }
    out
}

/// A straight segment as a run: is it upright, where it stands across, where it starts and
/// ends along. None for a slanted one or one of no length.
fn run(s: &[(f64, f64)]) -> Option<(bool, f64, f64, f64)> {
    let (p, q) = (s[0], s[1]);
    if (p.0 - q.0).abs() < 1e-9 && (p.1 - q.1).abs() > 0.5 {
        Some((true, p.0, p.1.min(q.1), p.1.max(q.1)))
    } else if (p.1 - q.1).abs() < 1e-9 && (p.0 - q.0).abs() > 0.5 {
        Some((false, p.1, p.0.min(q.0), p.0.max(q.0)))
    } else {
        None
    }
}

/// How tangled the line `id` is with the others: the lines it crosses, and the stretches of
/// other arrows it runs on or beside, too close to read as two lines.
fn tangles(routes: &Routes, id: &str) -> usize {
    let Some(r) = routes.get(id) else {
        return 0;
    };
    let others = || {
        routes
            .iter()
            .filter(|(k, _)| k.as_str() != id)
            .map(|(_, o)| o)
    };
    let beside = |a: &[(f64, f64)], b: &[(f64, f64)]| match (run(a), run(b)) {
        (Some(a), Some(b)) => {
            a.0 == b.0 && (a.1 - b.1).abs() < CLOSE && a.3.min(b.3) - a.2.max(b.2) > CLOSE_LEN
        }
        _ => false,
    };
    let close: usize = others()
        .map(|o| {
            r.windows(2)
                .flat_map(|a| o.windows(2).map(move |b| (a, b)))
                .filter(|(a, b)| beside(a, b))
                .count()
        })
        .sum();
    close + super::lanes::crossings(r, others())
}

/// The frames of the visible zones.
fn frames(model: &Model, boxes: &BTreeMap<String, Box2>) -> Vec<Box2> {
    model
        .nodes
        .iter()
        .filter(|n| n.kind == Kind::Zone && !crate::layout::invisible(n))
        .filter_map(|n| boxes.get(&n.id).copied())
        .collect()
}

/// Would the segment `s`, moved across by `by`, pass a border of one of `frames` that it runs
/// along, or stand closer than [`HUG`] before one?
fn meets_border(s: &[(f64, f64)], by: f64, frames: &[Box2]) -> bool {
    let Some((upright, at, lo, hi)) = run(s) else {
        return false;
    };
    // how far a border stands ahead of the segment, the way it moves
    let ahead = |e: f64| (e - at) * by.signum();
    frames.iter().any(|z| {
        let (e0, e1, s0, s1) = if upright {
            (z.x, z.right(), z.y, z.bottom())
        } else {
            (z.y, z.bottom(), z.x, z.right())
        };
        hi.min(s1) - lo.max(s0) >= HUG_LEN
            && [e0, e1]
                .iter()
                .any(|e| ahead(*e) > -0.5 && ahead(*e) < by.abs() + HUG)
    })
}

/// Where the free gap beside the source ends, from `start` toward the target's port `c`: the
/// nearest card other than the arrow's `ends` standing ahead on that line.
fn gap_end(
    all: &[(&str, Box2)],
    ends: (&str, &str),
    (start, c): ((f64, f64), (f64, f64)),
    vertical_tail: bool,
) -> Option<f64> {
    all.iter()
        .filter(|(k, _)| *k != ends.0 && *k != ends.1)
        .filter_map(|(_, cb)| {
            if vertical_tail {
                let toward = c.0 > start.0;
                let ahead = if toward {
                    cb.x >= start.0
                } else {
                    cb.right() <= start.0
                };
                ((cb.y..=cb.bottom()).contains(&start.1) && ahead).then_some(if toward {
                    cb.x
                } else {
                    cb.right()
                })
            } else {
                let toward = c.1 > start.1;
                let ahead = if toward {
                    cb.y >= start.1
                } else {
                    cb.bottom() <= start.1
                };
                ((cb.x..=cb.right()).contains(&start.0) && ahead).then_some(if toward {
                    cb.y
                } else {
                    cb.bottom()
                })
            }
        })
        .min_by(|a, b| {
            let s0 = if vertical_tail { start.0 } else { start.1 };
            (a - s0).abs().total_cmp(&(b - s0).abs())
        })
}

/// A Z whose last segment is still shorter than [`TAIL`] (two cards close along one axis, the
/// target off to the side) becomes an L: it leaves the source by the side facing the target and
/// enters the target where it did, with a tail as long as the gap allows.
fn squashed_z(model: &Model, boxes: &BTreeMap<String, Box2>, titles: Titles, routes: &mut Routes) {
    let all = cards(model, boxes);
    let ids: Vec<String> = routes.keys().cloned().collect();
    for id in ids {
        let Some(e) = model.edges.iter().find(|e| e.id == id) else {
            continue;
        };
        let (Some(fb), Some(tb)) = (boxes.get(&e.from), boxes.get(&e.to)) else {
            continue;
        };
        // a Z for good: a port's stub in line with the next segment is no bend
        let pts = &bends(&routes[&id]);
        if pts.len() != 4 {
            continue;
        }
        let (b, c) = (pts[2], pts[3]);
        let tail = (c.0 - b.0).abs() + (c.1 - b.1).abs();
        if tail >= TAIL - 0.5 {
            continue;
        }
        // the target entered from above or below: the source leaves by its left or right side,
        // level with its middle; from the left or the right: by its top or bottom, at its centre
        let vertical_tail = (c.0 - b.0).abs() < 0.5;
        let start = if vertical_tail {
            let x = if c.0 > fb.right() { fb.right() } else { fb.x };
            (x, fb.y + fb.h / 2.0)
        } else {
            let y = if c.1 > fb.bottom() { fb.bottom() } else { fb.y };
            (fb.x + fb.w / 2.0, y)
        };
        // the target's port stays, or moves to the middle of the free gap beside the source
        // (a card beyond it would stand on the corner)
        let gap_end = gap_end(&all, (&e.from, &e.to), (start, c), vertical_tail);
        let (lo, hi) = if vertical_tail {
            (tb.x + PORT_ROOM, tb.right() - PORT_ROOM)
        } else {
            (tb.y + PORT_ROOM, tb.bottom() - PORT_ROOM)
        };
        let here = if vertical_tail { c.0 } else { c.1 };
        let s0 = if vertical_tail { start.0 } else { start.1 };
        let blocked = gap_end.is_some_and(|g| (here - s0).abs() + MARGIN > (g - s0).abs());
        let port = match gap_end {
            Some(g) if blocked => f64::midpoint(s0, g).clamp(lo, hi),
            _ => here,
        };
        let c = if vertical_tail {
            (port, c.1)
        } else {
            (c.0, port)
        };
        let corner = if vertical_tail {
            (c.0, start.1)
        } else {
            (start.0, c.1)
        };
        // the corner must lie beyond the source's side, and the tail be long enough
        let beyond = if vertical_tail {
            !(fb.x..=fb.right()).contains(&c.0)
        } else {
            !(fb.y..=fb.bottom()).contains(&c.1)
        };
        let new_tail = (c.0 - corner.0).abs() + (c.1 - corner.1).abs();
        let clear = all
            .iter()
            .filter(|(k, _)| *k != e.from && *k != e.to)
            .all(|(_, cb)| !cuts(start, corner, cb, 0.0) && !cuts(corner, c, cb, 0.0));
        let line = vec![start, corner, c];
        let clear = clear && on_titles(&line, titles) <= on_titles(pts, titles);
        // the source's other ports on that side keep their room
        let taken = side(fb, start).is_some_and(|s| {
            let here = if s.is_multiple_of(2) {
                start.0
            } else {
                start.1
            };
            let mine = if s.is_multiple_of(2) {
                pts[0].0
            } else {
                pts[0].1
            };
            let was_there = side(fb, pts[0]) == Some(s);
            ports(routes, fb, s)
                .iter()
                .any(|o| !(was_there && (o - mine).abs() < 0.5) && (o - here).abs() < PORT_ROOM)
        });
        if beyond && new_tail >= TAIL && clear && !taken && !(tb.x..=tb.right()).contains(&start.0)
        {
            // no nearer to the other lines than the Z was
            let mut trial = routes.clone();
            trial.insert(id.clone(), line);
            if tangles(&trial, &id) <= tangles(routes, &id) {
                *routes = trial;
            }
        }
    }
}

/// The side of `b` that point `p` lies on: 0 top, 1 right, 2 bottom, 3 left.
fn side(b: &Box2, p: (f64, f64)) -> Option<u8> {
    let on_x = b.x - 1.0 <= p.0 && p.0 <= b.right() + 1.0;
    let on_y = b.y - 1.0 <= p.1 && p.1 <= b.bottom() + 1.0;
    if !(on_x && on_y) {
        return None;
    }
    if (p.1 - b.y).abs() < 1.0 {
        Some(0)
    } else if (p.0 - b.right()).abs() < 1.0 {
        Some(1)
    } else if (p.1 - b.bottom()).abs() < 1.0 {
        Some(2)
    } else if (p.0 - b.x).abs() < 1.0 {
        Some(3)
    } else {
        None
    }
}

/// The ends of every route on side `s` of card `b`: their position along that side.
fn ports(routes: &BTreeMap<String, Points>, b: &Box2, s: u8) -> Vec<f64> {
    routes
        .values()
        .flat_map(|r| r.first().into_iter().chain(r.last()))
        .filter(|p| side(b, **p) == Some(s))
        .map(|p| if s.is_multiple_of(2) { p.0 } else { p.1 })
        .collect()
}

/// A Z (three segments, the first and the last parallel) between two cards facing each other
/// becomes one segment: the end on the side with fewer links moves in line with the other, when
/// that place is on its side, clear of its other ports, and the straight line cuts no card and
/// runs over no zone title.
fn straighten(
    model: &Model,
    boxes: &BTreeMap<String, Box2>,
    titles: Titles,
    routes: &mut BTreeMap<String, Points>,
) {
    let all = cards(model, boxes);
    let ids: Vec<String> = routes.keys().cloned().collect();
    for id in ids {
        let Some(e) = model.edges.iter().find(|e| e.id == id) else {
            continue;
        };
        let (Some(fb), Some(tb)) = (boxes.get(&e.from), boxes.get(&e.to)) else {
            continue;
        };
        let pts = &routes[&id];
        if pts.len() != 4 {
            continue;
        }
        let (s, e_) = (pts[0], pts[3]);
        let horizontal = (pts[0].1 - pts[1].1).abs() < 0.5 && (pts[2].1 - pts[3].1).abs() < 0.5;
        let vertical = (pts[0].0 - pts[1].0).abs() < 0.5 && (pts[2].0 - pts[3].0).abs() < 0.5;
        if !(horizontal || vertical) {
            continue;
        }
        let (Some(ss), Some(es)) = (side(fb, s), side(tb, e_)) else {
            continue;
        };
        let (from_ports, to_ports) = (ports(routes, fb, ss), ports(routes, tb, es));
        // along a side: x for a top or bottom one, y for a left or right one
        let at = |p: (f64, f64)| if horizontal { p.1 } else { p.0 };
        let put = |p: (f64, f64), v: f64| if horizontal { (p.0, v) } else { (v, p.1) };
        let range = |b: &Box2, sd: u8| {
            if sd.is_multiple_of(2) {
                (b.x + PORT_ROOM, b.right() - PORT_ROOM)
            } else {
                (b.y + PORT_ROOM, b.bottom() - PORT_ROOM)
            }
        };
        let (fr, tr) = (range(fb, ss), range(tb, es));
        // can a port of a side (its range, its other ports, where it stands) move to `v`?
        let free = |v: f64, r: (f64, f64), others: &[f64], mine: f64| {
            v >= r.0
                && v <= r.1
                && !others
                    .iter()
                    .any(|o| (o - mine).abs() > 0.5 && (o - v).abs() < PORT_ROOM)
        };
        // the end on the side with fewer links moves first (the target's when they tie), then
        // the other, then both to the middle of what the two sides share
        let (sv, ev) = (at(s), at(e_));
        let mid = f64::midpoint(fr.0.max(tr.0), fr.1.min(tr.1));
        let mut tries = vec![(sv, sv), (ev, ev)];
        if from_ports.len() < to_ports.len() {
            tries.swap(0, 1);
        }
        tries.push((mid, mid));
        let line = tries.into_iter().find_map(|(a, b)| {
            let ok = free(a, fr, &from_ports, sv) && free(b, tr, &to_ports, ev);
            let (a, b) = (put(s, a), put(e_, b));
            let clear = all
                .iter()
                .filter(|(c, _)| *c != e.from && *c != e.to)
                .all(|(_, cb)| !cuts(a, b, cb, 0.0));
            let clear = clear && on_titles(&[a, b], titles) <= on_titles(pts, titles);
            (ok && clear).then_some(vec![a, b])
        });
        if let Some(line) = line {
            routes.insert(id, line);
        }
    }
}

/// An arrow's last segment shorter than [`TAIL`] gets the segment before it moved back, away from
/// the card, when that keeps the route's shape, every moved segment clear of the other cards and
/// the line off the zone titles.
/// Arrows ending on one point with the same last bend (a shared trunk) move together. The line
/// of another arrow that the moved segment would pass, or run beside, moves on with it
/// ([`pushed`]); when it cannot, the tail stays short: two lines read as one are worse.
fn tails(
    model: &Model,
    boxes: &BTreeMap<String, Box2>,
    titles: Titles,
    routes: &mut BTreeMap<String, Points>,
) {
    let all = cards(model, boxes);
    let zones = frames(model, boxes);
    let ids: Vec<String> = routes.keys().cloned().collect();
    for id in ids {
        let pts = routes[&id].clone();
        let n = pts.len();
        if n < 4 {
            continue; // a straight line or an L: its bends hang on the source's port
        }
        let (z, a, b, c) = (pts[n - 4], pts[n - 3], pts[n - 2], pts[n - 1]);
        let tail = (c.0 - b.0).abs() + (c.1 - b.1).abs();
        if tail >= TAIL - 0.5 || tail < 0.5 {
            continue;
        }
        let d = TAIL - tail;
        let u = ((b.0 - c.0) / tail, (b.1 - c.1) / tail);
        let shift = |p: (f64, f64)| (p.0 + u.0 * d, p.1 + u.1 * d);
        let (a2, b2) = (shift(a), shift(b));
        // the segment before keeps its direction, and the first stub its margin
        let before = (a.0 - z.0) * u.0 + (a.1 - z.1) * u.1;
        let after = before + d;
        let least = if n == 4 { MARGIN } else { 1.0 };
        if before.abs() < 0.5 || (before < 0.0 && -after < least) {
            continue;
        }
        let e = model.edges.iter().find(|e| e.id == id);
        let ends: Vec<&str> = e.map_or(Vec::new(), |e| vec![e.from.as_str(), e.to.as_str()]);
        let clear = all
            .iter()
            .filter(|(c, _)| !ends.contains(c))
            .all(|(_, cb)| {
                !cuts(z, a2, cb, 0.0) && !cuts(a2, b2, cb, MARGIN / 2.0) && !cuts(b2, c, cb, 0.0)
            });
        if !clear || on_titles(&[z, a2, b2, c], titles) > on_titles(&[z, a, b, c], titles) {
            continue;
        }
        // the arrows sharing this last segment move with it
        let shared: Vec<String> = routes
            .iter()
            .filter(|(_, r)| {
                r.len() >= 3 && r[r.len() - 1] == c && r[r.len() - 2] == b && r[r.len() - 3] == a
            })
            .map(|(k, _)| k.clone())
            .collect();
        let mut plain = routes.clone();
        for k in &shared {
            if let Some(r) = plain.get_mut(k) {
                let m = r.len();
                r[m - 3] = a2;
                r[m - 2] = b2;
            }
        }
        // lines that cannot be confused before a tail: the bend moves when it comes to cross,
        // or to run beside, no more lines than it did; else with the lines in its way moved on;
        // else it stays
        let tangled =
            |r: &Routes, ids: &[String]| -> usize { ids.iter().map(|k| tangles(r, k)).sum() };
        if tangled(&plain, &shared) <= tangled(routes, &shared) {
            *routes = plain;
            continue;
        }
        if !on("grid-push") {
            continue;
        }
        let around = (all.as_slice(), zones.as_slice(), titles);
        if let Some((with, moved)) = pushed(model, around, &plain, &shared, (a, b), (u, d)) {
            let ids: Vec<String> = shared.iter().chain(&moved).cloned().collect();
            if tangled(&with, &ids) <= tangled(routes, &ids) {
                *routes = with;
            }
        }
    }
}

/// `plain` (the routes once the last bend `a`–`b` of the arrows `mine` has moved back by `d`
/// along `u`), with the lines in its way moved on: a stretch of another arrow that ran
/// parallel to the bend, behind it, and that the bend now passes or stands less than [`CLOSE`]
/// from, goes a [`LANE`] behind the bend. It moves only as a tail does: between two segments
/// that keep their direction and their least length, clear of the cards and off the zone
/// titles; and it passes no zone border nor comes to run along one. Returns the routes and the
/// arrows moved; None when no line moves.
fn pushed(
    model: &Model,
    (all, zones, titles): (&[(&str, Box2)], &[Box2], Titles),
    plain: &Routes,
    mine: &[String],
    (a, b): ((f64, f64), (f64, f64)),
    (u, d): ((f64, f64), f64),
) -> Option<(Routes, Vec<String>)> {
    // how far `p` stands behind `q` (along `u`), and where it stands along the bend
    let behind = |p: (f64, f64), q: (f64, f64)| (p.0 - q.0) * u.0 + (p.1 - q.1) * u.1;
    let along = |p: (f64, f64)| if u.0.abs() > 0.5 { p.1 } else { p.0 };
    let (lo, hi) = (along(a).min(along(b)), along(a).max(along(b)));
    let kept = |before: f64, after: f64, least: f64| {
        before * after > 0.0 && (after.abs() >= before.abs() || after.abs() >= least)
    };
    let mut out = plain.clone();
    let mut moved: Vec<String> = Vec::new();
    for (id, o) in plain {
        if mine.contains(id) {
            continue;
        }
        let e = model.edges.iter().find(|e| e.id == *id);
        let ends: Vec<&str> = e.map_or(Vec::new(), |e| vec![e.from.as_str(), e.to.as_str()]);
        for j in 1..o.len().saturating_sub(2) {
            let (p0, p, q, q1) = (o[j - 1], o[j], o[j + 1], o[j + 2]);
            // parallel to the bend, beside it, and in its way
            let parallel = behind(p, q).abs() < 1e-9 && (along(p) - along(q)).abs() > 0.5;
            let s = behind(p, b);
            let beside = along(p).max(along(q)).min(hi) - along(p).min(along(q)).max(lo) > 0.5;
            if !(parallel && beside && s > 0.5 && s < d + CLOSE) {
                continue;
            }
            let by = d + LANE - s;
            let shift = |p: (f64, f64)| (p.0 + u.0 * by, p.1 + u.1 * by);
            let (n1, n2) = (shift(p), shift(q));
            // its neighbours run across, and keep their direction and their least length
            let across = (along(p0) - along(p)).abs() < 1e-9 && (along(q) - along(q1)).abs() < 1e-9;
            let first = if j == 1 { MARGIN } else { LANE };
            let last = if j + 3 == o.len() { TAIL } else { LANE };
            let shape = across
                && kept(behind(p, p0), behind(n1, p0), first)
                && kept(behind(q1, q), behind(q1, n2), last);
            let clear = all.iter().all(|(c, cb)| {
                let own = ends.contains(c);
                !cuts(n1, n2, cb, if own { 0.0 } else { MARGIN / 2.0 })
                    && (own || (!cuts(p0, n1, cb, 0.0) && !cuts(n2, q1, cb, 0.0)))
            });
            let titled = on_titles(&[p0, n1, n2, q1], titles) > on_titles(&[p0, p, q, q1], titles);
            if !shape || !clear || titled || meets_border(&[p, q], by, zones) {
                continue;
            }
            if let Some(r) = out.get_mut(id) {
                r[j] = n1;
                r[j + 1] = n2;
                moved.push(id.clone());
            }
        }
    }
    (!moved.is_empty()).then_some((out, moved))
}
