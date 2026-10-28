//! Widths set on the measured sizes, before ELK: a card is as wide as its text, except where a
//! set of cards should read as one (siblings, a grid column) or a zone title needs room.

use crate::measure::Sizes;
use skemo_lang::{Kind, Model};
use std::collections::BTreeMap;

/// Siblings share a width (D-048, L3): cards of one container fed by the same cards (the
/// branches of a fork, the workers behind one load balancer), or with no arrow in at all, take
/// the width of the widest. Cards are otherwise as wide as their text (the theme's minimum is
/// small), so a row of siblings reads as a set and a lone card does not get wider than needed.
/// Notes and call-outs (`.note`, `.info`) keep their own width, chips share theirs like any card
/// (D-049); so do legends, plain text and cards holding a table, whose width is their content's
/// (v0.8).
pub fn even_sibling_widths(model: &Model, sizes: &mut Sizes) {
    let secondary = |n: &skemo_lang::Node| {
        n.classes
            .iter()
            .any(|c| matches!(c.as_str(), "note" | "info" | "legend" | "text"))
            || n.li.iter().any(|l| !l.cells.is_empty())
    };
    let mut groups: BTreeMap<(Option<String>, Vec<String>), Vec<String>> = BTreeMap::new();
    for n in model
        .nodes
        .iter()
        .filter(|n| n.kind == Kind::Card && !secondary(n))
    {
        let mut parents: Vec<String> = model
            .edges
            .iter()
            .filter(|e| e.to == n.id && e.constraint)
            .map(|e| e.from.clone())
            .collect();
        parents.sort();
        parents.dedup();
        groups
            .entry((n.parent.clone(), parents))
            .or_default()
            .push(n.id.clone());
    }
    // even widths: cards whose edges line up then share their centre to the pixel, so that a card
    // pinned on whole pixels (D-031) keeps its straight arrows
    for s in sizes.cards.values_mut() {
        s.w = (s.w / 2.0).ceil() * 2.0;
    }
    for ids in groups.values().filter(|g| g.len() > 1) {
        let widest = ids
            .iter()
            .filter_map(|id| sizes.cards.get(id))
            .map(|s| s.w)
            .fold(0.0, f64::max);
        for id in ids {
            if let Some(s) = sizes.cards.get_mut(id) {
                s.w = widest;
            }
        }
    }
}

/// A zone's title fits beside an arrow entering its first card (D-048): now that cards are as
/// wide as their text, a short card under a long zone title left no room for an arrow coming in
/// from above or from the left. When every card of a zone is narrower than the zone's title plus
/// 48 px, its first card (under the title) takes that width; its siblings and its grid column
/// then follow (`even_sibling_widths`, `even_wrap_widths`). A bare zone's caption sits below its
/// cards, out of the arrows' way: it widens nothing (D-049).
pub fn widen_for_titles(model: &Model, scene: &crate::scene::Scene, sizes: &mut Sizes) {
    for z in model.nodes.iter().filter(|n| {
        n.kind == Kind::Zone && !n.title.is_empty() && !n.classes.iter().any(|c| c == "bare")
    }) {
        let Some(el) = scene.zones.get(&z.id) else {
            continue;
        };
        let need = (crate::measure::tw(scene.style(el.title), &z.title)
            + scene.icon_lead(&el.decor).0
            + 48.0)
            .ceil();
        let kids: Vec<String> = model
            .children(Some(&z.id))
            .filter(|c| c.kind == Kind::Card)
            .map(|c| c.id.clone())
            .collect();
        let widest = kids
            .iter()
            .filter_map(|k| sizes.cards.get(k))
            .map(|s| s.w)
            .fold(0.0, f64::max);
        // the first card (the one under the title); siblings and grid columns follow it
        if widest > 0.0
            && widest < need
            && let Some(s) = kids.first().and_then(|k| sizes.cards.get_mut(k))
        {
            s.w = (need / 2.0).ceil() * 2.0;
        }
    }
}

/// Cards of one column of a `wrap` zone's grid get the width of the widest (a stack reads as a
/// clean column). The column of a card follows from its rank in writing order (`i % N` with
/// `dir` down or up, `i / N` with right or left, where the grid is filled column by column), so
/// the result depends on the source only.
pub fn even_wrap_widths(model: &Model, sizes: &mut Sizes) {
    let across = matches!(model.option("dir"), Some("right" | "left"));
    for z in model.nodes.iter().filter(|n| n.kind == Kind::Zone) {
        let Some(n) = z.wrap.map(|w| w as usize) else {
            continue;
        };
        let kids: Vec<&str> = model.children(Some(&z.id)).map(|c| c.id.as_str()).collect();
        let col = |i: usize| if across { i / n } else { i % n };
        let mut widest: BTreeMap<usize, f64> = BTreeMap::new();
        for (i, id) in kids.iter().enumerate() {
            if let Some(s) = sizes.cards.get(*id) {
                let w = widest.entry(col(i)).or_insert(0.0);
                *w = w.max(s.w);
            }
        }
        for (i, id) in kids.iter().enumerate() {
            if let Some(s) = sizes.cards.get_mut(*id) {
                s.w = widest[&col(i)];
            }
        }
    }
}
