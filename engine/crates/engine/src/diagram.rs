//! The whole pipeline: source → model → scene → measure → placement → routing → labels.

use crate::anim::{Animation, resolve};
use crate::layout::{Box2, Placement, elk_layout};
use crate::measure::{Sizes, measure};
use crate::route::{Points, params, route_all};
use crate::scene::Scene;
use serde_json::{Value, json};
use skemo_lang::style::Theme;
use skemo_lang::tr;
use skemo_lang::{Diagnostic, Model};
use std::collections::BTreeMap;

/// Framing margin around the content (prototype: 28 px).
pub const FRAME_PAD: f64 = 28.0;

#[derive(Debug, Clone, Default)]
pub struct Options {
    /// Custom CSS, applied in order after the theme.
    pub css: Vec<String>,
    /// Imposed theme (otherwise `diagram(theme=…)`, otherwise light).
    pub theme: Option<Theme>,
    /// Base theme replacing the default theme (`--base-theme`).
    pub base_theme: Option<String>,
    /// Enforced contrast: changes text colours only (see `contrast::enforce`).
    pub high_contrast: bool,
    /// Transparent background: the SVG and the PNGs no longer have a background rectangle (the HTML
    /// and the GIF keep the theme's background). Shadows, semi-transparent, stay as they are.
    pub transparent: bool,
    /// Image files used as icons (`icon="img/logo.svg"`, D-046), keyed by the value written in
    /// the source; the engine reads no file (see `images`).
    pub images: BTreeMap<String, Vec<u8>>,
    /// Font files (TrueType or OpenType) to use besides the embedded ones, wherever a
    /// `font-family` names them (`fonts::register`); the engine reads no file.
    pub fonts: Vec<Vec<u8>>,
}

/// Framing: offset applied to the file's coordinates and size of the image.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Frame {
    pub ox: f64,
    pub oy: f64,
    pub w: f64,
    pub h: f64,
}

impl Diagram {
    /// Opacity of the elements dimmed during a focus (the theme's `--dim-opacity`, default 0.28).
    pub fn dim_opacity(&self) -> f64 {
        self.scene
            .style(self.scene.stage)
            .vars
            .get("--dim-opacity")
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(0.28)
    }
}

pub struct Diagram {
    pub model: Model,
    pub scene: Scene,
    pub sizes: Sizes,
    pub elk: Placement,
    pub boxes: BTreeMap<String, Box2>,
    pub routes: BTreeMap<String, Points>,
    /// Group of each grouped arrow (trunk, fork, bus), for the junctions (`route::junctions`).
    pub edge_groups: BTreeMap<String, String>,
    /// Centre of each label (file coordinates).
    pub labels: BTreeMap<String, (f64, f64)>,
    pub anim: Animation,
    pub theme: Theme,
    pub frame: Frame,
    /// Warnings of the engine (in the current language).
    pub warnings: Vec<String>,
    pub css: Vec<String>,
    /// Base theme actually used.
    pub base_css: String,
    /// Decorations of the timeline mode (axis, graduations, diamonds, today).
    pub timeline: Option<crate::timeline::Deco>,
    /// Decorations of the frieze mode (axis, graduations, centuries, stems).
    pub frise: Option<crate::frise::Deco>,
    /// Classes and inline styles of the modes (timeline, frieze), used again by the HTML.
    pub mode_extras: crate::scene::Extras,
    /// Transparent background asked for (see [`Options::transparent`]).
    pub transparent: bool,
    /// Box of the diagram's title, in the file's frame (D-050): above the drawing.
    pub title_box: Option<Box2>,
}

/// Failure to prepare a diagram.
#[derive(Debug, Clone)]
pub enum Error {
    /// Errors of the file (`lang`'s diagnostics).
    Source(Vec<Diagnostic>),
    /// Erreur interne (placement).
    Internal(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Source(d) => write!(
                f,
                "{}",
                tr!(
                    "{} error(s) in the file",
                    "{} erreur(s) dans le fichier",
                    d.len()
                )
            ),
            Error::Internal(m) => {
                write!(f, "{}", tr!("internal error: {m}", "erreur interne : {m}"))
            }
        }
    }
}

/// Parses the source then prepares the diagram. `lang`'s warnings are returned with it.
pub fn from_source(src: &str, opts: &Options) -> Result<(Diagram, Vec<Diagnostic>), Error> {
    let doc = skemo_lang::parse(src);
    if doc.has_errors() {
        return Err(Error::Source(doc.errors().cloned().collect()));
    }
    let warnings: Vec<Diagnostic> = doc.warnings().cloned().collect();
    let fit = doc.model.option("fit").is_some();
    let mut d = build(doc.model, opts)?;
    if fit && d.frame.h > 0.0 {
        d.warnings
            .extend(crate::fit::check(src, opts, (d.frame.w, d.frame.h)));
    }
    Ok((d, warnings))
}

/// Placement mode asked for by `diagram(layout=…)`.
pub(crate) enum Layout {
    Graph,
    /// Mind map around the root card (identifier).
    Mindmap(String),
    Timeline(crate::timeline::Scale),
    Frise {
        scale: crate::frise::Scale,
        /// Eras whose name goes to the right (too short to hold it).
        outside: Vec<String>,
    },
}

/// Result of the placement: everything needed to route and frame.
struct Placed {
    elk: Placement,
    boxes: BTreeMap<String, Box2>,
    timeline: Option<crate::timeline::Deco>,
    frise: Option<crate::frise::Deco>,
    mode: crate::route::Mode,
    params: skemo_router::Params,
}

/// Prepares a diagram from a valid model.
pub fn build(model: Model, opts: &Options) -> Result<Diagram, Error> {
    // arrows between a docked element and the rest are not drawn (FORMAT §3.2.9)
    let model = crate::dock::without_crossing(model);
    let mut warnings = Vec::new();
    for f in &opts.fonts {
        if let Err(e) = crate::fonts::register(f) {
            warnings.push(e);
        }
    }
    let theme = opts
        .theme
        .or_else(|| model.option("theme").and_then(Theme::parse))
        .unwrap_or(Theme::Light);
    let base = opts
        .base_theme
        .clone()
        .unwrap_or_else(|| skemo_lang::style::DEFAULT_THEME.to_string());
    let mut layout = choose_layout(&model, &mut warnings);
    let images = decode_images(opts, &mut warnings);
    let (scene, extras) = build_scene(&model, &base, opts, theme, &mut layout, &images);
    warnings.extend(scene.warnings.iter().cloned());
    warnings.extend(missing_fonts(&scene));
    if model.option("layout") != Some("mindmap")
        && scene
            .edges
            .values()
            .any(|e| crate::arrow::asks_curve(&scene, e.line))
    {
        warnings.push(tr!(
            "« --curve: on » only draws curves in a mind map (layout=\"mindmap\"): straight routes are kept",
            "« --curve: on » ne trace des courbes que dans une carte mentale (layout=\"mindmap\") : les tracés droits sont gardés"
        ));
    }
    let mut sizes = measure(&scene, &model);
    let (pad, _) = scene.zone_pad_head();
    let arranged = arrange_all(&model, &scene, &mut sizes, &layout, &mut warnings)?;
    let Arranged {
        elk,
        boxes,
        timeline,
        frise,
        routes,
        edge_groups,
        labels,
    } = arranged;
    let anim = resolve(&model);
    warnings.extend(anim.warnings.iter().map(|w| w.message.clone()));

    let mut framed = boxes.clone();
    if let Some(d) = &timeline {
        framed.insert("\u{0}axe".into(), crate::timeline::axis_box(d, pad));
    }
    if let Some(d) = &frise {
        framed.insert("\u{0}axe".into(), crate::frise::axis_box(d));
    }
    let title_box = title_box(&model, &scene, &framed, &routes);
    if let Some(b) = title_box {
        framed.insert("\u{0}title".into(), b);
    }
    framed.extend(label_boxes(&labels, &sizes));
    let frame = frame(&framed, &routes);
    let mut scene = scene;
    if opts.high_contrast {
        crate::contrast::enforce(&model, &mut scene, theme);
    }
    Ok(Diagram {
        model,
        scene,
        sizes,
        elk,
        boxes,
        routes,
        edge_groups,
        labels,
        anim,
        theme,
        frame,
        warnings,
        css: opts.css.clone(),
        base_css: base,
        timeline,
        frise,
        mode_extras: extras,
        transparent: opts.transparent,
        title_box,
    })
}

/// A warning per font family that the stylesheets ask for first and that no font draws: Inter
/// is used instead (give the file with `Options::fonts`, `--font`).
fn missing_fonts(scene: &Scene) -> Vec<String> {
    let mut seen: Vec<String> = Vec::new();
    for c in &scene.styles {
        if let Some(f) = c.font_family().into_iter().next()
            && !crate::fonts::available(&f)
            && !seen.iter().any(|s| s.eq_ignore_ascii_case(&f))
        {
            seen.push(f);
        }
    }
    seen.into_iter()
        .map(|f| {
            tr!(
                "font « {f} » is not available: Inter is used (give the font file with --font)",
                "police « {f} » indisponible : Inter est utilisée (donner le fichier de police avec --font)"
            )
        })
        .collect()
}

/// What the layout and the routing give for a model (the whole diagram, or the drawing and each
/// docked element apart, `dock`).
pub(crate) struct Arranged {
    pub elk: Placement,
    pub boxes: BTreeMap<String, Box2>,
    pub timeline: Option<crate::timeline::Deco>,
    pub frise: Option<crate::frise::Deco>,
    pub routes: BTreeMap<String, Points>,
    pub edge_groups: BTreeMap<String, String>,
    pub labels: BTreeMap<String, (f64, f64)>,
}

/// The whole diagram arranged: in one go, or, with docked elements (FORMAT §3.2.9), the drawing
/// laid out as if they were absent, each of them on its own, then set around it.
fn arrange_all(
    model: &Model,
    scene: &Scene,
    sizes: &mut Sizes,
    layout: &Layout,
    warnings: &mut Vec<String>,
) -> Result<Arranged, Error> {
    let ph = scene.zone_pad_head();
    let Some(split) = crate::dock::split(model) else {
        return arrange(model, scene, sizes, layout, ph, warnings);
    };
    let main = arrange(&split.main, scene, sizes, layout, ph, warnings)?;
    let mut parts = Vec::new();
    for p in &split.parts {
        let a = arrange(&p.model, scene, sizes, &Layout::Graph, ph, warnings)?;
        parts.push((p.clone(), a));
    }
    let gap = scene
        .style(scene.stage)
        .length("--dock-gap")
        .unwrap_or(32.0);
    Ok(crate::dock::compose(main, parts, sizes, gap))
}

/// Places, routes and labels the nodes and arrows of `model` with the shared scene and sizes.
fn arrange(
    model: &Model,
    scene: &Scene,
    sizes: &mut Sizes,
    layout: &Layout,
    (pad, head): (f64, f64),
    warnings: &mut Vec<String>,
) -> Result<Arranged, Error> {
    // D-048 (R7): a note's `.link` does not shape the layout; the note is set beside its target
    let mut layout_model = crate::tidy::notes_unconstrained(model);
    // D-050: an arrow against a stack's order is turned round for the layout (its order holds)
    crate::stack::layout_view(&mut layout_model);
    // an inventory (a zone of cards with no arrow) is laid as a grid rather than one long row
    crate::tidy::inventory_grids(&mut layout_model);
    let placed = place(&layout_model, scene, sizes, layout, (pad, head), warnings)?;
    let Placed {
        elk,
        boxes,
        timeline,
        frise,
        mode,
        params: rparams,
    } = placed;
    let soft = timeline.as_ref().map(cost_bands).unwrap_or_default();
    let titles = crate::labels::zone_titles(model, scene, &boxes);
    let (mut routes, rw, edge_groups) =
        route_all(model, &boxes, &elk, &rparams, mode, &soft, &titles);
    warnings.extend(rw);
    if timeline.is_some() {
        routes.extend(crate::timeline::dependency_routes(model, scene, &boxes));
    }
    let labels = crate::labels::place(model, scene, &boxes, &routes, sizes);
    Ok(Arranged {
        elk,
        boxes,
        timeline,
        frise,
        routes,
        edge_groups,
        labels,
    })
}

/// Mode asked for; an unknown mode falls back on automatic placement, with a warning. The scale
/// of the dated modes is computed here; the frieze's "outside" eras will be once the scene is
/// measured (`build_scene`).
fn choose_layout(model: &Model, warnings: &mut Vec<String>) -> Layout {
    match model.option("layout").unwrap_or("graph") {
        "graph" => Layout::Graph,
        "timeline" => Layout::Timeline(crate::timeline::scale(model)),
        "mindmap" => match crate::mindmap::root_of(model) {
            Some(root) => Layout::Mindmap(root),
            None => Layout::Graph, // nothing to place
        },
        // `frise`: the prototype's name, still read
        "frieze" | "frise" => Layout::Frise {
            scale: crate::frise::scale(model),
            outside: Vec::new(),
        },
        other => {
            warnings.push(tr!(
                "unknown mode « {other} »: automatic placement is used",
                "mode « {other} » inconnu : placement automatique utilisé"
            ));
            Layout::Graph
        }
    }
}

/// Styled scene (with the mode's own elements). In a frieze, the name of an era too short goes to
/// the right (class `outside`): a first scene must be measured beforehand.
fn build_scene(
    model: &Model,
    base: &str,
    opts: &Options,
    theme: Theme,
    layout: &mut Layout,
    images: &[crate::images::Image],
) -> (Scene, crate::scene::Extras) {
    let mut extras = match layout {
        Layout::Graph => Default::default(),
        Layout::Mindmap(root) => crate::mindmap::extras(model, root),
        Layout::Timeline(sc) => crate::timeline::extras(model, sc),
        Layout::Frise { scale, outside } => {
            let first = Scene::build_full(
                model,
                base,
                &opts.css,
                theme,
                &crate::scene::Extras {
                    images: images.to_vec(),
                    ..crate::frise::extras(model, scale, &[])
                },
            );
            *outside = crate::frise::outside(model, &first, scale);
            crate::frise::extras(model, scale, outside)
        }
    };
    extras.images = images.to_vec();
    (
        Scene::build_full(model, base, &opts.css, theme, &extras),
        extras,
    )
}

/// The image files handed over in `Options::images`; a refused one is left out with a warning
/// (the icons that name it then draw nothing).
fn decode_images(opts: &Options, warnings: &mut Vec<String>) -> Vec<crate::images::Image> {
    let mut out = Vec::new();
    for (name, data) in &opts.images {
        match crate::images::check_name(name)
            .and_then(|()| crate::images::decode(name, data.clone()))
        {
            Ok(i) => out.push(i),
            Err(e) => warnings.push(e),
        }
    }
    out
}

/// Placement of the cards according to the mode.
fn place(
    model: &Model,
    scene: &Scene,
    sizes: &mut Sizes,
    layout: &Layout,
    (pad, head): (f64, f64),
    warnings: &mut Vec<String>,
) -> Result<Placed, Error> {
    Ok(match layout {
        Layout::Frise { scale, outside } => {
            let (boxes, deco) =
                crate::frise::layout(model, scene, sizes, (scale, outside), (pad, head));
            Placed {
                elk: Placement::default(),
                boxes,
                timeline: None,
                frise: Some(deco),
                mode: crate::route::Mode::Frise,
                params: skemo_router::Params {
                    shape_buffer: 8.0,
                    ..params()
                },
            }
        }
        Layout::Timeline(scale) => {
            let (boxes, deco) = crate::timeline::layout(model, scene, sizes, scale, pad, head);
            Placed {
                elk: Placement::default(),
                boxes,
                timeline: Some(deco),
                mode: crate::route::Mode::Timeline,
                frise: None,
                // smaller margin: the gutters between lanes are narrow (pitfall P19); extra cost in the lanes:
                // dependencies take the gutters (F-TL-6)
                params: skemo_router::Params {
                    shape_buffer: 8.0,
                    region_cost: 0.3,
                    ..params()
                },
            }
        }
        Layout::Mindmap(root) => {
            let (boxes, mut warns) = crate::mindmap::layout(model, sizes, root);
            warnings.append(&mut warns);
            Placed {
                elk: Placement::default(),
                boxes,
                timeline: None,
                frise: None,
                mode: crate::route::Mode::Mindmap,
                params: params(),
            }
        }
        Layout::Graph => {
            crate::tidy::before_elk(model, scene, sizes);
            let mut elk = elk_layout(model, sizes, pad, head).map_err(|e| Error::Internal(e.0))?;
            crate::tidy::after_elk(model, scene, sizes, (pad, head), &mut elk);
            let (boxes, moved) = crate::layout::final_boxes_report(model, sizes, &elk, pad, head);
            warnings.extend(
                moved
                    .into_iter()
                    .map(|(m, by)| overlap_warning(model, &m, &by)),
            );
            warnings.extend(
                crate::tidy::hidden_by_wrap(model, &boxes)
                    .iter()
                    .map(wrap_warning),
            );
            Placed {
                elk,
                boxes,
                timeline: None,
                frise: None,
                mode: crate::route::Mode::Graph,
                params: crate::route::graph_params(model),
            }
        }
    })
}

fn wrap_warning(h: &crate::tidy::Hidden) -> String {
    let (zone, wrap) = (&h.zone, h.wrap);
    let (cards, from) = (h.cards.join(", "), h.from.join(", "));
    tr!(
        "zone « {zone} »: with wrap=\"{wrap}\", {cards} sit behind other cards of the zone, so the arrows from {from} go around them; remove wrap or change dir (`skemo fit` compares the options)",
        "zone « {zone} » : avec wrap=\"{wrap}\", {cards} sont derrière d'autres cartes de la zone et les flèches venant de {from} doivent les contourner ; retirer wrap ou changer dir (`skemo fit` compare les options)"
    )
}

fn overlap_warning(model: &Model, moved: &str, by: &str) -> String {
    let what = if model
        .node(moved)
        .is_some_and(|n| n.kind == skemo_lang::Kind::Zone)
    {
        tr!("zone", "zone")
    } else {
        tr!("card", "carte")
    };
    tr!(
        "{what} « {moved} » overlaps « {by} » (position pinned by at=): move one of the two",
        "{what} « {moved} » chevauche « {by} » (position figée par at=) : déplacer l'une des deux"
    )
}

/// Timeline: extra-cost bands along the "today" line and the grid lines, so that dependencies do
/// not run along them (F-TL-6).
fn cost_bands(d: &crate::timeline::Deco) -> Vec<(String, Box2)> {
    let (top, h) = (d.axis_h, d.bottom - d.axis_h);
    let band = |x: f64, w: f64| Box2 { x, y: top, w, h };
    let mut soft = Vec::new();
    if let Some((x, _)) = d.today {
        soft.push(("\u{0}aujourd'hui".to_string(), band(x - 12.0, 24.0)));
    }
    for (k, t) in d.ticks.iter().enumerate() {
        soft.push((format!("\u{0}grille{k}"), band(t.x - 4.0, 8.0)));
    }
    soft
}

/// The prototype's framing: the content starts at (28, 28); the file's coordinates do not change.
/// Box of the diagram's title (D-050): above everything drawn, as wide as the drawing or as its
/// title if wider, its bottom margin from the theme. It never moves a card.
pub(crate) fn title_box(
    model: &Model,
    scene: &Scene,
    boxes: &BTreeMap<String, Box2>,
    routes: &BTreeMap<String, Points>,
) -> Option<Box2> {
    let (t, el) = (model.title.as_ref()?, scene.title.as_ref()?);
    let (w, h) = crate::geometry::title_size(scene, el, t);
    let f = frame(boxes, routes);
    let (x0, y0) = (FRAME_PAD - f.ox, FRAME_PAD - f.oy);
    let gap = scene.style(el.el).margin()[2];
    Some(Box2 {
        x: x0,
        y: y0 - gap - h,
        w: (f.w - 2.0 * FRAME_PAD).max(w),
        h,
    })
}

/// The boxes of the arrow labels (centre and measured size): the frame holds them too, a label
/// beside a line at the drawing's edge was cut.
pub(crate) fn label_boxes(
    labels: &BTreeMap<String, (f64, f64)>,
    sizes: &Sizes,
) -> Vec<(String, Box2)> {
    labels
        .iter()
        .filter_map(|(id, (x, y))| {
            let s = sizes.labels.get(id)?;
            Some((
                format!("\u{0}label|{id}"),
                Box2 {
                    x: x - s.w / 2.0,
                    y: y - s.h / 2.0,
                    w: s.w,
                    h: s.h,
                },
            ))
        })
        .collect()
}

pub(crate) fn frame(boxes: &BTreeMap<String, Box2>, routes: &BTreeMap<String, Points>) -> Frame {
    let mut x0 = f64::INFINITY;
    let mut y0 = f64::INFINITY;
    let mut x1 = f64::NEG_INFINITY;
    let mut y1 = f64::NEG_INFINITY;
    for b in boxes.values() {
        x0 = x0.min(b.x);
        y0 = y0.min(b.y);
        x1 = x1.max(b.right());
        y1 = y1.max(b.bottom());
    }
    for p in routes.values().flatten() {
        x0 = x0.min(p.0);
        y0 = y0.min(p.1);
        x1 = x1.max(p.0);
        y1 = y1.max(p.1);
    }
    if !x0.is_finite() {
        return Frame {
            ox: FRAME_PAD,
            oy: FRAME_PAD,
            w: 2.0 * FRAME_PAD,
            h: 2.0 * FRAME_PAD,
        };
    }
    let ox = FRAME_PAD - x0;
    let oy = FRAME_PAD - y0;
    Frame {
        ox,
        oy,
        w: x1 + ox + FRAME_PAD,
        h: y1 + oy + FRAME_PAD,
    }
}

fn r1(v: f64) -> Value {
    let r = (v * 10.0).round() / 10.0;
    if r.fract() == 0.0 {
        json!(r as i64)
    } else {
        json!(r)
    }
}

impl Diagram {
    /// The full layout in the oracle's format (`sizes`, `nodes`, `edges`), plus `labels` and `anim`.
    pub fn layout_json(&self) -> Value {
        let sizes: serde_json::Map<String, Value> = self
            .sizes
            .cards
            .iter()
            .map(|(k, s)| (k.clone(), json!({ "w": r1(s.w), "h": r1(s.h) })))
            .collect();
        let mut nodes = serde_json::Map::new();
        for n in &self.model.nodes {
            if let Some(b) = self.boxes.get(&n.id) {
                nodes.insert(n.id.clone(), json!([r1(b.x), r1(b.y), r1(b.w), r1(b.h)]));
            }
        }
        let mut edges = serde_json::Map::new();
        for e in &self.model.edges {
            if let Some(r) = self.routes.get(&e.id) {
                edges.insert(
                    e.id.clone(),
                    Value::Array(r.iter().map(|p| json!([r1(p.0), r1(p.1)])).collect()),
                );
            }
        }
        let labels: serde_json::Map<String, Value> = self
            .labels
            .iter()
            .map(|(k, p)| (k.clone(), json!([r1(p.0), r1(p.1)])))
            .collect();
        let anim: serde_json::Map<String, Value> = self
            .anim
            .elements
            .keys()
            .filter_map(|k| {
                let a = self.anim.attributes(k);
                (!a.is_empty()).then(|| {
                    (
                        k.clone(),
                        Value::Object(
                            a.into_iter()
                                .map(|(n, v)| (n.to_string(), Value::String(v)))
                                .collect(),
                        ),
                    )
                })
            })
            .collect();
        json!({
            "model": self.model,
            "layout": {
                "engine": if self.timeline.is_some() { "timeline" } else if self.frise.is_some() { "frise" } else { "elk" },
                "router": "skemo",
                "theme": self.theme.name(),
                "sizes": sizes,
                "nodes": nodes,
                "edges": edges,
                "labels": labels,
                "states": self.anim.states.iter().map(|s| s.raw.clone()).collect::<Vec<_>>(),
                "anim": anim,
            }
        })
    }
}
