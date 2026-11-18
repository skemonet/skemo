//! Room for the lines between two rows (D-102): a corridor that more arrows cross than it holds
//! grows.
//!
//! ELK spaces its rows for its own routes; the router draws others, and the tidy passes move
//! cards after ELK. A corridor between two rows then carries more parallel lines than it has room
//! for: the router squeezes them a few pixels apart, along the cards (bug 22 of the private
//! corpus: five lines in 59 px, one laid over another; once apart, they ran 7 px from the cards
//! and under the labels). After a first routing, every corridor whose parallel lines need more
//! room than it has (the router's margin from the cards on both sides, [`LINE_GAP`] between two
//! lines) is widened: everything past it moves along the flow by what it lacks, then the drawing
//! is routed again. The widening is read on the layout without pins (D-031), like the zone
//! titles' wraps, so that pinning a card where it stands changes nothing.

use crate::breathe::LINE_GAP;
use crate::layout::{Box2, Placement};
use crate::route::Points;
use skemo_lang::{Kind, Model};
use std::collections::BTreeMap;

/// The router's margin between a line and a card (`route::graph_params`, `shape_buffer`).
const MARGIN: f64 = 14.0;
/// Shortest cross segment counted (px): a jog is not a line along the corridor.
const MIN_RUN: f64 = 4.0;
/// The straight run kept behind an arrow's head (`grid::finish`, the manual method's §6): what a
/// line that turns into a card needs between its bend and that card.
const TAIL: f64 = 24.0;

/// A corridor too narrow for its lines, at one place.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Lack {
    /// Where the corridor starts and where it ends, along the flow.
    pub lo: f64,
    pub hi: f64,
    /// The place, across the flow: where its lines stand side by side between what bounds it.
    pub reach: (f64, f64),
    /// The room it lacks (px).
    pub px: f64,
}

/// Does the drawing flow left to right (or right to left)? Its corridors are then columns.
pub fn across(model: &Model) -> bool {
    matches!(model.option("dir").unwrap_or("down"), "right" | "left")
}

/// What bounds a corridor or takes a level in it: the drawing's boxes (cards and zone frames), the
/// zone titles and the arrow labels.
pub type Drawn<'a> = (
    &'a BTreeMap<String, Box2>,
    &'a [(String, Box2)],
    &'a [(String, Box2)],
);

/// The room each corridor lacks: `(where it ends along the flow, px)`, in the order of the flow.
/// `boxes`: the drawing's boxes (cards and zone frames), `titles`: the zone titles, `routes` and
/// `groups` (the arrows sharing a trunk count as one line).
/// Along x (`across`: columns) or y (rows), whatever the drawing's direction (the grid proof of
/// concept reads both).
pub fn lacking_on(
    model: &Model,
    (boxes, titles, labels): Drawn,
    (routes, groups): (&BTreeMap<String, Points>, &BTreeMap<String, String>),
    across: bool,
) -> Vec<(f64, f64)> {
    let f = move |p: (f64, f64)| if across { p.0 } else { p.1 };
    let c = move |p: (f64, f64)| if across { p.1 } else { p.0 };
    let frames = bounds(model, boxes, titles);
    // in a diagram drawn in steps, only the arrows on screen at the same time are side by side
    let anim = crate::anim::resolve(model);
    let life = |id: &str| life(&anim, id);
    // every cross segment with the corridor it runs in: (start, end, level, room, line, span, life)
    let mut all: Vec<(f64, f64, f64, Run)> = Vec::new();
    for (id, r) in routes {
        let line = groups.get(id).map_or(id.as_str(), String::as_str);
        let alive = life(id);
        for w in r.windows(2) {
            let (p, q) = (w[0], w[1]);
            if (f(p) - f(q)).abs() > 0.5 || (c(p) - c(q)).abs() < MIN_RUN {
                continue;
            }
            let span = (c(p).min(c(q)), c(p).max(c(q)));
            if let Some((lo, hi)) = corridor(&frames, f(p), span, across) {
                all.push((lo, hi, f(p), (hi - lo, line, span, alive)));
            }
        }
    }
    // a label set in a corridor takes a level of its own, as a line does (its own line under it
    // included: a labelled line needs about two): the private map's `2-dev/assisted`, a label
    // beside a short arrow with its neighbour's line under it
    let keys: Vec<String> = labels
        .iter()
        .map(|(id, _)| format!("\u{0}label|{id}"))
        .collect();
    // each label's extent across the corridor (its width in a column): it needs that much room,
    // clear of what bounds the corridor
    let mut wide: BTreeMap<&str, f64> = BTreeMap::new();
    for ((id, b), key) in labels.iter().zip(&keys) {
        let centre = (b.x + b.w / 2.0, b.y + b.h / 2.0);
        let span = if across {
            (b.y, b.bottom())
        } else {
            (b.x, b.right())
        };
        if let Some((lo, hi)) = corridor(&frames, f(centre), span, across) {
            all.push((lo, hi, f(centre), (hi - lo, key.as_str(), span, life(id))));
            wide.insert(key.as_str(), if across { b.w } else { b.h });
        }
    }
    // by the end of their corridor (what moves when it widens); a line running in that band over
    // its reach counts too, whatever bounds its own corridor (one stopping short of the card that
    // closes this one: the private map's `2-dev/hive`, four lines where three were counted)
    let mut ends: Vec<f64> = all.iter().map(|a| a.1).collect();
    ends.sort_by(f64::total_cmp);
    ends.dedup_by(|a, b| (*a - *b).abs() < 0.5);
    let mut out = Vec::new();
    for end in ends {
        let own: Vec<&(f64, f64, f64, Run)> =
            all.iter().filter(|a| (a.1 - end).abs() < 0.5).collect();
        let start = own.iter().map(|a| a.0).fold(f64::NEG_INFINITY, f64::max);
        let reach = own
            .iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(l, h), a| {
                (l.min(a.3.2.0), h.max(a.3.2.1))
            });
        let members: Vec<Run> = all
            .iter()
            .filter(|a| {
                (a.1 - end).abs() < 0.5
                    || (a.2 > start && a.2 < end && a.3.2.0 < reach.1 && a.3.2.1 > reach.0)
            })
            .map(|a| a.3)
            .collect();
        // the lives of intervals that meet two by two share a moment: one of their starts
        let lines = members
            .iter()
            .map(|m| m.3.0)
            .map(|t| {
                let now: Vec<Run> = members
                    .iter()
                    .filter(|m| m.3.0 <= t && t <= m.3.1)
                    .copied()
                    .collect();
                most_side_by_side(&now)
            })
            .max()
            .unwrap_or(0);
        if lines < 2 {
            continue;
        }
        let room = own.iter().map(|a| a.3.0).fold(f64::INFINITY, f64::min);
        // a label stands clear of the corridor's sides with the other lines beside it (its own
        // line, when it runs in the corridor, under it)
        let label = members
            .iter()
            .filter_map(|m| {
                let w = wide.get(m.1)?;
                let id = m.1.strip_prefix("\u{0}label|")?;
                let line = groups.get(id).map_or(id, String::as_str);
                let own = usize::from(members.iter().any(|o| o.1 == line));
                let others = lines.saturating_sub(1 + own);
                Some(w + 2.0 * crate::labels::LABEL_ROOM + others as f64 * LINE_GAP)
            })
            .fold(0.0, f64::max);
        let need = (2.0 * MARGIN + (lines - 1) as f64 * LINE_GAP).max(label);
        if need - room >= 1.0 {
            out.push((end, (need - room).ceil()));
        }
    }
    out
}

/// What bounds a corridor: cards, drawn zone frames, zone titles (as `route::clean`).
fn bounds(model: &Model, boxes: &BTreeMap<String, Box2>, titles: &[(String, Box2)]) -> Vec<Box2> {
    model
        .nodes
        .iter()
        .filter(|n| !n.stack && !n.classes.iter().any(|c| c == "bare"))
        .filter(|n| n.kind == Kind::Card || !crate::layout::invisible(n))
        .filter_map(|n| boxes.get(&n.id).copied())
        .chain(titles.iter().map(|(_, b)| *b))
        .collect()
}

/// When arrow `id` is on screen (steps): from its appearance until it leaves.
fn life(anim: &crate::anim::Animation, id: &str) -> (f64, f64) {
    anim.elements
        .get(id)
        .map_or((f64::NEG_INFINITY, f64::INFINITY), |r| {
            (
                r.appear.value,
                r.until.as_ref().map_or(f64::INFINITY, |u| u.value),
            )
        })
}

/// A cross segment of a line, or an arrow label, as a corridor holds it: its level, its line (an
/// arrow, or the group of a shared trunk), its span across the flow, the life of its arrow
/// (steps) and, when the line turns at its end into its head, where it turns and where that
/// head stands.
struct Seg<'a> {
    level: f64,
    line: &'a str,
    span: (f64, f64),
    life: (f64, f64),
    head: Option<(f64, f64)>,
}

/// The segments of `routes` that run along the corridors (`across`: upright), a join or a fork
/// as one line; `tails`: with where a segment turns into its head.
fn route_segs<'a>(
    anim: &crate::anim::Animation,
    (routes, groups): (&'a BTreeMap<String, Points>, &'a BTreeMap<String, String>),
    across: bool,
    tails: bool,
) -> Vec<Seg<'a>> {
    let f = move |p: (f64, f64)| if across { p.0 } else { p.1 };
    let c = move |p: (f64, f64)| if across { p.1 } else { p.0 };
    let mut segs = Vec::new();
    for (id, r) in routes {
        let line = groups.get(id).map_or(id.as_str(), String::as_str);
        for (i, w) in r.windows(2).enumerate() {
            let (p, q) = (w[0], w[1]);
            if (f(p) - f(q)).abs() > 0.5 || (c(p) - c(q)).abs() < MIN_RUN {
                continue;
            }
            // the line turns at the end of this segment, into its head
            let last = r.last().filter(|_| tails && i + 3 == r.len());
            segs.push(Seg {
                level: f(p),
                line,
                span: (c(p).min(c(q)), c(p).max(c(q))),
                life: life(anim, id),
                head: last.map(|h| (c(q), f(*h))),
            });
        }
    }
    segs
}

/// The most lines of `list` shown at once: the lives of intervals that meet two by two share a
/// moment, one of their starts.
fn most_at_once(list: &[&Seg]) -> usize {
    list.iter()
        .map(|s| s.life.0)
        .map(|t| {
            let mut now: Vec<&str> = list
                .iter()
                .filter(|s| s.life.0 <= t && t <= s.life.1)
                .map(|s| s.line)
                .collect();
            now.sort_unstable();
            now.dedup();
            now.len()
        })
        .max()
        .unwrap_or(0)
}

/// The corridors too narrow for their lines, each place on its own: the reading of the grid
/// engine, which makes its room evenly (`grid::room`).
///
/// [`lacking_on`] reads a corridor over the whole reach of its lines, between whatever stands
/// nearest on each side anywhere along them, and what lacks is taken at its end. Even room is
/// asked of the gap between two things that face each other: here a corridor is read place by
/// place, between the two boxes that bound it there, with the lines that stand side by side
/// there. (The private map's `2-dev/fonctionnement/acm`: three lines between a card of one row
/// and a zone of the row above, which no gutter separates; the request came back at every
/// reading, 28 px of gutter each time for nothing.)
///
/// `tails`: a line that turns in a corridor right before its head needs a straight run behind
/// that head ([`TAIL`] rather than the router's margin on that side), from one line on: a tail
/// cut short is a request for room, like two lines too close (the manual method's §7).
pub fn lacks(
    model: &Model,
    (boxes, titles, labels): Drawn,
    (routes, groups): (&BTreeMap<String, Points>, &BTreeMap<String, String>),
    across: bool,
    tails: bool,
) -> Vec<Lack> {
    let f = move |p: (f64, f64)| if across { p.0 } else { p.1 };
    let frames = bounds(model, boxes, titles);
    let anim = crate::anim::resolve(model);
    let mut segs = route_segs(&anim, (routes, groups), across, tails);
    // a label set in a corridor takes a level of its own, as a line does (`lacking_on`)
    let keys: Vec<String> = labels
        .iter()
        .map(|(id, _)| format!("\u{0}label|{id}"))
        .collect();
    for ((id, b), key) in labels.iter().zip(&keys) {
        let centre = (b.x + b.w / 2.0, b.y + b.h / 2.0);
        segs.push(Seg {
            level: f(centre),
            line: key,
            span: if across {
                (b.y, b.bottom())
            } else {
                (b.x, b.right())
            },
            life: life(&anim, id),
            head: None,
        });
    }
    // the places: from one end of a line's span, or of a box, to the next (the same lines
    // between the same boxes all along). Two lines ending less than the router's margin apart
    // on one level touch at their corners: side by side (`most_side_by_side`)
    let grown = |s: &Seg| (s.span.0 - MARGIN / 2.0, s.span.1 + MARGIN / 2.0);
    let ends = |b: &Box2| {
        if across {
            [b.y, b.bottom()]
        } else {
            [b.x, b.right()]
        }
    };
    let mut cuts: Vec<f64> = segs
        .iter()
        .flat_map(|s| [grown(s).0, grown(s).1])
        .chain(frames.iter().flat_map(ends))
        .collect();
    cuts.sort_by(f64::total_cmp);
    cuts.dedup_by(|a, b| (*a - *b).abs() < 0.5);
    let mut out: Vec<Lack> = Vec::new();
    for w in cuts.windows(2) {
        let (a, b) = (w[0], w[1]);
        // the lines of this place, by the corridor each runs in here
        let mut here: Vec<((f64, f64), Vec<&Seg>)> = Vec::new();
        for s in &segs {
            if grown(s).0 > a + 0.5 || grown(s).1 < b - 0.5 {
                continue;
            }
            let Some(at) = corridor(&frames, s.level, (a, b), across) else {
                continue;
            };
            let same = |k: &(f64, f64)| (k.0 - at.0).abs() < 0.5 && (k.1 - at.1).abs() < 0.5;
            match here.iter_mut().find(|(k, _)| same(k)) {
                Some((_, list)) => list.push(s),
                None => here.push((at, vec![s])),
            }
        }
        for ((lo, hi), list) in here {
            let lines = most_at_once(&list);
            // the room kept on each side: the router's margin, a tail behind a head
            let side = |at: f64| {
                let turns = |s: &&Seg| {
                    s.head.is_some_and(|(bend, head)| {
                        bend > a - 0.5 && bend < b + 0.5 && (head - at).abs() < 0.5
                    })
                };
                if list.iter().any(turns) { TAIL } else { MARGIN }
            };
            let sides = side(lo) + side(hi);
            if lines < 2 && sides <= 2.0 * MARGIN {
                continue;
            }
            let need = sides + lines.saturating_sub(1) as f64 * LINE_GAP;
            if need - (hi - lo) < 1.0 {
                continue;
            }
            let px = (need - (hi - lo)).ceil();
            // the place, within what its lines span
            let from = list.iter().map(|s| s.span.0).fold(f64::INFINITY, f64::min);
            let to = list
                .iter()
                .map(|s| s.span.1)
                .fold(f64::NEG_INFINITY, f64::max);
            let reach = (a.max(from).min(to), b.min(to).max(from));
            // one lack for the places of a corridor that follow each other
            let follows = |l: &&mut Lack| {
                (l.lo - lo).abs() < 0.5 && (l.hi - hi).abs() < 0.5 && l.reach.1 >= reach.0 - 0.5
            };
            match out.iter_mut().find(follows) {
                Some(l) => {
                    l.reach.1 = l.reach.1.max(reach.1);
                    l.px = l.px.max(px);
                }
                None => out.push(Lack { lo, hi, reach, px }),
            }
        }
    }
    out
}

/// The free corridor around `level` over the cross range `span`: `None` when a box stands on
/// that level or nothing bounds it on one side (`route::clean::gap_middle`).
fn corridor(frames: &[Box2], level: f64, span: (f64, f64), across: bool) -> Option<(f64, f64)> {
    let (mut lo, mut hi) = (f64::MIN, f64::MAX);
    for b in frames {
        let (c0, c1, f0, f1) = if across {
            (b.y, b.bottom(), b.x, b.right())
        } else {
            (b.x, b.right(), b.y, b.bottom())
        };
        if c1 <= span.0 + 0.5 || c0 >= span.1 - 0.5 {
            continue;
        }
        // a frame the level runs inside (a zone the corridor is in) bounds nothing here
        if f1 <= level + 0.5 {
            lo = lo.max(f1);
        } else if f0 >= level - 0.5 {
            hi = hi.min(f0);
        }
    }
    (lo > f64::MIN && hi < f64::MAX).then_some((lo, hi))
}

/// A cross segment in a corridor: the corridor's room, its line (an arrow, or the group of a
/// shared trunk), its span across the flow and the life of its arrow (steps).
type Run<'a> = (f64, &'a str, (f64, f64), (f64, f64));

/// The most lines of different arrows (a shared trunk is one) side by side at one place of the
/// corridor. (Counting a whole chain of lines side by side two by two, as the router's clusters
/// stack them, widened a column of the private corpus's `bug8-cas-reel` far enough for the
/// router to centre three arrows in U shape out in it, onto other lines: measured and refused.)
fn most_side_by_side(members: &[Run]) -> usize {
    // one span per line: its segments in this corridor, merged where they meet
    let mut spans: BTreeMap<&str, Vec<(f64, f64)>> = BTreeMap::new();
    for (_, line, s, _) in members {
        spans.entry(line).or_default().push(*s);
    }
    let mut events: Vec<(f64, i32)> = Vec::new();
    for list in spans.values_mut() {
        list.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut merged: Vec<(f64, f64)> = Vec::new();
        for s in list.iter() {
            match merged.last_mut() {
                Some(m) if s.0 <= m.1 + 0.5 => m.1 = m.1.max(s.1),
                _ => merged.push(*s),
            }
        }
        for m in merged {
            // two lines ending less than the router's margin apart on one level touch at their
            // corners: side by side (`2-dev/hive` of the private map, 4 px between two of them)
            events.push((m.0 - MARGIN / 2.0, 1));
            events.push((m.1 + MARGIN / 2.0, -1));
        }
    }
    events.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    let (mut now, mut most) = (0i32, 0i32);
    for (_, d) in events {
        now += d;
        most = most.max(now);
    }
    most.max(0) as usize
}

/// Widens the corridors of `lacking` (read on this same layout): every box starting at or past a
/// corridor's end moves along the flow by the room that corridor lacks (and by the room of every
/// corridor before it); a zone across that end grows, a card across it stays; ELK's route hints
/// follow.
pub fn widen(model: &Model, elk: &mut Placement, lacking: &[(f64, f64)]) {
    widen_on(model, elk, lacking, across(model));
}

/// [`widen`] along x (`across`: columns) or y (rows), whatever the drawing's direction.
pub fn widen_on(model: &Model, elk: &mut Placement, lacking: &[(f64, f64)], across: bool) {
    if lacking.is_empty() {
        return;
    }
    let shift = |v: f64| -> f64 {
        lacking
            .iter()
            .filter(|(end, _)| v >= end - 0.5)
            .map(|(_, d)| d)
            .sum()
    };
    let before = elk.boxes.clone();
    for (id, b) in &mut elk.boxes {
        let (start, stop) = if across {
            (b.x, b.right())
        } else {
            (b.y, b.bottom())
        };
        // a zone across a corridor's end grows; a card keeps its size (it stands beside the
        // corridor, in another column)
        let zone = model.node(id).is_some_and(|n| n.kind == Kind::Zone);
        let d0 = shift(start);
        let d1 = if zone { shift(stop) } else { d0 };
        if across {
            b.x += d0;
            b.w += d1 - d0;
        } else {
            b.y += d0;
            b.h += d1 - d0;
        }
    }
    // a note keeps its place beside the card it annotates (FORMAT, D-048 R7): a corridor
    // widened between them would set it apart and read as a card of its own
    for e in &model.edges {
        let Some((note, target)) = crate::tidy::notes::note_link(model, e) else {
            continue;
        };
        let (Some(t0), Some(t1)) = (before.get(target), elk.boxes.get(target).copied()) else {
            continue;
        };
        if let Some(b) = elk.boxes.get_mut(note) {
            let (dx, dy) = (t1.x - t0.x, t1.y - t0.y);
            let n0 = before.get(note).copied().unwrap_or(*b);
            b.x = n0.x + dx;
            b.y = n0.y + dy;
        }
    }
    let moved = |p: &mut (f64, f64)| {
        if across {
            p.0 += shift(p.0);
        } else {
            p.1 += shift(p.1);
        }
    };
    for (points, label) in elk.routes.values_mut() {
        points.iter_mut().for_each(moved);
        label.iter_mut().for_each(moved);
    }
    crate::trace::boxes("corridors", &before, &elk.boxes);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_side_by_side_count_once_per_arrow() {
        let all = (f64::NEG_INFINITY, f64::INFINITY);
        let m = [
            (59.0, "a", (0.0, 100.0), all),
            (59.0, "b", (50.0, 200.0), all),
            (59.0, "b", (200.0, 300.0), all),
            (59.0, "c", (250.0, 400.0), all),
        ];
        // a and b side by side, then b and c: two at most
        assert_eq!(most_side_by_side(&m), 2);
        let near = [
            (59.0, "a", (0.0, 100.0), all),
            (59.0, "b", (104.0, 200.0), all),
            (59.0, "c", (300.0, 400.0), all),
        ];
        // a and b end 4 px apart: they touch; c is far
        assert_eq!(most_side_by_side(&near), 2);
        let apart = [
            (59.0, "a", (0.0, 100.0), all),
            (59.0, "b", (150.0, 200.0), all),
        ];
        assert_eq!(most_side_by_side(&apart), 1);
        let m = [
            (59.0, "a", (0.0, 100.0), all),
            (59.0, "b", (10.0, 90.0), all),
            (59.0, "c", (20.0, 80.0), all),
        ];
        assert_eq!(most_side_by_side(&m), 3);
    }

    /// Three cards and the arrow of `c`, which runs between `a` and `b` and turns into `b`.
    fn turning() -> (Model, BTreeMap<String, Box2>, BTreeMap<String, Points>) {
        let model =
            skemo_lang::parse("diagram\n  card#a A\n  card#b B\n  card#c C\n    to(b)\n").model;
        let at = |x: f64, y: f64| Box2 {
            x,
            y,
            w: 100.0,
            h: 50.0,
        };
        // `a` above `b`, 30 px apart; `c` far on the right
        let boxes = [
            ("a", at(0.0, 0.0)),
            ("b", at(0.0, 80.0)),
            ("c", at(300.0, 40.0)),
        ]
        .into_iter()
        .map(|(k, b)| (k.to_string(), b))
        .collect();
        let line = vec![(300.0, 65.0), (50.0, 65.0), (50.0, 80.0)];
        (model, boxes, [("c->b".to_string(), line)].into())
    }

    #[test]
    fn a_line_that_turns_into_its_head_needs_a_tail_behind_it() {
        let (model, boxes, routes) = turning();
        let groups = BTreeMap::new();
        let read =
            |tails: bool| lacks(&model, (&boxes, &[], &[]), (&routes, &groups), false, tails);
        // one line in 30 px: the router's margins hold
        assert_eq!(read(false), Vec::new());
        // its head stands on what ends the corridor: a margin above, a tail under, 38 px,
        // where the two cards face each other
        assert_eq!(
            read(true),
            vec![Lack {
                lo: 50.0,
                hi: 80.0,
                reach: (50.0, 100.0),
                px: MARGIN + TAIL - 30.0,
            }]
        );
    }

    #[test]
    fn a_corridor_is_read_where_its_lines_stand_side_by_side() {
        let (model, mut boxes, _) = turning();
        // `b` under `a`, far apart; `c` beside the gap, further right: three upright lines
        // pass between `a` (on their left, above) and `c` (on their right, below)
        boxes.get_mut("b").expect("b").y = 400.0;
        boxes.insert(
            "c".to_string(),
            Box2 {
                x: 150.0,
                y: 200.0,
                w: 100.0,
                h: 50.0,
            },
        );
        let upright = |x: f64| vec![(x, -40.0), (x, 380.0)];
        let routes: BTreeMap<String, Points> = [
            ("l1".to_string(), upright(112.0)),
            ("l2".to_string(), upright(125.0)),
            ("l3".to_string(), upright(138.0)),
        ]
        .into();
        let groups = BTreeMap::new();
        let drawn: Drawn = (&boxes, &[], &[]);
        // over their whole reach the three lines run in 50 px, between two cards that do not
        // face each other: no gutter lies between those
        assert_eq!(
            lacking_on(&model, drawn, (&routes, &groups), true),
            vec![(150.0, 18.0)]
        );
        // place by place nothing bounds them on both sides: nothing to widen
        assert_eq!(
            lacks(&model, drawn, (&routes, &groups), true, true),
            Vec::new()
        );
        // `c` level with `a`: they face each other, the corridor between them lacks 18 px
        boxes.get_mut("c").expect("c").y = 10.0;
        assert_eq!(
            lacks(&model, (&boxes, &[], &[]), (&routes, &groups), true, true),
            vec![Lack {
                lo: 100.0,
                hi: 150.0,
                reach: (10.0, 50.0),
                px: 18.0,
            }]
        );
    }

    #[test]
    fn a_corridor_is_bounded_on_both_sides_over_the_span() {
        let card = |x: f64, y: f64| Box2 {
            x,
            y,
            w: 100.0,
            h: 50.0,
        };
        let frames = [card(0.0, 0.0), card(0.0, 110.0), card(300.0, 120.0)];
        // under the first card, above the second: 50 to 110
        assert_eq!(
            corridor(&frames, 80.0, (10.0, 90.0), false),
            Some((50.0, 110.0))
        );
        // nothing below over x 150..250
        assert_eq!(corridor(&frames, 80.0, (150.0, 250.0), false), None);
    }
}
