//! Sides of the arrows' ends: the drawing's direction, back edges and their channel, cross links.

use super::groups::Bus;
use super::{Mode, sides};
use crate::layout::Box2;
use skemo_lang::{Kind, Model};
use skemo_router::Side;
use std::collections::BTreeMap;

/// Sides of an arrow's two ends, whether it is a back edge (no ELK hint), and its mind-map group.
pub(super) fn edge_sides(
    model: &Model,
    boxes: &BTreeMap<String, Box2>,
    e: &skemo_lang::Edge,
    mode: Mode,
    buses: &BTreeMap<String, Bus>,
) -> (Option<Side>, Option<Side>, bool, Option<String>) {
    let (mut s1, mut s2) = sides(e.side.as_deref());
    if mode == Mode::Timeline && e.side.is_none() {
        let lane = |id: &str| model.ancestors(id).first().map(|n| n.id.clone());
        s1 = Some(Side::Right);
        if lane(&e.from) == lane(&e.to) {
            s2 = Some(Side::Left);
        }
    }
    if let Some(b) = buses.get(&e.id) {
        (s1, s2) = (Some(b.sides.0), Some(b.sides.1));
    }
    let back = mode == Mode::Graph && is_back_edge(model, boxes, e);
    if mode == Mode::Graph {
        (s1, s2) = graph_sides(model, boxes, e, back, (s1, s2));
    }
    let mut mindmap_group = None;
    if mode == Mode::Mindmap && e.side.is_none() {
        let (from, to) = (boxes[&e.from], boxes[&e.to]);
        let outward_right = to.x + to.w / 2.0 > from.x + from.w / 2.0;
        (s1, s2) = if outward_right {
            (Some(Side::Right), Some(Side::Left))
        } else {
            (Some(Side::Left), Some(Side::Right))
        };
        mindmap_group = Some(format!("{}|mindmap|{outward_right}", e.from));
    }
    (s1, s2, back, mindmap_group)
}

/// Sides of an arrow in graph mode (D-048): a cross link (R9) enters its target by the facing side;
/// a back edge (R6) loops around the row (under it with `dir="right"`, on its right with
/// `dir="down"`) instead of running beside the arrow that goes forward.
fn graph_sides(
    model: &Model,
    boxes: &BTreeMap<String, Box2>,
    e: &skemo_lang::Edge,
    back: bool,
    (mut s1, mut s2): (Option<Side>, Option<Side>),
) -> (Option<Side>, Option<Side>) {
    if e.side.is_none()
        && e.via.is_none()
        && let Some((a, b)) = cross_link_sides(model, boxes, e)
    {
        (s1, s2) = (a, Some(b));
    }
    if back && e.side.is_none() && e.via.is_none() && straight_back(model, boxes, e) {
        let after = after_side(model);
        (s1, s2) = (Some(opposite(after)), Some(after));
    } else if back && let Some((a, b)) = corner_back(model, boxes, e) {
        (s1, s2) = (Some(a), Some(b));
    } else if back {
        let s = back_side(model);
        (s1, s2) = (Some(s), Some(s));
    }
    if e.side.is_none() && e.via.is_none() && against_stack(model, boxes, e) {
        let after = after_side(model);
        (s1, s2) = (Some(opposite(after)), Some(after));
    } else if e.side.is_none() && e.via.is_none() {
        // the side a trunk leaves by, or its branches come in by, is theirs: other arrows use
        // the opposite side (D-050)
        if trunk_back_from(model, boxes, e) {
            s2 = Some(after_side(model));
        }
        if branch_back_into(model, boxes, e) {
            s1 = Some(opposite(after_side(model)));
        }
    }
    (s1, s2)
}

/// A back edge whose target stands right before its source, face to face (their extents across
/// the flow share at least `FACE` px), with no card between them and no arrow the other way:
/// nothing runs beside it, so it goes straight back instead of looping round the row (R6), which
/// on a wide row went round the whole drawing.
pub(super) fn straight_back(
    model: &Model,
    boxes: &BTreeMap<String, Box2>,
    e: &skemo_lang::Edge,
) -> bool {
    const FACE: f64 = 24.0;
    let (Some(a), Some(b)) = (boxes.get(&e.from), boxes.get(&e.to)) else {
        return false;
    };
    let across = matches!(model.option("dir").unwrap_or("down"), "right" | "left");
    let span = |r: &Box2| {
        if across {
            (r.y, r.bottom())
        } else {
            (r.x, r.right())
        }
    };
    let flow = |r: &Box2| {
        if across {
            (r.x, r.right())
        } else {
            (r.y, r.bottom())
        }
    };
    let (sa, sb) = (span(a), span(b));
    let (lo, hi) = (sa.0.max(sb.0), sa.1.min(sb.1));
    if hi - lo < FACE {
        return false;
    }
    let (fa, fb) = (flow(a), flow(b));
    let (gap0, gap1) = (fa.1.min(fb.1), fa.0.max(fb.0));
    let between = model
        .nodes
        .iter()
        .filter(|n| n.kind == skemo_lang::Kind::Card && n.id != e.from && n.id != e.to)
        .filter_map(|n| boxes.get(&n.id))
        .any(|c| {
            let (s, f) = (span(c), flow(c));
            s.0 < hi && s.1 > lo && f.0 < gap1 && f.1 > gap0
        });
    let twin = model.edges.iter().any(|f| f.from == e.to && f.to == e.from);
    !between && !twin
}

/// A back edge to a card wholly to one side of its source (across the flow), with no arrow out
/// of it and no card on the corner between them: it leaves by the side facing the target and
/// enters it by its side after the flow, an L as by hand (an alarm below and beside the topic it
/// feeds), instead of looping round the row. No arrow leaves the target, so none goes forward
/// beside it. The sides (source, target), or `None`.
pub(super) fn corner_back(
    model: &Model,
    boxes: &BTreeMap<String, Box2>,
    e: &skemo_lang::Edge,
) -> Option<(Side, Side)> {
    const CLEAR: f64 = 8.0;
    let (a, b) = (boxes.get(&e.from)?, boxes.get(&e.to)?);
    if e.side.is_some() || e.via.is_some() || model.edges.iter().any(|f| f.from == e.to) {
        return None;
    }
    let ax = crate::tidy::geom::Axes::of(model);
    let ((sa, sb), (fa, fb)) = ((ax.span(a), ax.span(b)), (ax.flow(a), ax.flow(b)));
    let ahead = sb.0 >= sa.1;
    if !ahead && sb.1 > sa.0 {
        return None;
    }
    // the corner: across from the source's middle to the target's middle, then along the flow
    // to the target's side after the flow, which the source's middle lies beyond
    let (f, s) = (f64::midpoint(fa.0, fa.1), f64::midpoint(sb.0, sb.1));
    let edge = if ax.reverse { fb.0 } else { fb.1 };
    if (f - edge).abs() < 2.0 * CLEAR || (f < edge) != ax.reverse {
        return None;
    }
    let legs = [
        (
            (if ahead { sa.1 } else { s }, if ahead { s } else { sa.0 }),
            (f, f),
        ),
        ((s, s), (f.min(edge), f.max(edge))),
    ];
    let on_corner = model
        .nodes
        .iter()
        .filter(|n| n.kind == Kind::Card && n.id != e.from && n.id != e.to)
        .filter_map(|n| boxes.get(&n.id))
        .any(|c| {
            let (cs, cf) = (ax.span(c), ax.flow(c));
            legs.iter().any(|((s0, s1), (f0, f1))| {
                cs.0 < s1 + CLEAR && cs.1 > s0 - CLEAR && cf.0 < f1 + CLEAR && cf.1 > f0 - CLEAR
            })
        });
    let facing = match (ax.across, ahead) {
        (false, true) => Side::Right,
        (false, false) => Side::Left,
        (true, true) => Side::Bottom,
        (true, false) => Side::Top,
    };
    (!on_corner).then_some((facing, after_side(model)))
}

/// An arrow out of a card that a branch comes into back up a stack (by its side facing
/// forward), towards a card after it along the flow: it would leave by that side (D-050).
fn branch_back_into(model: &Model, boxes: &BTreeMap<String, Box2>, e: &skemo_lang::Edge) -> bool {
    let (Some(a), Some(b)) = (boxes.get(&e.from), boxes.get(&e.to)) else {
        return false;
    };
    after(model, a, b)
        && model.edges.iter().any(|f| {
            f.to == e.from
                && f.id != e.id
                && against_stack(model, boxes, f)
                && (fan_in(e) || forks_back(model, boxes, &f.from))
        })
}

/// An arrow into a card whose trunk leaves back up a stack, from a card before it along the
/// flow: it would enter by the trunk's side (D-050).
fn trunk_back_from(model: &Model, boxes: &BTreeMap<String, Box2>, e: &skemo_lang::Edge) -> bool {
    let (Some(a), Some(b)) = (boxes.get(&e.from), boxes.get(&e.to)) else {
        return false;
    };
    after(model, a, b)
        && model
            .edges
            .iter()
            .any(|f| f.from == e.to && f.id != e.id && against_stack(model, boxes, f))
        && (fan_in(e) || forks_back(model, boxes, &e.to))
}

/// The zones an arrow may cross: those around its ends, and those that keep a corridor between
/// their rows (D-056).
pub(super) fn crossable(model: &Model, e: &skemo_lang::Edge) -> Vec<String> {
    let mut pass: Vec<String> = model
        .ancestors(&e.from)
        .into_iter()
        .chain(model.ancestors(&e.to))
        .chain(
            model
                .nodes
                .iter()
                .filter(|z| crate::layout::corridor(model, &z.id)),
        )
        .map(|n| n.id.clone())
        .collect();
    pass.sort();
    pass.dedup();
    pass
}

/// Does card `id` fork back up a stack (two arrows or more against its order)? Such a trunk and
/// its branches keep a side to themselves; a single arrow back up shares its side, except with the
/// arrows of a named join, a flow of their own (D-058).
fn forks_back(model: &Model, boxes: &BTreeMap<String, Box2>, id: &str) -> bool {
    model
        .edges
        .iter()
        .filter(|f| f.from == id && against_stack(model, boxes, f))
        .nth(1)
        .is_some()
}

/// An arrow with a named join (`join="http"`, D-050): it shares a trunk into its target with the
/// other arrows of that name.
pub fn fan_in(e: &skemo_lang::Edge) -> bool {
    e.join.as_deref().is_some_and(|j| j != "trunk")
}

/// Marks the fork groups of arrows going against the flow between two children of a stack.
pub(super) const AGAINST: &str = "|against";

/// An arrow between two children of a stack whose target lies entirely before its source along
/// the flow (D-050): the stack's order holds, so it leaves by the side facing back and enters by
/// the side facing forward, straight, as a fork when it has siblings.
pub(super) fn against_stack(
    model: &Model,
    boxes: &BTreeMap<String, Box2>,
    e: &skemo_lang::Edge,
) -> bool {
    let (Some(a), Some(b)) = (boxes.get(&e.from), boxes.get(&e.to)) else {
        return false;
    };
    crate::stack::across_stack(model, e).is_some() && after(model, b, a)
}

/// Does `b` lie entirely after `a` along the drawing's direction?
fn after(model: &Model, a: &Box2, b: &Box2) -> bool {
    match model.option("dir").unwrap_or("down") {
        "right" => b.x >= a.right(),
        "left" => b.right() <= a.x,
        "up" => b.bottom() <= a.y,
        _ => b.y >= a.bottom(),
    }
}

/// The side arrows leave by in the drawing's direction.
pub(super) fn after_side(model: &Model) -> Side {
    match model.option("dir").unwrap_or("down") {
        "up" => Side::Top,
        "right" => Side::Right,
        "left" => Side::Left,
        _ => Side::Bottom,
    }
}

pub(super) fn opposite(s: Side) -> Side {
    match s {
        Side::Top => Side::Bottom,
        Side::Bottom => Side::Top,
        Side::Left => Side::Right,
        Side::Right => Side::Left,
    }
}

/// An arrow that goes against the drawing's direction: its target lies entirely before its
/// source along the flow. Only plain ones (no `side=`, no `via=`, no group), in the layout.
fn is_back_edge(model: &Model, boxes: &BTreeMap<String, Box2>, e: &skemo_lang::Edge) -> bool {
    let (Some(a), Some(b)) = (boxes.get(&e.from), boxes.get(&e.to)) else {
        return false;
    };
    // between two children of a stack, the written order holds: no loop around (D-050)
    if !e.constraint
        || e.side.is_some()
        || e.via.is_some()
        || e.join.is_some()
        || e.classes.iter().any(|c| c == "link")
        || crate::stack::across_stack(model, e).is_some()
    {
        return false;
    }
    after(model, b, a)
}

/// Sides of a cross link to a card that only has cross links (`constraint="false"`, D-048 R9):
/// the target is entered by its side facing the source; the source leaves by the side after the
/// flow when another card stands between it and the target (the arrow goes around instead of
/// crossing a fork), by the side facing the target otherwise.
fn cross_link_sides(
    model: &Model,
    boxes: &BTreeMap<String, Box2>,
    e: &skemo_lang::Edge,
) -> Option<(Option<Side>, Side)> {
    // a cell of a `wrap` grid keeps the router's choice (its place is the grid's)
    let in_grid = model
        .node(&e.to)
        .and_then(|n| n.parent.as_deref())
        .and_then(|z| model.node(z))
        .is_some_and(|z| z.wrap.is_some());
    if e.constraint
        || in_grid
        || model
            .edges
            .iter()
            .any(|f| (f.from == e.to || f.to == e.to) && f.constraint)
    {
        return None;
    }
    let (a, t) = (boxes.get(&e.from)?, boxes.get(&e.to)?);
    let dir = model.option("dir").unwrap_or("down");
    let across = matches!(dir, "right" | "left");
    let after = after_side(model);
    // the target's side facing the source: below or above it, or beside it
    let entry = if across {
        if a.right() <= t.x {
            Side::Left
        } else if a.x >= t.right() {
            Side::Right
        } else if a.y > t.y {
            Side::Bottom
        } else {
            Side::Top
        }
    } else if a.bottom() <= t.y {
        Side::Top
    } else if a.y >= t.bottom() {
        Side::Bottom
    } else if a.x > t.x {
        Side::Right
    } else {
        Side::Left
    };
    // the band between the source and the target, at the source's level
    let (lo, hi) = if across {
        (a.bottom().min(t.bottom()), a.y.max(t.y))
    } else {
        (a.right().min(t.right()), a.x.max(t.x))
    };
    let blocked = boxes.iter().any(|(id, b)| {
        id != &e.from
            && id != &e.to
            && model.node(id).is_some_and(|n| n.kind == Kind::Card)
            && if across {
                b.y < hi && b.bottom() > lo && b.x < a.right() && b.right() > a.x
            } else {
                b.x < hi && b.right() > lo && b.y < a.bottom() && b.bottom() > a.y
            }
    });
    // free: leave by the side facing the target, across the flow
    let facing = if across {
        if t.y > a.y { Side::Bottom } else { Side::Top }
    } else if t.x > a.x {
        Side::Right
    } else {
        Side::Left
    };
    Some((Some(if blocked { after } else { facing }), entry))
}

/// Gap between a back edge's run and what it goes around (px).
const BACK_GAP: f64 = 24.0;

/// The point a back edge runs through (D-049): beyond its two cards and the frames of the zones
/// holding either of them, mid-way between the cards. A hand drawing loops under the whole row,
/// zone frame included, instead of running along the frame's inner edge. `None` (the router's
/// choice) when a card or another zone stands on that run.
pub(super) fn back_channel(
    model: &Model,
    boxes: &BTreeMap<String, Box2>,
    e: &skemo_lang::Edge,
) -> Option<(f64, f64)> {
    let (a, b) = (boxes.get(&e.from)?, boxes.get(&e.to)?);
    let across = matches!(model.option("dir").unwrap_or("down"), "right" | "left");
    // along the run: from one card's centre to the other's
    let (c0, c1) = if across {
        let (p, q) = (a.x + a.w / 2.0, b.x + b.w / 2.0);
        (p.min(q), p.max(q))
    } else {
        let (p, q) = (a.y + a.h / 2.0, b.y + b.h / 2.0);
        (p.min(q), p.max(q))
    };
    let span = |r: &Box2| {
        if across {
            (r.x, r.right())
        } else {
            (r.y, r.bottom())
        }
    };
    let beyond = |r: &Box2| if across { r.bottom() } else { r.right() };
    let ancestors: Vec<&str> = model
        .ancestors(&e.from)
        .into_iter()
        .chain(model.ancestors(&e.to))
        .map(|n| n.id.as_str())
        .collect();
    // the zones holding one end only: a zone holding both is not left
    let shared = |z: &&str| {
        model.ancestors(&e.from).iter().any(|n| n.id == *z)
            && model.ancestors(&e.to).iter().any(|n| n.id == *z)
    };
    let level = ancestors
        .iter()
        .filter(|z| !shared(z))
        .filter_map(|z| boxes.get(*z))
        .filter(|r| span(r).0 < c1 && span(r).1 > c0)
        .map(beyond)
        .fold(beyond(a).max(beyond(b)), f64::max)
        + BACK_GAP;
    // the run must be free: no other card, no zone it does not belong to
    let blocked = model.nodes.iter().any(|n| {
        n.id != e.from
            && n.id != e.to
            && !ancestors.contains(&n.id.as_str())
            && boxes.get(&n.id).is_some_and(|r| {
                let (lo, hi) = span(r);
                let (top, bottom) = if across {
                    (r.y, r.bottom())
                } else {
                    (r.x, r.right())
                };
                lo < c1
                    && hi > c0
                    && top < level + BACK_GAP / 2.0
                    && bottom > level - BACK_GAP / 2.0
            })
    });
    let mid = f64::midpoint(c0, c1);
    (!blocked).then_some(if across { (mid, level) } else { (level, mid) })
}

/// The side a back edge leaves and enters by: across the flow, below or on the right.
fn back_side(model: &Model) -> Side {
    match model.option("dir").unwrap_or("down") {
        "right" | "left" => Side::Bottom,
        _ => Side::Right,
    }
}
