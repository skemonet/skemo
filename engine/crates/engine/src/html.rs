//! Standalone HTML (F-RENDER-1, F-ANIM-4, E-STYLE-6): theme and custom CSS, fonts as
//! `@font-face` (data URI), DOM positioned with imposed sizes, arrows in SVG, step player.
//! No embedded engine: the browser only has to display.
//!
//! The DOM is **flat** under `.stage` (as in the prototype: the z-index layers of the cards, zones
//! and arrows require it). The inheritance of steps is therefore written explicitly: each element
//! carries its **effective** values.

use crate::arrow::{arrow, edge_classes, triangle_path};
use crate::diagram::Diagram;
use crate::fonts::{FILES, select, weight_range};
use crate::measure::text_style;
use crate::svg::{esc, n};
use std::collections::BTreeSet;
use std::fmt::Write as _;

/// Options of the HTML rendering.
#[derive(Debug, Clone)]
pub struct HtmlOptions {
    /// Title of the page.
    pub title: String,
    /// Embed the fonts used (otherwise: fall back on the system's fonts).
    pub embed_fonts: bool,
}

impl Default for HtmlOptions {
    fn default() -> Self {
        HtmlOptions {
            title: "skemo".into(),
            embed_fonts: true,
        }
    }
}

const PLAYER_CSS: &str = include_str!("../assets/player.css");
const PLAYER_JS: &str = include_str!("../assets/player.js");

pub(crate) fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut o = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let b = [c[0], *c.get(1).unwrap_or(&0), *c.get(2).unwrap_or(&0)];
        let v = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        o.push(T[(v >> 18) as usize & 63] as char);
        o.push(T[(v >> 12) as usize & 63] as char);
        o.push(if c.len() > 1 {
            T[(v >> 6) as usize & 63] as char
        } else {
            '='
        });
        o.push(if c.len() > 2 {
            T[v as usize & 63] as char
        } else {
            '='
        });
    }
    o
}

/// Font files used by the elements that carry text.
fn used_fonts(d: &Diagram) -> BTreeSet<&'static str> {
    let sc = &d.scene;
    let mut els = Vec::new();
    for c in sc.cards.values() {
        els.push(c.title);
        if let Some((b, _)) = c.badge {
            els.push(b);
        }
        if let Some(h) = &c.header {
            els.push(h.type_el);
            if let Some((t, _)) = h.tag {
                els.push(t);
            }
        }
        if let Some(h) = &c.header
            && let Some((k, _)) = h.kind
        {
            els.push(k);
        }
        if let Some((k, _)) = c.kind {
            els.push(k);
        }
        for li in &c.lis {
            els.push(li.el);
            if let Some((m, _)) = li.mono {
                els.push(m);
            }
            // emphasis and table cells (v0.8)
            els.extend(li.parts.iter().map(|(_, e)| *e));
            for cell in &li.cells {
                els.push(cell.el);
                els.extend(cell.parts.iter().map(|(_, e)| *e));
            }
        }
    }
    els.extend(sc.zones.values().map(|z| z.title));
    els.extend(
        sc.zones
            .values()
            .flat_map(|z| z.subs.iter().map(|(e, _)| *e)),
    );
    if let Some(t) = &sc.title {
        els.push(t.text);
        els.extend(t.lines.iter().chain(&t.subs).copied());
    }
    els.extend(sc.labels.values().copied());
    // decorations of the timeline and frieze modes, only when the mode draws them
    els.extend(sc.deco.iter().filter_map(|(k, &el)| {
        let drawn = (k.starts_with("tl-") && d.timeline.is_some())
            || (k.starts_with("fr-") && d.frise.is_some());
        drawn.then_some(el)
    }));
    let mut used = BTreeSet::new();
    for e in els {
        let st = text_style(sc.style(e));
        used.insert(select(&st.families, st.weight, st.italic).file.file);
    }
    used
}

pub(crate) fn font_faces(d: &Diagram) -> String {
    let used = used_fonts(d);
    let mut css = String::new();
    let registered = crate::fonts::registered();
    let files = FILES
        .iter()
        .chain(registered.iter().map(|f| f.file))
        .filter(|f| used.contains(f.file));
    for f in files {
        let (lo, hi) = weight_range(f);
        let otf = f.file.ends_with(".otf") || f.data.starts_with(b"OTTO");
        let fmt = if otf { "opentype" } else { "truetype" };
        let mime = if otf { "font/otf" } else { "font/ttf" };
        let _ = writeln!(
            css,
            "@font-face{{font-family:'{}';font-style:{};font-weight:{lo} {hi};font-display:block;src:url(data:{mime};base64,{}) format('{fmt}')}}",
            f.family,
            if f.italic { "italic" } else { "normal" },
            base64(f.data)
        );
    }
    css
}

/// **Effective** animation attributes (the HTML DOM is flat).
fn anim_attrs(d: &Diagram, key: &str) -> String {
    let Some(r) = d.anim.elements.get(key) else {
        return String::new();
    };
    let mut s = String::new();
    if r.appear.value > 0.0 {
        let _ = write!(
            s,
            " data-step=\"{}\" data-fx=\"{}\"",
            esc(&r.appear.raw),
            esc(&r.fx)
        );
    }
    if let Some(u) = &r.until {
        let _ = write!(s, " data-until=\"{}\"", esc(&u.raw));
    }
    if !r.focus.is_empty() {
        let f: Vec<String> = r
            .focus
            .iter()
            .map(|(a, b)| {
                b.as_ref()
                    .map_or(a.raw.clone(), |b| format!("{}-{}", a.raw, b.raw))
            })
            .collect();
        let _ = write!(s, " data-focus=\"{}\"", esc(&f.join(" ")));
    }
    if r.is_link && !s.is_empty() {
        let _ = write!(
            s,
            " data-from=\"{}\" data-to=\"{}\"",
            esc(r.from.as_deref().unwrap_or("")),
            esc(r.to.as_deref().unwrap_or(""))
        );
    }
    s
}

fn pos(x: f64, y: f64, w: f64, h: f64) -> String {
    format!(
        "left:{}px;top:{}px;width:{}px;height:{}px",
        n(x),
        n(y),
        n(w),
        n(h)
    )
}

/// Timeline decorations as positioned `div`s (as the prototype's `drawTimeline`).
fn timeline_divs(d: &Diagram, body: &mut String) {
    let Some(tl) = &d.timeline else { return };
    let (ox, oy) = (d.frame.ox, d.frame.oy);
    let _ = write!(
        body,
        "<div class=\"tl-axis\" style=\"left:{}px;top:{}px;width:{}px;height:{}px\"></div>",
        n(ox - 22.0),
        n(oy),
        n(tl.width + 104.0),
        n(tl.axis_h)
    );
    for t in &tl.ticks {
        let m = if t.major { " major" } else { "" };
        let _ = write!(
            body,
            "<div class=\"tl-grid{m}\" style=\"left:{}px;top:{}px;height:{}px\"></div>",
            n(t.x + ox),
            n(oy + tl.axis_h - 8.0),
            n(tl.bottom - tl.axis_h + 8.0)
        );
        let _ = write!(
            body,
            "<div class=\"tl-tick{m}\" style=\"left:{}px;top:{}px\">{}</div>",
            n(t.x + ox),
            n(oy + 8.0),
            esc(&t.label)
        );
    }
    for (id, x) in &tl.marks {
        let _ = write!(
            body,
            "<div class=\"tl-mark\" data-for=\"{}\"{} style=\"left:{}px;top:{}px\"></div>",
            esc(id),
            anim_attrs(d, id),
            n(x + ox),
            n(oy + tl.axis_h - 6.0)
        );
    }
    if let Some((x, label)) = &tl.today {
        let _ = write!(
            body,
            "<div class=\"tl-today\" style=\"left:{}px;top:{}px;height:{}px\"></div>",
            n(x + ox),
            n(oy + tl.axis_h - 8.0),
            n(tl.bottom - tl.axis_h + 8.0)
        );
        let _ = write!(
            body,
            "<div class=\"tl-today-label\" style=\"left:{}px;top:{}px\">{}</div>",
            n(x + ox),
            n(oy + tl.axis_h - 4.0),
            esc(label)
        );
    }
}

/// Frieze decorations as positioned `div`s (as the prototype's `drawFrise`).
fn frise_divs(d: &Diagram, body: &mut String) {
    let Some(fr) = &d.frise else { return };
    let (ox, oy) = (d.frame.ox, d.frame.oy);
    let _ = write!(
        body,
        "<div class=\"fr-axis\" style=\"left:{}px;top:{}px;width:{}px\"></div>",
        n(ox - 10.0),
        n(oy - 2.0),
        n(fr.width + 20.0)
    );
    for (x, label) in &fr.ticks {
        let _ = write!(
            body,
            "<div class=\"fr-tick\" style=\"left:{}px;top:{}px\"></div>",
            n(x + ox),
            n(oy)
        );
        let _ = write!(
            body,
            "<div class=\"fr-tick-label\" style=\"left:{}px;top:{}px\">{}</div>",
            n(x + ox),
            n(oy + crate::frise::TICK_LABEL_TOP),
            esc(label)
        );
    }
    for (x, label) in &fr.centuries {
        let _ = write!(
            body,
            "<div class=\"fr-century\" style=\"left:{}px;top:{}px\">{}</div>",
            n(x + ox),
            n(oy - 22.0),
            esc(label)
        );
    }
    for st in &fr.stems {
        let a = anim_attrs(d, &st.id);
        let _ = write!(
            body,
            "<div class=\"fr-stem\" data-for=\"{}\"{a} style=\"left:{}px;top:{}px;height:{}px\"></div>",
            esc(&st.id),
            n(st.x + ox),
            n(st.y1.min(st.y2) + oy),
            n((st.y2 - st.y1).abs())
        );
        let _ = write!(
            body,
            "<div class=\"fr-dot\" data-for=\"{}\"{a} style=\"left:{}px;top:{}px\"></div>",
            esc(&st.id),
            n(st.x + ox),
            n(oy)
        );
    }
}

/// Rendu HTML autonome.
pub fn render(d: &Diagram, opts: &HtmlOptions) -> String {
    let (w, h) = (d.frame.w.ceil(), d.frame.h.ceil());
    let mut body = String::new();
    stage_open(d, &mut body);
    for node in &d.model.nodes {
        let Some(b) = d.boxes.get(&node.id) else {
            continue;
        };
        let style = pos(b.x + d.frame.ox, b.y + d.frame.oy, b.w, b.h);
        match node.kind {
            // a stack draws nothing (D-050): its children are placed on their own
            skemo_lang::Kind::Zone if node.stack => {}
            skemo_lang::Kind::Zone => zone_html(d, node, &style, &mut body),
            skemo_lang::Kind::Card => {
                let m = crate::measure::deck_margin(node);
                let style = if m > 0.0 {
                    deck_style(d, node, (b.x + d.frame.ox, b.y + d.frame.oy, b.w, b.h), m)
                } else {
                    style
                };
                card_html(d, node, &style, &mut body);
            }
        }
    }
    title_html(d, &mut body);
    edges_svg(d, (w, h), &mut body);
    timeline_divs(d, &mut body);
    frise_divs(d, &mut body);
    edge_labels(d, &mut body);
    body.push_str("</div>");
    page(d, opts, &body)
}

/// Opens `.stage`: classes (theme, look), size and list of states.
fn stage_open(d: &Diagram, body: &mut String) {
    let mut cls = vec!["stage".to_string(), d.scene.theme.class()];
    if let Some(l) = d.model.option("layout") {
        cls.push(format!("layout-{l}"));
    }
    if let Some(l) = d.model.option("look") {
        cls.push(format!("look-{l}"));
    }
    let states: Vec<String> = d.anim.states.iter().map(|s| s.raw.clone()).collect();
    let _ = write!(
        body,
        "<div class=\"{}\" style=\"width:{}px;height:{}px\" data-states=\"{}\">",
        esc(&cls.join(" ")),
        n(d.frame.w.ceil()),
        n(d.frame.h.ceil()),
        esc(&serde_json::to_string(&states).unwrap_or_default())
    );
}

/// `<svg>` of an icon or a link mark: the size comes from `--icon-size` (CSS), the drawing is
/// Lucide's.
fn icon_svg(class: &str, icon: &crate::icons::Icon, images: &[crate::images::Image]) -> String {
    if let Some(img) = icon.image.and_then(|i| images.get(i)) {
        // an image file (D-046), in the same box: drawn as an image, never inlined
        return format!(
            "<svg class=\"{class}\" viewBox=\"0 0 24 24\" aria-hidden=\"true\"><image href=\"{}\" width=\"24\" height=\"24\" preserveAspectRatio=\"xMidYMid meet\"/></svg>",
            img.data_url()
        );
    }
    format!(
        "<svg class=\"{class}\" viewBox=\"0 0 24 24\" fill=\"none\" stroke-width=\"2\" stroke-linecap=\"round\" stroke-linejoin=\"round\" aria-hidden=\"true\">{}</svg>",
        icon.markup
    )
}

/// Attributes of a link: `target` and `rel` for an external link (FORMAT §3.2.1).
fn href_attrs(href: &str) -> String {
    let lower = href.trim_start().to_ascii_lowercase();
    let ext = lower.starts_with("http:") || lower.starts_with("https:");
    format!(
        " href=\"{}\" rel=\"noopener\"{}",
        esc(href),
        if ext { " target=\"_blank\"" } else { "" }
    )
}

/// Content of a title: icon, text, link mark (in an `<a>` for a linked zone).
fn titled(d: &Diagram, decor: &crate::scene::Decor, text: &str, href: Option<&str>) -> String {
    let mut out = String::new();
    if let Some((_, i)) = &decor.icon {
        out.push_str(&icon_svg("icon", i, &d.scene.images));
    }
    out.push_str(text);
    if let Some((_, i)) = &decor.mark {
        out.push_str(&icon_svg("link-mark", i, &d.scene.images));
    }
    match href {
        Some(h) => format!("<a class=\"href\"{}>{out}</a>", href_attrs(h)),
        None => out,
    }
}

fn zone_html(d: &Diagram, node: &skemo_lang::Node, style: &str, body: &mut String) {
    let decor = d
        .scene
        .zones
        .get(&node.id)
        .map(|z| z.decor)
        .unwrap_or_default();
    // the pill of `tag=` follows the title (D-050)
    let tag = node
        .tag
        .as_ref()
        .map(|t| format!("<span class=\"tag\">{}</span>", esc(t)))
        .unwrap_or_default();
    // description lines under the title (v0.8)
    let subs: String = node
        .li
        .iter()
        .filter(|l| l.cells.is_empty() && l.rule.is_empty())
        .map(|l| {
            let cls = std::iter::once("sub".to_string())
                .chain(l.classes.iter().cloned())
                .collect::<Vec<_>>()
                .join(" ");
            format!(
                "<div class=\"{}\">{}</div>",
                esc(&cls),
                esc(&skemo_lang::inline::plain(&l.text))
            )
        })
        .collect();
    let _ = write!(
        body,
        "<zone data-id=\"{}\" class=\"{}\" style=\"{style}\"{}><div class=\"title\">{}{tag}{subs}</div></zone>",
        esc(&node.id),
        esc(&node.classes.join(" ")),
        anim_attrs(d, &node.id),
        titled(d, &decor, &esc(&node.title), node.href.as_deref())
    );
}

/// The diagram's title (D-050), in its box: the theme lays out its text and pills.
fn title_html(d: &Diagram, body: &mut String) {
    let (Some(t), Some(b)) = (&d.model.title, d.title_box) else {
        return;
    };
    let lines: String = t
        .lines
        .iter()
        .map(|l| format!("<li class=\"pill\">{}</li>", esc(l)))
        .collect();
    let subs: String = t
        .sub
        .iter()
        .map(|l| {
            format!(
                "<div class=\"sub\">{}</div>",
                esc(&skemo_lang::inline::plain(l))
            )
        })
        .collect();
    let _ = write!(
        body,
        "<div class=\"diagram-title\" style=\"{}\"><div class=\"text\">{}</div><ul>{lines}</ul>{subs}</div>",
        pos(b.x + d.frame.ox, b.y + d.frame.oy, b.w, b.h),
        esc(&t.text)
    );
}

/// A pile (`cards`): the top card's box, and the cards behind as outlined shadows in the card's
/// own fill and border colours, nearest first.
fn deck_style(
    d: &Diagram,
    node: &skemo_lang::Node,
    (x, y, w, h): (f64, f64, f64, f64),
    m: f64,
) -> String {
    let Some(el) = d.scene.cards.get(&node.id) else {
        return pos(x, y, w, h);
    };
    let c = d.scene.style(el.el);
    let fill = c
        .color_of("background-color")
        .or_else(|| skemo_lang::style::parse_color(c.get("background")?, c.color()))
        .unwrap_or(skemo_lang::style::Rgba::WHITE);
    let line = c.border_color(1);
    let bw = crate::measure::border(c)[1].max(1.0);
    let mut layers = Vec::new();
    let mut o = crate::measure::DECK_STEP;
    while o <= m {
        layers.push(format!(
            "{o}px {o}px 0 -{bw}px {}, {o}px {o}px 0 0 {}",
            fill.to_css(),
            line.to_css()
        ));
        o += crate::measure::DECK_STEP;
    }
    format!(
        "{};--deck-m:{}px;box-shadow:{}",
        pos(x, y, w - m, h - m),
        n(m),
        layers.join(", ")
    )
}

/// Card: classes (type, mode, look), inline style (position, period variables, label), then
/// header, title and lines.
fn card_html(d: &Diagram, node: &skemo_lang::Node, style: &str, body: &mut String) {
    let Some(c) = d.scene.cards.get(&node.id) else {
        return;
    };
    let mode_extras = &d.mode_extras;
    let mut ccls = node.classes.clone();
    if let Some(c) = mode_extras.classes.get(&node.id) {
        ccls.extend(c.iter().cloned());
    }
    let mut st = style.to_string();
    if let Some(vars) = mode_extras.inline.get(&node.id) {
        // period: width imposed by the dates (pitfall P18)
        if vars.iter().any(|(k, _)| k != "--date") {
            st.push_str(";max-width:none");
        }
        for (k, v) in vars {
            let _ = write!(st, ";{k}:{v}");
        }
        // frieze event: its date is the badge (`scene::add_card`)
        if node.tag.is_none()
            && vars.iter().any(|(k, _)| k == "--date")
            && let Some((_, txt)) = &c.badge
        {
            let _ = write!(
                st,
                ";--label:{}",
                esc(&serde_json::to_string(txt).unwrap_or_default())
            );
        }
    }
    if d.scene.style(c.el).get("--c").is_none() {
        ccls.push("no-type".into());
    }
    // a table never wraps: its card is as wide as the engine measured it (v0.8)
    if !c.tables.is_empty() {
        st.push_str(";max-width:none");
    }
    if c.header.is_some() {
        ccls.push(if c.band() { "look-band" } else { "look-header" }.into());
    } else if let Some(t) = &node.tag {
        let _ = write!(
            st,
            ";--label:{}",
            esc(&serde_json::to_string(t).unwrap_or_default())
        );
    }
    if node.tag.is_none() && c.badge.is_none() && c.header.is_none() {
        ccls.push("no-tag".into());
    }
    if crate::scene::lone_title(&d.model, node) {
        ccls.push("no-body".into());
    }
    if node.deck.is_some() {
        ccls.push("deck".into());
    }
    let extra = node
        .tag
        .as_ref()
        .map(|t| format!(" data-tag=\"{}\"", esc(t)))
        .unwrap_or_default();
    let _ = write!(
        body,
        "<card data-id=\"{}\" class=\"{}\" style=\"{st}\"{extra}{}>",
        esc(&node.id),
        esc(&ccls.join(" ")),
        anim_attrs(d, &node.id)
    );
    if let Some(hd) = &c.header {
        let tag_attrs = anim_attrs(d, &format!("{}/tag", node.id));
        head_html(d, hd, &c.decor, &tag_attrs, body);
    }
    if !c.band() {
        // icon before the title when there is no band; link mark after the title
        let decor = crate::scene::Decor {
            icon: if c.header.is_some() {
                None
            } else {
                c.decor.icon
            },
            mark: c.decor.mark,
        };
        if let Some((_, k)) = &c.kind {
            let _ = write!(body, "<div class=\"kind\">{}</div>", esc(k));
        }
        let _ = write!(
            body,
            "<div class=\"title\">{}</div>",
            titled(d, &decor, &esc(&node.title).replace('\n', "<br>"), None)
        );
    }
    lis_html(d, node, c, body);
    if let Some((_, text)) = &c.count {
        let _ = write!(body, "<span class=\"count\">{}</span>", esc(text));
    }
    if let Some(h) = &node.href {
        // link on the whole card: an anchor laid on top, without changing the layout
        let _ = write!(body, "<a class=\"href cover\"{}></a>", href_attrs(h));
    }
    body.push_str("</card>");
}

fn head_html(
    d: &Diagram,
    hd: &crate::scene::HeadEl,
    decor: &crate::scene::Decor,
    tag_attrs: &str,
    body: &mut String,
) {
    let head_decor = crate::scene::Decor {
        icon: decor.icon,
        mark: if hd.band { decor.mark } else { None },
    };
    let rich = head_decor.icon.is_some() || head_decor.mark.is_some();
    let kind = hd
        .kind
        .as_ref()
        .map(|(_, k)| format!("<span class=\"kind\">{}</span>", esc(k)))
        .unwrap_or_default();
    let _ = write!(
        body,
        "<div class=\"head\">{kind}<span class=\"type\"{}>{}</span>",
        if rich {
            " style=\"display:inline-flex;align-items:center\""
        } else {
            ""
        },
        titled(d, &head_decor, &esc(&hd.type_text), None)
    );
    if let Some((_, t)) = &hd.tag {
        let _ = write!(body, "<span class=\"tag\"{tag_attrs}>{}</span>", esc(t));
    }
    body.push_str("</div>");
}

fn lis_html(d: &Diagram, node: &skemo_lang::Node, c: &crate::scene::CardEl, body: &mut String) {
    if c.lis.is_empty() {
        return;
    }
    body.push_str("<ul>");
    let mut k = 0;
    while k < c.lis.len() {
        if let Some(t) = c.lis[k].table {
            let n = c.lis[k..].iter().take_while(|l| l.table == Some(t)).count();
            table_html(d, node, c, (t, k, n), body);
            k += n;
            continue;
        }
        let slot = c.lis[k].slot;
        let n = c.lis[k..]
            .iter()
            .take_while(|l| l.slot == slot && l.table.is_none())
            .count()
            .max(1);
        if n == 1 {
            let attrs = anim_attrs(d, &format!("{}/li[{k}]", node.id));
            let li = &c.lis[k];
            let cls = if li.classes.is_empty() {
                String::new()
            } else {
                format!(" class=\"{}\"", esc(&li.classes.join(" ")))
            };
            let _ = write!(body, "<li{cls}{attrs}>");
            li_text(d, li, body);
            body.push_str("</li>");
        } else {
            // lines never visible together share one place (`scene::slots`): stacked in a
            // one-cell grid, each animated on its own
            body.push_str("<li style=\"display:grid\">");
            for (j, li) in c.lis[k..k + n].iter().enumerate() {
                let attrs = anim_attrs(d, &format!("{}/li[{}]", node.id, k + j));
                let _ = write!(body, "<span style=\"grid-area:1/1\"{attrs}>");
                li_text(d, li, body);
                body.push_str("</span>");
            }
            body.push_str("</li>");
        }
        k += n;
    }
    body.push_str("</ul>");
}

/// A table (FORMAT §3.2.5): `li.table > table`, one `tr` per row (animated on its own), its
/// cells with their spans, classes and column (`data-col`) and alignment.
fn table_html(
    d: &Diagram,
    node: &skemo_lang::Node,
    c: &crate::scene::CardEl,
    (t, first, n): (usize, usize, usize),
    body: &mut String,
) {
    let align = c
        .tables
        .get(t)
        .map(|te| te.align.clone())
        .unwrap_or_default();
    body.push_str("<li class=\"table\"><table>");
    let mut header = true;
    for k in first..first + n {
        let li = &c.lis[k];
        if li.rule {
            continue;
        }
        let tag = if header
            && c.lis
                .get(k + 1)
                .is_some_and(|l| l.rule && l.table == Some(t))
        {
            "th"
        } else {
            "td"
        };
        header = false;
        let attrs = anim_attrs(d, &format!("{}/li[{k}]", node.id));
        let _ = write!(body, "<tr{attrs}>");
        for cell in &li.cells {
            let mut a = format!(" data-col=\"{}\"", cell.col);
            if !cell.classes.is_empty() {
                let _ = write!(a, " class=\"{}\"", esc(&cell.classes.join(" ")));
            }
            if cell.colspan > 1 {
                let _ = write!(a, " colspan=\"{}\"", cell.colspan);
            }
            if cell.rowspan > 1 {
                let _ = write!(a, " rowspan=\"{}\"", cell.rowspan);
            }
            match align.get(cell.col) {
                Some(skemo_lang::Align::Center) => a.push_str(" style=\"text-align:center\""),
                Some(skemo_lang::Align::Right) => a.push_str(" style=\"text-align:right\""),
                _ => {}
            }
            let _ = write!(body, "<{tag}{a}>{}</{tag}>", runs_html(&cell.text));
        }
        body.push_str("</tr>");
    }
    body.push_str("</table></li>");
}

/// Text with emphasis as HTML: `mark`, `strong`, `em`, `code` (FORMAT §3.2.4).
fn runs_html(text: &str) -> String {
    let mut out = String::new();
    for r in skemo_lang::inline::runs(text) {
        let mut open = String::new();
        let mut close = String::new();
        if let Some(cls) = &r.mark {
            if cls.is_empty() {
                open.push_str("<mark>");
            } else {
                let _ = write!(open, "<mark class=\"{}\">", esc(&cls.join(" ")));
            }
            close.insert_str(0, "</mark>");
        }
        for (on, tag) in [(r.strong, "strong"), (r.em, "em"), (r.code, "code")] {
            if on {
                let _ = write!(open, "<{tag}>");
                close.insert_str(0, &format!("</{tag}>"));
            }
        }
        let _ = write!(out, "{open}{}{close}", esc(&r.text));
    }
    out
}

fn li_text(d: &Diagram, li: &crate::scene::LiEl, body: &mut String) {
    if let Some((_, i)) = &li.icon {
        body.push_str(&icon_svg("icon state-mark", i, &d.scene.images));
    }
    match li.sample {
        Some(crate::scene::Sample::Arrow { .. }) => {
            let cls = li.classes.first().map(String::as_str).unwrap_or_default();
            let _ = write!(
                body,
                "<svg class=\"sample\" viewBox=\"0 0 34 10\" aria-hidden=\"true\"><g class=\"edge {}\"><path class=\"line\" d=\"M0,5 L27,5\"/><path class=\"head\" d=\"M34,5 L26,1 L26,9 Z\"/></g></svg>",
                esc(cls)
            );
        }
        Some(crate::scene::Sample::Icon(_, i)) => {
            let cls = li.classes.first().map(String::as_str).unwrap_or_default();
            let class = format!("icon sample-icon {cls}");
            body.push_str(&icon_svg(&class, &i, &d.scene.images));
        }
        Some(crate::scene::Sample::Chip(_)) => {
            let cls = li.classes.first().map(String::as_str).unwrap_or_default();
            let zone = if crate::scene::zone_class(&d.model, cls) {
                " zone-sample"
            } else {
                ""
            };
            let _ = write!(body, "<span class=\"swatch{zone} {}\"></span>", esc(cls));
        }
        None => {}
    }
    if !li.parts.is_empty() {
        body.push_str(&runs_html(&li.text));
        return;
    }
    if let Some((_, ip)) = &li.mono {
        let _ = write!(body, "<span class=\"mono\">{}</span>", esc(ip));
    }
    body.push_str(&esc(&li.rest));
}

/// Arrows (same drawing as the SVG), in an `<svg>` laid on the stage.
fn edges_svg(d: &Diagram, (w, h): (f64, f64), body: &mut String) {
    let _ = write!(
        body,
        "<svg class=\"edges\" viewBox=\"0 0 {} {}\" width=\"{}\" height=\"{}\">",
        n(w),
        n(h),
        n(w),
        n(h)
    );
    for e in &d.model.edges {
        let Some(r) = d.routes.get(&e.id) else {
            continue;
        };
        if r.len() >= 2 {
            edge_html(d, e, r, body);
        }
    }
    body.push_str("</svg>");
}

/// An arrow: rounded line, arrival head, start head (tail) and origin dot.
fn edge_html(d: &Diagram, e: &skemo_lang::Edge, route: &[(f64, f64)], body: &mut String) {
    let pts: Vec<(f64, f64)> = route
        .iter()
        .map(|p| (p.0 + d.frame.ox, p.1 + d.frame.oy))
        .collect();
    let a = arrow(&pts);
    let sharp =
        crate::route::junctions_on(&d.routes, &d.edge_groups, &e.id, (d.frame.ox, d.frame.oy));
    // a group's shared trunk and its dots are drawn once (D-049)
    let line = crate::arrow::drawn_line(d, &e.id, &a, &pts);
    let (line, own_head) = crate::arrow::unshared(d, e, &line);
    let head = if own_head {
        triangle_path(&a.head)
    } else {
        String::new()
    };
    let dots: String = crate::arrow::own_dots(d, e, &sharp)
        .iter()
        .map(|(x, y)| {
            format!(
                "<circle class=\"junction\" cx=\"{}\" cy=\"{}\" r=\"2.5\"/>",
                n(*x),
                n(*y)
            )
        })
        .collect();
    if let Some(h) = &e.href {
        let _ = write!(body, "<a class=\"href\"{}>", href_attrs(h));
    }
    let _ = write!(
        body,
        "<g class=\"{}\" data-id=\"{}\"{}><path class=\"line\" d=\"{}\"/>{dots}<path class=\"head\" d=\"{}\"/><path class=\"tail\" d=\"{}\"/><circle class=\"origin\" cx=\"{}\" cy=\"{}\" r=\"3.5\"/></g>",
        esc(&edge_classes(d, e).join(" ")),
        esc(&e.id),
        anim_attrs(d, &e.id),
        crate::svg::rounded_path_hops(
            &line,
            8.0,
            &sharp,
            &crate::arrow::hops(d, &e.id, (d.frame.ox, d.frame.oy))
        ),
        head,
        triangle_path(&a.tail),
        n(a.origin.0),
        n(a.origin.1)
    );
    if e.href.is_some() {
        body.push_str("</a>");
    }
}

fn edge_labels(d: &Diagram, body: &mut String) {
    for e in &d.model.edges {
        if let Some(&(lx, ly)) = d.labels.get(&e.id) {
            let _ = write!(
                body,
                "<div class=\"{}\" style=\"display:contents\"><div class=\"edge-label\" data-for=\"{}\" style=\"left:{}px;top:{}px\"{}>{}</div></div>",
                esc(&edge_classes(d, e).join(" ")),
                esc(&e.id),
                n(lx + d.frame.ox),
                n(ly + d.frame.oy),
                anim_attrs(d, &e.id),
                match &e.href {
                    Some(h) => format!("<a class=\"href\"{}>{}</a>", href_attrs(h), esc(&e.label)),
                    None => esc(&e.label),
                }
            );
        }
    }
}

/// Full document: fonts, theme and custom CSS, step player.
fn page(d: &Diagram, opts: &HtmlOptions, body: &str) -> String {
    let bg = d
        .scene
        .style(d.scene.stage)
        .var_color("--bg")
        .map_or_else(|| "#F6F7F9".into(), |c| c.to_css());
    let fonts = if opts.embed_fonts {
        font_faces(d)
    } else {
        String::new()
    };
    let custom: String = d.css.join("\n");
    let base = &d.base_css;
    format!(
        "<!doctype html>\n<html lang=\"fr\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><title>{}</title>\n<style>\n{fonts}body{{margin:0;background:{bg}}}\n{base}\n{custom}\n{PLAYER_CSS}</style></head><body>\n{body}\n<script>{PLAYER_JS}</script></body></html>\n",
        esc(&opts.title)
    )
}
