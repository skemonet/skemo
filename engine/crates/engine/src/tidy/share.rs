//! Cards over a stretched card (D-092). In a stack, a card stretched across its row (a `.wide`
//! card, or the cards of a `.wide` zone sharing its width) carries what stands on it, like a layer
//! drawn by hand: the cards of the layer before it that it is linked to split its width evenly,
//! each centred in its share. Their arrows then run straight into it, spread evenly over it.

use super::geom::{Axes, Boxes, collides, in_grid, within};
use crate::layout::Box2;
use skemo_lang::{Kind, Model};

/// Gap allowed between two cards of a row that share their zone's width (`stack::COL_GAP`).
const TILE_GAP: f64 = 48.0;

/// Each stack, layer by layer: the cards of a layer over each stretched card of the next.
pub fn share_stretched(model: &Model, boxes: &mut Boxes) {
    let ax = Axes::of(model);
    for s in model.nodes.iter().filter(|n| n.stack) {
        let layers: Vec<&str> = model.children(Some(&s.id)).map(|c| c.id.as_str()).collect();
        for w in layers.windows(2) {
            // a layer whose own cards are stretched stands on its own: they keep their shares
            if !stretched(model, ax, w[0], boxes).is_empty() {
                continue;
            }
            for bar in stretched(model, ax, w[1], boxes) {
                share(model, ax, (w[0], &bar), boxes);
            }
        }
    }
}

/// The stretched cards of a layer: itself if it is a `.wide` card, or the cards of a `.wide` zone
/// set on one row from one end of it to the other (`stack::fill`).
fn stretched(model: &Model, ax: Axes, layer: &str, boxes: &Boxes) -> Vec<String> {
    let Some(n) = model.node(layer) else {
        return Vec::new();
    };
    if !n.classes.iter().any(|c| c == "wide") {
        return Vec::new();
    }
    if n.kind == Kind::Card {
        return vec![n.id.clone()];
    }
    let Some(zone) = boxes.get(layer) else {
        return Vec::new();
    };
    let mut cards: Vec<(&str, Box2)> = model
        .children(Some(layer))
        .filter(|c| c.kind == Kind::Card)
        .filter_map(|c| boxes.get(&c.id).map(|b| (c.id.as_str(), *b)))
        .collect();
    cards.sort_by(|a, b| ax.mid(&a.1).total_cmp(&ax.mid(&b.1)));
    let (Some(first), Some(last)) = (cards.first(), cards.last()) else {
        return Vec::new();
    };
    let (lo, hi) = ax.span(zone);
    let tiled = cards.iter().all(|(_, b)| ax.same_row(b, &first.1))
        && ax.span(&first.1).0 - lo <= 2.0
        && hi - ax.span(&last.1).1 <= 2.0
        && cards
            .windows(2)
            .all(|p| ax.span(&p[1].1).0 - ax.span(&p[0].1).1 <= TILE_GAP);
    if tiled {
        cards.iter().map(|(id, _)| (*id).to_string()).collect()
    } else {
        Vec::new()
    }
}

/// The cards of `layer` linked to `bar` and standing over it split its width evenly, in their
/// order, each centred in its share. Notes on them follow (`place_notes`). Only if every one of
/// them finds room within the bar's width; a grid cell keeps the whole as it is.
/// The layer's frame does not change (its cards stay over the bar), so only collisions count.
fn share(model: &Model, ax: Axes, (layer, bar): (&str, &str), boxes: &mut Boxes) {
    let Some(bb) = boxes.get(bar).copied() else {
        return;
    };
    let (lo, hi) = ax.span(&bb);
    let linked = |id: &str| {
        model
            .edges
            .iter()
            .any(|e| (e.from == id && e.to == bar) || (e.to == id && e.from == bar))
    };
    let mut on: Vec<(&str, Box2)> = model
        .nodes
        .iter()
        .filter(|n| n.kind == Kind::Card && n.id != layer && within(model, &n.id, layer))
        .filter(|n| linked(&n.id))
        .filter_map(|n| boxes.get(&n.id).map(|b| (n.id.as_str(), *b)))
        .filter(|(_, b)| ax.mid(b) > lo && ax.mid(b) < hi)
        .collect();
    if on.is_empty() || on.iter().any(|(id, _)| in_grid(model, id)) {
        return;
    }
    on.sort_by(|a, b| ax.mid(&a.1).total_cmp(&ax.mid(&b.1)));
    let part = (hi - lo) / on.len() as f64;
    let moved: Vec<(&str, Box2)> = on
        .iter()
        .enumerate()
        .map(|(i, (id, b))| (*id, ax.moved(b, lo + part * (i as f64 + 0.5) - ax.mid(b))))
        .collect();
    // the notes on these cards are set again beside them, after the move
    let notes = notes_on(model, &on);
    let mut trial = boxes.clone();
    for (id, b) in &moved {
        trial.insert((*id).to_string(), *b);
    }
    for n in &notes {
        trial.remove(*n);
    }
    let ids: Vec<&str> = moved.iter().map(|(id, _)| *id).collect();
    let clear = |id: &str, b: &Box2, group: &[&str], boxes: &Boxes| {
        !collides(model, id, b, group, boxes) && ax.span(b).0 >= lo && ax.span(b).1 <= hi
    };
    if !moved.iter().all(|(id, b)| clear(id, b, &ids, &trial)) {
        return;
    }
    let mut placed = trial.clone();
    for n in &notes {
        placed.insert((*n).to_string(), boxes[*n]);
    }
    super::notes::place_notes(model, &mut placed);
    let mut rest = trial;
    for n in &notes {
        let b = placed[*n];
        if collides(model, n, &b, &[n], &rest) {
            return;
        }
        rest.insert((*n).to_string(), b);
    }
    *boxes = rest;
}

/// The notes linked to these cards (`notes::note_link`).
fn notes_on<'a>(model: &'a Model, cards: &[(&str, Box2)]) -> Vec<&'a str> {
    model
        .edges
        .iter()
        .filter_map(|e| super::notes::note_link(model, e))
        .filter(|(_, t)| cards.iter().any(|(id, _)| id == t))
        .map(|(n, _)| n)
        .collect()
}
