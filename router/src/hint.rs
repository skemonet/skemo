//! R-15: a hint route (ELK's) taken as is when it can be used unchanged.

use std::collections::BTreeMap;

use crate::geom::{Point, Rect, Side};
use crate::model::{Connector, End};
use crate::scene::Scene;
use crate::simplify::merge;

/// Tolerance (px) for snapping an end onto its shape's border.
const SNAP_TOL: f64 = 1.0;
/// Gap (px) allowed between an end and the exact border.
const BORDER_EPS: f64 = 0.01;
/// A slanted segment whose small component is under this threshold (px) is straightened.
const STRAIGHTEN_BELOW: f64 = 0.5;

fn finite(p: Point) -> bool {
    p.0.is_finite() && p.1.is_finite()
}

/// A hint route usable as is (R-15): orthogonal, ends on the shapes' borders (perpendicular, on
/// the imposed side if any), no active obstacle crossed, no waypoint asked for.
pub(crate) fn valid_hint(
    c: &Connector,
    active: &[bool],
    scene: &Scene,
    by_id: &BTreeMap<&str, usize>,
) -> Option<Vec<Point>> {
    let h = c.hint.as_ref()?;
    if h.len() < 2 || !c.checkpoints.is_empty() || !h.iter().all(|q| finite(*q)) {
        return None;
    }
    let mut pts = h.clone();
    merge(&mut pts);
    straighten_near_orthogonal(&mut pts);
    merge(&mut pts);
    if pts.len() < 2 || !is_orthogonal(&pts) {
        return None;
    }
    let rect_of = |e: &End| match e {
        End::Shape { id, .. } => by_id.get(id.as_str()).map(|&k| scene.items[k].hard),
        End::Point { .. } => None,
    };
    let n = pts.len();
    let next = pts[1];
    snap_to_border(rect_of(&c.from), &mut pts[0], next);
    let prev = pts[n - 2];
    snap_to_border(rect_of(&c.to), &mut pts[n - 1], prev);
    let n = pts.len();
    if !end_ok(&c.from, rect_of(&c.from), pts[0], pts[1])
        || !end_ok(&c.to, rect_of(&c.to), pts[n - 1], pts[n - 2])
    {
        return None;
    }
    if pts.windows(2).any(|w| scene.blocked(w[0], w[1], active)) {
        return None;
    }
    Some(pts)
}

/// Slanted segments of less than half a pixel (ELK's rounding): straightened.
fn straighten_near_orthogonal(pts: &mut [Point]) {
    for i in 1..pts.len() {
        let (dx, dy) = (pts[i].0 - pts[i - 1].0, pts[i].1 - pts[i - 1].1);
        if dy != 0.0 && dy.abs() < STRAIGHTEN_BELOW && dx.abs() >= 1.0 {
            pts[i].1 = pts[i - 1].1;
        } else if dx != 0.0 && dx.abs() < STRAIGHTEN_BELOW && dy.abs() >= 1.0 {
            pts[i].0 = pts[i - 1].0;
        }
    }
}

fn is_orthogonal(pts: &[Point]) -> bool {
    pts.windows(2)
        .all(|w| (w[0].0 - w[1].0).abs() < 1e-6 || (w[0].1 - w[1].1).abs() < 1e-6)
}

/// Is the end `at` (the next segment going towards `next`) valid for `end`?
fn end_ok(end: &End, rect: Option<Rect>, at: Point, next: Point) -> bool {
    match end {
        End::Shape { side, .. } => {
            let Some(r) = rect else { return false };
            match side {
                Some(s) => leaves_side(r, *s, at, next),
                None => Side::ALL.iter().any(|&s| leaves_side(r, s, at, next)),
            }
        }
        End::Point { x, y } => (at.0 - x).abs() < BORDER_EPS && (at.1 - y).abs() < BORDER_EPS,
    }
}

/// `at` is on side `side` of `r` (corners excluded) and the segment moves away from it towards `next`.
fn leaves_side(r: Rect, side: Side, at: Point, next: Point) -> bool {
    let within_x = at.0 > r.x0() && at.0 < r.x1();
    let within_y = at.1 > r.y0() && at.1 < r.y1();
    match side {
        Side::Top => (at.1 - r.y0()).abs() < BORDER_EPS && within_x && next.1 < at.1,
        Side::Bottom => (at.1 - r.y1()).abs() < BORDER_EPS && within_x && next.1 > at.1,
        Side::Left => (at.0 - r.x0()).abs() < BORDER_EPS && within_y && next.0 < at.0,
        Side::Right => (at.0 - r.x1()).abs() < BORDER_EPS && within_y && next.0 > at.0,
    }
}

/// An end less than a pixel from the border (a shape fixed but for rounding): snapped onto the
/// border, along its segment, which stays perpendicular (stability, D-031).
fn snap_to_border(rect: Option<Rect>, at: &mut Point, next: Point) {
    let Some(r) = rect else { return };
    if (at.0 - next.0).abs() < 1e-6 && at.0 > r.x0() && at.0 < r.x1() {
        for y in [r.y0(), r.y1()] {
            if (at.1 - y).abs() <= SNAP_TOL {
                at.1 = y;
            }
        }
    } else if (at.1 - next.1).abs() < 1e-6 && at.1 > r.y0() && at.1 < r.y1() {
        for x in [r.x0(), r.x1()] {
            if (at.0 - x).abs() <= SNAP_TOL {
                at.0 = x;
            }
        }
    }
}

/// Straight route between two shapes that face each other (their facing sides overlap across):
/// one segment, at the middle of the shorter side when the longer one can take it there, else as
/// close to it as the overlap allows, keeping `margin` from the corners. It removes the small
/// jog between two cards of different heights whose middles are not aligned (a card taller than
/// its neighbours). Arrows both ways get the same line here; `nudge::spread_bundles` sets them
/// apart, both ends together. Only for a single connector (not a hyperedge) without waypoints, whose
/// forced sides, if any, are the facing ones, and when nothing active is in the way.
pub(crate) fn straight(
    c: &Connector,
    active: &[bool],
    scene: &Scene,
    by_id: &BTreeMap<&str, usize>,
    margin: f64,
) -> Option<Vec<Point>> {
    if c.group.is_some() || !c.checkpoints.is_empty() {
        return None;
    }
    let (End::Shape { id: ia, side: sa }, End::Shape { id: ib, side: sb }) = (&c.from, &c.to)
    else {
        return None;
    };
    let a = scene.items[*by_id.get(ia.as_str())?].hard;
    let b = scene.items[*by_id.get(ib.as_str())?].hard;
    // facing sides: (side of a, side of b, along-axis spans, fixed coordinate of each border)
    let (side_a, side_b) = if b.x0() >= a.x1() {
        (Side::Right, Side::Left)
    } else if a.x0() >= b.x1() {
        (Side::Left, Side::Right)
    } else if b.y0() >= a.y1() {
        (Side::Bottom, Side::Top)
    } else if a.y0() >= b.y1() {
        (Side::Top, Side::Bottom)
    } else {
        return None;
    };
    if sa.is_some_and(|s| s != side_a) || sb.is_some_and(|s| s != side_b) {
        return None;
    }
    let horizontal = matches!(side_a, Side::Left | Side::Right);
    // decided on coordinates rounded to the pixel, like those of a pinned card (`at=`): pinning a
    // card in place must not switch a line between straight and bent (D-031)
    let span = |r: &Rect| {
        let (lo, hi) = if horizontal {
            (r.y0().round(), r.y1().round())
        } else {
            (r.x0().round(), r.x1().round())
        };
        let m = margin.min((hi - lo) / 4.0).round();
        (lo + m, hi - m, (lo + hi) / 2.0, hi - lo)
    };
    let (alo, ahi, amid, alen) = span(&a);
    let (blo, bhi, bmid, blen) = span(&b);
    let (lo, hi) = (alo.max(blo), ahi.min(bhi));
    if hi - lo < 1.0 {
        return None;
    }
    let want = if alen <= blen { amid } else { bmid };
    let t = want.clamp(lo, hi);
    let pa = side_a.point(&a, 0.0);
    let pb = side_b.point(&b, 0.0);
    let (p, q) = if horizontal {
        ((pa.0, t), (pb.0, t))
    } else {
        ((t, pa.1), (t, pb.1))
    };
    (!scene.blocked(p, q, active)).then(|| vec![p, q])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect() -> Rect {
        Rect::new(0.0, 0.0, 100.0, 50.0)
    }

    #[test]
    fn straightens_sub_half_pixel_obliques_only() {
        let mut p = vec![(0.0, 10.0), (30.0, 10.2), (30.0, 40.0)];
        straighten_near_orthogonal(&mut p);
        assert_eq!(p, vec![(0.0, 10.0), (30.0, 10.0), (30.0, 40.0)]);
        let mut q = vec![(0.0, 10.0), (30.0, 12.0)];
        straighten_near_orthogonal(&mut q);
        assert_eq!(q, vec![(0.0, 10.0), (30.0, 12.0)], "2 px: slant kept");
    }

    #[test]
    fn orthogonality() {
        assert!(is_orthogonal(&[(0.0, 0.0), (5.0, 0.0), (5.0, 9.0)]));
        assert!(!is_orthogonal(&[(0.0, 0.0), (5.0, 1.0)]));
    }

    #[test]
    fn leaving_a_side_needs_the_right_direction_and_no_corner() {
        let r = rect();
        assert!(leaves_side(r, Side::Bottom, (50.0, 50.0), (50.0, 80.0)));
        assert!(
            !leaves_side(r, Side::Bottom, (50.0, 50.0), (50.0, 20.0)),
            "inwards"
        );
        assert!(
            !leaves_side(r, Side::Bottom, (0.0, 50.0), (0.0, 80.0)),
            "coin exclu"
        );
        assert!(leaves_side(r, Side::Right, (100.0, 25.0), (130.0, 25.0)));
    }

    #[test]
    fn free_ends_must_match_exactly() {
        let e = End::Point { x: 3.0, y: 4.0 };
        assert!(end_ok(&e, None, (3.0, 4.0), (9.0, 4.0)));
        assert!(!end_ok(&e, None, (3.5, 4.0), (9.0, 4.0)));
    }

    #[test]
    fn snap_absorbs_a_sub_pixel_gap_along_the_segment() {
        let mut at = (50.0, 50.4);
        snap_to_border(Some(rect()), &mut at, (50.0, 90.0));
        assert_eq!(at, (50.0, 50.0));
        let mut far = (50.0, 53.0);
        snap_to_border(Some(rect()), &mut far, (50.0, 90.0));
        assert_eq!(far, (50.0, 53.0), "beyond the tolerance: unchanged");
    }
}
