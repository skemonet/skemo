//! `wrap="N"` zones: their children on a grid, and the cards the grid hides from their arrows.

use super::geom::{Axes, Boxes, within};
use crate::layout::Box2;
use skemo_lang::{Kind, Model};

/// `wrap="N"` zones: children aligned on a grid of N columns (rows with `dir` right or left),
/// in writing order. The layers come from ELK (see `layout::wrap_edges`); only the position
/// across the layers is set here: each column as wide as its widest child, 36 px apart (ELK's
/// node spacing), from the leftmost child.
pub fn wrap_grid(model: &Model, boxes: &mut Boxes) {
    let ax = Axes::of(model);
    let pos = |b: &Box2| (ax.span(b).0, ax.span_len(b));
    for z in model.nodes.iter().filter(|n| n.kind == Kind::Zone) {
        let Some(n) = z.wrap.map(|w| w as usize) else {
            continue;
        };
        let kids: Vec<&str> = model
            .children(Some(&z.id))
            .map(|c| c.id.as_str())
            .filter(|id| boxes.contains_key(*id))
            .collect();
        if kids.len() <= 1 {
            continue;
        }
        let start = kids
            .iter()
            .map(|id| pos(&boxes[*id]).0)
            .fold(f64::INFINITY, f64::min);
        let cols = n.min(kids.len());
        let mut width = vec![0.0_f64; cols];
        for (i, id) in kids.iter().enumerate() {
            width[i % cols] = width[i % cols].max(pos(&boxes[*id]).1);
        }
        let end = kids
            .iter()
            .map(|id| ax.span(&boxes[*id]).1)
            .fold(f64::NEG_INFINITY, f64::max);
        let grid = width.iter().sum::<f64>() + 36.0 * (cols - 1) as f64;
        // ELK kept less room than the grid takes (cells it laid in one column): what follows the
        // zone on its row moves on, at every level up
        if grid > end - start + 0.5 {
            push_beyond(model, ax, &z.id, end, grid - (end - start), boxes);
        }
        let mut col_at = Vec::with_capacity(cols);
        let mut at =
            start + toward_siblings(model, &z.id, boxes, ax) * (end - start - grid).max(0.0);
        for w in &width {
            col_at.push(at);
            at += w + 36.0;
        }
        for (i, id) in kids.iter().enumerate() {
            let d = col_at[i % cols] - pos(&boxes[*id]).0;
            ax.shift_across(model, id, d, boxes);
        }
        // each cell below the one before it in its column, and on the row of the first of its row
        let in_order = (cols..kids.len())
            .all(|i| ax.beyond(&boxes[kids[i - cols]], &boxes[kids[i]]))
            && (0..kids.len()).all(|i| ax.same_row(&boxes[kids[i - i % cols]], &boxes[kids[i]]));
        if !in_order {
            grid_rows(model, ax, &kids, cols, boxes);
        }
    }
}

/// Node `z` grows across the flow by `d` past `end` (its content's end): its siblings beyond
/// that end on its row, and any sibling one of them would then cover, move on by `d`; then the
/// same one level up, for the parent's siblings.
fn push_beyond(model: &Model, ax: Axes, z: &str, end: f64, d: f64, boxes: &mut Boxes) {
    let mut node = z.to_owned();
    let mut end = end;
    while let Some(n) = model.node(&node) {
        let parent = n.parent.clone();
        let me = boxes.get(&node).copied();
        let siblings: Vec<(String, Box2)> = model
            .children(parent.as_deref())
            .filter(|s| s.id != node)
            .filter_map(|s| boxes.get(&s.id).map(|b| (s.id.clone(), *b)))
            .collect();
        let beyond = |b: &Box2| ax.span(b).0 >= end - 1.0;
        let mut moved: Vec<(String, Box2)> = siblings
            .iter()
            .filter(|(_, b)| beyond(b) && me.is_none_or(|m| ax.same_row(b, &m)))
            .cloned()
            .collect();
        let shifted = |b: &Box2| ax.at(*b, ax.flow(b).0, ax.span(b).0 + d);
        loop {
            let more: Vec<(String, Box2)> = siblings
                .iter()
                .filter(|(id, b)| beyond(b) && !moved.iter().any(|(m, _)| m == id))
                .filter(|(_, b)| moved.iter().any(|(_, m)| overlaps(&shifted(m), b)))
                .cloned()
                .collect();
            if more.is_empty() {
                break;
            }
            moved.extend(more);
        }
        for (id, _) in &moved {
            ax.shift_across(model, id, d, boxes);
        }
        let Some(p) = parent else { return };
        // the parent's own end, as ELK left it
        end = boxes.get(&p).map_or(end, |b| ax.span(b).1);
        node = p;
    }
}

fn overlaps(a: &Box2, b: &Box2) -> bool {
    a.x < b.right() && b.x < a.right() && a.y < b.bottom() && b.y < a.bottom()
}

/// Fewest cards for an inventory to be laid as a grid.
const INVENTORY: usize = 4;

/// A zone holding only cards, at least four, none of them with an arrow (an inventory: the
/// monitors, the secrets, the registries of a platform) and no `wrap` of its own: laid as a grid
/// of about as many columns as rows (4 cards: 2 × 2; 5 to 9: 3 columns), rather than one row as
/// wide as all of them. In the layout's view of the model only; the text keeps no `wrap`.
pub fn inventory_grids(model: &mut Model) {
    let linked = |id: &str| model.edges.iter().any(|e| e.from == id || e.to == id);
    let grids: Vec<(String, u32)> = model
        .nodes
        .iter()
        .filter(|z| z.kind == Kind::Zone && z.wrap.is_none() && !z.stack)
        .filter_map(|z| {
            let kids: Vec<&skemo_lang::Node> = model.children(Some(&z.id)).collect();
            let inventory = kids.len() >= INVENTORY
                && kids
                    .iter()
                    .all(|k| k.kind == Kind::Card && k.at.is_none() && !linked(&k.id));
            // the side of the square the cards make
            let side = (kids.len() as f64).sqrt().ceil() as u32;
            inventory.then(|| (z.id.clone(), side))
        })
        .collect();
    for (id, side) in grids {
        if let Some(z) = model.nodes.iter_mut().find(|n| n.id == id) {
            z.wrap = Some(side);
        }
    }
}

/// Where a grid narrower than ELK's room for it sits in that room: 0 at its start, 1 at its end,
/// 0.5 in the middle. Towards the zone's siblings on its row, so that the room freed goes to the
/// outside of the parent, whose frame then shrinks, not between the zone and its neighbours.
fn toward_siblings(model: &Model, z: &str, boxes: &Boxes, ax: Axes) -> f64 {
    let Some(me) = boxes.get(z) else {
        return 0.0;
    };
    let parent = model.node(z).and_then(|n| n.parent.as_deref());
    let (mut before, mut after) = (false, false);
    for s in model.children(parent).filter(|s| s.id != z) {
        let Some(b) = boxes.get(&s.id) else { continue };
        if !ax.same_row(me, b) {
            continue;
        }
        before |= ax.span(b).1 <= ax.span(me).0 + 0.5;
        after |= ax.span(b).0 >= ax.span(me).1 - 0.5;
    }
    match (before, after) {
        (false, true) => 1.0,
        (true, true) => 0.5,
        _ => 0.0,
    }
}

/// The rows of a grid by writing order too, when ELK's layers do not follow it (an arrow between
/// two cells against the order makes a cycle with the invisible edges; ELK breaks it: two cells
/// of one column landed on one row, one over the other, or the columns started on different rows). Each row as deep as its deepest
/// cell, as far apart as ELK's rows were (its smallest gap, else 20 px), cells at its start.
fn grid_rows(model: &Model, ax: Axes, kids: &[&str], cols: usize, boxes: &mut Boxes) {
    let mut spans: Vec<(f64, f64)> = kids.iter().map(|id| ax.flow(&boxes[*id])).collect();
    spans.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut gap = f64::INFINITY;
    let mut end = spans[0].1;
    for s in &spans[1..] {
        if s.0 > end + 0.5 {
            gap = gap.min(s.0 - end);
        }
        end = end.max(s.1);
    }
    let gap = if gap.is_finite() { gap } else { 20.0 };
    // along the flow, from the drawing's start: the first row nearest to it
    let first = if ax.reverse {
        spans.iter().map(|s| s.1).fold(f64::NEG_INFINITY, f64::max)
    } else {
        spans[0].0
    };
    let mut at = first;
    for row in kids.chunks(cols) {
        let deep = row
            .iter()
            .map(|id| ax.flow_len(&boxes[*id]))
            .fold(0.0, f64::max);
        let start = if ax.reverse { at - deep } else { at };
        for id in row {
            let d = start - ax.flow(&boxes[*id]).0;
            ax.shift_along(model, id, d, boxes);
        }
        at = if ax.reverse {
            start - gap
        } else {
            at + deep + gap
        };
    }
}

/// Cards of a `wrap` zone that sit behind other cards of the zone, as seen from a node outside the
/// zone they are linked to: their arrow cannot reach them straight and goes around (a second
/// column with `dir="right"`, a second row with `dir="down"`). Not for `wrap="1"`.
#[derive(Debug, Clone, PartialEq)]
pub struct Hidden {
    pub zone: String,
    pub wrap: u32,
    pub cards: Vec<String>,
    pub from: Vec<String>,
}

/// The `wrap` zones that hide some of their children from their arrows (final boxes).
pub fn hidden_by_wrap(model: &Model, boxes: &Boxes) -> Vec<Hidden> {
    let mut out = Vec::new();
    for z in model.nodes.iter().filter(|n| n.kind == Kind::Zone) {
        // `wrap="1"`: a single file of cards (a server and its stack), each with two free sides
        // that its arrows enter by
        let (Some(wrap @ 2..), Some(zb)) = (z.wrap, boxes.get(&z.id)) else {
            continue;
        };
        let kids: Vec<&str> = model.children(Some(&z.id)).map(|n| n.id.as_str()).collect();
        let (mut cards, mut from) = (Vec::new(), Vec::new());
        for e in &model.edges {
            let (inner, outer) = match (within(model, &e.from, &z.id), within(model, &e.to, &z.id))
            {
                (true, false) => (&e.from, &e.to),
                (false, true) => (&e.to, &e.from),
                _ => continue,
            };
            let Some(kid) = kids.iter().find(|k| within(model, inner, k)) else {
                continue;
            };
            let (Some(c), Some(o)) = (boxes.get(*kid), boxes.get(outer.as_str())) else {
                continue;
            };
            let hidden = kids
                .iter()
                .filter(|k| *k != kid)
                .any(|k| boxes.get(*k).is_some_and(|s| in_front(zb, o, c, s)));
            if hidden {
                if !cards.contains(&kid.to_string()) {
                    cards.push(kid.to_string());
                }
                if !from.contains(outer) {
                    from.push(outer.clone());
                }
            }
        }
        if !cards.is_empty() {
            out.push(Hidden {
                zone: z.id.clone(),
                wrap,
                cards,
                from,
            });
        }
    }
    out
}

/// Is sibling `s` between card `c` and the border of zone `z` that faces the outside node `o`?
fn in_front(z: &Box2, o: &Box2, c: &Box2, s: &Box2) -> bool {
    let over = |a0: f64, a1: f64, b0: f64, b1: f64| a0.min(b1) - a1.max(b0) < -1.0;
    let rows = over(c.y, c.bottom(), s.y, s.bottom());
    let cols = over(c.x, c.right(), s.x, s.right());
    if o.right() <= z.x {
        rows && s.right() <= c.x + 0.5
    } else if o.x >= z.right() {
        rows && s.x >= c.right() - 0.5
    } else if o.bottom() <= z.y {
        cols && s.bottom() <= c.y + 0.5
    } else if o.y >= z.bottom() {
        cols && s.y >= c.bottom() - 0.5
    } else {
        false
    }
}
