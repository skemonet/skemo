//! SVG rendering: arrows (strokes, heads, labels).

use super::*;
use crate::anim::Presence;
use crate::arrow::{arrow, edge_classes, own_dots, triangle_path, unshared};
use crate::geometry::{self};
use skemo_lang::style::{Computed, parse_length};

pub(super) fn edge(ctx: &mut Ctx, e: &skemo_lang::Edge) {
    let d = ctx.d;
    let (Some(r), Some(el)) = (d.routes.get(&e.id), d.scene.edges.get(&e.id)) else {
        return;
    };
    if r.len() < 2 {
        return;
    }
    let pr = presence(ctx, &e.id);
    if pr.opacity <= 0.001 {
        return;
    }
    let pts: Vec<(f64, f64)> = r
        .iter()
        .map(|p| (p.0 + d.frame.ox, p.1 + d.frame.oy))
        .collect();
    let a = arrow(&pts);
    super::node::link_open(ctx, e.href.as_deref());
    let _ = write!(
        ctx.out,
        "<g class=\"{}\" data-id=\"{}\"{}{}>",
        esc(&edge_classes(d, e).join(" ")),
        esc(&e.id),
        anim_attrs(ctx, &e.id),
        presence_attrs(
            Presence {
                dy: 0.0,
                scale: 1.0,
                ..pr
            },
            0.0,
            0.0
        )
    );
    // a curved arrow (`--curve: on`) runs to the card itself, without junction dots
    let curved = crate::arrow::curved(d, el.line);
    let sharp = if curved {
        Vec::new()
    } else {
        crate::route::junctions_on(&d.routes, &d.edge_groups, &e.id, (d.frame.ox, d.frame.oy))
    };
    // a group's shared trunk and its dots are drawn once (D-049)
    let (line, own_head) = if curved {
        (pts.clone(), true)
    } else {
        unshared(d, e, &crate::arrow::drawn_line(d, &e.id, &a, &pts))
    };
    let dots = own_dots(d, e, &sharp);
    let hops = hops_of(d, &e.id, curved);
    edge_line(ctx, el.line, &line, &pr, &sharp, &dots, &hops);
    let hc = d.scene.style(el.head);
    // `--shape: dot` on the head: a dot where the arrow ends (the mind map's leaves)
    if !own_head {
        // the head is drawn by the first arrow of its join (D-050)
    } else if hc.display() != "none"
        && pr.draw > 0.95
        && hc.get("--shape").map(str::trim) == Some("dot")
    {
        if let Some(&(x, y)) = pts.last() {
            let _ = write!(
                ctx.out,
                "<circle class=\"head\" cx=\"{}\" cy=\"{}\" r=\"3.5\"{}/>",
                n(x),
                n(y),
                paint("fill", hc.color_of("fill").or(Some(hc.color())))
            );
        }
    } else if hc.display() != "none" && pr.draw > 0.95 {
        let _ = write!(
            ctx.out,
            "<path class=\"head\" d=\"{}\"{}/>",
            triangle_path(&a.head),
            paint("fill", hc.color_of("fill").or(Some(hc.color())))
        );
    }
    let tc = d.scene.style(el.tail);
    if tc.display() != "none" {
        let _ = write!(
            ctx.out,
            "<path class=\"tail\" d=\"{}\"{}/>",
            triangle_path(&a.tail),
            paint("fill", tc.color_of("fill").or(Some(tc.color())))
        );
    }
    let oc = d.scene.style(el.origin);
    if oc.display() != "none" {
        let _ = write!(
            ctx.out,
            "<circle class=\"origin\" cx=\"{}\" cy=\"{}\" r=\"3.5\"{}/>",
            n(a.origin.0),
            n(a.origin.1),
            paint("fill", oc.color_of("fill").or(Some(oc.color())))
        );
    }
    if let (Some(&lel), Some(&(lx, ly))) = (d.scene.labels.get(&e.id), d.labels.get(&e.id)) {
        let p = geometry::label(&d.scene, lel, &e.label, lx + d.frame.ox, ly + d.frame.oy);
        ctx.out.push_str("<g class=\"edge-label\">");
        pill(ctx, &p);
        ctx.out.push_str("</g>");
    }
    ctx.out.push_str("</g>");
    super::node::link_close(ctx, e.href.as_deref());
}

/// The bridges of an arrow's line where it crosses others (none on a curved line).
fn hops_of(d: &Diagram, id: &str, curved: bool) -> Vec<(f64, f64)> {
    if curved {
        Vec::new()
    } else {
        crate::arrow::hops(d, id, (d.frame.ox, d.frame.oy))
    }
}

/// The stroke: colour, width, dashes and caps from the style, or progressive drawing (`draw`
/// effect).
fn edge_line(
    ctx: &mut Ctx,
    line_el: usize,
    line: &[(f64, f64)],
    pr: &Presence,
    sharp: &[(f64, f64)],
    dots: &[(f64, f64)],
    hops: &[(f64, f64)],
) {
    let lc = ctx.d.scene.style(line_el);
    let stroke = lc.color_of("stroke").or_else(|| Some(lc.color()));
    // SVG takes a unitless stroke width (the theme's `1.4`), which is user units = px
    let sw = lc
        .length("stroke-width")
        .or_else(|| lc.get("stroke-width")?.trim().parse::<f64>().ok())
        .filter(|w| w.is_finite() && *w >= 0.0)
        .unwrap_or(1.0);
    let mut extra = style_stroke_attrs(lc);
    if pr.draw < 0.999 {
        // progressive drawing (draw effect): only a fraction of the stroke is visible
        // a bridge is a half circle where the line ran straight across its diameter
        let bridged = hops.len() as f64 * (std::f64::consts::PI - 2.0) * crate::arrow::HOP;
        let len = (crate::route::length(line) + bridged).max(1e-9);
        extra = format!(
            " stroke-dasharray=\"{} {}\" stroke-dashoffset=\"{}\"",
            n(len),
            n(len),
            (len * (1.0 - pr.draw) * 10000.0).round() / 10000.0
        );
    }
    // `--curve: on` (the mind map's theme, D-048 M1): one smooth curve from end to end, leaving
    // and arriving along the route's first and last segments
    let d = if crate::arrow::curved(ctx.d, line_el) && line.len() >= 2 {
        curve_path(line)
    } else {
        rounded_path_hops(line, 8.0, sharp, hops)
    };
    let _ = write!(
        ctx.out,
        "<path class=\"line\" d=\"{}\" fill=\"none\"{} stroke-width=\"{}\"{extra}/>",
        d,
        paint("stroke", stroke),
        n(sw)
    );
    // junction dots (D-047), drawn with the arrow so that they appear with it
    if pr.draw > 0.95 {
        for (x, y) in dots {
            let _ = write!(
                ctx.out,
                "<circle class=\"junction\" cx=\"{}\" cy=\"{}\" r=\"{}\"{}/>",
                n(*x),
                n(*y),
                n((sw * 2.2).max(2.5)),
                paint("fill", stroke)
            );
        }
    }
}

/// `stroke-dasharray` and `stroke-linecap` asked by the style, in user units (px): no
/// `pathLength`, which librsvg and other renderers ignore.
fn style_stroke_attrs(lc: &Computed) -> String {
    let mut extra = String::new();
    if let Some(da) = lc.get("stroke-dasharray").filter(|v| *v != "none") {
        let parts: Vec<String> = da
            .replace(',', " ")
            .split_whitespace()
            .filter_map(|t| {
                t.parse::<f64>()
                    .ok()
                    .or_else(|| parse_length(t, lc.font_size(), None))
            })
            .map(n)
            .collect();
        if !parts.is_empty() {
            let _ = write!(extra, " stroke-dasharray=\"{}\"", parts.join(" "));
        }
    }
    if let Some(lcap) = lc.get("stroke-linecap") {
        let _ = write!(extra, " stroke-linecap=\"{}\"", esc(lcap));
    }
    extra
}

/// A cubic curve from the first to the last point of a route, tangent to its first and last
/// segments, the control points half the distance away along the flow.
fn curve_path(line: &[(f64, f64)]) -> String {
    let (p0, p1) = (line[0], line[1]);
    let (q1, q0) = (line[line.len() - 2], line[line.len() - 1]);
    let unit = |a: (f64, f64), b: (f64, f64)| {
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let l = dx.hypot(dy).max(1e-9);
        (dx / l, dy / l)
    };
    let (u0, u1) = (unit(p0, p1), unit(q1, q0));
    let k = ((q0.0 - p0.0).abs().max((q0.1 - p0.1).abs()) / 2.0).max(8.0);
    format!(
        "M{},{} C{},{} {},{} {},{}",
        n(p0.0),
        n(p0.1),
        n(p0.0 + u0.0 * k),
        n(p0.1 + u0.1 * k),
        n(q0.0 - u1.0 * k),
        n(q0.1 - u1.1 * k),
        n(q0.0),
        n(q0.1)
    )
}
