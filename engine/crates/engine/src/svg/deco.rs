//! SVG rendering: decorations of the timeline and frieze modes (axes, grids, markers).

use super::*;
use crate::anim::Presence;
use crate::geometry::{self, Run};
use crate::measure::border;
use skemo_lang::style::Computed;

/// Style of a mode decoration (prototype from the scene).
pub(super) fn deco_style(ctx: &Ctx, name: &str) -> Option<Computed> {
    ctx.d
        .scene
        .deco
        .get(name)
        .map(|&i| ctx.d.scene.style(i).clone())
        .filter(|c| c.display() != "none")
}

/// Vertical lines of the timeline (grid, "today") between `y0` and `y1` (SVG coordinates).
pub(super) fn tl_lines(ctx: &mut Ctx, y0: f64, y1: f64) {
    let Some(tl) = ctx.d.timeline.clone() else {
        return;
    };
    let (ox, oy) = (ctx.d.frame.ox, ctx.d.frame.oy);
    let top = (oy + tl.axis_h - 8.0).max(y0);
    let bot = (oy + tl.bottom).min(y1);
    if bot <= top {
        return;
    }
    for t in &tl.ticks {
        let Some(c) = deco_style(ctx, if t.major { "tl-grid major" } else { "tl-grid" }) else {
            continue;
        };
        let w = border(&c)[3].max(1.0);
        let xx = t.x + ox + w / 2.0;
        let _ = write!(
            ctx.out,
            "<path class=\"tl-grid\" d=\"M{},{} V{}\"{} stroke-width=\"{}\"/>",
            n(xx),
            n(top),
            n(bot),
            paint("stroke", Some(c.border_color(3))),
            n(w)
        );
    }
    if let (Some((tx, _)), Some(c)) = (tl.today, deco_style(ctx, "tl-today")) {
        let w = border(&c)[3].max(1.0);
        let dash = if c.get("border-left-style") == Some("dashed") {
            format!(" stroke-dasharray=\"{} {}\"", n(w * 3.0), n(w * 3.0))
        } else {
            String::new()
        };
        let _ = write!(
            ctx.out,
            "<path class=\"tl-today\" d=\"M{},{} V{}\"{} stroke-width=\"{}\"{dash}/>",
            n(tx + ox + w / 2.0),
            n(top),
            n(bot),
            paint("stroke", Some(c.border_color(3))),
            n(w)
        );
    }
}

/// Axis and graduations of the timeline (under the nodes).
pub(super) fn tl_under(ctx: &mut Ctx) {
    let Some(tl) = ctx.d.timeline.clone() else {
        return;
    };
    let (ox, oy) = (ctx.d.frame.ox, ctx.d.frame.oy);
    if let Some(c) = deco_style(ctx, "tl-axis") {
        let (ax, aw) = (ox - 22.0, tl.width + 104.0);
        let _ = write!(
            ctx.out,
            "<rect class=\"tl-axis\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"{}/>",
            n(ax),
            n(oy),
            n(aw),
            n(tl.axis_h),
            paint("fill", background(&c))
        );
        let b = border(&c)[2];
        if b > 0.0 {
            let _ = write!(
                ctx.out,
                "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"{}/>",
                n(ax),
                n(oy + tl.axis_h),
                n(aw),
                n(b),
                paint("fill", Some(c.border_color(2)))
            );
        }
    }
    tl_lines(ctx, f64::NEG_INFINITY, f64::INFINITY);
    for t in &tl.ticks {
        let name = if t.major { "tl-tick major" } else { "tl-tick" };
        let Some(&el) = ctx.d.scene.deco.get(name) else {
            continue;
        };
        let c = ctx.d.scene.style(el).clone();
        let lh = crate::measure::line_height(&c);
        let run = Run {
            text: t.label.clone(),
            x: t.x + ox + 4.0,
            baseline: geometry::baseline(&c, oy + 8.0, lh),
            width: 0.0,
            el,
        };
        text(ctx, &run, None);
    }
}

/// Text centred horizontally at `cx`, line box starting at `top`.
pub(super) fn centered_text(ctx: &mut Ctx, el: usize, label: &str, cx: f64, top: f64, attrs: &str) {
    let c = ctx.d.scene.style(el).clone();
    let tw = crate::measure::tw(&c, label);
    let lh = crate::measure::line_height(&c);
    let op = c
        .get("opacity")
        .and_then(|o| o.trim().parse::<f64>().ok())
        .filter(|o| *o < 1.0);
    if let Some(o) = op {
        let _ = write!(ctx.out, "<g opacity=\"{}\"{attrs}>", n(o));
    } else if !attrs.is_empty() {
        let _ = write!(ctx.out, "<g{attrs}>");
    }
    let run = Run {
        text: crate::measure::transform(&c, label),
        x: cx - tw / 2.0,
        baseline: geometry::baseline(&c, top, lh),
        width: tw,
        el,
    };
    text(ctx, &run, None);
    if op.is_some() || !attrs.is_empty() {
        ctx.out.push_str("</g>");
    }
}

/// Frieze, under the nodes: stems of the events, axis and graduations.
pub(super) fn fr_under(ctx: &mut Ctx) {
    let Some(fr) = ctx.d.frise.clone() else {
        return;
    };
    let (ox, oy) = (ctx.d.frame.ox, ctx.d.frame.oy);
    if let Some(c) = deco_style(ctx, "fr-stem") {
        let w = c.border()[3].max(0.5);
        for st in &fr.stems {
            let p = presence(ctx, &st.id);
            if p.opacity <= 0.001 {
                continue;
            }
            let (y1, y2) = (st.y1.min(st.y2) + oy, st.y1.max(st.y2) + oy);
            let _ = write!(
                ctx.out,
                "<path class=\"fr-stem\" data-for=\"{}\"{}{} d=\"M{},{} V{}\"{} stroke-width=\"{}\"/>",
                esc(&st.id),
                anim_attrs(ctx, &st.id),
                presence_attrs(
                    Presence {
                        dy: 0.0,
                        scale: 1.0,
                        ..p
                    },
                    0.0,
                    0.0
                ),
                n(st.x + ox),
                n(y1),
                n(y2),
                paint("stroke", Some(c.border_color(3))),
                n(w)
            );
        }
    }
    if let Some(c) = deco_style(ctx, "fr-axis") {
        let h = c.length("height").unwrap_or(4.0);
        let (x, w) = (ox - 10.0, fr.width + 20.0);
        let _ = write!(
            ctx.out,
            "<rect class=\"fr-axis\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{}\"{}/>",
            n(x),
            n(oy - 2.0),
            n(w),
            n(h),
            n(radius(&c, w, h)),
            paint("fill", background(&c))
        );
    }
    if let Some(c) = deco_style(ctx, "fr-tick") {
        let (tw, th) = (
            c.length("width").unwrap_or(1.0),
            c.length("height").unwrap_or(8.0),
        );
        for (x, _) in &fr.ticks {
            let _ = write!(
                ctx.out,
                "<rect class=\"fr-tick\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"{}/>",
                n(x + ox),
                n(oy),
                n(tw),
                n(th),
                paint("fill", background(&c))
            );
        }
    }
}

/// Frieze, above everything: labels of the years, centuries, dots of the events.
pub(super) fn fr_over(ctx: &mut Ctx) {
    let Some(fr) = ctx.d.frise.clone() else {
        return;
    };
    let (ox, oy) = (ctx.d.frame.ox, ctx.d.frame.oy);
    if let Some(&el) = ctx.d.scene.deco.get("fr-tick-label") {
        for (x, label) in &fr.ticks {
            centered_text(
                ctx,
                el,
                label,
                x + ox,
                oy + crate::frise::TICK_LABEL_TOP,
                "",
            );
        }
    }
    if let Some(&el) = ctx.d.scene.deco.get("fr-century") {
        for (x, label) in &fr.centuries {
            centered_text(ctx, el, label, x + ox, oy - 22.0, "");
        }
    }
    if let Some(c) = deco_style(ctx, "fr-dot") {
        let size = c.length("width").unwrap_or(10.0);
        let b = c.border()[0];
        let mt = c.length("margin-top").unwrap_or(-3.0);
        for st in &fr.stems {
            let p = presence(ctx, &st.id);
            if p.opacity <= 0.001 {
                continue;
            }
            let (cx, cy) = (st.x + ox, oy + mt + size / 2.0);
            let _ = write!(
                ctx.out,
                "<circle class=\"fr-dot\" data-for=\"{}\"{}{} cx=\"{}\" cy=\"{}\" r=\"{}\"{}{} stroke-width=\"{}\"/>",
                esc(&st.id),
                anim_attrs(ctx, &st.id),
                presence_attrs(
                    Presence {
                        dy: 0.0,
                        scale: 1.0,
                        ..p
                    },
                    0.0,
                    0.0
                ),
                n(cx),
                n(cy),
                n((size - b) / 2.0),
                paint("fill", background(&c)),
                paint("stroke", Some(c.border_color(0))),
                n(b)
            );
        }
    }
}

/// Diamonds of the axis and the "today" chip (above everything).
pub(super) fn tl_over(ctx: &mut Ctx) {
    let Some(tl) = ctx.d.timeline.clone() else {
        return;
    };
    let (ox, oy) = (ctx.d.frame.ox, ctx.d.frame.oy);
    if let Some(c) = deco_style(ctx, "tl-mark") {
        let size = c.length("width").unwrap_or(9.0);
        for (id, x) in &tl.marks {
            let p = presence(ctx, id);
            if p.opacity <= 0.001 {
                continue;
            }
            let (mx, my) = (x + ox - size / 2.0, oy + tl.axis_h - 6.0);
            let (cx, cy) = (mx + size / 2.0, my + size / 2.0);
            let _ = write!(
                ctx.out,
                "<rect class=\"tl-mark\" data-for=\"{}\"{}{} x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{}\" transform=\"rotate(45 {} {})\"{}/>",
                esc(id),
                anim_attrs(ctx, id),
                presence_attrs(
                    Presence {
                        dy: 0.0,
                        scale: 1.0,
                        ..p
                    },
                    0.0,
                    0.0
                ),
                n(mx),
                n(my),
                n(size),
                n(size),
                n(radius(&c, size, size)),
                n(cx),
                n(cy),
                paint("fill", background(&c))
            );
        }
    }
    if let (Some((tx, label)), Some(&el)) =
        (tl.today.clone(), ctx.d.scene.deco.get("tl-today-label"))
    {
        let c = ctx.d.scene.style(el).clone();
        let p = c.padding();
        let tw = crate::measure::tw(&c, &label);
        let lh = crate::measure::line_height(&c);
        let (w, h) = (tw + p[1] + p[3], lh + p[0] + p[2]);
        let (x0, y0) = (tx + ox - w / 2.0, oy + tl.axis_h - 4.0 - h);
        let _ = write!(
            ctx.out,
            "<rect class=\"tl-today-label\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{}\"{}/>",
            n(x0),
            n(y0),
            n(w),
            n(h),
            n(radius(&c, w, h)),
            paint("fill", background(&c))
        );
        let run = Run {
            text: label,
            x: x0 + p[3],
            baseline: geometry::baseline(&c, y0 + p[0], lh),
            width: tw,
            el,
        };
        text(ctx, &run, None);
    }
}
