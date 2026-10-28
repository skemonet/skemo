//! Test sets (`fixtures/*.json`, SPEC §8) and quality criteria (SPEC §7).
//! Shared by the integration tests and the `review` example.
#![allow(dead_code)]

use std::collections::BTreeMap;

use serde_json::Value;
use skemo_router::{Connector, End, Obstacle, ObstacleKind, Params, Point, Rect, Side};

pub struct Fixture {
    pub name: String,
    pub params: Params,
    pub obstacles: Vec<Obstacle>,
    pub connectors: Vec<Connector>,
    pub expected: BTreeMap<String, Vec<Point>>,
}

fn side(s: &str) -> Option<Side> {
    match s {
        "top" => Some(Side::Top),
        "bottom" => Some(Side::Bottom),
        "left" => Some(Side::Left),
        "right" => Some(Side::Right),
        _ => None,
    }
}

fn pt(v: &Value) -> Point {
    (v[0].as_f64().unwrap(), v[1].as_f64().unwrap())
}

fn end(v: &Value) -> End {
    End::Shape {
        id: v["shape"].as_str().unwrap().to_string(),
        side: v["side"].as_str().and_then(side),
    }
}

impl Fixture {
    pub fn load(path: &str) -> Fixture {
        let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{path} : {e}"));
        let j: Value = serde_json::from_str(&text).unwrap();
        let p = &j["params"];
        let mut params = Params::default();
        let get = |k: &str, d: f64| p[k].as_f64().unwrap_or(d);
        params.shape_buffer = get("shapeBuffer", params.shape_buffer);
        params.ideal_nudging = get("idealNudging", params.ideal_nudging);
        params.segment_penalty = get("segmentPenalty", params.segment_penalty);
        params.crossing_penalty = get("crossingPenalty", params.crossing_penalty);
        let obstacles = j["obstacles"]
            .as_array()
            .unwrap()
            .iter()
            .map(|o| {
                let r = o["rect"].as_array().unwrap();
                let f = |i: usize| r[i].as_f64().unwrap();
                let rect = Rect::new(f(0), f(1), f(2), f(3));
                let id = o["id"].as_str().unwrap();
                if o["kind"] == "region" {
                    Obstacle::region(id, rect)
                } else {
                    Obstacle::shape(id, rect)
                }
            })
            .collect();
        let connectors = j["connectors"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| {
                let mut k =
                    Connector::new(c["id"].as_str().unwrap(), end(&c["from"]), end(&c["to"]));
                if let Some(cps) = c["checkpoints"].as_array() {
                    k.checkpoints = cps.iter().map(pt).collect();
                }
                if let Some(pass) = c["passThrough"].as_array() {
                    k.pass_through = pass
                        .iter()
                        .map(|v| v.as_str().unwrap().to_string())
                        .collect();
                }
                k
            })
            .collect();
        let expected = j["expected"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(k, v)| (k.clone(), v.as_array().unwrap().iter().map(pt).collect()))
            .collect();
        Fixture {
            name: j["name"].as_str().unwrap().to_string(),
            params,
            obstacles,
            connectors,
            expected,
        }
    }

    pub fn all() -> Vec<Fixture> {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures");
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .filter_map(std::result::Result::ok)
            .map(|e| e.path().display().to_string())
            .filter(|p| p.ends_with(".json"))
            .collect();
        names.sort();
        names.iter().map(|n| Fixture::load(n)).collect()
    }

    pub fn obstacle(&self, id: &str) -> &Obstacle {
        self.obstacles.iter().find(|o| o.id == id).unwrap()
    }

    fn shape_of(&self, e: &End) -> Option<&Obstacle> {
        match e {
            End::Shape { id, .. } => Some(self.obstacle(id)),
            End::Point { .. } => None,
        }
    }

    /// Regions forbidden to the connector (same rule as the router, D-018).
    pub fn blocking_regions(&self, c: &Connector) -> Vec<&Obstacle> {
        let anchors: Vec<Point> = [&c.from, &c.to]
            .iter()
            .filter_map(|e| self.shape_of(e))
            .map(|o| o.rect.center())
            .collect();
        self.obstacles
            .iter()
            .filter(|o| o.kind == ObstacleKind::Region)
            .filter(|o| !c.pass_through.contains(&o.id))
            .filter(|o| !anchors.iter().any(|&a| o.rect.contains(a)))
            .collect()
    }
}

const EPS: f64 = 0.06; // the oracle is rounded to 0.1 px

#[derive(Default, Clone, Debug)]
pub struct Conn {
    pub routed: bool,
    pub bends: usize,
    pub length: f64,
    pub r01: usize,
    pub r02: usize,
    pub buffer: usize,
    pub r03: usize,
    pub r06: usize,
    pub r07: usize,
}

#[derive(Default, Clone, Debug)]
pub struct Metrics {
    pub per_conn: BTreeMap<String, Conn>,
    pub routed: usize,
    pub total: usize,
    pub bends: usize,
    pub length: f64,
    pub r01: usize,
    pub r02: usize,
    pub buffer: usize,
    pub r03: usize,
    pub r06: usize,
    pub r07: usize,
    pub crossings: usize,
    pub min_spacing: Option<f64>,
    pub close_pairs: usize,
}

fn horizontal(a: Point, b: Point) -> bool {
    (a.1 - b.1).abs() <= EPS
}
fn vertical(a: Point, b: Point) -> bool {
    (a.0 - b.0).abs() <= EPS
}

/// Alignments and doubled points merged, to count the bends.
pub fn simplify(mut p: Vec<Point>) -> Vec<Point> {
    p.dedup_by(|a, b| (a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9);
    let mut i = 1;
    while i + 1 < p.len() {
        let (a, b, c) = (p[i - 1], p[i], p[i + 1]);
        let col = ((a.0 - b.0).abs() < 1e-9 && (b.0 - c.0).abs() < 1e-9)
            || ((a.1 - b.1).abs() < 1e-9 && (b.1 - c.1).abs() < 1e-9);
        let forward = (b.0 - a.0) * (c.0 - b.0) + (b.1 - a.1) * (c.1 - b.1) > 0.0;
        if col && forward {
            p.remove(i);
        } else {
            i += 1;
        }
    }
    p
}

fn side_of(r: &Rect, p: Point) -> Option<Side> {
    let inx = p.0 >= r.x0() - EPS && p.0 <= r.x1() + EPS;
    let iny = p.1 >= r.y0() - EPS && p.1 <= r.y1() + EPS;
    if inx && (p.1 - r.y0()).abs() <= EPS {
        Some(Side::Top)
    } else if inx && (p.1 - r.y1()).abs() <= EPS {
        Some(Side::Bottom)
    } else if iny && (p.0 - r.x0()).abs() <= EPS {
        Some(Side::Left)
    } else if iny && (p.0 - r.x1()).abs() <= EPS {
        Some(Side::Right)
    } else {
        None
    }
}

fn hits(r: &Rect, d: f64, a: Point, b: Point) -> bool {
    r.inflate(d).hit_by_segment(a, b)
}

/// End on the border, first (or last) segment perpendicular and leaving: validity and side
/// touched.
fn check_end(fx: &Fixture, e: &End, q: Point, next: Point) -> (bool, Option<Side>) {
    let Some(o) = fx.shape_of(e) else {
        return (true, None);
    };
    let Some(s) = side_of(&o.rect, q) else {
        return (false, None);
    };
    let (dx, dy) = (next.0 - q.0, next.1 - q.1);
    let ok = match s {
        Side::Top => dy < 0.0 && dx.abs() <= EPS,
        Side::Bottom => dy > 0.0 && dx.abs() <= EPS,
        Side::Left => dx < 0.0 && dy.abs() <= EPS,
        Side::Right => dx > 0.0 && dy.abs() <= EPS,
    };
    (ok, Some(s))
}

/// R-01 (borders) and R-06 (imposed side) for both ends; returns (R-01 violations, R-06).
fn end_violations(fx: &Fixture, c: &Connector, p: &[Point]) -> (usize, usize) {
    let n = p.len();
    let (oka, sa) = if n >= 2 {
        check_end(fx, &c.from, p[0], p[1])
    } else {
        (false, None)
    };
    let (okb, sb) = if n >= 2 {
        check_end(fx, &c.to, p[n - 1], p[n - 2])
    } else {
        (false, None)
    };
    let r01 = usize::from(!oka) + usize::from(!okb);
    let r06 = [(&c.from, sa), (&c.to, sb)]
        .into_iter()
        .filter(|(e, got)| matches!(e, End::Shape { side: Some(w), .. } if *got != Some(*w)))
        .count();
    (r01, r06)
}

/// Is point `cp` on segment `a`-`b` (within 0.5 px)?
fn on_segment(a: Point, b: Point, cp: Point) -> bool {
    (horizontal(a, b)
        && (cp.1 - a.1).abs() <= 0.5
        && cp.0 >= a.0.min(b.0) - 0.5
        && cp.0 <= a.0.max(b.0) + 0.5)
        || (vertical(a, b)
            && (cp.0 - a.0).abs() <= 0.5
            && cp.1 >= a.1.min(b.1) - 0.5
            && cp.1 <= a.1.max(b.1) + 0.5)
}

/// R-07: number of waypoints missing from the route or passed out of order.
fn missed_checkpoints(c: &Connector, p: &[Point]) -> usize {
    let mut pos = 0.0;
    let mut missed = 0;
    for cp in &c.checkpoints {
        let mut acc = 0.0;
        let mut found = None;
        for w in p.windows(2) {
            let (a, b) = (w[0], w[1]);
            let s = acc + (cp.0 - a.0).abs() + (cp.1 - a.1).abs();
            if on_segment(a, b, *cp) && s + 0.5 >= pos {
                found = Some(s);
                break;
            }
            acc += (b.0 - a.0).abs() + (b.1 - a.1).abs();
        }
        match found {
            Some(s) => pos = s,
            None => missed += 1,
        }
    }
    missed
}

/// R-02 (shapes crossed), margin (start and end shapes aside) and R-03 (blocking regions):
/// returns the three counts.
fn obstacle_violations(fx: &Fixture, c: &Connector, p: &[Point]) -> (usize, usize, usize) {
    let buf = fx.params.shape_buffer;
    let last = p.len().saturating_sub(2);
    let blocking = fx.blocking_regions(c);
    let own = |e: &End| match e {
        End::Shape { id, .. } => id.clone(),
        End::Point { .. } => String::new(),
    };
    let (from_id, to_id) = (own(&c.from), own(&c.to));
    let (mut r02, mut buffer, mut r03) = (0, 0, 0);
    for (i, w) in p.windows(2).enumerate() {
        let (a, b) = (w[0], w[1]);
        for o in fx
            .obstacles
            .iter()
            .filter(|o| o.kind == ObstacleKind::Shape)
        {
            if hits(&o.rect, -EPS, a, b) {
                r02 += 1;
            }
            let is_own = (i == 0 && o.id == from_id) || (i == last && o.id == to_id);
            if !is_own && hits(&o.rect, buf - 0.15, a, b) {
                buffer += 1;
            }
        }
        r03 += blocking
            .iter()
            .filter(|o| hits(&o.rect, -EPS, a, b))
            .count();
    }
    (r02, buffer, r03)
}

/// Measures of a routed connector (`p`: simplified route).
fn evaluate_connector(fx: &Fixture, c: &Connector, p: &[Point]) -> Conn {
    let (r01_ends, r06) = end_violations(fx, c, p);
    let (r02, buffer, r03) = obstacle_violations(fx, c, p);
    Conn {
        routed: true,
        bends: p.len().saturating_sub(2),
        length: skemo_router::path_length(p),
        r01: p
            .windows(2)
            .filter(|w| !horizontal(w[0], w[1]) && !vertical(w[0], w[1]))
            .count()
            + r01_ends,
        r02,
        buffer,
        r03,
        r06,
        r07: missed_checkpoints(c, p),
    }
}

/// Crossings and spacing between two segments of different routes.
fn compare_segments(m: &mut Metrics, (a, b): (Point, Point), (c, d): (Point, Point)) {
    let (h1, h2) = (horizontal(a, b), horizontal(c, d));
    if h1 != h2 {
        let (ha, hb, va, vb) = if h1 { (a, b, c, d) } else { (c, d, a, b) };
        let inside_h = va.0 > ha.0.min(hb.0) + EPS && va.0 < ha.0.max(hb.0) - EPS;
        let inside_v = ha.1 > va.1.min(vb.1) + EPS && ha.1 < va.1.max(vb.1) - EPS;
        if inside_h && inside_v {
            m.crossings += 1;
        }
        return;
    }
    // parallel: span along the axis, and fixed coordinate
    let (lo1, hi1, f1, lo2, hi2, f2) = if h1 {
        (
            a.0.min(b.0),
            a.0.max(b.0),
            a.1,
            c.0.min(d.0),
            c.0.max(d.0),
            c.1,
        )
    } else {
        (
            a.1.min(b.1),
            a.1.max(b.1),
            a.0,
            c.1.min(d.1),
            c.1.max(d.1),
            c.0,
        )
    };
    if hi1.min(hi2) - lo1.max(lo2) > 1.0 {
        let dist = (f1 - f2).abs();
        m.min_spacing = Some(m.min_spacing.map_or(dist, |v: f64| v.min(dist)));
        if dist < 12.0 - EPS {
            m.close_pairs += 1;
        }
    }
}

pub fn evaluate(fx: &Fixture, routes: &BTreeMap<String, Vec<Point>>) -> Metrics {
    let mut m = Metrics {
        total: fx.connectors.len(),
        ..Default::default()
    };
    let mut kept: Vec<Vec<Point>> = Vec::new();
    for c in &fx.connectors {
        let Some(raw) = routes.get(&c.id) else {
            m.per_conn.insert(c.id.clone(), Conn::default());
            continue;
        };
        let p = simplify(raw.clone());
        let cm = evaluate_connector(fx, c, &p);
        m.routed += 1;
        m.bends += cm.bends;
        m.length += cm.length;
        m.r01 += cm.r01;
        m.r02 += cm.r02;
        m.buffer += cm.buffer;
        m.r03 += cm.r03;
        m.r06 += cm.r06;
        m.r07 += cm.r07;
        kept.push(p);
        m.per_conn.insert(c.id.clone(), cm);
    }
    for (i, p1) in kept.iter().enumerate() {
        for p2 in &kept[i + 1..] {
            for w1 in p1.windows(2) {
                for w2 in p2.windows(2) {
                    compare_segments(&mut m, (w1[0], w1[1]), (w2[0], w2[1]));
                }
            }
        }
    }
    m
}

/// Gaps to the criteria of SPEC §7 (spacing aside if `with_spacing` is false).
pub fn criteria(fx: &Fixture, ours: &Metrics, exp: &Metrics, with_spacing: bool) -> Vec<String> {
    let mut v = Vec::new();
    if ours.routed != ours.total {
        v.push(format!(
            "{} connectors of {} routed",
            ours.routed, ours.total
        ));
    }
    for (k, n) in [
        ("R-01", ours.r01),
        ("R-02", ours.r02),
        ("R-03", ours.r03),
        ("R-06", ours.r06),
        ("R-07", ours.r07),
    ] {
        if n > 0 {
            v.push(format!("{k} : {n} violation(s)"));
        }
    }
    for c in &fx.connectors {
        let (o, e) = (&ours.per_conn[&c.id], &exp.per_conn[&c.id]);
        if o.routed && o.bends > e.bends + 1 {
            v.push(format!(
                "{} : {} coudes (expected {})",
                c.id, o.bends, e.bends
            ));
        }
    }
    if ours.length > 1.15 * exp.length {
        v.push(format!(
            "longueur {:.0} > 1,15 × {:.0}",
            ours.length, exp.length
        ));
    }
    if ours.crossings > exp.crossings {
        v.push(format!(
            "{} croisements (expected {})",
            ours.crossings, exp.crossings
        ));
    }
    if with_spacing && ours.close_pairs > 0 {
        v.push(format!(
            "{} pairs of parallel segments less than 12 px apart",
            ours.close_pairs
        ));
    }
    v
}

pub fn expected_metrics(fx: &Fixture) -> Metrics {
    evaluate(fx, &fx.expected)
}
