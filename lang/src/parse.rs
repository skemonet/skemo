//! Semantics: tree of tags → model, with validation (FORMAT §3) and a map of positions.

use crate::anim;
use crate::diag::{Diagnostic, Span, suggest};
use crate::model::{Anim, Deck, Edge, Join, Kind, Li, Model, Node, Step, Title, number};
use crate::syntax::{Attr, Tag, parse_tree};
use crate::table::{RowKind, glued_classes, read_row};
use crate::tr;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

const RESERVED: [&str; 19] = [
    "id",
    "class",
    "at",
    "size",
    "via",
    "step",
    "fx",
    "label",
    "from",
    "side",
    "tag",
    "date",
    "start",
    "end",
    "join",
    "icon",
    "href",
    "wrap",
    "constraint",
];
const SIDES: [&str; 4] = ["top", "bottom", "left", "right"];
/// Compass points of a docked element (`at="s"`, FORMAT §3.2.9).
pub const COMPASS: [&str; 8] = ["n", "s", "e", "w", "ne", "nw", "se", "sw"];
/// Attributes of the prototype no longer read (FORMAT §7).
const REMOVED: [&str; 3] = ["engine", "router", "elk"];

/// Positions of an element in the source (the base of surgical editing, L3).
#[derive(Debug, Clone, PartialEq)]
pub struct Located {
    /// Whole line, without the indentation.
    pub line: Span,
    pub indent: usize,
    pub name: Span,
    pub id: Option<Span>,
    pub classes: Vec<(String, Span)>,
    pub attr_group: Option<Span>,
    pub attrs: Vec<Attr>,
    /// Text shown (title, label, line), markers excluded.
    pub text: Option<Span>,
    pub markers: Vec<Span>,
}

impl Located {
    fn of(t: &Tag, text: Option<Span>, markers: Vec<Span>) -> Self {
        Located {
            line: t.line,
            indent: t.indent,
            name: t.name_span,
            id: t.id.as_ref().map(|(_, s)| *s),
            classes: t.classes.clone(),
            attr_group: t.attr_group,
            attrs: t.attrs.clone(),
            text,
            markers,
        }
    }
    pub fn attr(&self, name: &str) -> Option<&Attr> {
        self.attrs.iter().find(|a| a.name == name)
    }
}

/// Positions of every element, by id.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SourceMap {
    pub diagram: Option<Located>,
    pub nodes: BTreeMap<String, Located>,
    /// A tag `to(a b)` gives two arrows: each points to the same tag.
    pub edges: BTreeMap<String, Located>,
    /// Lines `|` (and old `li`) of cards and zones: (id of the card or zone, rank).
    pub lis: BTreeMap<(String, usize), Located>,
}

/// Result of the analysis: model (as good as it can be, even with errors), diagnostics, positions.
#[derive(Debug, Clone)]
pub struct Document {
    pub model: Model,
    pub diagnostics: Vec<Diagnostic>,
    pub map: SourceMap,
}

impl Document {
    pub fn has_errors(&self) -> bool {
        self.diagnostics.iter().any(Diagnostic::is_error)
    }
    pub fn errors(&self) -> impl Iterator<Item = &Diagnostic> {
        self.diagnostics.iter().filter(|d| d.is_error())
    }
    pub fn warnings(&self) -> impl Iterator<Item = &Diagnostic> {
        self.diagnostics.iter().filter(|d| !d.is_error())
    }
}

struct Ctx {
    model: Model,
    map: SourceMap,
    diags: Vec<Diagnostic>,
    auto: usize,
    /// Arrow targets, checked at the end: (target, position, id of the arrow).
    targets: Vec<(String, Span, String)>,
    id_spans: BTreeMap<String, Span>,
    /// Named joins (`join="http"`, D-050), checked at the end: (edge id, position of `join=`).
    joins: Vec<(String, Span)>,
}

/// Analyse un fichier de diagramme. Ne panique jamais.
pub fn parse(src: &str) -> Document {
    let mut diags = Vec::new();
    let tree = parse_tree(src, &mut diags);
    let mut cx = Ctx {
        model: Model::default(),
        map: SourceMap::default(),
        diags,
        auto: 0,
        targets: Vec::new(),
        id_spans: BTreeMap::new(),
        joins: Vec::new(),
    };
    for t in &tree {
        walk(&mut cx, t, None);
    }
    lonely_joins(&mut cx);
    docked_arrows(&mut cx);
    // cibles inconnues
    let ids: BTreeSet<String> = cx.model.nodes.iter().map(|n| n.id.clone()).collect();
    let cards: Vec<&str> = cx.model.nodes.iter().map(|n| n.id.as_str()).collect();
    for (target, span, _) in &cx.targets {
        if !ids.contains(target) {
            let mut d = Diagnostic::error(
                "unknown-target",
                *span,
                tr!("unknown target « {target} »", "cible « {target} » inconnue"),
            );
            if let Some(s) = suggest(target, cards.iter().copied()) {
                d = d.with_help(tr!("did you mean « {s} »?", "vouliez-vous « {s} » ?"));
            }
            cx.diags.push(d);
        }
    }
    validate_modes(&mut cx);
    cx.diags.sort_by_key(|d| (d.span.start, d.span.end));
    Document {
        model: cx.model,
        diagnostics: cx.diags,
        map: cx.map,
    }
}

fn attr<'a>(t: &'a Tag, name: &str) -> Option<&'a Attr> {
    t.attrs.iter().find(|a| a.name == name)
}

fn value<'a>(t: &'a Tag, name: &str) -> Option<&'a str> {
    attr(t, name).and_then(|a| a.value.as_deref())
}

fn classes(t: &Tag) -> Vec<String> {
    let mut c: Vec<String> = t.classes.iter().map(|(c, _)| c.clone()).collect();
    for a in t.attrs.iter().filter(|a| a.name == "class") {
        c.extend(
            a.value
                .iter()
                .flat_map(|v| v.split_whitespace().map(String::from)),
        );
    }
    c
}

/// Lit un point `x,y`.
fn point(s: &str) -> Option<[f64; 2]> {
    let (x, y) = s.split_once(',')?;
    let x: f64 = x.trim().parse().ok()?;
    let y: f64 = y.trim().parse().ok()?;
    (x.is_finite() && y.is_finite()).then_some([x, y])
}

fn point_attr(cx: &mut Ctx, t: &Tag, name: &str, what: &str) -> Option<[f64; 2]> {
    let a = attr(t, name)?;
    if let Some(p) = a.value.as_deref().and_then(point) {
        Some(p)
    } else {
        cx.diags.push(
            Diagnostic::error(
                "bad-number",
                a.span,
                tr!(
                    "« {name} » is malformed: {what} expected",
                    "« {name} » mal formé : {what} attendu"
                ),
            )
            .with_help(tr!(
                "example: {name}=\"{}\"",
                "exemple : {name}=\"{}\"",
                if name == "size" { "300,200" } else { "120,40" }
            )),
        );
        None
    }
}

/// Title (or label): the line's text, markers removed.
fn text_of(cx: &mut Ctx, t: &Tag) -> (String, Option<Anim>, Option<Span>, Vec<Span>) {
    let (s, anim, span, markers) = match &t.text {
        Some((txt, sp)) => {
            let m = anim::read(txt, sp.start, &mut cx.diags);
            let span = Span::new(sp.start, sp.start + m.text.len().min(txt.len()));
            (m.text, m.anim, Some(span), m.markers)
        }
        None => (String::new(), None, None, Vec::new()),
    };
    (s.trim().to_string(), anim, span, markers)
}

/// `tag="8080 @1"`: the chip text and its own step markers, read like the end of a line
/// (FORMAT §3.8).
fn tag_of(cx: &mut Ctx, t: &Tag) -> (Option<String>, Option<Anim>) {
    let Some(a) = attr(t, "tag") else {
        return (None, None);
    };
    let Some(v) = a.value.as_deref() else {
        return (None, None);
    };
    let base = a.value_span.map_or(a.span.start, |s| s.start + 1);
    let m = anim::read(v, base, &mut cx.diags);
    (Some(m.text), m.anim)
}

/// Old notation `step=` / `fx=` merged with the `@` markers.
fn legacy_anim(cx: &mut Ctx, t: &Tag, anim: Option<Anim>, is_link: bool) -> Option<Anim> {
    let step = attr(t, "step");
    let fx = attr(t, "fx");
    let mut anim = anim;
    if step.is_some() || fx.is_some() {
        if anim.as_ref().is_some_and(|a| a.start.is_some()) {
            let sp = step.or(fx).map_or(t.line, |a| a.span);
            cx.diags.push(
                Diagnostic::error(
                    "legacy-and-marker",
                    sp,
                    tr!(
                        "« step=/fx= » and an « @ » marker on the same element",
                        "« step=/fx= » et un marqueur « @ » sur le même élément"
                    ),
                )
                .with_help(tr!(
                    "keep only the @ marker",
                    "garder seulement le marqueur @"
                )),
            );
        } else {
            let sp = step.or(fx).map_or(t.line, |a| a.span);
            cx.diags.push(Diagnostic::warning("legacy-step", sp, tr!("old « step=/fx= » notation: prefer the « @N:effect » marker at the end of the line", "ancienne notation « step=/fx= » : préférer le marqueur « @N:effet » en fin de ligne")));
            let a = anim.get_or_insert_with(Anim::default);
            if let Some(s) = step.and_then(|s| s.value.as_deref()) {
                match s.parse::<f64>() {
                    Ok(v) if v >= 0.0 && v.is_finite() => {
                        a.start = Some(Step {
                            raw: s.to_string(),
                            value: v,
                        });
                    }
                    _ => cx.diags.push(Diagnostic::error(
                        "bad-number",
                        step.map_or(t.line, |s| s.span),
                        tr!(
                            "« step » is malformed: a number is expected",
                            "« step » mal formé : nombre attendu"
                        ),
                    )),
                }
            }
            if let Some(f) = fx.and_then(|f| f.value.as_deref()) {
                a.fx = Some(f.to_string());
            }
        }
    }
    if let Some(a) = anim.as_mut()
        && a.fx.as_deref() == Some("draw")
        && !is_link
    {
        cx.diags.push(Diagnostic::warning(
            "draw-on-node",
            t.line,
            tr!(
                "the « draw » effect only applies to links: « fade » is used",
                "l'effet « draw » ne s'applique qu'aux liens : « fade » est utilisé"
            ),
        ));
        a.fx = Some("fade".into());
    }
    anim
}

fn compat_step(anim: &Option<Anim>) -> (Option<serde_json::Number>, Option<String>) {
    match anim {
        Some(a) => (a.start.as_ref().map(|s| number(s.value)), a.fx.clone()),
        None => (None, None),
    }
}

fn declare_id(cx: &mut Ctx, id: &str, span: Span) -> bool {
    if let Some(first) = cx.id_spans.get(id) {
        let _ = first;
        cx.diags.push(
            Diagnostic::error(
                "duplicate-id",
                span,
                tr!(
                    "identifier « {id} » is already used",
                    "identifiant « {id} » déjà utilisé"
                ),
            )
            .with_help(tr!(
                "every card and zone needs a unique #id",
                "chaque carte et chaque zone doit avoir un #id unique"
            )),
        );
        false
    } else {
        cx.id_spans.insert(id.to_string(), span);
        true
    }
}

fn walk(cx: &mut Ctx, t: &Tag, parent: Option<&str>) {
    match t.name.as_str() {
        "diagram" => {
            diagram_options(cx, t);
            cx.map.diagram = Some(Located::of(t, None, Vec::new()));
            for c in &t.children {
                if c.name == "title" {
                    title(cx, c);
                } else {
                    walk(cx, c, None);
                }
            }
        }
        "card" | "cards" | "zone" | "stack" => node(cx, t, parent),
        "title" => cx.diags.push(Diagnostic::warning(
            "title-misplaced",
            t.name_span,
            tr!(
                "« title » is written directly under « diagram »: ignored",
                "« title » s'écrit directement sous « diagram » : ignoré"
            ),
        )),
        "to" => match parent.and_then(|p| cx.model.node(p)).map(|n| n.kind) {
            Some(Kind::Card) => edges(cx, t, parent.unwrap_or_default()),
            _ => cx.diags.push(
                Diagnostic::error(
                    "to-outside-card",
                    t.name_span,
                    tr!(
                        "« to » must be written inside its source card",
                        "« to » doit être écrit dans sa carte de départ"
                    ),
                )
                .with_help(tr!(
                    "indent to(...) under the card the arrow starts from",
                    "indenter to(...) sous la carte d'où part la flèche"
                )),
            ),
        },
        "li" | "|" => cx.diags.push(Diagnostic::warning(
            "line-misplaced",
            t.name_span,
            tr!(
                "a line belongs under a card, a zone or the title: ignored",
                "une ligne s'écrit sous une carte, une zone ou le titre : ignorée"
            ),
        )),
        other => cx.diags.push(
            Diagnostic::warning(
                "unknown-tag",
                t.name_span,
                tr!(
                    "unknown tag « {other} »: ignored with its content",
                    "balise « {other} » inconnue : ignorée avec son contenu"
                ),
            )
            .with_help(tr!(
                "known tags: diagram, title, card, cards, zone, stack, to; a line starts with « | »",
                "balises reconnues : diagram, title, card, cards, zone, stack, to ; une ligne commence par « | »"
            )),
        ),
    }
}

fn node(cx: &mut Ctx, t: &Tag, parent: Option<&str>) {
    let kind = if t.name == "card" || t.name == "cards" {
        Kind::Card
    } else {
        Kind::Zone
    };
    if let Some(p) = parent.and_then(|p| cx.model.node(p))
        && p.kind == Kind::Card
    {
        cx.diags.push(Diagnostic::error(
            "nested-in-card",
            t.name_span,
            tr!(
                "a card can only contain lines « | » and « to »: « {} » is ignored",
                "une carte ne peut contenir que des lignes « | » et des « to » : « {} » est ignoré",
                t.name
            ),
        ));
        return;
    }
    let explicit =
        t.id.as_ref()
            .map(|(i, s)| (i.clone(), *s))
            .or_else(|| attr(t, "id").and_then(|a| a.value.clone().map(|v| (v, a.span))));
    let id = if let Some((id, span)) = explicit {
        if !declare_id(cx, &id, span) {
            return;
        }
        id
    } else {
        cx.auto += 1;
        let id = format!("{}{}", t.name, cx.auto);
        cx.id_spans.insert(id.clone(), t.name_span);
        id
    };
    let (mut title, anim, text_span, markers) = text_of(cx, t);
    let stack = t.name == "stack";
    if stack && !title.is_empty() {
        cx.diags.push(Diagnostic::warning(
            "stack-text",
            text_span.unwrap_or(t.name_span),
            tr!(
                "a « stack » has no title: « {title} » is ignored",
                "un « stack » n'a pas de titre : « {title} » est ignoré"
            ),
        ));
        title.clear();
    }
    let anim = legacy_anim(cx, t, anim, false);
    let (at, dock) = at_attr(cx, t, parent);
    let size = point_attr(cx, t, "size", "« largeur,hauteur »");
    let (step, fx) = compat_step(&anim);
    let href = attr(t, "href").and_then(|a| parse_href(cx, a));
    let (tag, tag_anim) = tag_of(cx, t);
    let wrap = attr(t, "wrap").and_then(|a| parse_wrap(cx, a, kind));
    let deck = deck_of(cx, t);
    cx.map
        .nodes
        .insert(id.clone(), Located::of(t, text_span, markers));
    let idx = cx.model.nodes.len();
    cx.model.nodes.push(Node {
        id: id.clone(),
        kind,
        parent: parent.map(String::from),
        classes: classes(t),
        title,
        tag,
        label: value(t, "label").map(|v| v.trim().to_string()),
        dock,
        tag_anim,
        items: Vec::new(),
        at,
        size,
        step,
        fx,
        date: value(t, "date").map(String::from),
        start: value(t, "start").map(String::from),
        end: value(t, "end").map(String::from),
        anim,
        li: Vec::new(),
        icon: icon_attr(t),
        href,
        wrap,
        stack,
        deck,
    });
    for c in &t.children {
        if matches!(c.name.as_str(), "li" | "|") && !stack {
            node_line(cx, c, (idx, &id));
        } else {
            walk(cx, c, Some(&id));
        }
    }
    check_tables(cx, idx, &id);
}

/// A line of card or zone `id` (index `idx` in the model): `| …`, or the old `li …` (§7).
fn node_line(cx: &mut Ctx, c: &Tag, (idx, id): (usize, &str)) {
    let (li, loc) = read_line(cx, c);
    let rank = cx.model.nodes[idx].li.len();
    cx.map.lis.insert((id.to_string(), rank), loc);
    cx.model.nodes[idx].items.push(li.text.clone());
    cx.model.nodes[idx].li.push(li);
}

/// Reads a line: its text, markers and classes, or its cells when it is a table row.
fn read_line(cx: &mut Ctx, c: &Tag) -> (Li, Located) {
    if c.name == "li" {
        let (text, anim, span, markers) = text_of(cx, c);
        let anim = legacy_anim(cx, c, anim, false);
        if let Some(a) = attr(c, "icon") {
            warn_icon_ignored(cx, a);
        }
        let li = Li {
            text,
            anim,
            classes: classes(c),
            ..Li::default()
        };
        return (li, Located::of(c, span, markers));
    }
    let (raw, sp) = c.text.clone().unwrap_or_default();
    let m = anim::read(&raw, sp.start, &mut cx.diags);
    // the written text up to the markers, for the span of what is shown
    let end = m.markers.first().map_or(sp.end, |s| s.start);
    let written = raw[..end - sp.start].trim_end();
    let mut loc = Located::of(c, None, m.markers.clone());
    let mut li = Li {
        anim: m.anim,
        ..Li::default()
    };
    match read_row(&m.text) {
        Some(RowKind::Cells(cells)) => {
            li.text = m.text.trim().to_string();
            li.cells = cells;
        }
        Some(RowKind::Rule(al)) => {
            li.text = m.text.trim().to_string();
            li.rule = al;
        }
        None => {
            let (cls, rest) = glued_classes(&m.text);
            let mut at = sp.start;
            for k in &cls {
                loc.classes
                    .push((k.clone(), Span::new(at + 1, at + 1 + k.len())));
                at += 1 + k.len();
            }
            li.classes = cls;
            li.text = rest.trim().to_string();
        }
    }
    // the shown text starts after the classes and the space that ends them
    let rest = if li.cells.is_empty() && li.rule.is_empty() {
        glued_classes(written).1
    } else {
        written
    };
    let start = sp.start + written.len() - rest.trim_start().len();
    let end = sp.start + written.len();
    loc.text = (start < end).then(|| Span::new(start, end));
    (li, loc)
}

/// Warnings on the tables of a card (`bad-table`, FORMAT §3.2.5).
fn check_tables(cx: &mut Ctx, idx: usize, id: &str) {
    let li = cx.model.nodes[idx].li.clone();
    let row = |l: &Li| !l.cells.is_empty() || !l.rule.is_empty();
    let mut k = 0;
    while k < li.len() {
        if !row(&li[k]) {
            k += 1;
            continue;
        }
        let n = li[k..].iter().take_while(|l| row(l)).count();
        let warn = |cx: &mut Ctx, at: usize, msg: String| {
            let span = cx
                .map
                .lis
                .get(&(id.to_string(), at))
                .map_or_else(Span::default, |l| l.line);
            cx.diags.push(Diagnostic::warning("bad-table", span, msg));
        };
        for (j, l) in li[k..k + n].iter().enumerate() {
            let at = k + j;
            if !l.rule.is_empty() {
                if j != 1 {
                    warn(
                        cx,
                        at,
                        tr!(
                            "a separator row « |---| » only counts right after the first row of a table: ignored",
                            "une ligne de séparation « |---| » ne compte que juste après la première rangée d'un tableau : ignorée"
                        ),
                    );
                } else if l.rule.len() != li[k].cells.len() {
                    warn(
                        cx,
                        at,
                        tr!(
                            "the separator row has {} columns, the header {}",
                            "la ligne de séparation a {} colonnes, l'en-tête {}",
                            l.rule.len(),
                            li[k].cells.len()
                        ),
                    );
                }
                continue;
            }
            if j == 0 && l.cells.iter().any(|c| c.join == Some(Join::Up)) {
                warn(
                    cx,
                    at,
                    tr!(
                        "« ^^ » in the first row of a table: there is no cell above",
                        "« ^^ » dans la première rangée d'un tableau : aucune cellule au-dessus"
                    ),
                );
            }
            if l.cells.first().is_some_and(|c| c.join == Some(Join::Left)) {
                warn(
                    cx,
                    at,
                    tr!(
                        "a row starts with « || »: there is no cell on the left to join",
                        "une rangée commence par « || » : aucune cellule à gauche à joindre"
                    ),
                );
            }
        }
        let lonely = n == 1 && li[k].cells.len() == 1;
        let text_beside = (k > 0 && !row(&li[k - 1])) || li.get(k + 1).is_some_and(|l| !row(l));
        if lonely && text_beside {
            warn(
                cx,
                k,
                tr!(
                    "a table of one cell between lines of text: if this line only ends with « | », write « \\| »",
                    "un tableau d'une seule cellule entre des lignes de texte : si cette ligne finit seulement par « | », écrire « \\| »"
                ),
            );
        }
        k += n;
    }
}

/// `diagram(...)`: its attributes as options, written as they are; the prototype's `engine`,
/// `router` and `elk` are no longer read (FORMAT §7); the old axis bounds `from`/`to` are also
/// given as `start`/`end`.
fn diagram_options(cx: &mut Ctx, t: &Tag) {
    for a in &t.attrs {
        if a.name == "class" || a.value.is_none() {
            continue;
        }
        if REMOVED.contains(&a.name.as_str()) {
            cx.diags.push(Diagnostic::warning(
                "unknown-attribute",
                a.span,
                tr!(
                    "« {} » is no longer read: ignored",
                    "« {} » n'est plus lu : ignoré",
                    a.name
                ),
            ));
            continue;
        }
        let v = Value::String(a.value.clone().unwrap_or_default());
        cx.model.options.insert(a.name.clone(), v);
    }
    for (old, new) in [("from", "start"), ("to", "end")] {
        if let Some(v) = cx.model.options.get(old).cloned() {
            cx.model.options.entry(new.to_string()).or_insert(v);
        }
    }
}

/// `at="x,y"` (pinned) or `at="s"` (docked, top-level only); the compass point is returned apart.
fn at_attr(cx: &mut Ctx, t: &Tag, parent: Option<&str>) -> (Option<[f64; 2]>, Option<String>) {
    let Some(a) = attr(t, "at") else {
        return (None, None);
    };
    let v = a.value.clone().unwrap_or_default();
    if v.chars().any(|c| c.is_ascii_digit()) {
        return (point_attr(cx, t, "at", "« x,y »"), None);
    }
    let p = v.trim().to_ascii_lowercase();
    let msg = if !COMPASS.contains(&p.as_str()) {
        tr!(
            "« at=\"{v}\" »: coordinates « x,y » or a compass point (n s e w ne nw se sw) expected; ignored",
            "« at=\"{v}\" » : coordonnées « x,y » ou point cardinal (n s e w ne nw se sw) attendu ; ignoré"
        )
    } else if parent.is_some() {
        tr!(
            "« at=\"{p}\" » docks only a top-level element: ignored",
            "« at=\"{p}\" » n'ancre qu'un élément de premier niveau : ignoré"
        )
    } else {
        return (None, Some(p));
    };
    cx.diags.push(Diagnostic::warning("bad-at", a.span, msg));
    (None, None)
}

/// An arrow between a docked element and anything outside it is not drawn (`docked-arrow`).
fn docked_arrows(cx: &mut Ctx) {
    let top = |m: &Model, id: &str| -> Option<String> {
        let mut n = m.node(id)?;
        let mut seen = 0;
        while let Some(p) = n.parent.as_deref().and_then(|p| m.node(p)) {
            n = p;
            seen += 1;
            if seen > m.nodes.len() {
                break;
            }
        }
        n.dock.as_ref().map(|_| n.id.clone())
    };
    let mut warned: Vec<Span> = Vec::new();
    for e in &cx.model.edges {
        let (a, b) = (top(&cx.model, &e.from), top(&cx.model, &e.to));
        if a == b || cx.model.node(&e.to).is_none() {
            continue;
        }
        let span = cx
            .map
            .edges
            .get(&e.id)
            .map_or_else(Span::default, |l| l.line);
        if warned.contains(&span) {
            continue;
        }
        warned.push(span);
        cx.diags.push(Diagnostic::warning(
            "docked-arrow",
            span,
            tr!(
                "arrow « {} »: a docked element (at=\"s\"…) and the rest of the drawing are not linked; not drawn",
                "flèche « {} » : un élément ancré (at=\"s\"…) et le reste du dessin ne sont pas reliés ; non dessinée",
                e.id
            ),
        ));
    }
}

/// `title` under `diagram` (D-050): the first one counts; its lines `|` are subtitle lines, its
/// `|.pill` lines (and old `li`) its pills.
fn title(cx: &mut Ctx, t: &Tag) {
    if cx.model.title.is_some() {
        cx.diags.push(Diagnostic::warning(
            "title-twice",
            t.name_span,
            tr!(
                "a second « title »: ignored",
                "un deuxième « title » : ignoré"
            ),
        ));
        return;
    }
    let (text, _, _, _) = text_of(cx, t);
    let mut lines = Vec::new();
    let mut sub = Vec::new();
    for c in &t.children {
        if c.name == "li" {
            lines.push(text_of(cx, c).0);
        } else if c.name == "|" {
            let (li, _) = read_line(cx, c);
            if li.classes.iter().any(|k| k == "pill") {
                lines.push(li.text);
            } else {
                sub.push(li.text);
            }
        } else {
            cx.diags.push(Diagnostic::warning(
                "title-content",
                c.name_span,
                tr!(
                    "a « title » only contains lines « | »: « {} » is ignored",
                    "un « title » ne contient que des lignes « | » : « {} » est ignoré",
                    c.name
                ),
            ));
        }
    }
    cx.model.title = Some(Title { text, lines, sub });
}

fn icon_attr(t: &Tag) -> Option<String> {
    value(t, "icon")
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(String::from)
}

fn warn_icon_ignored(cx: &mut Ctx, a: &Attr) {
    cx.diags.push(Diagnostic::warning(
        "icon-ignored",
        a.span,
        tr!(
            "« icon » has no effect here: only cards and zones have an icon",
            "« icon » est sans effet ici : seules les cartes et les zones ont une icône"
        ),
    ));
}

/// URL schemes accepted for `href` (FORMAT §3.2.1).
const HREF_SCHEMES: [&str; 3] = ["http", "https", "mailto"];

/// `true` if `v` is a safe link: `http:`, `https:`, `mailto:`, a relative path or `#id`.
pub(crate) fn href_is_safe(v: &str) -> bool {
    let v = v.trim_start();
    if v.is_empty() || v.chars().any(char::is_control) {
        return false;
    }
    // a scheme is what precedes the first ":" when no "/ ? #" comes before it
    match v.find([':', '/', '?', '#']) {
        Some(i) if v.as_bytes()[i] == b':' => {
            HREF_SCHEMES.contains(&v[..i].to_ascii_lowercase().as_str())
        }
        _ => true,
    }
}

/// `href="…"`: refuses (error `unsafe-href`) a scheme outside the allow-list or an empty value.
fn parse_href(cx: &mut Ctx, a: &Attr) -> Option<String> {
    let v = a.value.as_deref().unwrap_or_default();
    if href_is_safe(v) {
        return Some(v.trim_start().to_string());
    }
    cx.diags.push(
        Diagnostic::error(
            "unsafe-href",
            a.span,
            tr!(
                "unsafe or empty link « {v} »",
                "lien non sûr ou vide « {v} »"
            ),
        )
        .with_help(tr!(
            "allowed: http:, https:, mailto:, a relative path or #id",
            "autorisés : http:, https:, mailto:, un chemin relatif ou #id"
        )),
    );
    None
}

/// `via="x,y x,y"`: waypoints; warns and returns `None` if malformed.
fn parse_via(cx: &mut Ctx, a: &Attr) -> Option<Vec<[f64; 2]>> {
    let v = a.value.as_deref().unwrap_or("");
    let pts: Option<Vec<[f64; 2]>> = v.split_whitespace().map(point).collect();
    match pts {
        Some(p) if !p.is_empty() => Some(p),
        _ => {
            cx.diags.push(
                Diagnostic::error(
                    "bad-number",
                    a.span,
                    tr!(
                        "« via » is malformed: a list of points « x,y x,y » is expected",
                        "« via » mal formé : liste de points « x,y x,y » attendue"
                    ),
                )
                .with_help(tr!(
                    "example: via=\"300,420 300,500\"",
                    "exemple : via=\"300,420 300,500\""
                )),
            );
            None
        }
    }
}

/// `side="start-end"`: imposed sides; ignored with a warning if invalid.
fn parse_side(cx: &mut Ctx, a: &Attr) -> Option<String> {
    let v = a.value.clone().unwrap_or_default();
    let ok = match v.split_once('-') {
        Some((s, e)) => [s, e].iter().all(|p| p.is_empty() || SIDES.contains(p)),
        None => SIDES.contains(&v.as_str()),
    };
    if ok {
        return Some(v);
    }
    cx.diags.push(Diagnostic::warning(
        "bad-side",
        a.span,
        tr!(
            "« side=\"{v}\" »: expected sides are top, bottom, left, right (e.g. \"left-top\", \"-top\"); ignored",
            "« side=\"{v}\" » : côtés attendus top, bottom, left, right (ex. \"left-top\", \"-top\") ; ignoré"
        ),
    ));
    None
}

/// `wrap="N"`: a whole number ≥ 1, on a zone only.
fn parse_wrap(cx: &mut Ctx, a: &Attr, kind: Kind) -> Option<u32> {
    let v = a.value.clone().unwrap_or_default();
    if kind != Kind::Zone {
        cx.diags.push(Diagnostic::warning(
            "bad-wrap",
            a.span,
            tr!(
                "« wrap » has no effect on a card: only a zone lays out its children in rows",
                "« wrap » est sans effet sur une carte : seule une zone range ses enfants en rangées"
            ),
        ));
        return None;
    }
    match v.trim().parse::<u32>() {
        Ok(n) if n >= 1 => Some(n),
        _ => {
            cx.diags.push(Diagnostic::warning(
                "bad-wrap",
                a.span,
                tr!(
                    "« wrap=\"{v}\" »: a whole number ≥ 1 is expected (children per row); ignored",
                    "« wrap=\"{v}\" » : nombre entier ≥ 1 attendu (enfants par rangée) ; ignoré"
                ),
            ));
            None
        }
    }
}

/// `cards` (a pile) and its `count="N"`; `count` on a `card` or a bad number: warning `bad-count`.
fn deck_of(cx: &mut Ctx, t: &Tag) -> Option<Deck> {
    let count = attr(t, "count").and_then(|a| {
        let v = a.value.clone().unwrap_or_default();
        let warn = |cx: &mut Ctx, msg: String| {
            cx.diags.push(Diagnostic::warning("bad-count", a.span, msg));
        };
        if t.name != "cards" {
            warn(
                cx,
                tr!(
                    "« count » is read on « cards » only (a pile of cards); ignored",
                    "« count » ne se lit que sur « cards » (un paquet de cartes) ; ignoré"
                ),
            );
            return None;
        }
        match v.trim().parse::<u32>() {
            Ok(n) if n >= 1 => Some(n),
            _ => {
                warn(
                    cx,
                    tr!(
                        "« count=\"{v}\" »: a whole number ≥ 1 is expected; ignored",
                        "« count=\"{v}\" » : nombre entier ≥ 1 attendu ; ignoré"
                    ),
                );
                None
            }
        }
    });
    (t.name == "cards").then_some(Deck { count })
}

/// `join="trunk"` : hyperarête (one source, a shared trunk out). Any other name (D-050): the
/// arrows to one card that carry it share a trunk in; checked by `lonely_joins`. An empty value
/// is ignored with a warning.
fn parse_join(cx: &mut Ctx, a: &Attr) -> Option<String> {
    let v = a.value.clone().unwrap_or_default();
    if !v.trim().is_empty() {
        return Some(v.trim().to_string());
    }
    cx.diags.push(Diagnostic::warning(
        "bad-join",
        a.span,
        tr!(
            "« join=\"\" »: a name is expected (\"trunk\", or one shared by arrows to one card); ignored",
            "« join=\"\" » : un nom est attendu (\"trunk\", ou un nom partagé par des flèches vers une même carte) ; ignoré"
        ),
    ));
    None
}

/// A named join that no other arrow to the same card carries joins nothing: warning `bad-join`,
/// and the join is dropped.
fn lonely_joins(cx: &mut Ctx) {
    let mut count: BTreeMap<(String, String), usize> = BTreeMap::new();
    for e in &cx.model.edges {
        if let Some(j) = e.join.as_ref().filter(|j| *j != "trunk") {
            *count.entry((j.clone(), e.to.clone())).or_default() += 1;
        }
    }
    let joins = std::mem::take(&mut cx.joins);
    // one warning per written `join=`, however many targets its `to(...)` has
    let mut warned: Vec<Span> = Vec::new();
    for (id, span) in joins {
        let Some(e) = cx.model.edges.iter_mut().find(|e| e.id == id) else {
            continue;
        };
        let Some(j) = e.join.clone().filter(|j| j != "trunk") else {
            continue;
        };
        if count.get(&(j.clone(), e.to.clone())).copied().unwrap_or(0) < 2 {
            let to = e.to.clone();
            e.join = None;
            if warned.contains(&span) {
                continue;
            }
            warned.push(span);
            cx.diags.push(
                Diagnostic::warning(
                    "bad-join",
                    span,
                    tr!(
                        "« join=\"{j}\" »: no other arrow to « {to} » carries it; ignored",
                        "« join=\"{j}\" » : aucune autre flèche vers « {to} » ne le porte ; ignoré"
                    ),
                )
                .with_help(tr!(
                    "\"trunk\" shares a trunk out of the source; another name, a trunk into a card shared by several arrows",
                    "\"trunk\" partage un tronc au départ ; un autre nom, un tronc d'arrivée partagé par plusieurs flèches"
                )),
            );
        }
    }
}

/// `constraint="false"` (or `"true"`, the default); any other value is ignored with a warning.
fn parse_constraint(cx: &mut Ctx, a: &Attr) -> bool {
    let v = a.value.clone().unwrap_or_default();
    match v.as_str() {
        "false" => return false,
        "true" => return true,
        _ => {}
    }
    cx.diags.push(Diagnostic::warning(
        "bad-constraint",
        a.span,
        tr!(
            "« constraint=\"{v}\" »: \"false\" or \"true\" expected; ignored",
            "« constraint=\"{v}\" » : \"false\" ou \"true\" attendu ; ignoré"
        ),
    ));
    true
}

fn edges(cx: &mut Ctx, t: &Tag, from: &str) {
    let (label, anim, text_span, markers) = text_of(cx, t);
    let anim = legacy_anim(cx, t, anim, true);
    let via = attr(t, "via").and_then(|a| parse_via(cx, a));
    let side = attr(t, "side").and_then(|a| parse_side(cx, a));
    let join_attr = attr(t, "join");
    let join = join_attr.and_then(|a| parse_join(cx, a));
    let constraint = attr(t, "constraint").is_none_or(|a| parse_constraint(cx, a));
    let href = attr(t, "href").and_then(|a| parse_href(cx, a));
    if let Some(a) = attr(t, "icon") {
        warn_icon_ignored(cx, a);
    }
    let (step, fx) = compat_step(&anim);
    let cls = classes(t);
    let targets: Vec<&Attr> = t
        .attrs
        .iter()
        .filter(|a| a.value.is_none() && !RESERVED.contains(&a.name.as_str()))
        .collect();
    if targets.is_empty() {
        cx.diags.push(
            Diagnostic::error(
                "no-target",
                t.name_span,
                tr!("arrow without a target", "flèche sans cible"),
            )
            .with_help(tr!(
                "write the target in parentheses: to(core)",
                "écrire la cible entre parenthèses : to(core)"
            )),
        );
    }
    let explicit = t.id.as_ref().map(|(i, _)| i.clone());
    for (k, a) in targets.iter().enumerate() {
        let base = match (&explicit, k) {
            (Some(i), 0) => i.clone(),
            (Some(i), _) => format!("{i}#{}", k + 1),
            (None, _) => format!("{from}->{}", a.name),
        };
        let mut id = base.clone();
        let mut n = 1;
        while cx.model.edges.iter().any(|e| e.id == id) {
            n += 1;
            id = format!("{base}#{n}");
        }
        cx.targets.push((a.name.clone(), a.name_span, id.clone()));
        if let (Some(j), Some(ja)) = (&join, join_attr)
            && j != "trunk"
        {
            cx.joins.push((id.clone(), ja.span));
        }
        cx.map
            .edges
            .insert(id.clone(), Located::of(t, text_span, markers.clone()));
        cx.model.edges.push(Edge {
            id,
            from: from.to_string(),
            to: a.name.clone(),
            classes: cls.clone(),
            label: label.clone(),
            via: via.clone(),
            side: side.clone(),
            join: join.clone(),
            constraint,
            href: href.clone(),
            step: step.clone(),
            fx: fx.clone(),
            anim: anim.clone(),
        });
    }
    for c in &t.children {
        cx.diags.push(Diagnostic::warning(
            "ignored-child",
            c.name_span,
            tr!(
                "content indented under « to »: ignored",
                "contenu indenté sous « to » : ignoré"
            ),
        ));
    }
}

/// Mode de placement par dates : timeline ou frise.
#[derive(Clone, Copy, PartialEq, Eq)]
enum DateMode {
    Timeline,
    Frieze,
}

impl DateMode {
    /// The mode's name in messages.
    fn name(self) -> String {
        match self {
            DateMode::Timeline => "timeline".to_string(),
            DateMode::Frieze => tr!("frieze", "frise"),
        }
    }

    fn parse_date(self, s: &str, end: bool) -> Result<f64, String> {
        match self {
            DateMode::Timeline => crate::dates::parse_timeline(s, end),
            DateMode::Frieze => crate::dates::parse_frise(s, end),
        }
    }
}

/// Timeline and frieze modes (F-PARSE-11, 12): dates required and well formed, `end ≥ start`,
/// `at=` without effect (warning); frieze: no year 0, no zone.
fn validate_modes(cx: &mut Ctx) {
    if cx.model.option("layout") == Some("mindmap") {
        validate_mindmap(cx);
        return;
    }
    let mode = match cx.model.option("layout") {
        Some("timeline") => DateMode::Timeline,
        // `frise`: the prototype's name, still read (its reference files use it)
        Some("frieze" | "frise") => DateMode::Frieze,
        _ => return,
    };
    validate_diagram_dates(cx, mode);
    let nodes = cx.model.nodes.clone();
    for n in &nodes {
        validate_node_dates(cx, mode, n);
    }
}

/// Mind-map mode (`FORMAT.md` §3.9): no zone, `at=` without effect (warning), `root` must name a
/// card.
fn validate_mindmap(cx: &mut Ctx) {
    let diagram_span = cx.map.diagram.as_ref().map(|d| d.line).unwrap_or_default();
    if let Some(root) = cx.model.option("root").map(String::from)
        && !cx
            .model
            .nodes
            .iter()
            .any(|n| n.kind == Kind::Card && n.id == root)
    {
        let span = cx
            .map
            .diagram
            .as_ref()
            .and_then(|d| d.attr("root"))
            .map_or(diagram_span, |a| a.span);
        cx.diags.push(Diagnostic::error(
            "bad-root",
            span,
            tr!(
                "« root=\"{root}\" »: no card with this identifier",
                "« root=\"{root}\" » : aucune carte de cet identifiant"
            ),
        ));
    }
    let nodes = cx.model.nodes.clone();
    for n in &nodes {
        let loc = cx.map.nodes.get(&n.id).cloned();
        let span_of = |name: &str| {
            loc.as_ref()
                .and_then(|l| l.attr(name))
                .map(|a| a.span)
                .or(loc.as_ref().map(|l| l.name))
                .unwrap_or_default()
        };
        if n.kind == Kind::Zone {
            cx.diags.push(
                Diagnostic::error(
                    "zone-in-mindmap",
                    span_of("id"),
                    tr!(
                        "zone « {} »: zones are not supported in mindmap mode",
                        "zone « {} » : les zones ne sont pas gérées en mode mindmap",
                        n.id
                    ),
                )
                .with_help(tr!(
                    "remove the zone: a mindmap's structure comes from the arrows",
                    "retirer la zone : la structure d'une mindmap vient des flèches"
                )),
            );
        } else if n.at.is_some() {
            cx.diags.push(Diagnostic::warning(
                "at-ignored",
                span_of("at"),
                tr!(
                    "« at » has no effect in mindmap mode: the position comes from the structure",
                    "« at » est sans effet en mode mindmap : la position vient de la structure"
                ),
            ));
        }
    }
}

/// Dates of the diagram's options (`start`, `end`, `today`; `from`/`to` are old names, §7).
fn validate_diagram_dates(cx: &mut Ctx, mode: DateMode) {
    let diagram_span = cx.map.diagram.as_ref().map(|d| d.line).unwrap_or_default();
    for opt in ["start", "end", "today"] {
        let Some(v) = cx.model.option(opt).map(String::from) else {
            continue;
        };
        if opt == "today" && mode == DateMode::Frieze {
            continue;
        }
        if let Err(m) = mode.parse_date(&v, opt == "end") {
            let old = if opt == "start" { "from" } else { "to" };
            let span = cx
                .map
                .diagram
                .as_ref()
                .and_then(|d| d.attr(opt).or_else(|| d.attr(old)))
                .map_or(diagram_span, |a| a.span);
            cx.diags.push(Diagnostic::error(
                "bad-date",
                span,
                tr!("« {opt} »: {m}", "« {opt} » : {m}"),
            ));
        }
    }
}

/// Date rules of a node: no nested zone in a frieze, dates present and valid, `end ≥ start`.
fn validate_node_dates(cx: &mut Ctx, mode: DateMode, n: &Node) {
    let loc = cx.map.nodes.get(&n.id).cloned();
    let span_of = |name: &str| {
        loc.as_ref()
            .and_then(|l| l.attr(name))
            .map(|a| a.span)
            .or(loc.as_ref().map(|l| l.name))
            .unwrap_or_default()
    };
    if n.kind == Kind::Zone {
        // frieze (D-045): a zone frames its events; a zone in a zone is not supported
        if mode == DateMode::Frieze && n.parent.is_some() {
            cx.diags.push(
                Diagnostic::error(
                    "zone-in-frise",
                    span_of("id"),
                    tr!(
                        "zone « {} »: a zone inside a zone is not supported in frieze mode",
                        "zone « {} » : une zone dans une zone n'est pas gérée en mode frise",
                        n.id
                    ),
                )
                .with_help(tr!(
                    "move it to the top level",
                    "la remonter au premier niveau"
                )),
            );
        }
        return;
    }
    if n.date.is_none() && n.start.is_none() {
        cx.diags.push(
            Diagnostic::error(
                "missing-date",
                span_of("id"),
                tr!(
                    "card « {} » has neither date= nor start= ({} mode)",
                    "carte « {} » sans date= ni start= (mode {})",
                    n.id,
                    mode.name()
                ),
            )
            .with_help(tr!(
                "date=\"2027-03-15\" for a milestone, start=\"…\" end=\"…\" for a period",
                "date=\"2027-03-15\" pour un jalon, start=\"…\" end=\"…\" pour une période"
            )),
        );
        return;
    }
    let start = n
        .start
        .as_deref()
        .and_then(|s| mode.parse_date(s, false).ok());
    for (name, v, end) in [
        ("date", &n.date, false),
        ("start", &n.start, false),
        ("end", &n.end, true),
    ] {
        let Some(v) = v else { continue };
        match mode.parse_date(v, end) {
            Ok(t) if name == "end" && start.is_some_and(|s0| t < s0) => {
                cx.diags.push(Diagnostic::error(
                    "end-before-start",
                    span_of("end"),
                    tr!(
                        "card « {} »: end is before start",
                        "carte « {} » : end antérieur à start",
                        n.id
                    ),
                ));
            }
            Ok(_) => {}
            Err(m) => cx.diags.push(Diagnostic::error(
                "bad-date",
                span_of(name),
                tr!("« {name} »: {m}", "« {name} » : {m}"),
            )),
        }
    }
    if n.at.is_some() {
        cx.diags.push(Diagnostic::warning(
            "at-ignored",
            span_of("at"),
            tr!(
                "« at » has no effect in {} mode: the position comes from the date",
                "« at » est sans effet en mode {} : la position vient de la date",
                mode.name()
            ),
        ));
    }
}

#[cfg(test)]
mod href_tests {
    use super::href_is_safe;

    #[test]
    fn whitelist() {
        for ok in [
            "https://a.b/c?d:e",
            "HTTP://a.b",
            "mailto:x@y.z",
            "docs/fw.md",
            "./a:b",
            "#core",
            "  https://a.b",
        ] {
            assert!(href_is_safe(ok), "{ok}");
        }
        for bad in [
            "",
            "   ",
            "javascript:alert(1)",
            "  JavaScript:x",
            "data:text/html,x",
            "vbscript:x",
            "file:///etc/passwd",
            "https://a\n.b",
            "java\tscript:x",
        ] {
            assert!(!href_is_safe(bad), "{bad:?}");
        }
    }
}
