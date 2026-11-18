//! Places for an arrow's label on its own line: on it as a tag (L2) or beside it, on a straight
//! stretch, not over a bend nor the head, a step from every card, frame and zone text
//! (`[label-box]`); beside a level line at the gap that puts its middle on a track (L9, A6).

use super::grid::{Route, key};
use super::lattice::Lattice;
use super::{BoxPx, Pt, STEP};
use std::collections::HashMap;

/// `[label-gap]` beside an upright line, px.
const LABEL_GAP: f64 = 4.0;
/// Places kept per route.
const SLOTS_PER_ROUTE: usize = 8;

#[derive(Clone, Debug)]
pub(crate) struct Slot {
    pub bx: BoxPx,
    /// Steps along the line from its start: a label near the start first (L8).
    pub along: i32,
    /// The stretch of its own line under (or beside) the label and a step either side, sorted
    /// node keys: the slot is valid when the line drawn runs there.
    pub seg: Vec<u32>,
    /// Nodes closer than `[label-air]` (two steps): no other line there.
    pub near: Vec<u32>,
    pub on: bool,
}

/// How far a label of height `h` stands beside a level line (L9: its middle on a track).
fn side_gap(h: f64) -> f64 {
    let g = (-h / 2.0).rem_euclid(STEP);
    if g >= 2.0 { g } else { g + STEP }
}

/// Every card, frame side and zone text as a box (x0, y0, x1, y1), px.
pub(crate) fn obstacles(l: &Lattice) -> Vec<(f64, f64, f64, f64)> {
    let mut out: Vec<(f64, f64, f64, f64)> = l
        .card
        .iter()
        .map(|r| {
            let b = r.px();
            (b.x, b.y, b.x + b.w, b.y + b.h)
        })
        .collect();
    for f in l.frame.iter().flatten() {
        let b = f.px();
        let (x0, y0, x1, y1) = (b.x, b.y, b.x + b.w, b.y + b.h);
        out.extend([
            (x0, y0, x1, y0),
            (x0, y1, x1, y1),
            (x0, y0, x0, y1),
            (x1, y0, x1, y1),
        ]);
    }
    out.extend(
        l.texts
            .iter()
            .flatten()
            .map(|b| (b.x, b.y, b.x + b.w, b.y + b.h)),
    );
    out
}

fn box_free(l: &Lattice, obs: &[(f64, f64, f64, f64)], b: BoxPx) -> bool {
    let s = STEP;
    if b.x < s
        || b.y < s
        || b.x + b.w > f64::from(l.width) * s - s
        || b.y + b.h > f64::from(l.height) * s - s
    {
        return false;
    }
    !obs.iter().any(|o| {
        let dx = (o.0 - (b.x + b.w)).max(b.x - o.2).max(0.0);
        let dy = (o.1 - (b.y + b.h)).max(b.y - o.3).max(0.0);
        dx * dx + dy * dy < s * s
    })
}

/// Lattice nodes closer than `d` px to a box.
pub(crate) fn nodes_near(b: BoxPx, d: f64) -> Vec<u32> {
    let s = STEP;
    let mut out = Vec::new();
    let gx0 = ((b.x - d) / s).floor() as i32;
    let gx1 = ((b.x + b.w + d) / s).ceil() as i32;
    let gy0 = ((b.y - d) / s).floor() as i32;
    let gy1 = ((b.y + b.h + d) / s).ceil() as i32;
    for gx in gx0..=gx1 {
        let px = f64::from(gx) * s;
        let dx = (b.x - px).max(px - (b.x + b.w)).max(0.0);
        if dx >= d {
            continue;
        }
        for gy in gy0..=gy1 {
            let py = f64::from(gy) * s;
            let dy = (b.y - py).max(py - (b.y + b.h)).max(0.0);
            if dx * dx + dy * dy < d * d && gx >= 0 && gy >= 0 {
                out.push(key((gx, gy)));
            }
        }
    }
    out.sort_unstable();
    out
}

/// The places of one stretch, cached per lattice: the routes of an arrow share most stretches.
pub(crate) struct SlotCache {
    obs: Vec<(f64, f64, f64, f64)>,
    stretch: HashMap<(usize, Pt, Pt, bool), Vec<Slot>>,
}

impl SlotCache {
    pub(crate) fn new(l: &Lattice) -> SlotCache {
        SlotCache {
            obs: obstacles(l),
            stretch: HashMap::new(),
        }
    }

    /// The places for label `size` (its id `lid`) on route `r`: near the start first (L8), and
    /// a few more spread along the line.
    pub(crate) fn slots(
        &mut self,
        l: &Lattice,
        r: &Route,
        lid: usize,
        size: (f64, f64),
    ) -> Vec<Slot> {
        let mut out = Vec::new();
        let mut dist0 = 0;
        let pts = &r.pts;
        for k in 0..pts.len() - 1 {
            let (a, b) = (pts[k], pts[k + 1]);
            let seglen = (b.0 - a.0).abs() + (b.1 - a.1).abs();
            let last = k == pts.len() - 2;
            let ck = (lid, a, b, last);
            if !self.stretch.contains_key(&ck) {
                let found = self.stretch_slots(l, a, b, last, size);
                self.stretch.insert(ck, found);
            }
            for s in &self.stretch[&ck] {
                let mut s = s.clone();
                s.along += dist0;
                out.push(s);
            }
            dist0 += seglen;
        }
        out.sort_by_key(|s| (s.along, s.on));
        let lim = SLOTS_PER_ROUTE;
        let half = lim / 2;
        let mut pick: Vec<Slot> = out.iter().take(half).cloned().collect();
        let step = (out.len() / half.max(1)).max(1);
        pick.extend(
            out.iter()
                .skip(half)
                .step_by(step)
                .take(lim - half)
                .cloned(),
        );
        let mut seen = std::collections::HashSet::new();
        pick.retain(|s| seen.insert((s.bx.x.round() as i64, s.bx.y.round() as i64)));
        pick
    }

    fn stretch_slots(
        &self,
        l: &Lattice,
        a: Pt,
        b: Pt,
        last: bool,
        (w, h): (f64, f64),
    ) -> Vec<Slot> {
        let s = STEP;
        let seglen = (b.0 - a.0).abs() + (b.1 - a.1).abs();
        let horiz = a.1 == b.1;
        let along = if horiz { w } else { h };
        let sgn = if (b.0 - a.0 + b.1 - a.1) > 0 { 1 } else { -1 };
        let at = |u: i32| {
            if horiz {
                (a.0 + sgn * u, a.1)
            } else {
                (a.0, a.1 + sgn * u)
            }
        };
        let g = side_gap(h);
        let mut found = Vec::new();
        for t in 0..=seglen {
            let c = at(t);
            let (cx, cy) = (f64::from(c.0) * s, f64::from(c.1) * s);
            let lo = f64::from(t) * s - along / 2.0;
            let hi = f64::from(t) * s + along / 2.0;
            let end = if last { 10.0 + 8.0 } else { 2.0 };
            if lo < 2.0 || hi > f64::from(seglen) * s - end {
                continue;
            }
            let on = BoxPx {
                x: cx - w / 2.0,
                y: cy - h / 2.0,
                w,
                h,
            };
            let (above, below) = if horiz {
                (
                    BoxPx {
                        x: cx - w / 2.0,
                        y: cy - g - h,
                        w,
                        h,
                    },
                    BoxPx {
                        x: cx - w / 2.0,
                        y: cy + g,
                        w,
                        h,
                    },
                )
            } else {
                (
                    BoxPx {
                        x: cx + LABEL_GAP,
                        y: cy - h / 2.0,
                        w,
                        h,
                    },
                    BoxPx {
                        x: cx - LABEL_GAP - w,
                        y: cy - h / 2.0,
                        w,
                        h,
                    },
                )
            };
            for (is_on, bx) in [(true, on), (false, above), (false, below)] {
                if !box_free(l, &self.obs, bx) {
                    continue;
                }
                let u0 = (lo / s).floor() as i32 - 2;
                let u1 = (hi / s).ceil() as i32 + 2;
                let mut seg: Vec<u32> = (u0..=u1)
                    .filter(|u| (0..=seglen).contains(u))
                    .map(|u| key(at(u)))
                    .collect();
                seg.sort_unstable();
                found.push(Slot {
                    bx,
                    along: t,
                    seg,
                    near: nodes_near(bx, 2.0 * s),
                    on: is_on,
                });
            }
        }
        found
    }
}

/// Two label boxes closer than a step.
pub(crate) fn too_close(a: BoxPx, b: BoxPx) -> bool {
    let dx = (b.x - (a.x + a.w)).max(a.x - (b.x + b.w)).max(0.0);
    let dy = (b.y - (a.y + a.h)).max(a.y - (b.y + b.h)).max(0.0);
    dx * dx + dy * dy < STEP * STEP
}

/// `[label-width]`: a label's pill is 140 px wide at most, on three lines at most (L6).
pub(crate) const LABEL_MAX_PX: f64 = 140.0;
pub(crate) const LABEL_MAX_LINES: usize = 3;

/// Greedy wrapping at `width` px; a word wider than the line is cut where a hand would cut it:
/// after a hyphen, a dot, a slash, an underscore, a colon or a comma, or before a capital that
/// follows a small letter (`nmStateConfig` / `LabelSelector`).
pub(crate) fn wrap(s: &str, width: f64, measure: &dyn Fn(&str) -> f64) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut cur = String::new();
    for w in s.split_whitespace() {
        let t = if cur.is_empty() {
            w.to_string()
        } else {
            format!("{cur} {w}")
        };
        if measure(&t) <= width {
            cur = t;
            continue;
        }
        if !cur.is_empty() {
            lines.push(std::mem::take(&mut cur));
        }
        if measure(w) <= width {
            cur = w.to_string();
            continue;
        }
        for part in soft_parts(w) {
            let t = format!("{cur}{part}");
            if cur.is_empty() || measure(&t) <= width {
                cur = t;
            } else {
                lines.push(std::mem::replace(&mut cur, part));
            }
        }
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

fn soft_parts(w: &str) -> Vec<String> {
    let cs: Vec<char> = w.chars().collect();
    let mut out = Vec::new();
    let mut cur = String::new();
    for (i, &c) in cs.iter().enumerate() {
        let before_cap =
            i > 0 && c.is_uppercase() && (cs[i - 1].is_lowercase() || cs[i - 1].is_ascii_digit());
        if before_cap && !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
        }
        cur.push(c);
        if matches!(c, '-' | '.' | '/' | '_' | ':' | ',') {
            out.push(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}
