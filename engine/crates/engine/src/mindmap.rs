//! Mindmap mode (D-034): a central card, branches that leave to the right and to the left, and
//! sub-branches that move away from the centre. The tree comes from the arrows: the targets of a
//! card's `to(…)` are its children, in writing order.
//!
//! Placement (deterministic, local to each subtree):
//! - the root is at the centre; its children are spread between right and left, balancing the
//!   height of each side (the writing order reads top to bottom on each side);
//! - a node is centred vertically on the block of its children; siblings are stacked;
//! - levels are `LEVEL_GAP` apart, siblings `SIBLING_GAP`.
//!
//! The tree's arrows leave by the side facing outward and arrive by the side facing the centre;
//! those of one parent on one side share a trunk (hyperedge, see `route::router_scene`).

use crate::layout::Box2;
use crate::measure::{Size, Sizes};
use skemo_lang::tr;
use skemo_lang::{Kind, Model};
use std::collections::{BTreeMap, BTreeSet};

/// Horizontal space between a node and its children (px).
const LEVEL_GAP: f64 = 72.0;
/// Vertical space between two siblings (px).
const SIBLING_GAP: f64 = 20.0;

/// Root: the `root` option, otherwise the first card no arrow targets, otherwise the first.
pub fn root_of(model: &Model) -> Option<String> {
    let cards = || model.nodes.iter().filter(|n| n.kind == Kind::Card);
    if let Some(r) = model.option("root")
        && cards().any(|n| n.id == r)
    {
        return Some(r.to_string());
    }
    let targeted: BTreeSet<&str> = model
        .edges
        .iter()
        .filter(|e| e.constraint)
        .map(|e| e.to.as_str())
        .collect();
    cards()
        .find(|n| !targeted.contains(n.id.as_str()))
        .or_else(|| cards().next())
        .map(|n| n.id.clone())
}

/// Breadth-first spanning tree from the root: children of each card, in the writing order of the
/// arrows (a card already reached is not taken again: the extra arrows stay plain arrows). Also
/// returns the cards not reached.
fn tree(model: &Model, root: &str) -> (BTreeMap<String, Vec<String>>, Vec<String>) {
    let mut children: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut seen: BTreeSet<String> = BTreeSet::from([root.to_string()]);
    let mut queue = std::collections::VecDeque::from([root.to_string()]);
    while let Some(n) = queue.pop_front() {
        for e in model.edges.iter().filter(|e| e.from == n && e.constraint) {
            let is_card = model.node(&e.to).is_some_and(|t| t.kind == Kind::Card);
            if is_card && seen.insert(e.to.clone()) {
                children.entry(n.clone()).or_default().push(e.to.clone());
                queue.push_back(e.to.clone());
            }
        }
    }
    let loose = model
        .nodes
        .iter()
        .filter(|n| n.kind == Kind::Card && !seen.contains(&n.id))
        .map(|n| n.id.clone())
        .collect();
    (children, loose)
}

struct Ctx<'a> {
    sizes: &'a BTreeMap<String, Size>,
    children: &'a BTreeMap<String, Vec<String>>,
    height: BTreeMap<String, f64>,
}

impl Ctx<'_> {
    fn size(&self, id: &str) -> Size {
        self.sizes
            .get(id)
            .copied()
            .unwrap_or(Size { w: 170.0, h: 48.0 })
    }

    fn kids(&self, id: &str) -> &[String] {
        self.children.get(id).map_or(&[], Vec::as_slice)
    }

    /// Height of the block of children of a node (0 if it has none).
    fn block(&self, id: &str) -> f64 {
        let kids = self.kids(id);
        if kids.is_empty() {
            return 0.0;
        }
        kids.iter().map(|k| self.height[k]).sum::<f64>() + SIBLING_GAP * (kids.len() - 1) as f64
    }

    /// Height of the subtree (computed from the leaves to the root).
    fn compute_heights(&mut self, id: &str) {
        for k in self.kids(id).to_vec() {
            self.compute_heights(&k);
        }
        let h = self.size(id).h.max(self.block(id));
        self.height.insert(id.to_string(), h);
    }

    /// Places `id` and its descendants. `edge`: x of the edge facing the centre; `dir`: +1 to the
    /// right, −1 to the left; `top`: top of the subtree's slot.
    fn place(&self, id: &str, dir: f64, edge: f64, top: f64, out: &mut BTreeMap<String, Box2>) {
        let s = self.size(id);
        let slot = self.height[id];
        let x = if dir > 0.0 { edge } else { edge - s.w };
        out.insert(
            id.to_string(),
            Box2 {
                x,
                y: top + (slot - s.h) / 2.0,
                w: s.w,
                h: s.h,
            },
        );
        let next_edge = if dir > 0.0 {
            x + s.w + LEVEL_GAP
        } else {
            x - LEVEL_GAP
        };
        let mut y = top + (slot - self.block(id)) / 2.0;
        for k in self.kids(id) {
            self.place(k, dir, next_edge, y, out);
            y += self.height[k] + SIBLING_GAP;
        }
    }
}

/// Positions of the cards of a mindmap, and warnings.
pub fn layout(model: &Model, sizes: &Sizes, root: &str) -> (BTreeMap<String, Box2>, Vec<String>) {
    let (children, loose) = tree(model, root);
    let mut cx = Ctx {
        sizes: &sizes.cards,
        children: &children,
        height: BTreeMap::new(),
    };
    cx.compute_heights(root);
    // spreading of the main branches: the shorter side gets the next one (right first)
    let (mut right, mut left): (Vec<&String>, Vec<&String>) = (Vec::new(), Vec::new());
    let (mut hr, mut hl) = (0.0, 0.0);
    for k in cx.kids(root) {
        let h = cx.height[k];
        if hr <= hl {
            right.push(k);
            hr += h + SIBLING_GAP;
        } else {
            left.push(k);
            hl += h + SIBLING_GAP;
        }
    }
    let mut boxes = BTreeMap::new();
    let rs = cx.size(root);
    boxes.insert(
        root.to_string(),
        Box2 {
            x: 0.0,
            y: -rs.h / 2.0,
            w: rs.w,
            h: rs.h,
        },
    );
    for (side, dir, edge) in [(&right, 1.0, rs.w + LEVEL_GAP), (&left, -1.0, -LEVEL_GAP)] {
        let total: f64 = side.iter().map(|k| cx.height[*k]).sum::<f64>()
            + SIBLING_GAP * side.len().saturating_sub(1) as f64;
        let mut y = -total / 2.0;
        for k in side {
            cx.place(k, dir, edge, y, &mut boxes);
            y += cx.height[*k] + SIBLING_GAP;
        }
    }
    let mut warnings = Vec::new();
    // cards not linked to the root: aligned under the card, with a warning
    let below = boxes.values().map(Box2::bottom).fold(0.0, f64::max) + 2.0 * SIBLING_GAP;
    let mut x = 0.0;
    for id in &loose {
        let s = cx.size(id);
        boxes.insert(
            id.clone(),
            Box2 {
                x,
                y: below,
                w: s.w,
                h: s.h,
            },
        );
        x += s.w + SIBLING_GAP;
        warnings.push(tr!(
            "card « {id} » is not linked to the root « {root} »: placed under the mindmap",
            "carte « {id} » sans lien avec la racine « {root} » : placée sous la mindmap"
        ));
    }
    (boxes, warnings)
}

/// Mode classes (D-048, M1): the root `mm-root`; its children `mm-branch` and every card below
/// them `mm-leaf`, each with the colour index of its branch `mm-b0` … `mm-b6` (the theme draws a
/// dark root pill, tinted branch pills and plain-text leaves, one colour per branch).
pub fn extras(model: &Model, root: &str) -> crate::scene::Extras {
    let (children, _) = tree(model, root);
    let mut e = crate::scene::Extras::default();
    e.classes.insert(root.to_string(), vec!["mm-root".into()]);
    let branches = children.get(root).cloned().unwrap_or_default();
    for (k, b) in branches.iter().enumerate() {
        let colour = format!("mm-b{}", k % 7);
        e.classes
            .insert(b.clone(), vec!["mm-branch".into(), colour.clone()]);
        let mut stack = children.get(b).cloned().unwrap_or_default();
        while let Some(n) = stack.pop() {
            e.classes
                .insert(n.clone(), vec!["mm-leaf".into(), colour.clone()]);
            stack.extend(children.get(&n).cloned().unwrap_or_default());
        }
    }
    e
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model(src: &str) -> Model {
        skemo_lang::parse(src).model
    }

    #[test]
    fn root_is_the_option_then_the_first_untargeted_card() {
        let m = model("diagram(layout=\"mindmap\")\n  card#a A\n    to(b)\n  card#b B\n");
        assert_eq!(root_of(&m).as_deref(), Some("a"));
        let m =
            model("diagram(layout=\"mindmap\" root=\"b\")\n  card#a A\n    to(b)\n  card#b B\n");
        assert_eq!(root_of(&m).as_deref(), Some("b"));
        // in a cycle, the first card
        let m = model("card#a A\n  to(b)\ncard#b B\n  to(a)\n");
        assert_eq!(root_of(&m).as_deref(), Some("a"));
    }

    #[test]
    fn tree_takes_each_card_once_and_reports_loose_ones() {
        let m = model("card#r R\n  to(a b)\ncard#a A\n  to(b c)\ncard#b B\ncard#c C\ncard#z Z\n");
        let (kids, loose) = tree(&m, "r");
        assert_eq!(kids["r"], ["a", "b"]);
        assert_eq!(kids["a"], ["c"], "b is already a child of r");
        assert_eq!(loose, ["z"]);
    }
}
