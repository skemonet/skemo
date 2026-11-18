//! Routing (E-ROUTE-1, 2): every arrow goes through the single router (D-010).
//!
//! Translation of the scene: cards → `Shape`, zones → `Region`, `pass_through` = ancestor zones of
//! the start and the arrival, `side` and `via` from the model, ELK routes as hints (`hint`).

use crate::layout::{Box2, Placement};
use skemo_lang::Model;
use skemo_lang::tr;
use skemo_router::{Connector, Diagnostic, Params, Side};
use std::collections::BTreeMap;

mod clean;
mod groups;
mod scene;
mod sides;

use clean::{center_in_gaps, drop_micro_jogs, flatten_steps, straighten_jogs};
pub use scene::router_scene;
pub use sides::fan_in;

pub type Points = Vec<(f64, f64)>;

fn side(s: &str) -> Option<Side> {
    match s {
        "top" => Some(Side::Top),
        "bottom" => Some(Side::Bottom),
        "left" => Some(Side::Left),
        "right" => Some(Side::Right),
        _ => None,
    }
}

/// `side="start-end"` → imposed sides (one half may be empty).
pub fn sides(s: Option<&str>) -> (Option<Side>, Option<Side>) {
    match s {
        None => (None, None),
        Some(v) => match v.split_once('-') {
            Some((a, b)) => (side(a), side(b)),
            None => (side(v), None),
        },
    }
}

/// Routing mode according to the placement mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Diagram: the zones that hold neither the start nor the arrival are obstacles.
    Graph,
    /// Grid (`grid`): as [`Mode::Graph`], but no side is chosen for an arrow (the router takes
    /// the cheapest), and the grid's own rules finish the routes.
    Grid,
    /// Timeline: crossable lanes, default sides `right-left` within one lane, start `right` towards
    /// another lane (F-TL-5).
    Timeline,
    /// Frieze: no zone obstacle (F-FR-5).
    Frise,
    /// Mindmap: sides facing outward and towards the centre, trunk shared by the children on one side.
    Mindmap,
}

impl Mode {
    /// One of the two automatic engines (graph, grid): zones are obstacles, zone titles too,
    /// and the finishing passes run.
    pub fn automatic(self) -> bool {
        matches!(self, Mode::Graph | Mode::Grid)
    }
}

/// Router parameters for a diagram (the prototype's defaults).
pub fn params() -> Params {
    Params::default()
}

/// Parameters of the graph mode: arrows leave and enter in the direction of the drawing
/// (`dir`), unless another side is clearly cheaper (a side costs about three bends).
pub fn graph_params(model: &Model) -> Params {
    use skemo_router::Dir;
    let flow = match model.option("dir") {
        Some("up") => Dir::Up,
        Some("right") => Dir::Right,
        Some("left") => Dir::Left,
        _ => Dir::Down,
    };
    Params {
        flow: Some(flow),
        flow_penalty: 150.0,
        port_spread: 42.0,
        // parallel lines a fixed gap apart where the corridor has room (D-063)
        ideal_nudging: crate::breathe::LINE_GAP,
        // a line between two zones stays between them when it is centred (the manual method's
        // §7: a zone holding neither end is no place for a line)
        region_walls: !crate::trace::off("graph-walls"),
        // the branches of a fork leave its trunk as late as their way allows
        late_split: !crate::trace::off("late-split"),
        ..params()
    }
}

/// The boxes the arrows see (D-087): a node with an icon takes its arrows at the icon's middle.
/// Drawn down or up, the node is its icon and its text, beside it, joins the zone titles as an
/// obstacle; drawn right or left, it is a band at the icon's height from the icon to the end of
/// the text, so that an arrow leaving on the text's side starts after the text. Other boxes are
/// kept.
pub fn anchors(
    model: &Model,
    scene: &crate::scene::Scene,
    boxes: &BTreeMap<String, Box2>,
    titles: &[(String, Box2)],
) -> (BTreeMap<String, Box2>, Vec<(String, Box2)>) {
    let mut out = boxes.clone();
    let mut obstacles = titles.to_vec();
    let across = matches!(model.option("dir"), Some("right" | "left"));
    for n in model.nodes.iter().filter(|n| n.node) {
        let (Some(card), Some(b)) = (scene.cards.get(&n.id), boxes.get(&n.id)) else {
            continue;
        };
        let cs = scene.style(card.el);
        let (pad, bor) = (cs.padding(), crate::measure::border(cs));
        let content_h = b.h - pad[0] - pad[2] - bor[0] - bor[2];
        let Some((size, column)) = card.node_icon(scene) else {
            continue;
        };
        let x = b.x + bor[3] + pad[3];
        let icon = Box2 {
            x,
            y: b.y + bor[0] + pad[0] + (content_h - size) / 2.0,
            w: size,
            h: size,
        };
        if across {
            out.insert(
                n.id.clone(),
                Box2 {
                    w: b.right() - x,
                    ..icon
                },
            );
            continue;
        }
        let text = Box2 {
            x: x + column,
            y: b.y,
            w: b.right() - x - column,
            h: b.h,
        };
        out.insert(n.id.clone(), icon);
        obstacles.push((n.id.clone(), text));
    }
    (out, obstacles)
}

/// Routes every arrow. Returns the routes, the warnings and the group of each grouped arrow
/// (trunk, fork, bus: `junctions`).
pub fn route_all(
    model: &Model,
    boxes: &BTreeMap<String, Box2>,
    elk: &Placement,
    params: &Params,
    mode: Mode,
    soft: &[(String, Box2)],
    titles: &[(String, Box2)],
) -> (
    BTreeMap<String, Points>,
    Vec<crate::EngineWarning>,
    BTreeMap<String, String>,
) {
    let (obstacles, conns) = router_scene(model, boxes, elk, mode, soft, titles);
    let groups = groups_of(&conns);
    let (routes, diags) = skemo_router::route(&obstacles, &conns, params);
    let raw: BTreeMap<String, Points> = routes.into_iter().map(|r| (r.id, r.points)).collect();
    crate::trace::routes("router", &raw);
    let out = finish(model, boxes, titles, &groups, &raw, mode);
    (out, warnings_of(diags), groups)
}

/// The group of each grouped arrow (trunk, fork, bus).
pub fn groups_of(conns: &[Connector]) -> BTreeMap<String, String> {
    conns
        .iter()
        .filter_map(|c| c.group.clone().map(|g| (c.id.clone(), g)))
        .collect()
}

/// Small steps straightened by sliding an end off the middle of its side (`straighten_jogs`):
/// off for now (user, 2026-11-06: every line lands in the middle of a side, a step is the
/// lesser evil).
const STRAIGHTEN_JOGS: bool = false;

/// The router's routes as drawn: micro jogs dropped, cross segments centred in their gap,
/// small jogs straightened when `STRAIGHTEN_JOGS` (D-049), then the grid's own rules. Shared by
/// the full computation and the editing session, so that a drag preview is the drawing a reload
/// gives.
pub fn finish(
    model: &Model,
    boxes: &BTreeMap<String, Box2>,
    titles: &[(String, Box2)],
    groups: &BTreeMap<String, String>,
    raw: &BTreeMap<String, Points>,
    mode: Mode,
) -> BTreeMap<String, Points> {
    let turned: Vec<&str> = model
        .edges
        .iter()
        // joins, named or automatic, were routed from their target
        .filter(|e| groups.get(&e.id).is_some_and(|g| groups::turned(g)))
        .map(|e| e.id.as_str())
        .collect();
    let on = |pass: &str| !crate::trace::off(pass);
    let mut out: BTreeMap<String, Points> = raw
        .iter()
        .map(|(id, p)| {
            let mut p = p.clone();
            if turned.contains(&id.as_str()) {
                p.reverse();
            }
            if on("micro-jogs") {
                p = drop_micro_jogs(p);
            }
            (id.clone(), p)
        })
        .collect();
    crate::trace::routes("micro-jogs", &out);
    if mode.automatic() && on("center-gaps") {
        center_in_gaps(model, boxes, titles, groups, &mut out);
        crate::trace::routes("center-gaps", &out);
    }

    if STRAIGHTEN_JOGS {
        straighten_jogs(model, boxes, groups, &mut out);
    }
    if mode.automatic() {
        if on("breathe") {
            crate::breathe::breathe(model, boxes, titles, groups, &mut out);
            crate::trace::routes("breathe", &out);
        }
        if on("flatten") {
            flatten_steps(model, boxes, titles, groups, &mut out);
            crate::trace::routes("flatten", &out);
        }
        if on("plug") {
            crate::stack::plug(model, boxes, &mut out);
            crate::trace::routes("plug", &out);
        }
    }
    if mode == Mode::Grid {
        crate::grid::finish(model, boxes, titles, &mut out);
    }
    out
}

/// The junctions an arrow passes through, moved by `(ox, oy)` (the rendering frame).
pub fn junctions_on(
    d_routes: &BTreeMap<String, Points>,
    groups: &BTreeMap<String, String>,
    id: &str,
    (ox, oy): (f64, f64),
) -> Vec<(f64, f64)> {
    let Some(r) = d_routes.get(id) else {
        return Vec::new();
    };
    let on = |p: (f64, f64)| {
        r.windows(2).any(|w| {
            let (a, b) = (w[0], w[1]);
            let within = |v: f64, s: f64, t: f64| v >= s.min(t) - 0.5 && v <= s.max(t) + 0.5;
            within(p.0, a.0, b.0) && within(p.1, a.1, b.1)
        })
    };
    if !groups.contains_key(id) {
        return Vec::new();
    }
    junctions(d_routes, groups)
        .into_iter()
        .filter(|p| on(*p))
        .map(|p| (p.0 + ox, p.1 + oy))
        .collect()
}

/// A segment of a route: its two ends.
type Segment = ((f64, f64), (f64, f64));

/// Junctions of grouped arrows (D-047): points where at least three directions meet on the
/// routes of one group (a trunk splitting in two, a bus taking in a branch). Drawn as a sharp
/// corner with a dot, where each route would otherwise round its own bend.
pub fn junctions(
    routes: &BTreeMap<String, Points>,
    groups: &BTreeMap<String, String>,
) -> Vec<(f64, f64)> {
    let mut by_group: BTreeMap<&str, Vec<&Points>> = BTreeMap::new();
    for (id, g) in groups {
        if let Some(r) = routes.get(id) {
            by_group.entry(g.as_str()).or_default().push(r);
        }
    }
    let near = |a: f64, b: f64| (a - b).abs() < 0.5;
    let mut out: Vec<(f64, f64)> = Vec::new();
    for rs in by_group.values().filter(|rs| rs.len() > 1) {
        let segs: Vec<Segment> = rs
            .iter()
            .flat_map(|r| r.windows(2).map(|w| (w[0], w[1])))
            .collect();
        for p in rs.iter().flat_map(|r| r.iter().copied()) {
            let ends_here = rs
                .iter()
                .any(|r| r.first() == Some(&p) || r.last() == Some(&p));
            if directions_at(p, &segs).iter().filter(|d| **d).count() >= 3
                && !ends_here
                && !out.iter().any(|q| near(q.0, p.0) && near(q.1, p.1))
            {
                out.push(p);
            }
        }
    }
    out
}

/// The directions leaving `p` along the segments through it (within 0.5 px): up, down, left,
/// right.
fn directions_at(p: (f64, f64), segs: &[Segment]) -> [bool; 4] {
    let near = |a: f64, b: f64| (a - b).abs() < 0.5;
    let mut dirs = [false; 4];
    for &(a, b) in segs {
        let (vertical, horizontal) = (near(a.0, b.0), near(a.1, b.1));
        let on = if vertical && near(p.0, a.0) {
            p.1 >= a.1.min(b.1) - 0.5 && p.1 <= a.1.max(b.1) + 0.5
        } else if horizontal && near(p.1, a.1) {
            p.0 >= a.0.min(b.0) - 0.5 && p.0 <= a.0.max(b.0) + 0.5
        } else {
            false
        };
        if !on {
            continue;
        }
        // towards either end of the segment
        for to in [b, a] {
            if vertical && (to.1 - p.1).abs() > 0.5 {
                dirs[usize::from(to.1 > p.1)] = true;
            } else if horizontal && (to.0 - p.0).abs() > 0.5 {
                dirs[2 + usize::from(to.0 > p.0)] = true;
            }
        }
    }
    dirs
}

/// Warnings (in the current language) of the router's diagnostics.
pub fn warnings_of(diags: Vec<Diagnostic>) -> Vec<crate::EngineWarning> {
    diags.into_iter().map(warning_of).collect()
}

fn warning_of(d: Diagnostic) -> crate::EngineWarning {
    use crate::EngineWarning as W;
    match d {
        Diagnostic::Unroutable { id, reason } => {
            let reason = reason_text(&reason);
            let m = tr!(
                "arrow `{id}` cannot be routed: {reason}",
                "flèche « {id} » impossible à tracer : {reason}"
            );
            W::new("unroutable", m).on(id)
        }
        Diagnostic::CheckpointUnreachable { id, index } => {
            let m = tr!(
                "arrow `{id}`: waypoint no. {} is unreachable, ignored",
                "flèche « {id} » : point de passage n° {} inatteignable, ignoré",
                index + 1
            );
            W::new("waypoint-unreachable", m).on(id)
        }
        Diagnostic::SideIgnored { id, at_start } => {
            let end = if at_start {
                tr!("at the start", "au départ")
            } else {
                tr!("at the end", "à l'arrivée")
            };
            let m = tr!(
                "arrow `{id}`: the forced side {end} is unusable, another side was used",
                "flèche « {id} » : côté imposé {end} inutilisable, un autre côté a servi"
            );
            W::new("side-ignored", m).on(id)
        }
        Diagnostic::InvalidInput { id, reason } => {
            let reason = reason_text(&reason);
            W::new(
                "route-input",
                tr!("`{id}`: {reason}", "« {id} » : {reason}"),
            )
            .on(id)
        }
    }
}

/// Middle of the longest segment (prototype's `midOfLongest`).
pub fn mid_of_longest(pts: &[(f64, f64)]) -> Option<(f64, f64)> {
    let mut best = -1.0;
    let mut m = *pts.first()?;
    for w in pts.windows(2) {
        let l = (w[1].0 - w[0].0).hypot(w[1].1 - w[0].1);
        if l > best {
            best = l;
            m = ((w[0].0 + w[1].0) / 2.0, (w[0].1 + w[1].1) / 2.0);
        }
    }
    Some(m)
}

/// Length of a polyline.
pub fn length(pts: &[(f64, f64)]) -> f64 {
    pts.windows(2)
        .map(|w| (w[1].0 - w[0].0).hypot(w[1].1 - w[0].1))
        .sum()
}

/// Cause from the router (structured) in the current language.
fn reason_text(r: &skemo_router::Reason) -> String {
    use skemo_router::Reason;
    match r {
        Reason::NonFiniteRect => tr!("non-finite rectangle", "rectangle non fini"),
        Reason::DuplicateId => tr!(
            "duplicate identifier, ignored",
            "identifiant en double, ignoré"
        ),
        Reason::NotAShape(id) => tr!(
            "`{id}` is a region, not a shape",
            "« {id} » est une région, pas une forme"
        ),
        Reason::UnknownShape(id) => tr!("unknown shape `{id}`", "forme « {id} » inconnue"),
        Reason::NonFiniteEnd => tr!("non-finite end point", "extrémité non finie"),
        Reason::EndInsideObstacle => tr!(
            "free end point inside an obstacle",
            "extrémité libre à l'intérieur d'un obstacle"
        ),
        Reason::NoClearSide(id) => tr!(
            "no side of `{id}` is clear",
            "aucun côté de « {id} » n'est dégagé"
        ),
        Reason::NoPath => tr!("no path", "aucun chemin"),
    }
}
