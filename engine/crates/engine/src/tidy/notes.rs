//! Cards off the flow: notes beside the card they annotate, targets of cross links beside the
//! drawing.

use super::geom::{Axes, Boxes, fits, in_grid, is_card, whole};
use crate::layout::Box2;
use skemo_lang::{Edge, Kind, Model};

/// The note a `.link` arrow attaches, and the card it annotates: `(note, target)`. A note is a
/// `.note` card whose only arrows are this one link.
fn note_link<'a>(model: &'a Model, e: &'a Edge) -> Option<(&'a str, &'a str)> {
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

/// Notes set beside the card they annotate, off the flow (D-048, R7): above it with `dir` right
/// or left, on its right with down or up (then the other side), centred on it, 24 px away, if
/// the place is free; else beyond everything at the target's level, on the side of the second
/// spot's opposite (the right with `dir` down, below with right). Otherwise the note stays where
/// the layout put it.
pub fn place_notes(model: &Model, boxes: &mut Boxes) {
    const GAP: f64 = 24.0;
    let ax = Axes::of(model);
    for e in &model.edges {
        let Some((note, target)) = note_link(model, e) else {
            continue;
        };
        if model.node(note).is_some_and(|n| n.at.is_some()) || in_grid(model, note) {
            continue;
        }
        let (Some(nb), Some(tb)) = (boxes.get(note).copied(), boxes.get(target).copied()) else {
            continue;
        };
        // centred on the target along the flow, beside it across
        let centred = ax.flow(&tb).0 + (ax.flow_len(&tb) - ax.flow_len(&nb)) / 2.0;
        let after = ax.at(nb, centred, ax.span(&tb).1 + GAP);
        let before = ax.at(nb, centred, ax.span(&tb).0 - GAP - ax.span_len(&nb));
        let spots = if ax.across {
            [before, after]
        } else {
            [after, before]
        };
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

/// The target of cross links only (every arrow it has is `constraint="false"`, like a `CoreDNS`
/// used by pods) is set beside the drawing at the level just before the cards that use it (D-048,
/// R9): its arrows then come in from below (or from the side with `dir` right), with room for
/// their ports. Only if the place is free; a pinned card stays.
pub fn place_cross_targets(model: &Model, boxes: &mut Boxes) {
    const GAP: f64 = 48.0;
    let ax = Axes::of(model);
    for n in model
        .nodes
        .iter()
        .filter(|n| n.kind == Kind::Card && n.at.is_none() && !in_grid(model, &n.id))
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
