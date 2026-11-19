//! Geometry of an arrow: shortened line, arrival head, start head (tail) and origin dot. Shared by
//! the SVG and HTML renderings (same drawing), with the arrow's classes.

use crate::diagram::Diagram;
use crate::svg::n;
use std::fmt::Write as _;

/// Size of the head (px): length of the segment taken off the line.
pub const SIZE: f64 = 7.0;

/// Un triangle (pointe ou queue).
pub type Triangle = [(f64, f64); 3];

/// Drawn elements of an arrow.
pub struct Arrow {
    /// Path of the line, shortened to end at the base of the head.
    pub line: Vec<(f64, f64)>,
    pub head: Triangle,
    pub tail: Triangle,
    /// Start point (centre of the origin dot).
    pub origin: (f64, f64),
}

/// Head of length `1.15 × SIZE` and half-width `0.57 × SIZE` (8 × 8 px, D-049: the hand drawings'
/// lighter head), with its tip at `tip`, its axis along direction `(ux, uy)` leaving the base.
fn triangle(tip: (f64, f64), (ux, uy): (f64, f64), sign: f64) -> Triangle {
    let (bx, by) = tip;
    let s = SIZE;
    [
        (bx, by),
        (
            bx + sign * ux * s * 1.15 - uy * s * 0.57,
            by + sign * uy * s * 1.15 + ux * s * 0.57,
        ),
        (
            bx + sign * ux * s * 1.15 + uy * s * 0.57,
            by + sign * uy * s * 1.15 - ux * s * 0.57,
        ),
    ]
}

/// Drawing of the arrow for a route of at least two points (image coordinates).
pub fn arrow(pts: &[(f64, f64)]) -> Arrow {
    let (ax, ay) = pts[pts.len() - 2];
    let (bx, by) = pts[pts.len() - 1];
    let l = (bx - ax).hypot(by - ay).max(1e-9);
    let (ux, uy) = ((bx - ax) / l, (by - ay) / l);
    let mut line = pts[..pts.len() - 1].to_vec();
    line.push((bx - ux * SIZE, by - uy * SIZE));
    let (cx, cy) = pts[0];
    let (dx, dy) = pts[1];
    let l2 = (dx - cx).hypot(dy - cy).max(1e-9);
    let v = ((dx - cx) / l2, (dy - cy) / l2);
    Arrow {
        line,
        head: triangle((bx, by), (ux, uy), -1.0),
        tail: triangle((cx, cy), v, 1.0),
        origin: (cx, cy),
    }
}

/// The line an arrow draws: up to the base of its head, or, when its head is no triangle (hidden
/// by the theme, or a dot with `--shape: dot`), to the end of its route, so that it touches the
/// card instead of stopping short of it.
pub fn drawn_line(d: &Diagram, id: &str, a: &Arrow, pts: &[(f64, f64)]) -> Vec<(f64, f64)> {
    let triangle = d.scene.edges.get(id).is_none_or(|el| {
        let hc = d.scene.style(el.head);
        hc.display() != "none" && hc.get("--shape").map(str::trim) != Some("dot")
    });
    let mut line = a.line.clone();
    if !triangle && let (Some(last), Some(end)) = (line.last_mut(), pts.last()) {
        *last = *end;
    }
    line
}

/// Attribut `d` d'un triangle : `M x,y L x,y L x,y Z`.
pub fn triangle_path(t: &Triangle) -> String {
    format!(
        "M{},{} L{},{} L{},{} Z",
        n(t[0].0),
        n(t[0].1),
        n(t[1].0),
        n(t[1].1),
        n(t[2].0),
        n(t[2].1)
    )
}

/// Classes of an arrow: `edge`, `tunnel` for a tunnel, its own, then those of its ends (`from-…`, `to-…`).
pub fn edge_classes(d: &Diagram, e: &skemo_lang::Edge) -> Vec<String> {
    let mut cls = vec!["edge".to_string()];
    // a tunnel is styled by its tag (D-116)
    if e.tunnel {
        cls.push("tunnel".into());
    }
    cls.extend(e.classes.iter().cloned());
    // a node's classes, then those of the layout mode (`mm-b2`…), as in `scene::add_edges`
    let classes = |id: &str| {
        let mut c: Vec<String> = d
            .model
            .node(id)
            .map(|n| n.classes.clone())
            .unwrap_or_default();
        c.extend(d.mode_extras.classes.get(id).cloned().unwrap_or_default());
        c
    };
    cls.extend(classes(&e.from).iter().map(|c| format!("from-{c}")));
    cls.extend(classes(&e.to).iter().map(|c| format!("to-{c}")));
    cls
}

/// The routes (rendering frame) of the arrows of `e`'s group drawn before it that appear with it
/// (same animation steps): the parts they draw need not be drawn again.
fn earlier_mates(d: &Diagram, e: &skemo_lang::Edge) -> Vec<Vec<(f64, f64)>> {
    let Some(g) = d.edge_groups.get(&e.id) else {
        return Vec::new();
    };
    // when an arrow shows: its steps of appearance, of disappearance and of focus
    let when = |id: &str| {
        d.anim.elements.get(id).map(|r| {
            (
                r.appear.clone(),
                r.until.clone(),
                r.fx.clone(),
                r.focus.clone(),
            )
        })
    };
    let step = when(&e.id);
    d.model
        .edges
        .iter()
        .take_while(|f| f.id != e.id)
        .filter(|f| d.edge_groups.get(&f.id) == Some(g) && when(&f.id) == step)
        .filter_map(|f| d.routes.get(&f.id))
        .map(|r| {
            r.iter()
                .map(|p| (p.0 + d.frame.ox, p.1 + d.frame.oy))
                .collect()
        })
        .collect()
}

/// How far from `a` towards `b` the segment `a`–`b` runs on the given routes (0 to 1): the
/// collinear segments overlapping it, chained from `a`.
fn covered(a: (f64, f64), b: (f64, f64), mates: &[Vec<(f64, f64)>]) -> f64 {
    let len = (b.0 - a.0).hypot(b.1 - a.1);
    if len < 1e-9 {
        return 1.0;
    }
    let (ux, uy) = ((b.0 - a.0) / len, (b.1 - a.1) / len);
    // the parameter range of each collinear segment, along a–b
    let mut spans: Vec<(f64, f64)> = mates
        .iter()
        .flat_map(|r| r.windows(2))
        .filter_map(|w| {
            let off = |p: (f64, f64)| ((p.0 - a.0) * uy - (p.1 - a.1) * ux).abs();
            let along = |p: (f64, f64)| (p.0 - a.0) * ux + (p.1 - a.1) * uy;
            (off(w[0]) < 0.5 && off(w[1]) < 0.5).then(|| {
                let (s, t) = (along(w[0]), along(w[1]));
                (s.min(t), s.max(t))
            })
        })
        .collect();
    spans.sort_by(|p, q| p.0.total_cmp(&q.0));
    let mut reach = 0.0;
    for (s, t) in spans {
        if s <= reach + 0.5 {
            reach = f64::max(reach, t);
        }
    }
    (reach / len).clamp(0.0, 1.0)
}

/// The part of an arrow's line not already drawn by the arrows of its group drawn before it
/// (D-049): a shared trunk is drawn once, each branch from where it leaves the others. Drawn
/// several times, the trunk's anti-aliased edges added up and it looked thicker. A line wholly
/// drawn already is kept whole.
///
/// Arrows into one card that share a trunk (a named join, D-050) share the end of their lines
/// instead: a later one stops where it meets the others, and draws no head (`false`).
pub fn unshared(d: &Diagram, e: &skemo_lang::Edge, line: &[(f64, f64)]) -> (Vec<(f64, f64)>, bool) {
    let mates = earlier_mates(d, e);
    if mates.is_empty() {
        return (line.to_vec(), true);
    }
    let line = trim_start(line, &mates).unwrap_or_else(|| line.to_vec());
    let back: Vec<(f64, f64)> = line.iter().rev().copied().collect();
    match trim_start(&back, &mates) {
        Some(mut t) => {
            t.reverse();
            (t, false)
        }
        None => (line, true),
    }
}

/// `line` from where it leaves the given routes; `None` when it does not start on them, or runs
/// on them all along.
fn trim_start(line: &[(f64, f64)], mates: &[Vec<(f64, f64)>]) -> Option<Vec<(f64, f64)>> {
    for (i, w) in line.windows(2).enumerate() {
        let t = covered(w[0], w[1], mates);
        if t >= 1.0 - 1e-9 {
            continue;
        }
        if i == 0 && t * (w[1].0 - w[0].0).hypot(w[1].1 - w[0].1) < 0.5 {
            return None;
        }
        let cut = (
            w[0].0 + (w[1].0 - w[0].0) * t,
            w[0].1 + (w[1].1 - w[0].1) * t,
        );
        let mut out = vec![cut];
        // start a little back on the trunk: the trunk's own bends are drawn rounded, so the cut
        // as computed on the route would leave a gap; and leaving at a bend, the branch draws
        // its own rounded bend into the trunk (D-093: curved junctions, no dot)
        let (from, to) = if t < 1e-9 && i > 0 {
            (line[i - 1], line[i])
        } else {
            (w[0], cut)
        };
        let len = (to.0 - from.0).hypot(to.1 - from.1);
        if len > 0.5 {
            let back = JOIN_LEAD.min(len);
            let lead = (
                to.0 - (to.0 - from.0) / len * back,
                to.1 - (to.1 - from.1) / len * back,
            );
            if t < 1e-9 && i > 0 {
                out.insert(0, lead);
            } else {
                out[0] = lead;
            }
        }
        out.extend_from_slice(&line[i + 1..]);
        return Some(out);
    }
    None
}

/// How far back on a shared trunk a branch starts, so that its bend into the trunk is drawn
/// rounded (px; at least twice the corner radius).
const JOIN_LEAD: f64 = 16.0;

/// Junctions drawn as rounded bends into the trunk, without a dot (D-093, user 2026-11-06);
/// `false` gives back the sharp corner and its dot (D-047).
pub const CURVED_JUNCTIONS: bool = true;

/// Length of a line as drawn: its corners rounded with radius `r` (as `rounded_path_hops` draws
/// them), each rounded corner a quadratic curve measured by sampling.
pub fn drawn_length(line: &[(f64, f64)], r: f64) -> f64 {
    let dist = |a: (f64, f64), b: (f64, f64)| (b.0 - a.0).hypot(b.1 - a.1);
    let mut total: f64 = line.windows(2).map(|w| dist(w[0], w[1])).sum();
    for i in 1..line.len().saturating_sub(1) {
        let (p, c, q) = (line[i - 1], line[i], line[i + 1]);
        let (l1, l2) = (dist(p, c), dist(c, q));
        if l1 == 0.0 || l2 == 0.0 {
            continue;
        }
        let k = r.min(l1 / 2.0).min(l2 / 2.0);
        let a = (c.0 - (c.0 - p.0) / l1 * k, c.1 - (c.1 - p.1) / l1 * k);
        let b = (c.0 + (q.0 - c.0) / l2 * k, c.1 + (q.1 - c.1) / l2 * k);
        let at = |t: f64| {
            let u = 1.0 - t;
            (
                u * u * a.0 + 2.0 * u * t * c.0 + t * t * b.0,
                u * u * a.1 + 2.0 * u * t * c.1 + t * t * b.1,
            )
        };
        let curve: f64 = (0..16)
            .map(|j| dist(at(f64::from(j) / 16.0), at(f64::from(j + 1) / 16.0)))
            .sum();
        total += curve - 2.0 * k;
    }
    total
}

/// Where a grouped arrow's dashes start, so that they fall on those of the trunk it shares
/// (D-093): a fork's branch counts from the shared source, the length drawn before the part it
/// draws itself; the arrows of a join count back from the shared target (negative). `full`: the
/// arrow's whole line, `line`: the part it draws; 0 for an arrow in no group.
pub fn dash_phase(
    d: &Diagram,
    e: &skemo_lang::Edge,
    full: &[(f64, f64)],
    line: &[(f64, f64)],
) -> f64 {
    let Some(g) = d.edge_groups.get(&e.id) else {
        return 0.0;
    };
    if g.contains("|join|") {
        return -drawn_length(full, CORNER);
    }
    let Some(&start) = line.first() else {
        return 0.0;
    };
    // the whole line up to where the drawn part starts
    let mut prefix = vec![full[0]];
    for w in full.windows(2) {
        let on = {
            let (a, b) = (w[0], w[1]);
            let cross = (b.0 - a.0) * (start.1 - a.1) - (b.1 - a.1) * (start.0 - a.0);
            let len = (b.0 - a.0).hypot(b.1 - a.1).max(1e-9);
            let inside = |v: f64, s: f64, t: f64| v >= s.min(t) - 0.5 && v <= s.max(t) + 0.5;
            (cross / len).abs() < 0.5 && inside(start.0, a.0, b.0) && inside(start.1, a.1, b.1)
        };
        if on {
            prefix.push(start);
            return drawn_length(&prefix, CORNER);
        }
        prefix.push(w[1]);
    }
    0.0
}

/// Radius of an arrow's rounded corners (px).
pub const CORNER: f64 = 8.0;

/// The points an arrow draws as a sharp corner: its junctions, unless they are curved.
pub fn sharp_corners(d: &Diagram, id: &str) -> Vec<(f64, f64)> {
    if CURVED_JUNCTIONS {
        return Vec::new();
    }
    crate::route::junctions_on(&d.routes, &d.edge_groups, id, (d.frame.ox, d.frame.oy))
}

/// The junction dots an arrow draws: those no arrow of its group drawn before it passes through
/// (D-049), so that each dot is drawn once.
pub fn own_dots(d: &Diagram, e: &skemo_lang::Edge, sharp: &[(f64, f64)]) -> Vec<(f64, f64)> {
    let mates = earlier_mates(d, e);
    sharp
        .iter()
        .copied()
        .filter(|p| {
            !mates.iter().any(|r| {
                r.windows(2).any(|w| {
                    let inside =
                        |v: f64, s: f64, t: f64| v >= s.min(t) - 0.5 && v <= s.max(t) + 0.5;
                    let cross =
                        (w[1].0 - w[0].0) * (p.1 - w[0].1) - (w[1].1 - w[0].1) * (p.0 - w[0].0);
                    let len = (w[1].0 - w[0].0).hypot(w[1].1 - w[0].1).max(1e-9);
                    (cross / len).abs() < 0.5
                        && inside(p.0, w[0].0, w[1].0)
                        && inside(p.1, w[0].1, w[1].1)
                })
            })
        })
        .collect()
}

/// Half the gap cut in a horizontal line where it passes under a vertical one it crosses (px).
pub const HOP: f64 = 5.0;

/// A mask that cuts a line at its crossings (`hops`, in the drawing's frame), `HOP` on each side,
/// and the attribute that applies it; `None` without crossings. A mask rather than a break in
/// the path: dashes and the progressive drawing run on as if the line were whole (D-093).
///
/// `inside`: boxes where the line is hidden whole, the cards an arrow through a tunnel crosses
/// (D-116).
pub fn gap_mask(
    prefix: &str,
    hops: &[(f64, f64)],
    inside: &[crate::layout::Box2],
) -> Option<(String, String)> {
    let first = hops
        .first()
        .copied()
        .or_else(|| inside.first().map(|b| (b.x, b.y)))?;
    let num = |v: f64| format!("{v:.0}").replace('-', "m");
    let id = format!("{prefix}-gap{}x{}", num(first.0), num(first.1));
    let mut cuts = String::new();
    for b in inside {
        let _ = write!(
            cuts,
            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"black\"/>",
            n(b.x),
            n(b.y),
            n(b.w),
            n(b.h)
        );
    }
    for (x, y) in hops {
        let _ = write!(
            cuts,
            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"black\"/>",
            n(x - HOP),
            n(y - HOP),
            n(2.0 * HOP),
            n(2.0 * HOP)
        );
    }
    let mask = format!(
        "<mask id=\"{id}\" maskUnits=\"userSpaceOnUse\" x=\"-100000\" y=\"-100000\" width=\"200000\" height=\"200000\"><rect x=\"-100000\" y=\"-100000\" width=\"200000\" height=\"200000\" fill=\"white\"/>{cuts}</mask>"
    );
    Some((mask, format!(" mask=\"url(#{id})\"")))
}

/// Where the horizontal segments of arrow `id` cross the vertical segment of another arrow (not
/// of its group: a shared trunk is one line, nor one never shown at the same time: two lines of
/// different steps never meet on screen), in the drawing's frame (`ox`, `oy`): each gets a
/// gap (`gap_mask`), so that a crossing does not read as a junction; near a bend too.
pub fn hops(d: &Diagram, id: &str, (ox, oy): (f64, f64)) -> Vec<(f64, f64)> {
    let Some(me) = d.routes.get(id) else {
        return Vec::new();
    };
    let life = |id: &str| {
        d.anim
            .elements
            .get(id)
            .map_or((f64::NEG_INFINITY, f64::INFINITY), |r| {
                (
                    r.appear.value,
                    r.until.as_ref().map_or(f64::INFINITY, |u| u.value),
                )
            })
    };
    let (a, z) = life(id);
    let group = d.edge_groups.get(id);
    let verticals: Vec<((f64, f64), (f64, f64))> = d
        .routes
        .iter()
        .filter(|(o, _)| *o != id && (group.is_none() || d.edge_groups.get(*o) != group))
        .filter(|(o, _)| {
            let (b, y) = life(o);
            b <= z && a <= y
        })
        .flat_map(|(_, r)| r.windows(2).map(|w| (w[0], w[1])).collect::<Vec<_>>())
        .filter(|(p, q)| (p.0 - q.0).abs() < 0.5 && (p.1 - q.1).abs() > 0.5)
        .collect();
    // a gap bites into nothing (D-093): every crossing inside both segments is cut, even near a
    // bend (a bridge needed HOP + CORNER of straight line)
    let room = 1.0;
    let mut out = Vec::new();
    for w in me.windows(2) {
        let (p, q) = (w[0], w[1]);
        if (p.1 - q.1).abs() >= 0.5 || (p.0 - q.0).abs() < 0.5 {
            continue;
        }
        let (x0, x1) = (p.0.min(q.0), p.0.max(q.0));
        for (a, b) in &verticals {
            let (y0, y1) = (a.1.min(b.1), a.1.max(b.1));
            let (x, y) = (a.0, p.1);
            if x > x0 + room && x < x1 - room && y > y0 + room && y < y1 - room {
                out.push((x + ox, y + oy));
            }
        }
    }
    out
}

/// `--curve: on` on an arrow's line: one smooth curve from end to end. Only in a mind map, whose
/// layout leaves room for it; elsewhere a curve would cut through cards (warning in `build`).
pub fn curved(d: &crate::diagram::Diagram, line_el: usize) -> bool {
    d.model.option("layout") == Some("mindmap") && asks_curve(&d.scene, line_el)
}

/// The theme or a stylesheet asks for a curve on this line.
pub fn asks_curve(scene: &crate::scene::Scene, line_el: usize) -> bool {
    scene.style(line_el).get("--curve").map(str::trim) == Some("on")
}

/// The cards an arrow through a tunnel crosses (D-116), in the drawing's frame: its line is
/// hidden there.
pub fn tunnel_cuts(d: &Diagram, e: &skemo_lang::Edge) -> Vec<crate::layout::Box2> {
    crate::tunnel::cuts(&d.model, e)
        .into_iter()
        .filter_map(|c| d.boxes.get(c))
        .map(|b| crate::layout::Box2 {
            x: b.x + d.frame.ox,
            y: b.y + d.frame.oy,
            ..*b
        })
        .collect()
}

/// The arrows in drawing order: the tunnels first, under the arrows that pass through them
/// (D-116).
pub fn drawing_order(model: &skemo_lang::Model) -> impl Iterator<Item = &skemo_lang::Edge> {
    let (tunnels, rest): (Vec<_>, Vec<_>) = model.edges.iter().partition(|e| e.tunnel);
    tunnels.into_iter().chain(rest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn head_points_along_the_last_segment() {
        let a = arrow(&[(0.0, 0.0), (100.0, 0.0)]);
        assert_eq!(
            a.line.last(),
            Some(&(93.0, 0.0)),
            "ligne raccourcie de SIZE"
        );
        assert_eq!(a.head[0], (100.0, 0.0), "tip of the head on the end");
        assert!(
            a.head[1].0 < 100.0 && a.head[2].0 < 100.0,
            "base behind the tip"
        );
        assert!((a.head[1].1 + a.head[2].1).abs() < 1e-9, "symmetric base");
    }

    #[test]
    fn tail_opens_forward_from_the_origin() {
        let a = arrow(&[(10.0, 10.0), (10.0, 90.0)]);
        assert_eq!(a.origin, (10.0, 10.0));
        assert!(a.tail[1].1 > 10.0 && a.tail[2].1 > 10.0);
    }

    #[test]
    fn degenerate_segment_does_not_produce_nan() {
        let a = arrow(&[(5.0, 5.0), (5.0, 5.0)]);
        assert!(
            a.head
                .iter()
                .chain(&a.tail)
                .all(|p| p.0.is_finite() && p.1.is_finite())
        );
    }
}
