//! Milestone R2 — looks: spreading of the ports (R-09) and spacing of parallel segments (R-08),
//! as a post-processing of the routes.
//!
//! Idea taken from Wybrow, Marriott and Stuckey, *Orthogonal Connector Routing* (GD 2009, §4):
//! the segments that share a channel are **ordered** to avoid crossings (by the direction each
//! route takes at its ends), then **spread** by an ideal gap, within the free space of the
//! channel. Here the separation is greedy and symmetric (no quadratic programming): enough for
//! the diagrams aimed at.

use std::collections::BTreeMap;

use crate::geom::{Point, Rect, Side};
use crate::model::{Connector, End, Params, Route};
use crate::simplify::merge;

const EPS: f64 = 1e-6;

/// Side of `r` that `p` is on (border within `EPS`).
fn side_of(r: &Rect, p: Point) -> Option<Side> {
    let on_x = p.0 >= r.x0() - 0.01 && p.0 <= r.x1() + 0.01;
    let on_y = p.1 >= r.y0() - 0.01 && p.1 <= r.y1() + 0.01;
    if on_x && (p.1 - r.y0()).abs() < 0.01 {
        Some(Side::Top)
    } else if on_x && (p.1 - r.y1()).abs() < 0.01 {
        Some(Side::Bottom)
    } else if on_y && (p.0 - r.x0()).abs() < 0.01 {
        Some(Side::Left)
    } else if on_y && (p.0 - r.x1()).abs() < 0.01 {
        Some(Side::Right)
    } else {
        None
    }
}

fn horizontal_side(s: Side) -> bool {
    matches!(s, Side::Top | Side::Bottom)
}

/// Coordinate "along the side" (x for top/bottom, y for left/right).
fn along(p: Point, s: Side) -> f64 {
    if horizontal_side(s) { p.0 } else { p.1 }
}

fn set_along(p: &mut Point, s: Side, v: f64) {
    if horizontal_side(s) {
        p.0 = v;
    } else {
        p.1 = v;
    }
}

struct PortRef {
    route: usize,
    at_start: bool,
    side: Side,
    rect: Rect,
}

/// Order key along the side: (direction of the first turn, signed depth, far coordinate).
fn port_key(pts: &[Point], at_start: bool, side: Side) -> (i8, f64, f64) {
    let seq: Vec<Point> = if at_start {
        pts.to_vec()
    } else {
        pts.iter().rev().copied().collect()
    };
    let a0 = along(seq[0], side);
    let far = along(*seq.last().unwrap_or(&seq[0]), side);
    if seq.len() < 3 {
        return (0, 0.0, far);
    }
    let depth = if horizontal_side(side) {
        (seq[1].1 - seq[0].1).abs()
    } else {
        (seq[1].0 - seq[0].0).abs()
    };
    let turn = along(seq[2], side) - along(seq[1], side);
    let _ = a0;
    // order along the side: direction of the first turn, then destination, then the route that turns
    // furthest out; `uncross_ports` then fixes the pairs that cross. The depths of the turns are
    // then chosen by the spacing (R-08).
    if turn < -EPS {
        (-1, far, depth)
    } else if turn > EPS {
        (1, far, -depth)
    } else {
        (0, far, 0.0)
    }
}

/// Groups the items of an ordered list into "slots": the items of one hyperedge group (R-18)
/// share a slot; the others have one each. The slots are in the order of their first item.
fn slots<T: Copy>(items: &[T], group_of: impl Fn(T) -> Option<String>) -> Vec<Vec<T>> {
    let mut out: Vec<(Option<String>, Vec<T>)> = Vec::new();
    for &it in items {
        let g = group_of(it);
        match g
            .as_ref()
            .and_then(|g| out.iter_mut().find(|(og, _)| og.as_ref() == Some(g)))
        {
            Some((_, members)) => members.push(it),
            None => out.push((g, vec![it])),
        }
    }
    out.into_iter().map(|(_, m)| m).collect()
}

/// Ports of the routes, grouped by (shape, side).
fn port_groups(
    routes: &[Route],
    conns: &BTreeMap<&str, &Connector>,
    shapes: &BTreeMap<&str, Rect>,
) -> BTreeMap<(String, u8), Vec<PortRef>> {
    let mut groups: BTreeMap<(String, u8), Vec<PortRef>> = BTreeMap::new();
    for (ri, r) in routes.iter().enumerate() {
        let Some(c) = conns.get(r.id.as_str()) else {
            continue;
        };
        if r.points.len() < 2 {
            continue;
        }
        for (at_start, end, p) in [
            (true, &c.from, r.points[0]),
            (false, &c.to, *r.points.last().unwrap_or(&r.points[0])),
        ] {
            let End::Shape { id, .. } = end else { continue };
            let Some(rect) = shapes.get(id.as_str()) else {
                continue;
            };
            let Some(side) = side_of(rect, p) else {
                continue;
            };
            groups
                .entry((id.clone(), side as u8))
                .or_default()
                .push(PortRef {
                    route: ri,
                    at_start,
                    side,
                    rect: *rect,
                });
        }
    }
    by_life(groups, routes, conns)
}

/// Anchoring (D-091): slots placed so that a line to a card alone on its facing side runs
/// straight into that card's middle. Off (user, 2026-11-06: every line in the middle of its
/// side, spread evenly and symmetrically, rather than aligned on the cards in front).
const ANCHOR: bool = false;

/// The lines leave from the middle half of a side (D-092), as a hand drawing does: a quarter of
/// the side is kept clear at each end. Within the room clear of the shapes in front
/// (`spread_room`). When the ports do not fit there at `port_spacing`, the middle part widens
/// evenly just enough to hold them (a card a little lower keeps nearly the same gaps, instead
/// of jumping to the whole side); when even the room cannot hold them, that room as it is.
fn middle_half(
    (lo, hi, margin): (f64, f64, f64),
    (side_lo, side_hi): (f64, f64),
    n: usize,
    params: &Params,
) -> (f64, f64, f64) {
    let quarter = (side_hi - side_lo) / 4.0;
    let (a, b) = (
        (lo + margin).max(side_lo + quarter),
        (hi - margin).min(side_hi - quarter),
    );
    let need = (n as f64 - 1.0) * params.port_spacing;
    if b - a >= need {
        return (a, b, 0.0);
    }
    let (room_lo, room_hi) = (lo + margin, hi - margin);
    if room_hi - room_lo < need {
        return (lo, hi, margin);
    }
    // widen around the middle part's centre, shifted back inside the room if it overflows
    let mid = f64::midpoint(a, b);
    let mut wa = mid - need / 2.0;
    wa = wa.max(room_lo).min(room_hi - need);
    (wa, wa + need, 0.0)
}

/// The ports of a side split by the connectors' lives (`Connector::life`): ports of connectors
/// never shown together form separate groups, as if they stood on different sides.
fn by_life(
    groups: BTreeMap<(String, u8), Vec<PortRef>>,
    routes: &[Route],
    conns: &BTreeMap<&str, &Connector>,
) -> BTreeMap<(String, u8), Vec<PortRef>> {
    let life = |p: &PortRef| {
        conns
            .get(routes[p.route].id.as_str())
            .and_then(|c| c.life)
            .unwrap_or((f64::NEG_INFINITY, f64::INFINITY))
    };
    let mut out = BTreeMap::new();
    for ((id, side), mut ports) in groups {
        ports.sort_by(|a, b| life(a).0.total_cmp(&life(b).0));
        // sweep by start: a port starting after every earlier one has ended opens a new era
        let (mut era, mut end) = (0, f64::NEG_INFINITY);
        for p in ports {
            let (a, z) = life(&p);
            if a > end && end > f64::NEG_INFINITY {
                era += 1;
            }
            end = end.max(z);
            let key = if era == 0 {
                id.clone()
            } else {
                format!("{id}\u{0}{era}")
            };
            out.entry((key, side)).or_insert_with(Vec::new).push(p);
        }
    }
    out
}

/// End segment of a route at one of its ports: (orientation is vertical, line coordinate, extent).
fn end_segment(pts: &[Point], at_start: bool) -> Option<(bool, f64, f64, f64)> {
    let n = pts.len();
    if n < 2 {
        return None;
    }
    let at = |k: usize| if at_start { pts[k] } else { pts[n - 1 - k] };
    let (a, b) = (at(0), at(1));
    let vertical = (a.0 - b.0).abs() < EPS;
    if !vertical && (a.1 - b.1).abs() >= EPS {
        return None;
    }
    // through the points that stay on the line (routes are not merged yet)
    let mut k = 1;
    while k + 1 < n
        && (if vertical {
            (at(k + 1).0 - a.0).abs()
        } else {
            (at(k + 1).1 - a.1).abs()
        }) < EPS
    {
        k += 1;
    }
    let far = at(k);
    Some(if vertical {
        (true, a.0, a.1.min(far.1), a.1.max(far.1))
    } else {
        (false, a.1, a.0.min(far.0), a.0.max(far.0))
    })
}

/// Two routes whose end segments lie on one line and overlap read as a single line (an arrow
/// leaving a card where another arrives at the card below it, two cards aligned). The arrival
/// (or else the later route) moves along its side, away from the side's middle, to the nearest
/// place at least one spacing from the other ports of that side. Hyperedge branches share
/// their ports on purpose and are left alone.
fn separate_end_overlaps(
    routes: &mut [Route],
    conns: &BTreeMap<&str, &Connector>,
    shapes: &BTreeMap<&str, Rect>,
    params: &Params,
    groups_of: &[Option<String>],
) {
    let step = params.ideal_nudging.max(params.port_spacing);
    for _ in 0..8 {
        let groups = port_groups(routes, conns, shapes);
        let ports: Vec<(&(String, u8), &PortRef)> = groups
            .iter()
            .flat_map(|(k, ps)| ps.iter().map(move |p| (k, p)))
            .collect();
        let seg = |p: &PortRef| end_segment(&routes[p.route].points, p.at_start);
        let clash = ports.iter().enumerate().find_map(|(i, (_, a))| {
            ports[i + 1..].iter().find_map(|(kb, b)| {
                let same_group =
                    groups_of[a.route].is_some() && groups_of[a.route] == groups_of[b.route];
                let (Some(sa), Some(sb)) = (seg(a), seg(b)) else {
                    return None;
                };
                let overlap = sa.0 == sb.0
                    && (sa.1 - sb.1).abs() < 1.0
                    && sa.3.min(sb.3) - sa.2.max(sb.2) > 1.0;
                // two arrows never shown together may share a port (`Connector::life`)
                let life = |p: &PortRef| {
                    conns
                        .get(routes[p.route].id.as_str())
                        .and_then(|c| c.life)
                        .unwrap_or((f64::NEG_INFINITY, f64::INFINITY))
                };
                let (la, lb) = (life(a), life(b));
                let together = la.0 <= lb.1 && lb.0 <= la.1;
                (a.route != b.route && !same_group && overlap && together).then(|| {
                    // move the arrival if one of them is one, else the later route
                    if !a.at_start && b.at_start {
                        (ports[i].0, *a)
                    } else {
                        (*kb, *b)
                    }
                })
            })
        });
        let Some((key, p)) = clash else { return };
        let (lo, hi) = if horizontal_side(p.side) {
            (p.rect.x0(), p.rect.x1())
        } else {
            (p.rect.y0(), p.rect.y1())
        };
        let margin = ((hi - lo) / 4.0).min(params.port_spacing.max(6.0));
        let pts = &routes[p.route].points;
        let cur = along(
            if p.at_start {
                pts[0]
            } else {
                pts[pts.len() - 1]
            },
            p.side,
        );
        let others: Vec<f64> = groups[key]
            .iter()
            .filter(|q| !(q.route == p.route && q.at_start == p.at_start))
            .map(|q| {
                let qp = &routes[q.route].points;
                along(if q.at_start { qp[0] } else { qp[qp.len() - 1] }, q.side)
            })
            .collect();
        let away = if cur >= (lo + hi) / 2.0 { 1.0 } else { -1.0 };
        let free = |t: f64| {
            t >= lo + margin
                && t <= hi - margin
                && others.iter().all(|o| (o - t).abs() >= step - EPS)
        };
        // nor through a shape: the leg that moves with the port can be long (the private map's
        // `index`, a line moved into the card beside its straight run)
        let hit_now = shapes_hit(pts, shapes);
        let clear = |t: f64| {
            let mut moved = pts.clone();
            move_port(&mut moved, p.at_start, p.side, t);
            shapes_hit(&moved, shapes) <= hit_now
        };
        let target = (1..64)
            .flat_map(|k| {
                [
                    cur + away * step * f64::from(k),
                    cur - away * step * f64::from(k),
                ]
            })
            .find(|t| free(*t) && clear(*t));
        let Some(t) = target else { return };
        let (route, at_start, side) = (p.route, p.at_start, p.side);
        move_port(&mut routes[route].points, at_start, side, t);
    }
}

/// R-09: spreads along each side the ports that share it.
fn spread_ports(
    routes: &mut [Route],
    conns: &BTreeMap<&str, &Connector>,
    shapes: &BTreeMap<&str, Rect>,
    params: &Params,
) {
    let groups = port_groups(routes, conns, shapes);
    let bundled = spread_bundles(routes, &groups, params);
    // ports alone on their side: (route, start?) → rectangle of the shape
    let mut lone: BTreeMap<(usize, bool), Rect> = BTreeMap::new();
    for ports in groups.values() {
        if ports.len() == 1 {
            lone.insert((ports[0].route, ports[0].at_start), ports[0].rect);
        }
    }
    for ((_, _), mut ports) in groups {
        if ports.len() < 2 || ports.iter().all(|p| bundled.contains(&p.route)) {
            continue;
        }
        let side = ports[0].side;
        let rect = ports[0].rect;
        ports.sort_by(|a, b| {
            let ka = port_key(&routes[a.route].points, a.at_start, side);
            let kb = port_key(&routes[b.route].points, b.at_start, side);
            ka.0.cmp(&kb.0)
                .then(ka.1.total_cmp(&kb.1))
                .then(ka.2.total_cmp(&kb.2))
                .then(a.route.cmp(&b.route))
        });
        let (lo, hi) = if horizontal_side(side) {
            (rect.x0(), rect.x1())
        } else {
            (rect.y0(), rect.y1())
        };
        let mid = (lo + hi) / 2.0;
        let (side_lo, side_hi) = (lo, hi);
        let margin = ((hi - lo) / 4.0).min(params.port_spacing.max(6.0));
        let group_of = |i: usize| {
            conns
                .get(routes[ports[i].route].id.as_str())
                .and_then(|c| c.group.clone())
        };
        let order: Vec<usize> = (0..ports.len()).collect();
        let slotted = slots(&order, group_of);
        // the part of the side no shape stands in front of, down to where the ends turn
        let depth = ports
            .iter()
            .filter_map(|p| first_turn_depth(&routes[p.route].points, p.at_start))
            .fold(0.0, f64::max);
        let Some((lo, hi, margin)) = spread_room(
            shapes,
            (&rect, side, depth),
            (lo, hi),
            margin,
            slotted.len(),
            params,
        ) else {
            // no room clear of the shapes in front: the ports stay where the search put them
            // rather than spread into a neighbour (a card 20 px beside)
            continue;
        };
        let (lo, hi, margin) =
            middle_half((lo, hi, margin), (side_lo, side_hi), slotted.len(), params);
        let targets = ANCHOR
            .then(|| {
                anchored_targets(
                    routes,
                    (&ports, &slotted),
                    &lone,
                    (lo, hi, margin),
                    mid,
                    params,
                )
            })
            .flatten()
            .unwrap_or_else(|| even_targets(slotted.len(), (lo, hi, margin), mid, params));
        let assigned: Vec<(usize, f64)> = slotted
            .iter()
            .zip(&targets)
            .flat_map(|(members, &t)| members.iter().map(move |&i| (i, t)))
            .collect();
        for (i, target) in assigned {
            let pr = &ports[i];
            let pts = &mut routes[pr.route].points;
            move_port(pts, pr.at_start, side, target);
        }
    }
    centre_lone_ends(routes, &lone, &bundled, shapes, conns);
}

/// An end alone on its side goes to its middle
/// (D-091, D-093): every line lands in the middle of a side, bending if it must; a move that
/// would cross a shape does not happen (R-02).
fn centre_lone_ends(
    routes: &mut [Route],
    lone: &BTreeMap<(usize, bool), Rect>,
    bundled: &std::collections::BTreeSet<usize>,
    shapes: &BTreeMap<&str, Rect>,
    conns: &BTreeMap<&str, &Connector>,
) {
    for (&(route, at_start), rect) in lone {
        if bundled.contains(&route) {
            continue;
        }
        let pts = &routes[route].points;
        if pts.len() < 2 {
            continue;
        }
        let end = if at_start { pts[0] } else { pts[pts.len() - 1] };
        let Some(side) = side_of(rect, end) else {
            continue;
        };
        let (lo, hi, v) = if horizontal_side(side) {
            (rect.x0(), rect.x1(), end.0)
        } else {
            (rect.y0(), rect.y1(), end.1)
        };
        let mid = f64::midpoint(lo, hi);
        if (v - mid).abs() < 0.5 {
            continue;
        }
        let mut moved = pts.clone();
        move_port(&mut moved, at_start, side, mid);
        // no segment through a shape (its own ends included, R-02), but what the connector
        // ignores (a card of another step)
        let conn = conns.get(routes[route].id.as_str());
        let free = |id: &str| conn.is_some_and(|c| c.ignore.iter().any(|p| p == id));
        let hits = shapes
            .iter()
            .any(|(id, r)| !free(id) && moved.windows(2).any(|w| segment_hits(w[0], w[1], r)));
        if !hits {
            routes[route].points = moved;
        }
    }
}

/// Straight lines that are all the ports of one side and all the ports of the facing side (arrows
/// both ways between two cards): spread together, both ends moved, around their current middle and
/// within both sides, so that they stay straight. Returns the routes so placed.
fn spread_bundles(
    routes: &mut [Route],
    groups: &BTreeMap<(String, u8), Vec<PortRef>>,
    params: &Params,
) -> std::collections::BTreeSet<usize> {
    let mut done = std::collections::BTreeSet::new();
    let key_of = |route: usize, at_start: bool| {
        groups
            .iter()
            .find(|(_, ps)| {
                ps.iter()
                    .any(|p| p.route == route && p.at_start == at_start)
            })
            .map(|(k, _)| k)
    };
    for (key, ports) in groups {
        if ports.len() < 2
            || ports
                .iter()
                .any(|p| routes[p.route].points.len() != 2 || done.contains(&p.route))
        {
            continue;
        }
        let far: Vec<_> = ports.iter().map(|p| key_of(p.route, !p.at_start)).collect();
        let Some(Some(fk)) = far.first().copied() else {
            continue;
        };
        if fk == key || far.iter().any(|k| *k != Some(fk)) || groups[fk].len() != ports.len() {
            continue;
        }
        let side = ports[0].side;
        let span = |r: &Rect| {
            let (lo, hi) = if horizontal_side(side) {
                (r.x0(), r.x1())
            } else {
                (r.y0(), r.y1())
            };
            let m = ((hi - lo) / 4.0).min(params.port_spacing.max(6.0));
            (lo + m, hi - m)
        };
        let ((alo, ahi), (blo, bhi)) = (span(&ports[0].rect), span(&groups[fk][0].rect));
        let (lo, hi) = (alo.max(blo), ahi.min(bhi));
        let n = ports.len() as f64;
        let step = params.ideal_nudging.max(params.port_spacing);
        if (n - 1.0) * step > hi - lo {
            continue;
        }
        let mut order: Vec<(usize, f64)> = ports
            .iter()
            .map(|p| (p.route, along(routes[p.route].points[0], side)))
            .collect();
        order.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)));
        let mean = order.iter().map(|o| o.1).sum::<f64>() / n;
        let half = (n - 1.0) * step / 2.0;
        let center = mean.clamp(lo + half, hi - half);
        for (k, (route, _)) in order.iter().enumerate() {
            let t = center + (k as f64 - (n - 1.0) / 2.0) * step;
            for q in &mut routes[*route].points {
                set_along(q, side, t);
            }
            done.insert(*route);
        }
    }
    done
}

/// Last, on the final routes (corridors centred, parallel segments spread): two neighbouring ports of a side swap places when that
/// removes crossings (the order key reads the routes before centring, so it can be wrong).
/// Ports of a hyperedge group never move; the other ports of their side still may.
fn uncross_ports(
    routes: &mut [Route],
    conns: &BTreeMap<&str, &Connector>,
    shapes: &BTreeMap<&str, Rect>,
    groups_of: &[Option<String>],
) {
    let total = |routes: &[Route], ri: usize| tangles(routes, conns, groups_of, ri);
    for ports in port_groups(routes, conns, shapes).into_values() {
        // the shared end of a group (a fork's start, a join's arrival: two of its routes on this
        // side) stays; a branch's own end moves like any other (a swap is kept only if it removes
        // crossings, the grouped routes' included)
        let shared = |p: &PortRef| {
            groups_of[p.route].as_ref().is_some_and(|g| {
                ports
                    .iter()
                    .any(|q| q.route != p.route && groups_of[q.route].as_ref() == Some(g))
            })
        };
        let keep: Vec<bool> = ports.iter().map(|p| !shared(p)).collect();
        let ports: Vec<PortRef> = ports
            .into_iter()
            .zip(keep)
            .filter_map(|(p, k)| k.then_some(p))
            .collect();
        if ports.len() < 2 {
            continue;
        }
        let side = ports[0].side;
        let at = |routes: &[Route], p: &PortRef| {
            let pts = &routes[p.route].points;
            along(
                if p.at_start {
                    pts[0]
                } else {
                    pts[pts.len() - 1]
                },
                side,
            )
        };
        let mut sorted = ports;
        sorted.sort_by(|a, b| at(routes, a).total_cmp(&at(routes, b)));
        for w in sorted.windows(2) {
            let (a, b) = (&w[0], &w[1]);
            if a.route == b.route {
                continue;
            }
            let (ta, tb) = (at(routes, a), at(routes, b));
            let before = total(routes, a.route) + total(routes, b.route);
            if before == 0 {
                continue;
            }
            let (pa, pb) = (
                routes[a.route].points.clone(),
                routes[b.route].points.clone(),
            );
            // three ways out, the best kept if it removes crossings: swap the ports; swap the
            // levels of the first turns (crossing where the two arrows turn); both
            let mut best: Option<(usize, Vec<Point>, Vec<Point>)> = None;
            for (ports, turns) in [(true, false), (false, true), (true, true)] {
                routes[a.route].points = pa.clone();
                routes[b.route].points = pb.clone();
                if ports && !swap_ports(routes, (a, b), (ta, tb), shapes) {
                    continue;
                }
                // a grouped route's turn is its group's shared line: only its port may move
                let grouped = groups_of[a.route].is_some() || groups_of[b.route].is_some();
                if turns && (grouped || !swap_turns(routes, a, b, shapes)) {
                    continue;
                }
                let n = total(routes, a.route) + total(routes, b.route);
                if n < before && best.as_ref().is_none_or(|b| n < b.0) {
                    best = Some((
                        n,
                        routes[a.route].points.clone(),
                        routes[b.route].points.clone(),
                    ));
                }
                if n == 0 {
                    break;
                }
            }
            let (na, nb) = best.map_or((pa, pb), |(_, x, y)| (x, y));
            routes[a.route].points = na;
            routes[b.route].points = nb;
        }
    }
}

/// Swaps the ports of `a` and `b` (at `ta` and `tb` along their side); `false` when a moved end
/// would then cross a shape (a port moved under a neighbouring card drew its line through it).
fn swap_ports(
    routes: &mut [Route],
    (a, b): (&PortRef, &PortRef),
    (ta, tb): (f64, f64),
    shapes: &BTreeMap<&str, Rect>,
) -> bool {
    let (pa, pb) = (
        routes[a.route].points.clone(),
        routes[b.route].points.clone(),
    );
    move_port(&mut routes[a.route].points, a.at_start, a.side, tb);
    move_port(&mut routes[b.route].points, b.at_start, b.side, ta);
    // every segment, not only the end's: the run that moves with a port can be long (a repeated
    // point hid it from `end_clear`, and a whole leg moved through a card: the private map's
    // `hypershift`)
    let through = |pts: &[Point]| shapes_hit(pts, shapes);
    end_clear(&routes[a.route].points, a.at_start, shapes)
        && end_clear(&routes[b.route].points, b.at_start, shapes)
        && through(&routes[a.route].points) <= through(&pa)
        && through(&routes[b.route].points) <= through(&pb)
}

/// The shapes a route passes through, its own end shapes left out.
fn shapes_hit(pts: &[Point], shapes: &BTreeMap<&str, Rect>) -> usize {
    let n = pts.len();
    if n < 2 {
        return 0;
    }
    shapes
        .values()
        .filter(|r| !clear_between(pts, (0, n - 1), &BTreeMap::from([("", **r)])))
        .count()
}

/// Index of the first bend after the port and of the corner after it, when the route leaves the
/// port straight then turns, and turns again before its other end (at least four points): with
/// three, the corner after the turn is the other end, and moving it took the arrow off its card
/// (bug 14 of the private corpus: an arrow ending on the corner of the card below its target).
fn first_turn(pts: &[Point], at_start: bool) -> Option<(usize, usize)> {
    if pts.len() < 4 {
        return None;
    }
    Some(if at_start {
        (1, 2)
    } else {
        (pts.len() - 2, pts.len() - 3)
    })
}

/// Swaps the levels (distance from the side) of the first turns of two routes leaving one side,
/// keeping every segment axis-aligned. `false` (nothing changed) when either route has no such
/// turn or a moved segment would cross a shape other than the route's own ends.
fn swap_turns(
    routes: &mut [Route],
    a: &PortRef,
    b: &PortRef,
    shapes: &BTreeMap<&str, Rect>,
) -> bool {
    let side = a.side;
    let level = |p: Point| if horizontal_side(side) { p.1 } else { p.0 };
    let (Some((a1, a2)), Some((b1, b2))) = (
        first_turn(&routes[a.route].points, a.at_start),
        first_turn(&routes[b.route].points, b.at_start),
    ) else {
        return false;
    };
    let (pa, pb) = (&routes[a.route].points, &routes[b.route].points);
    // both corners of each first turn sit at one level (an axis-aligned leg along the side)
    if (level(pa[a1]) - level(pa[a2])).abs() > EPS || (level(pb[b1]) - level(pb[b2])).abs() > EPS {
        return false;
    }
    let (la, lb) = (level(pa[a1]), level(pb[b1]));
    let set = |pts: &mut Vec<Point>, i: usize, j: usize, v: f64| {
        for k in [i, j] {
            if horizontal_side(side) {
                pts[k].1 = v;
            } else {
                pts[k].0 = v;
            }
        }
    };
    let (mut na, mut nb) = (pa.clone(), pb.clone());
    set(&mut na, a1, a2, lb);
    set(&mut nb, b1, b2, la);
    // only the segments that moved: around the two corners of the turn
    let clear = |pts: &[Point], i: usize, j: usize| {
        let lo = i.min(j).saturating_sub(1);
        let hi = (i.max(j) + 1).min(pts.len() - 1);
        clear_between(pts, (lo, hi), shapes)
    };
    if !clear(&na, a1, a2) || !clear(&nb, b1, b2) {
        return false;
    }
    // a "turn" whose next corner is only a stub in line with it (the end): moving the leg would
    // leave that stub on a slant
    if !crate::hint::is_orthogonal(&na) || !crate::hint::is_orthogonal(&nb) {
        return false;
    }
    routes[a.route].points = na;
    routes[b.route].points = nb;
    true
}

/// Segments `lo..hi` of a route cross no shape. A shape holding an end inside (a region around
/// it) is crossed anyway; the end shapes themselves, whose borders carry the ports, are not.
fn clear_between(pts: &[Point], (lo, hi): (usize, usize), shapes: &BTreeMap<&str, Rect>) -> bool {
    let (s, e) = (pts[0], pts[pts.len() - 1]);
    shapes.values().all(|r| {
        let own = |p: Point| {
            p.0 > r.x + EPS && p.0 < r.x + r.w - EPS && p.1 > r.y + EPS && p.1 < r.y + r.h - EPS
        };
        own(s) || own(e) || (lo..hi).all(|k| !segment_hits(pts[k], pts[k + 1], r))
    })
}

/// The segments a port's move changed (the end's leg and the one after its first turn) cross no
/// shape.
fn end_clear(pts: &[Point], at_start: bool, shapes: &BTreeMap<&str, Rect>) -> bool {
    let n = pts.len();
    if n < 2 {
        return true;
    }
    let (lo, hi) = if at_start {
        (0, 2.min(n - 1))
    } else {
        (n.saturating_sub(3), n - 1)
    };
    clear_between(pts, (lo, hi), shapes)
}

/// Does the axis-aligned segment `p`–`q` pass through the inside of `r`?
fn segment_hits(p: Point, q: Point, r: &Rect) -> bool {
    let (x0, x1) = (p.0.min(q.0), p.0.max(q.0));
    let (y0, y1) = (p.1.min(q.1), p.1.max(q.1));
    x1 > r.x + EPS && x0 < r.x + r.w - EPS && y1 > r.y + EPS && y0 < r.y + r.h - EPS
}

/// How far the end segment at a port runs before its first turn (the whole route if straight).
/// A point in line with the end (where the port's stub ends) is no turn: the depth stopped there
/// left a zone title just beyond the stub out of the strip, and ports spread under it. In line
/// within `IN_LINE`: the graph's points are rounded to 1e-4, a port is not.
fn first_turn_depth(pts: &[Point], at_start: bool) -> Option<f64> {
    const IN_LINE: f64 = 0.01;
    let at = |k: usize| {
        if at_start {
            pts.get(k)
        } else {
            pts.len().checked_sub(k + 1).and_then(|i| pts.get(i))
        }
    };
    let a = *at(0)?;
    let mut b = *at(1)?;
    let vertical = (a.0 - b.0).abs() <= (a.1 - b.1).abs();
    let mut k = 2;
    while let Some(q) = at(k) {
        let off = if vertical { q.0 - a.0 } else { q.1 - a.1 };
        if off.abs() > IN_LINE {
            break;
        }
        b = *q;
        k += 1;
    }
    Some((a.0 - b.0).abs() + (a.1 - b.1).abs())
}

/// The widest part of side `[lo, hi]` of `rect` that no other shape stands in front of, within
/// `depth` of the side and `pad` to either hand (a zone's title or tag over a card),
/// widened by `margin` back to the side's own ends; `None` when nothing stands in front.
/// Where the `n` ports of a side spread (`lo`, `hi` and the margin to keep inside): the whole side
/// when nothing stands in front of it, else the widest clear part that holds them, a shape's margin narrowed (half, then a quarter) when
/// the corridor is tight; `None` when even that does not hold them.
fn spread_room(
    shapes: &BTreeMap<&str, Rect>,
    strip: (&Rect, Side, f64),
    (lo, hi): (f64, f64),
    margin: f64,
    n: usize,
    params: &Params,
) -> Option<(f64, f64, f64)> {
    let b = params.shape_buffer;
    for pad in [b, b / 2.0, b / 4.0] {
        let (a, b) = match clear_part(shapes, strip, (lo, hi), margin, pad) {
            Clear::Whole => return Some((lo, hi, margin)),
            // shapes all along the side: no part is clear at this margin (it was taken for a
            // side with nothing in front, and the ports spread over the shapes: the private map's
            // `mce/installation`, a line through the card beside its straight run)
            Clear::None => continue,
            Clear::Part(a, b) => (a, b),
        };
        // the side's own ends keep `margin`; a shape's edge is already `pad` away
        let a = if a <= lo + EPS { a + margin } else { a };
        let b = if b >= hi - EPS { b - margin } else { b };
        if (n as f64 - 1.0) * params.port_spacing <= b - a {
            return Some((a, b, 0.0));
        }
    }
    None
}

/// What of a side is clear of the shapes in front of it.
enum Clear {
    /// No shape stands in front.
    Whole,
    /// Shapes stand all along it.
    None,
    /// The widest clear part.
    Part(f64, f64),
}

fn clear_part(
    shapes: &BTreeMap<&str, Rect>,
    (rect, side, depth): (&Rect, Side, f64),
    (lo, hi): (f64, f64),
    margin: f64,
    pad: f64,
) -> Clear {
    let v = side.outward().vector();
    let edge = side.point(rect, 0.0);
    // the strip in front of the side, `depth` deep
    let (s0, s1) = if horizontal_side(side) {
        let y = edge.1 + v.1 * depth;
        (edge.1.min(y), edge.1.max(y))
    } else {
        let x = edge.0 + v.0 * depth;
        (edge.0.min(x), edge.0.max(x))
    };
    let mut blocked: Vec<(f64, f64)> = shapes
        .values()
        .filter(|r| *r != rect)
        .filter_map(|r| {
            let (a0, a1, b0, b1) = if horizontal_side(side) {
                (r.x0(), r.x1(), r.y0(), r.y1())
            } else {
                (r.y0(), r.y1(), r.x0(), r.x1())
            };
            (b0 < s1 - EPS && b1 > s0 + EPS && a0 < hi && a1 > lo).then_some((a0 - pad, a1 + pad))
        })
        .collect();
    if blocked.is_empty() {
        return Clear::Whole;
    }
    blocked.sort_by(|a, b| a.0.total_cmp(&b.0));
    // the free stretches between the blocked ones, inside the side's margins
    let mut free = Vec::new();
    let mut at = lo + margin;
    for (b0, b1) in blocked {
        if b0 > at {
            free.push((at, b0.min(hi - margin)));
        }
        at = at.max(b1);
    }
    if at < hi - margin {
        free.push((at, hi - margin));
    }
    // widened by `margin` back to the side's own ends only: a shape's edge stays a bound
    free.into_iter()
        .filter(|(a, b)| b > a)
        .max_by(|x, y| (x.1 - x.0).total_cmp(&(y.1 - y.0)))
        .map_or(Clear::None, |(a, b)| {
            let a = if a <= lo + margin + EPS {
                a - margin
            } else {
                a
            };
            let b = if b >= hi - margin - EPS {
                b + margin
            } else {
                b
            };
            Clear::Part(a, b)
        })
}

/// Targets of the slots of a side when they are anchored (D-091, D-092): a line whose other end
/// is alone on its side, facing this one, is drawn from the middle of that other side, so its
/// slot sits there, and the side's slots are spread evenly and symmetrically around the side's
/// middle with the spacing that puts that slot there (two ports: mirror images). Several anchors
/// must agree on that spacing (two cards each centred in their half of a wide card). `None`
/// without an anchor, with anchors that disagree, or when the spacing would be too tight or leave
/// the side.
fn anchored_targets(
    routes: &[Route],
    (ports, slotted): (&[PortRef], &[Vec<usize>]),
    lone: &BTreeMap<(usize, bool), Rect>,
    (lo, hi, margin): (f64, f64, f64),
    mid: f64,
    params: &Params,
) -> Option<Vec<f64>> {
    let n = slotted.len() as f64;
    let spacings: Vec<f64> = slotted
        .iter()
        .enumerate()
        .filter_map(|(k, members)| {
            members.iter().find_map(|&i| {
                let pr = &ports[i];
                let far = lone.get(&(pr.route, !pr.at_start))?;
                let c = if horizontal_side(pr.side) {
                    far.center().0
                } else {
                    far.center().1
                };
                // a straight line, or a card that faces the side
                let facing = c >= lo + margin - 0.5 && c <= hi - margin + 0.5;
                if routes[pr.route].points.len() != 2 && !facing {
                    return None;
                }
                let rank = k as f64 - (n - 1.0) / 2.0;
                (rank.abs() >= EPS).then(|| (c - mid) / rank)
            })
        })
        .collect();
    let s = *spacings.first()?;
    if spacings.iter().any(|t| (t - s).abs() > 1.0) {
        return None;
    }
    // the slots keep the order that avoids crossings
    let half = (n - 1.0) / 2.0 * s;
    let fits = s >= params.port_spacing
        && mid - half >= lo + margin - 0.5
        && mid + half <= hi - margin + 0.5;
    fits.then(|| {
        (0..slotted.len())
            .map(|j| mid + (j as f64 - (n - 1.0) / 2.0) * s)
            .collect()
    })
}

fn even_targets(
    n: usize,
    (lo, hi, margin): (f64, f64, f64),
    mid: f64,
    params: &Params,
) -> Vec<f64> {
    let n = n as f64;
    let mut s = params.ideal_nudging.max(params.port_spacing);
    // wider apart on a roomy side, up to `port_spread`: arrows read as distinct, not as a cable;
    // the room is already the middle of the side (`middle_half`)
    s = s.max(
        params
            .port_spread
            .min((hi - lo - 2.0 * margin) / (n - 1.0).max(1.0)),
    );
    if (n - 1.0) * s > (hi - lo) - 2.0 * margin {
        s = (((hi - lo) - 2.0 * margin) / (n - 1.0)).max(0.0);
    }
    // centred on the side's middle, slid only as far as the clear part requires: a shape in
    // front of one end of the side does not pull the ports off the middle
    let half = (n - 1.0) * s / 2.0;
    let (a, b) = (lo + margin + half, hi - margin - half);
    let center = if a <= b {
        mid.clamp(a, b)
    } else {
        (lo + hi) / 2.0
    };
    (0..n as usize)
        .map(|k| center + (k as f64 - (n - 1.0) / 2.0) * s)
        .collect()
}

/// Moves a route's port along its side, keeping the route orthogonal.
fn move_port(pts: &mut Vec<Point>, at_start: bool, side: Side, target: f64) {
    if pts.len() < 2 {
        return;
    }
    let i0 = if at_start { 0 } else { pts.len() - 1 };
    let cur = along(pts[i0], side);
    if (cur - target).abs() < EPS {
        return;
    }
    if pts.len() == 2 {
        // a straight line: a jog is inserted halfway
        let (a, b) = (pts[0], pts[1]);
        let mid = if horizontal_side(side) {
            (a.1 + b.1) / 2.0
        } else {
            (a.0 + b.0) / 2.0
        };
        let mut moved = pts[i0];
        set_along(&mut moved, side, target);
        let (p_near, p_far) = if at_start { (moved, b) } else { (moved, a) };
        let knee = |p: Point| {
            if horizontal_side(side) {
                (p.0, mid)
            } else {
                (mid, p.1)
            }
        };
        let path = vec![p_near, knee(p_near), knee(p_far), p_far];
        *pts = if at_start {
            path
        } else {
            path.into_iter().rev().collect()
        };
        return;
    }
    // The points that follow the port on the same line (a stub, then the rest of a straight
    // run) move with it: moving only the first two would leave a slanted segment.
    let n = pts.len();
    let idx = |j: usize| if at_start { j } else { n - 1 - j };
    let mut run = 1;
    // tolerance: input coordinates may carry single-precision noise (248.19999 vs 248.2)
    let on_line = |p: Point| (along(p, side) - cur).abs() < 0.01;
    while run + 1 < n - 1 && on_line(pts[idx(run + 1)]) {
        run += 1;
    }
    if on_line(pts[idx(n - 1)]) && run == n - 2 {
        // a straight line with intermediate points: same as the plain straight line. `merge`
        // may keep points that are on the line within this tolerance only: then the whole line
        // moves as it is (calling again would never end)
        crate::simplify::merge(pts);
        if pts.len() < n {
            move_port(pts, at_start, side, target);
        } else {
            for q in pts.iter_mut() {
                set_along(q, side, target);
            }
        }
        return;
    }
    for j in 0..=run {
        set_along(&mut pts[idx(j)], side, target);
    }
}

/// A middle segment of a route (neither the first nor the last: they hold to the ports).
struct Seg {
    route: usize,
    k: usize,
    vertical: bool,
    coord: f64,
    lo: f64,
    hi: f64,
    /// A first or last segment: it holds to a port and does not move.
    fixed: bool,
}

fn middle_segments(routes: &[Route], vertical: bool) -> Vec<Seg> {
    segments(routes, vertical, false)
}

/// The middle segments (`ends` false) or the first and last ones (`ends` true, fixed).
fn segments(routes: &[Route], vertical: bool, ends: bool) -> Vec<Seg> {
    let mut out = Vec::new();
    for (ri, r) in routes.iter().enumerate() {
        let p = &r.points;
        let ks: Vec<usize> = if ends {
            match p.len() {
                0 | 1 => Vec::new(),
                2 => vec![0],
                n => vec![0, n - 2],
            }
        } else if p.len() < 4 {
            Vec::new()
        } else {
            (1..p.len() - 2).collect()
        };
        // a middle segment in line with an end one (a stub left apart from the stretch it
        // continues) is that end's line: it moves with it or not at all (moved alone, the stub was
        // left on a slant)
        let in_line = |i: usize, j: usize| {
            let (a, b, c) = (p[i], p[i + 1], p[j + 1]);
            ((a.0 - b.0).abs() < EPS && (b.0 - c.0).abs() < EPS)
                || ((a.1 - b.1).abs() < EPS && (b.1 - c.1).abs() < EPS)
        };
        for k in ks {
            let (a, b) = (p[k], p[k + 1]);
            let is_v = (a.0 - b.0).abs() < EPS && (a.1 - b.1).abs() > EPS;
            let is_h = (a.1 - b.1).abs() < EPS && (a.0 - b.0).abs() > EPS;
            let fixed =
                ends || (k == 1 && in_line(0, 1)) || (k + 3 == p.len() && in_line(k, k + 1));
            if vertical && is_v {
                out.push(Seg {
                    route: ri,
                    k,
                    vertical,
                    coord: a.0,
                    lo: a.1.min(b.1),
                    hi: a.1.max(b.1),
                    fixed,
                });
            } else if !vertical && is_h {
                out.push(Seg {
                    route: ri,
                    k,
                    vertical,
                    coord: a.1,
                    lo: a.0.min(b.0),
                    hi: a.0.max(b.0),
                    fixed,
                });
            }
        }
    }
    out
}

/// Order key in a channel: where the route goes at both its ends.
fn seg_key(routes: &[Route], s: &Seg) -> (i32, f64) {
    let p = &routes[s.route].points;
    // an end segment has nothing before (or after) it: its own end stands in
    let prev = if s.k > 0 { p[s.k - 1] } else { p[s.k] };
    let next = p.get(s.k + 2).copied().unwrap_or(p[s.k + 1]);
    let c = |q: Point| if s.vertical { q.0 } else { q.1 };
    let sign = |v: f64| {
        if v > EPS {
            1
        } else if v < -EPS {
            -1
        } else {
            0
        }
    };
    (
        sign(c(prev) - s.coord) + sign(c(next) - s.coord),
        (c(prev) + c(next)) / 2.0,
    )
}

/// Croisements stricts entre deux polylignes orthogonales.
fn crossings(a: &[Point], b: &[Point]) -> usize {
    // collinear points merged first: a line crossing a route exactly where its stub ends would
    // fall between two segments and go uncounted (seen under a card, 2026-11-09)
    let (mut a, mut b) = (a.to_vec(), b.to_vec());
    crate::simplify::merge(&mut a);
    crate::simplify::merge(&mut b);
    let mut n = 0;
    for w1 in a.windows(2) {
        for w2 in b.windows(2) {
            let (p, q, r, t) = (w1[0], w1[1], w2[0], w2[1]);
            // 0.01 px: coordinates may carry single-precision noise (290.4999 vs 290.5)
            let h1 = (p.1 - q.1).abs() < 0.01;
            let h2 = (r.1 - t.1).abs() < 0.01;
            if h1 == h2 {
                continue;
            }
            let (ha, hb, va, vb) = if h1 { (p, q, r, t) } else { (r, t, p, q) };
            let inside_h = va.0 > ha.0.min(hb.0) + EPS && va.0 < ha.0.max(hb.0) - EPS;
            let inside_v = ha.1 > va.1.min(vb.1) + EPS && ha.1 < va.1.max(vb.1) - EPS;
            if inside_h && inside_v {
                n += 1;
            }
        }
    }
    n
}

/// The crossings of route `ri` with the others, and its lines drawn on an unrelated route's line
/// (bug 22 of the private corpus: a swap that laid one arrow on another's line counted one
/// crossing fewer and was kept); an overlay hides which arrow goes where, so it weighs more than
/// any crossing a swap may remove.
fn tangles(
    routes: &[Route],
    conns: &BTreeMap<&str, &Connector>,
    groups_of: &[Option<String>],
    ri: usize,
) -> usize {
    tangles_over(routes, conns, groups_of, ri, 4.0)
}

/// `tangles` with overlays counted from `min` px on one line.
fn tangles_over(
    routes: &[Route],
    conns: &BTreeMap<&str, &Connector>,
    groups_of: &[Option<String>],
    ri: usize,
    min: f64,
) -> usize {
    // bounding boxes: two routes whose boxes do not meet cannot cross
    let bbox = |pts: &[Point]| {
        pts.iter().fold(
            (
                f64::INFINITY,
                f64::INFINITY,
                f64::NEG_INFINITY,
                f64::NEG_INFINITY,
            ),
            |(x0, y0, x1, y1), p| (x0.min(p.0), y0.min(p.1), x1.max(p.0), y1.max(p.1)),
        )
    };
    let a = bbox(&routes[ri].points);
    routes
        .iter()
        .enumerate()
        .filter(|(j, _)| *j != ri)
        .filter(|(_, r)| {
            let b = bbox(&r.points);
            a.0 <= b.2 && b.0 <= a.2 && a.1 <= b.3 && b.1 <= a.3
        })
        .map(|(j, r)| {
            let laid = if related(routes, conns, groups_of, ri, j) {
                0
            } else {
                overlays_over(&routes[ri].points, &r.points, min)
            };
            crossings(&routes[ri].points, &r.points) + OVERLAY_WEIGHT * laid
        })
        .sum()
}

/// What one overlay weighs against crossings in `uncross_ports`.
const OVERLAY_WEIGHT: usize = 100;

/// Two routes that may share a line: one connector's, a hyperedge group's, or two sharing an end
/// shape (a fork, a join, an arrow and its way back).
fn related(
    routes: &[Route],
    conns: &BTreeMap<&str, &Connector>,
    groups_of: &[Option<String>],
    i: usize,
    j: usize,
) -> bool {
    if i == j || (groups_of[i].is_some() && groups_of[i] == groups_of[j]) {
        return true;
    }
    let shapes_of = |k: usize| -> Vec<&str> {
        conns
            .get(routes[k].id.as_str())
            .map(|c| {
                [&c.from, &c.to]
                    .into_iter()
                    .filter_map(|e| match e {
                        End::Shape { id, .. } => Some(id.as_str()),
                        End::Point { .. } => None,
                    })
                    .collect()
            })
            .unwrap_or_default()
    };
    let a = shapes_of(i);
    shapes_of(j).iter().any(|s| a.contains(s))
}

/// Runs of `a` and `b` drawn on one line: parallel within 1.5 px over more than `min` px (4 px:
/// the engine's `overlay` property).
fn overlays_over(a: &[Point], b: &[Point], min: f64) -> usize {
    let runs = |pts: &[Point]| -> Vec<(bool, f64, f64, f64)> {
        pts.windows(2)
            .filter_map(|w| {
                let (p, q) = (w[0], w[1]);
                if (p.0 - q.0).abs() < 0.5 && (p.1 - q.1).abs() > EPS {
                    Some((true, p.0, p.1.min(q.1), p.1.max(q.1)))
                } else if (p.1 - q.1).abs() < 0.5 && (p.0 - q.0).abs() > EPS {
                    Some((false, p.1, p.0.min(q.0), p.0.max(q.0)))
                } else {
                    None
                }
            })
            .collect()
    };
    let (mut a, mut b) = (a.to_vec(), b.to_vec());
    merge(&mut a);
    merge(&mut b);
    let rb = runs(&b);
    runs(&a)
        .iter()
        .map(|x| {
            rb.iter()
                .filter(|y| {
                    x.0 == y.0 && (x.1 - y.1).abs() < 1.5 && x.3.min(y.3) - x.2.max(y.2) > min
                })
                .count()
        })
        .sum()
}

/// Route with segment `s` moved by `d`.
fn shifted(routes: &[Route], s: &Seg, d: f64) -> Vec<Point> {
    let mut p = routes[s.route].points.clone();
    for idx in [s.k, s.k + 1] {
        if s.vertical {
            p[idx].0 = s.coord + d;
        } else {
            p[idx].1 = s.coord + d;
        }
    }
    p
}

/// Should `a` pass before `b` (on the side of the small coordinates)? Both orders are tried and
/// the one that crosses least is kept; on a tie, the direction key decides.
fn before(routes: &[Route], a: &Seg, b: &Seg) -> bool {
    let h = 3.0;
    let ab = crossings(&shifted(routes, a, -h), &shifted(routes, b, h));
    let ba = crossings(&shifted(routes, a, h), &shifted(routes, b, -h));
    if ab != ba {
        return ab < ba;
    }
    let (ka, kb) = (seg_key(routes, a), seg_key(routes, b));
    ka.0.cmp(&kb.0)
        .then(ka.1.total_cmp(&kb.1))
        .then(a.route.cmp(&b.route))
        .is_lt()
}

/// Free space around a channel (the edges of the nearest shapes, on either side).
fn channel(
    shapes: &BTreeMap<&str, Rect>,
    vertical: bool,
    coord: f64,
    lo: f64,
    hi: f64,
) -> (f64, f64) {
    let (mut left, mut right) = (f64::NEG_INFINITY, f64::INFINITY);
    for r in shapes.values() {
        let (r_lo, r_hi, a, b) = if vertical {
            (r.y0(), r.y1(), r.x0(), r.x1())
        } else {
            (r.x0(), r.x1(), r.y0(), r.y1())
        };
        if r_hi <= lo + EPS || r_lo >= hi - EPS {
            continue;
        }
        if b <= coord + EPS {
            left = left.max(b);
        } else if a >= coord - EPS {
            right = right.min(a);
        }
    }
    (left, right)
}

/// Union-find: representative of `i`, with path compression.
fn find(p: &mut [usize], i: usize) -> usize {
    let mut r = i;
    while p[r] != r {
        r = p[r];
    }
    let mut j = i;
    while p[j] != r {
        let n = p[j];
        p[j] = r;
        j = n;
    }
    r
}

/// How far out a U along the drawing's flow may move from where the router set it, and whether
/// that holds for horizontal segments (a left-to-right drawing); `None` without a flow
/// (`center_segments`).
type Reach = Option<(f64, bool)>;

/// Free middle segments: centred in the corridor the shapes on either side bound. A route going
/// round an obstacle no longer hugs the margin of the shape it aims at: the bend sits in the
/// middle of the free space, as a draughtsman would put it; but a U (both bends turning the same
/// way) running along the drawing's flow moves out no further than `reach` from where the router
/// set it, beside the shape it goes round (twice the margin; `reach`: that distance, and whether it
/// holds for horizontal segments, `None` without a flow): in a column's open corridor the middle sent the bend hundreds of pixels
/// away, round a card standing between the route's ends (bug 19 of the private corpus). Across
/// the flow the gaps between rows bound the move, and the engine centres those segments with the
/// zone frames in view (`route::center_in_gaps`). Segments that carry a waypoint, whose corridor is bounded on one side only, or
/// whose move would cross a shape, stay. The result depends only on the shapes bounding the
/// corridor (stability, D-031).
fn center_segments(
    routes: &mut [Route],
    conns: &BTreeMap<&str, &Connector>,
    shapes: &BTreeMap<&str, Rect>,
    walls: &[Rect],
    reach: Reach,
) {
    let mut grouped: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    // routes a group's line may push across it: neither grouped nor held by waypoints
    let movable: Vec<bool> = routes
        .iter()
        .map(|r| {
            conns
                .get(r.id.as_str())
                .is_none_or(|c| c.group.is_none() && c.checkpoints.is_empty())
        })
        .collect();
    for (i, r) in routes.iter_mut().enumerate() {
        let conn = conns.get(r.id.as_str());
        if let Some(g) = conn.and_then(|c| c.group.clone()) {
            grouped.entry(g).or_default().push(i);
            continue;
        }
        let checkpoints: &[Point] = conn.map_or(&[], |c| &c.checkpoints);
        let n = r.points.len();
        for k in 1..n.saturating_sub(2) {
            if let Some(moved) =
                center_segment(&r.points, k, checkpoints, (shapes, walls), (None, reach))
            {
                r.points[k] = moved[1];
                r.points[k + 1] = moved[2];
            }
        }
    }
    for members in grouped.values() {
        center_group(routes, members, (shapes, walls), &movable, reach);
    }
}

/// The branches of a hyperedge (R-18) are centred together. First, the segments of the group
/// that are parallel and touch (the trunk splitting left and right, the collectors of a join
/// coming from cards at different depths) take one line, the middle of the corridor they all
/// share: fewer junctions. When no such line is free, the segments already on one line take the
/// middle of the corridor along their joint extent, all of them or none, so that the split stays
/// at one point.
fn center_group(
    routes: &mut [Route],
    members: &[usize],
    (shapes, walls): (&BTreeMap<&str, Rect>, &[Rect]),
    movable: &[bool],
    reach: Reach,
) {
    for &i in members {
        for k in 1..routes[i].points.len().saturating_sub(2) {
            // a branch leaving along a sibling's first or last stretch (a late split,
            // `Params::late_split`) shares a line held by that sibling's port: it stays on it
            if on_an_end(routes, members, (i, k)) {
                continue;
            }
            let (sib, span) = siblings(routes, members, (i, k), false);
            let shared = shared_line(routes, &sib, shapes, span)
                .and_then(|moved| clear_of_others(routes, members, &sib, &moved, shapes, movable));
            if let Some(changed) = shared {
                for (j, pts) in changed {
                    routes[j].points = pts;
                }
                continue;
            }
            let (sib, span) = siblings(routes, members, (i, k), true);
            let moved: Option<Vec<_>> = sib
                .iter()
                .map(|&(j, m)| {
                    center_segment(
                        &routes[j].points,
                        m,
                        &[],
                        (shapes, walls),
                        (Some(span), reach),
                    )
                })
                .collect();
            for (&(j, m), mv) in sib.iter().zip(moved.unwrap_or_default()) {
                routes[j].points[m] = mv[1];
                routes[j].points[m + 1] = mv[2];
            }
        }
    }
}

/// Does segment `k` of route `i` run on the first or last stretch of another route of the group
/// (on one line, over more than a pixel)?
fn on_an_end(routes: &[Route], members: &[usize], (i, k): (usize, usize)) -> bool {
    let (a, b) = (routes[i].points[k], routes[i].points[k + 1]);
    let horizontal = (a.1 - b.1).abs() < EPS;
    let (pos, lo, hi) = if horizontal {
        (a.1, a.0.min(b.0), a.0.max(b.0))
    } else {
        (a.0, a.1.min(b.1), a.1.max(b.1))
    };
    members.iter().filter(|&&j| j != i).any(|&j| {
        let pts = &routes[j].points;
        let n = pts.len();
        n >= 2
            && [(pts[0], pts[1]), (pts[n - 2], pts[n - 1])]
                .iter()
                .any(|&(p, q)| {
                    let (at, l, h) = if horizontal {
                        (p.1, p.0.min(q.0), p.0.max(q.0))
                    } else {
                        (p.0, p.1.min(q.1), p.1.max(q.1))
                    };
                    let parallel = if horizontal {
                        (p.1 - q.1).abs() < EPS
                    } else {
                        (p.0 - q.0).abs() < EPS
                    };
                    parallel && (at - pos).abs() < 1.0 && h.min(hi) - l.max(lo) > 1.0
                })
    })
}

/// The group's routes with segments `sib` moved (`moved`), plus the other routes that would then
/// run along one of them, each stepped across the group's new line; `None` if one of those cannot
/// step aside (the group then keeps its lines apart). Route index → new points.
fn clear_of_others(
    routes: &[Route],
    members: &[usize],
    sib: &[(usize, usize)],
    moved: &[[Point; 4]],
    shapes: &BTreeMap<&str, Rect>,
    movable: &[bool],
) -> Option<Vec<(usize, Vec<Point>)>> {
    let mut changed: BTreeMap<usize, Vec<Point>> = BTreeMap::new();
    for (&(j, m), mv) in sib.iter().zip(moved) {
        let pts = changed.entry(j).or_insert_with(|| routes[j].points.clone());
        pts[m] = mv[1];
        pts[m + 1] = mv[2];
    }
    let (a, b) = (moved[0][1], moved[0][2]);
    let horizontal = (a.1 - b.1).abs() < EPS;
    let line = if horizontal { a.1 } else { a.0 };
    let mine: Vec<Vec<Point>> = members
        .iter()
        .map(|&j| changed.get(&j).unwrap_or(&routes[j].points).clone())
        .collect();
    for (o, r) in routes.iter().enumerate() {
        if members.contains(&o) || !mine.iter().any(|p| runs_along(p, &r.points)) {
            continue;
        }
        if !movable[o] {
            return None;
        }
        let stepped = step_across(&r.points, horizontal, line, shapes)?;
        if mine.iter().any(|p| runs_along(p, &stepped)) {
            return None;
        }
        changed.insert(o, stepped);
    }
    Some(changed.into_iter().collect())
}

/// Do two routes run along each other somewhere (parallel segments less than 2 px apart over
/// more than 1 px)?
fn runs_along(a: &[Point], b: &[Point]) -> bool {
    let segs =
        |p: &[Point]| -> Vec<(Point, Point)> { p.windows(2).map(|w| (w[0], w[1])).collect() };
    segs(a).iter().any(|&(p, q)| {
        segs(b).iter().any(|&(u, v)| {
            let (h1, h2) = ((p.1 - q.1).abs() < EPS, (u.1 - v.1).abs() < EPS);
            let (v1, v2) = ((p.0 - q.0).abs() < EPS, (u.0 - v.0).abs() < EPS);
            let (near, l1, h1e, l2, h2e) = if h1 && h2 && !(v1 || v2) {
                (
                    (p.1 - u.1).abs() < 2.0,
                    p.0.min(q.0),
                    p.0.max(q.0),
                    u.0.min(v.0),
                    u.0.max(v.0),
                )
            } else if v1 && v2 && !(h1 || h2) {
                (
                    (p.0 - u.0).abs() < 2.0,
                    p.1.min(q.1),
                    p.1.max(q.1),
                    u.1.min(v.1),
                    u.1.max(v.1),
                )
            } else {
                return false;
            };
            near && h1e.min(h2e) - l1.max(l2) > 1.0
        })
    })
}

/// Route `pts` with its middle segments parallel to a group's line (at `line`) whose neighbours
/// cross that line moved to the far side, half-way between the line and the next shape: the
/// route steps aside where it met the group, instead of running along one of its branches.
fn step_across(
    pts: &[Point],
    horizontal: bool,
    line: f64,
    shapes: &BTreeMap<&str, Rect>,
) -> Option<Vec<Point>> {
    let coord = |p: Point| if horizontal { p.1 } else { p.0 };
    let along = |p: Point| if horizontal { p.0 } else { p.1 };
    let mut out = pts.to_vec();
    let mut any = false;
    for k in 1..pts.len().saturating_sub(2) {
        let (a, b) = (out[k], out[k + 1]);
        if (coord(a) - coord(b)).abs() > EPS {
            continue; // not parallel to the line
        }
        let pos = coord(a);
        // the side of the line its ends come from
        let ends = [coord(out[k - 1]), coord(out[k + 2])];
        let Some(&far) = ends.iter().find(|&&c| (c - line) * (pos - line) < 0.0) else {
            continue;
        };
        let (lo, hi) = (along(a).min(along(b)), along(a).max(along(b)));
        let up = far > line;
        let bound = shapes
            .values()
            .filter_map(|r| {
                let (s0, s1, e0, e1) = if horizontal {
                    (r.x0(), r.x1(), r.y0(), r.y1())
                } else {
                    (r.y0(), r.y1(), r.x0(), r.x1())
                };
                (s1 > lo + EPS && s0 < hi - EPS).then_some(if up { e0 } else { e1 })
            })
            .filter(|&e| if up { e >= line } else { e <= line })
            .fold(far, |m, e| if up { m.min(e) } else { m.max(e) });
        let mv = segment_at(&out, k, (line + bound) / 2.0, shapes)?;
        out[k] = mv[1];
        out[k + 1] = mv[2];
        any = true;
    }
    any.then_some(out)
}

/// The middle segments of a group's routes parallel to segment `k` of route `i` that touch it,
/// directly or through one another (`collinear`: on its line only), and their joint extent.
fn siblings(
    routes: &[Route],
    members: &[usize],
    (i, k): (usize, usize),
    collinear: bool,
) -> (Vec<(usize, usize)>, (f64, f64)) {
    let (a, b) = (routes[i].points[k], routes[i].points[k + 1]);
    let horizontal = (a.1 - b.1).abs() < EPS;
    let pos = if horizontal { a.1 } else { a.0 };
    let ext = |p: Point, q: Point| {
        if horizontal {
            (p.0.min(q.0), p.0.max(q.0))
        } else {
            (p.1.min(q.1), p.1.max(q.1))
        }
    };
    let (mut lo, mut hi) = ext(a, b);
    let mut sib: Vec<(usize, usize)> = Vec::new();
    for &j in members {
        let pts = &routes[j].points;
        for m in 1..pts.len().saturating_sub(2) {
            let (p, q) = (pts[m], pts[m + 1]);
            let at = if horizontal { p.1 } else { p.0 };
            let parallel = if horizontal {
                (p.1 - q.1).abs() < EPS
            } else {
                (p.0 - q.0).abs() < EPS
            };
            let (l, h) = ext(p, q);
            if parallel && (!collinear || (at - pos).abs() < 1.0) && l <= hi + 1.0 && h >= lo - 1.0
            {
                sib.push((j, m));
                lo = lo.min(l);
                hi = hi.max(h);
            }
        }
    }
    (sib, (lo, hi))
}

/// The segments `sib` all moved onto one line, the middle of the corridor they share over
/// `span`; `None` if they share none or one of them cannot move there.
fn shared_line(
    routes: &[Route],
    sib: &[(usize, usize)],
    shapes: &BTreeMap<&str, Rect>,
    span: (f64, f64),
) -> Option<Vec<[Point; 4]>> {
    let (mut before, mut after) = (f64::NEG_INFINITY, f64::INFINITY);
    for &(j, m) in sib {
        let (b, a) = corridor(&routes[j].points, m, (shapes, &[]), Some(span))?;
        before = before.max(b);
        after = after.min(a);
    }
    if before >= after {
        return None;
    }
    sib.iter()
        .map(|&(j, m)| segment_at(&routes[j].points, m, (before + after) / 2.0, shapes))
        .collect()
}

/// Segment `k` (`pts[k]`–`pts[k + 1]`) moved to the middle of its corridor (the four points
/// `k - 1` to `k + 2`), but in a U along the flow no further out than `reach` from where it stands,
/// or `None` if there is no reason to or it cannot move. `span` replaces the segment's own extent to find the
/// corridor (the joint extent of a hyperedge's branches).
fn center_segment(
    pts: &[Point],
    k: usize,
    checkpoints: &[Point],
    (shapes, walls): (&BTreeMap<&str, Rect>, &[Rect]),
    (span, reach): (Option<(f64, f64)>, Reach),
) -> Option<[Point; 4]> {
    let (a, b) = (pts[k], pts[k + 1]);
    if checkpoints.iter().any(|&c| on_segment(a, b, c)) {
        return None;
    }
    let (before, after) = corridor(pts, k, (shapes, walls), span)?;
    let horizontal = (a.1 - b.1).abs() < EPS;
    let coord = |p: Point| if horizontal { p.1 } else { p.0 };
    let (pos, prev, next) = (coord(a), coord(pts[k - 1]), coord(pts[k + 2]));
    let mid = (before + after) / 2.0;
    // the bends on either side turn the same way (a U round the shape on their side): moving the
    // segment away from them lengthens the route by twice the move, so it goes out no further than
    // `reach` from where the router set it; in a Z the move costs nothing and the middle stays
    let u = (prev - pos) * (next - pos) > 0.0;
    let along = reach.is_some_and(|(_, flat)| flat == horizontal);
    let reach = reach.map_or(f64::INFINITY, |(r, _)| r);
    let aim = if along && u && (mid - pos) * (prev - pos) < 0.0 {
        mid.clamp(pos - reach, pos + reach)
    } else {
        mid
    };
    segment_at(pts, k, aim, shapes)
}

/// The corridor of segment `k` across its direction: the nearest shape edges on either side over
/// its extent (or `span`); `None` if the segment is not orthogonal or the corridor is open.
/// `walls` (the regions' frames, `Params::region_walls`) bound it as shapes do, except one the
/// segment runs inside.
fn corridor(
    pts: &[Point],
    k: usize,
    (shapes, walls): (&BTreeMap<&str, Rect>, &[Rect]),
    span: Option<(f64, f64)>,
) -> Option<(f64, f64)> {
    let (a, b) = (pts[k], pts[k + 1]);
    let horizontal = (a.1 - b.1).abs() < EPS && (a.0 - b.0).abs() > EPS;
    let vertical = (a.0 - b.0).abs() < EPS && (a.1 - b.1).abs() > EPS;
    if !(horizontal || vertical) {
        return None;
    }
    // the segment is seen as "horizontal" in a possibly transposed frame
    let (pos, lo_span, hi_span) = if horizontal {
        (a.1, a.0.min(b.0), a.0.max(b.0))
    } else {
        (a.0, a.1.min(b.1), a.1.max(b.1))
    };
    let (lo_span, hi_span) = span.unwrap_or((lo_span, hi_span));
    let (mut before, mut after) = (f64::NEG_INFINITY, f64::INFINITY);
    for r in shapes.values().chain(walls) {
        let (s0, s1, e0, e1) = if horizontal {
            (r.x0(), r.x1(), r.y0(), r.y1())
        } else {
            (r.y0(), r.y1(), r.x0(), r.x1())
        };
        if s1 <= lo_span + EPS || s0 >= hi_span - EPS {
            continue; // does not bound the corridor over the segment's span
        }
        if e1 <= pos + EPS {
            before = before.max(e1);
        } else if e0 >= pos - EPS {
            after = after.min(e0);
        }
    }
    (before.is_finite() && after.is_finite()).then_some((before, after))
}

/// Segment `k` moved across to `mid` (the four points `k - 1` to `k + 2`), or `None` if its
/// neighbours would turn round or the move would cross a shape.
fn segment_at(
    pts: &[Point],
    k: usize,
    mid: f64,
    shapes: &BTreeMap<&str, Rect>,
) -> Option<[Point; 4]> {
    let (a, b) = (pts[k], pts[k + 1]);
    let horizontal = (a.1 - b.1).abs() < EPS && (a.0 - b.0).abs() > EPS;
    let pos = if horizontal { a.1 } else { a.0 };
    let mut moved = [pts[k - 1], pts[k], pts[k + 1], pts[k + 2]];
    if (mid - pos).abs() < EPS {
        return Some(moved); // already in the middle
    }
    // the (perpendicular) neighbours keep their direction: a segment is never turned round
    let coord = |p: Point| if horizontal { p.1 } else { p.0 };
    let (prev, next) = (coord(pts[k - 1]), coord(pts[k + 2]));
    // a neighbour on the segment's own line is no bend but its continuation (a port's stub):
    // the segment cannot leave it without slanting. The graph's coordinates are rounded
    // (`scene::snap`) while a port is not, so "on the line" is within half a pixel
    if (pos - prev).abs() < 0.5 || (pos - next).abs() < 0.5 {
        return None;
    }
    if (pos - prev) * (mid - prev) <= EPS || (pos - next) * (mid - next) <= EPS {
        return None;
    }
    for p in &mut moved[1..3] {
        if horizontal {
            p.1 = mid;
        } else {
            p.0 = mid;
        }
    }
    let blocked = moved.windows(2).any(|w| {
        shapes
            .values()
            .any(|r| r.inflate(-EPS).hit_by_segment(w[0], w[1]))
    });
    (!blocked).then_some(moved)
}

/// Is point `c` on the orthogonal segment `a`–`b` (within 0.5 px)?
fn on_segment(a: Point, b: Point, c: Point) -> bool {
    c.0 >= a.0.min(b.0) - 0.5
        && c.0 <= a.0.max(b.0) + 0.5
        && c.1 >= a.1.min(b.1) - 0.5
        && c.1 <= a.1.max(b.1) + 0.5
}

/// R-08 : écarte les segments colinéaires qui se chevauchent. The first and last segments of the
/// routes, which hold to their ports, take part without moving: a middle segment lying on one of
/// them steps aside (otherwise two unrelated arrows were drawn as one line).
fn nudge(
    routes: &mut [Route],
    shapes: &BTreeMap<&str, Rect>,
    params: &Params,
    vertical: bool,
    groups: &[Option<String>],
) {
    let mut segs = middle_segments(routes, vertical);
    segs.extend(segments(routes, vertical, true));
    // grouping: same coordinate (within 1 px) and overlapping spans; two fixed segments are not
    // grouped together (`separate_end_overlaps` sees to them)
    let mut parent: Vec<usize> = (0..segs.len()).collect();
    for i in 0..segs.len() {
        for j in i + 1..segs.len() {
            let (a, b) = (&segs[i], &segs[j]);
            if a.route != b.route
                && !(a.fixed && b.fixed)
                && (a.coord - b.coord).abs() < 1.0
                && a.hi.min(b.hi) - a.lo.max(b.lo) > 1.0
            {
                let (ra, rb) = (find(&mut parent, i), find(&mut parent, j));
                if ra != rb {
                    parent[ra.max(rb)] = ra.min(rb);
                }
            }
        }
    }
    let mut clusters: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for i in 0..segs.len() {
        let r = find(&mut parent, i);
        clusters.entry(r).or_default().push(i);
    }
    for (_, members) in clusters {
        if members.len() >= 2 && members.iter().any(|&m| !segs[m].fixed) {
            spread(routes, shapes, params, (&segs, &members), groups);
        }
    }
}

/// Spreads the segments of one cluster across their corridor, `ideal_nudging` apart (less when
/// the corridor is narrow): around their common line, or, when a fixed segment is among them,
/// around that one, which stays.
fn spread(
    routes: &mut [Route],
    shapes: &BTreeMap<&str, Rect>,
    params: &Params,
    (segs, members): (&[Seg], &[usize]),
    groups: &[Option<String>],
) {
    let vertical = segs[members[0]].vertical;
    // insertion sort: the "before" relation is not necessarily transitive
    let mut ordered: Vec<usize> = Vec::with_capacity(members.len());
    for &m in members {
        let pos = ordered
            .iter()
            .position(|&o| before(routes, &segs[m], &segs[o]))
            .unwrap_or(ordered.len());
        ordered.insert(pos, m);
    }
    // the segments of one hyperedge group stay on top of each other (a common trunk)
    let slotted = slots(&ordered, |m| groups[segs[m].route].clone());
    let fixed: Vec<usize> = (0..slotted.len())
        .filter(|&k| slotted[k].iter().any(|&m| segs[m].fixed))
        .collect();
    if fixed.len() > 1 {
        return;
    }
    let firsts: Vec<usize> = slotted.iter().map(|g| g[0]).collect();
    let lo = firsts
        .iter()
        .map(|&m| segs[m].lo)
        .fold(f64::INFINITY, f64::min);
    let hi = firsts
        .iter()
        .map(|&m| segs[m].hi)
        .fold(f64::NEG_INFINITY, f64::max);
    let n = slotted.len() as f64;
    // the slot that stays: the fixed one, else the middle of the cluster
    let (anchor, mut coord) = match fixed.first() {
        Some(&f) => (f as f64, segs[slotted[f][0]].coord),
        None => ((n - 1.0) / 2.0, segs[firsts[0]].coord),
    };
    let (left, right) = channel(shapes, vertical, coord, lo, hi);
    let mut s: f64 = params.ideal_nudging;
    // the lines keep the router's whole margin from the shapes when the channel holds them so
    // spaced, half of it only when it is narrow (a line 7 px from a card ran along it)
    let half = (params.shape_buffer / 2.0).max(4.0);
    let whole = params.shape_buffer.max(half);
    let fits = right - left - 2.0 * whole >= (n - 1.0) * s;
    let margin = if fits { whole } else { half };
    // a cluster with no fixed segment moves as a whole inside its channel, as near its line as
    // its spread allows: centred on a line near one side, it shrank its spacing to that side's room
    // (ten lines 1.75 px apart beside a column of cards, the private map's `inscription`); a
    // channel open on one side bounds it on the other alone
    if fixed.is_empty() && n > 1.0 && (left.is_finite() || right.is_finite()) {
        let span = right - left - 2.0 * margin;
        s = s.min((span / (n - 1.0)).max(0.0));
        let half = (n - 1.0) * s / 2.0;
        let (lo_c, hi_c) = (left + margin + half, right - margin - half);
        if lo_c <= hi_c {
            coord = coord.clamp(lo_c, hi_c);
        }
    }
    // each side of the anchor its own spacing: the room of the narrower side no longer squeezes
    // the lines on the other
    // the lines past the anchor on a side keep the margin chosen when they fit so spaced, else
    // half of it, and narrow their spacing to that room
    let side = |edge: f64, slots_beyond: f64| {
        if slots_beyond <= 0.0 || slots_beyond * s <= edge - margin {
            s
        } else {
            ((edge - half) / slots_beyond).clamp(0.0, s)
        }
    };
    let (s_lo, s_hi) = (
        side(coord - left, anchor),
        side(right - coord, n - 1.0 - anchor),
    );
    for (k, slot) in slotted.iter().enumerate() {
        let off = k as f64 - anchor;
        let at = coord + off * if off < 0.0 { s_lo } else { s_hi };
        for &m in slot.iter().filter(|&&m| !segs[m].fixed) {
            let sg = &segs[m];
            if (at - sg.coord).abs() < EPS {
                continue;
            }
            let pts = &mut routes[sg.route].points;
            for idx in [sg.k, sg.k + 1] {
                if vertical {
                    pts[idx].0 = at;
                } else {
                    pts[idx].1 = at;
                }
            }
        }
    }
}

/// R2 post-processing of every route.
pub(crate) fn beautify(
    routes: &mut [Route],
    connectors: &[Connector],
    (shapes, regions): (&BTreeMap<&str, Rect>, &[Rect]),
    params: &Params,
) {
    let walls: &[Rect] = if params.region_walls { regions } else { &[] };
    if !(params.ideal_nudging.is_finite() && params.ideal_nudging > 0.0) {
        return;
    }
    let conns: BTreeMap<&str, &Connector> = connectors.iter().map(|c| (c.id.as_str(), c)).collect();
    let groups: Vec<Option<String>> = routes
        .iter()
        .map(|r| conns.get(r.id.as_str()).and_then(|c| c.group.clone()))
        .collect();
    trace("in", routes);
    if on("spread-ports") {
        spread_ports(routes, &conns, shapes, params);
        trace("spread-ports", routes);
    }
    if on("center-segs") {
        // along the drawing's flow (vertical segments of a top-down drawing): across it, the
        // gaps between rows bound the move
        let along = params
            .flow
            .map(|f| (2.0 * params.shape_buffer, f.horizontal()));
        center_segments(routes, &conns, shapes, walls, along);
        trace("center-segs", routes);
    }
    // a horizontal segment moved lengthens the vertical ones beside it, which may then lie on
    // another line: the two passes again, until nothing moves (three rounds at most)
    if on("nudge") {
        for _ in 0..3 {
            let before: Vec<Vec<Point>> = routes.iter().map(|r| r.points.clone()).collect();
            nudge(routes, shapes, params, true, &groups);
            nudge(routes, shapes, params, false, &groups);
            if routes.iter().zip(&before).all(|(r, b)| r.points == *b) {
                break;
            }
        }
        trace("nudge", routes);
    }
    if on("end-overlaps") {
        separate_end_overlaps(routes, &conns, shapes, params, &groups);
        trace("end-overlaps", routes);
    }
    // a port may need several swaps to reach its place (a branch coming from far beyond its
    // neighbours): a few passes
    if on("uncross") {
        for _ in 0..4 {
            uncross_ports(routes, &conns, shapes, &groups);
        }
        trace("uncross", routes);
    }
    if on("unlay") {
        unlay(routes, &conns, shapes, &groups, params);
        trace("unlay", routes);
    }
    if on("merge") {
        for r in routes.iter_mut() {
            merge(&mut r.points);
        }
    }
}

/// Last, two unrelated routes still drawn on one line: two of their middle segments, parallel and
/// near each other, trade places when that leaves fewer tangles (crossings, overlays weighing
/// more) and no line through a shape. Two arrows leaving facing sides of two cards side by side
/// went down the gap between them in the wrong order, their ends' legs laid on each other (the
/// private map's current `hive/installation`).
fn unlay(
    routes: &mut [Route],
    conns: &BTreeMap<&str, &Connector>,
    shapes: &BTreeMap<&str, Rect>,
    groups_of: &[Option<String>],
    params: &Params,
) {
    let near = 2.0 * params.ideal_nudging.max(params.shape_buffer);
    for i in 0..routes.len() {
        for j in i + 1..routes.len() {
            // any length on one line: two ends' legs a few pixels on each other are spread
            // further later (`breathe`) and become a plain overlay
            const TOUCH: f64 = 0.5;
            // `Params::untangle`: two routes that cross, not only two on one line
            if related(routes, conns, groups_of, i, j)
                || (!params.untangle
                    && overlays_over(&routes[i].points, &routes[j].points, TOUCH) == 0)
            {
                continue;
            }
            let score = |routes: &[Route]| {
                tangles_over(routes, conns, groups_of, i, TOUCH)
                    + tangles_over(routes, conns, groups_of, j, TOUCH)
            };
            let before = score(routes);
            if before == 0 {
                continue;
            }
            let (pi, pj) = (routes[i].points.clone(), routes[j].points.clone());
            let mut best: Option<(usize, Vec<Point>, Vec<Point>)> = None;
            for (ni, nj) in trades(&pi, &pj, near, params.untangle) {
                if shapes_hit(&ni, shapes) > shapes_hit(&pi, shapes)
                    || shapes_hit(&nj, shapes) > shapes_hit(&pj, shapes)
                {
                    continue;
                }
                routes[i].points = ni.clone();
                routes[j].points = nj.clone();
                let after = score(routes);
                routes[i].points = pi.clone();
                routes[j].points = pj.clone();
                if after < before && best.as_ref().is_none_or(|b| after < b.0) {
                    best = Some((after, ni, nj));
                }
            }
            if let Some((_, ni, nj)) = best {
                routes[i].points = ni;
                routes[j].points = nj;
            }
        }
    }
}

/// A segment of a route: its index, whether it is upright, its position across, its extent along.
type Leg = (usize, bool, f64, f64, f64);

/// The orthogonal segments of `pts`: its middle ones, or (`all`) its first and last too.
fn legs(pts: &[Point], all: bool) -> Vec<Leg> {
    if pts.len() < if all { 2 } else { 4 } {
        return Vec::new();
    }
    let range = if all {
        0..pts.len() - 1
    } else {
        1..pts.len() - 2
    };
    range
        .filter_map(|k| {
            let (a, b) = (pts[k], pts[k + 1]);
            if (a.0 - b.0).abs() < EPS && (a.1 - b.1).abs() > EPS {
                Some((k, true, a.0, a.1.min(b.1), a.1.max(b.1)))
            } else if (a.1 - b.1).abs() < EPS && (a.0 - b.0).abs() > EPS {
                Some((k, false, a.1, a.0.min(b.0), a.0.max(b.0)))
            } else {
                None
            }
        })
        .collect()
}

/// Two parallel segments a few pixels apart that face each other over some length.
fn facing(a: &Leg, b: &Leg, near: f64) -> bool {
    a.1 == b.1
        && (a.2 - b.2).abs() <= near
        && (a.2 - b.2).abs() >= EPS
        && a.4.min(b.4) - a.3.max(b.3) > 0.0
}

/// The new shapes `unlay` tries for two routes: two of their middle segments trading places;
/// with `untangle`, also a middle segment of one moved to the other side of any segment of the
/// other (its first or last included: an L leaving a port), as far beyond it as it stood.
fn trades(pi: &[Point], pj: &[Point], near: f64, untangle: bool) -> Vec<(Vec<Point>, Vec<Point>)> {
    let mut out = Vec::new();
    for si in legs(pi, false) {
        for sj in legs(pj, false) {
            if !facing(&si, &sj, near) {
                continue;
            }
            let (mut ni, mut nj) = (pi.to_vec(), pj.to_vec());
            for k in [si.0, si.0 + 1] {
                if si.1 {
                    ni[k].0 = sj.2;
                } else {
                    ni[k].1 = sj.2;
                }
            }
            for k in [sj.0, sj.0 + 1] {
                if sj.1 {
                    nj[k].0 = si.2;
                } else {
                    nj[k].1 = si.2;
                }
            }
            out.push((ni, nj));
        }
    }
    if !untangle {
        return out;
    }
    for m in legs(pi, false) {
        for o in legs(pj, true) {
            if facing(&m, &o, near)
                && let Some(ni) = moved_segment(pi, m.0, m.1, 2.0 * o.2 - m.2)
            {
                out.push((ni, pj.to_vec()));
            }
        }
    }
    for m in legs(pj, false) {
        for o in legs(pi, true) {
            if facing(&m, &o, near)
                && let Some(nj) = moved_segment(pj, m.0, m.1, 2.0 * o.2 - m.2)
            {
                out.push((pi.to_vec(), nj));
            }
        }
    }
    out
}

/// Middle segment `k` of `pts` (upright when `upright`) moved across to `to`; `None` when one of
/// its neighbours would turn round or shrink under 8 px (a trade of two lines a few pixels apart
/// never does: their neighbours are long).
fn moved_segment(pts: &[Point], k: usize, upright: bool, to: f64) -> Option<Vec<Point>> {
    const LEAST: f64 = 8.0;
    let coord = |p: Point| if upright { p.0 } else { p.1 };
    let pos = coord(pts[k]);
    for n in [pts[k - 1], pts[k + 2]] {
        let (before, after) = (pos - coord(n), to - coord(n));
        if before * after <= 0.0 || (after.abs() < LEAST && before.abs() >= LEAST) {
            return None;
        }
    }
    let mut out = pts.to_vec();
    for p in &mut out[k..=k + 1] {
        if upright {
            p.0 = to;
        } else {
            p.1 = to;
        }
    }
    Some(out)
}

/// `SKEMO_OFF=pass,…` (the engine's `trace` module): is this pass still on? Ablation only.
fn on(pass: &str) -> bool {
    use std::sync::OnceLock;
    static OFF: OnceLock<Vec<String>> = OnceLock::new();
    !OFF.get_or_init(|| {
        std::env::var("SKEMO_OFF")
            .map(|v| {
                v.split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            })
            .unwrap_or_default()
    })
    .iter()
    .any(|n| n == pass)
}

/// `SKEMO_TRACE=id,…` (the engine's `trace` module): the named routes after each pass.
fn trace(stage: &str, routes: &[Route]) {
    use std::sync::OnceLock;
    static NAMES: OnceLock<Vec<String>> = OnceLock::new();
    let names = NAMES.get_or_init(|| {
        std::env::var("SKEMO_TRACE")
            .map(|v| {
                v.split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            })
            .unwrap_or_default()
    });
    for r in routes.iter().filter(|r| {
        names.iter().any(|n| match n.strip_suffix('*') {
            Some(p) => r.id.starts_with(p),
            None => r.id == *n,
        })
    }) {
        let pts: Vec<String> = r
            .points
            .iter()
            .map(|p| format!("({:.1}, {:.1})", p.0, p.1))
            .collect();
        eprintln!("trace route router/{stage:<12} {}: {}", r.id, pts.join(" "));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_depth_before_a_turn_skips_a_stub_end() {
        // the stub's end (rounded to 1e-4) is in line with the port: the first turn is at 901
        let pts = [
            (488.0, 826.0),
            (488.0, 901.0),
            (1192.9167, 901.0),
            (1192.9167, 949.0),
            (1_192.916_666_666_667, 963.0),
        ];
        let d = first_turn_depth(&pts, false).unwrap();
        assert!((d - 62.0).abs() < 0.01, "{d}");
        assert!((first_turn_depth(&pts, true).unwrap() - 75.0).abs() < 1e-9);
    }

    /// The private map's `abonnements` under two more pixels of gutter: a route whose "second
    /// corner" is a stub in line with the leg (its end on the card below). Swapping the levels of
    /// the first turns would move the leg and leave the stub on a slant: refused.
    #[test]
    fn a_turn_swap_never_leaves_a_slant() {
        let rect = Rect::new(519.0, 280.0, 200.0, 80.0);
        let route = |id: &str, points: Vec<Point>| Route {
            id: id.to_string(),
            points,
        };
        let mut routes = vec![
            route(
                "a",
                vec![
                    (519.0, 300.0),
                    (428.2, 300.0),
                    (428.2, 802.0),
                    (428.2, 816.0),
                ],
            ),
            route(
                "b",
                vec![
                    (519.0, 333.0),
                    (443.2, 333.0),
                    (443.2, 700.0),
                    (300.0, 700.0),
                ],
            ),
        ];
        let port = |route: usize| PortRef {
            route,
            at_start: true,
            side: Side::Left,
            rect,
        };
        let before = routes.clone();
        assert!(!swap_turns(
            &mut routes,
            &port(0),
            &port(1),
            &BTreeMap::new()
        ));
        assert_eq!(routes[0].points, before[0].points);
        assert!(orthogonal(&routes[0].points) && orthogonal(&routes[1].points));
    }

    /// Two lines on one another beside a column of cards, with room on their other side: spread
    /// apart, both keep the router's whole margin from the cards (the private map's `produits`: one
    /// went to 7 px of four cards and ran along them).
    #[test]
    fn lines_spread_beside_cards_keep_the_whole_margin() {
        let card = Rect::new(0.0, 0.0, 280.0, 400.0);
        let shapes = BTreeMap::from([("card", card)]);
        let route = |id: &str, y0: f64, y1: f64, x: f64| Route {
            id: id.to_string(),
            points: vec![(x, y0), (294.0, y0), (294.0, y1), (x, y1)],
        };
        let mut routes = vec![
            route("a", 50.0, 350.0, 500.0),
            route("b", 60.0, 340.0, 520.0),
        ];
        let params = Params {
            ideal_nudging: 20.0,
            ..Params::default()
        };
        nudge(&mut routes, &shapes, &params, true, &[None, None]);
        let xs: Vec<f64> = routes.iter().map(|r| r.points[1].0).collect();
        assert!(
            xs.iter().all(|x| *x >= 280.0 + params.shape_buffer - EPS),
            "{xs:?}"
        );
        assert!((xs[0] - xs[1]).abs() >= 20.0 - EPS, "{xs:?}");
    }

    fn orthogonal(p: &[Point]) -> bool {
        p.windows(2)
            .all(|w| (w[0].0 - w[1].0).abs() < EPS || (w[0].1 - w[1].1).abs() < EPS)
    }

    #[test]
    fn the_middle_half_widens_evenly_when_the_ports_do_not_fit() {
        let p = Params::default(); // port_spacing 12
        // side 0..100: middle half 25..75 holds 5 ports (48)
        assert_eq!(
            middle_half((0.0, 100.0, 0.0), (0.0, 100.0), 5, &p),
            (25.0, 75.0, 0.0)
        );
        // side 0..92: middle half 23..69 (46) is short: 22..70, centred, not the whole side
        assert_eq!(
            middle_half((0.0, 92.0, 0.0), (0.0, 92.0), 5, &p),
            (22.0, 70.0, 0.0)
        );
        // a room that cannot hold them stays as it is
        assert_eq!(
            middle_half((0.0, 40.0, 2.0), (0.0, 40.0), 5, &p),
            (0.0, 40.0, 2.0)
        );
        // widened inside the room clear of the shapes in front (here 30..92)
        let (a, b, _) = middle_half((30.0, 92.0, 0.0), (0.0, 92.0), 5, &p);
        assert!((a, b) == (30.0, 78.0), "{a} {b}");
    }

    /// A port is not rounded, the graph's points are (`scene::snap`): a stub 0.00001 px off the
    /// segment's line still continues it, and the segment never leaves it (it would slant; seen on
    /// a 50-card map, 2026-11-09).
    #[test]
    fn a_segment_never_leaves_the_stub_of_its_port() {
        let pts = vec![
            (1_732.785_714_285_714_2, 415.0),
            (1732.7857, 429.0),
            (1732.7857, 808.0),
            (1181.69, 808.0),
            (1181.69, 822.0),
        ];
        assert!(segment_at(&pts, 1, 1600.0, &BTreeMap::new()).is_none());
        // at the other end too
        let rev: Vec<Point> = pts.iter().rev().copied().collect();
        assert!(segment_at(&rev, 2, 1600.0, &BTreeMap::new()).is_none());
    }

    /// A line crossing a route exactly where its stub ends is a crossing (two arrows leaving the
    /// bottom of a card in the wrong order, 2026-11-09).
    #[test]
    fn a_crossing_at_the_end_of_a_stub_counts() {
        let stub = [
            (1190.0, 915.0),
            (1190.0, 929.0),
            (1190.0, 949.0),
            (936.0, 949.0),
        ];
        let across = [
            (1148.0, 915.0),
            (1148.0, 929.0),
            (1200.0, 929.0),
            (1200.0, 963.0),
        ];
        assert_eq!(crossings(&stub, &across), 1);
    }

    #[test]
    fn moving_a_port_carries_the_collinear_stub() {
        // stub 662 → 648, then the same line on to 442, then up
        let mut p = vec![
            (662.0, 248.199_996_948_242_2),
            (648.0, 248.2),
            (442.0, 248.2),
            (442.0, 144.2),
        ];
        move_port(&mut p, true, Side::Left, 257.2);
        assert!(orthogonal(&p), "{p:?}");
        assert_eq!(p[0], (662.0, 257.2));
        assert_eq!(p.last(), Some(&(442.0, 144.2)));
    }

    #[test]
    fn moving_a_port_of_a_straight_run_inserts_a_jog() {
        let mut p = vec![(0.0, 10.0), (20.0, 10.0), (80.0, 10.0)];
        move_port(&mut p, true, Side::Right, 14.0);
        assert!(orthogonal(&p), "{p:?}");
        assert_eq!(p[0], (0.0, 14.0));
        assert_eq!(p.last(), Some(&(80.0, 10.0)));
    }
}
