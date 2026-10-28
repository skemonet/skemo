//! Columns: cards of a zone stacked by ELK aligned on one edge, and stacked sibling zones of one
//! extent (`zones="stretch"`).

use super::geom::{Axes, Boxes, collides, columns, containers, extent, side_by_side};
use crate::layout::Box2;
use skemo_lang::{Kind, Model};

/// Free cards of one zone that overlap in width (a "column" laid by ELK) are aligned on the side
/// where most of the arrows from outside the column come in. A tie changes nothing. Zone frames
/// stay the bounding boxes: aligning on a column's outer edge does not change them. A `wrap`
/// zone keeps its grid (`wrap_grid`).
pub fn align_columns(model: &Model, boxes: &mut Boxes) {
    for zone in model
        .nodes
        .iter()
        .filter(|n| n.kind == Kind::Zone && n.wrap.is_none())
    {
        let free: Vec<&str> = model
            .nodes
            .iter()
            .filter(|n| n.kind == Kind::Card && n.parent.as_deref() == Some(zone.id.as_str()))
            .map(|n| n.id.as_str())
            .collect();
        for col in columns(&free, boxes) {
            align(model, &col, boxes);
        }
    }
}

/// Do two boxes share some area?
fn overlap(a: &Box2, b: &Box2) -> bool {
    a.x < b.right() && b.x < a.right() && a.y < b.bottom() && b.y < a.bottom()
}

/// Aligns a column on its most linked side (left or right).
fn align(model: &Model, col: &[&str], boxes: &mut Boxes) {
    let (left, right) = extent(col.iter().map(|id| (boxes[*id].x, boxes[*id].right())));
    let mid = (left + right) / 2.0;
    let (mut l, mut r) = (0usize, 0usize);
    for e in model.edges.iter().filter(|e| e.constraint) {
        let other = match (col.contains(&e.from.as_str()), col.contains(&e.to.as_str())) {
            (true, false) => &e.to,
            (false, true) => &e.from,
            _ => continue,
        };
        if let Some(o) = boxes.get(other.as_str()) {
            if o.x + o.w / 2.0 < mid {
                l += 1;
            } else {
                r += 1;
            }
        }
    }
    if l == r || side_by_side(col, boxes) {
        return;
    }
    let moved: Vec<(&str, Box2)> = col
        .iter()
        .map(|id| {
            let mut b = boxes[*id];
            b.x = if l > r { left } else { right - b.w };
            (*id, b)
        })
        .collect();
    if moved
        .iter()
        .any(|(id, b)| collides(model, id, b, col, boxes))
    {
        return;
    }
    for (id, b) in moved {
        boxes.insert(id.to_string(), b);
    }
}

/// `diagram(zones="stretch")` (D-045): sibling zones stacked along the direction (one above the
/// other with `dir` down or up) take the extent of the widest, across (the same width; the same
/// height with right or left), like layers drawn by hand. Only the frames change, never a card.
/// A group with two zones side by side (on one row) is left alone: stretching would overlap them.
/// Pinned zones (`at=` + `size=`) keep their size and do not count. The zones of a `stack` are
/// stretched so without asking (D-056): a stack is a pile of layers. A pile that would cover a
/// card or a zone beside it (an internet gateway beside the private subnets) is left alone too.
pub fn stretch_zones(model: &Model, frames: &mut Boxes, cards: &Boxes) {
    let everywhere = model.option("zones") == Some("stretch");
    let across = Axes::of(model).across;
    // seen transposed for right/left: "columns" of zones stacked along x
    let t = |b: Box2| {
        if across {
            Box2 {
                x: b.y,
                y: b.x,
                w: b.h,
                h: b.w,
            }
        } else {
            b
        }
    };
    for parent in containers(model) {
        let stack = parent.and_then(|p| model.node(p)).is_some_and(|p| p.stack);
        if !everywhere && !stack {
            continue;
        }
        // asked for (`zones="stretch"`), every zone; by default in a stack, the drawn ones
        let zones: Vec<&str> = model
            .children(parent)
            .filter(|n| n.kind == Kind::Zone && !(n.at.is_some() && n.size.is_some()))
            .filter(|n| everywhere || !crate::layout::invisible(n))
            .map(|n| n.id.as_str())
            .collect();
        let view: Boxes = zones
            .iter()
            .filter_map(|id| frames.get(*id).map(|b| (id.to_string(), t(*b))))
            .collect();
        // what stands beside the zones in this container: its cards and its other zones
        let others: Vec<(&str, Box2)> = model
            .children(parent)
            .filter_map(|n| match n.kind {
                Kind::Card => cards.get(&n.id).map(|b| (n.id.as_str(), t(*b))),
                _ => frames.get(&n.id).map(|b| (n.id.as_str(), t(*b))),
            })
            .collect();
        for col in columns(&zones, &view) {
            if side_by_side(&col, &view) {
                continue;
            }
            let (lo, hi) = extent(col.iter().map(|id| (view[*id].x, view[*id].right())));
            let stretched: Vec<(&str, Box2)> = col
                .iter()
                .map(|id| {
                    let mut b = view[*id];
                    b.x = lo;
                    b.w = hi - lo;
                    (*id, b)
                })
                .collect();
            let covers = stretched.iter().any(|(id, b)| {
                others
                    .iter()
                    .filter(|(o, _)| !col.contains(o))
                    .any(|(_, o)| overlap(b, o) && !overlap(&view[*id], o))
            });
            if covers {
                continue;
            }
            for (id, b) in stretched {
                frames.insert(id.to_string(), t(b));
            }
        }
    }
}
