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
    /// Timeline: crossable lanes, default sides `right-left` within one lane, start `right` towards
    /// another lane (F-TL-5).
    Timeline,
    /// Frieze: no zone obstacle (F-FR-5).
    Frise,
    /// Mindmap: sides facing outward and towards the centre, trunk shared by the children on one side.
    Mindmap,
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
        ..params()
    }
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
    Vec<String>,
    BTreeMap<String, String>,
) {
    let (obstacles, conns) = router_scene(model, boxes, elk, mode, soft, titles);
    let groups = groups_of(&conns);
    let (routes, diags) = skemo_router::route(&obstacles, &conns, params);
    let raw: BTreeMap<String, Points> = routes.into_iter().map(|r| (r.id, r.points)).collect();
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

/// The router's routes as drawn: micro jogs dropped, cross segments centred in their gap,
/// small jogs straightened (D-049). Shared by the full computation and the editing session, so
/// that a drag preview is the drawing a reload gives.
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
        .filter(|e| fan_in(e))
        .map(|e| e.id.as_str())
        .collect();
    let mut out: BTreeMap<String, Points> = raw
        .iter()
        .map(|(id, p)| {
            let mut p = p.clone();
            if turned.contains(&id.as_str()) {
                p.reverse();
            }
            (id.clone(), drop_micro_jogs(p))
        })
        .collect();
    if mode == Mode::Graph {
        center_in_gaps(model, boxes, titles, groups, &mut out);
    }
    straighten_jogs(model, boxes, groups, &mut out);
    if mode == Mode::Graph {
        crate::breathe::breathe(model, boxes, titles, groups, &mut out);
        flatten_steps(model, boxes, titles, groups, &mut out);
        crate::stack::plug(model, boxes, &mut out);
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
pub fn warnings_of(diags: Vec<Diagnostic>) -> Vec<String> {
    diags
        .into_iter()
        .map(|d| match d {
            Diagnostic::Unroutable { id, reason } => {
                let reason = reason_text(&reason);
                tr!(
                    "arrow « {id} » cannot be routed: {reason}",
                    "flèche « {id} » impossible à tracer : {reason}"
                )
            }
            Diagnostic::CheckpointUnreachable { id, index } => tr!(
                "arrow « {id} »: waypoint no. {} is unreachable, ignored",
                "flèche « {id} » : point de passage n° {} inatteignable, ignoré",
                index + 1
            ),
            Diagnostic::SideIgnored { id, at_start } => {
                tr!(
                    "arrow « {id} »: the forced side {} is unusable, another side was used",
                    "flèche « {id} » : côté imposé {} inutilisable, un autre côté a servi",
                    if at_start {
                        tr!("at the start", "au départ")
                    } else {
                        tr!("at the end", "à l'arrivée")
                    }
                )
            }
            Diagnostic::InvalidInput { id, reason } => {
                let reason = reason_text(&reason);
                tr!("« {id} »: {reason}", "« {id} » : {reason}")
            }
        })
        .collect()
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
            "« {id} » is a region, not a shape",
            "« {id} » est une région, pas une forme"
        ),
        Reason::UnknownShape(id) => tr!("unknown shape « {id} »", "forme « {id} » inconnue"),
        Reason::NonFiniteEnd => tr!("non-finite end point", "extrémité non finie"),
        Reason::EndInsideObstacle => tr!(
            "free end point inside an obstacle",
            "extrémité libre à l'intérieur d'un obstacle"
        ),
        Reason::NoClearSide(id) => tr!(
            "no side of « {id} » is clear",
            "aucun côté de « {id} » n'est dégagé"
        ),
        Reason::NoPath => tr!("no path", "aucun chemin"),
    }
}
