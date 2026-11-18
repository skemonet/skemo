//! The diagram as the slide engine reads it: one panel per zone (a loose card is a panel of its
//! own), the cards numbered in panel order, one module for every card, labels wrapped.

use super::{STEP, labels, steps};
use crate::measure::Sizes;
use crate::scene::Scene;
use skemo_lang::{Kind, Model};
use std::collections::BTreeMap;

pub(crate) struct Panel {
    pub zone: Option<String>,
    pub cards: Vec<usize>,
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
}

/// The diagram as the slide engine reads it.
pub(crate) struct Input {
    pub cards: Vec<String>,
    pub panels: Vec<Panel>,
    pub card_panel: Vec<usize>,
    pub edges: Vec<Edge>,
    pub card_w: i32,
    pub card_h: i32,
    pub head_steps: i32,
    /// The room of the cards behind a pile (`cards`), right and down, px (0 for a card): the
    /// top card is the module, the pile takes a step more of the lattice.
    pub pile: Vec<f64>,
    /// A node's icon (D-087), where its arrows end: none for a card.
    pub icon: Vec<Option<Icon>>,
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
                )
            })
            .collect();
        let panels: Vec<_> = self.panels.iter().map(|p| (&p.zone, &p.cards)).collect();
        format!(
            "{:?}{panels:?}{edges:?}{}x{}+{}",
            self.cards, self.card_w, self.card_h, self.head_steps
        ) + &format!("{:?}{:?}", self.pile, self.icon)
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

/// Does the slide engine read the whole drawing (its docked elements apart)? Not yet: nested
/// zones, an empty zone, a pinned or invisible card, a zone placed or sized by hand, `wrap`,
/// `stack`, a `.wide` element, an arrow to a zone, more than `MAX_ARROWS`
/// arrows. Those keep the engine that drew them before the slide engine became the default.
pub(crate) fn reads(model: &Model) -> bool {
    let split = crate::dock::split(model);
    let m = split.as_ref().map_or(model, |s| &s.main);
    let node_ok = |n: &skemo_lang::Node| match n.kind {
        Kind::Zone => {
            n.parent.is_none()
                && n.at.is_none()
                && n.size.is_none()
                && n.wrap.is_none()
                && !n.stack
                && !crate::layout::invisible(n)
        }
        Kind::Card => n.at.is_none() && !crate::layout::invisible(n),
    };
    let cards: std::collections::BTreeSet<&str> = m
        .nodes
        .iter()
        .filter(|n| n.kind == Kind::Card)
        .map(|n| n.id.as_str())
        .collect();
    let holds_a_card = |z: &skemo_lang::Node| {
        m.nodes
            .iter()
            .any(|n| n.kind == Kind::Card && n.parent.as_deref() == Some(z.id.as_str()))
    };
    m.nodes.iter().all(|n| {
        node_ok(n)
            && !n.classes.iter().any(|c| c == "wide")
            && (n.kind == Kind::Card || holds_a_card(n))
    }) && m.edges.len() <= MAX_ARROWS
        && m.edges
            .iter()
            .all(|e| cards.contains(e.from.as_str()) && cards.contains(e.to.as_str()))
}

/// The diagram as the slide engine reads it, or `None` when the grid engine draws it.
pub(super) fn read(model: &Model, scene: &Scene, sizes: &Sizes, head: f64) -> Option<Input> {
    let (cards, panels, card_panel) = panels_of(model)?;
    let edges = edges_of(model, &cards, sizes)?;
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
    let desc = sizes.zone_heads.values().copied().fold(0.0, f64::max);
    Some(Input {
        cards,
        panels,
        card_panel,
        edges,
        card_w: steps(w).max(CARD_W_MIN),
        card_h: steps(h).max(4),
        head_steps: steps(head + desc + STEP),
        pile,
        icon,
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
    // the panels in writing order: a zone, or a card outside every zone, where it is written
    // (P3: sibling groups keep their writing order)
    let mut panels: Vec<Panel> = Vec::new();
    let mut zone_at: BTreeMap<&str, usize> = BTreeMap::new();
    let mut cards = Vec::new();
    for n in &model.nodes {
        match n.kind {
            Kind::Zone if n.parent.is_some() => return None,
            Kind::Zone => {
                zone_at.insert(&n.id, panels.len());
                panels.push(Panel {
                    zone: Some(n.id.clone()),
                    cards: Vec::new(),
                });
            }
            Kind::Card if n.at.is_some() || crate::layout::invisible(n) => return None,
            Kind::Card if n.parent.is_none() => {
                panels.push(Panel {
                    zone: None,
                    cards: vec![cards.len()],
                });
                cards.push(n.id.clone());
            }
            _ => {}
        }
    }
    for n in model.nodes.iter().filter(|n| n.kind == Kind::Card) {
        if let Some(p) = &n.parent {
            let i = *zone_at.get(p.as_str())?;
            panels[i].cards.push(cards.len());
            cards.push(n.id.clone());
        }
    }
    panels.retain(|p| !p.cards.is_empty());
    if cards.is_empty() {
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

/// The arrows between cards (a loop left out); `None` on an arrow to a zone or past
/// `MAX_ARROWS`.
fn edges_of(model: &Model, cards: &[String], sizes: &Sizes) -> Option<Vec<Edge>> {
    let at = |id: &str| cards.iter().position(|c| c == id);
    let mut edges = Vec::new();
    for e in &model.edges {
        let (Some(from), Some(to)) = (at(&e.from), at(&e.to)) else {
            return None;
        };
        if from == to {
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
            constraint: e.constraint,
        });
    }
    (edges.len() <= MAX_ARROWS).then_some(edges)
}
