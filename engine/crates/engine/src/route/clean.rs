//! Clean-up of the router's routes: tiny jogs, crossings centred in the gaps, short jogs
//! straightened, steps flattened, collinear points dropped.

use super::Points;
use crate::layout::Box2;
use skemo_lang::{Kind, Model};
use std::collections::BTreeMap;

/// Jogs under this threshold (px): offset of a few tenths between two centres.
const MICRO_JOG: f64 = 0.75;

/// Removes tiny jogs (centres of two cards offset by a few tenths of a pixel): the route stays
/// straight, with an invisible slope. The ends and the short attachment segments (arrow) are never
/// touched; the result is the same whether the card is pinned (rounded positions) or not.
pub(super) fn drop_micro_jogs(mut pts: Points) -> Points {
    let seg = |a: (f64, f64), b: (f64, f64)| (a.0 - b.0).abs().max((a.1 - b.1).abs());
    let mut i = 1;
    while i + 2 < pts.len() {
        if seg(pts[i], pts[i + 1]) < MICRO_JOG
            && seg(pts[i - 1], pts[i]) >= 6.0
            && seg(pts[i + 1], pts[i + 2]) >= 6.0
        {
            pts.drain(i..i + 2);
        } else {
            i += 1;
        }
    }
    pts
}

/// A cross segment of an arrow: the route, the index of its first point, its level along the flow.
type CrossSeg = (String, usize);

/// Arrows cross the gap between two rows half-way through the free space (D-049): between the
/// bottom of what stands above the segment (card or zone frame) and the top of what stands below
/// it, zone titles included, zones the segment runs inside and bare zones (no frame) left out. The router centres a bend
/// between the two cards, so a fork's bar ran 5 px above the frame of the zone holding its
/// targets, and two arrows leaving one card for two zones bent at two levels. The segments of
/// one group (a fork's bar) move together; a segment stays when a card stands on its level, when
/// nothing frames it on one side, when another arrow already runs on the new level, or when the
/// arrows would cross more lines there than where the router put them. Arrows with waypoints
/// (`via=`) are left as routed.
/// The segments that cross a gap between two runs along the flow, by group (or arrow) and level
/// (in hundredths of a pixel). An arrow with waypoints (`via=`) keeps the path its author chose.
fn crossing_segments(
    model: &Model,
    groups: &BTreeMap<String, String>,
    routes: &BTreeMap<String, Points>,
    (f, c): (impl Fn((f64, f64)) -> f64, impl Fn((f64, f64)) -> f64),
) -> BTreeMap<(String, i64), Vec<CrossSeg>> {
    let placed: Vec<&str> = model
        .edges
        .iter()
        .filter(|e| e.via.is_some())
        .map(|e| e.id.as_str())
        .collect();
    let mut keys: BTreeMap<(String, i64), Vec<CrossSeg>> = BTreeMap::new();
    for (id, r) in routes
        .iter()
        .filter(|(id, _)| !placed.contains(&id.as_str()))
    {
        for i in 1..r.len().saturating_sub(2) {
            let (a, b) = (r[i], r[i + 1]);
            // the router's points may differ by a few millionths of a pixel
            let crossing = (f(a) - f(b)).abs() < 0.01 && (c(a) - c(b)).abs() > 0.01;
            let along = (c(r[i - 1]) - c(a)).abs() < 0.01 && (c(r[i + 2]) - c(b)).abs() < 0.01;
            if crossing && along {
                let g = groups.get(id).cloned().unwrap_or_else(|| id.clone());
                keys.entry((g, (f(a) * 100.0).round() as i64))
                    .or_default()
                    .push((id.clone(), i));
            }
        }
    }
    keys
}

pub(super) fn center_in_gaps(
    model: &Model,
    boxes: &BTreeMap<String, Box2>,
    titles: &[(String, Box2)],
    groups: &BTreeMap<String, String>,
    routes: &mut BTreeMap<String, Points>,
) {
    let across = matches!(model.option("dir").unwrap_or("down"), "right" | "left");
    // along the flow, and across it
    let f = move |p: (f64, f64)| if across { p.0 } else { p.1 };
    let c = move |p: (f64, f64)| if across { p.1 } else { p.0 };
    let keys = crossing_segments(model, groups, routes, (f, c));
    // a bare zone or a stack draws no frame: it bounds nothing
    let frames: Vec<(bool, Box2)> = model
        .nodes
        .iter()
        .filter(|n| !n.stack && !n.classes.iter().any(|c| c == "bare"))
        .filter_map(|n| boxes.get(&n.id).map(|b| (n.kind == Kind::Card, *b)))
        .chain(titles.iter().map(|(_, b)| (true, *b)))
        .collect();
    for members in keys.values() {
        let level = f(routes[&members[0].0][members[0].1]);
        let span = members
            .iter()
            .fold((f64::MAX, f64::MIN), |(lo, hi), (id, i)| {
                let r = &routes[id];
                let (u, v) = (c(r[*i]), c(r[*i + 1]));
                (lo.min(u.min(v)), hi.max(u.max(v)))
            });
        let Some(to) = gap_middle(&frames, level, span, across) else {
            continue;
        };
        if (to - level).abs() < 1.0 || level_taken(routes, members, to, span, f, c) {
            continue;
        }
        let mut moved = routes.clone();
        for (id, i) in members {
            if let Some(r) = moved.get_mut(id) {
                for k in [*i, *i + 1] {
                    r[k] = if across { (to, r[k].1) } else { (r[k].0, to) };
                }
            }
        }
        // the middle of the gap, unless the arrows cross more lines there
        let ids: Vec<&str> = members.iter().map(|(id, _)| id.as_str()).collect();
        if crossings_with(&moved, &ids, groups) <= crossings_with(routes, &ids, groups) {
            *routes = moved;
        }
    }
}

/// Crossings of the arrows `ids` with the others, those of their own groups (shared trunks)
/// left out: a vertical segment through a horizontal one, inside both.
fn crossings_with<'a>(
    routes: &'a BTreeMap<String, Points>,
    ids: &[&'a str],
    groups: &'a BTreeMap<String, String>,
) -> usize {
    fn group<'a>(groups: &'a BTreeMap<String, String>, id: &'a str) -> &'a str {
        groups.get(id).map_or(id, String::as_str)
    }
    let group = |id: &'a str| group(groups, id);
    let mine: Vec<&str> = ids.iter().map(|id| group(id)).collect();
    let cross = |s: &[(f64, f64)], t: &[(f64, f64)]| {
        let (hs, ht) = ((s[0].1 - s[1].1).abs() < 0.5, (t[0].1 - t[1].1).abs() < 0.5);
        if hs == ht {
            return false;
        }
        let (h, v) = if hs { (s, t) } else { (t, s) };
        let (x, y) = (v[0].0, h[0].1);
        h[0].0.min(h[1].0) + 0.5 < x
            && x < h[0].0.max(h[1].0) - 0.5
            && v[0].1.min(v[1].1) + 0.5 < y
            && y < v[0].1.max(v[1].1) - 0.5
    };
    ids.iter()
        .filter_map(|id| routes.get(*id))
        .map(|r| {
            routes
                .iter()
                .filter(|(o, _)| !mine.contains(&group(o)))
                .map(|(_, o)| {
                    r.windows(2)
                        .map(|s| o.windows(2).filter(|t| cross(s, t)).count())
                        .sum::<usize>()
                })
                .sum::<usize>()
        })
        .sum()
}

/// The middle of the free gap around `level` over the cross range `span`: `None` when a card
/// stands on that level or nothing bounds the gap on one side.
fn gap_middle(frames: &[(bool, Box2)], level: f64, span: (f64, f64), across: bool) -> Option<f64> {
    let (mut lo, mut hi) = (f64::MIN, f64::MAX);
    for (card, b) in frames {
        let (c0, c1, f0, f1) = if across {
            (b.y, b.bottom(), b.x, b.right())
        } else {
            (b.x, b.right(), b.y, b.bottom())
        };
        if c1 <= span.0 + 0.5 || c0 >= span.1 - 0.5 {
            continue;
        }
        if f1 <= level + 0.5 {
            lo = lo.max(f1);
        } else if f0 >= level - 0.5 {
            hi = hi.min(f0);
        } else if *card {
            return None;
        }
    }
    (lo > f64::MIN && hi < f64::MAX).then(|| ((lo + hi) / 2.0 * 2.0).round() / 2.0)
}

/// Another arrow's cross segment runs within 6 px of `to` over the range `span`.
fn level_taken(
    routes: &BTreeMap<String, Points>,
    members: &[CrossSeg],
    to: f64,
    span: (f64, f64),
    f: impl Fn((f64, f64)) -> f64,
    c: impl Fn((f64, f64)) -> f64,
) -> bool {
    routes.iter().any(|(id, r)| {
        r.windows(2).enumerate().any(|(i, w)| {
            let mine = members.iter().any(|(m, j)| m == id && *j == i);
            let (u, v) = (c(w[0]).min(c(w[1])), c(w[0]).max(c(w[1])));
            !mine
                && (f(w[0]) - f(w[1])).abs() < 0.01
                && (f(w[0]) - to).abs() < 6.0
                && u < span.1
                && v > span.0
        })
    })
}

/// Jogs up to this length (px) are straightened by sliding an end of the arrow (D-049).
const SMALL_JOG: f64 = 8.0;

/// Longer jogs, up to a centimetre on screen (px), are straightened too when the straight line
/// crosses no more lines (the user's rule: no step under a centimetre).
const MAX_JOG: f64 = 38.0;

/// Gap kept between a slid end and the other ends on its card side, for a longer jog (px, the
/// router's port spacing).
const PORT_GAP: f64 = 12.0;

/// Margin kept between a slid end and the corners of its card (px).
const END_MARGIN: f64 = 10.0;

/// Straightens arrows that go straight, step aside by a few pixels, then go straight again
/// (D-049): two cards whose centres differ by 2 px, or a port clamped at the edge of the source
/// while the target's port is 3 px further; a step up to a centimetre too when the straight line
/// crosses no more lines and its end keeps a port spacing from the others. Never onto another
/// arrow's line. The arrow's end slides along its card side by the jog,
/// when it stays clear of the corners, of the other arrows' ends on that card and of other cards;
/// otherwise its start does (never for a grouped arrow, whose start is a shared trunk, nor for
/// an arrow with waypoints). A hand
/// drawing has no such step, and a fork's branch no longer leaves a second junction dot.
pub(super) fn straighten_jogs(
    model: &Model,
    boxes: &BTreeMap<String, Box2>,
    groups: &BTreeMap<String, String>,
    routes: &mut BTreeMap<String, Points>,
) {
    for e in model.edges.iter().filter(|e| e.via.is_none()) {
        let Some(pts) = routes.get(&e.id) else {
            continue;
        };
        let [p0, p1, p2, p3] = pts[..] else { continue };
        let vertical = (p0.0 - p1.0).abs() < 0.01 && (p2.0 - p3.0).abs() < 0.01;
        let horizontal = (p0.1 - p1.1).abs() < 0.01 && (p2.1 - p3.1).abs() < 0.01;
        let jog = if vertical { p1.0 - p2.0 } else { p1.1 - p2.1 };
        if !(vertical || horizontal) || jog.abs() < 0.01 || jog.abs() > MAX_JOG {
            continue;
        }
        let long = jog.abs() > SMALL_JOG;
        let crossed = |r: &[(f64, f64)]| {
            routes
                .iter()
                .filter(|(o, _)| *o != &e.id)
                .map(|(_, o)| crossings_between(r, o))
                .sum::<usize>()
        };
        let before = if long { crossed(pts) } else { 0 };
        let shift = |p: (f64, f64), d: f64| {
            if vertical {
                (p.0 + d, p.1)
            } else {
                (p.0, p.1 + d)
            }
        };
        let mut tries = vec![(e.to.as_str(), (p0, shift(p3, jog)))];
        if !groups.contains_key(&e.id) {
            tries.push((e.from.as_str(), (shift(p0, -jog), p3)));
        }
        let gap = if long { PORT_GAP } else { SMALL_JOG };
        for (card, (a, b)) in tries {
            let moved = if card == e.to { b } else { a };
            if end_fits(model, boxes, routes, (&e.id, card, gap), moved, vertical)
                && clear_of_cards(model, boxes, e, a, b)
                && (!long || crossed(&[a, b]) <= before)
                && !runs_on_another(model, routes, e, &[a, b])
            {
                routes.insert(e.id.clone(), vec![a, b]);
                break;
            }
        }
    }
}

/// A slid end `p` stays on its card side, away from the corners and from the other arrows' ends.
fn end_fits(
    model: &Model,
    boxes: &BTreeMap<String, Box2>,
    routes: &BTreeMap<String, Points>,
    (id, card, gap): (&str, &str, f64),
    p: (f64, f64),
    vertical: bool,
) -> bool {
    let Some(b) = boxes.get(card) else {
        return false;
    };
    let (v, lo, hi) = if vertical {
        (p.0, b.x, b.right())
    } else {
        (p.1, b.y, b.bottom())
    };
    if v < lo + END_MARGIN || v > hi - END_MARGIN {
        return false;
    }
    model
        .edges
        .iter()
        .filter(|f| f.id != id && (f.from == card || f.to == card))
        .filter_map(|f| routes.get(&f.id))
        .flat_map(|r| [r.first().copied(), r.last().copied()])
        .flatten()
        .all(|q| (q.0 - p.0).abs() + (q.1 - p.1).abs() >= gap)
}

/// Does the polyline `pts` of arrow `e` run on another arrow's line (the same line within 1.5 px
/// over more than 4 px)? Arrows from the same card or into the same card share lines on purpose
/// (a fork's trunk, a join).
fn runs_on_another(
    model: &Model,
    routes: &BTreeMap<String, Points>,
    e: &skemo_lang::Edge,
    pts: &[(f64, f64)],
) -> bool {
    let run = |p: (f64, f64), q: (f64, f64)| {
        if (p.0 - q.0).abs() < 0.5 {
            Some((true, p.0, p.1.min(q.1), p.1.max(q.1)))
        } else if (p.1 - q.1).abs() < 0.5 {
            Some((false, p.1, p.0.min(q.0), p.0.max(q.0)))
        } else {
            None
        }
    };
    let mine: Vec<_> = pts.windows(2).filter_map(|w| run(w[0], w[1])).collect();
    model
        .edges
        .iter()
        .filter(|f| f.id != e.id && f.from != e.from && f.to != e.to)
        .filter_map(|f| routes.get(&f.id))
        .flat_map(|r| {
            r.windows(2)
                .filter_map(|w| run(w[0], w[1]))
                .collect::<Vec<_>>()
        })
        .any(|(v, c, lo, hi)| {
            mine.iter().any(|(v2, c2, lo2, hi2)| {
                v == *v2 && (c - c2).abs() < 1.5 && hi.min(*hi2) - lo.max(*lo2) > 4.0
            })
        })
}

/// Crossings of two routes: a horizontal and a vertical segment crossing inside both.
fn crossings_between(a: &[(f64, f64)], b: &[(f64, f64)]) -> usize {
    let cross = |s: &[(f64, f64)], t: &[(f64, f64)]| {
        let (hs, ht) = ((s[0].1 - s[1].1).abs() < 0.5, (t[0].1 - t[1].1).abs() < 0.5);
        if hs == ht {
            return false;
        }
        let (h, v) = if hs { (s, t) } else { (t, s) };
        let (x, y) = (v[0].0, h[0].1);
        h[0].0.min(h[1].0) + 0.5 < x
            && x < h[0].0.max(h[1].0) - 0.5
            && v[0].1.min(v[1].1) + 0.5 < y
            && y < v[0].1.max(v[1].1) - 0.5
    };
    a.windows(2)
        .map(|s| b.windows(2).filter(|t| cross(s, t)).count())
        .sum()
}

/// Steps in the middle of an arrow (D-049, the user's "no step under a centimetre"): two
/// parallel segments running the same way, joined by a segment shorter than `MAX_JOG`, go on one
/// level, the later one's or the earlier one's (never moving an end), when the arrow then crosses
/// no more lines, runs on no other line and passes through no card nor zone title. Arrows with
/// waypoints or in a group (a shared trunk) are left as they are.
pub(super) fn flatten_steps(
    model: &Model,
    boxes: &BTreeMap<String, Box2>,
    titles: &[(String, Box2)],
    groups: &BTreeMap<String, String>,
    routes: &mut BTreeMap<String, Points>,
) {
    for e in model
        .edges
        .iter()
        .filter(|e| e.via.is_none() && !groups.contains_key(&e.id))
    {
        let mut k = 1;
        while let Some(pts) = routes.get(&e.id).cloned() {
            if k + 2 >= pts.len() {
                break;
            }
            match flat_step(model, boxes, titles, routes, e, &pts, k) {
                Some(better) => {
                    routes.insert(e.id.clone(), better);
                }
                None => k += 1,
            }
        }
    }
}

/// The arrow with its step at corner `k` (segments `k - 1`, `k`, `k + 1`) on one level, if that
/// is allowed (see `flatten_steps`).
fn flat_step(
    model: &Model,
    boxes: &BTreeMap<String, Box2>,
    titles: &[(String, Box2)],
    routes: &BTreeMap<String, Points>,
    e: &skemo_lang::Edge,
    pts: &[(f64, f64)],
    k: usize,
) -> Option<Points> {
    let (a, b, c, d) = (pts[k - 1], pts[k], pts[k + 1], pts[k + 2]);
    let horiz = (a.1 - b.1).abs() < 0.5 && (c.1 - d.1).abs() < 0.5;
    let vert = (a.0 - b.0).abs() < 0.5 && (c.0 - d.0).abs() < 0.5;
    let level = |p: (f64, f64)| if horiz { p.1 } else { p.0 };
    let along = |p: (f64, f64)| if horiz { p.0 } else { p.1 };
    let jog = (level(b) - level(c)).abs();
    if !(horiz || vert)
        || jog < 0.5
        || jog > MAX_JOG
        || (along(b) - along(a)).signum() != (along(d) - along(c)).signum()
    {
        return None;
    }
    let set = |p: &mut (f64, f64), v: f64| {
        if horiz {
            p.1 = v;
        } else {
            p.0 = v;
        }
    };
    let crossed = |r: &[(f64, f64)]| {
        routes
            .iter()
            .filter(|(o, _)| *o != &e.id)
            .map(|(_, o)| crossings_between(r, o))
            .sum::<usize>()
    };
    let before = crossed(pts);
    // the later segment up to the earlier one's level, else the earlier one down to the later's;
    // the arrow's own ends stay
    let mut tries = Vec::new();
    if k + 2 < pts.len() - 1 {
        let mut p = pts.to_vec();
        set(&mut p[k + 1], level(b));
        set(&mut p[k + 2], level(b));
        tries.push(p);
    }
    if k > 1 {
        let mut p = pts.to_vec();
        set(&mut p[k - 1], level(c));
        set(&mut p[k], level(c));
        tries.push(p);
    }
    tries
        .into_iter()
        .map(|p| drop_micro_jogs(simplify(p)))
        .find(|p| {
            crossed(p) <= before
                && !runs_on_another(model, routes, e, p)
                && p.windows(2)
                    .all(|w| clear_of_cards(model, boxes, e, w[0], w[1]))
                && !titles
                    .iter()
                    .any(|(_, t)| p.windows(2).any(|w| through_box(w[0], w[1], t)))
        })
}

/// The polyline without repeated points nor middle points of straight runs.
fn simplify(pts: Points) -> Points {
    let mut out: Points = Vec::with_capacity(pts.len());
    for p in pts {
        if out
            .last()
            .is_some_and(|q: &(f64, f64)| (q.0 - p.0).abs() < 0.01 && (q.1 - p.1).abs() < 0.01)
        {
            continue;
        }
        if out.len() >= 2 {
            let (a, b) = (out[out.len() - 2], out[out.len() - 1]);
            let straight = ((a.0 - b.0).abs() < 0.01 && (b.0 - p.0).abs() < 0.01)
                || ((a.1 - b.1).abs() < 0.01 && (b.1 - p.1).abs() < 0.01);
            if straight {
                out.pop();
            }
        }
        out.push(p);
    }
    out
}

/// Does segment `a`–`b` pass through the inside of box `t` (2 px in from its edges)?
fn through_box(a: (f64, f64), b: (f64, f64), t: &Box2) -> bool {
    let (x0, x1, y0, y1) = (a.0.min(b.0), a.0.max(b.0), a.1.min(b.1), a.1.max(b.1));
    x1 > t.x + 2.0 && x0 < t.right() - 2.0 && y1 > t.y + 2.0 && y0 < t.bottom() - 2.0
}

/// The straight segment `a`–`b` crosses no card but the arrow's own two.
fn clear_of_cards(
    model: &Model,
    boxes: &BTreeMap<String, Box2>,
    e: &skemo_lang::Edge,
    a: (f64, f64),
    b: (f64, f64),
) -> bool {
    let (x0, x1, y0, y1) = (a.0.min(b.0), a.0.max(b.0), a.1.min(b.1), a.1.max(b.1));
    model
        .nodes
        .iter()
        .filter(|n| n.kind == Kind::Card && n.id != e.from && n.id != e.to)
        .filter_map(|n| boxes.get(&n.id))
        .all(|r| x1 <= r.x || x0 >= r.right() || y1 <= r.y || y0 >= r.bottom())
}
