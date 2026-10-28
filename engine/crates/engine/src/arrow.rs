//! Geometry of an arrow: shortened line, arrival head, start head (tail) and origin dot. Shared by
//! the SVG and HTML renderings (same drawing), with the arrow's classes.

use crate::diagram::Diagram;
use crate::svg::n;

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

/// Classes of an arrow: `edge`, its own, then those of its ends (`from-…`, `to-…`).
pub fn edge_classes(d: &Diagram, e: &skemo_lang::Edge) -> Vec<String> {
    let mut cls = vec!["edge".to_string()];
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
        out.extend_from_slice(&line[i + 1..]);
        return Some(out);
    }
    None
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

/// Radius of a bridge (px): a horizontal line hops over a vertical one it crosses.
pub const HOP: f64 = 5.0;

/// Where the horizontal segments of arrow `id` cross the vertical segment of another arrow (not
/// of its group: a shared trunk is one line), in the drawing's frame (`ox`, `oy`): each gets a
/// bridge, so that a crossing does not read as a junction. Crossings too close to a bend of
/// either line (where a bridge would bite into the rounded corner) get none.
pub fn hops(d: &Diagram, id: &str, (ox, oy): (f64, f64)) -> Vec<(f64, f64)> {
    const CORNER: f64 = 8.0;
    let Some(me) = d.routes.get(id) else {
        return Vec::new();
    };
    let group = d.edge_groups.get(id);
    let verticals: Vec<((f64, f64), (f64, f64))> = d
        .routes
        .iter()
        .filter(|(o, _)| *o != id && (group.is_none() || d.edge_groups.get(*o) != group))
        .flat_map(|(_, r)| r.windows(2).map(|w| (w[0], w[1])).collect::<Vec<_>>())
        .filter(|(p, q)| (p.0 - q.0).abs() < 0.5 && (p.1 - q.1).abs() > 0.5)
        .collect();
    let room = HOP + CORNER;
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
