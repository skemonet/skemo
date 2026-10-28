//! Centring, like a tree drawn by hand: a lone zone on what it links to, a parent on its
//! children, a child under its parents. A node moves only across the flow, and only if the place
//! is free (`fits`).

use super::geom::{
    Axes, Boxes, constrained, extent, fits, in_grid, is_card, only_parent, rows, within,
};
use crate::layout::Box2;
use skemo_lang::{Edge, Kind, Model};

/// A top-level zone alone on its row is centred, across the direction, on the span of
/// the nodes it is linked to (its own arrows and those of what it contains). ELK's hierarchical
/// placement leaves such a zone against one side of its targets ("Services" above two workers).
/// Alone on its row, it can move across without touching anything. Two passes, so that a node
/// linked to a moved one follows it.
pub fn center_lonely(model: &Model, boxes: &mut Boxes) {
    let ax = Axes::of(model);
    let tops: Vec<&str> = model.children(None).map(|n| n.id.as_str()).collect();
    for _ in 0..2 {
        for row in rows(&tops, boxes, ax, 1) {
            let [id] = row[..] else { continue };
            if model.node(id).is_none_or(|n| n.kind != Kind::Zone) {
                continue;
            }
            let (lo, hi) = extent(model.edges.iter().filter(|e| e.constraint).filter_map(|e| {
                let other = match (within(model, &e.from, id), within(model, &e.to, id)) {
                    (true, false) => &e.to,
                    (false, true) => &e.from,
                    _ => return None,
                };
                boxes.get(other.as_str()).map(|b| ax.span(b))
            }));
            let Some(b) = boxes.get(id) else {
                continue;
            };
            if !lo.is_finite() {
                continue;
            }
            let d = (lo + hi) / 2.0 - ax.mid(b);
            if d.abs() > 0.5 {
                ax.shift_across(model, id, d, boxes);
            }
        }
    }
}

/// Parents centred on their children (D-047), like a tree drawn by hand: a card whose children
/// (cards it links to, at least two, on one row beyond it in the drawing's direction, with no
/// other parent) are spread under it moves across to the middle of their span; a card with one
/// child it is the only parent of stands straight in line with it (a spine, D-048), beside the
/// title of a zone the arrow enters; otherwise it moves onto the child in its column; each time
/// if that place is free and its zones stay clear of their neighbours. Children are handled
/// before their parents, so that a chain of single parents stays straight. Arrows outside the
/// layout (`constraint="false"`) do not count; cells of a `wrap` grid stay in the grid.
pub fn center_parents(model: &Model, boxes: &mut Boxes, titles: &[(String, Box2)]) {
    let ax = Axes::of(model);
    let edges = constrained(model);
    let mut cards: Vec<&skemo_lang::Node> = model
        .nodes
        .iter()
        .filter(|n| n.kind == Kind::Card && !in_grid(model, &n.id))
        .collect();
    // children first: the farthest along the drawing's direction
    let start = |id: &str| boxes.get(id).map_or(0.0, |b| ax.flow(b).0);
    cards.sort_by(|a, b| ax.upstream_first(start(&a.id), start(&b.id)).reverse());
    for p in cards {
        let Some(pb) = boxes.get(&p.id).copied() else {
            continue;
        };
        let Some((lo, hi)) = parent_target(model, &edges, titles, ax, (&p.id, &pb), boxes) else {
            continue;
        };
        let d = (lo + hi) / 2.0 - ax.mid(&pb);
        if d.abs() < 0.5 {
            continue;
        }
        let moved = ax.moved(&pb, d);
        if fits(model, &p.id, &moved, &[p.id.as_str()], boxes) {
            boxes.insert(p.id.clone(), moved);
        }
    }
}

/// The span (across the flow) parent `p` (box `pb`) should be centred on, or `None`.
fn parent_target(
    model: &Model,
    edges: &[&Edge],
    titles: &[(String, Box2)],
    ax: Axes,
    (p, pb): (&str, &Box2),
    boxes: &Boxes,
) -> Option<(f64, f64)> {
    // (child box, this card is its only parent)
    let linked: Vec<(Box2, bool)> = edges
        .iter()
        .filter(|e| e.from == p)
        .filter_map(|e| {
            let cb = boxes.get(&e.to)?;
            (is_card(model, &e.to) && ax.beyond(pb, cb))
                .then_some((*cb, only_parent(edges, p, &e.to)))
        })
        .collect();
    let kids: Vec<Box2> = linked.iter().filter(|k| k.1).map(|k| k.0).collect();
    // at least two own children, all on the row of the first one: their middle
    if kids.len() >= 2 && kids.iter().all(|k| ax.same_row(k, &kids[0])) {
        return Some(extent(kids.iter().map(|b| ax.span(b))));
    }
    if let [(b, true)] = linked[..] {
        // a one-to-one link: the arrow is straight, as by hand (a spine), beside the title of a
        // zone it enters
        let m = clear_of_titles(model, titles, ax, (p, &linked_id(model, edges, p)), &b)?;
        return Some((m, m));
    }
    // otherwise the child in its column (the straight arrow of a hand drawing)
    let overlap = |b: &Box2| ax.span(b).1.min(ax.span(pb).1) - ax.span(b).0.max(ax.span(pb).0);
    linked
        .iter()
        .filter(|(b, _)| overlap(b) > 0.0)
        .max_by(|a, b| overlap(&a.0).total_cmp(&overlap(&b.0)))
        .map(|(b, _)| ax.span(b))
}

/// The only constrained child of `p` (the one-to-one case of `center_parents`).
fn linked_id(model: &Model, edges: &[&Edge], p: &str) -> String {
    edges
        .iter()
        .find(|e| e.from == p && is_card(model, &e.to))
        .map(|e| e.to.clone())
        .unwrap_or_default()
}

/// Where a straight arrow from `parent` may enter `child` (box `b`), across the flow: its middle,
/// or, when the title of a zone the arrow enters lies there, just past that title, if that is
/// still well inside the card. `None` when no such place exists.
fn clear_of_titles(
    model: &Model,
    titles: &[(String, Box2)],
    ax: Axes,
    (parent, child): (&str, &str),
    b: &Box2,
) -> Option<f64> {
    let (lo, hi) = ax.span(b);
    let mut m = (lo + hi) / 2.0;
    for (z, t) in titles {
        if !within(model, child, z) || within(model, parent, z) {
            continue;
        }
        let (a, e) = ax.span(t);
        if m > a - 6.0 && m < e + 6.0 {
            m = e + 12.0;
        }
    }
    (m <= hi - 12.0).then_some(m)
}

/// Cards centred under their parents, top-down, like a hand drawing (D-048, R2): a card fed by
/// two or more cards of one row before it sits in the middle of their span (Core between two
/// uplinks, an API under two web servers); the children a card is the only parent of, on one row,
/// are shifted together under it when it has moved (a spine and its fork stay centred). Each move
/// happens only if the place is free and inside the card's zone. Arrows outside the layout
/// (`constraint="false"`) do not count.
pub fn center_children(model: &Model, boxes: &mut Boxes) {
    let ax = Axes::of(model);
    let edges = constrained(model);
    let mut cards: Vec<&str> = model
        .nodes
        .iter()
        .filter(|n| n.kind == Kind::Card && boxes.contains_key(&n.id))
        .map(|n| n.id.as_str())
        .collect();
    // parents first: the nearest to the start of the drawing
    cards.sort_by(|a, b| ax.upstream_first(ax.flow(&boxes[*a]).0, ax.flow(&boxes[*b]).0));
    // a card may be held by a neighbour that moves later on its row: up to three rounds, until
    // nothing moves
    for _ in 0..3 {
        let round = boxes.clone();
        for id in &cards {
            center_under_parents(model, &edges, ax, id, boxes);
        }
        for id in &cards {
            center_own_children(model, &edges, ax, id, boxes);
        }
        if *boxes == round {
            break;
        }
    }
}

/// Boxes of the cards linked to `id` from one row before it (or after it, `out`); `None` unless
/// they all lie on one row.
fn row_neighbours(
    model: &Model,
    edges: &[&Edge],
    ax: Axes,
    (id, out): (&str, bool),
    boxes: &Boxes,
) -> Option<Vec<(String, Box2)>> {
    let b = boxes.get(id)?;
    let mut seen = Vec::new();
    let mut found: Vec<(String, Box2)> = Vec::new();
    for e in edges {
        let (me, other) = if out {
            (&e.from, &e.to)
        } else {
            (&e.to, &e.from)
        };
        if me != id || seen.contains(other) {
            continue;
        }
        seen.push(other.clone());
        let ob = boxes.get(other.as_str())?;
        let ordered = if out {
            ax.beyond(b, ob)
        } else {
            ax.beyond(ob, b)
        };
        if !ordered || !is_card(model, other) {
            return None;
        }
        found.push((other.clone(), *ob));
    }
    let first = found.first()?.1;
    found
        .iter()
        .all(|(_, k)| ax.same_row(k, &first))
        .then_some(found)
}

/// A card fed by two or more parents of one row, each with no other child, moves to the middle
/// of their span.
fn center_under_parents(model: &Model, edges: &[&Edge], ax: Axes, id: &str, boxes: &mut Boxes) {
    if in_grid(model, id) {
        return;
    }
    let Some(parents) = row_neighbours(model, edges, ax, (id, false), boxes) else {
        return;
    };
    // only parents whose one child is this card (not a bus feeding several cards)
    let sole = parents
        .iter()
        .all(|(p, _)| edges.iter().all(|e| e.from != *p || e.to == id));
    if parents.len() < 2 || !sole {
        return;
    }
    let (lo, hi) = extent(parents.iter().map(|(_, b)| ax.span(b)));
    let b = boxes[id];
    let d = (lo + hi) / 2.0 - ax.mid(&b);
    let m = ax.moved(&b, d);
    if d.abs() >= 0.5 && fits(model, id, &m, &[id], boxes) {
        boxes.insert(id.to_string(), m);
    }
}

/// The children card `id` is the only parent of, on one row, move together under it.
fn center_own_children(model: &Model, edges: &[&Edge], ax: Axes, id: &str, boxes: &mut Boxes) {
    let Some(kids) = row_neighbours(model, edges, ax, (id, true), boxes) else {
        return;
    };
    // only children this card is the sole parent of, not cells of a grid
    if kids.is_empty()
        || kids.iter().any(|(k, _)| !only_parent(edges, id, k))
        || kids.iter().any(|(k, _)| in_grid(model, k))
    {
        return;
    }
    let (lo, hi) = extent(kids.iter().map(|(_, b)| ax.span(b)));
    let d = ax.mid(&boxes[id]) - (lo + hi) / 2.0;
    if d.abs() < 0.5 {
        return;
    }
    let ids: Vec<&str> = kids.iter().map(|(k, _)| k.as_str()).collect();
    let free = kids
        .iter()
        .all(|(k, b)| fits(model, k, &ax.moved(b, d), &ids, boxes));
    if free {
        for (k, b) in &kids {
            boxes.insert(k.clone(), ax.moved(b, d));
        }
    }
}
