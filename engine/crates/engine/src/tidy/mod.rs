//! Tidying ELK's layout like a hand drawing (`layout="graph"` only).
//!
//! ELK's layered algorithm places every card, then these passes adjust the result: widths are
//! evened before ELK runs (`before_elk`), boxes are moved after it (`after_elk`). Each pass after
//! ELK reads and moves the boxes only; a card it moves loses the route hint ELK gave its arrows
//! (`drop_stale_routes`), so the router traces them anew.
//!
//! Everything here runs on ELK's placement, **before** the pinned positions (`at=`) are applied,
//! and sees no card as pinned (`layout::unpinned`): pinning a card moves no other card (D-031).

mod centre;
mod columns;
pub(crate) mod geom;
mod grid;
pub(crate) mod notes;
mod rows;
mod share;
pub mod shelf;
mod widths;

pub use columns::stretch_zones;
pub use grid::{Hidden, hidden_by_wrap, inventory_grids};
pub use notes::notes_unconstrained;
pub use rows::zones_in_order;

use crate::layout::{Box2, Placement};
use crate::measure::Sizes;
use crate::scene::Scene;
use geom::Boxes;
use skemo_lang::Model;

/// Measured sizes evened out before ELK runs: siblings, columns of a grid. A zone's title no
/// longer widens its first card (D-099): it keeps clear of the arrows by D-064, wrapping first.
pub fn before_elk(model: &Model, sizes: &mut Sizes) {
    // widths are evened on the top card of a pile, its cards behind added back after
    crate::measure::decks(model, sizes, -1.0);
    if !crate::trace::off("even_sibling_widths") {
        widths::even_sibling_widths(model, sizes);
    }
    if !crate::trace::off("even_wrap_widths") {
        widths::even_wrap_widths(model, sizes);
    }
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
    crate::trace::boxes("elk", &Boxes::new(), &elk.boxes);
    elk.frame = Some((pad, head));
    let on = |pass: &str| !crate::trace::off(pass);
    // stacks first, so that the passes below see each child apart from the others (D-050)
    if on("arrange") {
        crate::stack::arrange(model, elk);
    }
    // rows: sources on the first row, leaves back on their siblings' row, writing order, zones of a row on one line, grids
    let lifted = elk.boxes.clone();
    run(model, elk, Hints::Drop, rows::lift_sources);
    run(model, elk, Hints::Drop, rows::lift_leaves);
    // a zone whose card moved to another row is a row of its own no more: `order_rows` hands
    // the slots out on the zones' frames as they now stand
    refresh_frames(model, elk, &lifted);
    run(model, elk, Hints::Drop, rows::order_rows);
    run(model, elk, Hints::Drop, rows::align_zone_rows);
    run(model, elk, Hints::Drop, grid::wrap_grid);
    // centring: lone zones on their targets, parents on their children, children under parents
    run(model, elk, Hints::ZRoute, centre::center_lonely);
    let titles = crate::labels::zone_titles(model, scene, &elk.boxes);
    named("center_parents", model, elk, Hints::Drop, |m, b| {
        centre::center_parents(m, b, &titles);
    });
    run(model, elk, Hints::Drop, centre::center_children);
    // a leaf pushed aside by a zone, onto its parent's row once the rows are centred: it moves alone
    run(model, elk, Hints::Drop, rows::leaves_beside);
    // cards off the flow: notes, isolated cards, targets of cross links
    if on("note_room") {
        note_room(model, elk, (pad, head));
    }
    run(model, elk, Hints::Drop, notes::place_notes);
    run(model, elk, Hints::Drop, rows::row_isolated);
    run(model, elk, Hints::Drop, notes::place_cross_targets);
    run(model, elk, Hints::Drop, columns::align_columns);
    // a triangle a → b → c with a → c, in one column: straight long arrow, mirrored hooks (D-099)
    run(model, elk, Hints::Drop, centre::triangle_hooks);
    // room left of the cards that the zone titles would cover (D-064)
    let mut roomy = Vec::new();
    if on("title_room") {
        roomy = title_room(model, elk, (pad, head), &[]);
    }
    // room between two cards side by side for the label of the arrow between them
    if on("label_room") {
        label_room(model, sizes, elk, (pad, head));
    }
    // a card above its only target in a zone, now that the titles have room (D-064)
    let mut framed = elk.boxes.clone();
    framed.extend(crate::layout::zone_frames(
        model, &elk.boxes, elk, pad, head,
    ));
    let titles = crate::labels::zone_titles(model, scene, &framed);
    let rules = elk.clone();
    let framed_fit =
        |id: &str, b: &Box2, boxes: &Boxes| frames_fit(model, &rules, (pad, head), (id, b), boxes);
    named("spines_into_zones", model, elk, Hints::Drop, |m, b| {
        centre::spines_into_zones(m, b, &titles, &framed_fit);
    });
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
    if on("arrange") {
        crate::stack::arrange(model, elk);
    }
    if on("stretch") {
        crate::stack::stretch(model, elk, sizes, (pad, head, false));
    }
    if on("columns") {
        crate::stack::columns(model, elk);
    }
    // wide cards again, over the groups the columns may have moved
    if on("stretch") {
        crate::stack::stretch(model, elk, sizes, (pad, head, true));
    }
    // the cards standing on a stretched card split its width (D-092)
    run(model, elk, Hints::Drop, share::share_stretched);
    // cards with no arrow, left out of ELK's graph, on a shelf at the end of their zone
    if on("shelf") {
        shelf::place(model, sizes, elk, (pad, head));
    }
    // the passes since may have set a card linked from above in a zone's first row (D-064): that
    // zone widens now, and makes room as the others did (the private map's `hub`: a zone ran over
    // its neighbour once the card above its first one had moved)
    if on("title_room") {
        title_room(model, elk, (pad, head), &roomy);
    }
    whole_pixels(model, elk);
}

/// Cards on whole pixels, as the editor pins them (`edit::move_card` writes integers): pinning a
/// card where it stands then puts it exactly there (D-031). ELK centres cards on half pixels; a
/// card pinned half a pixel off lost its route hint and its arrows, then their neighbours, were
/// traced anew.
fn whole_pixels(model: &Model, elk: &mut Placement) {
    if crate::trace::off("whole_pixels") {
        return;
    }
    let before = elk.boxes.clone();
    for n in model
        .nodes
        .iter()
        .filter(|n| n.kind == skemo_lang::Kind::Card)
    {
        if let Some(b) = elk.boxes.get_mut(&n.id) {
            b.x = b.x.round();
            b.y = b.y.round();
        }
    }
    crate::trace::boxes("whole_pixels", &before, &elk.boxes);
}

/// D-064: a zone widens to the left so that its title leaves a card's top free
/// (`layout::zone_frames`); what lies left of it on its row moves left as far, at its level and
/// at each level above (an ancestor grows with it). Its own cards stay: a zone of a stack keeps
/// lined up with the others, which take its width (`stretch_zones`).
fn title_room(
    model: &Model,
    elk: &mut Placement,
    (pad, head): (f64, f64),
    skip: &[String],
) -> Vec<String> {
    // the zones widened for a title clear of a card's top (D-064) or for a title wider than its
    // cards (bug 3, D-099): a zone widened only for the second ran over the zone beside it (the
    // private map's `hub`, once a theme gave its title a longer label)
    let mut zones: Vec<String> = elk
        .title_ends
        .keys()
        .chain(elk.title_widths.keys())
        .cloned()
        .collect();
    zones.sort();
    zones.dedup();
    // every box before any zone widened: a gap already opened for one zone serves the next ones
    let plain = Placement {
        title_ends: std::collections::BTreeMap::new(),
        title_widths: std::collections::BTreeMap::new(),
        ..elk.clone()
    };
    let mut orig = elk.boxes.clone();
    orig.extend(crate::layout::zone_frames(
        model, &elk.boxes, &plain, pad, head,
    ));
    let mut done: Vec<&String> = Vec::new();
    let mut widened_now = Vec::new();
    for z in zones.iter().filter(|z| !skip.contains(z)) {
        let plain = Placement {
            title_ends: std::collections::BTreeMap::new(),
            title_widths: std::collections::BTreeMap::new(),
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
        // the frames with the title room of the zones already widened: each zone makes room
        // for its own title, a zone widened later for its own
        let keep = |m: &std::collections::BTreeMap<String, f64>| {
            m.iter()
                .filter(|(k, _)| done.contains(k))
                .map(|(k, v)| (k.clone(), *v))
                .collect()
        };
        let partial = Placement {
            title_ends: keep(&elk.title_ends),
            title_widths: keep(&elk.title_widths),
            ..elk.clone()
        };
        let widened = crate::layout::zone_frames(model, &elk.boxes, &partial, pad, head);
        let room = Room {
            frames: &frames,
            wide: &widened,
            orig: &orig,
        };
        // D-099: the zone widens as much to the right, so that its cards stay in its middle
        named("make_room", model, elk, Hints::Drop, |m, boxes| {
            make_room(m, z, (d, Side::Left), &room, boxes);
            make_room(m, z, (d, Side::Right), &room, boxes);
        });
        done.push(z);
        widened_now.push(z.clone());
    }
    widened_now
}

/// Can card `id` take box `b` without one of its zones, framed by `layout::zone_frames` (the
/// owner of that rule), coming nearer a neighbour than it stood? `fits` reads the zone boxes ELK
/// gave, which the passes do not keep up to date, and grows them by a fixed margin: a zone widened
/// on both sides for its title (D-064, D-099) follows its card both ways, and ran over the zone
/// beside it (`wrkr-hcp` of the private corpus). `rules`: the placement whose title room the
/// frames keep.
fn frames_fit(
    model: &Model,
    rules: &Placement,
    (pad, head): (f64, f64),
    (id, b): (&str, &Box2),
    boxes: &Boxes,
) -> bool {
    const GAP: f64 = 8.0;
    let mut trial = boxes.clone();
    trial.insert(id.to_string(), *b);
    let (before, after) = (
        crate::layout::zone_frames(model, boxes, rules, pad, head),
        crate::layout::zone_frames(model, &trial, rules, pad, head),
    );
    let get = |frames: &Boxes, bx: &Boxes, n: &str| frames.get(n).or_else(|| bx.get(n)).copied();
    // how much of `s` lies within `GAP` of `a`
    let close = |a: Box2, s: Box2| {
        let w = (a.right() + GAP).min(s.right()) - (a.x - GAP).max(s.x);
        let h = (a.bottom() + GAP).min(s.bottom()) - (a.y - GAP).max(s.y);
        w.max(0.0) * h.max(0.0)
    };
    let mut cur = model.node(id).and_then(|n| n.parent.as_deref());
    while let Some(z) = cur {
        let parent = model.node(z).and_then(|n| n.parent.as_deref());
        for s in model.children(parent).filter(|s| s.id != z) {
            let pair = |frames: &Boxes, bx: &Boxes| get(frames, bx, z).zip(get(frames, bx, &s.id));
            if let (Some((a0, s0)), Some((a1, s1))) = (pair(&before, boxes), pair(&after, &trial))
                && close(a1, s1) > close(a0, s0) + 0.5
            {
                return false;
            }
        }
        cur = parent;
    }
    true
}

/// The side of a zone where room is made.
#[derive(Clone, Copy, PartialEq)]
enum Side {
    Left,
    Right,
}

/// The frames `make_room` reads. `frames`: every box as it stands, zones without title room;
/// `wide`: the zones' frames with the title room of the zones already widened (each zone makes
/// room for its own title once, whichever comes first); `orig`: every box before any zone widened.
struct Room<'a> {
    frames: &'a Boxes,
    wide: &'a Boxes,
    orig: &'a Boxes,
}

/// Moves away what lies on `side` of `z` on its row (whole nodes, with their content), then
/// what lies on that side of each ancestor on its row, so that `z` can widen by `d` that way:
/// by `d` less what the gap has grown since title room began (several zones with one edge widen
/// it once). And at least so far that the two frames, each with its title room, stay as far
/// apart as before, or `NODE_GAP` if they stood farther: the second of two zones widening
/// toward each other counted the first one's widening as room of its own, and the two frames
/// overlapped (`hub` of the private corpus).
fn make_room(model: &Model, z: &str, (d, side): (f64, Side), room: &Room, boxes: &mut Boxes) {
    let Room { frames, wide, orig } = *room;
    // along x, outwards from the zone: its edge on that side, and a box's edge facing it
    let s = if side == Side::Left { -1.0 } else { 1.0 };
    let own = |b: &Box2| if side == Side::Left { b.x } else { b.right() };
    let facing = |b: &Box2| if side == Side::Left { b.right() } else { b.x };
    let mut node = z;
    while let Some(n) = model.node(node) {
        let (Some(me), Some(me0)) = (frames.get(node).copied(), orig.get(node).copied()) else {
            return;
        };
        let beyond: Vec<&str> = model
            .children(n.parent.as_deref())
            .filter(|o| {
                frames.get(&o.id).is_some_and(|b| {
                    o.id != node
                        && s * (facing(b) - own(&me)) >= -0.5
                        && b.y < me.bottom()
                        && me.y < b.bottom()
                })
            })
            .map(|o| o.id.as_str())
            .collect();
        // a box's facing edge, less the title room a zone takes on that side
        let extra = |id: &str, b: &Box2| {
            wide.get(id)
                .map_or(0.0, |w| (s * (facing(b) - facing(w))).max(0.0))
        };
        let edge = |bs: &Boxes, titles: bool| {
            beyond
                .iter()
                .filter_map(|id| bs.get(*id).map(|b| (id, b)))
                .map(|(id, b)| s * facing(b) - if titles { extra(id, b) } else { 0.0 })
                .fold(f64::INFINITY, f64::min)
        };
        let gap0 = edge(orig, false) - s * own(&me0);
        let grown = (edge(frames, false) - s * own(&me)) - gap0;
        // the gap the two frames with their title room would leave, and the least they keep
        let wide_gap = edge(frames, true) - s * own(&me) - d;
        let least = gap0.min(crate::layout::NODE_GAP);
        let shift = if grown.is_finite() {
            (d - grown.max(0.0)).max(least - wide_gap).max(0.0)
        } else {
            d
        };
        // a sibling off the row that a moved one would now cover moves too (zones laid as a
        // staircase), and so on
        let mut moved = beyond;
        loop {
            let more: Vec<&str> = model
                .children(n.parent.as_deref())
                .filter(|o| o.id != node && !moved.contains(&o.id.as_str()))
                .filter(|o| {
                    frames.get(&o.id).is_some_and(|b| {
                        s * (facing(b) - own(&me)) >= -0.5
                            && moved.iter().filter_map(|m| frames.get(*m)).any(|m| {
                                let m = Box2 {
                                    x: m.x + s * shift,
                                    ..*m
                                };
                                m.x < b.right()
                                    && b.x < m.right()
                                    && m.y < b.bottom()
                                    && b.y < m.bottom()
                            })
                    })
                })
                .map(|o| o.id.as_str())
                .collect();
            if more.is_empty() {
                break;
            }
            moved.extend(more);
        }
        for id in moved {
            shift_x(model, id, s * shift, boxes);
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

/// A note with no free place beside its card (D-104): with `dir` down or up, the row opens after
/// the card for it, as it does for a label between two cards (`label_room`), rather than the note
/// going beyond everything at the card's level with a link across the drawing (bug 25).
fn note_room(model: &Model, elk: &mut Placement, (pad, head): (f64, f64)) {
    let ax = geom::Axes::of(model);
    if ax.across {
        return;
    }
    for e in &model.edges {
        let Some((note, target)) = notes::note_link(model, e) else {
            continue;
        };
        if geom::in_grid(model, note) || notes::room_beside(model, note, target, &elk.boxes) {
            continue;
        }
        let (Some(nb), Some(tb)) = (elk.boxes.get(note).copied(), elk.boxes.get(target).copied())
        else {
            continue;
        };
        // the card right after the target on its row, among its siblings
        let parent = model.node(target).and_then(|n| n.parent.as_deref());
        let row = |b: &Box2| b.y < tb.bottom() && tb.y < b.bottom();
        let next = model
            .children(parent)
            .filter(|n| n.id != note && n.id != target)
            .filter_map(|n| elk.boxes.get(&n.id).map(|b| (n.id.clone(), *b)))
            .filter(|(_, b)| row(b) && b.x >= tb.right() - 0.5)
            .min_by(|a, b| a.1.x.total_cmp(&b.1.x));
        let Some((next, nx)) = next else {
            continue;
        };
        let d = (nb.w + 2.0 * notes::GAP - (nx.x - tb.right())).ceil();
        if d < 0.5 {
            continue;
        }
        let mut frames = elk.boxes.clone();
        frames.extend(crate::layout::zone_frames(
            model, &elk.boxes, elk, pad, head,
        ));
        run(model, elk, Hints::Drop, |m, boxes| {
            make_room_right(m, &next, d, &frames, boxes);
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
    // the pass's own name (`…::centre::center_children`); an inline closure is named by its caller
    let name = std::any::type_name_of_val(&pass);
    let short = name
        .rsplit("::")
        .find(|s| !s.starts_with('{'))
        .unwrap_or(name);
    named(short, model, elk, hints, pass);
}

/// `run` for a pass given its name (`SKEMO_TRACE`, `SKEMO_OFF`).
fn named(
    name: &str,
    model: &Model,
    elk: &mut Placement,
    hints: Hints,
    pass: impl FnOnce(&Model, &mut Boxes),
) {
    if crate::trace::off(name) {
        return;
    }
    let before = elk.boxes.clone();
    pass(model, &mut elk.boxes);
    if !crate::trace::off("follow") {
        notes::follow(model, &before, &mut elk.boxes);
    }
    crate::trace::boxes(name, &before, &elk.boxes);
    drop_stale_routes(model, elk, &before, hints);
}

/// The zones whose cards moved since `before` get the frame `layout::zone_frames` gives them (the
/// owner of that rule; D-064's room for titles left to `title_room`): the passes read zone boxes
/// as ELK gave them, and `order_rows` laid a zone over its neighbour by reading the box of a zone
/// whose card `lift_leaves` had moved to another row. A zone none of whose cards moved keeps
/// ELK's box. (Done after every pass, it moved lines on diagrams with nothing hidden: refused.)
fn refresh_frames(model: &Model, elk: &mut Placement, before: &Boxes) {
    let Some((pad, head)) = elk.frame else {
        return;
    };
    let moved = |id: &str| before.get(id) != elk.boxes.get(id);
    let stale: Vec<String> = model
        .nodes
        .iter()
        .filter(|z| z.kind == skemo_lang::Kind::Zone)
        .filter(|z| {
            model.nodes.iter().any(|c| {
                c.kind == skemo_lang::Kind::Card
                    && geom::within(model, &c.id, &z.id)
                    && moved(&c.id)
            })
        })
        .map(|z| z.id.clone())
        .collect();
    if stale.is_empty() {
        return;
    }
    let plain = Placement {
        title_ends: std::collections::BTreeMap::new(),
        title_widths: std::collections::BTreeMap::new(),
        ..elk.clone()
    };
    let frames = crate::layout::zone_frames(model, &elk.boxes, &plain, pad, head);
    for z in stale {
        if let Some(f) = frames.get(&z) {
            elk.boxes.insert(z, *f);
        }
    }
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
