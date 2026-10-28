//! Tidying ELK's layout like a hand drawing (`layout="graph"` only).
//!
//! ELK's layered algorithm places every card, then these passes adjust the result: widths are
//! evened before ELK runs (`before_elk`), boxes are moved after it (`after_elk`). Each pass after
//! ELK reads and moves the boxes only; a card it moves loses the route hint ELK gave its arrows
//! (`drop_stale_routes`), so the router traces them anew.
//!
//! Everything here runs on ELK's placement, **before** the pinned positions (`at=`) are applied:
//! pinning a card moves no other card (D-031).

mod centre;
mod columns;
pub(crate) mod geom;
mod grid;
mod notes;
mod rows;
pub mod shelf;
mod widths;

pub use columns::stretch_zones;
pub use grid::{Hidden, hidden_by_wrap, inventory_grids};
pub use notes::notes_unconstrained;

use crate::layout::{Box2, Placement};
use crate::measure::Sizes;
use crate::scene::Scene;
use geom::Boxes;
use skemo_lang::Model;

/// Measured sizes evened out before ELK runs: titles of zones, siblings, columns of a grid.
pub fn before_elk(model: &Model, scene: &Scene, sizes: &mut Sizes) {
    // widths are evened on the top card of a pile, its cards behind added back after
    crate::measure::decks(model, sizes, -1.0);
    widths::widen_for_titles(model, scene, sizes);
    widths::even_sibling_widths(model, sizes);
    widths::even_wrap_widths(model, sizes);
    crate::measure::decks(model, sizes, 1.0);
}

/// ELK's placement tidied, in this order (each pass sees the result of the previous ones).
pub fn after_elk(
    model: &Model,
    scene: &Scene,
    sizes: &mut Sizes,
    (pad, head): (f64, f64),
    elk: &mut Placement,
) {
    // stacks first, so that the passes below see each child apart from the others (D-050)
    crate::stack::arrange(model, elk);
    // rows: sources on the first row, leaves back on their siblings' row, writing order, zones of a row on one line, grids
    run(model, elk, Hints::Drop, rows::lift_sources);
    run(model, elk, Hints::Drop, rows::lift_leaves);
    run(model, elk, Hints::Drop, rows::order_rows);
    run(model, elk, Hints::Drop, rows::align_zone_rows);
    run(model, elk, Hints::Drop, grid::wrap_grid);
    // centring: lone zones on their targets, parents on their children, children under parents
    run(model, elk, Hints::ZRoute, centre::center_lonely);
    let titles = crate::labels::zone_titles(model, scene, &elk.boxes);
    run(model, elk, Hints::Drop, |m, b| {
        centre::center_parents(m, b, &titles);
    });
    run(model, elk, Hints::Drop, centre::center_children);
    // a leaf pushed aside by a zone, onto its parent's row once the rows are centred: it moves alone
    run(model, elk, Hints::Drop, rows::leaves_beside);
    // cards off the flow: notes, isolated cards, targets of cross links
    run(model, elk, Hints::Drop, notes::place_notes);
    run(model, elk, Hints::Drop, rows::row_isolated);
    run(model, elk, Hints::Drop, notes::place_cross_targets);
    run(model, elk, Hints::Drop, columns::align_columns);
    // room left of the cards that the zone titles would cover (D-064)
    title_room(model, elk, (pad, head));
    // room between two cards side by side for the label of the arrow between them
    label_room(model, sizes, elk, (pad, head));
    // and last, to restore the stacks' order and gaps, then wide nodes and columns (D-050, D-051);
    // a zone in a stack counts by its frame as the passes left it, not by ELK's box (a card lifted
    // to its first row made it shorter, D-056)
    let frames = crate::layout::zone_frames(model, &elk.boxes, elk, pad, head);
    for z in model.nodes.iter().filter(|n| {
        n.parent
            .as_deref()
            .and_then(|p| model.node(p))
            .is_some_and(|p| p.stack)
    }) {
        if let Some(f) = frames.get(&z.id) {
            elk.boxes.insert(z.id.clone(), *f);
        }
    }
    crate::stack::arrange(model, elk);
    crate::stack::stretch(model, elk, sizes, (pad, head, false));
    crate::stack::columns(model, elk);
    // wide cards again, over the groups the columns may have moved
    crate::stack::stretch(model, elk, sizes, (pad, head, true));
    // cards with no arrow, left out of ELK's graph, on a shelf at the end of their zone
    shelf::place(model, sizes, elk, (pad, head));
}

/// D-064: a zone widens to the left so that its title leaves a card's top free
/// (`layout::zone_frames`); what lies left of it on its row moves left as far, at its level and
/// at each level above (an ancestor grows with it). Its own cards stay: a zone of a stack keeps
/// lined up with the others, which take its width (`stretch_zones`).
fn title_room(model: &Model, elk: &mut Placement, (pad, head): (f64, f64)) {
    let mut zones: Vec<String> = elk.title_ends.keys().cloned().collect();
    zones.sort();
    // every box before any zone widened: a gap already opened for one zone serves the next ones
    let plain = Placement {
        title_ends: std::collections::BTreeMap::new(),
        ..elk.clone()
    };
    let mut orig = elk.boxes.clone();
    orig.extend(crate::layout::zone_frames(
        model, &elk.boxes, &plain, pad, head,
    ));
    for z in &zones {
        let plain = Placement {
            title_ends: std::collections::BTreeMap::new(),
            ..elk.clone()
        };
        let before = crate::layout::zone_frames(model, &elk.boxes, &plain, pad, head);
        let wide = crate::layout::zone_frames(model, &elk.boxes, elk, pad, head);
        let (Some(b), Some(w)) = (before.get(z), wide.get(z)) else {
            continue;
        };
        let d = b.x - w.x;
        if d < 0.5 {
            continue;
        }
        let mut frames = elk.boxes.clone();
        frames.extend(before);
        run(model, elk, Hints::Drop, |m, boxes| {
            make_room(m, z, d, (&frames, &orig), boxes);
        });
    }
}

/// Moves left what lies left of `z` on its row (whole nodes, with their content), then what lies
/// left of each ancestor on its row, so that `z` can widen by `d`: by `d` less what the gap on
/// their right has grown since title room began (several zones with one left edge widen it
/// once). `frames`: every box as it stood before; `orig`: before any zone widened.
fn make_room(model: &Model, z: &str, d: f64, (frames, orig): (&Boxes, &Boxes), boxes: &mut Boxes) {
    let mut node = z;
    while let Some(n) = model.node(node) {
        let (Some(me), Some(me0)) = (frames.get(node).copied(), orig.get(node).copied()) else {
            return;
        };
        let left: Vec<&str> = model
            .children(n.parent.as_deref())
            .filter(|s| {
                frames.get(&s.id).is_some_and(|b| {
                    s.id != node
                        && b.right() <= me.x + 0.5
                        && b.y < me.bottom()
                        && me.y < b.bottom()
                })
            })
            .map(|s| s.id.as_str())
            .collect();
        let edge = |bs: &Boxes| {
            left.iter()
                .filter_map(|id| bs.get(*id))
                .map(Box2::right)
                .fold(f64::NEG_INFINITY, f64::max)
        };
        let grown = (me.x - edge(frames)) - (me0.x - edge(orig));
        let shift = if grown.is_finite() {
            (d - grown.max(0.0)).max(0.0)
        } else {
            d
        };
        for id in left {
            shift_x(model, id, -shift, boxes);
        }
        let Some(p) = n.parent.as_deref() else {
            return;
        };
        node = p;
    }
}

/// Room kept on each side of an arrow label between two cards side by side (px): the least for
/// the label to sit on its line, or over it (D-049), without touching either card.
const LABEL_CLEAR: f64 = 8.0;

/// Two sibling cards side by side, linked by an arrow with a label: the gap between them holds
/// the label with a little room on each side ("replica" between two databases 20 px apart ran
/// over both cards and hid the arrow). The right-hand card and what lies right of it on its row
/// move right, and what lies right of each enclosing zone on its row, as the zone grows.
fn label_room(model: &Model, sizes: &Sizes, elk: &mut Placement, (pad, head): (f64, f64)) {
    for e in &model.edges {
        let Some(size) = sizes.labels.get(&e.id) else {
            continue;
        };
        let (Some(a), Some(b)) = (
            elk.boxes.get(&e.from).copied(),
            elk.boxes.get(&e.to).copied(),
        ) else {
            continue;
        };
        let siblings =
            model.node(&e.from).map(|n| &n.parent) == model.node(&e.to).map(|n| &n.parent);
        let side_by_side = a.y < b.bottom() && b.y < a.bottom();
        let (left, right) = if a.right() <= b.x {
            (a, &e.to)
        } else if b.right() <= a.x {
            (b, &e.from)
        } else {
            continue;
        };
        if !siblings || !side_by_side {
            continue;
        }
        let gap = elk.boxes[right.as_str()].x - left.right();
        let d = (size.w + 2.0 * LABEL_CLEAR - gap).ceil();
        if d < 0.5 {
            continue;
        }
        let mut frames = elk.boxes.clone();
        frames.extend(crate::layout::zone_frames(
            model, &elk.boxes, elk, pad, head,
        ));
        run(model, elk, Hints::Drop, |m, boxes| {
            make_room_right(m, right, d, &frames, boxes);
        });
    }
}

/// Moves right by `d` card `card` and what lies right of it on its row, then what lies right of
/// each enclosing zone on its row. `frames`: every box as it stood before.
fn make_room_right(model: &Model, card: &str, d: f64, frames: &Boxes, boxes: &mut Boxes) {
    let row = |a: &Box2, b: &Box2| a.y < b.bottom() && b.y < a.bottom();
    let mut node = card;
    let mut first = true;
    while let Some(n) = model.node(node) {
        let Some(me) = frames.get(node).copied() else {
            return;
        };
        // the card itself and what starts at or after it; above it, what lies beyond the zone
        let from = if first { me.x - 0.5 } else { me.right() - 0.5 };
        let moved: Vec<&str> = model
            .children(n.parent.as_deref())
            .filter(|s| {
                frames
                    .get(&s.id)
                    .is_some_and(|b| (first || s.id != node) && b.x >= from && row(b, &me))
            })
            .map(|s| s.id.as_str())
            .collect();
        for id in moved {
            shift_x(model, id, d, boxes);
        }
        let Some(p) = n.parent.as_deref() else {
            return;
        };
        node = p;
        first = false;
    }
}

/// Moves node `id` and everything inside it along x.
fn shift_x(model: &Model, id: &str, d: f64, boxes: &mut Boxes) {
    if let Some(b) = boxes.get_mut(id) {
        b.x += d;
    }
    for k in model.children(Some(id)) {
        shift_x(model, &k.id, d, boxes);
    }
}

/// What becomes of ELK's route hint for an arrow whose card a pass moved.
#[derive(Clone, Copy, PartialEq)]
enum Hints {
    /// Dropped: the arrow is routed as if the card were pinned.
    Drop,
    /// Replaced by a Z route along the drawing's direction (`z_route`), which the router keeps
    /// if it touches no obstacle (R-15); without it, the arrows of a re-centred zone often come
    /// in by the side of its cards.
    ZRoute,
}

/// Runs one pass on the boxes, then updates the route hints of the arrows it moved.
fn run(model: &Model, elk: &mut Placement, hints: Hints, pass: impl FnOnce(&Model, &mut Boxes)) {
    let before = elk.boxes.clone();
    pass(model, &mut elk.boxes);
    drop_stale_routes(model, elk, &before, hints);
}

/// ELK's route hints no longer hold for a moved card (see `Hints`).
fn drop_stale_routes(model: &Model, placement: &mut Placement, before: &Boxes, hints: Hints) {
    let moved = |id: &str| placement.boxes.get(id) != before.get(id);
    let dir = model.option("dir").unwrap_or("down");
    let mut fresh = Vec::new();
    for e in model
        .edges
        .iter()
        .filter(|e| moved(&e.from) || moved(&e.to))
    {
        let z = match (placement.boxes.get(&e.from), placement.boxes.get(&e.to)) {
            (Some(a), Some(b)) if hints == Hints::ZRoute => z_route(a, b, dir),
            _ => None,
        };
        fresh.push((e.id.clone(), z));
    }
    for (id, z) in fresh {
        match z {
            Some(pts) => {
                placement.routes.insert(id, (pts, None));
            }
            None => {
                placement.routes.remove(&id);
            }
        }
    }
}

/// A Z route from `a` to `b` when `b` lies further along the drawing's direction: out of the
/// middle of the side facing `b`, a step half-way, into the middle of `b`'s opposite side
/// (straight when the middles line up). `None` when the boxes do not follow each other.
fn z_route(a: &Box2, b: &Box2, dir: &str) -> Option<Vec<(f64, f64)>> {
    const GAP: f64 = 8.0;
    let (acx, acy) = (a.x + a.w / 2.0, a.y + a.h / 2.0);
    let (bcx, bcy) = (b.x + b.w / 2.0, b.y + b.h / 2.0);
    let vertical = |y0: f64, y1: f64| {
        let ym = (y0 + y1) / 2.0;
        if (acx - bcx).abs() < 1.0 {
            vec![(acx, y0), (acx, y1)]
        } else {
            vec![(acx, y0), (acx, ym), (bcx, ym), (bcx, y1)]
        }
    };
    let horizontal = |x0: f64, x1: f64| {
        let xm = (x0 + x1) / 2.0;
        if (acy - bcy).abs() < 1.0 {
            vec![(x0, acy), (x1, acy)]
        } else {
            vec![(x0, acy), (xm, acy), (xm, bcy), (x1, bcy)]
        }
    };
    match dir {
        "up" if a.y >= b.bottom() + GAP => Some(vertical(a.y, b.bottom())),
        "right" if b.x >= a.right() + GAP => Some(horizontal(a.right(), b.x)),
        "left" if a.x >= b.right() + GAP => Some(horizontal(a.x, b.right())),
        "down" if b.y >= a.bottom() + GAP => Some(vertical(a.bottom(), b.y)),
        _ => None,
    }
}
