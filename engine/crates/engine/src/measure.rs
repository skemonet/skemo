//! Measurement of the cards and labels (E-MEAS-1, 2, 4), according to the style resolved by `lang`.
//!
//! Box model reduced to what the prototype produces, matched on Chromium:
//! - width = natural content ("max-content" width) + padding + borders, bounded by
//!   `min-width` / `max-width` (`box-sizing: border-box`); beyond that, the text wraps;
//! - height of a text block = number of lines × line height truncated to 1/64 px
//!   (the `span.mono` and the badge do not enlarge the line: checked on the oracle);
//! - vertical margins collapsed between `ul` and its first `li`;
//! - final size rounded up to the pixel (as `Math.ceil(getBoundingClientRect())`).

use crate::fonts::{TextStyle, layout_unit, layout_unit_floor};
use crate::scene::{CardEl, Scene};
use skemo_lang::style::Computed;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Size {
    pub w: f64,
    pub h: f64,
}

#[derive(Debug, Clone, Default)]
pub struct Sizes {
    pub cards: BTreeMap<String, Size>,
    pub labels: BTreeMap<String, Size>,
    /// Zones whose title the theme sets below their content (`bottom`, no `top`; `.bare`): their
    /// room is kept below (D-050).
    pub captions_below: std::collections::BTreeSet<String>,
    /// Right end of a zone's title (its tag pill included) from the zone's left edge, for titles
    /// at the top left only (D-064): a card linked from above keeps clear of it.
    pub title_ends: BTreeMap<String, f64>,
    /// Room the description lines of a zone take under its title (FORMAT §3.2.3), added to the
    /// zone's top margin.
    pub zone_heads: BTreeMap<String, f64>,
}

pub fn text_style(c: &Computed) -> TextStyle {
    TextStyle {
        families: c.font_family(),
        size: c.font_size(),
        weight: c.font_weight(),
        italic: c.italic(),
        letter_spacing: c.letter_spacing(),
    }
}

/// Line height used (1/64 px, truncated); `normal` ≈ 1.2 × the size.
pub fn line_height(c: &Computed) -> f64 {
    layout_unit_floor(c.line_height().unwrap_or(1.2 * c.font_size()))
}

pub fn tw(c: &Computed, text: &str) -> f64 {
    layout_unit(text_style(c).width(&transform(c, text)))
}

/// `text-transform` (only `uppercase` is used by the theme).
pub fn transform(c: &Computed, text: &str) -> String {
    match c.get("text-transform") {
        Some("uppercase") => text.to_uppercase(),
        Some("lowercase") => text.to_lowercase(),
        _ => text.to_string(),
    }
}

/// Border widths as Chromium applies them: a width of at least 1 px is truncated to the whole pixel
/// (1.5 px → 1 px), a thinner non-zero width is 1 px.
pub fn border(c: &Computed) -> [f64; 4] {
    c.border().map(|w| {
        if w <= 0.0 {
            0.0
        } else if w < 1.0 {
            1.0
        } else {
            w.floor()
        }
    })
}

fn ceil(v: f64) -> f64 {
    (v - 1e-6).ceil()
}

/// Splits `text` into lines in a width `avail`, the first line being reduced by `first_indent`
/// (inline badge). Breaks at spaces, as the browser.
pub fn break_lines(c: &Computed, text: &str, avail: f64, first_indent: f64) -> Vec<String> {
    let st = text_style(c);
    let text = transform(c, text);
    if c.get("white-space")
        .is_some_and(|w| w.trim() == "nowrap" || w.trim() == "pre")
    {
        return text.split('\n').map(String::from).collect();
    }
    let mut lines: Vec<String> = Vec::new();
    for para in text.split('\n') {
        let mut cur = String::new();
        let mut width_left = avail - if lines.is_empty() { first_indent } else { 0.0 };
        for w in para.split(' ') {
            let cand = if cur.is_empty() {
                w.to_string()
            } else {
                format!("{cur} {w}")
            };
            if !cur.is_empty() && layout_unit(st.width(&cand)) > width_left + 1e-6 {
                lines.push(std::mem::take(&mut cur));
                width_left = avail;
                cur = w.to_string();
            } else {
                cur = cand;
            }
        }
        lines.push(cur);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

fn line_count(c: &Computed, text: &str, avail: f64, first_indent: f64) -> usize {
    break_lines(c, text, avail, first_indent).len()
}

/// Natural width of the band: icon, name (or type), link mark, chip.
fn head_width(scene: &Scene, h: &crate::scene::HeadEl, (lead, trail): (f64, f64)) -> f64 {
    let hc = scene.style(h.head);
    let hp = hc.padding();
    let mut w = hp[1] + hp[3] + lead + tw(scene.style(h.type_el), &h.type_text) + trail;
    if let Some((t, txt)) = &h.tag {
        let tc = scene.style(*t);
        let gap = hc
            .length("gap")
            .or_else(|| hc.length("column-gap"))
            .unwrap_or(0.0);
        w += gap + tw(tc, txt) + tc.padding()[1] + tc.padding()[3];
    }
    // the category's own line above the name (band look)
    if let Some((k, txt)) = &h.kind {
        w = w.max(hp[1] + hp[3] + tw(scene.style(*k), txt));
    }
    w
}

/// Height of the category line above the name (`.kind`, FORMAT §3.2.6), margins included.
pub fn kind_height(scene: &Scene, kind: Option<&(usize, String)>) -> f64 {
    kind.map_or(0.0, |(k, _)| {
        let c = scene.style(*k);
        line_height(c) + c.margin()[0] + c.margin()[2]
    })
}

/// Size of a card. `forced`: imposed width (timeline period: max(bar, natural)), without
/// `max-width`; the height is then measured at that width.
pub fn card_size(scene: &Scene, card: &CardEl, forced: Option<f64>) -> Size {
    let cs = scene.style(card.el);
    let pad = cs.padding();
    let bor = border(cs);
    let frame_w = pad[1] + pad[3] + bor[1] + bor[3];
    let title = scene.style(card.title);
    let tp = title.padding();
    let title_text = card.title_text.clone();

    // largeur naturelle
    let mut natural: f64 = 0.0;
    let mut badge_w = 0.0;
    let mut band_w = 0.0;
    let (icon_w, icon_size) = scene.icon_lead(&card.decor);
    let (mark_w, _) = scene.mark_trail(&card.decor);
    // icon and link mark in the band, or in the title
    let in_head = card.header.is_some();
    let title_lead = if in_head { 0.0 } else { icon_w };
    let title_trail = if card.band() { 0.0 } else { mark_w };
    if let Some(h) = &card.header {
        let w = head_width(scene, h, (icon_w, if h.band { mark_w } else { 0.0 }));
        natural = natural.max(w);
        if h.band {
            band_w = w + frame_w;
        }
    } else if let Some((b, txt)) = &card.badge {
        let bc = scene.style(*b);
        badge_w = tw(bc, txt) + bc.padding()[1] + bc.padding()[3] + bc.margin()[1] + bc.margin()[3];
    }
    // a badge on its own line above the title (`display: block`): no indent of the title
    let eyebrow = block_badge(scene, card);
    if let Some((bw, _)) = eyebrow {
        natural = natural.max(tp[1] + tp[3] + bw);
        badge_w = 0.0;
    }
    if !card.band() {
        natural = natural
            .max(tp[1] + tp[3] + badge_w + title_lead + tw(title, &title_text) + title_trail);
    }
    if let Some((k, txt)) = &card.kind {
        natural = natural.max(tw(scene.style(*k), txt));
    }
    let ul_pad = card.ul.map(|u| {
        let p = scene.style(u).padding();
        p[1] + p[3]
    });
    // a table never wraps: the card is at least as wide as its widest table
    let mut table_w: f64 = 0.0;
    for (k, li) in card.lis.iter().enumerate() {
        let w = match li.table {
            Some(t) if crate::lines::first_of_table(card, k) => {
                let w = crate::lines::table(scene, card, t).w + host_frame(scene, card, t).0;
                table_w = table_w.max(ul_pad.unwrap_or(0.0) + w + frame_w);
                w
            }
            Some(_) => 0.0,
            None => crate::lines::line_width(scene, li),
        };
        natural = natural.max(ul_pad.unwrap_or(0.0) + w);
    }
    let mut width = natural + frame_w;
    if let Some(mx) = cs.max_width() {
        // a name in the band does not wrap: it keeps its natural width
        width = width.min(mx.max(band_w).max(table_w));
    }
    width = width.max(cs.min_width());
    if let Some(f) = forced {
        width = f;
    }
    let avail = width - frame_w;

    // hauteur
    let mut h = 0.0;
    if let Some(hd) = &card.header {
        let hc = scene.style(hd.head);
        let hp = hc.padding();
        let mut inner = line_height(scene.style(hd.type_el)).max(icon_size);
        if let Some((t, _)) = &hd.tag {
            let tc = scene.style(*t);
            inner = inner.max(line_height(tc) + tc.padding()[0] + tc.padding()[2]);
        }
        h += hp[0] + hp[2] + inner + kind_height(scene, hd.kind.as_ref());
    }
    if let Some((_, bh)) = eyebrow {
        h += bh;
    }
    h += kind_height(scene, card.kind.as_ref());
    if !card.band() {
        let tl = line_count(
            title,
            &title_text,
            avail - tp[1] - tp[3],
            badge_w + title_lead,
        );
        h += tp[0] + tp[2] + tl as f64 * line_height(title);
    }
    if let Some(u) = card.ul {
        h += list_height(scene, card, u, avail, title.margin()[2]);
    }
    h += pad[0] + pad[2] + bor[0] + bor[2];
    Size {
        w: if forced.is_some() { width } else { ceil(width) },
        h: ceil(h),
    }
}

/// Badge drawn on its own line above the title (`card .title::before { display: block }`, used by
/// the `eyebrow` look and by frieze dates): `(width, height)` with padding and margins, or `None`
/// for an inline badge, an empty one or a card with a header.
pub fn block_badge(scene: &Scene, card: &CardEl) -> Option<(f64, f64)> {
    if card.header.is_some() {
        return None;
    }
    let (b, txt) = card.badge.as_ref()?;
    let bc = scene.style(*b);
    if bc.display() != "block" || txt.is_empty() {
        return None;
    }
    let (p, m) = (bc.padding(), bc.margin());
    Some((
        tw(bc, txt) + p[1] + p[3] + m[1] + m[3],
        line_height(bc) + p[0] + p[2] + m[0] + m[2],
    ))
}

/// Horizontal and vertical room around table `t` in its `li.table` (padding and borders), and
/// the host's top margin.
pub fn host_frame(scene: &Scene, card: &CardEl, t: usize) -> (f64, f64, f64) {
    card.tables.get(t).map_or((0.0, 0.0, 0.0), |te| {
        let c = scene.style(te.host);
        let (p, b) = (c.padding(), border(c));
        (
            p[1] + p[3] + b[1] + b[3],
            p[0] + p[2] + b[0] + b[2],
            c.margin()[0],
        )
    })
}

/// Height of one line of a card (`None` for a table row other than the table's first, which
/// takes the whole table's room) and its top margin.
pub fn line_box(scene: &Scene, card: &CardEl, k: usize, avail: f64) -> Option<(f64, f64)> {
    let li = &card.lis[k];
    if let Some(t) = li.table {
        if !crate::lines::first_of_table(card, k) {
            return None;
        }
        let (_, vh, mt) = host_frame(scene, card, t);
        return Some((crate::lines::table(scene, card, t).h + vh, mt));
    }
    let lc = scene.style(li.el);
    let lp = lc.padding();
    let room = avail - lp[1] - lp[3] - crate::lines::icon_lead(scene, li).0;
    let n = if li.parts.is_empty() {
        line_count(lc, &li.rest, room - li_width_mono(scene, li), 0.0)
    } else {
        crate::lines::break_parts(scene, li.el, &li.parts, room).len()
    };
    Some((n as f64 * line_height(lc) + lp[0] + lp[2], lc.margin()[0]))
}

/// Height of a card's `ul`, margins included: the top one merges with the title's bottom margin
/// (`title_mb`) and with the first line's.
fn list_height(scene: &Scene, card: &CardEl, u: usize, avail: f64, title_mb: f64) -> f64 {
    let uc = scene.style(u);
    let up = uc.padding();
    let mut first = true;
    let mut ul_h = up[0] + up[2];
    let mut top_margin = uc.margin()[0];
    // lines that share a place (`scene::slots`) count once, for the tallest
    let mut slot: Option<(usize, f64)> = None;
    for (k, li) in card.lis.iter().enumerate() {
        let Some((lh, mt)) = line_box(scene, card, k, avail - up[1] - up[3]) else {
            continue;
        };
        if li.table.is_none()
            && let Some((s, h)) = slot.as_mut()
            && *s == li.slot
        {
            *h = h.max(lh);
            continue;
        }
        ul_h += slot.map_or(0.0, |(_, h)| h);
        slot = Some((
            if li.table.is_some() {
                usize::MAX
            } else {
                li.slot
            },
            lh,
        ));
        if first && up[0] == 0.0 && border(uc)[0] == 0.0 {
            // margin collapsing: the ul's and the first li's
            top_margin = top_margin.max(mt);
        } else {
            ul_h += mt;
        }
        first = false;
    }
    ul_h += slot.map_or(0.0, |(_, h)| h);
    // top margin of the ul facing the title (collapsed with the title's bottom margin)
    top_margin.max(title_mb) + ul_h + uc.margin()[2]
}

pub fn li_width_mono(scene: &Scene, li: &crate::scene::LiEl) -> f64 {
    li.mono.as_ref().map_or(0.0, |(m, ip)| {
        let mc = scene.style(*m);
        tw(mc, ip) + mc.margin()[1] + mc.margin()[3]
    })
}

/// Offset of each card behind the top one in a pile (`cards`), right and down (px).
pub const DECK_STEP: f64 = 5.0;

/// Room the cards behind take beyond the top card of a pile, right and down (0 for a card).
pub fn deck_margin(n: &skemo_lang::Node) -> f64 {
    n.deck
        .map_or(0.0, |d| f64::from(d.layers().saturating_sub(1)) * DECK_STEP)
}

/// Adds (`sign` 1) or removes (-1) the room of the piles: the passes that even widths out work
/// on the top cards, so that a pile's top card is as wide as its siblings.
pub fn decks(model: &skemo_lang::Model, sizes: &mut Sizes, sign: f64) {
    for n in model.nodes.iter().filter(|n| n.deck.is_some()) {
        if let Some(s) = sizes.cards.get_mut(&n.id) {
            s.w += sign * deck_margin(n);
            s.h += sign * deck_margin(n);
        }
    }
}

/// Right end of zone `id`'s title from the zone's left edge, when it does not depend on the
/// zone's width (a title at the left; a centred or right-aligned one is left out).
fn title_end(
    scene: &Scene,
    model: &skemo_lang::Model,
    id: &str,
    z: &crate::scene::ZoneEl,
) -> Option<f64> {
    let n = model.node(id).filter(|n| !n.title.is_empty())?;
    let end = |w: f64| {
        let t = crate::geometry::zone_title(scene, z, (&n.title, n.tag.as_deref()), w, 400.0);
        t.tag.map_or(t.span.1, |p| t.span.1.max(p.x + p.w))
    };
    let e = end(4000.0);
    ((e - end(8000.0)).abs() < 0.5).then_some(e + 2.0)
}

/// Measures every card and every label of the scene.
pub fn measure(scene: &Scene, model: &skemo_lang::Model) -> Sizes {
    let mut out = Sizes::default();
    for n in &model.nodes {
        if let Some(c) = scene.cards.get(&n.id) {
            let mut s = card_size(scene, c, None);
            // `size="w,h"` on a card: at least that room (the editor resizes cards this way)
            if let Some([w, h]) = n.size {
                s.w = s.w.max(w);
                s.h = s.h.max(h);
            }
            out.cards.insert(n.id.clone(), s);
        }
    }
    decks(model, &mut out, 1.0);
    for (id, z) in &scene.zones {
        let extra: f64 = z
            .subs
            .iter()
            .map(|(el, _)| {
                let c = scene.style(*el);
                line_height(c) + c.margin()[0] + c.margin()[2]
            })
            .sum();
        if extra > 0.0 {
            out.zone_heads.insert(id.clone(), extra.ceil());
        }
        let c = scene.style(z.title);
        let set = |n: &str| c.get(n).is_some_and(|v| v.trim() != "auto");
        if set("bottom") && !set("top") {
            out.captions_below.insert(id.clone());
        } else if let Some(end) = title_end(scene, model, id, z) {
            out.title_ends.insert(id.clone(), end);
        }
    }
    for e in &model.edges {
        if let Some(&l) = scene.labels.get(&e.id) {
            let c = scene.style(l);
            let p = c.padding();
            let b = border(c);
            let w = tw(c, &e.label) + p[1] + p[3] + b[1] + b[3];
            let h = line_height(c) + p[0] + p[2] + b[0] + b[2];
            out.labels.insert(
                e.id.clone(),
                Size {
                    w: ceil(w),
                    h: ceil(h),
                },
            );
        }
    }
    out
}
