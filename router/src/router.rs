//! Orchestration: checks the inputs, the ports, builds the scene, then routes the connectors one
//! by one, in the order given (the routes already made count for the crossing penalty).

use std::collections::BTreeMap;

use crate::geom::{Dir, Point, Rect, Side};
use crate::model::{Connector, Diagnostic, End, Obstacle, ObstacleKind, Params, Reason, Route};
use crate::scene::{Item, Scene};
use crate::search::{Node, Outcome, Query, Source, Target, Workspace, search};
use crate::simplify::{merge, remove_jogs};

/// A possible port: point on the border, outward direction, end of the stub.
#[derive(Clone, Debug)]
struct Port {
    on_border: Point,
    dir: Option<Dir>,
    stub: Point,
}

/// A prepared end.
#[derive(Clone)]
struct PreparedEnd {
    ports: Vec<Port>,
    side_ignored: bool,
}

#[derive(Clone)]
struct Prepared {
    active: Vec<bool>,
    from: PreparedEnd,
    to: PreparedEnd,
    checkpoints: Vec<(usize, Point)>,
}

fn finite(p: Point) -> bool {
    p.0.is_finite() && p.1.is_finite()
}

/// Computes the routes of every connector (SPEC §4).
pub fn route(
    obstacles: &[Obstacle],
    connectors: &[Connector],
    params: &Params,
) -> (Vec<Route>, Vec<Diagnostic>) {
    let mut diags = Vec::new();
    let buffer = sanitized_buffer(params.shape_buffer);
    let (items, by_id) = collect_items(obstacles, buffer, &mut diags);

    // Provisional scene (without the ports' lines) to prepare the stubs.
    let probe = Scene::new(items.clone(), buffer, vec![], vec![], &[], false);
    let prepared = prepare_all(connectors, &probe, &by_id, buffer, &mut diags);

    let mut items = items;
    // a shape some connector ignores is no hole in the graph (its line may cross it there)
    for (k, it) in items.iter_mut().enumerate() {
        it.optional =
            it.kind == ObstacleKind::Shape && prepared.iter().flatten().any(|p| !p.active[k]);
    }
    let scene = build_final_scene(items, &prepared, buffer);
    let mut routes = route_in_order(connectors, &prepared, &scene, &by_id, params, &mut diags);

    // Looks (R2): spreading of the ports and spacing of parallel segments.
    let shapes: BTreeMap<&str, crate::geom::Rect> = scene
        .items
        .iter()
        .filter(|it| it.kind == ObstacleKind::Shape)
        .map(|it| (it.id.as_str(), it.hard))
        .collect();
    crate::nudge::beautify(&mut routes, connectors, &shapes, params);
    (routes, diags)
}

fn sanitized_buffer(buffer: f64) -> f64 {
    if buffer.is_finite() {
        buffer.max(0.0)
    } else {
        0.0
    }
}

/// Valid obstacles, unique ids (the first one wins); the others are reported.
fn collect_items<'a>(
    obstacles: &'a [Obstacle],
    buffer: f64,
    diags: &mut Vec<Diagnostic>,
) -> (Vec<Item>, BTreeMap<&'a str, usize>) {
    let mut items: Vec<Item> = Vec::new();
    let mut by_id: BTreeMap<&str, usize> = BTreeMap::new();
    for o in obstacles {
        if !o.rect.is_finite() {
            diags.push(Diagnostic::InvalidInput {
                id: o.id.clone(),
                reason: Reason::NonFiniteRect,
            });
            continue;
        }
        if by_id.contains_key(o.id.as_str()) {
            diags.push(Diagnostic::InvalidInput {
                id: o.id.clone(),
                reason: Reason::DuplicateId,
            });
            continue;
        }
        let hard = o.rect.normalized();
        by_id.insert(&o.id, items.len());
        items.push(Item {
            id: o.id.clone(),
            kind: o.kind,
            hard,
            soft: hard.inflate(buffer),
            optional: false,
        });
    }
    (items, by_id)
}

/// Each connector prepared; `None` if one of its ends is invalid (reported).
fn prepare_all(
    connectors: &[Connector],
    probe: &Scene,
    by_id: &BTreeMap<&str, usize>,
    buffer: f64,
    diags: &mut Vec<Diagnostic>,
) -> Vec<Option<Prepared>> {
    connectors
        .iter()
        .map(|c| match prepare(c, probe, by_id, buffer, diags) {
            Ok(p) => Some(p),
            Err(reason) => {
                diags.push(Diagnostic::Unroutable {
                    id: c.id.clone(),
                    reason,
                });
                None
            }
        })
        .collect()
}

/// Final scene: lines of the ports, stubs and waypoints.
fn build_final_scene(items: Vec<Item>, prepared: &[Option<Prepared>], buffer: f64) -> Scene {
    let (mut ex, mut ey, mut pts) = (Vec::new(), Vec::new(), Vec::new());
    for p in prepared.iter().flatten() {
        for port in p.from.ports.iter().chain(&p.to.ports) {
            ex.extend([port.on_border.0, port.stub.0]);
            ey.extend([port.on_border.1, port.stub.1]);
            pts.extend([port.on_border, port.stub]);
        }
        for (_, cp) in &p.checkpoints {
            ex.push(cp.0);
            ey.push(cp.1);
            pts.push(*cp);
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    let t0 = std::time::Instant::now();
    let scene = Scene::new(items, buffer, ex, ey, &pts, true);
    #[cfg(not(target_arch = "wasm32"))]
    if std::env::var_os("ROUTER_STATS").is_some() {
        eprintln!(
            "scene {}×{} built in {:.0} ms, {} active nodes",
            scene.xs.len(),
            scene.ys.len(),
            t0.elapsed().as_secs_f64() * 1000.0,
            scene.active_nodes()
        );
    }
    scene
}

/// Routing, in the connectors' order: the routes already made count for the crossing penalty.
fn route_in_order(
    connectors: &[Connector],
    prepared: &[Option<Prepared>],
    scene: &Scene,
    by_id: &BTreeMap<&str, usize>,
    params: &Params,
    diags: &mut Vec<Diagnostic>,
) -> Vec<Route> {
    let mut ws = Workspace::default();
    let mut through: Vec<(u32, u32)> = vec![(0, 0); scene.n_nodes()];
    let mut routes = Vec::new();
    let trunk_dirs = trunk_directions(connectors, scene, by_id);
    for (c, p) in connectors.iter().zip(prepared) {
        let Some(p) = p else { continue };
        report_ignored_sides(c, p, diags);
        let rect = |e: &End| match e {
            End::Shape { id, .. } => by_id.get(id.as_str()).map(|&k| scene.items[k].hard),
            End::Point { .. } => None,
        };
        let shared = shared_trunk(c, p, &trunk_dirs, rect);
        let p = shared.as_ref().unwrap_or(p);
        // R-15: a valid hint route is taken as is (it will only be spaced, R2); not for a hyperedge,
        // whose trunk must be common
        // a straight line between two facing shapes comes first (no jog, R-20)
        let hinted = if c.group.is_some() {
            None
        } else {
            crate::hint::straight(c, &p.active, scene, by_id, params.shape_buffer)
                .or_else(|| crate::hint::valid_hint(c, &p.active, scene, by_id))
        };
        let points = hinted
            .or_else(|| route_one(scene, p, params, &through, &mut ws))
            .or_else(|| route_without_checkpoints(c, p, scene, params, &through, &mut ws, diags));
        match points {
            Some(points) => {
                record(scene, &points, &mut through);
                routes.push(Route {
                    id: c.id.clone(),
                    points,
                });
            }
            None => diags.push(Diagnostic::Unroutable {
                id: c.id.clone(),
                reason: Reason::NoPath,
            }),
        }
    }
    routes
}

/// Côté de départ commun de chaque groupe d'hyperarête (R-18). First choice: a side that every
/// target lies entirely beyond (all of them below the source: down), the one with the widest gap
/// if several do; a tree drawn downwards then branches downwards even when one target is far to
/// the side. Otherwise the side facing the targets' centre (the axis with the larger offset).
fn trunk_directions<'a>(
    connectors: &'a [Connector],
    scene: &Scene,
    by_id: &BTreeMap<&str, usize>,
) -> BTreeMap<&'a str, Dir> {
    let rect = |e: &End| match e {
        End::Shape { id, .. } => by_id.get(id.as_str()).map(|&k| scene.items[k].hard),
        End::Point { x, y } => Some(Rect::new(*x, *y, 0.0, 0.0)),
    };
    let mut groups: BTreeMap<&str, (Rect, Vec<Rect>)> = BTreeMap::new();
    for c in connectors {
        let (Some(g), Some(from), Some(to)) = (c.group.as_deref(), rect(&c.from), rect(&c.to))
        else {
            continue;
        };
        groups.entry(g).or_insert((from, Vec::new())).1.push(to);
    }
    groups
        .into_iter()
        .map(|(g, (from, targets))| (g, trunk_direction(&from, &targets)))
        .collect()
}

fn trunk_direction(from: &Rect, targets: &[Rect]) -> Dir {
    // smallest gap between the source and the targets beyond each side (None: not all beyond)
    let gap = |d: Dir| {
        targets
            .iter()
            .map(|t| match d {
                Dir::Down => t.y0() - from.y1(),
                Dir::Up => from.y0() - t.y1(),
                Dir::Right => t.x0() - from.x1(),
                Dir::Left => from.x0() - t.x1(),
            })
            .try_fold(f64::INFINITY, |m, g| (g > 0.0).then_some(m.min(g)))
    };
    let beyond = Dir::ALL
        .into_iter()
        .filter_map(|d| gap(d).map(|g| (d, g)))
        .max_by(|a, b| a.1.total_cmp(&b.1));
    if let Some((d, _)) = beyond {
        return d;
    }
    let n = targets.len().max(1) as f64;
    let c = from.center();
    let (dx, dy) = targets.iter().fold((0.0, 0.0), |(x, y), t| {
        (x + t.center().0 / n, y + t.center().1 / n)
    });
    let (dx, dy) = (dx - c.0, dy - c.1);
    match (dx.abs() > dy.abs(), dx > 0.0, dy > 0.0) {
        (true, true, _) => Dir::Right,
        (true, false, _) => Dir::Left,
        (false, _, true) => Dir::Down,
        (false, _, false) => Dir::Up,
    }
}

/// Connector of a group: its start restricted to the common side (one port per side, hence the
/// same point for all), so that the start of their routes is a single trunk. `None` if nothing
/// changes (no group, or side unavailable).
///
/// The branch enters its target by the side that faces the trunk when the target lies beyond the
/// source in the trunk's direction (no entry by the side, half-way up a card); otherwise, or if
/// that side is not available (`side=`), its ports are left as they are.
fn shared_trunk(
    c: &Connector,
    p: &Prepared,
    dirs: &BTreeMap<&str, Dir>,
    rect: impl Fn(&End) -> Option<Rect>,
) -> Option<Prepared> {
    let dir = *dirs.get(c.group.as_deref()?)?;
    let mut shared = p.clone();
    shared.from.ports.retain(|port| port.dir == Some(dir));
    if shared.from.ports.is_empty() {
        return None;
    }
    let beyond = match (rect(&c.from), rect(&c.to)) {
        (Some(a), Some(b)) => match dir {
            Dir::Up => b.y1() <= a.y0(),
            Dir::Down => b.y0() >= a.y1(),
            Dir::Left => b.x1() <= a.x0(),
            Dir::Right => b.x0() >= a.x1(),
        },
        _ => false,
    };
    let facing: Vec<Port> =
        p.to.ports
            .iter()
            .filter(|port| port.dir == Some(dir.reverse()))
            .cloned()
            .collect();
    if beyond && !facing.is_empty() {
        shared.to.ports = facing;
    }
    Some(shared)
}

fn report_ignored_sides(c: &Connector, p: &Prepared, diags: &mut Vec<Diagnostic>) {
    for (ignored, at_start) in [(p.from.side_ignored, true), (p.to.side_ignored, false)] {
        if ignored {
            diags.push(Diagnostic::SideIgnored {
                id: c.id.clone(),
                at_start,
            });
        }
    }
}

/// R-12: without the waypoints, and reported. `None` if there were none or if the connector
/// cannot be routed without them either.
fn route_without_checkpoints(
    c: &Connector,
    p: &Prepared,
    scene: &Scene,
    params: &Params,
    through: &[(u32, u32)],
    ws: &mut Workspace,
    diags: &mut Vec<Diagnostic>,
) -> Option<Vec<Point>> {
    if p.checkpoints.is_empty() {
        return None;
    }
    let relaxed = Prepared {
        active: p.active.clone(),
        from: PreparedEnd {
            ports: p.from.ports.clone(),
            side_ignored: false,
        },
        to: PreparedEnd {
            ports: p.to.ports.clone(),
            side_ignored: false,
        },
        checkpoints: Vec::new(),
    };
    let points = route_one(scene, &relaxed, params, through, ws)?;
    for (index, _) in &p.checkpoints {
        diags.push(Diagnostic::CheckpointUnreachable {
            id: c.id.clone(),
            index: *index,
        });
    }
    Some(points)
}

fn prepare(
    c: &Connector,
    scene: &Scene,
    by_id: &BTreeMap<&str, usize>,
    buffer: f64,
    diags: &mut Vec<Diagnostic>,
) -> Result<Prepared, Reason> {
    let resolve = |e: &End| -> Result<(), Reason> {
        match e {
            End::Shape { id, .. } => match by_id.get(id.as_str()) {
                Some(&k) if scene.items[k].kind == ObstacleKind::Shape => Ok(()),
                Some(_) => Err(Reason::NotAShape(id.clone())),
                None => Err(Reason::UnknownShape(id.clone())),
            },
            End::Point { x, y } if finite((*x, *y)) => Ok(()),
            End::Point { .. } => Err(Reason::NonFiniteEnd),
        }
    };
    resolve(&c.from)?;
    resolve(&c.to)?;

    // Reference points of the ends, for the regions' rule.
    let anchor = |e: &End| match e {
        End::Shape { id, .. } => scene.items[by_id[id.as_str()]].hard.center(),
        End::Point { x, y } => (*x, *y),
    };
    let anchors = [anchor(&c.from), anchor(&c.to)];
    let is_end = |id: &str| {
        [&c.from, &c.to]
            .iter()
            .any(|e| matches!(e, End::Shape { id: s, .. } if s == id))
    };
    // R-03: a region blocks, unless it is allowed or holds an end.
    let active: Vec<bool> = scene
        .items
        .iter()
        .map(|it| match it.kind {
            // a shape the connector ignores does not block it, unless it is one of its ends
            ObstacleKind::Shape => !c.ignore.contains(&it.id) || is_end(&it.id),
            ObstacleKind::Region => {
                !c.pass_through.contains(&it.id) && !anchors.iter().any(|&a| it.hard.contains(a))
            }
        })
        .collect();

    let from = prepare_end(&c.from, scene, by_id, &active, buffer)?;
    let to = prepare_end(&c.to, scene, by_id, &active, buffer)?;

    let mut checkpoints = Vec::new();
    for (index, &cp) in c.checkpoints.iter().enumerate() {
        if !finite(cp) || scene.inside(cp, &active) {
            diags.push(Diagnostic::CheckpointUnreachable {
                id: c.id.clone(),
                index,
            });
        } else {
            checkpoints.push((index, cp));
        }
    }
    Ok(Prepared {
        active,
        from,
        to,
        checkpoints,
    })
}

fn prepare_end(
    e: &End,
    scene: &Scene,
    by_id: &BTreeMap<&str, usize>,
    active: &[bool],
    buffer: f64,
) -> Result<PreparedEnd, Reason> {
    match e {
        End::Point { x, y } => {
            let p = (*x, *y);
            if scene.inside(p, active) {
                return Err(Reason::EndInsideObstacle);
            }
            Ok(PreparedEnd {
                ports: vec![Port {
                    on_border: p,
                    dir: None,
                    stub: p,
                }],
                side_ignored: false,
            })
        }
        End::Shape { id, side } => {
            let me = by_id[id.as_str()];
            let r = scene.items[me].hard;
            let make =
                |s: Side| -> Option<Port> { port_at(scene, active, buffer, s, s.point(&r, 0.5)) };
            let make_all = |s: Side| -> Vec<Port> {
                let mut out: Vec<Port> = make(s).into_iter().collect();
                for p in slid_ports(scene, active, buffer, me, s) {
                    out.extend(port_at(scene, active, buffer, s, p));
                }
                out
            };
            let all: Vec<Port> = Side::ALL.iter().flat_map(|&s| make_all(s)).collect();
            match side {
                Some(s) => match make_all(*s) {
                    ports if !ports.is_empty() => Ok(PreparedEnd {
                        ports,
                        side_ignored: false,
                    }),
                    _ if !all.is_empty() => Ok(PreparedEnd {
                        ports: all,
                        side_ignored: true,
                    }),
                    _ => Err(Reason::NoClearSide(id.clone())),
                },
                None if all.is_empty() => Err(Reason::NoClearSide(id.clone())),
                None => Ok(PreparedEnd {
                    ports: all,
                    side_ignored: false,
                }),
            }
        }
    }
}

/// Expansions beyond which avoiding crossings is given up.
const BUDGET: usize = 40_000;

fn route_one(
    scene: &Scene,
    p: &Prepared,
    params: &Params,
    through: &[(u32, u32)],
    ws: &mut Workspace,
) -> Option<Vec<Point>> {
    let node = |q: Point| Some((scene.ix(q.0)?, scene.iy(q.1)?));
    let len = |a: Point, b: Point| (a.0 - b.0).abs() + (a.1 - b.1).abs();
    // a side against the flow (`Params::flow`) costs extra: perpendicular once, opposite twice
    let off_flow = |dir: Option<Dir>, want: Dir| match (params.flow, dir) {
        (Some(_), Some(d)) if d == want => 0.0,
        (Some(_), Some(d)) if d == want.reverse() => 2.0 * params.flow_penalty,
        (Some(_), Some(_)) => params.flow_penalty,
        _ => 0.0,
    };
    let flow = params.flow.unwrap_or(Dir::Down);
    let mut sources = Vec::new();
    let mut src_ports = Vec::new();
    for port in &p.from.ports {
        if let Some(n) = node(port.stub) {
            sources.push(Source {
                node: n,
                dir: port.dir,
                cost: len(port.on_border, port.stub) + off_flow(port.dir, flow),
            });
            src_ports.push(port);
        }
    }
    let mut targets = Vec::new();
    let mut tgt_ports = Vec::new();
    for port in &p.to.ports {
        if let Some(n) = node(port.stub) {
            targets.push(Target {
                node: n,
                arrive: port.dir.map(Dir::reverse),
                cost: len(port.on_border, port.stub) + off_flow(port.dir, flow.reverse()),
            });
            tgt_ports.push(port);
        }
    }
    let checkpoints: Vec<Node> = p
        .checkpoints
        .iter()
        .filter_map(|(_, cp)| node(*cp))
        .collect();
    let q = Query {
        scene,
        active: &p.active,
        params,
        sources: &sources,
        targets: &targets,
        checkpoints: &checkpoints,
        through,
    };
    // Budget: a search that drags on (most often to avoid crossings) is run again without the
    // crossing penalty, which is fast; the result stays deterministic.
    let budget = Params {
        max_expansions: params.max_expansions.min(BUDGET),
        ..params.clone()
    };
    let q = Query {
        params: &budget,
        ..q
    };
    let f = match search(&q, ws) {
        Outcome::Found(f) => f,
        Outcome::NoPath => return None,
        Outcome::Exhausted => {
            let relaxed = Params {
                crossing_penalty: 0.0,
                ..params.clone()
            };
            match search(
                &Query {
                    params: &relaxed,
                    ..q
                },
                ws,
            ) {
                Outcome::Found(f) => f,
                _ => return None,
            }
        }
    };
    let mut pts = vec![src_ports[f.source].on_border];
    pts.extend(f.nodes.iter().map(|&(i, j)| (scene.xs[i], scene.ys[j])));
    pts.push(tgt_ports[f.target].on_border);
    merge(&mut pts);
    if params.min_jog > 0.0 {
        // The waypoints must stay on the route.
        let cps: Vec<Point> = p.checkpoints.iter().map(|c| c.1).collect();
        let before = pts.clone();
        remove_jogs(&mut pts, params.min_jog, |a, b| {
            !scene.blocked(a, b, &p.active)
        });
        if !cps.iter().all(|&c| on_path(&pts, c)) {
            pts = before;
        }
    }
    Some(pts)
}

fn on_path(p: &[Point], c: Point) -> bool {
    p.windows(2).any(|w| {
        let (a, b) = (w[0], w[1]);
        (c.0 - a.0) * (c.0 - b.0) <= 1e-9
            && (c.1 - a.1) * (c.1 - b.1) <= 1e-9
            && ((a.0 - b.0).abs() < 1e-9 || (a.1 - b.1).abs() < 1e-9)
    })
}

/// Records the nodes a route goes through (for the crossing penalty).
fn record(scene: &Scene, pts: &[Point], through: &mut [(u32, u32)]) {
    for w in pts.windows(2) {
        let (a, b) = (w[0], w[1]);
        if (a.1 - b.1).abs() < 1e-9 {
            let Some(j) = scene.iy(a.1) else { continue };
            let (lo, hi) = (a.0.min(b.0), a.0.max(b.0));
            let start = scene.xs.partition_point(|&x| x <= lo);
            for i in start..scene.xs.len() {
                if scene.xs[i] >= hi {
                    break;
                }
                if let Some(id) = scene.id(i, j) {
                    through[id as usize].0 += 1;
                }
            }
        } else if (a.0 - b.0).abs() < 1e-9 {
            let Some(i) = scene.ix(a.0) else { continue };
            let (lo, hi) = (a.1.min(b.1), a.1.max(b.1));
            let start = scene.ys.partition_point(|&y| y <= lo);
            for j in start..scene.ys.len() {
                if scene.ys[j] >= hi {
                    break;
                }
                if let Some(id) = scene.id(i, j) {
                    through[id as usize].1 += 1;
                }
            }
        }
    }
}

/// Port on the border at `on_border`, its stub as long as the buffer, shortened if another shape
/// is closer; `None` if even a half-pixel stub is blocked.
fn port_at(scene: &Scene, active: &[bool], buffer: f64, s: Side, on_border: Point) -> Option<Port> {
    let d = s.outward();
    let v = d.vector();
    let mut l = buffer.max(1.0);
    while l >= 0.5 {
        let stub = (on_border.0 + v.0 * l, on_border.1 + v.1 * l);
        if !scene.blocked(on_border, stub, active) && !scene.inside(stub, active) {
            return Some(Port {
                on_border,
                dir: Some(d),
                stub,
            });
        }
        l /= 2.0;
    }
    None
}

/// Extra ports when a shape sits right in front of the middle of side `s` of item `me` (closer
/// than twice the buffer: a zone title above a card): the nearest points of the side, on each
/// side of the middle, whose way out is clear, one buffer away from the blocking shapes and from
/// the corners. The middle port is kept; the search picks the cheapest.
fn slid_ports(scene: &Scene, active: &[bool], buffer: f64, me: usize, s: Side) -> Vec<Point> {
    let r = scene.items[me].hard;
    let depth = 2.0 * buffer.max(1.0);
    // along: coordinate along the side; the strip in front of the side, `depth` deep
    let (lo, hi) = match s {
        Side::Top | Side::Bottom => (r.x0(), r.x1()),
        Side::Left | Side::Right => (r.y0(), r.y1()),
    };
    let v = s.outward().vector();
    let at = |t: f64| -> (Point, Point) {
        let p = match s {
            Side::Top | Side::Bottom => (t, s.point(&r, 0.0).1),
            Side::Left | Side::Right => (s.point(&r, 0.0).0, t),
        };
        (p, (p.0 + v.0 * depth, p.1 + v.1 * depth))
    };
    let blockers: Vec<crate::geom::Rect> = scene
        .items
        .iter()
        .enumerate()
        .filter(|(k, it)| *k != me && active[*k] && it.kind == ObstacleKind::Shape)
        .map(|(_, it)| it.hard)
        .filter(|h| {
            let (a, b) = at((lo + hi) / 2.0);
            h.hit_by_segment(a, b)
        })
        .collect();
    if blockers.is_empty() {
        return Vec::new();
    }
    let clear = |t: f64| {
        let (a, b) = at(t);
        t >= lo + buffer && t <= hi - buffer && !scene.blocked(a, b, active)
    };
    let mid = (lo + hi) / 2.0;
    let mut cands: Vec<f64> = blockers
        .iter()
        .flat_map(|h| match s {
            Side::Top | Side::Bottom => [h.x0() - buffer, h.x1() + buffer],
            Side::Left | Side::Right => [h.y0() - buffer, h.y1() + buffer],
        })
        .filter(|t| clear(*t))
        .collect();
    cands.sort_by(|a, b| (a - mid).abs().total_cmp(&(b - mid).abs()));
    let below = cands.iter().find(|t| **t < mid);
    let above = cands.iter().find(|t| **t > mid);
    below.into_iter().chain(above).map(|t| at(*t).0).collect()
}
