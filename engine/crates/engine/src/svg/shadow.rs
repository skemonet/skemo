//! SVG rendering: shadows (SVG filter and approximation for rasterisation).

use super::*;
use skemo_lang::style::css::split_top;
use skemo_lang::style::{Computed, Rgba, parse_color, parse_length};

/// Shadow filter for a `box-shadow` value; returns the `filter` attribute (empty if none).
/// Drop shadow approximated without a filter: for each `box-shadow` layer, a stack of rounded
/// rectangles inflated from −blur/2 to +blur/2, of equal opacity (linear ramp ≈ Gaussian edge).
pub(super) fn raster_shadow(ctx: &mut Ctx, c: &Computed, x: f64, y: f64, w: f64, h: f64, r: f64) {
    let Some(v) = c
        .get("box-shadow")
        .map(str::trim)
        .filter(|v| !v.is_empty() && *v != "none")
    else {
        return;
    };
    for sh in split_top(v, ',') {
        let mut lens = Vec::new();
        let mut col = None;
        for t in split_top(sh.trim(), ' ')
            .into_iter()
            .filter(|t| !t.is_empty())
        {
            match parse_length(t, c.font_size(), None) {
                Some(l) if lens.len() < 4 => lens.push(l),
                _ => col = parse_color(t, c.color()),
            }
        }
        let (dx, dy, blur) = (
            lens.first().copied().unwrap_or(0.0),
            lens.get(1).copied().unwrap_or(0.0),
            lens.get(2).copied().unwrap_or(0.0),
        );
        let spread = lens.get(3).copied().unwrap_or(0.0);
        let col = col.unwrap_or(Rgba {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 0.3,
        });
        let steps = if blur <= 0.5 {
            1
        } else {
            (blur.ceil() as usize).clamp(2, 8)
        };
        for k in 0..steps {
            let e = if steps == 1 {
                spread
            } else {
                spread - blur / 2.0 + blur * (k as f64 + 0.5) / steps as f64
            };
            let a = col.a / steps as f64;
            let (sx, sy, sw, sh_) = (x + dx - e, y + dy - e, w + 2.0 * e, h + 2.0 * e);
            if sw <= 0.0 || sh_ <= 0.0 {
                continue;
            }
            let rr = (r + e).max(0.0);
            let _ = write!(
                ctx.out,
                "<path d=\"{}\"{}/>",
                rrect(sx, sy, sw, sh_, rr, rr),
                paint("fill", Some(Rgba { a, ..col }))
            );
        }
    }
}

pub(super) fn shadow(ctx: &mut Ctx, c: &Computed) -> String {
    let Some(v) = c
        .get("box-shadow")
        .map(str::trim)
        .filter(|v| !v.is_empty() && *v != "none")
    else {
        return String::new();
    };
    let v = v.to_string();
    if let Some((id, _)) = ctx.filters.iter().find(|(_, s)| *s == v) {
        return format!(" filter=\"url(#{id})\"");
    }
    let mut prims = String::new();
    let mut merge = String::new();
    for (k, sh) in split_top(&v, ',').iter().enumerate() {
        let mut lens = Vec::new();
        let mut col = None;
        for t in split_top(sh.trim(), ' ')
            .into_iter()
            .filter(|t| !t.is_empty())
        {
            match parse_length(t, c.font_size(), None) {
                Some(l) if lens.len() < 4 => lens.push(l),
                _ => col = parse_color(t, c.color()),
            }
        }
        let (dx, dy, blur) = (
            lens.first().copied().unwrap_or(0.0),
            lens.get(1).copied().unwrap_or(0.0),
            lens.get(2).copied().unwrap_or(0.0),
        );
        let (hex, op) = color(col.unwrap_or(Rgba {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 0.3,
        }));
        let _ = write!(
            prims,
            "<feGaussianBlur in=\"SourceAlpha\" stdDeviation=\"{}\"/><feOffset dx=\"{}\" dy=\"{}\" result=\"o{k}\"/><feFlood flood-color=\"{hex}\" flood-opacity=\"{}\"/><feComposite in2=\"o{k}\" operator=\"in\" result=\"s{k}\"/>",
            n(blur / 2.0),
            n(dx),
            n(dy),
            op.unwrap_or_else(|| "1".into())
        );
        let _ = write!(merge, "<feMergeNode in=\"s{k}\"/>");
    }
    let id = format!("{}-ombre{}", ctx.prefix, ctx.filters.len());
    ctx.defs.push(format!(
        "<filter id=\"{id}\" x=\"-20%\" y=\"-20%\" width=\"140%\" height=\"160%\">{prims}<feMerge>{merge}<feMergeNode in=\"SourceGraphic\"/></feMerge></filter>"
    ));
    ctx.filters.push((id.clone(), v));
    format!(" filter=\"url(#{id})\"")
}
