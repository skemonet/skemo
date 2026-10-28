//! Simplification of the routes (R-10): aligned segments merged, micro-jogs removed.

use crate::geom::Point;

const EPS: f64 = 1e-9;

fn same(a: f64, b: f64) -> bool {
    (a - b).abs() <= EPS
}

/// Removes doubled points and the middle points of an alignment.
pub(crate) fn merge(p: &mut Vec<Point>) {
    p.dedup_by(|a, b| same(a.0, b.0) && same(a.1, b.1));
    let mut i = 1;
    while i + 1 < p.len() {
        let (a, b, c) = (p[i - 1], p[i], p[i + 1]);
        let aligned = (same(a.0, b.0) && same(b.0, c.0)) || (same(a.1, b.1) && same(b.1, c.1));
        if aligned {
            p.remove(i);
            i = i.max(2) - 1;
        } else {
            i += 1;
        }
    }
}

fn seg_len(a: Point, b: Point) -> f64 {
    (a.0 - b.0).abs() + (a.1 - b.1).abs()
}

/// Removes the jogs shorter than `min_jog` by aligning one of the two neighbouring segments on the
/// other, if the changed segments stay free (`free(a, b)`). The first and last points (on the
/// shapes) do not move.
pub(crate) fn remove_jogs(p: &mut Vec<Point>, min_jog: f64, free: impl Fn(Point, Point) -> bool) {
    merge(p);
    let mut guard = 0;
    'outer: while guard < 64 {
        guard += 1;
        let n = p.len();
        if n < 4 {
            return;
        }
        for k in 1..n - 2 {
            // jog = p[k] → p[k+1], between two parallel segments
            if seg_len(p[k], p[k + 1]) >= min_jog {
                continue;
            }
            let jog_vertical = same(p[k].0, p[k + 1].0);
            // (1) bring the next segment onto the previous one's line
            if k + 2 < n - 1 {
                let mut q = p.clone();
                if jog_vertical {
                    q[k + 1].1 = q[k].1;
                    q[k + 2].1 = q[k].1;
                } else {
                    q[k + 1].0 = q[k].0;
                    q[k + 2].0 = q[k].0;
                }
                if free(q[k], q[k + 2]) && free(q[k + 2], q[k + 3]) {
                    *p = q;
                    merge(p);
                    continue 'outer;
                }
            }
            // (2) bring the previous segment onto the next one's line
            if k >= 2 {
                let mut q = p.clone();
                if jog_vertical {
                    q[k].1 = q[k + 1].1;
                    q[k - 1].1 = q[k + 1].1;
                } else {
                    q[k].0 = q[k + 1].0;
                    q[k - 1].0 = q[k + 1].0;
                }
                if free(q[k - 2], q[k - 1]) && free(q[k - 1], q[k + 1]) {
                    *p = q;
                    merge(p);
                    continue 'outer;
                }
            }
        }
        return;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_removes_collinear_and_duplicates() {
        let mut p = vec![(0.0, 0.0), (0.0, 0.0), (5.0, 0.0), (10.0, 0.0), (10.0, 5.0)];
        merge(&mut p);
        assert_eq!(p, vec![(0.0, 0.0), (10.0, 0.0), (10.0, 5.0)]);
    }

    #[test]
    fn small_jog_is_removed() {
        let mut p = vec![
            (0.0, 0.0),
            (0.0, 10.0),
            (20.0, 10.0),
            (20.0, 12.0),
            (40.0, 12.0),
            (40.0, 30.0),
        ];
        remove_jogs(&mut p, 6.0, |_, _| true);
        assert_eq!(p, vec![(0.0, 0.0), (0.0, 10.0), (40.0, 10.0), (40.0, 30.0)]);
    }

    #[test]
    fn jog_kept_when_alignment_is_blocked() {
        let orig = vec![
            (0.0, 0.0),
            (0.0, 10.0),
            (20.0, 10.0),
            (20.0, 12.0),
            (40.0, 12.0),
            (40.0, 30.0),
        ];
        let mut p = orig.clone();
        remove_jogs(&mut p, 6.0, |_, _| false);
        assert_eq!(p, orig);
    }
}
