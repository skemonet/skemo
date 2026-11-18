//! Linked cards lined up: two linked cards one above the other but a little offset make a jog in
//! their arrow; the content of the zone of one of them slides inside that zone until they line
//! up, when the zone is wider than its content and its padding.
//!
//! The content slides as one: the gaps between the cards of a group stay even (the manual
//! method's §7), which the column of one card moved alone did not keep. Nothing slides down
//! either: the first row of a zone stays under its title.

use crate::layout::Box2;
use skemo_lang::{Kind, Model};
use std::collections::BTreeMap;

/// Lines up the linked cards that stand one above the other a little offset. A move is kept
/// only if more arrows then run straight. `side`: the room a zone keeps between its border and
/// its content along x (its padding, with the room made for its labels).
pub(super) fn linked(
    model: &Model,
    boxes: &mut BTreeMap<String, Box2>,
    titles: &[(String, Box2)],
    side: &dyn Fn(&str) -> f64,
) {
    for _ in 0..3 {
        let mut moved = false;
        for e in &model.edges {
            let (Some(a), Some(b)) = (boxes.get(&e.from).copied(), boxes.get(&e.to).copied())
            else {
                continue;
            };
            if !is_card(model, &e.from) || !is_card(model, &e.to) {
                continue;
            }
            // facing: one above the other, their x spans overlapping
            let above = a.bottom() <= b.y || b.bottom() <= a.y;
            let d = (b.x + b.w / 2.0) - (a.x + a.w / 2.0);
            if !above || d.abs() < 0.5 || d.abs() > a.w.min(b.w) / 2.0 {
                continue;
            }
            // the content of `a`'s zone slides by `d`, else that of `b`'s by `-d`, only if more
            // arrows then run straight (a move never breaks a line already straight)
            let before = straight(model, boxes, titles);
            let mut tried = |id: &str, by: f64| {
                let keep = boxes.clone();
                let ok = slide(model, boxes, id, by, side) && {
                    let after = straight(model, boxes, titles);
                    after.0 > before.0 && after.1 >= before.1
                };
                if !ok {
                    *boxes = keep;
                }
                ok
            };
            moved |= tried(&e.from, d) || tried(&e.to, -d);
        }
        if !moved {
            break;
        }
    }
}

/// The arrows between two cards facing each other: lined up (centres within half a pixel), and
/// with room for one straight segment (their sides share 28 px or more, the router's margins);
/// in both cases the straight line between them must cross no other card and no zone title.
fn straight(
    model: &Model,
    boxes: &BTreeMap<String, Box2>,
    titles: &[(String, Box2)],
) -> (usize, usize) {
    let mut out = (0, 0);
    let pairs = model
        .edges
        .iter()
        .filter(|e| is_card(model, &e.from) && is_card(model, &e.to))
        .filter_map(|e| Some((&e.from, &e.to, boxes.get(&e.from)?, boxes.get(&e.to)?)));
    for (ia, ib, a, b) in pairs {
        // the segment between the two cards, on the middle of what their sides share
        let line = straight_line(a, b);
        let blocked = line.is_some_and(|l| {
            let hit = |o: &Box2| {
                l.x < o.right() + 14.0
                    && o.x < l.right() + 14.0
                    && l.y < o.bottom() + 14.0
                    && o.y < l.bottom() + 14.0
            };
            titles.iter().any(|(_, t)| hit(t))
                || boxes
                    .iter()
                    .any(|(o, ob)| o != ia && o != ib && is_card(model, o) && hit(ob))
        });
        if blocked {
            continue;
        }
        let share = |s: (f64, f64), t: (f64, f64)| s.1.min(t.1) - s.0.max(t.0);
        let (col, row) = (
            a.bottom() <= b.y || b.bottom() <= a.y,
            a.right() <= b.x || b.right() <= a.x,
        );
        let mid = |p: f64, w: f64, q: f64, v: f64| ((p + w / 2.0) - (q + v / 2.0)).abs() < 0.5;
        if (col && mid(a.x, a.w, b.x, b.w)) || (row && mid(a.y, a.h, b.y, b.h)) {
            out.0 += 1;
        }
        if (col && share((a.x, a.right()), (b.x, b.right())) >= 28.0)
            || (row && share((a.y, a.bottom()), (b.y, b.bottom())) >= 28.0)
        {
            out.1 += 1;
        }
    }
    out
}

/// The straight segment between two facing cards, as a thin box on the middle of what their
/// sides share; `None` when they do not face each other.
fn straight_line(a: &Box2, b: &Box2) -> Option<Box2> {
    let (top, bottom) = if a.y <= b.y { (a, b) } else { (b, a) };
    let (left, right) = if a.x <= b.x { (a, b) } else { (b, a) };
    if top.bottom() <= bottom.y {
        let (x0, x1) = (a.x.max(b.x), a.right().min(b.right()));
        (x1 > x0).then(|| Box2 {
            x: f64::midpoint(x0, x1),
            y: top.bottom(),
            w: 0.0,
            h: bottom.y - top.bottom(),
        })
    } else if left.right() <= right.x {
        let (y0, y1) = (a.y.max(b.y), a.bottom().min(b.bottom()));
        (y1 > y0).then(|| Box2 {
            x: left.right(),
            y: f64::midpoint(y0, y1),
            w: right.x - left.right(),
            h: 0.0,
        })
    } else {
        None
    }
}

fn is_card(model: &Model, id: &str) -> bool {
    model.node(id).is_some_and(|n| n.kind == Kind::Card)
}

/// Slides the whole content of the zone that holds card `id` by `d` along x, if it stays that
/// zone's padding from its borders; `true` when it moved. A card that stands in no zone does
/// not move: the gaps between the blocks stay even.
fn slide(
    model: &Model,
    boxes: &mut BTreeMap<String, Box2>,
    id: &str,
    d: f64,
    side: &dyn Fn(&str) -> f64,
) -> bool {
    let Some(zone) = model.node(id).and_then(|n| n.parent.clone()) else {
        return false;
    };
    let Some(frame) = boxes.get(&zone).copied() else {
        return false;
    };
    let pad = side(&zone);
    let kids = || model.children(Some(&zone)).filter_map(|n| boxes.get(&n.id));
    let left = kids().map(|b| b.x).fold(f64::INFINITY, f64::min);
    let right = kids().map(Box2::right).fold(f64::NEG_INFINITY, f64::max);
    if left + d < frame.x + pad - 0.5 || right + d > frame.right() - pad + 0.5 {
        return false;
    }
    let held: Vec<String> = model
        .nodes
        .iter()
        .filter(|n| model.ancestors(&n.id).iter().any(|a| a.id == zone))
        .map(|n| n.id.clone())
        .collect();
    for k in &held {
        if let Some(b) = boxes.get_mut(k) {
            b.x += d;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A card in a zone above a card in another, linked; a second card beside the first.
    fn model() -> Model {
        let src = "diagram(layout=\"grid\")
  zone#z Z
    card#a A
      to(c)
    card#b B
  zone#y Y
    card#c C
  card#e E
";
        skemo_lang::parse(src).model
    }

    fn boxes() -> BTreeMap<String, Box2> {
        let at = |x: f64, y: f64, w: f64, h: f64| Box2 { x, y, w, h };
        [
            ("z", at(0.0, 0.0, 400.0, 100.0)),
            ("a", at(60.0, 40.0, 100.0, 40.0)),
            ("b", at(188.0, 40.0, 100.0, 40.0)),
            ("y", at(0.0, 128.0, 400.0, 100.0)),
            ("c", at(90.0, 168.0, 100.0, 40.0)),
            ("e", at(428.0, 40.0, 100.0, 40.0)),
        ]
        .into_iter()
        .map(|(k, b)| (k.to_string(), b))
        .collect()
    }

    #[test]
    fn the_content_of_a_zone_slides_as_one_within_its_padding() {
        let m = model();
        let mut b = boxes();
        let pad = |_: &str| 20.0;
        assert!(slide(&m, &mut b, "a", 30.0, &pad));
        // both cards moved: the gap between them is what it was
        assert_eq!((b["a"].x, b["b"].x), (90.0, 218.0));
        assert_eq!(b["z"], boxes()["z"]);
        // not into the padding, on either side
        assert!(!slide(&m, &mut b, "a", -80.0, &pad));
        assert!(!slide(&m, &mut b, "a", 80.0, &pad));
        // a card in no zone stays: the gaps between the blocks are even
        assert!(!slide(&m, &mut b, "e", 10.0, &pad));
        assert_eq!(b["e"].x, 428.0);
    }

    #[test]
    fn two_linked_cards_one_above_the_other_line_up() {
        let m = model();
        let mut b = boxes();
        linked(&m, &mut b, &[], &|_| 20.0);
        // `a` stood 30 px left of `c`: the content of its zone slid over it
        assert_eq!(b["a"].x, b["c"].x);
        assert_eq!(b["b"].x - b["a"].right(), 28.0);
        // with no room in the zones, nothing moves
        let mut tight = boxes();
        linked(&m, &mut tight, &[], &|_| 90.0);
        assert_eq!(tight, boxes());
    }
}
