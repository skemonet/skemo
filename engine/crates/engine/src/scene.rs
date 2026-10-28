//! Scene: the tree of rendered elements (FORMAT §4.2), styled by `lang`.
//!
//! We reproduce the prototype's structure (`buildDOM`): `.stage` holds, flat, the cards, the zones,
//! the arrow labels and the SVG of the arrows. The style of each element serves the measurement
//! (geometry) and the rendering (concrete colours).

use crate::icons::{self, Icon};
use skemo_lang::style::{
    Computed, DEFAULT_THEME, MediaContext, StyleTree, Styler, Stylesheet, Theme,
};
use skemo_lang::{Kind, Model, tr};
use std::collections::BTreeMap;

/// "Header" look (old: type in the band, name below) or "band" look (name in the band) of a card.
#[derive(Debug, Clone)]
pub struct HeadEl {
    /// `true`: "band" look (the name is in the band, no title below).
    pub band: bool,
    pub head: usize,
    pub type_el: usize,
    pub type_text: String,
    pub tag: Option<(usize, String)>,
    /// The category above the name in the band look (`label=` or `--label`, FORMAT §3.2.6).
    pub kind: Option<(usize, String)>,
}

impl CardEl {
    /// The name is in the band: no title under the header.
    pub fn band(&self) -> bool {
        self.header.as_ref().is_some_and(|h| h.band)
    }
}

/// A line `|` of a card: a possible leading IP (`span.mono`), then the rest of the text; or one row
/// of a table (FORMAT §3.2.5). One per line of the model, in order (animation keys `li[k]`).
#[derive(Debug, Clone, Default)]
pub struct LiEl {
    pub el: usize,
    pub mono: Option<(usize, String)>,
    pub rest: String,
    /// Place of the line in the card: consecutive lines that are never visible together share
    /// one (`slots`).
    pub slot: usize,
    /// Emphasis (FORMAT §3.2.4): the text in pieces, each with the element of its style
    /// (`strong`, `em`, `code`, `mark` under the line); empty for a plain line (`mono`, `rest`).
    pub parts: Vec<(String, usize)>,
    /// A mark before the line: the state classes `.ok`, `.warn`, `.error` (§3.2.7).
    pub icon: Option<(usize, Icon)>,
    /// A table row: its cells, and the table it belongs to (`CardEl::tables`).
    pub cells: Vec<CellEl>,
    pub table: Option<usize>,
    /// A separator row (`|---|`): nothing drawn, no room.
    pub rule: bool,
    /// In a legend (`card.legend`, FORMAT §3.2.8): the sample of the class the line names.
    pub sample: Option<Sample>,
    /// The line as written (emphasis delimiters included) and its classes, for the HTML.
    pub text: String,
    pub classes: Vec<String>,
}

/// The sample before a legend's line.
#[derive(Debug, Clone, Copy)]
pub enum Sample {
    /// An arrow class: `g.edge.<class>` with its `.line` and `.head` (colour, stroke, dashes).
    Arrow { g: usize, line: usize, head: usize },
    /// A card type: `span.swatch.<class>`, filled with the type's accent.
    Chip(usize),
    /// A class with an icon (`--icon`): `svg.icon.sample-icon.<class>`, in the type's accent.
    Icon(usize, Icon),
}

/// A cell of a table row: `td` (or `th` in a header row) and its text in pieces.
#[derive(Debug, Clone)]
pub struct CellEl {
    pub el: usize,
    pub col: usize,
    pub colspan: usize,
    pub rowspan: usize,
    pub parts: Vec<(String, usize)>,
    /// The cell as written, and its classes, for the HTML.
    pub text: String,
    pub classes: Vec<String>,
}

/// A table of a card (FORMAT §3.2.5): `ul > li.table > table`.
#[derive(Debug, Clone)]
pub struct TableEl {
    /// The `li.table` that holds it in the card's list.
    pub host: usize,
    pub el: usize,
    pub cols: usize,
    pub align: Vec<skemo_lang::Align>,
}

/// The mark of a state class (`.ok`, `.warn`, `.error`, FORMAT §3.2.7).
pub fn state_icon(classes: &[String]) -> Option<&'static str> {
    classes.iter().find_map(|c| match c.as_str() {
        "ok" => Some("circle-check"),
        "warn" => Some("triangle-alert"),
        "error" => Some("circle-x"),
        _ => None,
    })
}

/// Pieces of a text with emphasis: each piece gets the chain of elements of its style under
/// `parent` (`mark.class > strong > em > code`); a plain piece is styled by `parent` itself.
pub fn add_parts(tree: &mut StyleTree, parent: usize, text: &str) -> Vec<(String, usize)> {
    skemo_lang::inline::runs(text)
        .into_iter()
        .map(|r| {
            let mut at = parent;
            if let Some(cls) = &r.mark {
                let refs: Vec<&str> = cls.iter().map(String::as_str).collect();
                at = tree.add(Some(at), "mark", None, &refs);
            }
            if r.strong {
                at = tree.add(Some(at), "strong", None, &[]);
            }
            if r.em {
                at = tree.add(Some(at), "em", None, &[]);
            }
            if r.code {
                at = tree.add(Some(at), "code", None, &[]);
            }
            (r.text, at)
        })
        .collect()
}

/// `true` when the text has emphasis (more than one piece, or a styled one).
pub fn rich(text: &str) -> bool {
    let r = skemo_lang::inline::runs(text);
    r.len() > 1
        || r.first()
            .is_some_and(|r| r.strong || r.em || r.code || r.mark.is_some())
}

#[derive(Debug, Clone)]
pub struct CardEl {
    pub el: usize,
    pub header: Option<HeadEl>,
    pub title: usize,
    pub title_text: String,
    /// Badge (`.title::before`) and its text, if shown.
    pub badge: Option<(usize, String)>,
    /// The category on its own line above the title, when a card without a band has both a
    /// category and a tag (`div.kind`, FORMAT §3.2.6).
    pub kind: Option<(usize, String)>,
    pub ul: Option<usize>,
    pub lis: Vec<LiEl>,
    /// Tables of the card (rows are in `lis`).
    pub tables: Vec<TableEl>,
    /// Icon (FORMAT §3.2.1) and link mark: laid in the band (look `band`, `header` for the icon) or in
    /// the title.
    pub decor: Decor,
    /// `::after` of the card (diamond of a milestone, bar of a period…).
    pub after: usize,
    /// Where the badge goes (D-050): `.tag-left` (-1), `.tag-right` (1), the look's place (0).
    pub tag_side: i8,
    /// `cards` with a `count`: the `×N` badge (`.count`) and its text.
    pub count: Option<(usize, String)>,
}

/// `.tag-left` → -1, `.tag-right` → 1, otherwise 0 (D-050). Read on the element's own classes:
/// a zone's choice does not reach its cards.
pub fn tag_side(n: &skemo_lang::Node) -> i8 {
    if n.classes.iter().any(|c| c == "tag-right") {
        1
    } else if n.classes.iter().any(|c| c == "tag-left") {
        -1
    } else {
        0
    }
}

/// Additions of the placement modes, laid **before** the measurement (pitfall P17): classes and
/// inline declarations per node.
#[derive(Debug, Clone, Default)]
pub struct Extras {
    pub classes: BTreeMap<String, Vec<String>>,
    pub inline: BTreeMap<String, Vec<(String, String)>>,
    /// Image files usable as icons (D-046).
    pub images: Vec<crate::images::Image>,
    /// Classes used on arrows: a legend's line naming one shows an arrow (FORMAT §3.2.8).
    pub arrows: std::collections::BTreeSet<String>,
    /// Classes used on zones and never on cards: a legend's line naming one shows a small frame.
    pub zones: std::collections::BTreeSet<String>,
    /// Classes a legend's line names that set an icon (`--icon`): the line shows that icon.
    pub icons: BTreeMap<String, Icon>,
}

#[derive(Debug, Clone)]
pub struct ZoneEl {
    pub el: usize,
    pub title: usize,
    pub decor: Decor,
    /// The pill of `tag=` after the zone title (D-050): `zone > .title > .tag`.
    pub tag: Option<usize>,
    /// `.tag-right`: the pill at the right end of the title row (D-050).
    pub tag_right: bool,
    /// Description lines under the title (`|`, FORMAT §3.2.3): `.title > .sub`, plain text.
    pub subs: Vec<(usize, String)>,
}

/// The diagram's title (`title`, D-050): `.diagram-title > .text`, and one `li` per pill.
#[derive(Debug, Clone)]
pub struct TitleEl {
    pub el: usize,
    pub text: usize,
    pub lines: Vec<usize>,
    /// Subtitle lines (`|` under `title`, v0.8): `.diagram-title > .sub`.
    pub subs: Vec<usize>,
}

/// Icon and link mark of a node: the styled element of each (size, colour, margins come from the
/// CSS) and the drawing of the icon.
#[derive(Debug, Clone, Copy, Default)]
pub struct Decor {
    pub icon: Option<(usize, Icon)>,
    pub mark: Option<(usize, Icon)>,
}

/// Default values when the base theme declares neither `--icon-size` nor `--icon-gap`.
const ICON_SIZE: f64 = 16.0;
const ICON_GAP: f64 = 6.0;
const MARK_SIZE: f64 = 12.0;
const MARK_GAP: f64 = 4.0;

impl Scene {
    /// Width taken before the text by the icon (`size + gap`) and its size; `(0, 0)` without an icon.
    pub fn icon_lead(&self, d: &Decor) -> (f64, f64) {
        d.icon.as_ref().map_or((0.0, 0.0), |(e, _)| {
            let c = self.style(*e);
            let size = c.length("--icon-size").unwrap_or(ICON_SIZE);
            (size + c.length("--icon-gap").unwrap_or(ICON_GAP), size)
        })
    }

    /// Width of the link mark after the text (margin included) and its size.
    pub fn mark_trail(&self, d: &Decor) -> (f64, f64) {
        d.mark.as_ref().map_or((0.0, 0.0), |(e, _)| {
            let c = self.style(*e);
            let size = c.length("width").unwrap_or(MARK_SIZE);
            (size + c.margin()[3].max(0.0).max(MARK_GAP), size)
        })
    }
}

/// Resolves the icon of a node: `icon=` first, otherwise the style's `--icon`; `none` = none. An
/// unknown name draws nothing, reserves no room and adds a warning.
fn resolve_icon(
    n: &skemo_lang::Node,
    ps: &Computed,
    images: &[crate::images::Image],
    warnings: &mut Vec<String>,
) -> Option<Icon> {
    let name = n.icon.clone().or_else(|| ps.string("--icon"))?;
    if name == "none" {
        return None;
    }
    if crate::images::is_file(&name) {
        let found = images.iter().position(|i| i.name == name);
        if found.is_none() {
            warnings.push(tr!(
                "image « {name} » on « {} » was not provided: nothing drawn",
                "image « {name} » sur « {} » non fournie : rien n'est dessiné",
                n.id
            ));
        }
        return found.map(Icon::image);
    }
    let found = icons::get(&name);
    if found.is_none() {
        let mut msg = tr!(
            "unknown icon « {name} » on « {} »: nothing drawn",
            "icône « {name} » inconnue sur « {} » : rien n'est dessiné",
            n.id
        );
        if let Some(s) = icons::suggest(&name) {
            msg.push_str(&tr!(
                " (did you mean « {s} »?)",
                " (vouliez-vous « {s} » ?)"
            ));
        }
        warnings.push(msg);
    }
    found
}

/// Adds to `tree` the icon (before `host`) and the link mark (after) of a node.
fn add_decor(
    tree: &mut StyleTree,
    (icon_host, mark_host): (usize, usize),
    n: &skemo_lang::Node,
    ps: &Computed,
    images: &[crate::images::Image],
    warnings: &mut Vec<String>,
) -> Decor {
    let icon = resolve_icon(n, ps, images, warnings)
        .map(|i| (tree.add(Some(icon_host), "svg", None, &["icon"]), i));
    let hidden = ps.get("--link-mark").is_some_and(|v| v.trim() == "none");
    let mark = (n.href.is_some() && !hidden)
        .then(|| icons::get("external-link"))
        .flatten()
        .map(|i| (tree.add(Some(mark_host), "svg", None, &["link-mark"]), i));
    Decor { icon, mark }
}

#[derive(Debug, Clone)]
pub struct EdgeEl {
    pub g: usize,
    pub line: usize,
    pub head: usize,
    pub tail: usize,
    pub origin: usize,
}

pub struct Scene {
    pub tree: StyleTree,
    pub styles: Vec<Computed>,
    pub stage: usize,
    pub cards: BTreeMap<String, CardEl>,
    pub zones: BTreeMap<String, ZoneEl>,
    /// The diagram's title, if written (D-050).
    pub title: Option<TitleEl>,
    pub labels: BTreeMap<String, usize>,
    pub edges: BTreeMap<String, EdgeEl>,
    pub theme: Theme,
    /// Styled prototypes of the mode decorations (`tl-tick`, `tl-tick major`…), by classes.
    pub deco: BTreeMap<String, usize>,
    /// Warnings of the scene (`unknown-icon`).
    pub warnings: Vec<String>,
    /// Image files used as icons (`Icon::image` indexes them).
    pub images: Vec<crate::images::Image>,
}

/// Decorations of the timeline and frieze modes (the theme's classes, FORMAT §4.2).
pub const DECO: [&str; 14] = [
    "tl-axis",
    "tl-tick",
    "tl-tick major",
    "tl-grid",
    "tl-grid major",
    "tl-mark",
    "tl-today",
    "tl-today-label",
    "fr-axis",
    "fr-tick",
    "fr-tick-label",
    "fr-century",
    "fr-stem",
    "fr-dot",
];

/// IP (v4, optional CIDR) at the start of a line, followed by a blank or the end (F-STYLE-7).
pub fn leading_ip(s: &str) -> Option<usize> {
    let b = s.as_bytes();
    let mut i = 0;
    for part in 0..4 {
        let st = i;
        while i < b.len() && b[i].is_ascii_digit() && i - st < 3 {
            i += 1;
        }
        if i == st {
            return None;
        }
        if part < 3 {
            if b.get(i) != Some(&b'.') {
                return None;
            }
            i += 1;
        }
    }
    if b.get(i) == Some(&b'/') {
        let st = i + 1;
        let mut j = st;
        while j < b.len() && b[j].is_ascii_digit() {
            j += 1;
        }
        if j > st {
            i = j;
        }
    }
    match b.get(i) {
        None => Some(i),
        Some(c) if c.is_ascii_whitespace() => Some(i),
        _ => None,
    }
}

fn styler(base: &str, custom: &[String], theme: Theme) -> Styler {
    let mut sheets = vec![Stylesheet::parse(base)];
    sheets.extend(custom.iter().map(|c| Stylesheet::parse(c)));
    // `auto`: resolved as the light theme (the HTML will keep the system's switch)
    let _ = theme;
    Styler::new(sheets, MediaContext::default())
}

/// Racine commune : `html > body > div#stage.stage.theme-…[.look-…]`.
fn root(tree: &mut StyleTree, model: &Model, theme: Theme) -> usize {
    let html = tree.add(None, "html", None, &[]);
    let body = tree.add(Some(html), "body", None, &[]);
    let mut cls = vec!["stage".to_string(), theme.class()];
    // the layout mode, for the theme (`.stage.layout-timeline`, `.stage.layout-mindmap`)
    if let Some(layout) = model.option("layout") {
        cls.push(format!("layout-{layout}"));
    }
    if let Some(look) = model.option("look") {
        cls.push(format!("look-{look}"));
    }
    let refs: Vec<&str> = cls.iter().map(String::as_str).collect();
    tree.add(Some(body), "div", Some("stage"), &refs)
}

/// Elements alone, to read `--label` and `--look` (as the prototype, before creating the content).
/// Returns their styles and the index of each node.
fn probe_styles(
    styler: &Styler,
    model: &Model,
    theme: Theme,
) -> (Vec<Computed>, BTreeMap<String, usize>) {
    let mut probe = StyleTree::new();
    let pstage = root(&mut probe, model, theme);
    let mut pidx = BTreeMap::new();
    for n in &model.nodes {
        let cls: Vec<&str> = n.classes.iter().map(String::as_str).collect();
        let tag = if n.kind == Kind::Card { "card" } else { "zone" };
        pidx.insert(
            n.id.clone(),
            probe.add(Some(pstage), tag, Some(&n.id), &cls),
        );
    }
    (styler.compute(&probe), pidx)
}

/// Content of a card: header or label, title, lines, pseudo-elements. `ps`: style of the probe
/// (carries `--label` and `--look`).
/// A pile (`cards`, D-062): its class, and the `×N` badge when it has a count above 1.
fn add_deck(tree: &mut StyleTree, el: usize, n: &skemo_lang::Node) -> Option<(usize, String)> {
    if n.deck.is_some() {
        tree.add_class(el, "deck");
    }
    n.deck.and_then(|d| d.count).filter(|&c| c > 1).map(|c| {
        (
            tree.add(Some(el), "span", None, &["count"]),
            format!("×{c}"),
        )
    })
}

fn add_card(
    tree: &mut StyleTree,
    (el, lone): (usize, bool),
    n: &skemo_lang::Node,
    ps: &Computed,
    extras: &Extras,
    warnings: &mut Vec<String>,
) -> CardEl {
    for c in extras.classes.get(&n.id).into_iter().flatten() {
        tree.add_class(el, c);
    }
    for (k, v) in extras.inline.get(&n.id).into_iter().flatten() {
        tree.set_inline(el, k, v);
    }
    // the category (FORMAT §3.2.6): `label=`, else the type's `--label`; `label=""` shows none
    let type_label = n
        .label
        .clone()
        .unwrap_or_else(|| ps.string("--label").unwrap_or_default());
    let look = ps.string("--look").unwrap_or_default();
    // frieze events: their date becomes the badge when the theme asks for it (`--event-date: on`)
    let date = extras
        .inline
        .get(&n.id)
        .and_then(|v| v.iter().find(|(k, _)| k == "--date"))
        .map(|(_, v)| v.clone())
        .filter(|_| n.tag.is_none() && ps.get("--event-date").is_some_and(|v| v.trim() == "on"));
    // cards of the timeline and frieze modes: never with a header (their classes come from the mode)
    let mode_card = extras.classes.contains_key(&n.id);
    // a card with no accent of its own: the theme draws it light (`.no-type`)
    if ps.get("--c").is_none() {
        tree.add_class(el, "no-type");
    }
    let header = add_head(tree, el, n, (&type_label, &look, mode_card, date.as_ref()));
    // no band, a category and a tag: the category on its own line above the title, the tag in
    // the badge
    let kind =
        (header.is_none() && n.tag.is_some() && !type_label.is_empty() && !mode_card).then(|| {
            (
                tree.add(Some(el), "div", None, &["kind"]),
                type_label.clone(),
            )
        });
    let title = tree.add(Some(el), "div", None, &["title"]);
    let before = tree.add_pseudo(title, "before");
    let legend = n.classes.iter().any(|c| c == "legend").then_some((
        &extras.arrows,
        &extras.zones,
        &extras.icons,
    ));
    let (ul, lis, tables) = add_items(tree, el, &n.li, (&slots(&n.li), legend));
    if n.tag.is_none() && type_label.is_empty() && date.is_none() {
        tree.add_class(el, "no-tag");
    }
    // a card without lines: its title is centred (theme rules on `.no-body`)
    if lone {
        tree.add_class(el, "no-body");
    }
    let after = tree.add_pseudo(el, "after");
    let count = add_deck(tree, el, n);
    let hosts = match &header {
        Some(h) if h.band => (h.type_el, h.type_el),
        Some(h) => (h.type_el, title),
        None => (title, title),
    };
    let decor = add_decor(tree, hosts, n, ps, &extras.images, warnings);
    // the name is in the band: no title under the header
    let title_text = if header.as_ref().is_some_and(|h| h.band) {
        String::new()
    } else {
        n.title.clone()
    };
    CardEl {
        el,
        header,
        title,
        // the name is in the band: no title under the header
        title_text,
        badge: Some((before, String::new())),
        kind,
        ul,
        lis,
        tables,
        decor,
        after,
        tag_side: tag_side(n),
        count,
    }
}

/// The band (or the header) of a card in the `band` and `header` looks: category, name or type,
/// tag; otherwise the badge's text is set as `--label` (the tag, `label=`, a frieze date).
fn add_head(
    tree: &mut StyleTree,
    el: usize,
    n: &skemo_lang::Node,
    (type_label, look, mode_card, date): (&str, &str, bool, Option<&String>),
) -> Option<HeadEl> {
    let mut header = None;
    let band = look == "band";
    if (band || (look == "header" && (!type_label.is_empty() || n.tag.is_some()))) && !mode_card {
        tree.add_class(el, if band { "look-band" } else { "look-header" });
        let head = tree.add(Some(el), "div", None, &["head"]);
        // the band look: the category on a small line above the name, the tag in the chip
        let kind = (band && !type_label.is_empty()).then(|| {
            (
                tree.add(Some(head), "span", None, &["kind"]),
                type_label.to_string(),
            )
        });
        let type_el = tree.add(Some(head), "span", None, &["type"]);
        let pill = n.tag.clone();
        if let Some(t) = &n.tag {
            tree.set_attr(el, "data-tag", t);
        }
        let tag = pill.map(|t| (tree.add(Some(head), "span", None, &["tag"]), t));
        header = Some(HeadEl {
            band,
            head,
            type_el,
            type_text: if band {
                n.title.clone()
            } else {
                type_label.to_string()
            },
            tag,
            kind,
        });
    } else if let Some(t) = &n.tag {
        tree.set_attr(el, "data-tag", t);
        tree.set_inline(el, "--label", &serde_json::to_string(t).unwrap_or_default());
    } else if n.label.is_some() && date.is_none() {
        // `label=` without a tag: the badge (or the eyebrow line) shows the category
        tree.set_inline(
            el,
            "--label",
            &serde_json::to_string(type_label).unwrap_or_default(),
        );
    } else if let Some(d) = date {
        let label = if type_label.is_empty() {
            d.clone()
        } else {
            format!("{d} · {type_label}")
        };
        tree.set_inline(
            el,
            "--label",
            &serde_json::to_string(&label).unwrap_or_default(),
        );
    }
    header
}

/// A zone: its title, icon, `tag=` pill and description lines (`.title > .sub`, v0.8).
fn add_zone(
    tree: &mut StyleTree,
    stage: usize,
    n: &skemo_lang::Node,
    ps: &Computed,
    (images, warnings): (&[crate::images::Image], &mut Vec<String>),
) -> ZoneEl {
    let cls: Vec<&str> = n.classes.iter().map(String::as_str).collect();
    let el = tree.add(Some(stage), "zone", Some(&n.id), &cls);
    let title = tree.add(Some(el), "div", None, &["title"]);
    let decor = add_decor(tree, (title, title), n, ps, images, warnings);
    let tag = n
        .tag
        .as_ref()
        .map(|_| tree.add(Some(title), "span", None, &["tag"]));
    let subs =
        n.li.iter()
            .filter(|l| l.cells.is_empty() && l.rule.is_empty())
            .map(|l| {
                let cls: Vec<&str> = std::iter::once("sub")
                    .chain(l.classes.iter().map(String::as_str))
                    .collect();
                (
                    tree.add(Some(title), "div", None, &cls),
                    skemo_lang::inline::plain(&l.text),
                )
            })
            .collect();
    ZoneEl {
        el,
        title,
        decor,
        tag,
        tag_right: tag_side(n) == 1,
        subs,
    }
}

/// A card without lines, whose title is centred (D-047): not one stacked in a column (a
/// `wrap="1"` zone, a stack) with cards that have lines, which keeps its column's alignment
/// (D-056).
pub fn lone_title(model: &Model, n: &skemo_lang::Node) -> bool {
    if !n.items.is_empty() {
        return false;
    }
    let column = n
        .parent
        .as_deref()
        .and_then(|p| model.node(p))
        .is_some_and(|p| p.stack || p.wrap == Some(1));
    !column
        || model
            .children(n.parent.as_deref())
            .all(|c| c.kind != Kind::Card || c.items.is_empty())
}

/// List `ul > li` of a card; the IP at the start of a line goes into a `span.mono`.
/// Place of each `li` (D-045): a line shares the place of the previous one when its visibility
/// range, as written (`@0-2`, `@3`), overlaps none of the lines already there, so that a line
/// replaced by another keeps its place. Inheritance only narrows ranges: two lines disjoint as
/// written are never visible together.
pub fn slots(lis: &[skemo_lang::Li]) -> Vec<usize> {
    let range = |li: &skemo_lang::Li| {
        let a = li.anim.as_ref();
        (
            a.and_then(|a| a.start.as_ref()).map_or(0.0, |s| s.value),
            a.and_then(|a| a.until.as_ref())
                .map_or(f64::INFINITY, |u| u.value),
        )
    };
    let mut out: Vec<usize> = Vec::with_capacity(lis.len());
    let mut current: Vec<(f64, f64)> = Vec::new();
    for li in lis {
        let (a, b) = range(li);
        let apart = !current.is_empty() && current.iter().all(|&(c, d)| b < c || d < a);
        if apart {
            current.push((a, b));
        } else {
            current = vec![(a, b)];
        }
        let slot = match out.last() {
            Some(&k) if apart => k,
            Some(&k) => k + 1,
            None => 0,
        };
        out.push(slot);
    }
    out
}

fn add_items(
    tree: &mut StyleTree,
    card: usize,
    li: &[skemo_lang::Li],
    (slots, legend): (&[usize], Option<Kinds>),
) -> (Option<usize>, Vec<LiEl>, Vec<TableEl>) {
    if li.is_empty() {
        return (None, Vec::new(), Vec::new());
    }
    let ul = tree.add(Some(card), "ul", None, &[]);
    let mut lis: Vec<LiEl> = Vec::with_capacity(li.len());
    let mut tables = Vec::new();
    for b in skemo_lang::table::blocks(li) {
        match b {
            skemo_lang::table::Block::Line(k) => {
                let slot = slots.get(k).copied().unwrap_or(k);
                let mut l = line_el(tree, ul, &li[k], slot);
                if let Some(kinds) = legend {
                    l.sample = sample(tree, l.el, &li[k].classes, kinds);
                }
                lis.push(l);
            }
            skemo_lang::table::Block::Table(t) => {
                let host = tree.add(Some(ul), "li", None, &["table"]);
                let el = tree.add(Some(host), "table", None, &[]);
                let ti = tables.len();
                // one element per source line: rows, and the separator (drawn as nothing)
                let first = lis.len();
                let n = li[first..]
                    .iter()
                    .take_while(|l| !l.cells.is_empty() || !l.rule.is_empty())
                    .count();
                for k in first..first + n {
                    let slot = slots.get(k).copied().unwrap_or(k);
                    lis.push(row_el(tree, el, (&t, k, ti), slot));
                }
                tables.push(TableEl {
                    host,
                    el,
                    cols: t.cols,
                    align: t.align.clone(),
                });
            }
        }
    }
    (Some(ul), lis, tables)
}

/// A text line: `li` with the line's classes (`li.pill`, D-050; `li.warn`), its state mark, its
/// leading address in `span.mono`, or its pieces of emphasis.
fn line_el(tree: &mut StyleTree, ul: usize, l: &skemo_lang::Li, slot: usize) -> LiEl {
    let cls: Vec<&str> = l.classes.iter().map(String::as_str).collect();
    let li = tree.add(Some(ul), "li", None, &cls);
    let icon = state_icon(&l.classes)
        .and_then(icons::get)
        .map(|i| (tree.add(Some(li), "svg", None, &["icon", "state-mark"]), i));
    let it = &l.text;
    let mut out = LiEl {
        el: li,
        slot,
        icon,
        text: l.text.clone(),
        classes: l.classes.clone(),
        ..LiEl::default()
    };
    if rich(it) {
        out.parts = add_parts(tree, li, it);
        out.rest = skemo_lang::inline::plain(it);
        return out;
    }
    let it = skemo_lang::inline::plain(it);
    match leading_ip(&it) {
        Some(k) => {
            let mono = tree.add(Some(li), "span", None, &["mono"]);
            out.mono = Some((mono, it[..k].to_string()));
            out.rest = it[k..].to_string();
        }
        None => out.rest = it,
    }
    out
}

/// The extras with the classes a legend's samples need: those of the arrows, of the zones only,
/// and the icons the named classes set.
fn legend_kinds(styler: &Styler, model: &Model, theme: Theme, extras: &Extras) -> Extras {
    let mut extras = extras.clone();
    extras.arrows = model
        .edges
        .iter()
        .flat_map(|e| e.classes.iter().cloned())
        .collect();
    extras.icons = legend_icons(styler, model, theme);
    extras.zones = model
        .nodes
        .iter()
        .flat_map(|n| n.classes.iter())
        .filter(|c| zone_class(model, c))
        .cloned()
        .collect();
    extras
}

/// Is `c` a zone type: a class some zone carries and no card does?
pub fn zone_class(model: &Model, c: &str) -> bool {
    let on = |k: Kind| {
        model
            .nodes
            .iter()
            .any(|n| n.kind == k && n.classes.iter().any(|x| x == c))
    };
    on(Kind::Zone) && !on(Kind::Card)
}

/// The classes of the arrows, and of the zones only, of a diagram (what a legend's sample shows).
type Kinds<'a> = (
    &'a std::collections::BTreeSet<String>,
    &'a std::collections::BTreeSet<String>,
    &'a BTreeMap<String, Icon>,
);

/// The icon each class a legend's line names sets (`--icon`), resolved before the tree is
/// styled, as an automatic legend's labels are.
fn legend_icons(styler: &Styler, model: &Model, theme: Theme) -> BTreeMap<String, Icon> {
    let named: Vec<&String> = model
        .nodes
        .iter()
        .filter(|n| n.classes.iter().any(|c| c == "legend"))
        .flat_map(|n| n.li.iter().filter_map(|l| l.classes.first()))
        .collect();
    let mut probe = StyleTree::new();
    let stage = root(&mut probe, model, theme);
    let els: Vec<usize> = named
        .iter()
        .map(|c| probe.add(Some(stage), "card", None, &[c.as_str()]))
        .collect();
    let st = styler.compute(&probe);
    named
        .into_iter()
        .zip(els)
        .filter_map(|(c, el)| {
            let name = st[el].string("--icon")?;
            icons::get(&name).map(|i| (c.clone(), i))
        })
        .collect()
}

/// The sample of a legend's line: an arrow for a class used on arrows, a small frame for a zone
/// type, a chip for any other class (a card type), none for a state class (its mark is the
/// line's) or a line without class.
fn sample(
    tree: &mut StyleTree,
    li: usize,
    classes: &[String],
    (arrows, zones, with_icon): Kinds,
) -> Option<Sample> {
    let c = classes.first()?;
    if arrows.contains(c) {
        let g = tree.add(Some(li), "g", None, &["edge", c, "sample"]);
        return Some(Sample::Arrow {
            g,
            line: tree.add(Some(g), "path", None, &["line"]),
            head: tree.add(Some(g), "path", None, &["head"]),
        });
    }
    if state_icon(classes).is_some() {
        return None;
    }
    if let Some(i) = with_icon.get(c) {
        let el = tree.add(Some(li), "svg", None, &["icon", "sample-icon", c]);
        return Some(Sample::Icon(el, *i));
    }
    let swatch: &[&str] = if zones.contains(c) {
        &["swatch", "zone-sample", c]
    } else {
        &["swatch", c]
    };
    Some(Sample::Chip(tree.add(Some(li), "span", None, swatch)))
}

/// An automatic legend (a `card.legend` without lines): one line per class used in the diagram
/// that has a `--label`, arrow classes first, then card classes, in order of first use.
fn legend_lines(styler: &Styler, model: &Model, theme: Theme) -> Vec<skemo_lang::Li> {
    let auto = model
        .nodes
        .iter()
        .any(|n| n.li.is_empty() && n.classes.iter().any(|c| c == "legend"));
    if !auto {
        return Vec::new();
    }
    let mut seen: Vec<(String, bool)> = Vec::new();
    for e in &model.edges {
        for c in &e.classes {
            if !seen.iter().any(|(s, _)| s == c) {
                seen.push((c.clone(), true));
            }
        }
    }
    for n in &model.nodes {
        for c in &n.classes {
            if !seen.iter().any(|(s, _)| s == c) {
                seen.push((c.clone(), false));
            }
        }
    }
    let mut probe = StyleTree::new();
    let stage = root(&mut probe, model, theme);
    let els: Vec<usize> = seen
        .iter()
        .map(|(c, arrow)| {
            if *arrow {
                probe.add(Some(stage), "g", None, &["edge", c])
            } else {
                probe.add(Some(stage), "card", None, &[c])
            }
        })
        .collect();
    let st = styler.compute(&probe);
    seen.iter()
        .zip(els)
        .filter_map(|((c, _), el)| {
            let label = st[el].string("--label").filter(|l| !l.is_empty())?;
            Some(skemo_lang::Li {
                text: label,
                classes: vec![c.clone()],
                ..skemo_lang::Li::default()
            })
        })
        .collect()
}

/// One line of a table: a row (`tr`, its cells `th` in a header row, `td` otherwise) or the
/// separator.
fn row_el(
    tree: &mut StyleTree,
    table: usize,
    (t, k, ti): (&skemo_lang::table::Table, usize, usize),
    slot: usize,
) -> LiEl {
    let Some((r, row)) = t.rows.iter().enumerate().find(|(_, r)| r.li == k) else {
        return LiEl {
            el: table,
            slot,
            table: Some(ti),
            rule: true,
            ..LiEl::default()
        };
    };
    let tr = tree.add(Some(table), "tr", None, &[]);
    let tag = if t.header && r == 0 { "th" } else { "td" };
    let cells = row
        .cells
        .iter()
        .map(|c| {
            let cls: Vec<&str> = c.classes.iter().map(String::as_str).collect();
            let el = tree.add(Some(tr), tag, None, &cls);
            tree.set_attr(el, "data-col", &c.col.to_string());
            CellEl {
                el,
                col: c.col,
                colspan: c.colspan,
                rowspan: c.rowspan,
                parts: add_parts(tree, el, &c.text),
                text: c.text.clone(),
                classes: c.classes.clone(),
            }
        })
        .collect();
    LiEl {
        el: tr,
        slot,
        cells,
        table: Some(ti),
        ..LiEl::default()
    }
}

/// One `<g>` per arrow (classes of the arrow and its ends): line, heads, origin dot.
fn add_edges(
    tree: &mut StyleTree,
    svg: usize,
    model: &Model,
    extras: &Extras,
) -> BTreeMap<String, EdgeEl> {
    let mut edges = BTreeMap::new();
    // the classes of a node: its own, then those of the layout mode (`mm-b2`, `phase`…)
    let classes = |id: &str| {
        let mut c: Vec<String> = model
            .node(id)
            .map(|n| n.classes.clone())
            .unwrap_or_default();
        c.extend(extras.classes.get(id).cloned().unwrap_or_default());
        c
    };
    for e in &model.edges {
        let mut cls: Vec<String> = vec!["edge".into()];
        cls.extend(e.classes.iter().cloned());
        cls.extend(classes(&e.from).iter().map(|c| format!("from-{c}")));
        cls.extend(classes(&e.to).iter().map(|c| format!("to-{c}")));
        let refs: Vec<&str> = cls.iter().map(String::as_str).collect();
        let g = tree.add(Some(svg), "g", None, &refs);
        tree.set_attr(g, "data-id", &e.id);
        edges.insert(
            e.id.clone(),
            EdgeEl {
                g,
                line: tree.add(Some(g), "path", None, &["line"]),
                head: tree.add(Some(g), "path", None, &["head"]),
                tail: tree.add(Some(g), "path", None, &["tail"]),
                origin: tree.add(Some(g), "circle", None, &["origin"]),
            },
        );
    }
    edges
}

/// Badge: shown if the `content` of the `::before` is a non-empty string.
fn resolve_badges(cards: &mut BTreeMap<String, CardEl>, styles: &[Computed]) {
    for c in cards.values_mut() {
        if let Some((b, _)) = c.badge {
            c.badge = styles[b]
                .string("content")
                .filter(|s| !s.is_empty())
                .map(|s| (b, s));
        }
    }
}

impl Scene {
    /// Scene with the default theme.
    pub fn build(model: &Model, custom_css: &[String], theme: Theme) -> Scene {
        Self::build_with_base(model, DEFAULT_THEME, custom_css, theme)
    }

    /// Scene with a given base theme (`--base-theme`).
    pub fn build_with_base(
        model: &Model,
        base: &str,
        custom_css: &[String],
        theme: Theme,
    ) -> Scene {
        Self::build_full(model, base, custom_css, theme, &Extras::default())
    }

    /// Full scene, with the additions of a placement mode.
    pub fn build_full(
        model: &Model,
        base: &str,
        custom_css: &[String],
        theme: Theme,
        extras: &Extras,
    ) -> Scene {
        let styler = styler(base, custom_css, theme);
        let (pst, pidx) = probe_styles(&styler, model, theme);
        let extras = legend_kinds(&styler, model, theme, extras);
        let legend = legend_lines(&styler, model, theme);
        let extras = &extras;

        // arbre complet
        let mut tree = StyleTree::new();
        let stage = root(&mut tree, model, theme);
        let mut cards = BTreeMap::new();
        let mut zones = BTreeMap::new();
        let mut warnings = Vec::new();
        for n in &model.nodes {
            let cls: Vec<&str> = n.classes.iter().map(String::as_str).collect();
            if n.kind == Kind::Zone {
                let ps = &pst[pidx[&n.id]];
                let z = add_zone(&mut tree, stage, n, ps, (&extras.images, &mut warnings));
                zones.insert(n.id.clone(), z);
            } else {
                let ps = &pst[pidx[&n.id]];
                let el = tree.add(Some(stage), "card", Some(&n.id), &cls);
                // an automatic legend: its lines come from the classes used (FORMAT §3.2.8)
                let auto =
                    (n.li.is_empty() && n.classes.iter().any(|c| c == "legend")).then(|| {
                        let mut a = n.clone();
                        a.li.clone_from(&legend);
                        a.items = legend.iter().map(|l| l.text.clone()).collect();
                        a
                    });
                let n = auto.as_ref().unwrap_or(n);
                cards.insert(
                    n.id.clone(),
                    add_card(
                        &mut tree,
                        (el, lone_title(model, n) && auto.is_none()),
                        n,
                        ps,
                        extras,
                        &mut warnings,
                    ),
                );
            }
        }
        let svg = tree.add(Some(stage), "svg", None, &["edges"]);
        let edges = add_edges(&mut tree, svg, model, extras);
        let mut deco = BTreeMap::new();
        for d in DECO {
            let cls: Vec<&str> = d.split(' ').collect();
            deco.insert(d.to_string(), tree.add(Some(stage), "div", None, &cls));
        }
        let title = model.title.as_ref().map(|t| {
            let el = tree.add(Some(stage), "div", None, &["diagram-title"]);
            TitleEl {
                el,
                text: tree.add(Some(el), "div", None, &["text"]),
                lines: t
                    .lines
                    .iter()
                    .map(|_| tree.add(Some(el), "li", None, &["pill"]))
                    .collect(),
                subs: t
                    .sub
                    .iter()
                    .map(|_| tree.add(Some(el), "div", None, &["sub"]))
                    .collect(),
            }
        });
        let mut labels = BTreeMap::new();
        // a label belongs to its arrow: `.edge.ok { color }` tints it (D-049)
        for e in model.edges.iter().filter(|e| !e.label.is_empty()) {
            let host = edges.get(&e.id).map_or(stage, |el| el.g);
            labels.insert(
                e.id.clone(),
                tree.add(Some(host), "div", None, &["edge-label"]),
            );
        }
        let styles = styler.compute(&tree);
        resolve_badges(&mut cards, &styles);
        Scene {
            tree,
            styles,
            stage,
            cards,
            zones,
            title,
            labels,
            edges,
            theme,
            deco,
            warnings,
            images: extras.images.clone(),
        }
    }

    pub fn style(&self, i: usize) -> &Computed {
        &self.styles[i]
    }

    /// Margins of the zones (`--zone-pad`, `--zone-head`), read on `.stage`.
    pub fn zone_pad_head(&self) -> (f64, f64) {
        let s = self.style(self.stage);
        let px = |n: &str, d: f64| {
            s.vars
                .get(n)
                .and_then(|v| skemo_lang::style::parse_length(v, 14.0, None))
                .unwrap_or(d)
        };
        (px("--zone-pad", 22.0), px("--zone-head", 30.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ip() {
        assert_eq!(leading_ip("10.0.0.1/24"), Some(11));
        assert_eq!(leading_ip("10.10.0.0/16 VLAN 10"), Some(12));
        assert_eq!(leading_ip("10.0.0.1x"), None);
        assert_eq!(leading_ip("~400 postes"), None);
    }
}
