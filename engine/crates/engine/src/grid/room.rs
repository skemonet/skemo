//! Room for the lines and for the arrow labels, made evenly (the manual method's §7).
//!
//! What a routing lacks somewhere (a corridor too narrow for its lines or for the tail behind a
//! head, a stretch too short for a label) is never taken there alone: it is asked of the gap that
//! owns the place, the gutters of a group along an axis or a zone's padding, and that gap grows
//! the same between all the children of the group (`mosaic::Ctx::gap`). The owner of the gap
//! between two boxes is the lowest group that holds both; the owner of a stretch along a zone's
//! border, seen from inside, is that zone's padding.
//!
//! A gutter is asked what it lacks (px more: the corridor grows by as much). A padding is asked
//! what it must be (px in all): a zone stretched to its row keeps its content centred, further
//! from its border than its padding, and a padding grown by what lacks would change nothing.

use super::lanes::{Around, Track};
use crate::corridors::Lack;
use crate::layout::Box2;
use crate::measure::Spacing;
use crate::route::Points;
use skemo_lang::{Kind, Model};
use std::collections::BTreeMap;

/// The arrow labels of a layout: their centres, then their sizes.
pub(crate) type Labels<'a> = (
    &'a BTreeMap<String, (f64, f64)>,
    &'a BTreeMap<String, crate::measure::Size>,
);

/// The lowest group that holds both `a` and `b`: a zone's id, or the empty string (the drawing).
fn common(model: &Model, a: &str, b: &str) -> String {
    let up = model.ancestors(b);
    model
        .ancestors(a)
        .iter()
        .find(|z| up.iter().any(|o| o.id == z.id))
        .map_or_else(String::new, |z| z.id.clone())
}

/// The group that holds `id`: a zone's id, or the empty string (the drawing).
fn group_of(model: &Model, id: &str) -> String {
    model
        .node(id)
        .and_then(|n| n.parent.clone())
        .unwrap_or_default()
}

/// The room between a zone's border and its content, on its left (`0`), its right (`1`) or
/// under its top (`2`): `None` when it holds nothing drawn.
fn margin(model: &Model, boxes: &BTreeMap<String, Box2>, zone: &str, side: usize) -> Option<f64> {
    let z = boxes.get(zone)?;
    let kids = || {
        model
            .children(Some(zone))
            .filter(|n| n.at.is_none())
            .filter_map(|n| boxes.get(&n.id))
    };
    let room = match side {
        0 => kids().map(|b| b.x).fold(f64::INFINITY, f64::min) - z.x,
        1 => z.right() - kids().map(Box2::right).fold(f64::NEG_INFINITY, f64::max),
        _ => kids().map(|b| b.y).fold(f64::INFINITY, f64::min) - z.y,
    };
    room.is_finite().then_some(room)
}

/// Does a label `size` centred at `c` stand on a card or astride a zone's border?
fn hides(c: (f64, f64), (w, h): (f64, f64), cards: &[Box2], frames: &[Box2]) -> bool {
    let l = Box2 {
        x: c.0 - w / 2.0,
        y: c.1 - h / 2.0,
        w,
        h,
    };
    cards.iter().any(|b| super::lanes::meet(&l, b))
        || frames.iter().any(|z| super::lanes::astride(&l, z))
}

/// Asks `px` of the gap `key`: the largest request of a reading is kept (one gutter serves all
/// the places of its group).
fn ask(out: &mut Spacing, key: (String, bool, bool), px: f64) {
    let room = out.entry(key).or_insert(0.0);
    *room = room.max(px);
}

/// Two readings in one: for each gap, the largest request.
pub(crate) fn merge(out: &mut Spacing, more: Spacing) {
    for (key, px) in more {
        ask(out, key, px);
    }
}

/// The room the lines lack (`lacks`: the corridors too narrow along x, `across`, or along y, as
/// `corridors::lacks` reads them), by the gap that owns each corridor: the gutters of the lowest
/// group that holds what bounds it on both sides; under a zone's title, that zone's head.
pub(crate) fn line_room(
    model: &Model,
    boxes: &BTreeMap<String, Box2>,
    titles: &[(String, Box2)],
    lacks: &[Lack],
    across: bool,
) -> Spacing {
    // what bounds a corridor (as `corridors::lacks`): the cards, the drawn zones, the titles
    let bounds: Vec<(&str, Box2, bool)> = model
        .nodes
        .iter()
        .filter(|n| !n.stack && !n.classes.iter().any(|c| c == "bare"))
        .filter(|n| n.kind == Kind::Card || !crate::layout::invisible(n))
        .filter_map(|n| boxes.get(&n.id).map(|b| (n.id.as_str(), *b, false)))
        .chain(titles.iter().map(|(id, b)| (id.as_str(), *b, true)))
        .collect();
    let along = |b: &Box2| {
        if across {
            (b.x, b.right())
        } else {
            (b.y, b.bottom())
        }
    };
    let cross = |b: &Box2| {
        if across {
            (b.y, b.bottom())
        } else {
            (b.x, b.right())
        }
    };
    let mut out = Spacing::new();
    for l in lacks {
        let level = |b: &Box2| cross(b).1 > l.reach.0 + 0.5 && cross(b).0 < l.reach.1 - 0.5;
        let before = bounds
            .iter()
            .filter(|(_, b, _)| level(b) && (along(b).1 - l.lo).abs() < 0.5);
        for (a, _, a_title) in before {
            let after = bounds
                .iter()
                .filter(|(_, b, _)| level(b) && (along(b).0 - l.hi).abs() < 0.5);
            for (b, _, b_title) in after {
                let (key, px) = match (a_title, b_title) {
                    (false, false) => ((common(model, a, b), across, false), l.px),
                    // under a zone's title, above its first row: the zone's head
                    (true, false) if !across => {
                        let Some(head) = margin(model, boxes, a, 2) else {
                            continue;
                        };
                        (((*a).to_string(), false, true), head + l.px)
                    }
                    _ => continue,
                };
                if crate::trace::wanted("room") {
                    eprintln!(
                        "trace room lines {a} | {b}: {px} px at {key:?} ({:.0}..{:.0})",
                        l.lo, l.hi
                    );
                }
                ask(&mut out, key, px);
            }
        }
    }
    out
}

/// What bounds a label's stretch on one side: a card or a zone beside it, or the border of the
/// zone it lies in.
enum Bound<'a> {
    Beside(&'a str),
    Inside(&'a str),
}

/// The room the labels of a routing lack (a label never sits on a card nor astride a zone's
/// border): each label needs, somewhere on its arrow, a stretch clear of the cards and of the
/// zone borders as wide as it is (`labels::slots`, the places `labels::place` then finds);
/// when it has none, the stretch that lacks the least is widened. The lines of the other arrows
/// that run upright through a stretch stand beside the label (`lanes::fit`). Returns the room
/// lacked along x (a label is one line of text: the gaps are high enough for it), by the gap
/// that owns each stretch: the gutters of the lowest group that holds what stands on both its
/// sides, or the padding of the zone whose border ends it. A name rather than a place, as the
/// widened drawing is read again for what its labels still lack (`diagram::arrange_wrapped`).
pub(crate) fn label_room(
    model: &Model,
    boxes: &BTreeMap<String, Box2>,
    titles: &[(String, Box2)],
    (routes, groups): (&BTreeMap<String, Points>, &BTreeMap<String, String>),
    labels: Labels,
) -> Spacing {
    let (centres, sizes) = labels;
    let around = Around::new(model, boxes, titles, groups, sizes);
    let all = super::cards(model, boxes);
    let frames: Vec<(&str, Box2)> = model
        .nodes
        .iter()
        .filter(|n| n.kind == Kind::Zone && !crate::layout::invisible(n))
        .filter_map(|n| boxes.get(&n.id).map(|b| (n.id.as_str(), *b)))
        .collect();
    let only = |v: &[(&str, Box2)]| -> Vec<Box2> { v.iter().map(|(_, b)| *b).collect() };
    let (card_boxes, frame_boxes) = (only(&all), only(&frames));
    // level with the rows of a label, as `labels::walls` reads it
    let level = |b: &Box2, rows: (f64, f64)| b.y < rows.1 + 2.0 && rows.0 - 2.0 < b.bottom();
    // what ends a stretch on the rows `rows`: the zone or the card that starts there, else the
    // zone that ends there (the stretch lies in it)
    let ends = |at: f64, rows: (f64, f64)| {
        let starts = |(_, b): &&(&str, Box2)| (b.x - at).abs() < 0.5 && level(b, rows);
        let stops = |(_, b): &&(&str, Box2)| (b.right() - at).abs() < 0.5 && level(b, rows);
        (frames.iter().find(starts))
            .or_else(|| all.iter().find(starts))
            .map(|(id, _)| Bound::Beside(id))
            .or_else(|| frames.iter().find(stops).map(|(id, _)| Bound::Inside(id)))
    };
    // what starts the gap a stretch lies in (`from`: where the gap starts, the wall's margin
    // counted): the zone or the card that ends there, else the zone that starts there
    let starts = |from: f64, rows: (f64, f64)| {
        let room = crate::labels::LABEL_ROOM;
        let zone = |(_, b): &&(&str, Box2)| (b.right() + room - from).abs() < 0.5 && level(b, rows);
        let card = |(_, b): &&(&str, Box2)| {
            (b.right() + crate::labels::PAD - from).abs() < 0.5 && level(b, rows)
        };
        let inside = |(_, b): &&(&str, Box2)| (b.x + room - from).abs() < 0.5 && level(b, rows);
        (frames.iter().find(zone))
            .or_else(|| all.iter().find(card))
            .map(|(id, _)| Bound::Beside(id))
            .or_else(|| frames.iter().find(inside).map(|(id, _)| Bound::Inside(id)))
    };
    let tracks = if super::on("grid-label-lanes") {
        super::lanes::tracks(routes, groups)
    } else {
        Vec::new()
    };
    let mut out = Spacing::new();
    // the labels shown: one of a join's arrows carries the label they share
    for id in centres.keys() {
        let (Some(s), Some(r)) = (sizes.get(id), routes.get(id)) else {
            continue;
        };
        // the other arrows' upright lines (not those of its join or fork: they share the label)
        let lines: Vec<&Track> = tracks
            .iter()
            .filter(|t| super::lanes::foreign(groups, t.line, id))
            .collect();
        // each place of the label: its rows, where its gap starts, where what ends it starts,
        // the room it lacks
        let found: Vec<Place> = crate::labels::slots(r, (s.w, s.h), &card_boxes, &frame_boxes)
            .iter()
            .map(|slot| {
                let ok = |plan: &[(usize, f64)]| {
                    super::lanes::refusal(&around, routes, &lines, plan).is_none()
                };
                let lack = super::lanes::fit(slot, s.w, None, &lines, &ok).lack;
                let lack = lack.max(slot.lack(s.w));
                (slot.rows, slot.gap.0, slot.ends.1, lack)
            })
            .collect();
        // a place is wide enough, and the label hides nothing where it stands: it has its room.
        // When it stands on a card or astride a border all the same, something else holds the
        // places that looked wide enough (another label, a level line): one of the others widens.
        // When a line runs through it or another label lies over it, and every place looked wide
        // enough, a line could not stand aside there after all (it would cross one more, `lanes`):
        // those places lack a lane
        let hides = centres.get(id).is_some_and(|c| {
            hides(*c, (s.w, s.h), &card_boxes, &frame_boxes)
                || super::lanes::faults(&around, routes, centres, id, false) > 0
        });
        if !hides && found.iter().any(|f| f.3 <= 0.0) {
            continue;
        }
        // those something ends (not a bend of the line), with the gap that owns each, what it
        // lacks and what is asked of it, the one that lacks the least first
        let mut gaps: Vec<((String, bool, bool), f64, f64)> = asked(&found)
            .into_iter()
            .filter_map(|(rows, from, at, lack)| {
                // a zone's padding: the room its content keeps on that side, and what lacks
                let padded = |z: &str, side: usize| {
                    let room = margin(model, boxes, z, side)?;
                    Some(((z.to_string(), true, true), lack, (room + lack).ceil()))
                };
                match (starts(from, rows), ends(at, rows)?) {
                    (_, Bound::Inside(z)) => padded(z, 1),
                    (Some(Bound::Inside(z)), _) => padded(z, 0),
                    (Some(Bound::Beside(a)), Bound::Beside(b)) => {
                        Some(((common(model, a, b), true, false), lack, lack))
                    }
                    // nothing before on those rows: the gap before what ends the stretch
                    (None, Bound::Beside(b)) => {
                        Some(((group_of(model, b), true, false), lack, lack))
                    }
                }
            })
            .collect();
        gaps.sort_by(|a, b| a.1.total_cmp(&b.1));
        let Some(least) = gaps.first().map(|g| g.1) else {
            continue;
        };
        // a gutter between two blocks rather than a zone's padding, when it lacks little more:
        // the zones keep their padding, and the label of an arrow from a zone to the next
        // stands between them
        let gutter = gaps.iter().find(|g| !g.0.2 && g.1 <= least + GUTTER_FIRST);
        if let Some((key, lack, px)) = gutter.or(gaps.first()).cloned() {
            if crate::trace::wanted(id) || crate::trace::wanted("room") {
                eprintln!("trace room label {id}: lacks {lack} px, {px} px at {key:?}");
            }
            ask(&mut out, key, px);
        }
    }
    out
}

/// A place of a label: its rows, where its gap starts, where what ends it starts, the room it
/// lacks (px; none or less: it fits; infinite: no line may stand aside there).
type Place = ((f64, f64), f64, f64, f64);

/// The places of a hidden label that are asked for room, with what each lacks: those too
/// narrow, when some are; else every place, a lane more (a line could not stand aside there
/// after all).
fn asked(found: &[Place]) -> Vec<Place> {
    let short = found.iter().any(|f| f.3 > 0.0 && f.3.is_finite());
    found
        .iter()
        .filter(|f| !short || (f.3 > 0.0 && f.3.is_finite()))
        .map(|&(rows, from, at, lack)| {
            let lack = if short {
                lack.ceil()
            } else if lack.is_finite() {
                (lack.max(0.0) + crate::labels::LANE).ceil()
            } else {
                // no line may stand aside there yet: a lane more may let one
                crate::labels::LANE
            };
            (rows, from, at, lack)
        })
        .collect()
}

/// How much more room a gutter may lack than a zone's padding and still be the one widened for
/// a label (px): about what the padding is wider than the gap between two zones.
const GUTTER_FIRST: f64 = 16.0;

#[cfg(test)]
mod tests {
    use super::*;

    /// Two cards in a zone, a zone in a zone beside a card, a card alone.
    fn model() -> Model {
        let src = "diagram(layout=\"grid\")
  zone#z Z
    card#a A
    card#b B
  zone#y Y
    zone#y1 Y1
      card#c C
      card#c2 C2
    card#d D
  zone#empty Empty
  card#e E
";
        skemo_lang::parse(src).model
    }

    fn at(x: f64, y: f64, w: f64, h: f64) -> Box2 {
        Box2 { x, y, w, h }
    }

    #[test]
    fn a_gap_belongs_to_the_lowest_group_that_holds_both_sides() {
        let m = model();
        assert_eq!(common(&m, "a", "b"), "z");
        assert_eq!(common(&m, "c", "c2"), "y1");
        assert_eq!(common(&m, "c", "d"), "y");
        assert_eq!(common(&m, "y1", "d"), "y");
        // two blocks, or a card of one and a card of the other: the drawing's gutters
        assert_eq!(common(&m, "z", "y"), "");
        assert_eq!(common(&m, "a", "c"), "");
        assert_eq!(common(&m, "a", "e"), "");
        assert_eq!(group_of(&m, "c"), "y1");
        assert_eq!(group_of(&m, "y1"), "y");
        assert_eq!(group_of(&m, "e"), "");
    }

    #[test]
    fn a_margin_is_the_room_a_zone_keeps_on_one_side() {
        let m = model();
        let boxes: BTreeMap<String, Box2> = [
            ("z", at(0.0, 0.0, 300.0, 200.0)),
            ("a", at(30.0, 50.0, 100.0, 40.0)),
            ("b", at(160.0, 50.0, 100.0, 40.0)),
            ("empty", at(400.0, 0.0, 100.0, 100.0)),
        ]
        .into_iter()
        .map(|(k, b)| (k.to_string(), b))
        .collect();
        assert_eq!(margin(&m, &boxes, "z", 0), Some(30.0));
        assert_eq!(margin(&m, &boxes, "z", 1), Some(40.0));
        assert_eq!(margin(&m, &boxes, "z", 2), Some(50.0));
        // nothing drawn in it, or not placed: no margin to grow
        assert_eq!(margin(&m, &boxes, "empty", 0), None);
        assert_eq!(margin(&m, &boxes, "y", 0), None);
    }

    #[test]
    fn a_corridor_too_narrow_widens_the_gutters_of_its_group() {
        let m = model();
        let boxes: BTreeMap<String, Box2> = [
            ("z", at(0.0, 0.0, 300.0, 200.0)),
            ("a", at(30.0, 50.0, 100.0, 40.0)),
            ("b", at(160.0, 50.0, 100.0, 40.0)),
            ("e", at(330.0, 50.0, 100.0, 40.0)),
        ]
        .into_iter()
        .map(|(k, b)| (k.to_string(), b))
        .collect();
        let lack = |lo: f64, hi: f64, reach: (f64, f64), px: f64| Lack { lo, hi, reach, px };
        // between the two cards of the zone: the zone's gutters along x
        let inside = line_room(
            &m,
            &boxes,
            &[],
            &[lack(130.0, 160.0, (50.0, 90.0), 12.0)],
            true,
        );
        assert_eq!(inside.get(&("z".to_string(), true, false)), Some(&12.0));
        assert_eq!(inside.len(), 1);
        // between the zone and the card beside it: the drawing's
        let between = line_room(
            &m,
            &boxes,
            &[],
            &[lack(300.0, 330.0, (50.0, 90.0), 8.0)],
            true,
        );
        assert_eq!(between.get(&(String::new(), true, false)), Some(&8.0));
        // a corridor that nothing level with it bounds is nobody's
        let none = line_room(
            &m,
            &boxes,
            &[],
            &[lack(130.0, 160.0, (300.0, 400.0), 12.0)],
            true,
        );
        assert!(none.is_empty());
    }

    #[test]
    fn a_corridor_under_a_title_asks_what_the_head_of_its_zone_must_be() {
        let m = model();
        let boxes: BTreeMap<String, Box2> = [
            ("z", at(0.0, 0.0, 300.0, 200.0)),
            ("a", at(30.0, 50.0, 100.0, 40.0)),
        ]
        .into_iter()
        .map(|(k, b)| (k.to_string(), b))
        .collect();
        let titles = [("z".to_string(), at(10.0, 5.0, 60.0, 20.0))];
        let lacks = [Lack {
            lo: 25.0,
            hi: 50.0,
            reach: (30.0, 60.0),
            px: 9.0,
        }];
        // the first row stands 50 px under the top: 9 px more is 59 px in all
        let down = line_room(&m, &boxes, &titles, &lacks, false);
        assert_eq!(down.get(&("z".to_string(), false, true)), Some(&59.0));
        assert_eq!(down.len(), 1);
    }

    #[test]
    fn the_largest_request_of_a_gap_is_kept() {
        let key = |g: &str| (g.to_string(), true, false);
        let mut out = Spacing::new();
        ask(&mut out, key("z"), 10.0);
        ask(&mut out, key("z"), 4.0);
        let mut more = Spacing::new();
        ask(&mut more, key("z"), 12.0);
        ask(&mut more, key(""), 6.0);
        merge(&mut out, more);
        assert_eq!(out.get(&key("z")), Some(&12.0));
        assert_eq!(out.get(&key("")), Some(&6.0));
    }
}
