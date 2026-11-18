//! The grid engine (`diagram(layout="grid")`, D-106): a diagram read as a grid of zones and cards,
//! not as a graph. The arrows do not shape the drawing: they only choose, among the grids that
//! keep the zones together, the one with the shortest links.
//!
//! One pipeline, in this order. A rule has a name, and `SKEMO_OFF=name,…` switches it off: what
//! a rule earns is measured on the ledger (`quality`), and a rule that earns nothing goes.
//!
//! Before the placement (`diagram::build`):
//!
//! 1. `grid-dock`: a legend, a paragraph and a link outside the flow dock themselves around the
//!    drawing (`dock::dock_defaults`).
//!
//! Placement ([`place`]):
//!
//! 2. **Blocks** ([`blocks`]): a top-level zone, or a top-level card, is a block. In a zone of
//!    cards, the cards stand in the levels of its arrows, across or down (`grid-turn`).
//! 3. **Mosaic** ([`mosaic`]): the blocks in rows, as many rows as brings the whole nearest to
//!    the target proportions; each row is justified to the widest (`grid-justify`), and the
//!    first cards of a row of zones line up under the tallest title (`grid-heads`).
//! 4. **Order**: the block that starts the most arrows comes first (`grid-start`); then two
//!    blocks trade places (`grid-swap-blocks`), a block turns (`grid-turn`) and two cards of a
//!    level trade places (`grid-swap-cards`) as long as the links get shorter.
//! 5. **Room, made evenly** ([`room`], the manual method's §7): what a routing lacks, for its
//!    lines between rows (`grid-gutter-rows`) and between columns (`grid-gutter-columns`), for the
//!    tail behind a head (`grid-tail-room`) and for its labels (`grid-label-room`), widens the
//!    gutters of the whole group that owns the place, or a zone's padding; never one gap among
//!    the others, and the structure stays (`Sizes::spacing`, `mosaic::Ctx::gap`).
//! 6. `grid-align`: two linked cards one above the other a little offset line up, the content
//!    of a zone sliding as one in the room its row gives it ([`align`]).
//!
//! Routing ([`params`], and `route::sides`): no side is imposed and every side costs the same;
//! a line keeps out of the zones that hold neither of its ends (`grid-walls`), is routed again
//! once the others stand (`grid-reroute`), trades its lane with a line it crosses needlessly
//! (`grid-untangle`), and a bend costs more than the length it saves (`grid-bend`).
//!
//! Finishing ([`finish`], after the passes shared with the graph engine): `grid-straighten`,
//! `grid-tails`, `grid-squashed-z`.
//!
//! Labels ([`labelled`]): placed as in the graph engine (`labels::place`), then a line that runs
//! through another arrow's label moves to a lane at the side of the gap, and two labels lying over
//! one another on straight lines side by side part, their lines moving apart (`grid-label-lanes`).
//!
//! Pins (D-031) come after all of that, in `layout::final_boxes`: the grid is placed as if no
//! card were pinned, then a pinned card stands where its `at` says, with the size the grid gave
//! it, and the zones that hold it grow to it ([`frames`]). Pinning a card where it stands
//! therefore changes nothing, and a moved card moves only itself, its zones and its arrows.

mod align;
mod blocks;
mod finish;
mod lanes;
mod mosaic;
mod room;

pub(crate) use finish::finish;
pub(crate) use lanes::{Around, hidden, labelled};
pub(crate) use room::{label_room, line_room, merge};

use crate::layout::{Box2, Placement};
use crate::measure::Sizes;
use crate::scene::Scene;
use skemo_lang::{Kind, Model};
use std::collections::BTreeMap;

/// Is rule `name` on? (`SKEMO_OFF=name,…` switches rules off.)
pub(crate) fn on(name: &str) -> bool {
    !crate::trace::off(name)
}

/// Places the cards and the zones of `model` (no pin: D-031). `sizes.spacing`: the room the
/// routings before lacked, for their lines and their labels, by the gap that owns it.
pub(crate) fn place(
    model: &Model,
    scene: &Scene,
    sizes: &Sizes,
    (pad, head): (f64, f64),
) -> Placement {
    let mut placed = Placement {
        boxes: blocks::place(model, scene, sizes, (pad, head)),
        captions_below: sizes.captions_below.clone(),
        title_ends: sizes.title_ends.clone(),
        title_widths: sizes.title_widths.clone(),
        zone_heads: sizes.zone_heads.clone(),
        ..Placement::default()
    };
    if on("grid-align") {
        let titles = crate::labels::zone_titles(model, scene, &placed.boxes);
        let ctx = mosaic::Ctx {
            sizes,
            pad,
            head,
            room: true,
        };
        let side = |zone: &str| mosaic::padding(model, &ctx, zone).0;
        align::linked(model, &mut placed.boxes, &titles, &side);
    }
    placed
}

/// The zones' frames are the mosaic's (a zone takes the height of its row, the width its row
/// gives it), not the least frame around their cards: `boxes` gets them back from `placed`. A
/// card pinned out of its zone is still held (nothing hidden; D-031): the zone grows to it, and
/// the zones around that one with it.
pub(crate) fn frames(
    model: &Model,
    sizes: &Sizes,
    placed: &Placement,
    boxes: &mut BTreeMap<String, Box2>,
    (pad, head): (f64, f64),
) {
    for n in model.nodes.iter().filter(|n| n.kind == Kind::Zone) {
        if let Some(b) = placed.boxes.get(&n.id) {
            boxes.insert(n.id.clone(), *b);
        }
    }
    let ctx = mosaic::Ctx {
        sizes,
        pad,
        head,
        room: true,
    };
    let pinned = model
        .nodes
        .iter()
        .filter(|n| n.kind == Kind::Card && n.at.is_some());
    for card in pinned {
        let (Some(mut held), Some(free)) =
            (boxes.get(&card.id).copied(), placed.boxes.get(&card.id))
        else {
            continue;
        };
        // pinned where the grid sets it: its zone holds it already
        if (held.x - free.x).abs() < 0.5 && (held.y - free.y).abs() < 0.5 {
            continue;
        }
        for zone in model.ancestors(&card.id) {
            let (l, t, r, b) = mosaic::padding(model, &ctx, &zone.id);
            let Some(frame) = boxes.get_mut(&zone.id) else {
                break;
            };
            let x = frame.x.min((held.x - l).floor());
            let y = frame.y.min((held.y - t).floor());
            let right = frame.right().max((held.right() + r).ceil());
            let bottom = frame.bottom().max((held.bottom() + b).ceil());
            *frame = Box2 {
                x,
                y,
                w: right - x,
                h: bottom - y,
            };
            held = *frame;
        }
    }
}

/// The router's parameters: no flow (every side of a card costs the same), the zones' frames as
/// walls, every line routed again once the others stand, needless crossings traded away, and a
/// bend dearer than the length it saves (no staircase).
pub(crate) fn params(model: &Model) -> skemo_router::Params {
    let graph = crate::route::graph_params(model);
    skemo_router::Params {
        flow_penalty: 0.0,
        region_walls: on("grid-walls"),
        reroute: if on("grid-reroute") { 2 } else { 0 },
        untangle: on("grid-untangle"),
        segment_penalty: if on("grid-bend") {
            150.0
        } else {
            graph.segment_penalty
        },
        ..graph
    }
}

/// The cards of `model` that `boxes` places.
fn cards<'a>(model: &'a Model, boxes: &'a BTreeMap<String, Box2>) -> Vec<(&'a str, Box2)> {
    model
        .nodes
        .iter()
        .filter(|n| n.kind == Kind::Card)
        .filter_map(|n| boxes.get(&n.id).map(|b| (n.id.as_str(), *b)))
        .collect()
}

/// Does the orthogonal segment `p`–`q`, grown by `m`, cut box `b`?
fn cuts(p: (f64, f64), q: (f64, f64), b: &Box2, m: f64) -> bool {
    let (x0, x1) = (p.0.min(q.0), p.0.max(q.0));
    let (y0, y1) = (p.1.min(q.1), p.1.max(q.1));
    x0 < b.right() + m && b.x - m < x1 && y0 < b.bottom() + m && b.y - m < y1
}
