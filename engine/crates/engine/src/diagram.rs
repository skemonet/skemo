//! The whole pipeline: source → model → scene → measure → placement → routing → labels.

use crate::anim::{Animation, resolve};
use crate::layout::{Box2, Placement, elk_layout};
use crate::measure::{Sizes, measure};
use crate::route::{Points, params, route_all};
use crate::scene::Scene;
use serde_json::{Value, json};
use skemo_lang::style::Theme;
use skemo_lang::tr;
use skemo_lang::{Diagnostic, Kind, Model};
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
    pub warnings: Vec<crate::EngineWarning>,
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
    /// Laid out by the grid engine (`grid`), not by the graph engine.
    pub grid: bool,
    /// Arrow labels wrapped on more than one line (the slide engine, L6), by arrow.
    pub label_lines: BTreeMap<String, Vec<String>>,
    /// Laid out by the slide engine (`slide`).
    pub slide: bool,
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
        d.warnings.extend(
            crate::fit::check(src, opts, (d.frame.w, d.frame.h))
                .map(|m| crate::EngineWarning::new("fit", m)),
        );
    }
    Ok((d, warnings))
}

/// Placement mode asked for by `diagram(layout=…)`.
pub(crate) enum Layout {
    /// The graph engine: ELK's layers, then the tidy passes (`layout`, `tidy`).
    Graph,
    /// The grid engine (`grid`).
    Grid,
    /// The slide engine (`slide`): placed, routed and labelled as one model; the grid engine
    /// for what it does not read. `true`: a preview, the cards and zones placed but no line
    /// routed (what the zone titles' wraps are read on).
    Slide(bool),
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
    let (model, engine) = automatic_engine(model);
    let mut warnings = Vec::new();
    for f in &opts.fonts {
        if let Err(e) = crate::fonts::register(f) {
            warnings.push(crate::EngineWarning::new("font-unreadable", e));
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
    let mut layout = engine.unwrap_or_else(|| choose_layout(&model, &mut warnings));
    let images = decode_images(opts, &mut warnings);
    let (mut scene, extras) = build_scene(&model, &base, opts, theme, &mut layout, &images);
    warnings.extend(scene.warnings.iter().cloned());
    warnings.extend(missing_fonts(&scene));
    warnings.extend(missing_glyphs(&model, &scene));
    if model.option("layout") != Some("mindmap")
        && scene
            .edges
            .values()
            .any(|e| crate::arrow::asks_curve(&scene, e.line))
    {
        warnings.push(crate::EngineWarning::new("curve-ignored", tr!(
            "`--curve: on` only draws curves in a mind map (layout=\"mindmap\"): straight routes are kept",
            "« --curve: on » ne trace des courbes que dans une carte mentale (layout=\"mindmap\") : les tracés droits sont gardés"
        )));
    }
    let mut sizes = measure(&scene, &model);
    let (pad, _) = scene.zone_pad_head();
    let arranged = arrange_wrapped(&model, &mut scene, &mut sizes, &layout, &mut warnings)?;
    let Arranged {
        label_lines,
        elk,
        boxes,
        timeline,
        frise,
        routes,
        edge_groups,
        labels,
    } = arranged;
    let anim = resolve(&model);
    warnings.extend(anim.warnings.iter().map(crate::EngineWarning::from));

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
        grid: matches!(layout, Layout::Grid | Layout::Slide(_)),
        label_lines,
        slide: matches!(layout, Layout::Slide(_)) && crate::slide::drew(),
    })
}

/// The diagram arranged, its zone titles wrapped where D-099 asks (the layout then runs again with
/// their room). Which titles wrap is read on the layout without pins (D-031): a card pinned where
/// the second layout put it would otherwise change the first one, and the wraps with it.
fn arrange_wrapped(
    model: &Model,
    scene: &mut Scene,
    sizes: &mut Sizes,
    layout: &Layout,
    warnings: &mut Vec<crate::EngineWarning>,
) -> Result<Arranged, Error> {
    let (pad, _) = scene.zone_pad_head();
    let mut placed = Vec::new();
    let grid = matches!(layout, Layout::Grid);
    let graph = grid || matches!(layout, Layout::Graph | Layout::Slide(_));
    let free = crate::layout::unpinned(model);
    let pinned = graph && matches!(free, std::borrow::Cow::Owned(_));
    let first = if pinned { &*free } else { model };
    // the first layout keeps no room for the titles (bug 3): their wraps are read on the frames
    // it gives (stacked zones stretched included); a title still wider, once wrapped, than the
    // frame its cards give widens its zone, in the second (D-064's room for a title is left out
    // of that frame: the second layout may not need it, a title kept on one line by it then ran
    // under the zone beside)
    let widths = std::mem::take(&mut sizes.title_widths);
    // the slide engine: the wraps are read on a preview (its cards and zones, no line), then the
    // drawing is made once
    let slide = matches!(layout, Layout::Slide(_));
    let first_layout = if slide { &Layout::Slide(true) } else { layout };
    let mut arranged = arrange_all(first, scene, sizes, first_layout, &mut placed)?;
    let plain = crate::layout::Placement {
        title_ends: BTreeMap::new(),
        title_widths: BTreeMap::new(),
        ..arranged.elk.clone()
    };
    let (_, head) = scene.zone_pad_head();
    let frames: BTreeMap<String, f64> =
        crate::layout::zone_frames(first, &arranged.boxes, &plain, pad, head)
            .into_iter()
            .map(|(id, b)| (id, b.w))
            .collect();
    let too_wide = |id: &String, w: &f64| frames.get(id).is_some_and(|f| *w > f + 0.5);
    let (mut narrow, mut wrapped_subs) = (false, false);
    if graph {
        sizes.title_widths = widths;
        let wraps = crate::layout::title_wraps(model, scene, sizes, &arranged.boxes, pad);
        let subs = crate::layout::sub_wraps(model, scene, sizes, &arranged.boxes, pad);
        if !wraps.is_empty() {
            scene.head_lines = crate::layout::head_lines(model, &wraps, &arranged.boxes);
            scene.title_wraps = wraps;
        }
        wrapped_subs = !subs.is_empty();
        for (id, texts) in subs {
            if let Some(z) = scene.zones.get_mut(&id) {
                for ((_, t), new) in z.subs.iter_mut().zip(texts) {
                    *t = new;
                }
            }
        }
        narrow = sizes.title_widths.iter().any(|(id, w)| too_wide(id, w));
        sizes.title_widths.clear();
    }
    // every decision on titles is read on layouts without pins (D-031): the pinned drawing is
    // laid out last, with the sizes they gave
    let mut retained: Vec<String> = Vec::new();
    if slide || narrow || wrapped_subs || !scene.title_wraps.is_empty() {
        // measured again: the layout evens and stretches the cards' sizes it is given
        *sizes = measure(scene, model);
        sizes.title_widths.retain(|id, w| graph && too_wide(id, w));
        retained = sizes.title_widths.keys().cloned().collect();
        placed.clear();
        arranged = arrange_all(first, scene, sizes, layout, &mut placed)?;
        // the second layout may set the cards otherwise than the first: a title it leaves wider
        // than its frame wraps on that frame, and keeps room if still too wide
        if graph && overflowing(first, scene, &arranged, pad).is_some() {
            *sizes = measure(scene, model);
            let final_w = |id: &String| arranged.boxes.get(id).map(|b| b.w);
            sizes
                .title_widths
                .retain(|id, w| retained.contains(id) || final_w(id).is_some_and(|f| *w > f + 0.5));
            retained = sizes.title_widths.keys().cloned().collect();
            placed.clear();
            arranged = arrange_all(first, scene, sizes, layout, &mut placed)?;
        }
    }
    // bug 20: a title wrapped on an earlier layout, where its zone stood narrower or its arrow
    // came in further left, wraps again on the frame it has now when that takes fewer lines
    if graph && unwrapped(first, scene, &arranged, pad) {
        *sizes = measure(scene, model);
        sizes.title_widths.retain(|id, _| retained.contains(id));
        placed.clear();
        arranged = arrange_all(first, scene, sizes, layout, &mut placed)?;
    }
    let ((corridors, corridors_across), spacing) = if graph {
        make_room(
            first,
            model,
            scene,
            sizes,
            layout,
            (&mut placed, &mut arranged),
            &retained,
        )?
    } else {
        ((Vec::new(), Vec::new()), crate::measure::Spacing::new())
    };
    if pinned {
        // the zones stretched without pins stretch with them, and no other (D-085, D-031)
        let stretched = stretched_zones(first, &arranged, (pad, scene.zone_pad_head().1));
        *sizes = measure(scene, model);
        sizes.title_widths.retain(|id, _| retained.contains(id));
        // the room read on the layout without pins is kept with them
        sizes.corridors = corridors;
        sizes.corridors_across = corridors_across;
        sizes.spacing = spacing;
        sizes.stretched = Some(stretched);
        placed.clear();
        arranged = arrange_all(model, scene, sizes, layout, &mut placed)?;
    }
    warnings.append(&mut placed);
    Ok(arranged)
}

/// D-102 and the grid's room, after the titles: the room the lines and the labels lack, read on
/// `arranged` (laid out without pins), and the drawing laid out again with it; returns the room
/// made, for the pinned layout to keep.
fn make_room(
    first: &Model,
    model: &Model,
    scene: &mut Scene,
    sizes: &mut Sizes,
    layout: &Layout,
    (placed, arranged): (&mut Vec<crate::EngineWarning>, &mut Arranged),
    retained: &[String],
) -> Result<(Corridors, crate::measure::Spacing), Error> {
    // D-102: corridors too narrow for the lines the router drew in them widen, and the drawing
    // is laid out and routed again (read on the layout without pins, like the titles). The grid
    // engine makes its room evenly (`grid::room`): what the lines and the labels lack is asked
    // of the gap that owns the place, the same between all the children of a group
    let grid = matches!(layout, Layout::Grid);
    let mut corridors = Vec::new();
    let mut corridors_across = Vec::new();
    let mut spacing = crate::measure::Spacing::new();
    {
        let mut more = crate::measure::Spacing::new();
        if grid {
            more = lacking_spacing(first, scene, sizes, arranged);
        } else {
            let across = crate::corridors::across(first);
            corridors = lacking_corridors(first, scene, sizes, arranged, across);
            if !crate::trace::off("graph-columns") {
                corridors_across = lacking_corridors(first, scene, sizes, arranged, !across);
            }
        }
        // the widened drawing is routed and its labels placed again, elsewhere for some (a
        // line bends otherwise, a card lines up in the room made): what they still lack is
        // added, twice at most. The grid keeps the round that hides the fewest labels (the last
        // one widened a gap for the lines and put a label back under a line, `oadp`)
        let mut best: Option<(usize, crate::measure::Spacing)> = None;
        for round in 0..ROOM_ROUNDS {
            if more.is_empty()
                && (round > 0 || (corridors.is_empty() && corridors_across.is_empty()))
            {
                break;
            }
            for (gap, d) in more {
                if crate::trace::wanted("room") {
                    eprintln!("trace room round {round}: {d} px more at {gap:?}");
                }
                // a gutter gains what it lacks; a padding is what it must be (`grid::room`)
                let padding = gap.2;
                let room = spacing.entry(gap).or_insert(0.0);
                // what the widened drawing still lacks, asked with some more (`ROOM_MORE`)
                let d = if round > 0 { d + ROOM_MORE } else { d };
                *room = if padding { room.max(d) } else { *room + d };
            }
            // measured again: the layout evens and stretches the cards' sizes it is given
            *sizes = measure(scene, model);
            sizes.title_widths.retain(|id, _| retained.contains(id));
            sizes.corridors = corridors.clone();
            sizes.corridors_across = corridors_across.clone();
            sizes.spacing = spacing.clone();
            placed.clear();
            *arranged = arrange_all(first, scene, sizes, layout, placed)?;
            if grid {
                let hidden = hidden_labels(first, scene, sizes, arranged);
                if crate::trace::wanted("room") {
                    eprintln!("trace room round {round}: {hidden} labels hidden");
                }
                if best.as_ref().is_none_or(|(least, _)| hidden <= *least) {
                    best = Some((hidden, spacing.clone()));
                }
            }
            more = if grid && round + 1 < ROOM_ROUNDS {
                lacking_spacing(first, scene, sizes, arranged)
            } else {
                crate::measure::Spacing::new()
            };
        }
        if let Some((_, kept)) = best.filter(|(_, kept)| *kept != spacing) {
            spacing = kept;
            *sizes = measure(scene, model);
            sizes.title_widths.retain(|id, _| retained.contains(id));
            sizes.corridors = corridors.clone();
            sizes.corridors_across = corridors_across.clone();
            sizes.spacing = spacing.clone();
            placed.clear();
            *arranged = arrange_all(first, scene, sizes, layout, placed)?;
        }
    }
    Ok(((corridors, corridors_across), spacing))
}

/// The corridors widened (D-102): along the flow, and across it.
type Corridors = (crate::measure::Widened, crate::measure::Widened);

/// The grid engine: how many labels of `arranged` a line, a card, a zone's frame or another
/// label hides, as `grid::labelled` counts them.
fn hidden_labels(first: &Model, scene: &Scene, sizes: &Sizes, arranged: &Arranged) -> usize {
    own_part(
        first,
        scene,
        sizes,
        arranged,
        |main, drawn, (routes, groups)| {
            let (boxes, titles, _) = drawn;
            let around = crate::grid::Around::new(main, boxes, titles, groups, &sizes.labels);
            let labels: BTreeMap<String, (f64, f64)> = arranged
                .labels
                .iter()
                .filter(|(id, _)| routes.contains_key(*id))
                .map(|(id, c)| (id.clone(), *c))
                .collect();
            crate::grid::hidden(&around, routes, &labels)
        },
    )
}

/// The zones `tidy::stretch_zones` stretched on `arranged`: their frames differ from the frames
/// laid out with no stretching.
fn stretched_zones(
    model: &Model,
    arranged: &Arranged,
    (pad, head): (f64, f64),
) -> std::collections::BTreeSet<String> {
    let fitted = crate::layout::Placement {
        stretched: Some(std::collections::BTreeSet::new()),
        ..arranged.elk.clone()
    };
    let with = crate::layout::zone_frames(model, &arranged.boxes, &arranged.elk, pad, head);
    let without = crate::layout::zone_frames(model, &arranged.boxes, &fitted, pad, head);
    with.into_iter()
        .filter(|(id, b)| without.get(id) != Some(b))
        .map(|(id, _)| id)
        .collect()
}

/// Layouts made for the room, at most: the first widening, then what the lines and the labels
/// of the widened drawing still lack, twice.
const ROOM_ROUNDS: usize = 3;

/// What a gap of the grid still lacks once widened is asked with this much more: the lines and
/// the labels move into the room made, so asking the bare lack again leaves it short after the
/// last round (`oadp`, `hypershift`: 46 → 24 px, 18 → 9 px lacking). 16 px measured on the
/// noise band of the private corpus (D-109): 8 px left more labels hidden, 24 px more crossings.
const ROOM_MORE: f64 = 16.0;

/// The grid engine: the room the lines and the labels of `arranged` lack, by the gap that owns
/// each place (`grid::room`): between its rows (y) and its columns (x) for the lines, along x
/// for the labels.
fn lacking_spacing(
    first: &Model,
    scene: &Scene,
    sizes: &Sizes,
    arranged: &Arranged,
) -> crate::measure::Spacing {
    let on = crate::grid::on;
    let mut out = crate::measure::Spacing::new();
    own_part(first, scene, sizes, arranged, |main, drawn, lines| {
        for (rule, across) in [("grid-gutter-rows", false), ("grid-gutter-columns", true)] {
            if !on(rule) {
                continue;
            }
            let tails = on("grid-tail-room");
            let lacks = crate::corridors::lacks(main, drawn, lines, across, tails);
            let room = crate::grid::line_room(main, drawn.0, drawn.1, &lacks, across);
            crate::grid::merge(&mut out, room);
        }
    });
    if on("grid-label-room") {
        let split = crate::dock::split(first);
        let main = split.as_ref().map_or(first, |s| &s.main);
        let titles = crate::labels::zone_titles(main, scene, &arranged.boxes);
        let room = crate::grid::label_room(
            main,
            &arranged.boxes,
            &titles,
            (&arranged.routes, &arranged.edge_groups),
            (&arranged.labels, &sizes.labels),
        );
        crate::grid::merge(&mut out, room);
    }
    out
}

/// D-102: the room the corridors of `arranged` lack, on the drawing's own part.
fn lacking_corridors(
    first: &Model,
    scene: &Scene,
    sizes: &Sizes,
    arranged: &Arranged,
    across: bool,
) -> Vec<(f64, f64)> {
    own_part(first, scene, sizes, arranged, |main, drawn, lines| {
        crate::corridors::lacking_on(main, drawn, lines, across)
    })
}

/// Reads the drawing's own part with `read`: its model, what is drawn (boxes, zone titles, arrow
/// labels) and its lines (routes, groups). A docked element is laid out apart (FORMAT §3.2.9),
/// and set around the drawing without moving it.
fn own_part<T>(
    first: &Model,
    scene: &Scene,
    sizes: &Sizes,
    arranged: &Arranged,
    read: impl FnOnce(
        &Model,
        crate::corridors::Drawn,
        (&BTreeMap<String, Points>, &BTreeMap<String, String>),
    ) -> T,
) -> T {
    let split = crate::dock::split(first);
    let main = split.as_ref().map_or(first, |s| &s.main);
    let boxes: BTreeMap<String, Box2> = arranged
        .boxes
        .iter()
        .filter(|(id, _)| main.node(id).is_some())
        .map(|(id, b)| (id.clone(), *b))
        .collect();
    let routes: BTreeMap<String, Points> = arranged
        .routes
        .iter()
        .filter(|(id, _)| main.edges.iter().any(|e| e.id == **id))
        .map(|(id, r)| (id.clone(), r.clone()))
        .collect();
    let titles = crate::labels::zone_titles(main, scene, &boxes);
    let labels: Vec<(String, Box2)> = arranged
        .labels
        .iter()
        .filter(|(id, _)| routes.contains_key(*id))
        .filter_map(|(id, c)| {
            sizes.labels.get(id).map(|s| {
                let b = Box2 {
                    x: c.0 - s.w / 2.0,
                    y: c.1 - s.h / 2.0,
                    w: s.w,
                    h: s.h,
                };
                (id.clone(), b)
            })
        })
        .collect();
    read(
        main,
        (&boxes, &titles, &labels),
        (&routes, &arranged.edge_groups),
    )
}

/// Bug 20 of the private corpus: the wrapped titles that take fewer lines on the frames of
/// `arranged` (wrapped from their written text there) get that wrap (`scene` updated); `true`
/// when one did. A zone narrow on the first layout, then as wide as its stack, kept "ICI → /
/// AUTRES / NOTIONS" on three lines with room for two.
fn unwrapped(model: &Model, scene: &mut Scene, arranged: &Arranged, pad: f64) -> bool {
    if scene.title_wraps.is_empty() {
        return false;
    }
    // measured and wrapped from the written titles, the wraps set aside meanwhile, on the frames
    // as they stand without room for titles (a zone stretched to its stack's width keeps that)
    let mut wraps = std::mem::take(&mut scene.title_wraps);
    let fresh = measure(scene, model);
    let plain = Placement {
        title_ends: BTreeMap::new(),
        title_widths: BTreeMap::new(),
        ..arranged.elk.clone()
    };
    let (_, head) = scene.zone_pad_head();
    let mut frames = arranged.boxes.clone();
    frames.extend(crate::layout::zone_frames(
        model,
        &arranged.boxes,
        &plain,
        pad,
        head,
    ));
    let now = crate::layout::title_wraps_on(model, scene, &fresh, (&frames, true), pad);
    let lines = |t: &str| t.matches('\n').count() + 1;
    let mut trial = wraps.clone();
    let mut candidates = Vec::new();
    for (id, old) in &wraps {
        match now.get(id) {
            Some(t) if lines(t) < lines(old) => {
                trial.insert(id.clone(), t.clone());
                candidates.push(id.clone());
            }
            None => {
                trial.remove(id);
                candidates.push(id.clone());
            }
            _ => {}
        }
    }
    // kept only when the zone, widened for that title (D-064 on its left, as much on its right,
    // D-099), stays inside the frame it has: a wider zone ran over the card beside it (the
    // private map's current `mce/installation`, its legend)
    scene.title_wraps = trial;
    let ends = measure(scene, model).title_ends;
    let mut changed = false;
    for id in candidates {
        let (Some(frame), Some(&end)) = (arranged.boxes.get(&id), ends.get(&id)) else {
            continue;
        };
        let kids: Vec<&Box2> = model
            .children(Some(&id))
            .filter_map(|c| arranged.boxes.get(&c.id))
            .collect();
        let x1_cards = kids.iter().map(|b| b.x).fold(f64::INFINITY, f64::min) - pad;
        let x2_cards = kids
            .iter()
            .map(|b| b.right())
            .fold(f64::NEG_INFINITY, f64::max)
            + pad;
        let clear = crate::layout::title_clear_left(model, &id, &arranged.boxes);
        let x1 = x1_cards.min((clear - end).floor());
        let x2 = x2_cards + (x1_cards - x1);
        if x1 >= frame.x - 0.5 && x2 <= frame.right() + 0.5 && end <= frame.w {
            match now.get(&id) {
                Some(t) => wraps.insert(id, t.clone()),
                None => wraps.remove(&id),
            };
            changed = true;
        }
    }
    scene.title_wraps = wraps;
    if changed {
        let all = scene.title_wraps.clone();
        scene.head_lines = crate::layout::head_lines(model, &all, &arranged.boxes);
    }
    changed
}

/// Bug 3, after the second layout: the zones whose title or description lines run past the frame
/// it gave are wrapped again on that frame (`scene` updated); `None` when every title fits.
fn overflowing(
    model: &Model,
    scene: &mut Scene,
    arranged: &Arranged,
    pad: f64,
) -> Option<Vec<String>> {
    let full = measure(scene, model);
    let over: Vec<String> = full
        .title_widths
        .iter()
        .filter(|(id, w)| arranged.boxes.get(*id).is_some_and(|b| **w > b.w + 0.5))
        .map(|(id, _)| id.clone())
        .collect();
    if over.is_empty() {
        return None;
    }
    // wrapped from their written text, on the frames as they now are
    let mut fresh = full;
    for id in &over {
        scene.title_wraps.remove(id);
    }
    fresh.title_ends = measure(scene, model).title_ends;
    fresh.title_ends.retain(|id, _| over.contains(id));
    let wraps = crate::layout::title_wraps(model, scene, &fresh, &arranged.boxes, pad);
    let subs = crate::layout::sub_wraps(model, scene, &fresh, &arranged.boxes, pad);
    scene.title_wraps.extend(wraps);
    let all = scene.title_wraps.clone();
    scene.head_lines = crate::layout::head_lines(model, &all, &arranged.boxes);
    for (id, texts) in subs {
        if let Some(z) = scene.zones.get_mut(&id) {
            for ((_, t), new) in z.subs.iter_mut().zip(texts) {
                *t = new;
            }
        }
    }
    Some(over)
}

/// A warning per font family that the stylesheets ask for first and that no font draws: Inter
/// is used instead (give the file with `Options::fonts`, `--font`).
fn missing_fonts(scene: &Scene) -> Vec<crate::EngineWarning> {
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
            crate::EngineWarning::new(
                "font-missing",
                tr!(
                    "font `{f}` is not available: Inter is used (give the font file with --font)",
                    "police « {f} » indisponible : Inter est utilisée (donner le fichier de police avec --font)"
                ),
            )
        })
        .collect()
}

/// Characters the font does not draw: a blank in their place, said rather than left silent.
fn missing_glyphs(model: &Model, scene: &Scene) -> Vec<crate::EngineWarning> {
    crate::quality::missing_glyph(model, scene)
        .into_iter()
        .map(|found| {
            let (id, chars) = found.split_once(' ').unwrap_or((&found, ""));
            crate::EngineWarning::new(
                "missing-glyph",
                tr!(
                    "`{id}`: the font does not draw `{chars}` (drawn blank): use another character or give a font that has it with --font",
                    "« {id} » : la police ne dessine pas « {chars} » (un blanc à la place) : utiliser un autre caractère ou donner une police qui l'a avec --font"
                ),
            )
        })
        .collect()
}

/// What the layout and the routing give for a model (the whole diagram, or the drawing and each
/// docked element apart, `dock`).
pub(crate) struct Arranged {
    pub label_lines: BTreeMap<String, Vec<String>>,
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
    warnings: &mut Vec<crate::EngineWarning>,
) -> Result<Arranged, Error> {
    let ph = scene.zone_pad_head();
    let Some(split) = crate::dock::split(model) else {
        return arrange(model, scene, sizes, layout, ph, warnings);
    };
    let main = arrange(&split.main, scene, sizes, layout, ph, warnings)?;
    // a docked element is a small drawing of its own: the grid's when the drawing is a grid
    let part = if matches!(layout, Layout::Grid | Layout::Slide(_)) {
        &Layout::Grid
    } else {
        &Layout::Graph
    };
    // the corridors widened are the drawing's own (D-102), not a docked element's
    let corridors = std::mem::take(&mut sizes.corridors);
    let corridors_across = std::mem::take(&mut sizes.corridors_across);
    // each docked element laid out as if none of its cards were pinned: where it goes around the
    // drawing does not depend on its pins (D-031)
    let mut parts = Vec::new();
    // the sizes of a part with pins, as measured: its layout evens and stretches its cards
    let mut pinned: Vec<Option<Sizes>> = Vec::new();
    for p in &split.parts {
        let free = crate::layout::unpinned(&p.model);
        let mut scratch = Vec::new();
        let has_pins = matches!(free, std::borrow::Cow::Owned(_));
        pinned.push(has_pins.then(|| sizes.clone()));
        let w = if has_pins {
            &mut scratch
        } else {
            &mut *warnings
        };
        let a = arrange(&free, scene, sizes, part, ph, w)?;
        parts.push((p.clone(), a));
    }
    let gap = scene
        .style(scene.stage)
        .length("--dock-gap")
        .unwrap_or(32.0);
    // bug 2: a `.wide` card docked above or below spans the bands' whole width (FORMAT: `.wide`
    // spans the width of its group), its text wrapped to it; it kept its own width, a note 280 px
    // wide under a drawing of 400
    let full = crate::dock::band_width(&main, &parts, sizes, gap);
    for ((p, a), measured) in parts.iter_mut().zip(&pinned) {
        if !p.wide || !matches!(p.point.as_str(), "n" | "s") || measured.is_some() {
            continue;
        }
        let Some(card) = p
            .model
            .nodes
            .iter()
            .find(|n| n.parent.is_none() && n.kind == Kind::Card)
            .and_then(|n| scene.cards.get(&n.id).map(|c| (n.id.clone(), c)))
        else {
            continue;
        };
        let size = crate::measure::card_size(scene, card.1, Some(full));
        sizes.cards.insert(card.0, size);
        *a = arrange(&p.model, scene, sizes, part, ph, warnings)?;
    }
    let offsets = crate::dock::offsets(&main, &parts, sizes, gap);
    // then again with its pins, which are in the drawing's frame, brought into its own
    for (((p, a), d), measured) in parts.iter_mut().zip(&offsets).zip(&pinned) {
        let Some(measured) = measured else { continue };
        for n in &p.model.nodes {
            if let Some(s) = measured.cards.get(&n.id) {
                sizes.cards.insert(n.id.clone(), *s);
            }
        }
        let local = crate::dock::pins_by(&p.model, (-d.0, -d.1));
        *a = arrange(&local, scene, sizes, part, ph, warnings)?;
    }
    sizes.corridors = corridors;
    sizes.corridors_across = corridors_across;
    Ok(crate::dock::merge(main, parts, &offsets))
}

/// The model as the layout sees it: the arrows that must not shape it are marked or turned, the
/// inventories wrapped. The routing keeps the model as written.
fn layout_view(model: &Model) -> Model {
    let on = |pass: &str| !crate::trace::off(pass);
    let mut view = model.clone();
    // D-048 (R7): a note's `.link` does not shape the layout; the note is set beside its target
    if on("notes_unconstrained") {
        view = crate::tidy::notes_unconstrained(model);
    }
    // D-050: an arrow against a stack's order is turned round for the layout (its order holds)
    if on("layout_view") {
        crate::stack::layout_view(&mut view);
    }
    // sibling zones linked both ways keep their writing order: the arrows back do not shape it
    if on("zones_in_order") {
        crate::tidy::zones_in_order(&mut view);
    }
    // an inventory (a zone of cards with no arrow) is laid as a grid rather than one long row
    if on("inventory_grids") {
        crate::tidy::inventory_grids(&mut view);
    }
    view
}

/// Places, routes and labels the nodes and arrows of `model` with the shared scene and sizes.
fn arrange(
    model: &Model,
    scene: &Scene,
    sizes: &mut Sizes,
    layout: &Layout,
    (pad, head): (f64, f64),
    warnings: &mut Vec<crate::EngineWarning>,
) -> Result<Arranged, Error> {
    if let Layout::Slide(preview) = *layout {
        // the slide is drawn without its pins (D-031); a pin where the slide put the card changes
        // nothing, a card moved elsewhere stands where it is pinned (`slide_pinned`)
        let free = crate::layout::unpinned(model);
        if let Some(d) = crate::slide::arrange(&free, scene, sizes, head, preview) {
            sizes.labels.extend(d.label_sizes.clone());
            let zones = zone_pins(model, &d.boxes);
            let mut moved = pins_moved(model, &d.boxes);
            // the cards of a zone placed by hand go with it (their own pins first)
            for (z, dx, dy, _) in &zones {
                for n in model.nodes.iter().filter(|n| n.kind == Kind::Card) {
                    if n.at.is_none()
                        && within(model, &n.id, z)
                        && let Some(b) = d.boxes.get(&n.id)
                    {
                        moved.push((n.id.clone(), b.x + dx, b.y + dy));
                    }
                }
            }
            if (!moved.is_empty() || !zones.is_empty()) && !preview {
                return Ok(slide_pinned(
                    model,
                    scene,
                    sizes,
                    d,
                    (&moved, &zones),
                    warnings,
                ));
            }
            return Ok(Arranged {
                label_lines: d.label_lines,
                elk: Placement::default(),
                boxes: d.boxes,
                timeline: None,
                frise: None,
                routes: d.routes,
                edge_groups: BTreeMap::new(),
                labels: d.labels,
            });
        }
        return arrange(model, scene, sizes, &Layout::Grid, (pad, head), warnings);
    }
    let layout_model = layout_view(model);
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
    let (anchored, obstacles) = crate::route::anchors(model, scene, &boxes, &titles);
    let (mut routes, rw, edge_groups) =
        route_all(model, &anchored, &elk, &rparams, mode, &soft, &obstacles);
    warnings.extend(rw);
    if timeline.is_some() {
        routes.extend(crate::timeline::dependency_routes(model, scene, &boxes));
    }
    let labels = label_all(
        model,
        scene,
        &boxes,
        &mut routes,
        sizes,
        &edge_groups,
        mode == crate::route::Mode::Grid,
    );
    Ok(Arranged {
        label_lines: BTreeMap::new(),
        elk,
        boxes,
        timeline,
        frise,
        routes,
        edge_groups,
        labels,
    })
}

/// Is node `id` inside zone `z`, at any depth?
fn within(model: &Model, id: &str, z: &str) -> bool {
    let mut p = model.node(id).and_then(|n| n.parent.as_deref());
    while let Some(q) = p {
        if q == z {
            return true;
        }
        p = model.node(q).and_then(|n| n.parent.as_deref());
    }
    false
}

/// The zones placed by hand (`at=`, `size=`) elsewhere than the slide's frame, or sized: each
/// with the move of its frame and the size written.
type ZonePin = (String, f64, f64, Option<[f64; 2]>);

fn zone_pins(model: &Model, boxes: &BTreeMap<String, Box2>) -> Vec<ZonePin> {
    model
        .nodes
        .iter()
        .filter(|n| n.kind == Kind::Zone && (n.at.is_some() || n.size.is_some()))
        .filter_map(|n| {
            let b = boxes.get(&n.id)?;
            let [x, y] = n.at.unwrap_or([b.x, b.y]);
            Some((n.id.clone(), x - b.x, y - b.y, n.size))
        })
        .collect()
}

/// The cards pinned elsewhere than where the slide put them (a pin within half a pixel of its
/// place is that place), with their pinned corner.
fn pins_moved(model: &Model, boxes: &BTreeMap<String, Box2>) -> Vec<(String, f64, f64)> {
    model
        .nodes
        .iter()
        .filter(|n| n.kind == Kind::Card)
        .filter_map(|n| {
            let [x, y] = n.at?;
            let b = boxes.get(&n.id)?;
            ((b.x - x).abs() > 0.5 || (b.y - y).abs() > 0.5).then(|| (n.id.clone(), x, y))
        })
        .collect()
}

/// The slide with cards (or zones) moved by hand: each card stands where it is pinned, a zone
/// placed by hand where written with what it holds and the size written, the zones holding a
/// moved card grow round it (their margin kept), and every arrow is routed again by the router,
/// as the grid engine routes (the lattice's routes hold only for the lattice's places).
fn slide_pinned(
    model: &Model,
    scene: &Scene,
    sizes: &Sizes,
    d: crate::slide::Drawn,
    (moved, zones): (&[(String, f64, f64)], &[ZonePin]),
    warnings: &mut Vec<crate::EngineWarning>,
) -> Arranged {
    // a zone placed by hand: its frame and those inside it move with it
    let mut boxes = d.boxes;
    for (z, dx, dy, _) in zones {
        for (id, b) in &mut boxes {
            let zone = model.node(id).is_some_and(|n| n.kind == Kind::Zone);
            if zone && (id == z || within(model, id, z)) {
                b.x += dx;
                b.y += dy;
            }
        }
    }
    let mut boxes = grown_round_pins(model, boxes, moved);
    // … and takes the size written
    for (z, _, _, size) in zones {
        if let (Some([w, h]), Some(b)) = (size, boxes.get_mut(z)) {
            b.w = *w;
            b.h = *h;
        }
    }
    let elk = Placement {
        boxes: boxes.clone(),
        ..Placement::default()
    };
    warnings.extend(pin_overlaps(model, scene, &boxes, &elk));
    let titles = crate::labels::zone_titles(model, scene, &boxes);
    let (anchored, obstacles) = crate::route::anchors(model, scene, &boxes, &titles);
    let params = crate::grid::params(model);
    let mode = crate::route::Mode::Grid;
    let (mut routes, rw, edge_groups) =
        route_all(model, &anchored, &elk, &params, mode, &[], &obstacles);
    warnings.extend(rw);
    let labels = label_all(model, scene, &boxes, &mut routes, sizes, &edge_groups, true);
    Arranged {
        label_lines: d.label_lines,
        elk,
        boxes,
        timeline: None,
        frise: None,
        routes,
        edge_groups,
        labels,
    }
}

/// The slide's boxes with each moved card where it is pinned, and the zones holding it grown
/// round it (their margin, and the room above their content, kept).
fn grown_round_pins(
    model: &Model,
    boxes: BTreeMap<String, Box2>,
    moved: &[(String, f64, f64)],
) -> BTreeMap<String, Box2> {
    const MARGIN: f64 = 30.0;
    // the room each zone keeps above what it holds (its title), read on the slide as drawn
    let above_of: BTreeMap<String, f64> = model
        .nodes
        .iter()
        .filter(|z| z.kind == Kind::Zone)
        .filter_map(|z| {
            let f = boxes.get(&z.id)?;
            let top = model
                .children(Some(&z.id))
                .filter_map(|c| boxes.get(&c.id))
                .map(|b| b.y)
                .reduce(f64::min)?;
            Some((z.id.clone(), top - f.y))
        })
        .collect();
    let mut boxes = boxes;
    for (id, x, y) in moved {
        let Some(b) = boxes.get_mut(id) else {
            continue;
        };
        b.x = *x;
        b.y = *y;
        let mut inner = *b;
        let mut at = model.node(id).and_then(|n| n.parent.clone());
        while let Some(z) = at {
            let Some(f) = boxes.get_mut(&z) else {
                break;
            };
            // the room above the zone's content (its title) is kept above the card too
            let above = above_of.get(&z).copied().unwrap_or(MARGIN);
            let (x0, y0) = (f.x.min(inner.x - MARGIN), f.y.min(inner.y - above));
            let x1 = f.right().max(inner.right() + MARGIN);
            let y1 = f.bottom().max(inner.bottom() + MARGIN);
            *f = Box2 {
                x: x0,
                y: y0,
                w: x1 - x0,
                h: y1 - y0,
            };
            inner = *f;
            at = model.node(&z).and_then(|n| n.parent.clone());
        }
    }
    boxes
}

/// What the pins make overlap (Q7, D-033: reported, never corrected): cards over one another,
/// and a zone grown over a card that is not its own (which it would hide).
fn pin_overlaps(
    model: &Model,
    scene: &Scene,
    boxes: &BTreeMap<String, Box2>,
    elk: &Placement,
) -> Vec<crate::EngineWarning> {
    // Q7 (D-033): what a pin makes overlap is reported, never corrected
    let cards: BTreeMap<String, Box2> = boxes
        .iter()
        .filter(|(id, _)| model.node(id).is_some_and(|n| n.kind == Kind::Card))
        .map(|(id, b)| (id.clone(), *b))
        .collect();
    let (pad, head) = scene.zone_pad_head();
    let mut out: Vec<crate::EngineWarning> =
        crate::layout::suggest_separation(model, &cards, elk, pad, head)
            .into_iter()
            .map(|(m, by)| overlap_warning(model, &m, &by))
            .collect();
    // a zone grown round a moved card over a card that is not its own (which it would hide)
    let within = |c: &str, z: &str| {
        let mut p = model.node(c).and_then(|n| n.parent.as_deref());
        while let Some(id) = p {
            if id == z {
                return true;
            }
            p = model.node(id).and_then(|n| n.parent.as_deref());
        }
        false
    };
    for z in model.nodes.iter().filter(|n| n.kind == Kind::Zone) {
        let Some(f) = boxes.get(&z.id) else { continue };
        for (c, b) in &cards {
            let meet = b.x < f.right() && b.right() > f.x && b.y < f.bottom() && b.bottom() > f.y;
            if meet && !within(c, &z.id) {
                out.push(overlap_warning(model, &z.id, c));
            }
        }
    }
    out
}

/// The arrow labels of `routes`; the grid engine then moves the lines that run through a label
/// to a lane beside it (`grid::labelled`). Shared by the full computation and the editing
/// session, as `route::finish` is.
pub(crate) fn label_all(
    model: &Model,
    scene: &Scene,
    boxes: &BTreeMap<String, Box2>,
    routes: &mut BTreeMap<String, crate::route::Points>,
    sizes: &Sizes,
    groups: &BTreeMap<String, String>,
    grid: bool,
) -> BTreeMap<String, (f64, f64)> {
    let place = |routes: &BTreeMap<String, crate::route::Points>| {
        crate::labels::place(model, scene, boxes, routes, sizes, groups)
    };
    if !grid {
        return place(routes);
    }
    let titles = crate::labels::zone_titles(model, scene, boxes);
    let around = crate::grid::Around::new(model, boxes, &titles, groups, &sizes.labels);
    crate::grid::labelled(&around, routes, &place)
}

/// The engine of a diagram drawn by the slide or the grid engine, and its model as that engine
/// takes it (a legend, a paragraph and a link outside the flow docked, D-105); `None`: the mode
/// its `layout` names. A diagram the slide engine does not read yet keeps ELK when it named no
/// layout, and gets the grid when it asked for `slide` by name. Arrows between a docked element
/// and the rest are not drawn (FORMAT §3.2.9).
fn automatic_engine(model: Model) -> (Model, Option<Layout>) {
    let docked = |m: Model| {
        let m = if crate::grid::on("grid-dock") {
            crate::dock::dock_defaults(m)
        } else {
            m
        };
        crate::dock::without_crossing(m)
    };
    if slide_asked(&model) {
        let d = docked(model.clone());
        if crate::slide::reads(&d) {
            return (d, Some(Layout::Slide(false)));
        }
        if model.option("layout") == Some("slide") {
            return (d, Some(Layout::Grid));
        }
        return (crate::dock::without_crossing(model), None);
    }
    if grid_asked(&model) {
        return (docked(model), Some(Layout::Grid));
    }
    (crate::dock::without_crossing(model), None)
}

/// Does the diagram ask for the grid engine? By name (`layout="grid"`); a diagram that names no
/// layout gets it when the environment says `SKEMO_LAYOUT=grid`; the slide engine hands it the
/// diagrams it does not read.
fn grid_asked(model: &Model) -> bool {
    static DEFAULT: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    match model.option("layout") {
        Some(l) => l == "grid",
        None => *DEFAULT.get_or_init(|| std::env::var("SKEMO_LAYOUT").is_ok_and(|v| v == "grid")),
    }
}

/// Does the slide engine draw the diagram? It is the default (the user's choice, 2026-11-18): a
/// diagram that names no layout gets it, unless `SKEMO_LAYOUT` names another engine (`graph`,
/// ELK; `grid`); `layout="slide"` asks for it by name.
fn slide_asked(model: &Model) -> bool {
    match model.option("layout") {
        Some(l) => l == "slide",
        None => {
            std::env::var("SKEMO_LAYOUT").map_or(true, |v| !matches!(v.as_str(), "graph" | "grid"))
        }
    }
}

/// Is the diagram laid out by one of the automatic engines (graph, grid, slide), which the
/// properties of `quality` are counted on?
pub(crate) fn automatic(model: &Model) -> bool {
    matches!(
        model.option("layout"),
        None | Some("graph" | "grid" | "slide")
    )
}

/// Mode asked for; an unknown mode falls back on automatic placement, with a warning. The scale
/// of the dated modes is computed here; the frieze's "outside" eras will be once the scene is
/// measured (`build_scene`).
fn choose_layout(model: &Model, warnings: &mut Vec<crate::EngineWarning>) -> Layout {
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
            warnings.push(crate::EngineWarning::new(
                "unknown-layout",
                tr!(
                    "unknown mode `{other}`: automatic placement is used",
                    "mode « {other} » inconnu : placement automatique utilisé"
                ),
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
        Layout::Graph | Layout::Grid | Layout::Slide(_) => Default::default(),
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
fn decode_images(
    opts: &Options,
    warnings: &mut Vec<crate::EngineWarning>,
) -> Vec<crate::images::Image> {
    let mut out = Vec::new();
    for (name, data) in &opts.images {
        match crate::images::check_name(name)
            .and_then(|()| crate::images::decode(name, data.clone()))
        {
            Ok(i) => out.push(i),
            Err(e) => warnings.push(crate::EngineWarning::new("image-type", e)),
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
    warnings: &mut Vec<crate::EngineWarning>,
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
        Layout::Graph | Layout::Grid | Layout::Slide(_) => {
            // D-031: the layout and every tidy pass see the cards as if none were pinned; the
            // pinned positions apply after, so pinning a card where it stands changes nothing
            let free = crate::layout::unpinned(model);
            let grid = matches!(layout, Layout::Grid);
            crate::tidy::before_elk(&free, sizes);
            let elk = if grid {
                crate::grid::place(&free, scene, sizes, (pad, head))
            } else {
                let mut elk =
                    elk_layout(&free, sizes, pad, head).map_err(|e| Error::Internal(e.0))?;
                crate::tidy::after_elk(&free, scene, sizes, (pad, head), &mut elk);
                // D-102: the corridors a first routing found too narrow for their lines
                crate::corridors::widen(&free, &mut elk, &sizes.corridors);
                let across = crate::corridors::across(&free);
                crate::corridors::widen_on(&free, &mut elk, &sizes.corridors_across, !across);
                elk
            };
            let (mut boxes, moved) =
                crate::layout::final_boxes_report(model, sizes, &elk, pad, head);
            if grid {
                crate::grid::frames(model, sizes, &elk, &mut boxes, (pad, head));
            }
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
                mode: if grid {
                    crate::route::Mode::Grid
                } else {
                    crate::route::Mode::Graph
                },
                params: if grid {
                    crate::grid::params(model)
                } else {
                    crate::route::graph_params(model)
                },
            }
        }
    })
}

fn wrap_warning(h: &crate::tidy::Hidden) -> crate::EngineWarning {
    let (zone, wrap) = (&h.zone, h.wrap);
    let (cards, from) = (h.cards.join(", "), h.from.join(", "));
    let message = tr!(
        "zone `{zone}`: with wrap=\"{wrap}\", {cards} sit behind other cards of the zone, so the arrows from {from} go around them; remove wrap or change dir (`skemo lint --fit` compares the options)",
        "zone « {zone} » : avec wrap=\"{wrap}\", {cards} sont derrière d'autres cartes de la zone et les flèches venant de {from} doivent les contourner ; retirer wrap ou changer dir (`skemo lint --fit` compare les options)"
    );
    crate::EngineWarning::new("wrap-hides-cards", message).on(zone.clone())
}

fn overlap_warning(model: &Model, moved: &str, by: &str) -> crate::EngineWarning {
    let what = if model.node(moved).is_some_and(|n| n.kind == Kind::Zone) {
        tr!("zone", "zone")
    } else {
        tr!("card", "carte")
    };
    let message = tr!(
        "{what} `{moved}` overlaps `{by}` (position pinned by at=): move one of the two",
        "{what} « {moved} » chevauche « {by} » (position figée par at=) : déplacer l'une des deux"
    );
    crate::EngineWarning::new("overlap", message).on(moved)
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
                "engine": if self.timeline.is_some() { "timeline" } else if self.frise.is_some() { "frise" } else if self.slide { "slide" } else if self.grid { "grid" } else { "elk" },
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
