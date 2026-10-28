//! Rows: leaves back on their siblings' row, writing order inside a row, sibling zones on one
//! line, isolated cards lined up.

use super::geom::{Axes, Boxes, constrained, fits, is_card, only_parent, rows, whole};
use crate::layout::Box2;
use skemo_lang::{Kind, Model};

/// Leaves of a fork back on their siblings' row (D-048, R1): ELK puts every card with no arrow out
/// on the last row, so in an organisation chart a director without a team drops below the one
/// with a team. A child with no arrow out and no other parent, further than the nearest of its
/// siblings, moves along the flow to that sibling's row if the place is free (and its zone stays
/// clear of its neighbours). Before `order_rows`, which then hands the row out in writing order.
pub fn lift_leaves(model: &Model, boxes: &mut Boxes) {
    let ax = Axes::of(model);
    let edges = constrained(model);
    // how far a box lies along the drawing's direction
    let depth = |b: &Box2| {
        if ax.reverse {
            -ax.flow(b).1
        } else {
            ax.flow(b).0
        }
    };
    for p in model.nodes.iter().filter(|n| n.kind == Kind::Card) {
        let kids: Vec<&str> = edges
            .iter()
            .filter(|e| e.from == p.id && is_card(model, &e.to))
            .map(|e| e.to.as_str())
            .collect();
        if kids.len() < 2 {
            continue;
        }
        let Some(nearest) = kids
            .iter()
            .filter_map(|k| boxes.get(*k))
            .min_by(|a, b| depth(a).total_cmp(&depth(b)))
            .copied()
        else {
            continue;
        };
        let mut lifted = false;
        for k in &kids {
            let leaf = !edges.iter().any(|e| e.from == *k);
            let Some(b) = boxes.get(*k).copied() else {
                continue;
            };
            if !leaf || !only_parent(&edges, &p.id, k) || depth(&b) <= depth(&nearest) + 0.5 {
                continue;
            }
            // the nearest sibling's start along the flow (its end with `up` or `left`)
            let start = if ax.reverse {
                ax.flow(&nearest).1 - ax.flow_len(&b)
            } else {
                ax.flow(&nearest).0
            };
            let m = ax.at(b, start, ax.span(&b).0);
            if fits(model, k, &m, &[k], boxes) {
                boxes.insert((*k).to_string(), m);
                lifted = true;
            }
        }
        if lifted {
            spread_evenly(model, ax, &kids, boxes);
        }
    }
}

/// A leaf beside its parent: a card with no arrow out and no other parent, that the layout put on
/// a later row, to the side of a zone that sits under its parent or beyond it (a CDN hanging from
/// a front card above the whole region, drawn at the far edge or under the region), moves onto
/// its parent's row, next to it: before the parent if written before it, else after, or the other
/// side if that one is taken. After the centring passes: the leaf alone moves, the row it leaves
/// is not centred again.
pub fn leaves_beside(model: &Model, boxes: &mut Boxes) {
    const GAP: f64 = 40.0;
    let ax = Axes::of(model);
    let edges = constrained(model);
    let rank = |id: &str| model.nodes.iter().position(|n| n.id == id);
    for e in &edges {
        let (p, k) = (e.from.as_str(), e.to.as_str());
        let (Some(pb), Some(kb)) = (boxes.get(p).copied(), boxes.get(k).copied()) else {
            continue;
        };
        if !pushed_aside(model, &edges, (p, &pb), (k, &kb), boxes) {
            continue;
        }
        let (p0, p1) = ax.span(&pb);
        let before = ax.at(kb, ax.flow(&pb).0, p0 - GAP - ax.span_len(&kb));
        let after = ax.at(kb, ax.flow(&pb).0, p1 + GAP);
        let sides = if rank(k) < rank(p) {
            [before, after]
        } else {
            [after, before]
        };
        if let Some(m) = sides.into_iter().find(|m| fits(model, k, m, &[k], boxes)) {
            boxes.insert(k.to_string(), whole(m));
        }
    }
}

/// Is card `k`, the only child of `p` by this arrow and a leaf, on a later row than `p`, with a
/// zone under `p` on its row or between them (the arrow would go round or along it)?
fn pushed_aside(
    model: &Model,
    edges: &[&skemo_lang::Edge],
    (p, pb): (&str, &Box2),
    (k, kb): (&str, &Box2),
    boxes: &Boxes,
) -> bool {
    let ax = Axes::of(model);
    let (Some(pn), Some(kn)) = (model.node(p), model.node(k)) else {
        return false;
    };
    let plain = |n: &skemo_lang::Node| {
        n.kind == Kind::Card && n.at.is_none() && !super::geom::in_grid(model, &n.id)
    };
    let stacked = kn
        .parent
        .as_deref()
        .and_then(|z| model.node(z))
        .is_some_and(|z| z.stack);
    if !plain(pn) || !plain(kn) || pn.parent != kn.parent || stacked {
        return false;
    }
    if edges.iter().any(|e| e.from == k) || !only_parent(edges, p, k) || !ax.beyond(pb, kb) {
        return false;
    }
    let (p0, p1) = ax.span(pb);
    model
        .children(kn.parent.as_deref())
        .filter(|z| z.kind == Kind::Zone)
        .filter_map(|z| boxes.get(&z.id))
        .any(|zb| {
            ax.beyond(pb, zb) && !ax.beyond(kb, zb) && ax.span(zb).0 < p1 && p0 < ax.span(zb).1
        })
}

/// Sources on their zone's first row (D-056): a card of a zone with arrows out, none in, and
/// every arrow leaving the zone, that the layout set further down (ELK counts it after the zone's
/// own chains: kubectl below the manifest's chips), moves along the flow to the first row of its
/// siblings if the place is free. The top level is left to the layout. Before `order_rows`, which
/// hands the row out in writing order.
pub fn lift_sources(model: &Model, boxes: &mut Boxes) {
    let ax = Axes::of(model);
    let edges = constrained(model);
    for parent in super::geom::containers(model).skip(1) {
        let kids: Vec<&str> = model.children(parent).map(|n| n.id.as_str()).collect();
        let zone = parent.unwrap_or_default();
        let Some(first) = kids
            .iter()
            .filter_map(|k| boxes.get(*k))
            .min_by(|a, b| ax.upstream_first(ax.flow(a).0, ax.flow(b).0))
            .copied()
        else {
            continue;
        };
        for k in &kids {
            let mut out = edges.iter().filter(|e| e.from == *k).peekable();
            let source = is_card(model, k)
                && out.peek().is_some()
                && out.all(|e| !super::geom::within(model, &e.to, zone))
                && !edges.iter().any(|e| e.to == *k);
            let Some(b) = boxes.get(*k).copied() else {
                continue;
            };
            if !source || ax.same_row(&b, &first) {
                continue;
            }
            let start = if ax.reverse {
                ax.flow(&first).1 - ax.flow_len(&b)
            } else {
                ax.flow(&first).0
            };
            let m = ax.at(b, start, ax.span(&b).0);
            if fits(model, k, &m, &[k], boxes) {
                boxes.insert((*k).to_string(), m);
            }
        }
    }
}

/// The middle cards of a row of siblings, centred at even steps between the first and the last
/// (a fork whose leaves were lifted keeps a regular rhythm), each if the place is free.
fn spread_evenly(model: &Model, ax: Axes, kids: &[&str], boxes: &mut Boxes) {
    let mut row: Vec<(&str, Box2)> = kids
        .iter()
        .filter_map(|k| boxes.get(*k).map(|b| (*k, *b)))
        .collect();
    if row.len() < 3 || !row.iter().all(|(_, b)| ax.same_row(b, &row[0].1)) {
        return;
    }
    row.sort_by(|a, b| ax.mid(&a.1).total_cmp(&ax.mid(&b.1)));
    let (c0, c1) = (ax.mid(&row[0].1), ax.mid(&row[row.len() - 1].1));
    let step = (c1 - c0) / (row.len() - 1) as f64;
    for (i, (id, b)) in row.iter().enumerate().take(row.len() - 1).skip(1) {
        let m = ax.moved(b, c0 + step * i as f64 - ax.mid(b));
        if fits(model, id, &m, &[id], boxes) {
            boxes.insert((*id).to_string(), m);
        }
    }
}

/// Writing order inside a row (limit L3 of the prototype, F-LAYOUT-2): ELK keeps the model
/// order at the top level but not always inside a zone (adding a note linked to a card could
/// swap the cards of a zone). Siblings that share a layer (same row with `dir` down or up, same
/// column with right or left) are put back in writing order: the slots ELK chose are kept, with
/// their gaps, and handed out in writing order. A moved zone carries its descendants.
pub fn order_rows(model: &Model, boxes: &mut Boxes) {
    let ax = Axes::of(model);
    for parent in super::geom::containers(model) {
        let kids: Vec<&str> = model.children(parent).map(|n| n.id.as_str()).collect();
        for row in rows(&kids, boxes, ax, 2) {
            reorder(model, ax, &row, boxes);
        }
    }
}

/// Hands the slots of a row out in writing order, keeping the first slot and the gaps.
fn reorder(model: &Model, ax: Axes, row: &[&str], boxes: &mut Boxes) {
    let pos = |b: &Box2| (ax.span(b).0, ax.span_len(b));
    let mut slots: Vec<(&str, f64, f64)> = row
        .iter()
        .map(|id| {
            let (p, s) = pos(&boxes[*id]);
            (*id, p, s)
        })
        .collect();
    slots.sort_by(|a, b| a.1.total_cmp(&b.1));
    let rank = |id: &str| model.nodes.iter().position(|n| n.id == id);
    let mut written: Vec<&str> = row.to_vec();
    written.sort_by_key(|id| rank(id));
    if slots.iter().map(|s| s.0).eq(written.iter().copied()) {
        return;
    }
    let gaps: Vec<f64> = slots
        .windows(2)
        .map(|w| (w[1].1 - (w[0].1 + w[0].2)).max(0.0))
        .collect();
    let mut at = slots[0].1;
    for (i, id) in written.iter().enumerate() {
        let (p, s) = pos(&boxes[*id]);
        ax.shift_across(model, id, at - p, boxes);
        at += s + gaps.get(i).copied().unwrap_or(0.0);
    }
}

/// Sibling zones of one row start on the same line (same top with `dir` down, same left edge with
/// right): ELK leaves them a few px apart when an arrow enters one of them, which shows on frames
/// side by side. A zone moves only towards the row's start, by less than `MAX`, with its content.
pub fn align_zone_rows(model: &Model, boxes: &mut Boxes) {
    const MAX: f64 = 12.0;
    let ax = Axes::of(model);
    let start = |b: &Box2| ax.flow(b).0;
    for parent in super::geom::containers(model) {
        let zones: Vec<&str> = model
            .children(parent)
            .filter(|n| n.kind == Kind::Zone)
            .map(|n| n.id.as_str())
            .collect();
        for row in rows(&zones, boxes, ax, 2) {
            let top = row
                .iter()
                .map(|id| start(&boxes[*id]))
                .fold(f64::INFINITY, f64::min);
            for id in row {
                let d = top - start(&boxes[id]);
                if d.abs() > 1e-6 && d.abs() < MAX {
                    ax.shift_along(model, id, d, boxes);
                }
            }
        }
    }
}

/// Cards with no arrow at all, at the top level, follow the reading order (D-048, R5): with `dir`
/// right or left ELK stacks them in one column, which reads as a sequence that does not exist.
/// They are lined up left to right, in writing order, centred on one line, under the rest of the
/// drawing (or alone). With `dir` down or up ELK already puts them on one row.
pub fn row_isolated(model: &Model, boxes: &mut Boxes) {
    const GAP: f64 = 28.0;
    if !Axes::of(model).across {
        return;
    }
    let linked = |id: &str| model.edges.iter().any(|e| e.from == id || e.to == id);
    let lone: Vec<(&str, Box2)> = model
        .children(None)
        .filter(|n| n.kind == Kind::Card && n.at.is_none() && !linked(&n.id))
        .filter_map(|n| boxes.get(&n.id).map(|b| (n.id.as_str(), *b)))
        .collect();
    if lone.len() < 2 {
        return;
    }
    let rest: Vec<Box2> = boxes
        .iter()
        .filter(|(id, _)| !lone.iter().any(|(l, _)| l == id))
        .map(|(_, b)| *b)
        .collect();
    let left = rest
        .iter()
        .chain(lone.iter().map(|(_, b)| b))
        .map(|b| b.x)
        .fold(f64::INFINITY, f64::min);
    let top = if rest.is_empty() {
        lone.iter().map(|(_, b)| b.y).fold(f64::INFINITY, f64::min)
    } else {
        rest.iter()
            .map(Box2::bottom)
            .fold(f64::NEG_INFINITY, f64::max)
            + 48.0
    };
    let tallest = lone.iter().map(|(_, b)| b.h).fold(0.0, f64::max);
    let mut x = left;
    for (id, b) in lone {
        let moved = Box2 {
            x,
            y: top + (tallest - b.h) / 2.0,
            ..b
        };
        x += b.w + GAP;
        boxes.insert(id.to_string(), whole(moved));
    }
}
