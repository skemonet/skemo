//! SVG rendering: cards, zones, texts, chips and box decorations.

use super::*;
use crate::fonts::select;
use crate::geometry::{self, Pill, Run};
use crate::measure::{border, text_style};
use skemo_lang::Kind;
use skemo_lang::style::{Computed, Rgba, parse_length};

/// `font-family` with a generic fallback, for viewers that have neither Inter nor the embedded
/// fonts (`render_with_fonts`): the text then overflows a little, but keeps a sans-serif face.
fn family_list(family: &str) -> String {
    if family.contains("Mono") {
        format!("'{family}', ui-monospace, monospace")
    } else {
        format!("{family}, 'Helvetica Neue', Arial, sans-serif")
    }
}

pub(super) fn text(ctx: &mut Ctx, run: &Run, fill: Option<Rgba>) {
    if run.text.is_empty() {
        return;
    }
    let c = ctx.d.scene.style(run.el);
    let st = text_style(c);
    let f = select(&st.families, st.weight, st.italic);
    let mut a = format!(
        "<text x=\"{}\" y=\"{}\" font-family=\"{}\" font-size=\"{}\" font-weight=\"{}\"",
        n(run.x),
        n(run.baseline),
        family_list(f.file.family),
        n(st.size),
        f.file.weight
    );
    if f.file.italic {
        a.push_str(" font-style=\"italic\"");
    }
    if st.letter_spacing != 0.0 {
        let _ = write!(a, " letter-spacing=\"{}\"", n(st.letter_spacing));
    }
    a.push_str(&paint("fill", fill.or_else(|| Some(c.color()))));
    let _ = write!(
        ctx.out,
        "{a} xml:space=\"preserve\">{}</text>",
        esc(&run.text)
    );
}

/// Icon and link mark of a node, `at` = corner of the element in the image.
pub(super) fn deco(ctx: &mut Ctx, g: &geometry::DecoGeom, at: (f64, f64)) {
    for b in g.icon.iter().chain(g.mark.iter()) {
        icon(ctx, b, at);
    }
}

/// `<g class="icon">`: Lucide's 24 × 24 drawing scaled, stroke of the colour of `--icon-color`
/// (otherwise the text's).
pub(super) fn icon(ctx: &mut Ctx, b: &geometry::IconBox, (ox, oy): (f64, f64)) {
    let c = ctx.d.scene.style(b.el);
    let col = c.var_color("--icon-color").unwrap_or_else(|| c.color());
    let class = if b.is_mark { "link-mark" } else { "icon" };
    if let Some(img) = b.icon.image.and_then(|i| ctx.d.scene.images.get(i)) {
        // an image file (D-046): its own colours, scaled to fit the icon's box
        let _ = write!(
            ctx.out,
            "<image class=\"{class}\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" preserveAspectRatio=\"xMidYMid meet\" href=\"{}\"/>",
            n(b.x + ox),
            n(b.y + oy),
            n(b.size),
            n(b.size),
            img.data_url()
        );
        return;
    }
    let _ = write!(
        ctx.out,
        "<g class=\"{class}\" transform=\"translate({} {}) scale({})\" fill=\"none\"{} stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\" color=\"{}\">{}</g>",
        n(b.x + ox),
        n(b.y + oy),
        n(b.size / 24.0),
        paint("stroke", Some(col)),
        col.to_css(),
        b.icon.markup
    );
}

pub(super) fn pill(ctx: &mut Ctx, p: &Pill) {
    let c = ctx.d.scene.style(p.el);
    let r = radius(c, p.w, p.h);
    let fill = background(c);
    // an even border all round (edge-label pills of the theme), drawn inside the pill
    let bw = border(c)[0];
    let stroke = if bw > 0.0 {
        format!(
            "{} stroke-width=\"{}\"",
            paint("stroke", Some(c.border_color(0))),
            n(bw)
        )
    } else {
        String::new()
    };
    let inset = bw / 2.0;
    let _ = write!(
        ctx.out,
        "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{}\"{}{stroke}/>",
        n(p.x + inset),
        n(p.y + inset),
        n(p.w - bw),
        n(p.h - bw),
        n((r - inset).max(0.0)),
        paint("fill", fill)
    );
    text(ctx, &p.run, None);
}

pub(super) fn anim_attrs(ctx: &Ctx, key: &str) -> String {
    let mut s = String::new();
    for (k, v) in ctx.d.anim.attributes(key) {
        let _ = write!(s, " {k}=\"{}\"", esc(&v));
    }
    s
}

/// Decorated box (background, borders per side, shadow) of an element of `w × h` at `(x, y)`.
pub(super) fn decorated_box(ctx: &mut Ctx, c: &Computed, x: f64, y: f64, w: f64, h: f64) {
    let r = radius(c, w, h);
    let b = border(c);
    let bg = background(c);
    // a box that does not hide what is under it (`transparent`, `rgba(…)`): its shadow is cut out
    // of it and its border drawn as a ring, as a browser draws them
    let dashed_sides = ["top", "right", "bottom", "left"].iter().all(|side| {
        matches!(
            c.get(&format!("border-{side}-style")).unwrap_or("none"),
            "dashed" | "dotted"
        )
    });
    if bg.is_none_or(|c| c.a < 0.999) && !(dashed_sides && b[0] > 0.0) {
        see_through_box(ctx, c, (x, y, w, h), (r, b, bg));
        return;
    }
    // rasterisation: shadow approximated without a filter (resvg's blur costs ~30 ms per card)
    let filt = if ctx.raster {
        raster_shadow(ctx, c, x, y, w, h, r);
        String::new()
    } else {
        shadow(ctx, c)
    };
    let style = |side: &str| {
        c.get(&format!("border-{side}-style"))
            .unwrap_or("none")
            .to_string()
    };
    let dashed = ["top", "right", "bottom", "left"]
        .iter()
        .all(|s| matches!(style(s).as_str(), "dashed" | "dotted"));
    let side_col = c.border_color(3);
    let top_col = c.border_color(0);
    if dashed && b[0] > 0.0 {
        // dashed border: background, then stroke in the middle of the border
        let bw = b[0];
        let _ = write!(
            ctx.out,
            "<path d=\"{}\"{}{}/>",
            rrect(x, y, w, h, r, r),
            paint("fill", bg),
            filt
        );
        let dash = if style("top") == "dotted" {
            format!("{} {}", n(bw), n(bw))
        } else {
            format!("{} {}", n(bw * 3.0), n(bw * 3.0))
        };
        let _ = write!(
            ctx.out,
            "<path d=\"{}\" fill=\"none\"{} stroke-width=\"{}\" stroke-dasharray=\"{dash}\"/>",
            rrect(
                x + bw / 2.0,
                y + bw / 2.0,
                w - bw,
                h - bw,
                (r - bw / 2.0).max(0.0),
                (r - bw / 2.0).max(0.0)
            ),
            paint("stroke", Some(side_col)),
            n(bw)
        );
        return;
    }
    if b.iter().all(|v| *v == 0.0) {
        let _ = write!(
            ctx.out,
            "<path d=\"{}\"{}{}/>",
            rrect(x, y, w, h, r, r),
            paint("fill", bg),
            filt
        );
        return;
    }
    // outline (colour of the sides), top band (colour of the top), then inner background
    let _ = write!(
        ctx.out,
        "<path d=\"{}\"{}{}/>",
        rrect(x, y, w, h, r, r),
        paint("fill", Some(side_col)),
        filt
    );
    if b[0] > 0.0 && top_col != side_col {
        let _ = write!(
            ctx.out,
            "<path d=\"{}\"{}/>",
            top_band(x, y, w, r, b[0]),
            paint("fill", Some(top_col))
        );
    }
    let ri_t = (r - b[0].max(b[3])).max(0.0);
    let ri_b = (r - b[2].max(b[3])).max(0.0);
    let _ = write!(
        ctx.out,
        "<path d=\"{}\"{}/>",
        rrect(
            x + b[3],
            y + b[0],
            w - b[1] - b[3],
            h - b[0] - b[2],
            ri_t,
            ri_b
        ),
        paint("fill", bg.or(Some(Rgba::WHITE)))
    );
}

/// A box whose background lets what is under it show (FORMAT §4: `background: transparent`,
/// `rgba(…)`): the shadow only outside the box (clipped), the background inside the border, the
/// border as a ring (outer outline minus inner one), the top band of another colour on it.
fn see_through_box(
    ctx: &mut Ctx,
    c: &Computed,
    (x, y, w, h): (f64, f64, f64, f64),
    (r, b, bg): (f64, [f64; 4], Option<Rgba>),
) {
    let outer = rrect(x, y, w, h, r, r);
    let has_shadow = c
        .get("box-shadow")
        .is_some_and(|v| !v.trim().is_empty() && v.trim() != "none");
    if has_shadow {
        let id = format!("{}-cut{}", ctx.prefix, ctx.defs.len());
        let m = 200.0;
        ctx.defs.push(format!(
            "<clipPath id=\"{id}\"><path clip-rule=\"evenodd\" d=\"M{},{} H{} V{} H{} Z {outer}\"/></clipPath>",
            n(x - m),
            n(y - m),
            n(x + w + m),
            n(y + h + m),
            n(x - m)
        ));
        let _ = write!(ctx.out, "<g clip-path=\"url(#{id})\">");
        if ctx.raster {
            raster_shadow(ctx, c, x, y, w, h, r);
        } else {
            let filt = shadow(ctx, c);
            let _ = write!(ctx.out, "<path d=\"{outer}\" fill=\"#000\"{filt}/>");
        }
        ctx.out.push_str("</g>");
    }
    let ri_t = (r - b[0].max(b[3])).max(0.0);
    let ri_b = (r - b[2].max(b[3])).max(0.0);
    let inner = rrect(
        x + b[3],
        y + b[0],
        w - b[1] - b[3],
        h - b[0] - b[2],
        ri_t,
        ri_b,
    );
    let _ = write!(ctx.out, "<path d=\"{inner}\"{}/>", paint("fill", bg));
    if b.iter().any(|v| *v > 0.0) {
        let _ = write!(
            ctx.out,
            "<path fill-rule=\"evenodd\" d=\"{outer} {inner}\"{}/>",
            paint("fill", Some(c.border_color(3)))
        );
        if b[0] > 0.0 && c.border_color(0) != c.border_color(3) {
            let _ = write!(
                ctx.out,
                "<path d=\"{}\"{}/>",
                top_band(x, y, w, r, b[0]),
                paint("fill", Some(c.border_color(0)))
            );
        }
    }
}

/// Shifts a piece of text from the card's frame to the image's.
fn shifted(r: &Run, (x, y): (f64, f64)) -> Run {
    Run {
        x: r.x + x,
        baseline: r.baseline + y,
        ..r.clone()
    }
}

fn shifted_pill(p: &Pill, (x, y): (f64, f64)) -> Pill {
    Pill {
        x: p.x + x,
        y: p.y + y,
        run: shifted(&p.run, (x, y)),
        ..p.clone()
    }
}

pub(super) fn card(ctx: &mut Ctx, id: &str) {
    let d = ctx.d;
    let (Some(b), Some(el)) = (d.boxes.get(id), d.scene.cards.get(id)) else {
        return;
    };
    let (x, y) = (b.x + d.frame.ox, b.y + d.frame.oy);
    let pr = presence(ctx, id);
    if pr.opacity <= 0.001 {
        return;
    }
    let node = d.model.node(id);
    link_open(ctx, node.and_then(|n| n.href.as_deref()));
    let cls: Vec<String> = std::iter::once("card".to_string())
        .chain(node.map(|n| n.classes.clone()).unwrap_or_default())
        .collect();
    let _ = write!(
        ctx.out,
        "<g class=\"{}\" data-id=\"{}\"{}{}>",
        esc(&cls.join(" ")),
        esc(id),
        anim_attrs(ctx, id),
        presence_attrs(pr, x + b.w / 2.0, y + b.h / 2.0)
    );
    let c = d.scene.style(el.el).clone();
    if pr.ring > 0.001 {
        highlight_ring(ctx, &c, (x, y, b.w, b.h), pr.ring);
    }
    // a pile (`cards`): the cards behind first, furthest first, then the top card
    let m = node.map_or(0.0, crate::measure::deck_margin);
    let (fw, fh) = (b.w - m, b.h - m);
    pile(ctx, &c, (x, y, b.w, b.h), m);
    decorated_box(ctx, &c, x, y, fw, fh);
    outline(ctx, &c, (x, y, fw, fh));
    let g = geometry::card(&d.scene, el, fw, fh);
    let at = (x, y);
    if let Some(h) = &g.head {
        head_band(ctx, el, &c, h, (x, y, fw, fh));
        if let Some(k) = &h.kind {
            text(ctx, &shifted(k, at), None);
        }
        text(ctx, &shifted(&h.type_run, at), None);
        deco(ctx, &g.deco, at);
        if let Some(t) = &h.tag {
            tag_pill(ctx, id, &shifted_pill(t, at));
        }
    }
    if let Some(p) = &g.badge {
        tag_pill(ctx, id, &shifted_pill(p, at));
    }
    if let Some(k) = &g.kind {
        text(ctx, &shifted(k, at), None);
    }
    for r in &g.title {
        text(ctx, &shifted(r, at), None);
    }
    if g.head.is_none() {
        deco(ctx, &g.deco, at);
    }
    for (k, li) in g.lis.iter().enumerate() {
        let key = format!("{id}/li[{k}]");
        let lp = presence(ctx, &key);
        if lp.opacity <= 0.001 {
            continue;
        }
        let _ = write!(
            ctx.out,
            "<g class=\"li\"{}{}>",
            anim_attrs(ctx, &key),
            presence_attrs(lp, x + fw / 2.0, y + li.y + li.h / 2.0)
        );
        if let Some((bx, by, bw, bh)) = li.boxed {
            let lc = d.scene.style(el.lis[k].el).clone();
            decorated_box(ctx, &lc, x + bx, y + by, bw, bh);
        }
        line_extras(ctx, li, at);
        for r in &li.runs {
            text(ctx, &shifted(r, at), None);
        }
        ctx.out.push_str("</g>");
    }
    after(ctx, &c, el.after, x, y, fw, fh);
    if let Some((cel, text)) = &el.count {
        pill(
            ctx,
            &geometry::count_pill(&d.scene, *cel, text, x + b.w, y + fh),
        );
    }
    ctx.out.push_str("</g>");
    link_close(ctx, node.and_then(|n| n.href.as_deref()));
}

/// `outline: 2.5px solid <colour>` and `outline-offset` (the state classes' ring, FORMAT
/// §3.2.7): a rounded ring outside the box, as the browser draws it.
fn outline(ctx: &mut Ctx, c: &Computed, (x, y, w, h): (f64, f64, f64, f64)) {
    let Some(v) = c.get("outline") else { return };
    let mut width = None;
    let mut colour = None;
    let mut solid = false;
    for t in v.split_whitespace() {
        if let Some(l) = parse_length(t, c.font_size(), None).filter(|_| width.is_none()) {
            width = Some(l);
        } else if matches!(t, "solid" | "dashed" | "dotted" | "double") {
            solid = true;
        } else if let Some(col) = skemo_lang::style::parse_color(t, c.color()) {
            colour = Some(col);
        }
    }
    let (Some(bw), true) = (width.filter(|w| *w > 0.0), solid) else {
        return;
    };
    let off = c
        .get("outline-offset")
        .and_then(|o| parse_length(o, c.font_size(), None))
        .unwrap_or(0.0);
    let d = off + bw / 2.0;
    let r = radius(c, w, h) + d;
    let _ = write!(
        ctx.out,
        "<path d=\"{}\" fill=\"none\"{} stroke-width=\"{}\"/>",
        rrect(x - d, y - d, w + 2.0 * d, h + 2.0 * d, r, r),
        paint("stroke", Some(colour.unwrap_or_else(|| c.color()))),
        n(bw)
    );
}

/// What a line draws besides its text: the fills under it (`mark`, `code`, table cells), the
/// cells' borders and its state mark.
fn line_extras(ctx: &mut Ctx, li: &geometry::LiGeom, (x, y): (f64, f64)) {
    for f in &li.fills {
        let c = ctx.d.scene.style(f.el);
        let r = radius(c, f.w, f.h);
        let fill = background(c);
        let _ = write!(
            ctx.out,
            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{}\"{}/>",
            n(f.x + x),
            n(f.y + y),
            n(f.w),
            n(f.h),
            n(r),
            paint("fill", fill)
        );
    }
    for r in &li.rules {
        let c = ctx.d.scene.style(r.el);
        let w = border(c)[r.side];
        let _ = write!(
            ctx.out,
            "<path d=\"M{},{} L{},{}\" fill=\"none\"{} stroke-width=\"{}\"/>",
            n(r.x1 + x),
            n(r.y1 + y),
            n(r.x2 + x),
            n(r.y2 + y),
            paint("stroke", Some(c.border_color(r.side))),
            n(w)
        );
    }
    if let Some(b) = &li.icon {
        icon(ctx, b, (x, y));
    }
    if let Some(((sx, sy, sw, sh), s)) = li.sample {
        sample(ctx, s, (sx + x, sy + y, sw, sh));
    }
}

/// A legend's sample: a short arrow in its class's colour, stroke and head, or a chip in a
/// card type's accent.
fn sample(ctx: &mut Ctx, s: crate::scene::Sample, (x, my, w, h): (f64, f64, f64, f64)) {
    match s {
        crate::scene::Sample::Icon(el, i) => {
            let b = geometry::IconBox {
                x: x + (w - h) / 2.0,
                y: my - h / 2.0,
                size: h,
                el,
                icon: i,
                is_mark: false,
            };
            icon(ctx, &b, (0.0, 0.0));
        }
        crate::scene::Sample::Chip(el) => {
            let c = ctx.d.scene.style(el);
            let fill = background(c).or_else(|| c.var_color("--c"));
            // an edge (a zone type's frame), drawn inside the box
            let bw = border(c)[0];
            let stroke = if bw > 0.0 {
                format!(
                    "{} stroke-width=\"{}\"",
                    paint("stroke", Some(c.border_color(0))),
                    n(bw)
                )
            } else {
                String::new()
            };
            let i = bw / 2.0;
            let _ = write!(
                ctx.out,
                "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{}\"{}{stroke}/>",
                n(x + i),
                n(my - h / 2.0 + i),
                n(w - bw),
                n(h - bw),
                n((radius(c, w, h) - i).max(0.0)),
                paint("fill", fill)
            );
        }
        crate::scene::Sample::Arrow { g, line, head } => {
            let col = ctx.d.scene.style(g).color();
            let lc = ctx.d.scene.style(line);
            let sw = lc.length("stroke-width").unwrap_or(1.4);
            let dash = lc
                .get("stroke-dasharray")
                .map(|d| format!(" stroke-dasharray=\"{}\"", esc(d.trim())))
                .unwrap_or_default();
            let cap = lc
                .get("stroke-linecap")
                .map(|d| format!(" stroke-linecap=\"{}\"", esc(d.trim())))
                .unwrap_or_default();
            let shown = ctx.d.scene.style(head).display() != "none";
            let end = if shown { x + w - 7.0 } else { x + w };
            let _ = write!(
                ctx.out,
                "<path d=\"M{},{} L{},{}\" fill=\"none\"{} stroke-width=\"{}\"{dash}{cap}/>",
                n(x),
                n(my),
                n(end),
                n(my),
                paint("stroke", Some(col)),
                n(sw)
            );
            if shown {
                let _ = write!(
                    ctx.out,
                    "<path d=\"M{},{} L{},{} L{},{} Z\"{}/>",
                    n(x + w),
                    n(my),
                    n(x + w - 8.0),
                    n(my - 4.0),
                    n(x + w - 8.0),
                    n(my + 4.0),
                    paint("fill", Some(col))
                );
            }
        }
    }
}

/// The cards behind a pile (`cards`), furthest first; behind a see-through top card they only
/// show around it (as in HTML, where they are shadows of the card).
fn pile(ctx: &mut Ctx, c: &Computed, (x, y, w, h): (f64, f64, f64, f64), m: f64) {
    let (fw, fh) = (w - m, h - m);
    let see_through = m > 0.0 && background(c).is_none_or(|b| b.a < 0.999);
    if see_through {
        let id = format!("{}-cut{}", ctx.prefix, ctx.defs.len());
        let r = radius(c, fw, fh);
        ctx.defs.push(format!(
            "<clipPath id=\"{id}\"><path clip-rule=\"evenodd\" d=\"M{},{} H{} V{} H{} Z {}\"/></clipPath>",
            n(x - 50.0),
            n(y - 50.0),
            n(x + w + 50.0),
            n(y + h + 50.0),
            n(x - 50.0),
            rrect(x, y, fw, fh, r, r)
        ));
        let _ = write!(ctx.out, "<g clip-path=\"url(#{id})\">");
    }
    let mut o = m;
    while o > 0.0 {
        decorated_box(ctx, c, x + o, y + o, fw, fh);
        o -= crate::measure::DECK_STEP;
    }
    if see_through {
        ctx.out.push_str("</g>");
    }
}

/// The chip of a card; in its own group when `tag=` carries step markers (key `card/tag`).
fn tag_pill(ctx: &mut Ctx, id: &str, p: &Pill) {
    let key = format!("{id}/tag");
    if !ctx.d.anim.elements.contains_key(&key) {
        pill(ctx, p);
        return;
    }
    let pr = presence(ctx, &key);
    if pr.opacity <= 0.001 {
        return;
    }
    let _ = write!(
        ctx.out,
        "<g class=\"tag\"{}{}>",
        anim_attrs(ctx, &key),
        presence_attrs(pr, p.x + p.w / 2.0, p.y + p.h / 2.0)
    );
    pill(ctx, p);
    ctx.out.push_str("</g>");
}

/// Opens `<a href>` around a linked element (FORMAT §3.2.1). The value is already validated by
/// `lang` (allow-list of schemes); it is escaped here.
pub(super) fn link_open(ctx: &mut Ctx, href: Option<&str>) {
    if let Some(h) = href {
        let _ = write!(ctx.out, "<a href=\"{}\">", esc(h));
    }
}

pub(super) fn link_close(ctx: &mut Ctx, href: Option<&str>) {
    if href.is_some() {
        ctx.out.push_str("</a>");
    }
}

/// Emphasis outline (F-ANIM-7): accent of the card, with an opacity proportional to `ring`.
fn highlight_ring(ctx: &mut Ctx, c: &Computed, (x, y, w, h): (f64, f64, f64, f64), ring: f64) {
    let accent = c.var_color("--c").unwrap_or(Rgba {
        r: 62.0,
        g: 111.0,
        b: 196.0,
        a: 1.0,
    });
    let r = radius(c, w, h);
    let _ = write!(
        ctx.out,
        "<path d=\"{}\" fill=\"none\"{} stroke-width=\"3\"/>",
        rrect(x - 1.5, y - 1.5, w + 3.0, h + 3.0, r + 1.5, r + 1.5),
        paint(
            "stroke",
            Some(Rgba {
                a: 0.6 * ring,
                ..accent
            })
        )
    );
}

/// Background of the header band: rounded corners at the top, and at the bottom if it fills the
/// card.
fn head_band(
    ctx: &mut Ctx,
    el: &crate::scene::CardEl,
    c: &Computed,
    h: &geometry::HeadGeom,
    (x, y, w, bh): (f64, f64, f64, f64),
) {
    let hc = ctx
        .d
        .scene
        .style(el.header.as_ref().map_or(el.el, |hh| hh.head))
        .clone();
    let r = (radius(c, w, bh) - border(c)[0]).max(0.0);
    // a card without lines is all band: a band that stops within a corner's radius of the card's
    // bottom (a fraction of a pixel when text sizes are scaled and the card is rounded to whole
    // pixels) runs down to it, with the card's bottom corners (`overflow: hidden` clips them in
    // HTML)
    let gap = bh - border(c)[2] - (h.y + h.h);
    let fills = gap < r.max(0.5);
    let band_h = if fills && gap >= 0.5 { h.h + gap } else { h.h };
    let _ = write!(
        ctx.out,
        "<path d=\"{}\"{}/>",
        rrect(
            x + h.x,
            y + h.y,
            h.w,
            band_h,
            r,
            if fills { r } else { 0.0 }
        ),
        paint("fill", background(&hc))
    );
}

/// Pseudo-element `::after` of a card (diamond of a milestone, bar of a period): rectangle
/// positioned in the padding box, possible 45° rotation, `overflow: hidden` honoured.
pub(super) fn after(ctx: &mut Ctx, card: &Computed, el: usize, x: f64, y: f64, w: f64, h: f64) {
    let a = ctx.d.scene.style(el).clone();
    match a.get("content") {
        None => return,
        Some(c) if c.trim() == "none" || c.trim() == "normal" => return,
        _ => {}
    }
    let cb = border(card);
    let (px, py) = (x + cb[3], y + cb[0]);
    let (pw, ph) = (w - cb[1] - cb[3], h - cb[0] - cb[2]);
    let len = |n: &str, of: f64| {
        a.get(n)
            .and_then(|v| parse_length(v, a.font_size(), Some(of)))
    };
    let (mut aw, ah) = (
        len("width", pw).unwrap_or(0.0),
        len("height", ph).unwrap_or(0.0),
    );
    // `top`, or else `bottom` (a period's bar under its label)
    let left = len("left", pw).unwrap_or(0.0);
    let top = len("top", ph)
        .or_else(|| len("bottom", ph).map(|b| ph - b - ah))
        .unwrap_or(0.0);
    let b = border(&a)[0];
    let hidden = card.get("overflow").is_some_and(|o| o.contains("hidden"));
    if hidden {
        aw = aw.min(pw - left);
    }
    if aw <= 0.0 || ah <= 0.0 {
        return;
    }
    let (ow, oh) = (aw + 2.0 * b, ah + 2.0 * b);
    let (ax, ay) = (px + left, py + top);
    let r = radius(&a, ow, oh);
    let rot = a
        .get("transform")
        .is_some_and(|t| t.contains("rotate(45deg)"));
    let tr = if rot {
        format!(
            " transform=\"rotate(45 {} {})\"",
            n(ax + ow / 2.0),
            n(ay + oh / 2.0)
        )
    } else {
        String::new()
    };
    let fill = background(&a);
    if b > 0.0 {
        let _ = write!(
            ctx.out,
            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{}\"{}{}/>",
            n(ax),
            n(ay),
            n(ow),
            n(oh),
            n(r),
            paint("fill", Some(a.border_color(0))),
            tr
        );
        let _ = write!(
            ctx.out,
            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{}\"{}{}/>",
            n(ax + b),
            n(ay + b),
            n(aw),
            n(ah),
            n((r - b).max(0.0)),
            paint("fill", fill),
            tr
        );
    } else {
        let _ = write!(
            ctx.out,
            "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{}\"{}{}/>",
            n(ax),
            n(ay),
            n(aw),
            n(ah),
            n(r),
            paint("fill", fill),
            tr
        );
    }
}

pub(super) fn zone(ctx: &mut Ctx, id: &str) {
    let d = ctx.d;
    let (Some(b), Some(el)) = (d.boxes.get(id), d.scene.zones.get(id)) else {
        return;
    };
    let (x, y) = (b.x + d.frame.ox, b.y + d.frame.oy);
    let pr = presence(ctx, id);
    if pr.opacity <= 0.001 {
        return;
    }
    let node = d.model.node(id);
    if node.is_some_and(|n| n.stack) {
        // a stack draws nothing of its own (D-050)
        let kids: Vec<(String, Kind)> = d
            .model
            .children(Some(id))
            .map(|n| (n.id.clone(), n.kind))
            .collect();
        for (k, kind) in kids {
            match kind {
                Kind::Zone => zone(ctx, &k),
                Kind::Card => card(ctx, &k),
            }
        }
        return;
    }
    link_open(ctx, node.and_then(|n| n.href.as_deref()));
    let cls: Vec<String> = std::iter::once("zone".to_string())
        .chain(node.map(|n| n.classes.clone()).unwrap_or_default())
        .collect();
    let _ = write!(
        ctx.out,
        "<g class=\"{}\" data-id=\"{}\"{}{}>",
        esc(&cls.join(" ")),
        esc(id),
        anim_attrs(ctx, id),
        presence_attrs(pr, x + b.w / 2.0, y + b.h / 2.0)
    );
    let c = d.scene.style(el.el).clone();
    decorated_box(ctx, &c, x, y, b.w, b.h);
    tl_lines(ctx, y, y + b.h);
    let title = node.map(|n| n.title.clone()).unwrap_or_default();
    let tag = node.and_then(|n| n.tag.clone());
    let mut zt = geometry::zone_title(&d.scene, el, (&title, tag.as_deref()), b.w, b.h);
    zt.run.x += x;
    zt.run.baseline += y;
    text(ctx, &zt.run, None);
    deco(ctx, &zt.deco, (x, y));
    if let Some(mut p) = zt.tag {
        p.x += x;
        p.y += y;
        p.run.x += x;
        p.run.baseline += y;
        pill(ctx, &p);
    }
    for s in &zt.subs {
        text(ctx, &shifted(s, (x, y)), None);
    }
    let kids: Vec<(String, Kind)> = d
        .model
        .children(Some(id))
        .map(|n| (n.id.clone(), n.kind))
        .collect();
    for (k, kind) in kids {
        match kind {
            Kind::Zone => zone(ctx, &k),
            Kind::Card => card(ctx, &k),
        }
    }
    ctx.out.push_str("</g>");
    link_close(ctx, node.and_then(|n| n.href.as_deref()));
}

/// The diagram's title (D-050): its text, then its pills on the right of the same line.
pub(super) fn diagram_title(ctx: &mut Ctx) {
    let d = ctx.d;
    let (Some(t), Some(el), Some(b)) = (&d.model.title, &d.scene.title, d.title_box) else {
        return;
    };
    let (x, y) = (b.x + d.frame.ox, b.y + d.frame.oy);
    let mut g = geometry::title_layout(&d.scene, el, t, (b.w, b.h));
    ctx.out.push_str("<g class=\"diagram-title\">");
    g.run.x += x;
    g.run.baseline += y;
    text(ctx, &g.run, None);
    for mut p in g.pills {
        p.x += x;
        p.y += y;
        p.run.x += x;
        p.run.baseline += y;
        pill(ctx, &p);
    }
    for s in &g.subs {
        text(ctx, &shifted(s, (x, y)), None);
    }
    ctx.out.push_str("</g>");
}
