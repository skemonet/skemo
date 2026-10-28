//! Placement (F-LAYOUT-1 to 4): ELK layered with the prototype's options,
//! then pinned positions (`at=`) and zones computed again.

use crate::measure::Sizes;
use serde_json::{Value, json};
use skemo_lang::tr;
use skemo_lang::{Kind, Model};
use std::collections::BTreeMap;

/// Box `x, y, w, h` in file coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Box2 {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Box2 {
    pub fn right(&self) -> f64 {
        self.x + self.w
    }
    pub fn bottom(&self) -> f64 {
        self.y + self.h
    }
}

/// ELK's indicative route: absolute points and centre of the label.
pub type ElkRoute = (Vec<(f64, f64)>, Option<(f64, f64)>);

/// Result of the automatic placement.
#[derive(Debug, Clone, Default)]
pub struct Placement {
    pub boxes: BTreeMap<String, Box2>,
    /// ELK's indicative routes (absolute) and centre of the label.
    pub routes: BTreeMap<String, ElkRoute>,
    /// Zones whose title sits below their content (`Sizes::captions_below`, D-050).
    pub captions_below: std::collections::BTreeSet<String>,
    /// Right end of each zone's title from its left edge (`Sizes::title_ends`, D-064).
    pub title_ends: BTreeMap<String, f64>,
    /// Room of a zone's description lines under its title (`Sizes::zone_heads`, v0.8).
    pub zone_heads: BTreeMap<String, f64>,
}

/// Error of the placement engine (graph refused by ELK).
#[derive(Debug, Clone)]
pub struct LayoutError(pub String);

const DIRS: [(&str, &str); 4] = [
    ("down", "DOWN"),
    ("right", "RIGHT"),
    ("up", "UP"),
    ("left", "LEFT"),
];

/// Options of the root (prototype §5), plus `diagram(elk='{…}')`.
fn root_options(model: &Model) -> serde_json::Map<String, Value> {
    let dir = model
        .option("dir")
        .and_then(|d| DIRS.iter().find(|(k, _)| *k == d))
        .map_or("DOWN", |(_, v)| v);
    let mut o = serde_json::Map::new();
    for (k, v) in [
        ("elk.algorithm", "layered"),
        ("elk.direction", dir),
        ("elk.edgeRouting", "ORTHOGONAL"),
        ("elk.hierarchyHandling", "INCLUDE_CHILDREN"),
        // D-048 (R4): the rhythm of a hand drawing, rows 48 px apart (the prototype: 36 and 64)
        ("elk.spacing.nodeNode", "28"),
        ("elk.layered.spacing.nodeNodeBetweenLayers", "48"),
        ("elk.spacing.edgeNode", "22"),
        ("elk.spacing.edgeEdge", "14"),
        ("elk.layered.spacing.edgeNodeBetweenLayers", "22"),
        ("elk.edgeLabels.placement", "CENTER"),
        ("elk.layered.nodePlacement.strategy", "NETWORK_SIMPLEX"),
        ("elk.padding", "[top=24,left=24,bottom=24,right=24]"),
        ("elk.layered.considerModelOrder.strategy", "NODES_AND_EDGES"),
        (
            "elk.layered.crossingMinimization.forceNodeModelOrder",
            "true",
        ),
        // D-041: only real cycles are broken (writing order chooses the edge to reverse); the
        // prototype's MODEL_ORDER reversed every arrow written after its target
        ("elk.layered.cycleBreaking.strategy", "GREEDY_MODEL_ORDER"),
        // leaves go to the last row; `tidy` brings the leaves of a fork back to their
        // siblings' row (D-048, R1). LONGEST_PATH_SOURCE would rather lift every card with no
        // arrow in, and zones side by side would no longer share their top.
        ("elk.layered.layering.strategy", "LONGEST_PATH"),
        ("elk.layered.edgeLabels.sideSelection", "ALWAYS_DOWN"),
    ] {
        o.insert(k.into(), Value::String(v.into()));
    }
    if let Some(Value::Object(extra)) = model
        .option("elk")
        .and_then(|s| serde_json::from_str::<Value>(s).ok())
    {
        for (k, v) in extra {
            if !k.starts_with("skemo.") {
                o.insert(k, v);
            }
        }
    }
    o
}

/// `"skemo.labels": "elk"` in `diagram(elk=…)`: arrow labels are handed to ELK as inline labels
/// (the prototype's graph, for the parity test). Otherwise ELK does not see them: an inline label
/// takes a layer of its own and pushes its target one row down (D-048), and skemo places labels
/// itself after routing (`labels::place`); the rows are spaced to leave them room
/// (`layer_spacing`).
fn elk_labels(model: &Model) -> bool {
    model
        .option("elk")
        .and_then(|s| serde_json::from_str::<Value>(s).ok())
        .is_some_and(|v| v["skemo.labels"] == "elk")
}

/// Space between rows: 48 px (D-048, a hand drawing's rhythm), more when an arrow label must fit
/// between two rows: its height plus 14 px on each side across rows, its width plus 8 px on each
/// side across columns, where a label too wide to sit on a short arrow goes over it (D-049).
fn layer_spacing(model: &Model, sizes: &Sizes) -> f64 {
    let across = matches!(model.option("dir"), Some("right" | "left"));
    sizes
        .labels
        .values()
        .map(|s| if across { s.w + 16.0 } else { s.h + 28.0 })
        .fold(48.0, f64::max)
        .ceil()
}

/// A group that draws nothing (D-050): a stack, or a bare zone without a title.
pub fn invisible(n: &skemo_lang::Node) -> bool {
    n.stack || (n.title.is_empty() && n.classes.iter().any(|c| c == "bare"))
}

/// Room between the rows of a `wrap` zone, and between it and its sibling zones, when an arrow
/// must reach in (px): the router's margin on each side and a visible line (D-056).
const CORRIDOR: f64 = 48.0;

/// Does an arrow from outside `wrap` zone `id` enter a card beyond its first row? The arrow then
/// needs a corridor between the rows (and beside the zone): the zone's rows are spread like the
/// diagram's (instead of a compact grid) and its sibling zones keep `CORRIDOR` apart (D-056).
pub fn corridor(model: &Model, id: &str) -> bool {
    let Some(n) = model.node(id).filter(|n| n.kind == Kind::Zone) else {
        return false;
    };
    let Some(wrap) = n.wrap.map(|w| w as usize) else {
        return false;
    };
    let inside = |x: &str| x == id || model.ancestors(x).iter().any(|a| a.id == id);
    model
        .children(Some(id))
        .enumerate()
        .filter(|(i, _)| *i >= wrap)
        .any(|(_, c)| model.edges.iter().any(|e| e.to == c.id && !inside(&e.from)))
}

/// The rows of `wrap` zone `id` are spread (not a compact grid) when it, or a `wrap` zone beside
/// it, keeps a corridor: the rows of sibling zones stay aligned (D-056).
fn spread_rows(model: &Model, id: &str) -> bool {
    let parent = model.node(id).and_then(|n| n.parent.as_deref());
    corridor(model, id)
        || model
            .children(parent)
            .any(|s| s.wrap.is_some() && corridor(model, &s.id))
}

/// `"skemo.zoneSpacing": "elk"` in `diagram(elk=…)`: zones keep ELK's default spacing.
fn elk_zone_spacing(model: &Model) -> bool {
    model
        .option("elk")
        .and_then(|s| serde_json::from_str::<Value>(s).ok())
        .is_some_and(|v| v["skemo.zoneSpacing"] == "elk")
}

/// ELK graph as JSON (identical to the prototype's).
pub fn elk_graph(model: &Model, sizes: &Sizes, pad: f64, head: f64) -> Value {
    fn mk(model: &Model, id: &str, sizes: &Sizes, pads: &(String, String, (f64, f64))) -> Value {
        let Some(n) = model.node(id) else {
            // never reached (the children come from the model): an empty node rather than a panic
            return json!({ "id": id });
        };
        if n.kind == Kind::Zone {
            // a bare zone has its caption below its cards, not above; a stack or a bare zone
            // without a title keeps no room at all (D-050)
            let below = n.classes.iter().any(|c| c == "bare") || sizes.captions_below.contains(id);
            let own;
            let zpad = if invisible(n) {
                "[top=0,left=0,bottom=0,right=0]"
            } else if below && !elk_zone_spacing(model) {
                pads.1.as_str()
            } else if let Some(extra) = sizes.zone_heads.get(id) {
                // description lines under the title (v0.8): a taller top margin
                let (pad, head) = pads.2;
                own = format!(
                    "[top={},left={pad},bottom={pad},right={pad}]",
                    fmt_num(head + extra + pad * 0.6)
                );
                own.as_str()
            } else {
                pads.0.as_str()
            };
            let children: Vec<Value> = model
                .children(Some(id))
                .filter(|c| !crate::tidy::shelf::shelved(model, &c.id))
                .map(|c| mk(model, &c.id, sizes, pads))
                .collect();
            // an empty zone keeps its frame and title visible: ELK keeps room for them (with no
            // size it was a point, and the frame drawn there lay over the zone beside it)
            if children.is_empty() && !invisible(n) && model.children(Some(id)).next().is_none() {
                let (pad, head) = pads.2;
                let extra = sizes.zone_heads.get(id).copied().unwrap_or(0.0);
                let title = sizes.title_ends.get(id).map_or(0.0, |e| e + pad);
                return json!({
                    "id": id,
                    "width": (2.0 * pad).max(title),
                    "height": 2.0 * pad + head + extra,
                });
            }
            // the root's spacing between rows, which ELK does not hand down to a zone (its rows were
            // 20 px apart, too little for a bend and an arrowhead); cards side by side keep
            // ELK's tighter spacing inside a zone. Not for a `wrap` zone: its rows are a grid
            // (invisible edges), meant to stay compact
            // `diagram(elk='{"skemo.zoneSpacing": "elk"}')`: ELK's own spacing (the prototype's,
            // for the parity test against its oracle)
            if elk_zone_spacing(model) || (n.wrap.is_some() && !spread_rows(model, id)) {
                return json!({ "id": id, "children": children, "layoutOptions": { "elk.padding": zpad } });
            }
            let mut opts = json!({
                "elk.padding": zpad,
                "elk.layered.spacing.nodeNodeBetweenLayers": fmt_num(layer_spacing(model, sizes)),
                "elk.layered.spacing.edgeNodeBetweenLayers": "22",
            });
            // zones side by side that an arrow runs between keep a corridor apart (D-056)
            if model.children(Some(id)).any(|c| corridor(model, &c.id)) {
                opts["elk.spacing.nodeNode"] = json!(fmt_num(CORRIDOR));
            }
            json!({ "id": id, "children": children, "layoutOptions": opts })
        } else {
            let s = sizes
                .cards
                .get(id)
                .copied()
                .unwrap_or(crate::measure::Size { w: 170.0, h: 48.0 });
            json!({ "id": id, "width": s.w, "height": s.h })
        }
    }
    let pads = (
        format!(
            "[top={},left={pad},bottom={pad},right={pad}]",
            fmt_num(head + pad * 0.6)
        ),
        format!(
            "[top={},left={pad},bottom={},right={pad}]",
            fmt_num(pad * 0.6),
            fmt_num(head + pad * 0.6)
        ),
        (pad, head),
    );
    let children: Vec<Value> = model
        .children(None)
        .filter(|c| !crate::tidy::shelf::shelved(model, &c.id))
        .map(|c| mk(model, &c.id, sizes, &pads))
        .collect();
    // `constraint="false"`: routed like any arrow, but left out of the layout
    let edges: Vec<Value> = model
        .edges
        .iter()
        .filter(|e| e.constraint)
        .map(|e| {
            let labels: Vec<Value> = match sizes.labels.get(&e.id).filter(|_| elk_labels(model)) {
                Some(s) => vec![json!({ "text": e.label, "width": s.w, "height": s.h, "layoutOptions": { "elk.edgeLabels.inline": "true" } })],
                None => vec![],
            };
            json!({ "id": e.id, "sources": [e.from], "targets": [e.to], "labels": labels })
        })
        .collect();
    let mut edges = edges;
    edges.extend(wrap_edges(model));
    let mut options = root_options(model);
    if !elk_labels(model) {
        options
            .entry("elk.layered.spacing.nodeNodeBetweenLayers")
            .and_modify(|v| {
                if v == "48" {
                    *v = Value::String(fmt_num(layer_spacing(model, sizes)));
                }
            });
    }
    json!({ "id": "root", "layoutOptions": options, "children": children, "edges": edges })
}

/// Prefix of the invisible ELK edges that make `wrap="N"` (never routed nor drawn).
pub const WRAP_EDGE: &str = "__wrap:";

/// `wrap="N"` on a zone: an invisible edge from child `i` to child `i + N` puts each group of N
/// children on its own layer, so rows of N (columns with `dir` right or left), in writing order.
fn wrap_edges(model: &Model) -> Vec<Value> {
    let mut out = Vec::new();
    for z in model.nodes.iter().filter(|n| n.kind == Kind::Zone) {
        let Some(n) = z.wrap.map(|w| w as usize) else {
            continue;
        };
        let kids: Vec<&str> = model
            .children(Some(&z.id))
            .filter(|c| !crate::tidy::shelf::shelved(model, &c.id))
            .map(|c| c.id.as_str())
            .collect();
        for i in 0..kids.len().saturating_sub(n) {
            out.push(json!({
                "id": format!("{WRAP_EDGE}{}:{i}", z.id),
                "sources": [kids[i]],
                "targets": [kids[i + n]],
            }));
        }
    }
    out
}

/// Number as JavaScript would write it in a string (`43.2`, `24`).
pub fn fmt_num(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

/// Adds the absolute boxes of the descendants of `n` (origin `(ox, oy)`).
fn walk(n: &Value, ox: f64, oy: f64, boxes: &mut BTreeMap<String, Box2>) {
    for c in n["children"].as_array().into_iter().flatten() {
        let x = ox + c["x"].as_f64().unwrap_or(0.0);
        let y = oy + c["y"].as_f64().unwrap_or(0.0);
        if let Some(id) = c["id"].as_str() {
            boxes.insert(
                id.to_string(),
                Box2 {
                    x,
                    y,
                    w: c["width"].as_f64().unwrap_or(0.0),
                    h: c["height"].as_f64().unwrap_or(0.0),
                },
            );
        }
        walk(c, x, y, boxes);
    }
}
/// Gathers the arrows of `n` and all its descendants.
fn edges<'a>(n: &'a Value, acc: &mut Vec<&'a Value>) {
    acc.extend(n["edges"].as_array().into_iter().flatten());
    for c in n["children"].as_array().into_iter().flatten() {
        edges(c, acc);
    }
}
/// Reading of ELK's result: absolute boxes, absolute routes (frame of the arrow's container).
pub fn read_elk(out: &Value) -> Placement {
    let mut p = Placement::default();
    walk(out, 0.0, 0.0, &mut p.boxes);
    let mut all = Vec::new();
    edges(out, &mut all);
    for e in all {
        let Some(id) = e["id"].as_str() else { continue };
        if id.starts_with(WRAP_EDGE) {
            continue;
        }
        let off = match e["container"].as_str() {
            Some(c) if c != "root" => p.boxes.get(c).map_or((0.0, 0.0), |b| (b.x, b.y)),
            _ => (0.0, 0.0),
        };
        let Some(s) = e["sections"].get(0) else {
            continue;
        };
        let mut pts = vec![&s["startPoint"]];
        pts.extend(s["bendPoints"].as_array().into_iter().flatten());
        pts.push(&s["endPoint"]);
        let pts: Vec<(f64, f64)> = pts
            .iter()
            .map(|q| {
                (
                    q["x"].as_f64().unwrap_or(0.0) + off.0,
                    q["y"].as_f64().unwrap_or(0.0) + off.1,
                )
            })
            .collect();
        let label = e["labels"].get(0).and_then(|l| {
            Some((
                l["x"].as_f64()? + off.0 + l["width"].as_f64()? / 2.0,
                l["y"].as_f64()? + off.1 + l["height"].as_f64()? / 2.0,
            ))
        });
        p.routes.insert(id.to_string(), (pts, label));
    }
    p
}

/// Placement automatique par ELK.
pub fn elk_layout(
    model: &Model,
    sizes: &Sizes,
    pad: f64,
    head: f64,
) -> Result<Placement, LayoutError> {
    if model.nodes.is_empty() {
        return Ok(Placement::default());
    }
    let graph = elk_graph(model, sizes, pad, head).to_string();
    let run = std::panic::catch_unwind(|| {
        org_eclipse_elk_graph_json::org::eclipse::elk::graph::json::layout_api::layout_json(
            &graph, "{}",
        )
    });
    let out = match run {
        Ok(Ok(s)) => s,
        Ok(Err(e)) => {
            return Err(LayoutError(tr!(
                "ELK rejected the graph: {e}",
                "ELK a refusé le graphe : {e}"
            )));
        }
        Err(_) => {
            return Err(LayoutError(tr!(
                "ELK failed (internal panic)",
                "ELK a échoué (panique interne)"
            )));
        }
    };
    let v: Value = serde_json::from_str(&out)
        .map_err(|e| LayoutError(format!("sortie ELK illisible : {e}")))?;
    let mut p = read_elk(&v);
    // ELK does not return the arrows it cannot draw: they will be routed anyway
    p.boxes.retain(|k, _| model.node(k).is_some());
    p.captions_below.clone_from(&sizes.captions_below);
    p.title_ends.clone_from(&sizes.title_ends);
    p.zone_heads.clone_from(&sizes.zone_heads);
    Ok(p)
}

/// Final boxes: ELK, then pinned positions and overlap removal. A single chain, with or without
/// `at=` (D-031): pinning a card where it is changes nothing.
pub fn final_boxes(
    model: &Model,
    sizes: &Sizes,
    base: &Placement,
    pad: f64,
    head: f64,
) -> BTreeMap<String, Box2> {
    final_boxes_report(model, sizes, base, pad, head).0
}

/// Frames of the zones, **one rule** (D-031, end of limit L6), with or without pinned cards:
/// imposed size (`at=` and `size=`) as is; otherwise bounding box of the children + the margins
/// given to ELK (`pad` on the sides and at the bottom, `head + pad × 0.6` at the top). In automatic
/// placement, it is exactly the frame computed by ELK; a zone therefore only changes if one of its
/// children moves. Empty zone: its ELK frame.
pub fn zone_frames(
    model: &Model,
    boxes: &BTreeMap<String, Box2>,
    base: &Placement,
    pad: f64,
    head: f64,
) -> BTreeMap<String, Box2> {
    /// Common inputs: boxes of the cards, ELK's placement, (pad, head).
    type Ctx<'a> = (&'a BTreeMap<String, Box2>, &'a Placement, (f64, f64));
    fn frame(
        model: &Model,
        id: &str,
        ctx: Ctx,
        out: &mut BTreeMap<String, Box2>,
        depth: usize,
    ) -> Option<Box2> {
        let (boxes, base, (pad, head)) = ctx;
        if let Some(b) = out.get(id) {
            return Some(*b);
        }
        let z = model.node(id)?;
        if z.kind != Kind::Zone {
            return boxes.get(id).copied();
        }
        if let (Some([x, y]), Some([w, h])) = (z.at, z.size) {
            let b = Box2 { x, y, w, h };
            out.insert(id.to_string(), b);
            return Some(b);
        }
        if depth > 64 {
            return None;
        }
        let kids: Vec<Box2> = model
            .children(Some(id))
            .filter_map(|c| frame(model, &c.id, ctx, out, depth + 1))
            .collect();
        let b = if kids.is_empty() {
            // zone without a card: ELK's frame, or a minimal size if it is empty (an empty zone stays
            // visible with its title)
            let (w, h) = (2.0 * pad, 2.0 * pad + head);
            match base.boxes.get(id).copied() {
                Some(b) if b.w > 0.0 && b.h > 0.0 => b,
                Some(b) => Box2 {
                    x: b.x,
                    y: b.y,
                    w,
                    h,
                },
                None => Box2 {
                    x: 0.0,
                    y: 0.0,
                    w,
                    h,
                },
            }
        } else {
            // a bare zone keeps the room of its caption below its cards; an invisible group none
            let caption_below =
                z.classes.iter().any(|c| c == "bare") || base.captions_below.contains(id);
            let (above, below, pad) = if invisible(z) {
                (0.0, 0.0, 0.0)
            } else if caption_below && !elk_zone_spacing(model) {
                (pad * 0.6, head + pad * 0.6, pad)
            } else {
                let extra = base.zone_heads.get(id).copied().unwrap_or(0.0);
                (head + extra + pad * 0.6, pad, pad)
            };
            let x1 = kids.iter().map(|b| b.x).fold(f64::INFINITY, f64::min) - pad;
            let x1 = base
                .title_ends
                .get(id)
                .filter(|_| !caption_below)
                .map_or(x1, |end| {
                    x1.min((title_clear_left(model, id, boxes) - end).floor())
                });
            let y1 = kids.iter().map(|b| b.y).fold(f64::INFINITY, f64::min) - above;
            let x2 = kids
                .iter()
                .map(Box2::right)
                .fold(f64::NEG_INFINITY, f64::max)
                + pad;
            let y2 = kids
                .iter()
                .map(Box2::bottom)
                .fold(f64::NEG_INFINITY, f64::max)
                + below;
            Box2 {
                x: x1,
                y: y1,
                w: x2 - x1,
                h: y2 - y1,
            }
        };
        out.insert(id.to_string(), b);
        Some(b)
    }
    let mut out = BTreeMap::new();
    for z in model.nodes.iter().filter(|n| n.kind == Kind::Zone) {
        frame(model, &z.id, (boxes, base, (pad, head)), &mut out, 0);
    }
    crate::tidy::stretch_zones(model, &mut out, boxes);
    out
}

/// Room between a line and a zone's title.
const TITLE_MARGIN: f64 = 10.0;
/// D-064 (the user's choice B): a card of a zone's first row linked to something above it keeps
/// the middle of its top clear of the zone's title, so that its arrow (or the trunk of its fork)
/// comes straight in (titles are walls for the lines). The zone's left edge must then be at
/// most the returned value minus the title's end: the zone widens to the left rather than the
/// title moving.
pub(crate) fn title_clear_left(model: &Model, id: &str, boxes: &BTreeMap<String, Box2>) -> f64 {
    let cards: Vec<(&str, Box2)> = model
        .children(Some(id))
        .filter(|n| n.kind == Kind::Card)
        .filter_map(|n| boxes.get(&n.id).map(|b| (n.id.as_str(), *b)))
        .collect();
    let top = cards.iter().map(|(_, b)| b.y).fold(f64::INFINITY, f64::min);
    let above = |c: &str, b: &Box2| {
        model.edges.iter().any(|e| {
            let other = if e.from == c {
                &e.to
            } else if e.to == c {
                &e.from
            } else {
                return false;
            };
            // above, and not wholly to the left (that one comes in by the left side)
            boxes
                .get(other)
                .is_some_and(|o| o.bottom() <= b.y && o.right() > b.x)
        })
    };
    cards
        .iter()
        .filter(|(c, b)| b.y < top + 1.0 && above(c, b))
        // its middle: where a straight line or a fork's trunk leaves
        .map(|(_, b)| b.x + b.w / 2.0 - TITLE_MARGIN)
        .fold(f64::INFINITY, f64::min)
}

/// Minimal gap left between a card pushed aside and the one it overlapped.
const SEPARATION: f64 = 12.0;

fn overlap(a: &Box2, b: &Box2, gap: f64) -> bool {
    a.x < b.right() + gap
        && b.x < a.right() + gap
        && a.y < b.bottom() + gap
        && b.y < a.bottom() + gap
}

/// Minimal move of `m` to no longer overlap `fixed` (gap `gap`): the shortest of the four
/// directions; on a tie, down then right.
fn push_out(m: &Box2, fixed: &Box2, gap: f64) -> (f64, f64) {
    let cands = [
        (0.0, fixed.bottom() + gap - m.y),
        (fixed.right() + gap - m.x, 0.0),
        (0.0, fixed.y - gap - m.bottom()),
        (fixed.x - gap - m.right(), 0.0),
    ];
    cands
        .into_iter()
        .min_by(|a, b| (a.0.abs() + a.1.abs()).total_cmp(&(b.0.abs() + b.1.abs())))
        .unwrap_or((0.0, 0.0))
}

/// Is node `id` inside zone `zone` (at any depth)?
fn inside(model: &Model, id: &str, zone: &str) -> bool {
    model.ancestors(id).iter().any(|a| a.id == zone)
}

/// Cards (descendants) of a zone.
fn cards_in<'a>(model: &'a Model, zone: &str) -> Vec<&'a skemo_lang::Node> {
    model
        .nodes
        .iter()
        .filter(|n| n.kind == Kind::Card && inside(model, &n.id, zone))
        .collect()
}

/// What a free zone must not cover: the pinned cards outside it, and the frames of the zones
/// holding a pinned card that neither contain it nor lie inside it.
fn zone_obstacles(
    model: &Model,
    boxes: &BTreeMap<String, Box2>,
    (base, pad, head): (&Placement, f64, f64),
    zone: &str,
    pinned: &[String],
) -> Vec<(String, Box2)> {
    let mut out: Vec<(String, Box2)> = pinned
        .iter()
        .filter(|p| !inside(model, p, zone))
        .filter_map(|p| boxes.get(p).map(|b| (p.clone(), *b)))
        .collect();
    for y in model
        .nodes
        .iter()
        .filter(|n| n.kind == Kind::Zone && n.id != zone)
    {
        let related = inside(model, &y.id, zone) || inside(model, zone, &y.id);
        if related || !cards_in(model, &y.id).iter().any(|c| c.at.is_some()) {
            continue;
        }
        if let Some(yb) = zone_frames(model, boxes, base, pad, head).get(&y.id) {
            out.push((y.id.clone(), *yb));
        }
    }
    out
}

/// Separation proposal (editor, ED-10): the elements that would have to move so that no free card
/// overlaps a pinned card any more, with the pinned card in their way. Computed on a copy: `boxes`
/// is not changed. See [`separate`] for the two levels.
pub fn suggest_separation(
    model: &Model,
    boxes: &BTreeMap<String, Box2>,
    base: &Placement,
    pad: f64,
    head: f64,
) -> Vec<(String, String)> {
    let mut trial = boxes.clone();
    separate(model, &mut trial, base, pad, head)
}

/// Overlap removal (F-LAYOUT-5), in two levels; pinned cards never move.
/// 1. A zone whose frame would cover a pinned card that does not belong to it is shifted as a
///    block (its free cards keep their arrangement);
/// 2. within one zone, a free card that overlaps a pinned card is pushed aside, and the free cards
///    it bumps are too, in turn.
///
/// Returns the elements moved (id, card in the way).
pub fn separate(
    model: &Model,
    boxes: &mut BTreeMap<String, Box2>,
    base: &Placement,
    pad: f64,
    head: f64,
) -> Vec<(String, String)> {
    let pinned: Vec<String> = model
        .nodes
        .iter()
        .filter(|n| n.kind == Kind::Card && n.at.is_some())
        .map(|n| n.id.clone())
        .collect();
    if pinned.is_empty() {
        return Vec::new();
    }
    let mut moved: Vec<(String, String)> = Vec::new();
    separate_zones(model, boxes, base, (pad, head), &pinned, &mut moved);
    separate_cards(model, boxes, &pinned, &mut moved);
    moved
}

/// First pass, on the zones: a free zone whose frame would cover a pinned card that does not
/// belong to it is shifted as a block.
fn separate_zones(
    model: &Model,
    boxes: &mut BTreeMap<String, Box2>,
    base: &Placement,
    (pad, head): (f64, f64),
    pinned: &[String],
    moved: &mut Vec<(String, String)>,
) {
    let zones: Vec<&skemo_lang::Node> = model
        .nodes
        .iter()
        .filter(|n| n.kind == Kind::Zone && n.at.is_none())
        .collect();
    for _round in 0..8 {
        let mut changed = false;
        for z in &zones {
            let members = cards_in(model, &z.id);
            if members.is_empty() || members.iter().any(|c| c.at.is_some()) {
                continue; // a zone that holds a pinned card does not move as a block
            }
            for (p, pb) in zone_obstacles(model, boxes, (base, pad, head), &z.id, pinned) {
                let Some(zb) = zone_frames(model, boxes, base, pad, head)
                    .get(&z.id)
                    .copied()
                else {
                    continue;
                };
                if overlap(&zb, &pb, 0.0) {
                    let (dx, dy) = push_out(&zb, &pb, SEPARATION);
                    for c in &members {
                        if let Some(b) = boxes.get_mut(&c.id) {
                            b.x += dx;
                            b.y += dy;
                        }
                    }
                    if !moved.iter().any(|(m, _)| m == &z.id) {
                        moved.push((z.id.clone(), p.clone()));
                    }
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
}

/// Second pass, on the cards of one zone (or outside zones): a free card that overlaps a pinned
/// card is pushed aside, and the free cards it bumps are too, in turn.
fn separate_cards(
    model: &Model,
    boxes: &mut BTreeMap<String, Box2>,
    pinned: &[String],
    moved: &mut Vec<(String, String)>,
) {
    let free: Vec<String> = model
        .nodes
        .iter()
        .filter(|n| n.kind == Kind::Card && n.at.is_none())
        .map(|n| n.id.clone())
        .collect();
    let mut queue: Vec<String> = pinned.to_vec();
    let mut guard = 0;
    while !queue.is_empty() && guard < 10_000 {
        let src = queue.remove(0);
        guard += 1;
        let Some(sb) = boxes.get(&src).copied() else {
            continue;
        };
        for f in &free {
            if *f == src {
                continue;
            }
            let Some(fb) = boxes.get(f).copied() else {
                continue;
            };
            if overlap(&fb, &sb, 0.0) {
                let (dx, dy) = push_out(&fb, &sb, SEPARATION);
                boxes.insert(
                    f.clone(),
                    Box2 {
                        x: fb.x + dx,
                        y: fb.y + dy,
                        ..fb
                    },
                );
                if !moved.iter().any(|(m, _)| m == f) {
                    moved.push((f.clone(), src.clone()));
                }
                queue.push(f.clone());
            }
        }
    }
}

/// Like [`final_boxes`], with the elements that overlap a pinned card: `(element, pinned card in
/// the way)`. Nothing is moved (Q7, D-033). Pinned cards (`at=`) take their place (POC
/// `applyPins`), then zones follow the single rule of [`zone_frames`].
pub fn final_boxes_report(
    model: &Model,
    sizes: &Sizes,
    base: &Placement,
    pad: f64,
    head: f64,
) -> (BTreeMap<String, Box2>, Vec<(String, String)>) {
    let mut boxes = BTreeMap::new();
    for n in model.nodes.iter().filter(|n| n.kind == Kind::Card) {
        let s = sizes
            .cards
            .get(&n.id)
            .copied()
            .unwrap_or(crate::measure::Size { w: 170.0, h: 48.0 });
        let b = match n.at {
            Some([x, y]) => Box2 {
                x,
                y,
                w: s.w,
                h: s.h,
            },
            None => base.boxes.get(&n.id).copied().unwrap_or(Box2 {
                x: 0.0,
                y: 0.0,
                w: s.w,
                h: s.h,
            }),
        };
        boxes.insert(n.id.clone(), b);
    }
    // Q7 (D-033): overlaps are reported, never corrected; the separation computed on a copy serves to
    // name them and stays available as a proposal (`suggest_separation`).
    let moved = suggest_separation(model, &boxes, base, pad, head);
    let zones = zone_frames(model, &boxes, base, pad, head);
    boxes.extend(zones);
    (boxes, moved)
}
