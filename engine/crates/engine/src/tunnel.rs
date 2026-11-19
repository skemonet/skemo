//! Tunnels (D-116). `tunnel#vpn(gw-b)` written in card `gw-a` joins the two cards; an arrow
//! written `to(db tunnel="vpn")` passes through it. Such an arrow is laid out as the legs that
//! lead to the tunnel and away from it — from its source to the near end, from the far end to
//! its target — so that every engine places and routes it with nothing new to learn. Along the
//! tunnel it takes the tunnel's own route, moved aside to a lane of its own (`GAP` apart, the
//! lanes centred on the tunnel's line), and the tube widens to hold them all. The legs and its
//! lane are then sewn into one line, whose parts inside the two cards are hidden when drawn
//! (`cuts`).

use crate::layout::Box2;
use crate::measure::Sizes;
use crate::route::Points;
use crate::scene::Scene;
use skemo_lang::{Edge, Model};
use std::collections::BTreeMap;

/// Between two lanes of a tube.
pub const GAP: f64 = 6.0;

/// An arrow through a tunnel, as laid out.
#[derive(Debug, Clone)]
struct Flow {
    id: String,
    tunnel: String,
    /// It runs along the tunnel from the tunnel's `to` to its `from`.
    backwards: bool,
    /// Its leg from the source to the near end, and the card it enters (none: it starts there).
    before: Option<(String, String)>,
    /// Its leg from the far end to the target, and the card it leaves (none: it ends there).
    after: Option<(String, String)>,
}

/// The legs of the arrows through a tunnel.
#[derive(Debug, Default, Clone)]
pub struct Plan {
    flows: Vec<Flow>,
}

/// Ancestors of `id`, the outermost first, `id` included.
fn chain<'m>(model: &'m Model, id: &'m str) -> Vec<&'m str> {
    let mut out = vec![id];
    let mut at = model.node(id).and_then(|n| n.parent.as_deref());
    while let Some(p) = at {
        out.push(p);
        at = model.node(p).and_then(|n| n.parent.as_deref());
    }
    out.reverse();
    out
}

/// How deep `a` and `b` share their zones: the near end of a tunnel is the one that shares
/// more with the arrow's source.
fn shared(model: &Model, a: &str, b: &str) -> usize {
    chain(model, a)
        .iter()
        .zip(chain(model, b))
        .take_while(|(x, y)| *x == y)
        .count()
}

/// Does arrow `e` run along tunnel `t` from its `to` to its `from`?
fn backwards(model: &Model, e: &Edge, t: &Edge) -> bool {
    let (a, b) = (t.from.as_str(), t.to.as_str());
    if e.from == a || e.to == b {
        return false;
    }
    if e.from == b || e.to == a {
        return true;
    }
    let ab = shared(model, &e.from, a) + shared(model, &e.to, b);
    let ba = shared(model, &e.from, b) + shared(model, &e.to, a);
    ba > ab
}

/// The two halves of `side="start-end"`.
fn halves(side: Option<&str>) -> (Option<String>, Option<String>) {
    let Some(s) = side else {
        return (None, None);
    };
    let (a, b) = s.split_once('-').unwrap_or((s, ""));
    let some = |h: &str| (!h.is_empty()).then(|| h.to_string());
    (some(a), some(b))
}

fn tunnel_of<'m>(model: &'m Model, e: &Edge) -> Option<&'m Edge> {
    let id = e.through.as_deref()?;
    model.edges.iter().find(|t| t.tunnel && t.id == id)
}

/// The model to lay out, the arrows through a tunnel cut into their legs, drawn with their
/// arrow's style (`scene`); `None` without any.
pub fn expand(model: &Model, scene: &mut Scene) -> Option<(Model, Plan)> {
    let (out, plan) = legs(model)?;
    for (leg, id) in leg_ids(&plan) {
        if let Some(el) = scene.edges.get(id).cloned() {
            scene.edges.insert(leg.to_string(), el);
        }
    }
    Some((out, plan))
}

fn legs(model: &Model) -> Option<(Model, Plan)> {
    if !model.edges.iter().any(|e| tunnel_of(model, e).is_some()) {
        return None;
    }
    let mut out = model.clone();
    out.edges.clear();
    let mut plan = Plan::default();
    for e in &model.edges {
        let Some(t) = tunnel_of(model, e) else {
            out.edges.push(e.clone());
            continue;
        };
        let back = backwards(model, e, t);
        let (near, far) = if back {
            (&t.to, &t.from)
        } else {
            (&t.from, &t.to)
        };
        let (start, end) = halves(e.side.as_deref());
        let leg = |id: String, from: &str, to: &str, label: bool, side: Option<String>| Edge {
            id,
            from: from.to_string(),
            to: to.to_string(),
            label: if label {
                e.label.clone()
            } else {
                String::new()
            },
            via: None,
            side,
            join: None,
            through: None,
            ..e.clone()
        };
        // the label goes with the first leg (the arrow's id), as it would on the arrow
        let mut first = true;
        let before = (e.from != *near).then(|| {
            first = false;
            out.edges.push(leg(
                e.id.clone(),
                &e.from,
                near,
                true,
                start.as_ref().map(|s| format!("{s}-")),
            ));
            (e.id.clone(), near.clone())
        });
        let after = (e.to != *far).then(|| {
            let id = if first {
                e.id.clone()
            } else {
                format!("{}\u{1}after", e.id)
            };
            out.edges.push(leg(
                id.clone(),
                far,
                &e.to,
                first,
                end.as_ref().map(|s| format!("-{s}")),
            ));
            (id, far.clone())
        });
        plan.flows.push(Flow {
            id: e.id.clone(),
            tunnel: t.id.clone(),
            backwards: back,
            before,
            after,
        });
    }
    Some((out, plan))
}

/// The ids of the legs that are not their arrow's (for the scene: drawn as their arrow).
pub fn leg_ids(plan: &Plan) -> impl Iterator<Item = (&str, &str)> {
    plan.flows.iter().flat_map(|f| {
        [&f.before, &f.after]
            .into_iter()
            .flatten()
            .filter(|(id, _)| *id != f.id)
            .map(|(id, _)| (id.as_str(), f.id.as_str()))
    })
}

/// How far inside a card the line runs before turning to its next part: enough for the bend to
/// be hidden, never past the middle.
fn inside(b: &Box2, (dx, dy): (f64, f64)) -> f64 {
    let half = if dx.abs() > dy.abs() { b.w } else { b.h } / 2.0;
    (half - 1.0).clamp(0.0, 12.0)
}

fn unit((ax, ay): (f64, f64), (bx, by): (f64, f64)) -> (f64, f64) {
    let (dx, dy) = (bx - ax, by - ay);
    let l = dx.hypot(dy);
    if l < 1e-9 {
        (0.0, 0.0)
    } else {
        (dx / l, dy / l)
    }
}

/// Sews `next` after `line`, both ending on card `b`: each runs on into the card, then the two
/// meet (that part is hidden, `cuts`).
fn sew(line: &mut Points, next: &[(f64, f64)], b: &Box2) {
    let (Some(&e), Some(&s)) = (line.last(), next.first()) else {
        line.extend_from_slice(next);
        return;
    };
    let before = line.len().checked_sub(2).map_or(e, |i| line[i]);
    let din = unit(before, e);
    let after = next.get(1).copied().unwrap_or(s);
    let dout = unit(after, s);
    let (a, c) = (inside(b, din), inside(b, dout));
    line.push((e.0 + din.0 * a, e.1 + din.1 * a));
    line.push((s.0 + dout.0 * c, s.1 + dout.1 * c));
    line.extend_from_slice(next);
}

/// `line` moved `d` to its left (an orthogonal route: each corner moves with both its segments).
fn aside(line: &[(f64, f64)], d: f64) -> Points {
    let normal = |a: (f64, f64), b: (f64, f64)| {
        let (ux, uy) = unit(a, b);
        (-uy, ux)
    };
    let segs: Vec<(f64, f64)> = line.windows(2).map(|w| normal(w[0], w[1])).collect();
    let mut out = Vec::with_capacity(line.len());
    for (i, p) in line.iter().enumerate() {
        let (n0, n1) = (segs.get(i.wrapping_sub(1)).copied(), segs.get(i).copied());
        let (nx, ny) = match (n0, n1) {
            (Some(a), Some(b)) if (a.0 * b.0 + a.1 * b.1).abs() < 0.5 => (a.0 + b.0, a.1 + b.1),
            (_, Some(b)) => b,
            (Some(a), None) => a,
            (None, None) => (0.0, 0.0),
        };
        out.push((p.0 + nx * d, p.1 + ny * d));
    }
    out
}

/// The arrows through a tunnel sewn into one line each; returns how many lanes each tube holds.
pub fn merge(
    plan: &Plan,
    routes: &mut BTreeMap<String, Points>,
    boxes: &BTreeMap<String, Box2>,
    labels: &mut BTreeMap<String, (f64, f64)>,
    label_lines: &mut BTreeMap<String, Vec<String>>,
    edge_groups: &mut BTreeMap<String, String>,
) -> BTreeMap<String, usize> {
    let mut lanes: BTreeMap<String, usize> = BTreeMap::new();
    for f in &plan.flows {
        *lanes.entry(f.tunnel.clone()).or_default() += 1;
    }
    let mut taken: BTreeMap<&str, usize> = BTreeMap::new();
    for f in &plan.flows {
        let n = lanes[&f.tunnel];
        let k = taken.entry(f.tunnel.as_str()).or_default();
        let shift = (*k as f64 - (n as f64 - 1.0) / 2.0) * GAP;
        *k += 1;
        let Some(along) = routes.get(&f.tunnel).filter(|r| r.len() >= 2) else {
            continue;
        };
        let mut lane = aside(along, shift);
        if f.backwards {
            lane.reverse();
        }
        let before = f
            .before
            .as_ref()
            .and_then(|(id, _)| routes.get(id).cloned());
        let after = f.after.as_ref().and_then(|(id, _)| routes.get(id).cloned());
        let mut line = match (&before, &f.before) {
            (Some(b), Some((_, card))) => {
                let mut l = b.clone();
                match boxes.get(card) {
                    Some(bx) => sew(&mut l, &lane, bx),
                    None => l.extend(lane.iter().copied()),
                }
                l
            }
            _ => lane.clone(),
        };
        if let (Some(a), Some((_, card))) = (&after, &f.after) {
            match boxes.get(card) {
                Some(bx) => sew(&mut line, a, bx),
                None => line.extend(a.iter().copied()),
            }
        }
        // an arrow from one end of the tunnel to the other: its label in the middle of its lane
        if f.before.is_none() && f.after.is_none() {
            let m = lane.len() / 2;
            if m > 0 {
                let (a, b) = (lane[m - 1], lane[m]);
                labels.insert(f.id.clone(), ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0));
            }
        }
        for (leg, _) in [&f.before, &f.after].into_iter().flatten() {
            if *leg != f.id {
                routes.remove(leg);
                labels.remove(leg);
                label_lines.remove(leg);
                edge_groups.remove(leg);
            }
        }
        routes.insert(f.id.clone(), line);
    }
    lanes
}

/// The cards an arrow through a tunnel crosses (the ends it does not start or end on): the
/// line is hidden there.
pub fn cuts<'m>(model: &'m Model, e: &'m Edge) -> Vec<&'m str> {
    let Some(t) = tunnel_of(model, e) else {
        return Vec::new();
    };
    [t.from.as_str(), t.to.as_str()]
        .into_iter()
        .filter(|c| *c != e.from && *c != e.to)
        .collect()
}

/// The two ends of the tunnel an arrow passes through (none for any other arrow): the arrow
/// belongs to them as to its own ends (a card moved, the arrows it holds move with it).
pub fn ends<'m>(model: &'m Model, e: &Edge) -> Vec<&'m str> {
    tunnel_of(model, e).map_or_else(Vec::new, |t| vec![t.from.as_str(), t.to.as_str()])
}

/// The tunnel whose tube arrow `id` runs in, if any.
pub fn tube_of<'m>(model: &'m Model, id: &str) -> Option<&'m str> {
    let e = model.edges.iter().find(|e| e.id == id)?;
    if e.tunnel {
        return Some(e.id.as_str());
    }
    tunnel_of(model, e).map(|t| t.id.as_str())
}

/// A tunnel's label set beside its tube rather than on it, where it would hide the lanes: moved
/// across the segment it sits on by half the tube and half the label, plus a gap.
pub fn label_beside(
    route: &[(f64, f64)],
    at: (f64, f64),
    tube: f64,
    (lw, lh): (f64, f64),
) -> (f64, f64) {
    let Some((a, b)) = route
        .windows(2)
        .map(|w| (w[0], w[1]))
        .min_by(|(a, b), (c, d)| {
            let dist = |p: (f64, f64), q: (f64, f64)| {
                let (mx, my) = ((p.0 + q.0) / 2.0, (p.1 + q.1) / 2.0);
                (mx - at.0).hypot(my - at.1)
            };
            dist(*a, *b).total_cmp(&dist(*c, *d))
        })
    else {
        return at;
    };
    let gap = 4.0;
    if (a.1 - b.1).abs() < 0.5 {
        // along a row: above it
        (at.0, a.1 - tube / 2.0 - lh / 2.0 - gap)
    } else {
        // along a column: on its right
        (a.0 + tube / 2.0 + lw / 2.0 + gap, at.1)
    }
}

/// What the layout gave, which the legs are sewn back into.
pub struct Laid<'a> {
    pub routes: &'a mut BTreeMap<String, Points>,
    pub labels: &'a mut BTreeMap<String, (f64, f64)>,
    pub label_lines: &'a mut BTreeMap<String, Vec<String>>,
    pub edge_groups: &'a mut BTreeMap<String, String>,
}

/// After the layout: the legs sewn back into their arrows (`merge`), dropped from the scene, and
/// each tunnel's label set beside its tube; returns how many lanes each tube holds.
pub fn sew_back(
    plan: &Plan,
    model: &Model,
    scene: &mut Scene,
    sizes: &Sizes,
    boxes: &BTreeMap<String, Box2>,
    laid: Laid<'_>,
) -> BTreeMap<String, usize> {
    let legs: Vec<String> = leg_ids(plan).map(|(l, _)| l.to_string()).collect();
    for leg in legs {
        scene.edges.remove(&leg);
    }
    let Laid {
        routes,
        labels,
        label_lines,
        edge_groups,
    } = laid;
    let tubes = merge(plan, routes, boxes, labels, label_lines, edge_groups);
    for e in model.edges.iter().filter(|e| e.tunnel) {
        let (Some(r), Some(at), Some(sz), Some(el)) = (
            routes.get(&e.id),
            labels.get(&e.id).copied(),
            sizes.labels.get(&e.id),
            scene.edges.get(&e.id),
        ) else {
            continue;
        };
        let base = scene.style(el.line).length("stroke-width").unwrap_or(16.0);
        let lanes = tubes.get(&e.id).copied().unwrap_or(1).max(1);
        let tube = base + (lanes - 1) as f64 * GAP;
        labels.insert(e.id.clone(), label_beside(r, at, tube, (sz.w, sz.h)));
    }
    tubes
}
