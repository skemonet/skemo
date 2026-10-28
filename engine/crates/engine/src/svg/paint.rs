//! Rendu SVG : couleurs, fonds, rayons et formes de base.

use super::*;
use skemo_lang::style::{Computed, Rgba, parse_color, parse_length};

pub(super) fn color(c: Rgba) -> (String, Option<String>) {
    let opaque = Rgba { a: 1.0, ..c };
    let hex = opaque.to_css();
    if c.a >= 0.999 {
        (hex, None)
    } else {
        (hex, Some(n(c.a)))
    }
}

/// Attributs `fill="…" [fill-opacity]` (ou `stroke`).
pub(super) fn paint(attr: &str, c: Option<Rgba>) -> String {
    match c {
        None => format!(" {attr}=\"none\""),
        Some(c) if c.a <= 0.001 => format!(" {attr}=\"none\""),
        Some(c) => {
            let (hex, op) = color(c);
            match op {
                Some(o) => format!(" {attr}=\"{hex}\" {attr}-opacity=\"{o}\""),
                None => format!(" {attr}=\"{hex}\""),
            }
        }
    }
}

pub(super) fn background(c: &Computed) -> Option<Rgba> {
    c.color_of("background-color").or_else(|| {
        let b = c.get("background")?;
        parse_color(b, c.color())
    })
}

/// Border radius (first value), bounded by half of the smallest side.
pub(super) fn radius(c: &Computed, w: f64, h: f64) -> f64 {
    let r = c
        .get("border-radius")
        .and_then(|v| {
            v.split_whitespace()
                .next()
                .and_then(|t| parse_length(t, c.font_size(), Some(w)))
        })
        .unwrap_or(0.0);
    r.min(w / 2.0).min(h / 2.0).max(0.0)
}

/// Rectangle arrondi (rayons haut et bas distincts) en chemin.
pub(super) fn rrect(x: f64, y: f64, w: f64, h: f64, rt: f64, rb: f64) -> String {
    let rt = rt.min(w / 2.0).min(h / 2.0).max(0.0);
    let rb = rb.min(w / 2.0).min(h / 2.0).max(0.0);
    format!(
        "M{},{} H{} A{},{} 0 0 1 {},{} V{} A{},{} 0 0 1 {},{} H{} A{},{} 0 0 1 {},{} V{} A{},{} 0 0 1 {},{} Z",
        n(x + rt),
        n(y),
        n(x + w - rt),
        n(rt),
        n(rt),
        n(x + w),
        n(y + rt),
        n(y + h - rb),
        n(rb),
        n(rb),
        n(x + w - rb),
        n(y + h),
        n(x + rb),
        n(rb),
        n(rb),
        n(x),
        n(y + h - rb),
        n(y + rt),
        n(rt),
        n(rt),
        n(x + rt),
        n(y)
    )
}

/// Top band of a rounded rectangle (radius `r`), of thickness `t`.
pub(super) fn top_band(x: f64, y: f64, w: f64, r: f64, t: f64) -> String {
    let r = r.min(w / 2.0);
    if r <= 0.0 || t >= r {
        return format!(
            "M{},{} H{} V{} H{} Z",
            n(x),
            n(y + r.min(t)),
            n(x + w),
            n(y + t),
            n(x)
        );
    }
    // intersection of the arc with the line y + t
    let dx = r - (r * r - (r - t) * (r - t)).sqrt();
    format!(
        "M{},{} A{},{} 0 0 1 {},{} H{} A{},{} 0 0 1 {},{} Z",
        n(x + dx),
        n(y + t),
        n(r),
        n(r),
        n(x + r),
        n(y),
        n(x + w - r),
        n(r),
        n(r),
        n(x + w - dx),
        n(y + t)
    )
}

/// Path with rounded corners (prototype's `roundedPath`, radius 8).
pub fn rounded_path(pts: &[(f64, f64)], r: f64) -> String {
    rounded_path_sharp(pts, r, &[])
}

/// The same, with sharp corners at the points of `sharp` (junctions of grouped arrows, D-047).
pub fn rounded_path_sharp(pts: &[(f64, f64)], r: f64, sharp: &[(f64, f64)]) -> String {
    rounded_path_hops(pts, r, sharp, &[])
}

/// Bridges (`arrow::HOP`) on segment `p`–`q`, at the points of `hops` lying on it, in the order
/// the line runs: a half circle over the crossing.
fn bridges(d: &mut String, p: (f64, f64), q: (f64, f64), hops: &[(f64, f64)]) {
    let dir = if q.0 >= p.0 { 1.0 } else { -1.0 };
    let mut on: Vec<f64> = hops
        .iter()
        .filter(|h| {
            (h.1 - p.1).abs() < 0.5
                && (q.1 - p.1).abs() < 0.5
                && h.0 > p.0.min(q.0)
                && h.0 < p.0.max(q.0)
        })
        .map(|h| h.0)
        .collect();
    on.sort_by(|a, b| (a * dir).total_cmp(&(b * dir)));
    let r = crate::arrow::HOP;
    // the arc goes over the crossing: clockwise from left to right, the other way back
    let sweep = u8::from(dir > 0.0);
    for x in on {
        let _ = write!(
            d,
            " L{},{} A{},{} 0 0 {sweep} {},{}",
            n(x - dir * r),
            n(p.1),
            n(r),
            n(r),
            n(x + dir * r),
            n(p.1)
        );
    }
}

/// The same, with a bridge at each point of `hops` (crossings of its horizontal segments).
pub fn rounded_path_hops(
    pts: &[(f64, f64)],
    r: f64,
    sharp: &[(f64, f64)],
    hops: &[(f64, f64)],
) -> String {
    if pts.len() < 3 {
        let mut d = String::new();
        for (i, p) in pts.iter().enumerate() {
            if i > 0 {
                bridges(&mut d, pts[i - 1], *p, hops);
            }
            let _ = write!(
                d,
                "{}{},{}",
                if i == 0 { "M" } else { " L" },
                n(p.0),
                n(p.1)
            );
        }
        return d;
    }
    let mut d = format!("M{},{}", n(pts[0].0), n(pts[0].1));
    for i in 1..pts.len() - 1 {
        let (px, py) = pts[i - 1];
        let (x, y) = pts[i];
        let (nx, ny) = pts[i + 1];
        let l1 = (x - px).hypot(y - py);
        let l2 = (nx - x).hypot(ny - y);
        if l1 == 0.0 || l2 == 0.0 {
            continue;
        }
        let at_junction = sharp
            .iter()
            .any(|q| (q.0 - x).abs() < 0.5 && (q.1 - y).abs() < 0.5);
        let k = if at_junction {
            0.0
        } else {
            r.min(l1 / 2.0).min(l2 / 2.0)
        };
        bridges(&mut d, (px, py), (x, y), hops);
        let _ = write!(
            d,
            " L{},{} Q{},{} {},{}",
            n(x - (x - px) / l1 * k),
            n(y - (y - py) / l1 * k),
            n(x),
            n(y),
            n(x + (nx - x) / l2 * k),
            n(y + (ny - y) / l2 * k)
        );
    }
    let (lx, ly) = pts[pts.len() - 1];
    bridges(&mut d, pts[pts.len() - 2], (lx, ly), hops);
    let _ = write!(d, " L{},{}", n(lx), n(ly));
    d
}
