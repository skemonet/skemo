//! Cards with no arrow go on a shelf at the end of their zone (G4, HOLA's "trees outside"): they
//! cut no flow and read as an inventory. They are left out of ELK's graph (`shelved`), then lined
//! up in writing order on one row after the zone's other content, and whatever stood beyond the
//! zone in its column moves on to make room.

use super::geom::{Axes, Boxes, extent, within};
use crate::layout::{Box2, Placement};
use crate::measure::Sizes;
use skemo_lang::{Kind, Model};

/// Gap between the zone's content and its shelf, as between rows.
const ROW_GAP: f64 = 48.0;
/// Gap between two cards of a shelf.
const GAP: f64 = 28.0;

fn linked(model: &Model, id: &str) -> bool {
    model.edges.iter().any(|e| e.from == id || e.to == id)
}

/// Does container `parent` hold a card with an arrow (at any depth)?
fn has_flow(model: &Model, parent: Option<&str>) -> bool {
    model.nodes.iter().any(|n| {
        n.kind == Kind::Card
            && linked(model, &n.id)
            && parent.is_none_or(|p| n.id != p && within(model, &n.id, p))
    })
}

/// Is card `id` set on its zone's shelf instead of being laid out by ELK? A card with no arrow,
/// a note too when nothing links it (pinned or not: `at=` applies after the layout, D-031), in a
/// zone whose other cards have arrows; never at the top level, in a stack or a `wrap` grid.
pub fn shelved(model: &Model, id: &str) -> bool {
    let Some(n) = model.node(id) else {
        return false;
    };
    if n.kind != Kind::Card || linked(model, id) {
        return false;
    }
    if n.classes.iter().any(|c| c == "wide") {
        return false;
    }
    // at the top level ELK already lines them up, centred on one row (`row_isolated` with `dir`
    // right or left); a stack or a `wrap` grid lays its cards in the author's order
    let Some(parent) = n.parent.as_deref() else {
        return false;
    };
    if model
        .node(parent)
        .is_some_and(|p| p.stack || p.wrap.is_some())
    {
        return false;
    }
    has_flow(model, Some(parent))
}

/// Sets the shelved cards of every container, innermost zones first.
pub fn place(model: &Model, sizes: &Sizes, elk: &mut Placement, (pad, head): (f64, f64)) {
    // the shelf reads like a last line of text: below the content, whatever `dir`
    let ax = Axes {
        across: false,
        reverse: false,
    };
    let mut zones: Vec<(usize, Option<&str>)> = model
        .nodes
        .iter()
        .filter(|n| n.kind == Kind::Zone)
        .map(|n| (depth(model, &n.id), Some(n.id.as_str())))
        .collect();
    zones.sort_by_key(|z| std::cmp::Reverse(z.0));
    zones.push((0, None));
    for (_, z) in zones {
        let shelf: Vec<&str> = model
            .children(z)
            .filter(|n| shelved(model, &n.id))
            .map(|n| n.id.as_str())
            .collect();
        if !shelf.is_empty() {
            set_shelf(model, ax, z, &shelf, (sizes, elk), (pad, head));
        }
    }
}

fn depth(model: &Model, id: &str) -> usize {
    let mut d = 0;
    let mut cur = model.node(id).and_then(|n| n.parent.as_deref());
    while let Some(p) = cur {
        d += 1;
        cur = model.node(p).and_then(|n| n.parent.as_deref());
    }
    d
}

/// Does an arrow leave container `z` past the end of its content along the flow (the side its
/// shelf would take)?
fn exits_at_end(model: &Model, ax: Axes, z: &str, end: f64, boxes: &Boxes) -> bool {
    model.edges.iter().any(|e| {
        let (a, b) = (within(model, &e.from, z), within(model, &e.to, z));
        let other = if a && !b {
            &e.to
        } else if b && !a {
            &e.from
        } else {
            return false;
        };
        boxes.get(other).is_some_and(|o| {
            if ax.reverse {
                ax.flow(o).1 <= end
            } else {
                ax.flow(o).0 >= end
            }
        })
    })
}

/// The shelf of container `z`: one row after its content along the flow, or, when arrows leave
/// the zone that way, one column after its content across the flow (it would stand in their way).
fn set_shelf(
    model: &Model,
    ax: Axes,
    z: Option<&str>,
    shelf: &[&str],
    (sizes, elk): (&Sizes, &mut Placement),
    (pad, head): (f64, f64),
) {
    let size = |id: &str| {
        let s = sizes
            .cards
            .get(id)
            .copied()
            .unwrap_or(crate::measure::Size { w: 170.0, h: 48.0 });
        Box2 {
            x: 0.0,
            y: 0.0,
            w: s.w,
            h: s.h,
        }
    };
    let frames = crate::layout::zone_frames(model, &cards_only(model, &elk.boxes), elk, pad, head);
    let content: Vec<Box2> = model
        .children(z)
        .filter(|n| !shelf.contains(&n.id.as_str()))
        .filter_map(|n| frames.get(&n.id).or_else(|| elk.boxes.get(&n.id)).copied())
        .collect();
    if content.is_empty() {
        return;
    }
    let (f0, f1) = extent(content.iter().map(|b| ax.flow(b)));
    let (s0, s1) = extent(content.iter().map(|b| ax.span(b)));
    let end = if ax.reverse { f0 } else { f1 };
    let along = z.is_none_or(|z| !exits_at_end(model, ax, z, end, &elk.boxes));
    let sizes_of: Vec<Box2> = shelf.iter().map(|id| size(id)).collect();
    if along {
        let deep = sizes_of.iter().map(|b| ax.flow_len(b)).fold(0.0, f64::max);
        let flow = if ax.reverse {
            end - ROW_GAP - deep
        } else {
            end + ROW_GAP
        };
        make_room(model, ax, z, shelf, (&frames, elk), (true, deep + ROW_GAP));
        // a shelf wider than the content widens the zone too: what stands beside it on its row
        // moves on (two shelved machines under a single broker ran into the next zone)
        let span = sizes_of.iter().map(|b| ax.span_len(b)).sum::<f64>()
            + GAP * (sizes_of.len() as f64 - 1.0);
        if s0 + span > s1 + 0.5 {
            make_room(model, ax, z, shelf, (&frames, elk), (false, s0 + span - s1));
        }
        let mut at = s0;
        for (id, b) in shelf.iter().zip(&sizes_of) {
            elk.boxes.insert((*id).to_string(), ax.at(*b, flow, at));
            at += ax.span_len(b) + GAP;
        }
    } else {
        // a column after the content, from the content's start along the flow
        let wide = sizes_of.iter().map(|b| ax.span_len(b)).fold(0.0, f64::max);
        // a zone wider than its content (a band, `.wide`) already has room on that side
        let slack = z
            .and_then(|z| frames.get(z))
            .map_or(0.0, |f| (ax.span(f).1 - s1 - pad).max(0.0));
        make_room(
            model,
            ax,
            z,
            shelf,
            (&frames, elk),
            (false, (wide + GAP - slack).max(0.0)),
        );
        let span = s1 + GAP;
        let mut at = if ax.reverse { f1 } else { f0 };
        for (id, b) in shelf.iter().zip(&sizes_of) {
            let flow = if ax.reverse { at - ax.flow_len(b) } else { at };
            elk.boxes.insert((*id).to_string(), ax.at(*b, flow, span));
            let step = ax.flow_len(b) + GAP;
            at += if ax.reverse { -step } else { step };
        }
    }
}

/// The zone grows by `room` (along the flow, or across it): what follows it in its container
/// that way moves on as far as needed to keep its gap, and so on up to the top level, as long
/// as a container actually grows.
fn make_room(
    model: &Model,
    ax: Axes,
    z: Option<&str>,
    shelf: &[&str],
    (frames, elk): (&Boxes, &mut Placement),
    (along, room): (bool, f64),
) {
    let end_of = |b: &Box2| {
        if along {
            if ax.reverse {
                -ax.flow(b).0
            } else {
                ax.flow(b).1
            }
        } else {
            ax.span(b).1
        }
    };
    let start_of = |b: &Box2| {
        if along {
            if ax.reverse {
                -ax.flow(b).1
            } else {
                ax.flow(b).0
            }
        } else {
            ax.span(b).0
        }
    };
    let mut cur = z;
    let mut grown = room;
    while let (Some(id), true) = (cur, grown > 0.5) {
        let Some(f) = frames.get(id).copied() else {
            break;
        };
        let (old_end, new_end) = (end_of(&f), end_of(&f) + grown);
        let parent = model.node(id).and_then(|n| n.parent.as_deref());
        let siblings: Vec<(String, Box2)> = model
            .children(parent)
            .filter(|n| n.id != id && !shelf.contains(&n.id.as_str()))
            .filter_map(|n| {
                let b = frames
                    .get(&n.id)
                    .or_else(|| elk.boxes.get(&n.id))
                    .copied()?;
                Some((n.id.clone(), b))
            })
            .collect();
        let mut after: Vec<(String, Box2)> = siblings
            .iter()
            .filter(|(_, b)| start_of(b) >= old_end - 1.0 && (along || ax.same_row(b, &f)))
            .cloned()
            .collect();
        let first = after
            .iter()
            .map(|(_, b)| start_of(b))
            .fold(f64::INFINITY, f64::min);
        let gap = if first.is_finite() {
            first - old_end
        } else {
            0.0
        };
        let shift = (new_end + gap - first).max(0.0);
        // across the flow, a sibling off the row that a moved one would now cover moves too
        // (zones laid as a staircase), and so on
        if !along {
            spread_shift(ax, &siblings, &mut after, shift, |b| {
                start_of(b) >= old_end - 1.0
            });
        }
        for (n, _) in &after {
            if along {
                ax.shift_along(
                    model,
                    n,
                    if ax.reverse { -shift } else { shift },
                    &mut elk.boxes,
                );
            } else {
                ax.shift_across(model, n, shift, &mut elk.boxes);
            }
        }
        // how much the parent grows: its content's end moves from the old to the new one
        let Some(pf) = parent.and_then(|p| frames.get(p)).copied() else {
            break;
        };
        let content_end = model
            .children(parent)
            .filter(|n| !shelf.contains(&n.id.as_str()))
            .filter_map(|n| frames.get(&n.id).or_else(|| elk.boxes.get(&n.id)).copied())
            .map(|b| end_of(&b))
            .fold(f64::NEG_INFINITY, f64::max);
        let pad = end_of(&pf) - content_end;
        let new_content_end = after
            .iter()
            .map(|(_, b)| end_of(b) + shift)
            .fold(new_end, f64::max)
            .max(content_end);
        grown = (new_content_end + pad - end_of(&pf)).max(0.0);
        cur = parent;
    }
}

/// Adds to `moved` every sibling beyond the grown zone (`beyond`) that a moved box, shifted
/// across the flow by `shift`, would cover, until none is left.
fn spread_shift(
    ax: Axes,
    siblings: &[(String, Box2)],
    moved: &mut Vec<(String, Box2)>,
    shift: f64,
    beyond: impl Fn(&Box2) -> bool,
) {
    let shifted = |b: &Box2| ax.at(*b, ax.flow(b).0, ax.span(b).0 + shift);
    loop {
        let more: Vec<(String, Box2)> = siblings
            .iter()
            .filter(|(id, b)| beyond(b) && !moved.iter().any(|(m, _)| m == id))
            .filter(|(_, b)| moved.iter().any(|(_, m)| overlaps(&shifted(m), b)))
            .cloned()
            .collect();
        if more.is_empty() {
            return;
        }
        moved.extend(more);
    }
}

fn overlaps(a: &Box2, b: &Box2) -> bool {
    a.x < b.right() && b.x < a.right() && a.y < b.bottom() && b.y < a.bottom()
}

/// The card boxes only (zone frames are computed from them).
fn cards_only(model: &Model, boxes: &Boxes) -> Boxes {
    boxes
        .iter()
        .filter(|(id, _)| model.node(id).is_some_and(|n| n.kind == Kind::Card))
        .map(|(id, b)| (id.clone(), *b))
        .collect()
}
