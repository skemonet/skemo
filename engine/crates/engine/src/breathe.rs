//! Room around the lines (D-063): a line does not run along another arrow's line, nor along a
//! zone's frame, closer than a fixed gap, when the drawing has room for it.
//!
//! The router squeezes parallel lines into a narrow corridor and lets a line run right along a
//! frame it may cross. After routing, every inner segment of an arrow (not the first nor the last,
//! which hold its ends) that runs closer than [`LINE_GAP`] to a parallel line of another arrow, or
//! closer than [`FRAME_GAP`] to the frame of a zone over some length, slides away to keep that gap,
//! or to the middle of its corridor when the corridor is narrower than two gaps. It never crosses
//! a card, a zone title, a frame or another line to get there, never comes closer to a card than
//! it was or than the router's margin, and never adds a crossing. Segments that run on one line
//! (a shared trunk) move together.

use crate::layout::Box2;
use skemo_lang::{Kind, Model};
use std::collections::BTreeMap;

type Points = Vec<(f64, f64)>;

/// Smallest gap between two parallel lines of different arrows (px).
pub const LINE_GAP: f64 = 20.0;
/// Smallest gap between a line and the frame of a zone it runs along (px).
pub const FRAME_GAP: f64 = 20.0;
/// Margin kept from cards when a segment moves (the router's margin).
const CARD_GAP: f64 = 14.0;
/// Two segments face each other when they run side by side over more than this (px).
const OVERLAP: f64 = 12.0;

/// One straight piece of an arrow: route `id`, points `k` and `k + 1`.
#[derive(Clone, Debug)]
struct Seg {
    id: String,
    k: usize,
    /// Horizontal: `level` is a y, the span runs along x
    horiz: bool,
    level: f64,
    lo: f64,
    hi: f64,
}

fn segments(routes: &BTreeMap<String, Points>) -> Vec<Seg> {
    let mut out = Vec::new();
    for (id, r) in routes {
        for (k, w) in r.windows(2).enumerate() {
            let (a, b) = (w[0], w[1]);
            let horiz = (a.1 - b.1).abs() < 0.01 && (a.0 - b.0).abs() > 0.01;
            let vert = (a.0 - b.0).abs() < 0.01 && (a.1 - b.1).abs() > 0.01;
            if !(horiz || vert) {
                continue;
            }
            let (level, u, v) = if horiz {
                (a.1, a.0, b.0)
            } else {
                (a.0, a.1, b.1)
            };
            out.push(Seg {
                id: id.clone(),
                k,
                horiz,
                level,
                lo: u.min(v),
                hi: u.max(v),
            });
        }
    }
    out
}

fn overlap(a: (f64, f64), b: (f64, f64)) -> f64 {
    a.1.min(b.1) - a.0.max(b.0)
}

/// What bounds a segment on one side: where it stands, the gap wanted from it, and whether that
/// gap is a hard limit (a card) or a wish (a line, a frame).
struct Bound {
    pos: f64,
    gap: f64,
    hard: bool,
}

struct Scene<'a> {
    frames: Vec<Box2>,
    /// Cards and zone titles: never crossed
    solids: Vec<Box2>,
    groups: &'a BTreeMap<String, String>,
}

impl Scene<'_> {
    fn same_group(&self, a: &str, b: &str) -> bool {
        a == b || matches!((self.groups.get(a), self.groups.get(b)), (Some(x), Some(y)) if x == y)
    }

    /// Lower and upper bounds of a segment's level, from what runs beside it.
    fn bounds(&self, s: &Seg, bundle: &[Seg], all: &[Seg]) -> (Vec<Bound>, Vec<Bound>) {
        let (mut below, mut above) = (Vec::new(), Vec::new());
        let mut put = |pos: f64, gap: f64, hard: bool| {
            if pos < s.level - 0.25 {
                below.push(Bound { pos, gap, hard });
            } else if pos > s.level + 0.25 {
                above.push(Bound { pos, gap, hard });
            }
        };
        let span = (s.lo, s.hi);
        let extent = |b: &Box2| {
            if s.horiz {
                ((b.x, b.right()), (b.y, b.bottom()))
            } else {
                ((b.y, b.bottom()), (b.x, b.right()))
            }
        };
        for f in &self.frames {
            let (along, (a, b)) = extent(f);
            if overlap(along, span) > OVERLAP {
                put(a, FRAME_GAP, false);
                put(b, FRAME_GAP, false);
            }
        }
        for c in &self.solids {
            let (along, (a, b)) = extent(c);
            if overlap(along, span) > 0.0 {
                // a card keeps the router's margin, or what the line already had
                let gap = |edge: f64| CARD_GAP.min((s.level - edge).abs());
                put(a, gap(a), true);
                put(b, gap(b), true);
            }
        }
        for o in all {
            let mine = bundle.iter().any(|b| b.id == o.id && b.k == o.k);
            if !mine
                && o.horiz == s.horiz
                && !self.same_group(&o.id, &s.id)
                && overlap((o.lo, o.hi), span) > OVERLAP
            {
                put(o.level, LINE_GAP, false);
            }
        }
        (below, above)
    }
}

/// Where a segment should stand, or `None` when it is fine where it is or cannot do better.
fn target(level: f64, below: &[Bound], above: &[Bound]) -> Option<f64> {
    let max = |v: &[Bound], f: &dyn Fn(&Bound) -> f64| v.iter().map(f).fold(f64::MIN, f64::max);
    let min = |v: &[Bound], f: &dyn Fn(&Bound) -> f64| v.iter().map(f).fold(f64::MAX, f64::min);
    let (near_lo, near_hi) = (max(below, &|b| b.pos), min(above, &|b| b.pos));
    let (soft_lo, soft_hi) = (
        max(below, &|b| b.pos + b.gap),
        min(above, &|b| b.pos - b.gap),
    );
    let hard_lo = max(below, &|b| if b.hard { b.pos + b.gap } else { b.pos + 1.0 });
    let hard_hi = min(above, &|b| if b.hard { b.pos - b.gap } else { b.pos - 1.0 });
    let wish = if soft_lo <= soft_hi {
        level.clamp(soft_lo, soft_hi)
    } else if near_lo > f64::MIN && near_hi < f64::MAX {
        (near_lo + near_hi) / 2.0
    } else {
        return None;
    };
    if hard_lo > hard_hi {
        return None;
    }
    let t = (wish.clamp(hard_lo, hard_hi) * 2.0).round() / 2.0;
    // only a move that widens the narrowest gap
    let worst = |at: f64| {
        below
            .iter()
            .map(|b| (at - b.pos) / b.gap.max(1.0))
            .chain(above.iter().map(|b| (b.pos - at) / b.gap.max(1.0)))
            .fold(f64::MAX, f64::min)
    };
    ((t - level).abs() >= 1.0 && worst(t) > worst(level) + 0.01).then_some(t)
}

/// The segment strictly crosses the inside of a box.
fn cuts(a: (f64, f64), b: (f64, f64), r: &Box2) -> bool {
    let (x0, x1) = (a.0.min(b.0), a.0.max(b.0));
    let (y0, y1) = (a.1.min(b.1), a.1.max(b.1));
    x0 < r.right() - 0.5 && x1 > r.x + 0.5 && y0 < r.bottom() - 0.5 && y1 > r.y + 0.5
}

fn crossings(r: &[(f64, f64)], others: &BTreeMap<String, Points>, id: &str) -> usize {
    let cross = |a: ((f64, f64), (f64, f64)), b: ((f64, f64), (f64, f64))| {
        let ha = (a.0.1 - a.1.1).abs() < 0.01;
        let hb = (b.0.1 - b.1.1).abs() < 0.01;
        if ha == hb {
            return false;
        }
        let (h, v) = if ha { (a, b) } else { (b, a) };
        let (hx0, hx1) = (h.0.0.min(h.1.0), h.0.0.max(h.1.0));
        let (vy0, vy1) = (v.0.1.min(v.1.1), v.0.1.max(v.1.1));
        v.0.0 > hx0 + 0.5 && v.0.0 < hx1 - 0.5 && h.0.1 > vy0 + 0.5 && h.0.1 < vy1 - 0.5
    };
    others
        .iter()
        .filter(|(o, _)| o.as_str() != id)
        .map(|(_, o)| {
            r.windows(2)
                .map(|a| {
                    o.windows(2)
                        .filter(|b| cross((a[0], a[1]), (b[0], b[1])))
                        .count()
                })
                .sum::<usize>()
        })
        .sum()
}

/// The routes once the segments of `bundle` stand at level `t`.
fn moved(routes: &BTreeMap<String, Points>, bundle: &[Seg], t: f64) -> BTreeMap<String, Points> {
    let mut out = routes.clone();
    for s in bundle {
        if let Some(r) = out.get_mut(&s.id) {
            for p in [s.k, s.k + 1] {
                if s.horiz {
                    r[p].1 = t;
                } else {
                    r[p].0 = t;
                }
            }
        }
    }
    out
}

/// The move keeps every changed piece off cards and titles and adds no crossing.
fn acceptable(
    scene: &Scene,
    before: &BTreeMap<String, Points>,
    after: &BTreeMap<String, Points>,
    bundle: &[Seg],
) -> bool {
    bundle.iter().all(|s| {
        let r = &after[&s.id];
        let pieces = [s.k.wrapping_sub(1), s.k, s.k + 1];
        let clear = pieces
            .iter()
            .filter(|&&i| i + 1 < r.len())
            .all(|&i| scene.solids.iter().all(|b| !cuts(r[i], r[i + 1], b)));
        clear && crossings(r, after, &s.id) <= crossings(&before[&s.id], before, &s.id)
    })
}

/// Visible zone frames, and the cards and titles no line may cross.
fn scene<'a>(
    model: &Model,
    boxes: &BTreeMap<String, Box2>,
    titles: &[(String, Box2)],
    groups: &'a BTreeMap<String, String>,
) -> Scene<'a> {
    let mut frames = Vec::new();
    let mut solids: Vec<Box2> = titles.iter().map(|(_, b)| *b).collect();
    for n in &model.nodes {
        let Some(b) = boxes.get(&n.id) else { continue };
        match n.kind {
            Kind::Card => solids.push(*b),
            Kind::Zone
                if !crate::layout::invisible(n) && !n.classes.iter().any(|c| c == "bare") =>
            {
                frames.push(*b);
            }
            Kind::Zone => {}
        }
    }
    Scene {
        frames,
        solids,
        groups,
    }
}

pub fn breathe(
    model: &Model,
    boxes: &BTreeMap<String, Box2>,
    titles: &[(String, Box2)],
    groups: &BTreeMap<String, String>,
    routes: &mut BTreeMap<String, Points>,
) {
    let scene = scene(model, boxes, titles, groups);
    // arrows with waypoints stay as written
    let pinned: Vec<&str> = model
        .edges
        .iter()
        .filter(|e| e.via.is_some())
        .map(|e| e.id.as_str())
        .collect();
    for _ in 0..3 {
        let mut changed = false;
        let all = segments(routes);
        for s in &all {
            let len = routes[&s.id].len();
            if s.k == 0 || s.k + 2 >= len {
                continue;
            }
            // the segments drawn on this one (a shared trunk) move with it
            let bundle: Vec<Seg> = segments(routes)
                .into_iter()
                .filter(|o| {
                    o.horiz == s.horiz
                        && (o.level - s.level).abs() < 0.5
                        && overlap((o.lo, o.hi), (s.lo, s.hi)) > -0.5
                })
                .collect();
            let movable = bundle.iter().all(|o| {
                o.k > 0 && o.k + 2 < routes[&o.id].len() && !pinned.contains(&o.id.as_str())
            });
            let Some(me) = bundle.iter().find(|o| o.id == s.id && o.k == s.k) else {
                continue;
            };
            if !movable {
                continue;
            }
            let (below, above) = scene.bounds(me, &bundle, &segments(routes));
            let Some(t) = target(me.level, &below, &above) else {
                continue;
            };
            let after = moved(routes, &bundle, t);
            if acceptable(&scene, routes, &after, &bundle) {
                *routes = after;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn b(pos: f64, gap: f64, hard: bool) -> Bound {
        Bound { pos, gap, hard }
    }

    #[test]
    fn a_line_leaves_a_frame_when_there_is_room() {
        // a frame 3 px below, a card far above: the line keeps 20 px from the frame
        assert_eq!(
            target(97.0, &[b(94.0, 20.0, false)], &[b(200.0, 14.0, true)]),
            Some(114.0)
        );
    }

    #[test]
    fn a_narrow_corridor_is_shared_in_the_middle() {
        assert_eq!(
            target(103.0, &[b(100.0, 20.0, false)], &[b(130.0, 20.0, false)]),
            Some(115.0)
        );
    }

    #[test]
    fn nothing_moves_when_a_card_holds_the_line() {
        // the card above is 14 px away: the line cannot leave the frame below any further
        assert_eq!(
            target(110.0, &[b(105.0, 20.0, false)], &[b(124.0, 14.0, true)]),
            None
        );
        // already clear of everything
        assert_eq!(
            target(150.0, &[b(100.0, 20.0, false)], &[b(200.0, 20.0, false)]),
            None
        );
    }
}
