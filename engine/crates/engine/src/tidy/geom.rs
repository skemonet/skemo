//! Geometry shared by the passes: the drawing's axes, groups of overlapping boxes, and the test a
//! node passes before it moves (no neighbour within 8 px, its zones still clear of theirs).

use crate::layout::Box2;
use skemo_lang::{Edge, Kind, Model};
use std::collections::BTreeMap;

/// The boxes of the placement, by node id.
pub type Boxes = BTreeMap<String, Box2>;

/// Smallest gap kept between a moved node and a neighbour.
const GAP: f64 = 8.0;

/// The drawing's axes: along the flow (rows follow each other) and across it.
#[derive(Clone, Copy)]
pub struct Axes {
    /// `dir` right or left: the flow runs along x.
    pub across: bool,
    /// `dir` up or left: the flow runs towards smaller coordinates.
    pub reverse: bool,
}

impl Axes {
    pub fn of(model: &Model) -> Axes {
        let dir = model.option("dir").unwrap_or("down");
        Axes {
            across: matches!(dir, "right" | "left"),
            reverse: matches!(dir, "up" | "left"),
        }
    }
    /// Extent along the flow.
    pub fn flow(self, b: &Box2) -> (f64, f64) {
        if self.across {
            (b.x, b.right())
        } else {
            (b.y, b.bottom())
        }
    }
    /// Extent across the flow.
    pub fn span(self, b: &Box2) -> (f64, f64) {
        if self.across {
            (b.y, b.bottom())
        } else {
            (b.x, b.right())
        }
    }
    /// Size along the flow.
    pub fn flow_len(self, b: &Box2) -> f64 {
        if self.across { b.w } else { b.h }
    }
    /// Size across the flow.
    pub fn span_len(self, b: &Box2) -> f64 {
        if self.across { b.h } else { b.w }
    }
    pub fn mid(self, b: &Box2) -> f64 {
        let (a, z) = self.span(b);
        (a + z) / 2.0
    }
    /// Do `a` and `b` share a row (their extents along the flow overlap)?
    pub fn same_row(self, a: &Box2, b: &Box2) -> bool {
        let (a, b) = (self.flow(a), self.flow(b));
        a.0.max(b.0) < a.1.min(b.1)
    }
    /// Is `c` entirely beyond `p` in the drawing's direction?
    pub fn beyond(self, p: &Box2, c: &Box2) -> bool {
        if self.reverse {
            self.flow(c).1 <= self.flow(p).0
        } else {
            self.flow(c).0 >= self.flow(p).1
        }
    }
    /// Order of two starts along the flow, the nearest to the drawing's start first.
    pub fn upstream_first(self, a: f64, b: f64) -> std::cmp::Ordering {
        if self.reverse {
            b.total_cmp(&a)
        } else {
            a.total_cmp(&b)
        }
    }
    /// `b` moved across by `d`.
    pub fn moved(self, b: &Box2, d: f64) -> Box2 {
        let mut m = *b;
        if self.across {
            m.y += d;
        } else {
            m.x += d;
        }
        m
    }
    /// `b` set at `flow` along the flow and `span` across it.
    pub fn at(self, b: Box2, flow: f64, span: f64) -> Box2 {
        if self.across {
            Box2 {
                x: flow,
                y: span,
                ..b
            }
        } else {
            Box2 {
                x: span,
                y: flow,
                ..b
            }
        }
    }
    /// Start and size along the flow, then across it.
    pub fn split(self, b: &Box2) -> ((f64, f64), (f64, f64)) {
        if self.across {
            ((b.x, b.w), (b.y, b.h))
        } else {
            ((b.y, b.h), (b.x, b.w))
        }
    }
    /// The box with these start and size along the flow, then across it (`split`'s inverse).
    pub fn join(self, (f, fs): (f64, f64), (c, cs): (f64, f64)) -> Box2 {
        if self.across {
            Box2 {
                x: f,
                y: c,
                w: fs,
                h: cs,
            }
        } else {
            Box2 {
                x: c,
                y: f,
                w: cs,
                h: fs,
            }
        }
    }
    /// Moves node `id` across the flow by `d`, with everything inside it.
    pub fn shift_across(self, model: &Model, id: &str, d: f64, boxes: &mut Boxes) {
        shift(model, id, d, self.across, boxes);
    }
    /// Moves node `id` along the flow by `d`, with everything inside it.
    pub fn shift_along(self, model: &Model, id: &str, d: f64, boxes: &mut Boxes) {
        shift(model, id, d, !self.across, boxes);
    }
}

/// Moves a node by `d` (along y if `vertical`, otherwise along x) and, for a zone, everything
/// inside it.
fn shift(model: &Model, id: &str, d: f64, vertical: bool, boxes: &mut Boxes) {
    if d.abs() < 1e-9 {
        return;
    }
    if let Some(b) = boxes.get_mut(id) {
        if vertical {
            b.y += d;
        } else {
            b.x += d;
        }
    }
    let kids: Vec<String> = model.children(Some(id)).map(|n| n.id.clone()).collect();
    for k in kids {
        shift(model, &k, d, vertical, boxes);
    }
}

/// The smallest start and the largest end of some extents (infinite when there are none).
pub fn extent(spans: impl IntoIterator<Item = (f64, f64)>) -> (f64, f64) {
    spans
        .into_iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), (a, z)| {
            (lo.min(a), hi.max(z))
        })
}

/// Groups (of at least `min` members) of the given nodes whose extents, as `ext` reads them,
/// overlap from one to the next, in order of their start.
fn overlapping<'a>(
    ids: &[&'a str],
    boxes: &Boxes,
    ext: impl Fn(&Box2) -> (f64, f64),
    min: usize,
) -> Vec<Vec<&'a str>> {
    let mut sorted: Vec<(&str, (f64, f64))> = ids
        .iter()
        .filter_map(|id| boxes.get(*id).map(|b| (*id, ext(b))))
        .collect();
    sorted.sort_by(|a, b| a.1.0.total_cmp(&b.1.0).then(a.0.cmp(b.0)));
    let mut out: Vec<Vec<&str>> = Vec::new();
    let mut reach = f64::NEG_INFINITY;
    for (id, (lo, hi)) in sorted {
        match out.last_mut() {
            Some(cur) if lo < reach => {
                cur.push(id);
                reach = reach.max(hi);
            }
            _ => {
                out.push(vec![id]);
                reach = hi;
            }
        }
    }
    out.retain(|r| r.len() >= min);
    out
}

/// Rows of the drawing (at least `min` nodes): extents along the flow overlap.
pub fn rows<'a>(ids: &[&'a str], boxes: &Boxes, ax: Axes, min: usize) -> Vec<Vec<&'a str>> {
    overlapping(ids, boxes, |b| ax.flow(b), min)
}

/// Columns of at least two nodes: horizontal extents overlap, whatever the direction.
pub fn columns<'a>(ids: &[&'a str], boxes: &Boxes) -> Vec<Vec<&'a str>> {
    overlapping(ids, boxes, |b| (b.x, b.right()), 2)
}

/// Two members of the group share a row: the group is not a column (two cards side by side above
/// a common target), and aligning it would put them on top of each other.
pub fn side_by_side(col: &[&str], boxes: &Boxes) -> bool {
    col.iter().enumerate().any(|(i, a)| {
        col[i + 1..].iter().any(|b| {
            let (a, b) = (&boxes[*a], &boxes[*b]);
            a.y.max(b.y) + 1.0 < a.bottom().min(b.bottom())
        })
    })
}

/// Are `a` and `b` closer than `GAP`?
fn near(a: &Box2, b: &Box2) -> bool {
    a.x < b.right() + GAP
        && b.x < a.right() + GAP
        && a.y < b.bottom() + GAP
        && b.y < a.bottom() + GAP
}

/// Can node `id` move to `b`? No node outside `group` comes within 8 px (`collides`), and its
/// zones, grown to hold it, run into no neighbour (`in_container`).
pub fn fits(model: &Model, id: &str, b: &Box2, group: &[&str], boxes: &Boxes) -> bool {
    !collides(model, id, b, group, boxes) && in_container(model, id, b, boxes)
}

/// Would node `id`, moved to `b`, come within 8 px of a node outside `group` that does not
/// contain it?
pub fn collides(model: &Model, id: &str, b: &Box2, group: &[&str], boxes: &Boxes) -> bool {
    boxes
        .iter()
        .any(|(o, ob)| !group.contains(&o.as_str()) && !within(model, id, o) && near(b, ob))
}

/// Can node `id` take box `b` without its zones, grown to hold it, running into a neighbour? A
/// card centred across zones may widen its zone's frame, but never over another node.
fn in_container(model: &Model, id: &str, b: &Box2, boxes: &Boxes) -> bool {
    // a zone frame: its padding on the sides and below, its title above (`zone_frames`)
    const PAD: f64 = 24.0;
    const HEAD: f64 = 48.0;
    let mut grown = *b;
    let mut cur = model.node(id).and_then(|n| n.parent.clone());
    while let Some(z) = cur {
        let Some(zb) = boxes.get(&z) else {
            return true;
        };
        let x0 = zb.x.min(grown.x - PAD);
        let y0 = zb.y.min(grown.y - HEAD);
        grown = Box2 {
            x: x0,
            y: y0,
            w: zb.right().max(grown.right() + PAD) - x0,
            h: zb.bottom().max(grown.bottom() + PAD) - y0,
        };
        let hit = boxes.iter().any(|(o, ob)| {
            o != &z && !within(model, o, &z) && !within(model, &z, o) && near(&grown, ob)
        });
        if hit {
            return false;
        }
        cur = model.node(&z).and_then(|n| n.parent.clone());
    }
    true
}

/// Is `n` the node `id` or inside it?
pub fn within(model: &Model, n: &str, id: &str) -> bool {
    let mut cur = Some(n);
    while let Some(c) = cur {
        if c == id {
            return true;
        }
        cur = model.node(c).and_then(|x| x.parent.as_deref());
    }
    false
}

/// Is `id` a cell of a `wrap` grid? Its place is the grid's (`wrap_grid`), not moved by centring.
pub fn in_grid(model: &Model, id: &str) -> bool {
    model
        .node(id)
        .and_then(|n| n.parent.as_deref())
        .and_then(|z| model.node(z))
        .is_some_and(|z| z.wrap.is_some())
}

pub fn is_card(model: &Model, id: &str) -> bool {
    model.node(id).is_some_and(|n| n.kind == Kind::Card)
}

/// The containers of the drawing: the top level (`None`), then every zone.
pub fn containers(model: &Model) -> impl Iterator<Item = Option<&str>> {
    std::iter::once(None).chain(
        model
            .nodes
            .iter()
            .filter(|n| n.kind == Kind::Zone)
            .map(|n| Some(n.id.as_str())),
    )
}

/// The arrows that shape the layout (not `constraint="false"`).
pub fn constrained(model: &Model) -> Vec<&Edge> {
    model.edges.iter().filter(|e| e.constraint).collect()
}

/// Is `p` the only node with an arrow into `child`?
pub fn only_parent(edges: &[&Edge], p: &str, child: &str) -> bool {
    edges.iter().all(|e| e.to != child || e.from == p)
}

/// A box on whole pixels (a pinned card is written back rounded and must find the same place).
pub fn whole(b: Box2) -> Box2 {
    Box2 {
        x: b.x.round(),
        y: b.y.round(),
        ..b
    }
}
