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
    groups
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
                (a.route != b.route && !same_group && overlap).then(|| {
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
        let target = (1..64)
            .flat_map(|k| {
                [
                    cur + away * step * f64::from(k),
                    cur - away * step * f64::from(k),
                ]
            })
            .find(|t| free(*t));
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
        let targets = even_targets(slotted.len(), lo, hi, margin, params);
        let assigned: Vec<(usize, f64)> = slotted
            .iter()
            .zip(&targets)
            .flat_map(|(members, &t)| members.iter().map(move |&i| (i, t)))
            .collect();
        for (i, target) in assigned {
            let pr = &ports[i];
            let pts = &mut routes[pr.route].points;
            // a straight line whose other end is alone on its side: both ends move
            if pts.len() == 2
                && let Some(far) = lone.get(&(pr.route, !pr.at_start))
            {
                let (flo, fhi) = if horizontal_side(side) {
                    (far.x0(), far.x1())
                } else {
                    (far.y0(), far.y1())
                };
                if target > flo + margin && target < fhi - margin {
                    for q in pts.iter_mut() {
                        set_along(q, side, target);
                    }
                    continue;
                }
            }
            move_port(pts, pr.at_start, side, target);
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
    let total = |routes: &[Route], ri: usize| -> usize {
        let a = bbox(&routes[ri].points);
        routes
            .iter()
            .enumerate()
            .filter(|(j, _)| *j != ri)
            .filter(|(_, r)| {
                let b = bbox(&r.points);
                a.0 <= b.2 && b.0 <= a.2 && a.1 <= b.3 && b.1 <= a.3
            })
            .map(|(_, r)| crossings(&routes[ri].points, &r.points))
            .sum()
    };
    for ports in port_groups(routes, conns, shapes).into_values() {
        // a hyperedge's ports stay; the others of the side may still swap (a swap is kept only if
        // it removes crossings, the grouped routes' included)
        let ports: Vec<PortRef> = ports
            .into_iter()
            .filter(|p| groups_of[p.route].is_none())
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
                if ports {
                    move_port(&mut routes[a.route].points, a.at_start, side, tb);
                    move_port(&mut routes[b.route].points, b.at_start, side, ta);
                }
                if turns && !swap_turns(routes, a, b, shapes) {
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

/// Index of the first bend after the port and of the corner after it, when the route leaves the
/// port straight then turns (at least three points).
fn first_turn(pts: &[Point], at_start: bool) -> Option<(usize, usize)> {
    if pts.len() < 3 {
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
        let (s, e) = (pts[0], pts[pts.len() - 1]);
        let lo = i.min(j).saturating_sub(1);
        let hi = (i.max(j) + 1).min(pts.len() - 1);
        // a shape holding an end inside (a region around it) is crossed anyway; the end shapes
        // themselves, whose borders carry the ports, are not
        shapes.values().all(|r| {
            let own = |p: Point| {
                p.0 > r.x + EPS && p.0 < r.x + r.w - EPS && p.1 > r.y + EPS && p.1 < r.y + r.h - EPS
            };
            own(s) || own(e) || (lo..hi).all(|k| !segment_hits(pts[k], pts[k + 1], r))
        })
    };
    if !clear(&na, a1, a2) || !clear(&nb, b1, b2) {
        return false;
    }
    routes[a.route].points = na;
    routes[b.route].points = nb;
    true
}

/// Does the axis-aligned segment `p`–`q` pass through the inside of `r`?
fn segment_hits(p: Point, q: Point, r: &Rect) -> bool {
    let (x0, x1) = (p.0.min(q.0), p.0.max(q.0));
    let (y0, y1) = (p.1.min(q.1), p.1.max(q.1));
    x1 > r.x + EPS && x0 < r.x + r.w - EPS && y1 > r.y + EPS && y0 < r.y + r.h - EPS
}

/// Ports spread evenly around the middle of side `[lo, hi]`.
/// How far the end segment at a port runs before its first turn (the whole route if straight).
fn first_turn_depth(pts: &[Point], at_start: bool) -> Option<f64> {
    let (a, b) = if at_start {
        (*pts.first()?, *pts.get(1)?)
    } else {
        (*pts.last()?, *pts.get(pts.len().checked_sub(2)?)?)
    };
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
        let Some((a, b)) = clear_part(shapes, strip, (lo, hi), margin, pad) else {
            return Some((lo, hi, margin));
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

fn clear_part(
    shapes: &BTreeMap<&str, Rect>,
    (rect, side, depth): (&Rect, Side, f64),
    (lo, hi): (f64, f64),
    margin: f64,
    pad: f64,
) -> Option<(f64, f64)> {
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
        return None;
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
        .map(|(a, b)| {
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
            (a, b)
        })
}

fn even_targets(n: usize, lo: f64, hi: f64, margin: f64, params: &Params) -> Vec<f64> {
    let n = n as f64;
    let mut s = params.ideal_nudging.max(params.port_spacing);
    // wider apart on a roomy side, up to `port_spread`: arrows read as distinct, not as a cable
    s = s.max(params.port_spread.min((hi - lo) / (n + 1.0)));
    if (n - 1.0) * s > (hi - lo) - 2.0 * margin {
        s = (((hi - lo) - 2.0 * margin) / (n - 1.0)).max(0.0);
    }
    let center = (lo + hi) / 2.0;
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
        for k in ks {
            let (a, b) = (p[k], p[k + 1]);
            let is_v = (a.0 - b.0).abs() < EPS && (a.1 - b.1).abs() > EPS;
            let is_h = (a.1 - b.1).abs() < EPS && (a.0 - b.0).abs() > EPS;
            if vertical && is_v {
                out.push(Seg {
                    route: ri,
                    k,
                    vertical,
                    coord: a.0,
                    lo: a.1.min(b.1),
                    hi: a.1.max(b.1),
                    fixed: ends,
                });
            } else if !vertical && is_h {
                out.push(Seg {
                    route: ri,
                    k,
                    vertical,
                    coord: a.1,
                    lo: a.0.min(b.0),
                    hi: a.0.max(b.0),
                    fixed: ends,
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

/// Free middle segments: centred in the corridor the shapes on either side bound. A route going
/// round an obstacle no longer hugs the margin of the shape it aims at: the bend sits in the
/// middle of the free space, as a draughtsman would put it. Segments that carry a waypoint, whose
/// corridor is bounded on one side only, or whose move would cross a shape, stay. The result
/// depends only on the shapes bounding the corridor (stability, D-031).
fn center_segments(
    routes: &mut [Route],
    conns: &BTreeMap<&str, &Connector>,
    shapes: &BTreeMap<&str, Rect>,
) {
    let mut grouped: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (i, r) in routes.iter_mut().enumerate() {
        let conn = conns.get(r.id.as_str());
        if let Some(g) = conn.and_then(|c| c.group.clone()) {
            grouped.entry(g).or_default().push(i);
            continue;
        }
        let checkpoints: &[Point] = conn.map_or(&[], |c| &c.checkpoints);
        let n = r.points.len();
        for k in 1..n.saturating_sub(2) {
            if let Some(moved) = center_segment(&r.points, k, checkpoints, shapes, None) {
                r.points[k] = moved[1];
                r.points[k + 1] = moved[2];
            }
        }
    }
    for members in grouped.values() {
        center_group(routes, members, shapes);
    }
}

/// The branches of a hyperedge (R-18) are centred together: segments of the group that are
/// collinear and touch (the trunk splitting left and right) take the middle of the corridor along
/// their joint extent, all of them or none, so that the split stays at one point.
fn center_group(routes: &mut [Route], members: &[usize], shapes: &BTreeMap<&str, Rect>) {
    for &i in members {
        for k in 1..routes[i].points.len().saturating_sub(2) {
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
                    let same = if horizontal {
                        (p.1 - q.1).abs() < EPS && (p.1 - pos).abs() < 1.0
                    } else {
                        (p.0 - q.0).abs() < EPS && (p.0 - pos).abs() < 1.0
                    };
                    let (l, h) = ext(p, q);
                    if same && l <= hi + 1.0 && h >= lo - 1.0 {
                        sib.push((j, m));
                        lo = lo.min(l);
                        hi = hi.max(h);
                    }
                }
            }
            let span = Some((lo, hi));
            let moved: Option<Vec<_>> = sib
                .iter()
                .map(|&(j, m)| center_segment(&routes[j].points, m, &[], shapes, span))
                .collect();
            let Some(moved) = moved else { continue };
            for (&(j, m), mv) in sib.iter().zip(moved) {
                routes[j].points[m] = mv[1];
                routes[j].points[m + 1] = mv[2];
            }
        }
    }
}

/// Segment `k` (`pts[k]`–`pts[k + 1]`) moved to the middle of its corridor (the four points
/// `k - 1` to `k + 2`), or `None` if there is no reason to or it cannot move. `span` replaces the
/// segment's own extent to find the corridor (the joint extent of a hyperedge's branches).
fn center_segment(
    pts: &[Point],
    k: usize,
    checkpoints: &[Point],
    shapes: &BTreeMap<&str, Rect>,
    span: Option<(f64, f64)>,
) -> Option<[Point; 4]> {
    let (a, b) = (pts[k], pts[k + 1]);
    let horizontal = (a.1 - b.1).abs() < EPS && (a.0 - b.0).abs() > EPS;
    let vertical = (a.0 - b.0).abs() < EPS && (a.1 - b.1).abs() > EPS;
    if !(horizontal || vertical) || checkpoints.iter().any(|&c| on_segment(a, b, c)) {
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
    for r in shapes.values() {
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
    if !(before.is_finite() && after.is_finite()) {
        return None;
    }
    let mut moved = [pts[k - 1], pts[k], pts[k + 1], pts[k + 2]];
    let mid = (before + after) / 2.0;
    if (mid - pos).abs() < EPS {
        return Some(moved); // already in the middle
    }
    // the (perpendicular) neighbours keep their direction: a segment is never turned round
    let coord = |p: Point| if horizontal { p.1 } else { p.0 };
    let (prev, next) = (coord(pts[k - 1]), coord(pts[k + 2]));
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
    let margin = (params.shape_buffer / 2.0).max(4.0);
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
    let (anchor, coord) = match fixed.first() {
        Some(&f) => (f as f64, segs[slotted[f][0]].coord),
        None => ((n - 1.0) / 2.0, segs[firsts[0]].coord),
    };
    let (left, right) = channel(shapes, vertical, coord, lo, hi);
    let mut s = params.ideal_nudging;
    for (room, slots_beyond) in [
        (coord - left - margin, anchor),
        (right - coord - margin, n - 1.0 - anchor),
    ] {
        if slots_beyond > 0.0 && slots_beyond * s > room {
            s = (room / slots_beyond).max(0.0);
        }
    }
    for (k, slot) in slotted.iter().enumerate() {
        let d = (k as f64 - anchor) * s;
        if d.abs() < EPS {
            continue;
        }
        for &m in slot.iter().filter(|&&m| !segs[m].fixed) {
            let sg = &segs[m];
            let pts = &mut routes[sg.route].points;
            for idx in [sg.k, sg.k + 1] {
                if vertical {
                    pts[idx].0 = sg.coord + d;
                } else {
                    pts[idx].1 = sg.coord + d;
                }
            }
        }
    }
}

/// R2 post-processing of every route.
pub(crate) fn beautify(
    routes: &mut [Route],
    connectors: &[Connector],
    shapes: &BTreeMap<&str, Rect>,
    params: &Params,
) {
    if !(params.ideal_nudging.is_finite() && params.ideal_nudging > 0.0) {
        return;
    }
    let conns: BTreeMap<&str, &Connector> = connectors.iter().map(|c| (c.id.as_str(), c)).collect();
    let groups: Vec<Option<String>> = routes
        .iter()
        .map(|r| conns.get(r.id.as_str()).and_then(|c| c.group.clone()))
        .collect();
    spread_ports(routes, &conns, shapes, params);
    center_segments(routes, &conns, shapes);
    // a horizontal segment moved lengthens the vertical ones beside it, which may then lie on
    // another line: the two passes again, until nothing moves (three rounds at most)
    for _ in 0..3 {
        let before: Vec<Vec<Point>> = routes.iter().map(|r| r.points.clone()).collect();
        nudge(routes, shapes, params, true, &groups);
        nudge(routes, shapes, params, false, &groups);
        if routes.iter().zip(&before).all(|(r, b)| r.points == *b) {
            break;
        }
    }
    separate_end_overlaps(routes, &conns, shapes, params, &groups);
    uncross_ports(routes, &conns, shapes, &groups);
    for r in routes.iter_mut() {
        merge(&mut r.points);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn orthogonal(p: &[Point]) -> bool {
        p.windows(2)
            .all(|w| (w[0].0 - w[1].0).abs() < EPS || (w[0].1 - w[1].1).abs() < EPS)
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
