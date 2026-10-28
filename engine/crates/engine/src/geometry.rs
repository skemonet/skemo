//! Inner geometry of the cards, zones and labels: position of each text and each chip, relative to
//! the element's top-left corner. Shared by the SVG and PNG renderings (the HTML lets the browser
//! do it, with the sizes imposed).

use crate::icons::Icon;
use crate::measure::{border, break_lines, li_width_mono, line_height, text_style, transform, tw};
use crate::scene::{CardEl, Decor, Scene, ZoneEl};
use skemo_lang::style::Computed;

/// A piece of text on a baseline.
#[derive(Debug, Clone)]
pub struct Run {
    pub text: String,
    pub x: f64,
    pub baseline: f64,
    pub width: f64,
    /// Element whose style applies.
    pub el: usize,
}

/// A chip (badge, tag): rounded rectangle and its text.
#[derive(Debug, Clone)]
pub struct Pill {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub el: usize,
    pub run: Run,
}

#[derive(Debug, Clone)]
pub struct HeadGeom {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub type_run: Run,
    pub tag: Option<Pill>,
    /// The category above the name (band look).
    pub kind: Option<Run>,
}

#[derive(Debug, Clone, Default)]
pub struct LiGeom {
    pub y: f64,
    pub h: f64,
    pub runs: Vec<Run>,
    /// The box of a styled line (`li.pill`, D-050): `(x, y, w, h)`, in the card's frame.
    pub boxed: Option<(f64, f64, f64, f64)>,
    /// Boxes drawn under the text with their element's background: a `mark`, a `code`, a table
    /// cell (`td.error`), in the card's frame.
    pub fills: Vec<Fill>,
    /// Borders of table cells: segments `(x1, y1, x2, y2)`, their width and colour's element and
    /// side (0 top, 1 right, 2 bottom, 3 left).
    pub rules: Vec<Rule>,
    /// The line's state mark (`.warn`, FORMAT §3.2.7).
    pub icon: Option<IconBox>,
    /// A legend's sample: `(x, middle y, width, height)` and the sample.
    pub sample: Option<((f64, f64, f64, f64), crate::scene::Sample)>,
}

/// A box filled with the background of its element.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fill {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub el: usize,
}

/// A cell's border: from `(x1, y1)` to `(x2, y2)`, with the width and colour of side `side` of
/// element `el`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rule {
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
    pub el: usize,
    pub side: usize,
}

/// An icon laid out: top-left corner, side of the square, element whose style gives the colour.
#[derive(Debug, Clone)]
pub struct IconBox {
    pub x: f64,
    pub y: f64,
    pub size: f64,
    pub el: usize,
    pub icon: Icon,
    /// `true`: link mark (not the node's icon).
    pub is_mark: bool,
}

/// Icon of a node and link mark.
#[derive(Debug, Clone, Default)]
pub struct DecoGeom {
    pub icon: Option<IconBox>,
    pub mark: Option<IconBox>,
}

/// Lays the icon (of width `lead`, before the text at `x`) and the mark (after the text, which ends
/// at `end`), centred vertically on the line `[top, top + lh]`.
fn place_decor(scene: &Scene, d: &Decor, (x, end): (f64, f64), (top, lh): (f64, f64)) -> DecoGeom {
    let (_, isize) = scene.icon_lead(d);
    let (mw, msize) = scene.mark_trail(d);
    DecoGeom {
        icon: d.icon.as_ref().map(|(el, icon)| IconBox {
            x,
            y: top + (lh - isize) / 2.0,
            size: isize,
            el: *el,
            icon: *icon,
            is_mark: false,
        }),
        mark: d.mark.as_ref().map(|(el, icon)| IconBox {
            x: end + mw - msize,
            y: top + (lh - msize) / 2.0,
            size: msize,
            el: *el,
            icon: *icon,
            is_mark: true,
        }),
    }
}

#[derive(Debug, Clone)]
pub struct CardGeom {
    pub deco: DecoGeom,
    pub head: Option<HeadGeom>,
    pub badge: Option<Pill>,
    /// The category on its own line above the title (a card without a band).
    pub kind: Option<Run>,
    pub title: Vec<Run>,
    pub lis: Vec<LiGeom>,
}

/// Baseline of a line of height `lh` starting at `top`, for the style `c`.
/// As Chromium: metrics rounded to the pixel, half-leading truncated to the pixel
/// (measured in Chromium on the oracle during milestone E0).
pub fn baseline(c: &Computed, top: f64, lh: f64) -> f64 {
    let (asc, desc) = text_style(c).metrics();
    top + ((lh - (asc + desc)) / 2.0).floor() + asc
}

fn align(c: &Computed, avail: f64, used: f64) -> f64 {
    match c.get("text-align") {
        Some("center") => (avail - used) / 2.0,
        Some("right" | "end") => avail - used,
        _ => 0.0,
    }
}

/// Inner margins of a card: left edge of the content and available width.
struct CardBox {
    left: f64,
    avail: f64,
    bor: [f64; 4],
    pad: [f64; 4],
}

/// Geometry of a card of size `w × h` (the card's frame).
pub fn card(scene: &Scene, card: &CardEl, w: f64, h: f64) -> CardGeom {
    let cs = scene.style(card.el);
    let (pad, bor) = (cs.padding(), border(cs));
    let cb = CardBox {
        left: bor[3] + pad[3],
        avail: w - bor[3] - pad[3] - bor[1] - pad[1],
        bor,
        pad,
    };
    let mut y = bor[0] + pad[0];
    let mut deco = DecoGeom::default();
    let head = card
        .header
        .as_ref()
        .map(|hd| card_head(scene, (hd, &card.decor), &cb, (y, card.tag_side), &mut deco));
    if let Some(hd) = &head {
        y += hd.h;
    }
    let kind = card.kind.as_ref().map(|(k, txt)| {
        let c = scene.style(*k);
        let lh = line_height(c);
        let top = y + c.margin()[0];
        y += lh + c.margin()[0] + c.margin()[2];
        let tcs = scene.style(card.title);
        let x = cb.left + tcs.padding()[3] + align(c, cb.avail, tw(c, txt));
        Run {
            text: transform(c, txt),
            x,
            baseline: baseline(c, top, lh),
            width: tw(c, txt),
            el: *k,
        }
    });
    let (title, badge, title_bottom) = card_title(scene, card, &cb, (w, h), (y, &mut deco));
    let lis = card_lis(scene, card, &cb, title_bottom);
    CardGeom {
        deco,
        head,
        badge,
        kind,
        title,
        lis,
    }
}

/// Header band: text of the type and chip (tag) aligned right.
fn card_head(
    scene: &Scene,
    (h, decor): (&crate::scene::HeadEl, &Decor),
    cb: &CardBox,
    (y, tag_side): (f64, i8),
    deco: &mut DecoGeom,
) -> HeadGeom {
    let hc = scene.style(h.head);
    let hp = hc.padding();
    let tcs = scene.style(h.type_el);
    // the category's line above the name: the name line starts below it
    let kind_h = crate::measure::kind_height(scene, h.kind.as_ref());
    let kind = h.kind.as_ref().map(|(k, txt)| {
        let c = scene.style(*k);
        let lh = line_height(c);
        Run {
            text: transform(c, txt),
            // centred with the name on a card without lines (`text-align` from the theme)
            x: cb.left + hp[3] + align(c, cb.avail - hp[1] - hp[3], tw(c, txt)),
            baseline: baseline(c, y + hp[0] + c.margin()[0], lh),
            width: tw(c, txt),
            el: *k,
        }
    });
    let y = y + kind_h;
    let (icon_w, icon_size) = scene.icon_lead(decor);
    let mut inner = line_height(tcs).max(icon_size);
    let tag_dims = h.tag.as_ref().map(|(t, txt)| {
        let tc = scene.style(*t);
        let tp = tc.padding();
        (tc, tp, tw(tc, txt), line_height(tc) + tp[0] + tp[2])
    });
    if let Some((_, _, _, th)) = &tag_dims {
        inner = inner.max(*th);
    }
    let (hx, hw) = (cb.left, cb.avail);
    let ty = y + hp[0] + (inner - line_height(tcs)) / 2.0;
    let type_run = Run {
        text: transform(tcs, &h.type_text),
        x: hx + hp[3] + icon_w,
        baseline: baseline(tcs, ty, line_height(tcs)),
        width: tw(tcs, &h.type_text),
        el: h.type_el,
    };
    // icon before the name; mark after the name in the "band" look
    let head_decor = Decor {
        icon: decor.icon,
        mark: if h.band { decor.mark } else { None },
    };
    let line = (y + hp[0], inner);
    let tag = h
        .tag
        .as_ref()
        .zip(tag_dims)
        .map(|((t, txt), (tc, tp, tw_, th))| {
            // `.tag-left` (D-050): the tag before the name, otherwise at the right end
            let px = if tag_side < 0 {
                hx + hp[3]
            } else {
                hx + hw - hp[1] - (tw_ + tp[1] + tp[3])
            };
            let py = y + hp[0] + (inner - th) / 2.0;
            Pill {
                x: px,
                y: py,
                w: tw_ + tp[1] + tp[3],
                h: th,
                el: *t,
                run: Run {
                    text: transform(tc, txt),
                    x: px + tp[3],
                    baseline: baseline(tc, py + tp[0], line_height(tc)),
                    width: tw_,
                    el: *t,
                },
            }
        });
    // `text-align: center` on the name (a card without lines): the icon, name and link mark are
    // centred between the left padding and the tag (or the right padding)
    let mut type_run = type_run;
    let mut lead = hx + hp[3];
    if tag_side < 0
        && let Some(t) = &tag
    {
        let shift = t.w + hc.length("column-gap").unwrap_or(12.0);
        lead += shift;
        type_run.x += shift;
    }
    if tcs.get("text-align") == Some("center") {
        let right = tag.as_ref().map_or(hx + hw - hp[1], |t| {
            t.x - hc.length("column-gap").unwrap_or(12.0)
        });
        let used = icon_w + type_run.width + scene.mark_trail(&head_decor).0;
        let shift = ((right - lead) - used).max(0.0) / 2.0;
        lead += shift;
        type_run.x += shift;
    }
    let end = type_run.x + type_run.width;
    *deco = place_decor(scene, &head_decor, (lead, end), line);
    HeadGeom {
        x: hx,
        y: y - kind_h,
        w: hw,
        h: hp[0] + hp[2] + inner + kind_h,
        type_run,
        tag,
        kind,
    }
}

/// Offset of the title in absolute position `left: calc(100% + Npx)` (frieze era too short: name
/// to the right).
fn outside_offset(tcs: &Computed) -> Option<f64> {
    if tcs.get("position") != Some("absolute") {
        return None;
    }
    let rest = tcs.get("left")?.trim().strip_prefix("calc(100% +")?;
    skemo_lang::style::parse_length(rest.trim_end_matches(')').trim(), tcs.font_size(), None)
}

/// Text cut by "…" (`text-overflow: ellipsis`, a single line).
fn ellipsize(tcs: &Computed, lines: &mut [String], avail: f64) {
    for line in lines {
        if tw(tcs, line) <= avail + 0.01 {
            continue;
        }
        let mut chars: Vec<char> = line.chars().collect();
        while !chars.is_empty()
            && tw(tcs, &format!("{}…", chars.iter().collect::<String>())) > avail + 0.01
        {
            chars.pop();
        }
        *line = format!("{}…", chars.iter().collect::<String>().trim_end());
    }
}

/// Inline chip in the title (card without header): `(element, style, text)`.
type InlineBadge<'a> = (usize, &'a Computed, &'a str);

/// Chip laid on the title's baseline `bl`, from `x`.
fn inline_badge(b: InlineBadge, x: f64, bl: f64) -> Pill {
    let (el, bc, txt) = b;
    let bp = bc.padding();
    let inner_w = tw(bc, txt);
    let blh = line_height(bc);
    // vertical-align: upward offset (e.g. 1px)
    let raise = bc
        .get("vertical-align")
        .and_then(|v| skemo_lang::style::parse_length(v, bc.font_size(), None))
        .unwrap_or(0.0);
    let (asc, desc) = text_style(bc).metrics();
    let half = (blh - (asc + desc)) / 2.0;
    let bbl = bl - raise;
    let top = bbl - asc - half - bp[0];
    Pill {
        x: x + bc.margin()[3],
        y: top,
        w: inner_w + bp[1] + bp[3],
        h: blh + bp[0] + bp[2],
        el,
        run: Run {
            text: transform(bc, txt),
            x: x + bc.margin()[3] + bp[3],
            baseline: bbl,
            width: inner_w,
            el,
        },
    }
}

/// Title (lines, possible inline chip); also returns the y under the title.
fn card_title(
    scene: &Scene,
    card: &CardEl,
    cb: &CardBox,
    (w, h): (f64, f64),
    (y, deco): (f64, &mut DecoGeom),
) -> (Vec<Run>, Option<Pill>, f64) {
    if card.band() {
        return (Vec::new(), None, y);
    }
    let cs = scene.style(card.el);
    let tcs = scene.style(card.title);
    let tp = tcs.padding();
    let tavail = cb.avail - tp[1] - tp[3];
    let eyebrow = crate::measure::block_badge(scene, card);
    let inline = match (&card.header, &card.badge) {
        (None, Some((b, txt))) if eyebrow.is_none() => Some((*b, scene.style(*b), txt.as_str())),
        _ => None,
    };
    let badge_w = inline.map_or(0.0, |(_, bc, txt)| {
        tw(bc, txt) + bc.padding()[1] + bc.padding()[3] + bc.margin()[1] + bc.margin()[3]
    });
    // icon before the first line (if it is not in the band), mark after the last
    let lead = if card.header.is_some() {
        0.0
    } else {
        scene.icon_lead(&card.decor).0
    };
    let (mark_w, _) = scene.mark_trail(&card.decor);
    let mut lines = break_lines(tcs, &card.title_text, tavail, badge_w + lead);
    let lh = line_height(tcs);
    let outside = outside_offset(tcs);
    if outside.is_none() && tcs.get("text-overflow") == Some("ellipsis") {
        ellipsize(tcs, &mut lines, tavail);
    }
    // flex container centred vertically (era band)
    let mut ty0 = y + tp[0];
    if cs.display() == "flex" && cs.get("align-items") == Some("center") {
        let content_h = h - cb.bor[0] - cb.bor[2] - cb.pad[0] - cb.pad[2];
        let block = lines.len() as f64 * lh + tp[0] + tp[2];
        ty0 = y + (content_h - block) / 2.0 + tp[0];
    }
    let mut title = Vec::new();
    let mut badge = None;
    if let (Some((bw, bh)), Some((b, txt))) = (eyebrow, &card.badge) {
        let bc = scene.style(*b);
        let x = cb.left
            + tp[3]
            + match card.tag_side {
                1 => tavail - bw,
                -1 => 0.0,
                _ => align(tcs, tavail, bw),
            };
        let blh = line_height(bc);
        let top = ty0 + bc.margin()[0] + bc.padding()[0];
        badge = Some(inline_badge((*b, bc, txt), x, baseline(bc, top, blh)));
        ty0 += bh;
    }
    for (k, line) in lines.iter().enumerate() {
        let bl = baseline(tcs, ty0 + k as f64 * lh, lh);
        let last = k + 1 == lines.len();
        // `.tag-right` (D-050): the inline badge at the right end of the first line
        let right_badge = card.tag_side > 0;
        let lw = tw(tcs, line)
            + if k == 0 {
                lead + if right_badge { 0.0 } else { badge_w }
            } else {
                0.0
            }
            + if last { mark_w } else { 0.0 };
        let mut x = match outside {
            Some(off) => w - cb.bor[1] + off,
            None => cb.left + tp[3] + align(tcs, tavail, lw),
        };
        let line_top = ty0 + k as f64 * lh;
        if k == 0 && card.header.is_none() {
            let d = Decor {
                mark: None,
                ..card.decor
            };
            deco.icon = place_decor(scene, &d, (x, x), (line_top, lh)).icon;
            x += lead;
        }
        if k == 0
            && let Some(b) = inline
        {
            if right_badge {
                badge = Some(inline_badge(b, cb.left + tp[3] + tavail - badge_w, bl));
            } else {
                badge = Some(inline_badge(b, x, bl));
                x += badge_w;
            }
        }
        if last {
            let d = Decor {
                icon: None,
                ..card.decor
            };
            deco.mark = place_decor(scene, &d, (x, x + tw(tcs, line)), (line_top, lh)).mark;
        }
        title.push(Run {
            text: line.clone(),
            x,
            baseline: bl,
            width: tw(tcs, line),
            el: card.title,
        });
    }
    let bottom = ty0 + lines.len() as f64 * lh + tp[2];
    (title, badge, bottom)
}

/// Lines of the card (`ul > li`), under the title that ends at `y`: text lines and table rows.
fn card_lis(scene: &Scene, card: &CardEl, cb: &CardBox, y: f64) -> Vec<LiGeom> {
    let ul_hidden = card.ul.is_some_and(|u| scene.style(u).display() == "none");
    let Some(u) = card.ul.filter(|_| !ul_hidden) else {
        return Vec::new();
    };
    let uc = scene.style(u);
    let up = uc.padding();
    let ubor = border(uc);
    let lavail = cb.avail - up[1] - up[3];
    let mut top_margin = uc.margin()[0];
    let mut first = true;
    let mut cur = 0.0;
    // offset of each line (a table's rows after its first have none: the table holds them)
    let mut offs: Vec<Option<f64>> = Vec::with_capacity(card.lis.len());
    // lines that share a place (`scene::slots`) are drawn at the same offset
    let mut slot: Option<(usize, f64, f64)> = None; // (place, offset, height)
    for (k, li) in card.lis.iter().enumerate() {
        let Some((lh, mt)) = crate::measure::line_box(scene, card, k, lavail) else {
            offs.push(None);
            continue;
        };
        if li.table.is_none()
            && let Some((s, off, h)) = slot.as_mut()
            && *s == li.slot
        {
            *h = h.max(lh);
            offs.push(Some(*off));
            continue;
        }
        cur += slot.map_or(0.0, |(_, _, h)| h);
        if first && up[0] == 0.0 && ubor[0] == 0.0 {
            top_margin = top_margin.max(mt);
        } else {
            cur += mt;
        }
        first = false;
        let s = if li.table.is_some() {
            usize::MAX
        } else {
            li.slot
        };
        slot = Some((s, cur, lh));
        offs.push(Some(cur));
    }
    let title_margin = scene.style(card.title).margin()[2];
    let ul_top = y + top_margin.max(title_margin) + up[0];
    let left = cb.left + up[3];
    let mut tables: Vec<Option<(f64, f64, crate::lines::TableGeom)>> =
        vec![None; card.tables.len()];
    let mut out = Vec::with_capacity(card.lis.len());
    for (k, li) in card.lis.iter().enumerate() {
        let Some(t) = li.table else {
            let top = ul_top + offs[k].unwrap_or(0.0);
            out.push(line_geom(scene, li, (left, lavail), top));
            continue;
        };
        if let Some(off) = offs[k] {
            let hc = scene.style(card.tables[t].host);
            let (hp, hb) = (hc.padding(), border(hc));
            let g = crate::lines::table(scene, card, t);
            tables[t] = Some((left + hp[3] + hb[3], ul_top + off + hp[0] + hb[0], g));
        }
        out.push(
            tables[t]
                .as_ref()
                .map(|tg| row_geom(scene, card, k, tg))
                .unwrap_or_default(),
        );
    }
    out
}

/// A text line at `top`: its state mark, its box (`li.pill`), its text (address, pieces).
fn line_geom(
    scene: &Scene,
    li: &crate::scene::LiEl,
    (left, lavail): (f64, f64),
    top: f64,
) -> LiGeom {
    let lc = scene.style(li.el);
    let lp = lc.padding();
    let llh = line_height(lc);
    let (lead, isize) = crate::lines::icon_lead(scene, li);
    let mono_w = li_width_mono(scene, li);
    let room = lavail - lp[1] - lp[3] - lead;
    let mut g = LiGeom {
        y: top,
        icon: li.icon.map(|(el, icon)| IconBox {
            x: left + lp[3],
            y: top + lp[0] + (llh - isize) / 2.0,
            size: isize,
            el,
            icon,
            is_mark: false,
        }),
        sample: crate::lines::sample_box(scene, li)
            .zip(li.sample)
            .map(|((w, h, _), s)| ((left + lp[3], top + lp[0] + llh / 2.0, w, h), s)),
        ..LiGeom::default()
    };
    if !li.parts.is_empty() {
        let rows = crate::lines::break_parts(scene, li.el, &li.parts, room);
        g.h = rows.len() as f64 * llh + lp[0] + lp[2];
        for (r, row) in rows.iter().enumerate() {
            let row_top = top + lp[0] + r as f64 * llh;
            let used: f64 = row.iter().map(|s| s.width).sum();
            let mut x = left + lp[3] + lead + align(lc, room, used);
            for s in row {
                let sc = scene.style(s.el);
                if s.el != li.el && crate::svg::has_fill(sc) {
                    g.fills.push(Fill {
                        x,
                        y: row_top + 1.0,
                        w: s.width,
                        h: llh - 2.0,
                        el: s.el,
                    });
                }
                g.runs.push(Run {
                    text: transform(sc, &s.text),
                    x: x + s.inset,
                    baseline: baseline(lc, row_top, llh),
                    width: tw(sc, &s.text),
                    el: s.el,
                });
                x += s.width;
            }
        }
        return g;
    }
    let lines = break_lines(lc, &li.rest, room - mono_w, 0.0);
    let text_h = lines.len() as f64 * llh;
    g.h = text_h + lp[0] + lp[2];
    // a line with a background or a border (`li.pill`) is drawn as a box around its text
    let boxed = (crate::svg::has_box(lc) || lp.iter().any(|p| *p > 0.0)).then(|| {
        let inner = lines
            .iter()
            .enumerate()
            .map(|(k, l)| tw(lc, l) + if k == 0 { mono_w } else { 0.0 })
            .fold(0.0, f64::max);
        let bw = inner + lp[1] + lp[3];
        let bx = left + align(lc, lavail, bw);
        (bx, top, bw, text_h + lp[0] + lp[2])
    });
    g.runs = match boxed {
        Some((bx, by, bw, _)) => li_runs(
            scene,
            li,
            (bx + lp[3] + lead, bw - lp[1] - lp[3]),
            by + lp[0],
            &lines,
            (llh, mono_w),
        ),
        None => li_runs(scene, li, (left + lead, room), top, &lines, (llh, mono_w)),
    };
    g.boxed = boxed.filter(|_| crate::svg::has_box(lc));
    g
}

/// One row of a table placed at `(x0, y0)`: its cells' fills, borders and text.
fn row_geom(
    scene: &Scene,
    card: &CardEl,
    k: usize,
    (x0, y0, tg): &(f64, f64, crate::lines::TableGeom),
) -> LiGeom {
    let li = &card.lis[k];
    let (_, ry, rh) = tg
        .rows
        .iter()
        .find(|r| r.0 == k)
        .copied()
        .unwrap_or((k, 0.0, 0.0));
    let mut g = LiGeom {
        y: y0 + ry,
        h: rh,
        ..LiGeom::default()
    };
    let align_of = |col: usize| {
        li.table
            .and_then(|t| card.tables[t].align.get(col).copied())
            .unwrap_or_default()
    };
    for b in tg.cells.iter().filter(|b| b.li == k) {
        let c = &li.cells[b.cell];
        let cs = scene.style(c.el);
        let (p, bor) = (cs.padding(), border(cs));
        let (x, y) = (x0 + b.x, y0 + b.y);
        if crate::svg::has_fill(cs) {
            g.fills.push(Fill {
                x,
                y,
                w: b.w,
                h: b.h,
                el: c.el,
            });
        }
        let sides = [
            (x, y, x + b.w, y),
            (x + b.w, y, x + b.w, y + b.h),
            (x, y + b.h, x + b.w, y + b.h),
            (x, y, x, y + b.h),
        ];
        for (side, (x1, y1, x2, y2)) in sides.into_iter().enumerate() {
            if bor[side] > 0.0 {
                g.rules.push(Rule {
                    x1,
                    y1,
                    x2,
                    y2,
                    el: c.el,
                    side,
                });
            }
        }
        let used = crate::lines::parts_width(scene, c.el, &c.parts);
        let inner = b.w - p[1] - p[3] - bor[1] - bor[3];
        let shift = match align_of(c.col) {
            skemo_lang::Align::Left => 0.0,
            skemo_lang::Align::Center => (inner - used) / 2.0,
            skemo_lang::Align::Right => inner - used,
        };
        let lh = line_height(cs);
        let top = y + (b.h - lh) / 2.0;
        let mut tx = x + bor[3] + p[3] + shift;
        for (text, el) in &c.parts {
            let s = crate::lines::seg(scene, c.el, *el, text);
            let sc = scene.style(*el);
            if *el != c.el && crate::svg::has_fill(sc) {
                g.fills.push(Fill {
                    x: tx,
                    y: top + 1.0,
                    w: s.width,
                    h: lh - 2.0,
                    el: *el,
                });
            }
            g.runs.push(Run {
                text: transform(sc, text),
                x: tx + s.inset,
                baseline: baseline(cs, top, lh),
                width: tw(sc, text),
                el: *el,
            });
            tx += s.width;
        }
    }
    g
}

/// Pieces of text of a list line: IP in mono font on the first, then the text.
fn li_runs(
    scene: &Scene,
    li: &crate::scene::LiEl,
    (x0, lavail): (f64, f64),
    top: f64,
    lines: &[String],
    (llh, mono_w): (f64, f64),
) -> Vec<Run> {
    let lc = scene.style(li.el);
    // the space between the address and the text is dropped by line breaking: keep its width
    let gap = if li.mono.is_some() && li.rest.starts_with(' ') {
        tw(lc, " ")
    } else {
        0.0
    };
    let mut runs = Vec::new();
    for (k, line) in lines.iter().enumerate() {
        let bl = baseline(lc, top + k as f64 * llh, llh);
        let used = tw(lc, line) + if k == 0 { mono_w + gap } else { 0.0 };
        let mut x = x0 + align(lc, lavail, used);
        if k == 0
            && let Some((m, ip)) = &li.mono
        {
            let mc = scene.style(*m);
            runs.push(Run {
                text: ip.clone(),
                x: x + mc.margin()[3],
                baseline: bl,
                width: tw(mc, ip),
                el: *m,
            });
            x += mono_w + gap;
        }
        if !line.is_empty() {
            runs.push(Run {
                text: line.clone(),
                x,
                baseline: bl,
                width: tw(lc, line),
                el: li.el,
            });
        }
    }
    runs
}

/// Title of a zone (positioned absolutely by the theme: `left`, `top` or `bottom`, centred if
/// `text-align: center` with `left: 0; right: 0`).
pub fn zone_title(
    scene: &Scene,
    z: &ZoneEl,
    (text, tag): (&str, Option<&str>),
    w: f64,
    h: f64,
) -> ZoneTitle {
    let title_el = z.title;
    let c = scene.style(title_el);
    let lh = line_height(c);
    let (lead, _) = scene.icon_lead(&z.decor);
    let (trail, _) = scene.mark_trail(&z.decor);
    let text_w = tw(c, text);
    // the pill of `tag=` follows the title (D-050), its left margin from the theme
    let pill = z.tag.zip(tag).map(|(el, t)| {
        let gap = scene.style(el).margin()[3];
        (label(scene, el, t, 0.0, 0.0), gap)
    });
    // `.tag-right`: the pill at the end of the row, out of the title's alignment
    let pill_w = pill
        .as_ref()
        .filter(|_| !z.tag_right)
        .map_or(0.0, |(p, gap)| p.w + gap);
    // the icon and the mark are part of the line for the alignment
    let width = lead + text_w + trail + pill_w;
    let px = |n: &str| {
        c.get(n)
            .and_then(|v| skemo_lang::style::parse_length(v, c.font_size(), None))
    };
    let (l, r) = (px("left"), px("right"));
    let x = match (l, r, c.get("text-align")) {
        (Some(l), Some(r), Some("center")) => l + (w - l - r - width) / 2.0,
        (Some(l), _, _) => l,
        (None, Some(r), _) => w - r - width,
        _ => 0.0,
    };
    let top = match (px("top"), px("bottom")) {
        (Some(t), _) => t,
        (None, Some(b)) => h - b - lh,
        _ => 0.0,
    };
    let run = Run {
        text: transform(c, text),
        x: x + lead,
        baseline: baseline(c, top, lh),
        width: text_w,
        el: title_el,
    };
    let deco = place_decor(scene, &z.decor, (x, run.x + text_w), (top, lh));
    let tag = pill.map(|(p, gap)| {
        let cx = if z.tag_right {
            w - r.or(l).unwrap_or(16.0) - p.w / 2.0
        } else {
            x + width - pill_w + gap + p.w / 2.0
        };
        moved(p, (cx, top + lh / 2.0))
    });
    let mut sy = top + lh;
    let subs = z
        .subs
        .iter()
        .map(|(el, t)| {
            let sc = scene.style(*el);
            let slh = line_height(sc);
            sy += sc.margin()[0];
            let r = Run {
                text: transform(sc, t),
                x,
                baseline: baseline(sc, sy, slh),
                width: tw(sc, t),
                el: *el,
            };
            sy += slh + sc.margin()[2];
            r
        })
        .collect();
    ZoneTitle {
        run,
        deco,
        span: (x, x + width),
        tag,
        subs,
    }
}

/// A pill built around `(0, 0)`, moved to be centred on `(cx, cy)`.
fn moved(mut p: Pill, (cx, cy): (f64, f64)) -> Pill {
    let (dx, dy) = (cx - (p.x + p.w / 2.0), cy - (p.y + p.h / 2.0));
    p.x += dx;
    p.y += dy;
    p.run.x += dx;
    p.run.baseline += dy;
    p
}

/// Zone title: text, icon and mark, and horizontal extent of the whole.
#[derive(Debug, Clone)]
pub struct ZoneTitle {
    pub run: Run,
    pub deco: DecoGeom,
    pub span: (f64, f64),
    /// The pill of `tag=` (D-050), in the zone's frame.
    pub tag: Option<Pill>,
    /// Description lines under the title (v0.8), in the zone's frame.
    pub subs: Vec<Run>,
}

/// The `×N` pill of a pile (`cards`): its right edge on the pile's right edge, centred on the
/// top card's bottom edge (`mid_y`), the same place in every look.
pub fn count_pill(scene: &Scene, el: usize, text: &str, right: f64, mid_y: f64) -> Pill {
    let p = label(scene, el, text, 0.0, 0.0);
    let w = p.w;
    moved(p, (right - w / 2.0, mid_y))
}

/// Arrow label centred at `(cx, cy)`: box and text (absolute frame).
pub fn label(scene: &Scene, el: usize, text: &str, cx: f64, cy: f64) -> Pill {
    let c = scene.style(el);
    let p = c.padding();
    let width = tw(c, text);
    let lh = line_height(c);
    let w = width + p[1] + p[3];
    let h = lh + p[0] + p[2];
    let x = cx - w / 2.0;
    let y = cy - h / 2.0;
    Pill {
        x,
        y,
        w,
        h,
        el,
        run: Run {
            text: text.to_string(),
            x: x + p[3],
            baseline: baseline(c, y + p[0], lh),
            width,
            el,
        },
    }
}

/// The diagram's title (D-050) laid out in a box `w` wide: its text on the left, its pills on the
/// right of the same line, in the box's frame.
#[derive(Debug, Clone)]
pub struct TitleGeom {
    pub run: Run,
    pub pills: Vec<Pill>,
    /// Subtitle lines under the first row (v0.8).
    pub subs: Vec<Run>,
}

/// Smallest width and height of the diagram's title: the text, then its pills with their left
/// margins (from the theme), then its subtitle lines (v0.8).
pub fn title_size(scene: &Scene, el: &crate::scene::TitleEl, t: &skemo_lang::Title) -> (f64, f64) {
    let (w, row) = title_row(scene, el, t);
    let mut w = w;
    let mut h = row;
    for (s, text) in el.subs.iter().zip(&t.sub) {
        let c = scene.style(*s);
        let text = skemo_lang::inline::plain(text);
        w = w.max(tw(c, &text));
        h += line_height(c) + c.margin()[0] + c.margin()[2];
    }
    (w.ceil(), h.ceil())
}

/// Width and height of the title's first row (its text and its pills).
fn title_row(scene: &Scene, el: &crate::scene::TitleEl, t: &skemo_lang::Title) -> (f64, f64) {
    let c = scene.style(el.text);
    let mut w = tw(c, &t.text);
    let mut h = line_height(c);
    for (li, text) in el.lines.iter().zip(&t.lines) {
        let p = label(scene, *li, text, 0.0, 0.0);
        w += scene.style(*li).margin()[3] + p.w;
        h = h.max(p.h);
    }
    (w, h.ceil())
}

/// Title laid out in a box `w` wide and `h` high (`title_size`), at the origin.
pub fn title_layout(
    scene: &Scene,
    el: &crate::scene::TitleEl,
    t: &skemo_lang::Title,
    (w, h): (f64, f64),
) -> TitleGeom {
    let c = scene.style(el.text);
    let lh = line_height(c);
    // the first row (text and pills) is as tall as `title_row`; subtitle lines go below it
    let has_subs = !t.sub.is_empty() && !el.subs.is_empty();
    let row = if has_subs {
        title_row(scene, el, t).1
    } else {
        h
    };
    let h = row;
    let run = Run {
        text: transform(c, &t.text),
        x: 0.0,
        baseline: baseline(c, (h - lh) / 2.0, lh),
        width: tw(c, &t.text),
        el: el.text,
    };
    let mut sy = row;
    let subs = el
        .subs
        .iter()
        .zip(&t.sub)
        .map(|(s, text)| {
            let sc = scene.style(*s);
            let text = skemo_lang::inline::plain(text);
            let slh = line_height(sc);
            sy += sc.margin()[0];
            let r = Run {
                text: transform(sc, &text),
                x: 0.0,
                baseline: baseline(sc, sy, slh),
                width: tw(sc, &text),
                el: *s,
            };
            sy += slh + sc.margin()[2];
            r
        })
        .collect();
    // pills from the right edge, in writing order
    let mut pills: Vec<Pill> = el
        .lines
        .iter()
        .zip(&t.lines)
        .map(|(li, text)| label(scene, *li, text, 0.0, 0.0))
        .collect();
    let mut right = w;
    for (p, li) in pills.iter_mut().zip(&el.lines).rev() {
        let at = (right - p.w / 2.0, h / 2.0);
        *p = moved(p.clone(), at);
        right -= p.w + scene.style(*li).margin()[3];
    }
    TitleGeom { run, pills, subs }
}
