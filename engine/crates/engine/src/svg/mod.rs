//! Standalone SVG rendering (F-RENDER-2, F-ANIM-3; FORMAT §4.2 and §4.4).
//!
//! Decorative subset translated: backgrounds, borders (width and colour per side, dotted), radii,
//! concrete colours (`color-mix` resolved by `lang`), shadow (`box-shadow` → filter), badges,
//! chips, header band, arrows (strokes, dashes, heads, origin dot).
//! Not translated (documented): gradients, background images, transforms, CSS animations (`.flow`
//! is rendered as fixed dashes), `text-decoration`, partial `opacity` of elements.
//!
//! Structure: one `<g>` per node, children nested (zone ⊃ cards ⊃ lines); links in a separate
//! group, above. `data-id` (never `id`); animation attributes only on the animated elements; no
//! class, style or animation script.

use crate::anim::{Presence, View};
use crate::diagram::Diagram;
use skemo_lang::Kind;
use std::fmt::Write as _;

mod deco;
mod edge;
mod node;
mod paint;
mod shadow;

pub use self::paint::{rounded_path, rounded_path_hops, rounded_path_sharp};
use self::{deco::*, edge::*, node::*, paint::*, shadow::*};

pub fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            _ => o.push(c),
        }
    }
    o
}

/// Short number for the SVG (2 decimals at most).
pub fn n(v: f64) -> String {
    let r = (v * 100.0).round() / 100.0;
    if r == 0.0 {
        return "0".into();
    }
    let s = format!("{r:.2}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

pub(super) struct Ctx<'a> {
    pub(super) d: &'a Diagram,
    pub(super) out: String,
    pub(super) filters: Vec<(String, String)>,
    pub(super) defs: Vec<String>,
    pub(super) prefix: String,
    /// Rasterisation variant (resvg): shadows drawn without SVG filters.
    pub(super) raster: bool,
    /// Image of a state or a transition (`None` = full diagram, marked for the animation).
    pub(super) view: Option<&'a View>,
}

fn presence(ctx: &Ctx, key: &str) -> Presence {
    ctx.view.map_or(Presence::FULL, |v| v.get(key))
}

/// Attributes of a group according to its presence (opacity, offset, scale around `(cx, cy)`).
fn presence_attrs(p: Presence, cx: f64, cy: f64) -> String {
    let mut a = String::new();
    if p.opacity < 0.999 {
        let _ = write!(a, " opacity=\"{}\"", n(p.opacity.max(0.0)));
    }
    let mut t = String::new();
    if p.dy.abs() > 0.001 {
        let _ = write!(t, "translate(0 {})", n(p.dy));
    }
    if (p.scale - 1.0).abs() > 0.0001 {
        let _ = write!(
            t,
            " translate({} {}) scale({}) translate({} {})",
            n(cx),
            n(cy),
            (p.scale * 10000.0).round() / 10000.0,
            n(-cx),
            n(-cy)
        );
    }
    if !t.is_empty() {
        let _ = write!(a, " transform=\"{}\"", t.trim());
    }
    a
}

/// Stable prefix of the internal identifiers (`<defs>`), to avoid collisions between several
/// diagrams of one page.
fn prefix(d: &Diagram) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in serde_json::to_string(&d.model).unwrap_or_default().bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("sk{:08x}", h as u32)
}

/// SVG rendering of the diagram.
pub fn render(d: &Diagram) -> String {
    render_inner(d, false, None, false)
}

/// SVG rendering with the fonts it uses embedded (`@font-face` as data URIs), so that the text
/// matches the measured boxes on a machine without Inter. Heavy: about 0.8 MB per weight used.
pub fn render_with_fonts(d: &Diagram) -> String {
    render_inner(d, false, None, true)
}

/// Variant for resvg (PNG, WebP): shadows without SVG filters. A still image of an animated
/// diagram shows its last step, as `--step` with that step would: drawing every step at once put
/// the lines that replace each other on one another (user's choice, defect 15). The SVG keeps
/// every element, marked for integrations (F-ANIM-3).
pub fn render_for_raster(d: &Diagram) -> String {
    render_inner(d, true, last_state(d).as_ref(), false)
}

/// Image of a state or a transition (`view`), for the PNG and the GIF.
pub fn render_view(d: &Diagram, view: &View, raster: bool) -> String {
    render_inner(d, raster, Some(view), false)
}

/// The view of an animated diagram's last step.
fn last_state(d: &Diagram) -> Option<View> {
    if !d.anim.is_animated() {
        return None;
    }
    let s = d.anim.states.last()?.value;
    Some(d.anim.view_at(&d.model, s, d.dim_opacity()))
}

/// `data-states` on the root, as in the HTML output: the ordered steps (values as written), so
/// that an integration (reveal.js fragments…) can map `2.5` to its own index. Static diagrams
/// carry nothing.
fn states_attr(d: &Diagram) -> String {
    if d.anim.states.len() < 2 {
        return String::new();
    }
    let states: Vec<&str> = d.anim.states.iter().map(|s| s.raw.as_str()).collect();
    format!(
        " data-states=\"{}\"",
        esc(&serde_json::to_string(&states).unwrap_or_default())
    )
}

fn render_inner(d: &Diagram, raster: bool, view: Option<&View>, fonts: bool) -> String {
    let mut ctx = Ctx {
        view,
        raster,
        d,
        out: String::new(),
        filters: Vec::new(),
        defs: Vec::new(),
        prefix: prefix(d),
    };
    let stage = d.scene.style(d.scene.stage).clone();
    let bg = background(&stage).or_else(|| stage.var_color("--bg"));
    if !d.transparent {
        let _ = write!(
            ctx.out,
            "<rect class=\"stage\" width=\"{}\" height=\"{}\"{}/>",
            n(d.frame.w.ceil()),
            n(d.frame.h.ceil()),
            paint("fill", bg)
        );
    }
    tl_under(&mut ctx);
    let top: Vec<(String, Kind)> = d
        .model
        .children(None)
        .map(|n| (n.id.clone(), n.kind))
        .collect();
    // frieze: a zone frames its events, under their stems and the axis (D-045)
    let early = |k: Kind| d.frise.is_some() && k == Kind::Zone;
    if top.iter().any(|(_, k)| early(*k)) {
        ctx.out.push_str("<g class=\"nodes\">");
        for (id, _) in top.iter().filter(|(_, k)| early(*k)) {
            zone(&mut ctx, id);
        }
        ctx.out.push_str("</g>");
    }
    fr_under(&mut ctx);
    ctx.out.push_str("<g class=\"nodes\">");
    for (id, kind) in top.iter().filter(|(_, k)| !early(*k)) {
        match kind {
            Kind::Zone => zone(&mut ctx, id),
            Kind::Card => card(&mut ctx, id),
        }
    }
    ctx.out.push_str("</g>");
    node::diagram_title(&mut ctx);
    ctx.out.push_str("<g class=\"edges\">");
    for e in &d.model.edges {
        edge(&mut ctx, e);
    }
    ctx.out.push_str("</g>");
    tl_over(&mut ctx);
    fr_over(&mut ctx);
    if fonts {
        ctx.defs
            .insert(0, format!("<style>{}</style>", crate::html::font_faces(d)));
    }
    let defs = ctx.defs.join("");
    let (w, h) = (d.frame.w.ceil(), d.frame.h.ceil());
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" class=\"skemo {}\" viewBox=\"0 0 {} {}\" width=\"{}\" height=\"{}\"{}>{}{}</svg>\n",
        d.scene.theme.class(),
        n(w),
        n(h),
        n(w),
        n(h),
        if view.is_none() {
            states_attr(d)
        } else {
            String::new()
        },
        if defs.is_empty() {
            String::new()
        } else {
            format!("<defs>{defs}</defs>")
        },
        ctx.out
    )
}

/// `true` when a style draws a box: a visible background or a border (D-050, `li.pill`).
/// `true` when the element has a visible background (a `mark`, a `code`, a table cell).
pub fn has_fill(c: &skemo_lang::style::Computed) -> bool {
    paint::background(c).is_some_and(|b| b.a > 0.0)
}

pub fn has_box(c: &skemo_lang::style::Computed) -> bool {
    paint::background(c).is_some_and(|b| b.a > 0.0)
        || crate::measure::border(c).iter().any(|b| *b > 0.0)
}
