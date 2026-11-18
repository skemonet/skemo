//! Cards off the flow: notes beside the card they annotate, targets of cross links beside the
//! drawing.

use super::geom::{Axes, Boxes, fits, in_grid, is_card, whole};
use crate::layout::Box2;
use skemo_lang::{Edge, Kind, Model};

/// The note a `.link` arrow attaches, and the card it annotates: `(note, target)`. A note is a
/// `.note` card whose only arrows are this one link.
pub(crate) fn note_link<'a>(model: &'a Model, e: &'a Edge) -> Option<(&'a str, &'a str)> {
    if !e.classes.iter().any(|c| c == "link") {
        return None;
    }
    let is_note = |id: &str| {
        model
            .node(id)
            .is_some_and(|n| n.kind == Kind::Card && n.classes.iter().any(|c| c == "note"))
    };
    let (note, target) = if is_note(&e.from) {
        (e.from.as_str(), e.to.as_str())
    } else if is_note(&e.to) {
        (e.to.as_str(), e.from.as_str())
    } else {
        return None;
    };
    let arrows = model
        .edges
        .iter()
        .filter(|f| f.from == note || f.to == note)
        .count();
    (arrows == 1).then_some((note, target))
}

/// The model as the layout sees it (D-048, R7): the `.link` of a note does not shape the layout,
/// like `constraint="false"`, so the note does not take a row of the flow.
pub fn notes_unconstrained(model: &Model) -> Model {
    let mut m = model.clone();
    let links: Vec<String> = model
        .edges
        .iter()
        .filter(|e| note_link(model, e).is_some())
        .map(|e| e.id.clone())
        .collect();
    for e in m.edges.iter_mut().filter(|e| links.contains(&e.id)) {
        e.constraint = false;
    }
    m
}

/// Distance between a note and the card it annotates (D-048, R7).
pub(crate) const GAP: f64 = 24.0;

/// The two places of note `nb` beside its card `tb`, centred on it along the flow, `GAP` away
/// across, in the order they are tried: its right then its left with `dir` down or up, above
/// then below with right or left.
fn beside(ax: Axes, nb: Box2, tb: Box2) -> [Box2; 2] {
    let centred = ax.flow(&tb).0 + (ax.flow_len(&tb) - ax.flow_len(&nb)) / 2.0;
    let after = ax.at(nb, centred, ax.span(&tb).1 + GAP);
    let before = ax.at(nb, centred, ax.span(&tb).0 - GAP - ax.span_len(&nb));
    if ax.across {
        [before, after]
    } else {
        [after, before]
    }
}

/// Is there a free place for `note` beside `target` (`beside`)?
pub(crate) fn room_beside(model: &Model, note: &str, target: &str, boxes: &Boxes) -> bool {
    let (Some(nb), Some(tb)) = (boxes.get(note).copied(), boxes.get(target).copied()) else {
        return true;
    };
    beside(Axes::of(model), nb, tb)
        .iter()
        .any(|b| fits(model, note, b, &[note], boxes))
}

/// Notes set beside the card they annotate, off the flow (D-048, R7): above it with `dir` right
/// or left, on its right with down or up (then the other side), centred on it, 24 px away, if
/// the place is free; else beyond everything at the target's level, on the side of the second
/// spot's opposite (the right with `dir` down, below with right). Otherwise the note stays where
/// the layout put it.
pub fn place_notes(model: &Model, boxes: &mut Boxes) {
    let ax = Axes::of(model);
    for e in &model.edges {
        let Some((note, target)) = note_link(model, e) else {
            continue;
        };
        if in_grid(model, note) {
            continue;
        }
        let (Some(nb), Some(tb)) = (boxes.get(note).copied(), boxes.get(target).copied()) else {
            continue;
        };
        let spots = beside(ax, nb, tb);
        let centred = ax.flow(&spots[0]).0;
        let level = |b: &Box2| ax.flow(b).0 < ax.flow(&tb).1 && ax.flow(&tb).0 < ax.flow(b).1;
        let far = boxes
            .iter()
            .filter(|(id, b)| id.as_str() != note && level(b))
            .map(|(_, b)| ax.span(b).1)
            .fold(f64::NEG_INFINITY, f64::max);
        let beyond = ax.at(nb, centred, far + GAP);
        if let Some(b) = spots
            .into_iter()
            .chain(std::iter::once(beyond))
            .find(|b| fits(model, note, b, &[note], boxes))
        {
            boxes.insert(note.to_string(), whole(b));
        }
    }
}

/// A note beside the card it annotates stays beside it (bug 25): a pass that moved the card and
/// left its note behind sets the note on the free side of the card nearest to where it stands (it
/// moves the least; moved as far as the card, a note on the far side widened its zone, which let a
/// stacked zone stretch). Only a note `place_notes` set beside its card (`GAP` away) follows;
/// before that it stands where ELK put it, and moving it along changed what the passes saw. Run
/// after every named pass, so that no pass has to know about notes.
pub fn follow(model: &Model, before: &Boxes, boxes: &mut Boxes) {
    for e in &model.edges {
        let Some((note, target)) = note_link(model, e) else {
            continue;
        };
        let (Some(t0), Some(t1), Some(n0), Some(n1)) = (
            before.get(target),
            boxes.get(target).copied(),
            before.get(note),
            boxes.get(note).copied(),
        ) else {
            continue;
        };
        let (dx, dy) = (t1.x - t0.x, t1.y - t0.y);
        if (dx.abs() < 0.5 && dy.abs() < 0.5) || n0 != &n1 {
            continue;
        }
        let gx = (n0.x - t0.right()).max(t0.x - n0.right());
        let gy = (n0.y - t0.bottom()).max(t0.y - n0.bottom());
        if gx.max(gy) > GAP + 1.0 {
            continue;
        }
        let sides = beside(Axes::of(model), n1, t1);
        let far = |b: &Box2| (b.x - n1.x).abs() + (b.y - n1.y).abs();
        if let Some(b) = sides
            .into_iter()
            .filter(|b| fits(model, note, b, &[note], boxes))
            .min_by(|a, b| far(a).total_cmp(&far(b)))
        {
            boxes.insert(note.to_string(), b);
        }
    }
}

/// The target of cross links only (every arrow it has is `constraint="false"`, like a `CoreDNS`
/// used by pods) is set beside the drawing at the level just before the cards that use it (D-048,
/// R9): its arrows then come in from below (or from the side with `dir` right), with room for
/// their ports. Only if the place is free.
pub fn place_cross_targets(model: &Model, boxes: &mut Boxes) {
    const GAP: f64 = 48.0;
    let ax = Axes::of(model);
    for n in model
        .nodes
        .iter()
        .filter(|n| n.kind == Kind::Card && !in_grid(model, &n.id))
    {
        let incident: Vec<&Edge> = model
            .edges
            .iter()
            .filter(|e| e.from == n.id || e.to == n.id)
            .collect();
        // a note is placed by `place_notes`
        if incident.is_empty()
            || incident.iter().any(|e| e.constraint)
            || incident.iter().any(|e| note_link(model, e).is_some())
        {
            continue;
        }
        let Some(nb) = boxes.get(&n.id).copied() else {
            continue;
        };
        let users: Vec<Box2> = incident
            .iter()
            .filter_map(|e| {
                let other = if e.from == n.id { &e.to } else { &e.from };
                boxes.get(other).copied()
            })
            .collect();
        if users.is_empty() {
            continue;
        }
        // along the flow: just before the nearest user
        let len = ax.flow(&nb).1 - ax.flow(&nb).0;
        let f0 = if ax.reverse {
            users
                .iter()
                .map(|b| ax.flow(b).1)
                .fold(f64::NEG_INFINITY, f64::max)
                + GAP
        } else {
            users
                .iter()
                .map(|b| ax.flow(b).0)
                .fold(f64::INFINITY, f64::min)
                - GAP
                - len
        };
        // across: beyond every card from that level to the farthest user
        let end = users
            .iter()
            .map(|b| {
                if ax.reverse {
                    ax.flow(b).0
                } else {
                    ax.flow(b).1
                }
            })
            .fold(
                f0 + len,
                |a, v| if ax.reverse { a.min(v) } else { a.max(v) },
            );
        let (b0, b1) = (f0.min(end), (f0 + len).max(end));
        let band = |b: &Box2| ax.flow(b).0 < b1 && b0 < ax.flow(b).1;
        let far = boxes
            .iter()
            .filter(|(id, b)| id.as_str() != n.id && band(b) && is_card(model, id))
            .map(|(_, b)| ax.span(b).1)
            .fold(f64::NEG_INFINITY, f64::max);
        let s0 = if far.is_finite() {
            far + GAP
        } else {
            ax.span(&nb).0
        };
        let moved = ax.at(nb, f0, s0);
        if fits(model, &n.id, &moved, &[n.id.as_str()], boxes) {
            boxes.insert(n.id.clone(), whole(moved));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn b(x: f64, y: f64, w: f64, h: f64) -> Box2 {
        Box2 { x, y, w, h }
    }

    /// Bug 25: a pass moved a card far to the right and left its note behind; the note joins it
    /// on its nearest free side (here the left one, the side it came from).
    #[test]
    fn a_note_beside_its_card_follows_it() {
        let doc = skemo_lang::parse("card#a A\ncard#n.note N\n  to(a).link\ncard#c C\n");
        let m = doc.model;
        let mut before = Boxes::new();
        before.insert("a".into(), b(0.0, 0.0, 100.0, 40.0));
        before.insert("n".into(), b(124.0, 5.0, 60.0, 30.0));
        before.insert("c".into(), b(0.0, 200.0, 100.0, 40.0));
        let mut boxes = before.clone();
        boxes.insert("a".into(), b(600.0, 0.0, 100.0, 40.0));
        follow(&m, &before, &mut boxes);
        assert_eq!(boxes["n"], b(516.0, 5.0, 60.0, 30.0));
        // a note that was not beside its card stays where it is
        let mut far = before.clone();
        far.insert("n".into(), b(400.0, 5.0, 60.0, 30.0));
        let mut boxes = far.clone();
        boxes.insert("a".into(), b(600.0, 0.0, 100.0, 40.0));
        follow(&m, &far, &mut boxes);
        assert_eq!(boxes["n"], far["n"]);
    }
}
