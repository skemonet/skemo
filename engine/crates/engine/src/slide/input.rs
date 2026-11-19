//! The diagram as the slide engine reads it: one panel per zone, at any depth (a card outside every
//! zone is a panel of its own), the cards numbered in panel order, one module for every card,
//! labels wrapped.

use super::grid::Side;
use super::{STEP, labels, steps};
use crate::measure::Sizes;
use crate::scene::Scene;
use skemo_lang::{Kind, Model};
use std::collections::BTreeMap;

pub(crate) struct Panel {
    pub zone: Option<String>,
    /// The group's id (a zone, a stack, a bare zone), none for a card's own panel.
    pub id: Option<String>,
    /// Its own cards (a zone's cards in its sub-zones are theirs).
    pub cards: Vec<usize>,
    /// The zone (or stack) it sits in.
    pub parent: Option<usize>,
    /// A `stack` (D-050): no frame, its children one after the other in writing order along `dir`.
    pub stack: bool,
    /// A `.wide` card or zone: a row of its own across its group.
    pub wide: bool,
    /// `wrap="N"` on a zone: its children in writing order, at most N a row (a column across
    /// `dir`).
    pub wrap: Option<usize>,
    /// Where it is written (its node's place in the model).
    pub order: usize,
    /// Where its first own card is written.
    pub first: usize,
    /// `zones` (D-114): the room its frames behind take beyond its frame, right and down (px).
    pub pile: f64,
    /// Where its title and description stand.
    pub title: TitleAt,
    /// `at="n|s|e|w"` inside its zone or block, or at the drawing's edge with arrows (D-117): the
    /// band it takes round the rest, as Java's `BorderLayout` (a corner: its east or west side).
    pub dock: Option<Band>,
    /// `dir=` on a zone, a stack or a block (D-117): its children one after the other that way.
    pub dir: Option<(bool, bool)>,
    /// `block` (D-117): drawn as nothing, the other lines go round it.
    pub block: bool,
    /// `align=` (D-117): where its cards stand across its `dir` when their block is wider.
    pub align: Option<Align>,
}

/// Where a frame's title and description stand.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TitleAt {
    /// A band above its cards (the header).
    Above,
    /// A caption below its cards (`.bare`, or set so in CSS, D-050): no header text.
    Below,
    /// A column left of its cards (`head="left"`, D-117).
    Left,
}

impl Panel {
    /// Its title is a caption below its cards.
    pub(crate) fn below(&self) -> bool {
        self.title == TitleAt::Below
    }

    /// Its title is a column left of its cards.
    pub(crate) fn side(&self) -> bool {
        self.title == TitleAt::Left
    }
}

/// A band round the rest of a group (`at=` inside it, D-117).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Band {
    N,
    S,
    E,
    W,
}

/// Where cards stand across a group's `dir` (D-117).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Align {
    Start,
    Center,
    End,
}

/// The band of `at="…"` (`+n`: a docked element the slide engine lays in, D-117).
fn band(n: &skemo_lang::Node) -> Option<Band> {
    match n.dock.as_deref()?.trim_start_matches('+') {
        "n" => Some(Band::N),
        "s" => Some(Band::S),
        "e" | "ne" | "se" => Some(Band::E),
        "w" | "nw" | "sw" => Some(Band::W),
        _ => None,
    }
}

/// A container's `dir=`: (across, reverse).
fn dir_of(n: &skemo_lang::Node) -> Option<(bool, bool)> {
    match n.dir.as_deref()? {
        "right" => Some((true, false)),
        "left" => Some((true, true)),
        "up" => Some((false, true)),
        _ => Some((false, false)),
    }
}

fn align_of(n: &skemo_lang::Node) -> Option<Align> {
    match n.align.as_deref()? {
        "start" => Some(Align::Start),
        "end" => Some(Align::End),
        _ => Some(Align::Center),
    }
}

impl Panel {
    /// The step a pile of frames takes beyond the frame, right and down (0 or 1).
    pub(crate) fn pile_step(&self) -> i32 {
        i32::from(self.pile > 0.0 && self.zone.is_some())
    }
}

pub(crate) struct Edge {
    pub id: String,
    pub from: usize,
    pub to: usize,
    pub label: Option<String>,
    pub look: String,
    pub lsize: (f64, f64),
    /// `constraint="false"`: drawn, but no part of the placement.
    pub constraint: bool,
    /// `side="start-end"`: the sides it leaves and enters by, when written.
    pub sides: (Option<Side>, Option<Side>),
    /// `via="x,y …"`: the lattice nodes it runs through, in order.
    pub via: Vec<(i32, i32)>,
    /// `join="…"`: `trunk`, the arrows of one `to(…)` share their start; a name, the arrows
    /// into one card carrying it share their end.
    pub join: Option<String>,
    /// The zone (panel) an end is, when it is one: `from` and `to` are then a card of it, for
    /// the placement; the line leaves or enters the zone's frame.
    pub ends: (Option<usize>, Option<usize>),
}

/// The diagram as the slide engine reads it.
pub(crate) struct Input {
    pub cards: Vec<String>,
    pub panels: Vec<Panel>,
    pub card_panel: Vec<usize>,
    pub edges: Vec<Edge>,
    pub card_w: i32,
    pub card_h: i32,
    /// The header a zone's title alone asks (steps).
    pub head_steps: i32,
    /// Each card's own height (steps, the top card's for a pile): a row takes its tallest (P7, V5).
    pub heights: Vec<i32>,
    /// Each panel's header: its title and its description lines (steps; the title's alone for a
    /// panel without a frame).
    pub heads: Vec<i32>,
    /// The room of the cards behind a pile (`cards`), right and down, px (0 for a card): the
    /// top card is the module, the pile takes a step more of the lattice.
    pub pile: Vec<f64>,
    /// A node's icon (D-087), where its arrows end: none for a card.
    pub icon: Vec<Option<Icon>>,
    /// Each note (D-104) and the card it annotates.
    pub notes: Vec<(usize, usize)>,
    /// Columns (`.col-<name>`, D-051): per column, its members' cards (a card, or a zone's
    /// cards), whose axes line up across `dir`.
    pub cols: Vec<Vec<Vec<usize>>>,
    /// The flow's direction (`dir`).
    pub dir: Dir,
    /// `zones="fit"`: each frame fitted to its cards, not stretched to its cells.
    pub fit: bool,
}

/// The flow's direction (`dir`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Dir {
    /// Right or left: a stack lays its children side by side, a wide element is a column.
    pub across: bool,
    /// Up or left: a stack lays its children from the end.
    pub reverse: bool,
    /// Written: the flow goes that way, an arrow set aside across it pays.
    pub written: bool,
}

/// A node's icon within its box, px: from the box's left edge, from its top and bottom edges to
/// the content (the icon sits in the content's middle), and its size.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Icon {
    pub left: f64,
    pub top: f64,
    pub bottom: f64,
    pub size: f64,
    /// The node's own size (icon and text), px: it stands in the middle of its cell's height.
    pub w: f64,
    pub h: f64,
}

impl Icon {
    /// The node's box in its cell `cell`, px: its own size, in the middle of the cell's height.
    pub(crate) fn node_box(self, cell: crate::layout::Box2) -> crate::layout::Box2 {
        crate::layout::Box2 {
            x: cell.x,
            y: cell.y + (cell.h - self.h) / 2.0,
            w: self.w,
            h: self.h,
        }
    }

    /// The icon's box in a node box `b`, px.
    pub(crate) fn within(self, b: crate::layout::Box2) -> crate::layout::Box2 {
        crate::layout::Box2 {
            x: b.x + self.left,
            y: b.y + self.top + (b.h - self.top - self.bottom - self.size) / 2.0,
            w: self.size,
            h: self.size,
        }
    }

    /// The lattice node nearest the icon's middle, steps from the box's top left corner (`h`
    /// the box's height in steps).
    pub(crate) fn port(self, h: i32) -> (i32, i32) {
        let b = self.within(crate::layout::Box2 {
            x: 0.0,
            y: 0.0,
            w: 0.0,
            h: f64::from(h) * STEP,
        });
        (
            ((b.x + b.w / 2.0) / STEP).round() as i32,
            ((b.y + b.h / 2.0) / STEP).round() as i32,
        )
    }
}

impl Input {
    /// The zones holding card `c`, its own first, outwards (panel indices).
    pub(crate) fn zones_of(&self, c: usize) -> Vec<usize> {
        self.chain(self.card_panel[c])
    }

    /// The blocks holding card `c` (D-117), outwards.
    pub(crate) fn blocks_of(&self, c: usize) -> Vec<usize> {
        let mut out = Vec::new();
        let mut at = Some(self.card_panel[c]);
        while let Some(i) = at {
            if self.panels[i].block {
                out.push(i);
            }
            at = self.panels[i].parent;
        }
        out
    }

    /// Panel `p` if it is a zone, and the zones it sits in, outwards.
    pub(crate) fn chain(&self, p: usize) -> Vec<usize> {
        let mut out = Vec::new();
        let mut at = Some(p);
        while let Some(i) = at {
            if self.panels[i].zone.is_some() {
                out.push(i);
            }
            at = self.panels[i].parent;
        }
        out
    }

    /// Card `c` is a `.wide` one: a bar across its group.
    pub(crate) fn wide(&self, c: usize) -> bool {
        let p = &self.panels[self.card_panel[c]];
        p.wide && p.zone.is_none()
    }

    /// The panels directly inside panel `p` (`None`: at the top), in writing order.
    pub(crate) fn children(&self, p: Option<usize>) -> Vec<usize> {
        (0..self.panels.len())
            .filter(|&i| self.panels[i].parent == p)
            .collect()
    }

    /// What the drawing depends on, as text.
    pub(super) fn key(&self) -> String {
        let edges: Vec<_> = self
            .edges
            .iter()
            .map(|e| {
                (
                    &e.id,
                    e.from,
                    e.to,
                    &e.label,
                    &e.look,
                    e.lsize,
                    e.constraint,
                    e.sides,
                    &e.via,
                    &e.join,
                    e.ends,
                )
            })
            .collect();
        let panels: Vec<_> = self
            .panels
            .iter()
            .map(|p| {
                (
                    &p.zone,
                    &p.cards,
                    p.parent,
                    p.stack,
                    p.wide,
                    p.wrap,
                    p.order,
                    (p.title, p.dock, p.dir, p.block, p.align),
                )
            })
            .collect();
        format!(
            "{:?}{panels:?}{edges:?}{}x{}+{}",
            self.cards, self.card_w, self.card_h, self.head_steps
        ) + &format!("{:?}{:?}", self.heights, self.heads)
            + &format!(
                "{:?}{:?}{:?}{:?}{:?}",
                self.pile, self.icon, self.dir, self.notes, self.cols
            )
            + &format!("{}", self.fit)
    }
}

/// The card module (POC S1): 280 px at least.
const CARD_W_MIN: i32 = 28;
/// More arrows than the search's bit sets hold: the grid engine draws it (W2 says 20 anyway).
const MAX_ARROWS: usize = 120;

/// Every label wrapped at `[label-width]`: its lines and its size.
pub(super) fn wrapped(
    model: &Model,
    scene: &Scene,
) -> BTreeMap<String, (Vec<String>, crate::measure::Size)> {
    use crate::measure::{border, line_height, tw};
    let mut out = BTreeMap::new();
    for e in &model.edges {
        let Some(&el) = scene.labels.get(&e.id) else {
            continue;
        };
        if e.label.trim().is_empty() {
            continue;
        }
        let c = scene.style(el);
        let (p, b) = (c.padding(), border(c));
        let side = p[1] + p[3] + b[1] + b[3];
        let mut lines = labels::wrap(&e.label, labels::LABEL_MAX_PX - side, &|t| tw(c, t));
        if lines.len() > labels::LABEL_MAX_LINES {
            let rest = lines.split_off(labels::LABEL_MAX_LINES - 1).join(" ");
            lines.push(rest);
        }
        let w = lines.iter().map(|l| tw(c, l)).fold(0.0, f64::max) + side;
        let h = line_height(c) * lines.len() as f64 + p[0] + p[2] + b[0] + b[2];
        out.insert(
            e.id.clone(),
            (
                lines,
                crate::measure::Size {
                    w: w.ceil(),
                    h: h.ceil(),
                },
            ),
        );
    }
    out
}

/// Does the slide engine read the whole drawing (its docked elements apart)? Not yet: an invisible
/// card, more than `MAX_ARROWS` arrows. Those keep the engine that drew them before the slide
/// engine became the default.
pub(crate) fn reads(model: &Model) -> bool {
    let split = crate::dock::split(model);
    let m = split.as_ref().map_or(model, |s| &s.main);
    let node_ok = |n: &skemo_lang::Node| match n.kind {
        Kind::Zone => true,
        Kind::Card => !crate::layout::invisible(n),
    };
    let cards: std::collections::BTreeSet<&str> = m
        .nodes
        .iter()
        .filter(|n| n.kind == Kind::Card)
        .map(|n| n.id.as_str())
        .collect();
    m.nodes.iter().all(node_ok)
        && m.edges.len() <= MAX_ARROWS
        && m.edges.iter().all(|e| {
            let known = |id: &str| cards.contains(id) || m.node(id).is_some();
            known(&e.from) && known(&e.to)
        })
}

/// The diagram as the slide engine reads it, or `None` when the grid engine draws it.
pub(super) fn read(model: &Model, scene: &Scene, sizes: &Sizes, head: f64) -> Option<Input> {
    let (cards, mut panels, card_panel) = panels_of(model)?;
    for p in &mut panels {
        let below = p.zone.as_ref().is_some_and(|z| {
            sizes.captions_below.contains(z)
                || model
                    .node(z)
                    .is_some_and(|n| n.classes.iter().any(|c| c == "bare"))
        });
        if below {
            p.title = TitleAt::Below;
        }
    }
    let cards_at = cards.clone();
    let cols = columns(model, &cards, &panels);
    let edges = edges_of(model, &cards, (&panels, &card_panel), sizes)?;
    let icon: Vec<Option<Icon>> = cards
        .iter()
        .map(|c| icon_of(model, scene, sizes, c))
        .collect();
    let pile: Vec<f64> = cards
        .iter()
        .map(|c| model.node(c).map_or(0.0, crate::measure::deck_margin))
        .collect();
    // the module is the top card's: a pile's size holds the cards behind it too
    let (mut w, mut h) = (0.0f64, 0.0f64);
    for (c, m) in cards.iter().zip(&pile) {
        if let Some(s) = sizes.cards.get(c) {
            w = w.max(s.w - m);
            h = h.max(s.h - m);
        }
    }
    let heights: Vec<i32> = cards
        .iter()
        .zip(&pile)
        .map(|(c, m)| sizes.cards.get(c).map_or(4, |s| steps(s.h - m).max(4)))
        .collect();
    let heads: Vec<i32> = panels
        .iter()
        .map(|p| {
            let desc = p
                .zone
                .as_ref()
                .and_then(|z| sizes.zone_heads.get(z))
                .copied()
                .unwrap_or(0.0);
            steps(head + desc + STEP)
        })
        .collect();
    Some(Input {
        cards,
        panels,
        card_panel,
        edges,
        card_w: steps(w).max(CARD_W_MIN),
        card_h: steps(h).max(4),
        head_steps: steps(head + STEP),
        heights,
        heads,
        pile,
        icon,
        cols,
        notes: model
            .edges
            .iter()
            .filter_map(|e| crate::tidy::notes::note_link(model, e))
            .filter_map(|(n, t)| {
                let at = |id: &str| cards_at.iter().position(|c| c == id);
                Some((at(n)?, at(t)?))
            })
            .collect(),
        dir: Dir {
            across: matches!(model.option("dir"), Some("right" | "left")),
            reverse: matches!(model.option("dir"), Some("up" | "left")),
            written: model.option("dir").is_some(),
        },
        fit: model.option("zones") == Some("fit"),
    })
}

/// The icon of node `id`, as `route::anchors` reads it.
fn icon_of(model: &Model, scene: &Scene, sizes: &Sizes, id: &str) -> Option<Icon> {
    model.node(id).filter(|n| n.node)?;
    let card = scene.cards.get(id)?;
    let (size, _) = card.node_icon(scene)?;
    let own = sizes.cards.get(id)?;
    let cs = scene.style(card.el);
    let (pad, bor) = (cs.padding(), crate::measure::border(cs));
    Some(Icon {
        left: bor[3] + pad[3],
        top: bor[0] + pad[0],
        bottom: bor[2] + pad[2],
        size,
        w: own.w,
        h: own.h,
    })
}

/// The cards numbered in panel order, the panels in writing order, and each card's panel;
/// `None` on nested zones, a pinned or an invisible card.
fn panels_of(model: &Model) -> Option<(Vec<String>, Vec<Panel>, Vec<usize>)> {
    // the panels in writing order: a zone, at any depth, or a card outside every zone, where it is
    // written (P3: sibling groups keep their writing order)
    let mut panels: Vec<Panel> = Vec::new();
    let mut zone_at: BTreeMap<&str, usize> = BTreeMap::new();
    let mut cards = Vec::new();
    let wide = |n: &skemo_lang::Node| n.classes.iter().any(|c| c == "wide");
    for (order, n) in model.nodes.iter().enumerate() {
        let parent = match &n.parent {
            Some(p) => Some(*zone_at.get(p.as_str())?),
            None => None,
        };
        match n.kind {
            Kind::Zone => {
                zone_at.insert(&n.id, panels.len());
                panels.push(Panel {
                    id: Some(n.id.clone()),
                    // a stack or a bare zone draws no frame
                    zone: (!crate::layout::invisible(n)).then(|| n.id.clone()),
                    cards: Vec::new(),
                    parent,
                    stack: n.stack,
                    wide: wide(n),
                    wrap: n.wrap.map(|w| w.max(1) as usize),
                    order,
                    first: usize::MAX,
                    pile: crate::measure::deck_margin(n),
                    title: if n.head.as_deref() == Some("left") {
                        TitleAt::Left
                    } else {
                        TitleAt::Above
                    },
                    dock: band(n),
                    dir: dir_of(n),
                    block: n.block,
                    align: align_of(n),
                });
            }
            Kind::Card if n.at.is_some() || crate::layout::invisible(n) => return None,
            // a card of its own: outside every zone, in a stack or a `wrap` zone, a wide one, or
            // one docked in its group
            Kind::Card
                if parent.is_none_or(|p| panels[p].stack || panels[p].wrap.is_some())
                    || wide(n)
                    || band(n).is_some() =>
            {
                panels.push(Panel {
                    id: None,
                    zone: None,
                    cards: vec![cards.len()],
                    parent,
                    stack: false,
                    wide: wide(n),
                    wrap: None,
                    order,
                    first: order,
                    pile: 0.0,
                    title: TitleAt::Above,
                    dock: band(n),
                    dir: None,
                    block: false,
                    align: None,
                });
                cards.push(n.id.clone());
            }
            Kind::Card => {
                let p = parent?;
                panels[p].first = panels[p].first.min(order);
                panels[p].cards.push(cards.len());
                cards.push(n.id.clone());
            }
        }
    }
    if panels.is_empty() {
        return None;
    }
    let order: Vec<usize> = panels
        .iter()
        .flat_map(|p| p.cards.iter().copied())
        .collect();
    let mut renum = vec![0; cards.len()];
    for (new, &old) in order.iter().enumerate() {
        renum[old] = new;
    }
    let cards: Vec<String> = order.iter().map(|&o| cards[o].clone()).collect();
    for p in &mut panels {
        for c in &mut p.cards {
            *c = renum[*c];
        }
    }
    let mut card_panel = vec![0; cards.len()];
    for (i, p) in panels.iter().enumerate() {
        for &c in &p.cards {
            card_panel[c] = i;
        }
    }
    Some((cards, panels, card_panel))
}

/// The columns (`.col-<name>`): for each name, its members' cards, a card's own or a zone's
/// (those of the zones inside it too).
fn columns(model: &Model, cards: &[String], panels: &[Panel]) -> Vec<Vec<Vec<usize>>> {
    let mut by: BTreeMap<&str, Vec<Vec<usize>>> = BTreeMap::new();
    for n in &model.nodes {
        let Some(name) = n.classes.iter().find_map(|c| c.strip_prefix("col-")) else {
            continue;
        };
        let member: Vec<usize> = match n.kind {
            Kind::Card => cards.iter().position(|c| *c == n.id).into_iter().collect(),
            Kind::Zone => {
                let Some(z) = panels.iter().position(|p| p.id.as_deref() == Some(&n.id)) else {
                    continue;
                };
                let inside = |mut at: Option<usize>| {
                    while let Some(q) = at {
                        if q == z {
                            return true;
                        }
                        at = panels[q].parent;
                    }
                    false
                };
                panels
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| inside(Some(*i)))
                    .flat_map(|(_, p)| p.cards.iter().copied())
                    .collect()
            }
        };
        if !member.is_empty() {
            by.entry(name).or_default().push(member);
        }
    }
    by.into_values().filter(|m| m.len() > 1).collect()
}

/// `side="start-end"` read: either half may be empty.
fn sides_of(side: Option<&str>) -> (Option<Side>, Option<Side>) {
    let one = |s: &str| match s.trim() {
        "top" => Some(Side::Top),
        "bottom" => Some(Side::Bottom),
        "left" => Some(Side::Left),
        "right" => Some(Side::Right),
        _ => None,
    };
    match side.and_then(|s| s.split_once('-')) {
        Some((a, b)) => (one(a), one(b)),
        None => (side.and_then(one), None),
    }
}

/// The arrows between cards (a loop left out); `None` on an arrow to a zone or past
/// `MAX_ARROWS`.
fn edges_of(
    model: &Model,
    cards: &[String],
    (panels, card_panel): (&[Panel], &[usize]),
    sizes: &Sizes,
) -> Option<Vec<Edge>> {
    let card = |id: &str| cards.iter().position(|c| c == id);
    // a zone: its panel, and its first card (one of a zone inside it), for the placement
    let zone = |id: &str| -> Option<(usize, usize)> {
        let z = panels.iter().position(|p| p.id.as_deref() == Some(id))?;
        let inside = |c: usize| {
            let mut at = Some(card_panel[c]);
            while let Some(q) = at {
                if q == z {
                    return true;
                }
                at = panels[q].parent;
            }
            false
        };
        let first = (0..cards.len()).find(|&c| inside(c)).unwrap_or(0);
        Some((z, first))
    };
    let end = |id: &str| -> Option<(usize, Option<usize>)> {
        card(id)
            .map(|c| (c, None))
            .or_else(|| zone(id).map(|(z, c)| (c, Some(z))))
    };
    let mut edges = Vec::new();
    for e in &model.edges {
        let (Some((from, fz)), Some((to, tz))) = (end(&e.from), end(&e.to)) else {
            return None;
        };
        if from == to && fz == tz {
            continue;
        }
        let mut look = e.classes.clone();
        look.sort();
        let label = (!e.label.trim().is_empty()).then(|| e.label.clone());
        let lsize = sizes.labels.get(&e.id).map_or((0.0, 0.0), |s| (s.w, s.h));
        edges.push(Edge {
            id: e.id.clone(),
            from,
            to,
            label: label.filter(|_| lsize.0 > 0.0),
            look: look.join(" "),
            lsize,
            // a note's link does not shape the placement: the note stands beside its card
            constraint: e.constraint,
            ends: (fz, tz),
            join: e
                .join
                .clone()
                .filter(|j| !j.is_empty() && !crate::trace::off("slide-join")),
            sides: sides_of(e.side.as_deref()),
            via: e
                .via
                .iter()
                .flatten()
                .map(|p| ((p[0] / STEP).round() as i32, (p[1] / STEP).round() as i32))
                .collect(),
        });
    }
    (edges.len() <= MAX_ARROWS).then_some(edges)
}
