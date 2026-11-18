//! The blocks of a drawing and their order: the placement of the grid engine.
//!
//! Every top-level child is a block. A zone of cards lays its cards along their sequences (an
//! arrow `a → b` sets `b` a level after `a`), in columns or in rows; anything else (a card alone,
//! a zone holding zones) is laid as a mosaic and keeps its shape. The blocks then fill rows, as a
//! mosaic does, and a search moves them while the arrows shorten (a solid arrow counting twice a
//! dashed one): two blocks swapped, a block's sequences turned from columns to rows or back, two
//! cards of one level swapped. The block the most arrows leave stands first and stays there.

use super::mosaic::{Ctx, extent, padding, rows, tile};
use super::on;
use crate::layout::Box2;
use crate::measure::Sizes;
use skemo_lang::{Kind, Model};
use std::collections::BTreeMap;

/// One tile of the optimised mosaic: a zone of cards (laid by sequences), or anything else laid
/// as `tile` does (a card alone, a zone holding zones).
struct Block {
    id: String,
    /// The zone's cards, its levels (indices into `cards`) and whether the levels run across
    /// (sequences in a row) or along (sequences in a column); empty for a fixed block.
    cards: Vec<String>,
    levels: Vec<Vec<usize>>,
    across: bool,
    cell: (f64, f64),
    /// A block laid by `tile`: its boxes relative to its own origin, and its size.
    fixed: Option<(BTreeMap<String, Box2>, f64, f64)>,
}

/// The weight of an arrow in the search: a solid line (primary) counts twice a dashed or dotted
/// one (secondary).
fn weight(scene: &crate::scene::Scene, id: &str) -> f64 {
    scene.edges.get(id).map_or(1.0, |e| {
        let dashed = scene
            .style(e.line)
            .get("stroke-dasharray")
            .is_some_and(|d| !matches!(d.trim(), "" | "none"));
        if dashed { 1.0 } else { 2.0 }
    })
}

/// A zone's cards in levels along its sequences: an arrow `a → b` sets `b` a level after `a`
/// (solid arrows first, an arrow that would close a cycle left out); cards with no arrow inside
/// go to the shortest level.
fn levels(model: &Model, scene: &crate::scene::Scene, cards: &[String]) -> Vec<Vec<usize>> {
    let at = |id: &str| cards.iter().position(|c| c == id);
    let mut arrows: Vec<(f64, usize, usize)> = model
        .edges
        .iter()
        .filter_map(|e| Some((weight(scene, &e.id), at(&e.from)?, at(&e.to)?)))
        .filter(|(_, a, b)| a != b)
        .collect();
    arrows.sort_by(|x, y| y.0.total_cmp(&x.0).then(x.1.cmp(&y.1)));
    let n = cards.len();
    let mut next: Vec<Vec<usize>> = vec![Vec::new(); n];
    let reaches = |next: &Vec<Vec<usize>>, from: usize, to: usize| {
        let mut seen = vec![false; n];
        let mut todo = vec![from];
        while let Some(v) = todo.pop() {
            if v == to {
                return true;
            }
            if !std::mem::replace(&mut seen[v], true) {
                todo.extend(&next[v]);
            }
        }
        false
    };
    let mut linked = vec![false; n];
    for (_, a, b) in arrows {
        if !reaches(&next, b, a) {
            next[a].push(b);
            linked[a] = true;
            linked[b] = true;
        }
    }
    let mut level = vec![0usize; n];
    for _ in 0..n {
        for a in 0..n {
            for &b in &next[a] {
                level[b] = level[b].max(level[a] + 1);
            }
        }
    }
    let depth = (0..n)
        .filter(|&k| linked[k])
        .map(|k| level[k])
        .max()
        .map_or(1, |d| d + 1);
    let mut out: Vec<Vec<usize>> = vec![Vec::new(); depth];
    for k in (0..n).filter(|&k| linked[k]) {
        out[level[k]].push(k);
    }
    for k in (0..n).filter(|&k| !linked[k]) {
        if let Some(l) = (0..depth).min_by_key(|&l| out[l].len()) {
            out[l].push(k);
        }
    }
    out
}

impl Block {
    /// The distance from one row of cards to the next: a cell and the zone's gap, or `pitch` (the
    /// rows shared with the zones beside it, `pitches`).
    fn pitch(&self, ctx: &Ctx, pitch: Option<f64>) -> f64 {
        pitch.unwrap_or(self.cell.1 + ctx.gap(Some(&self.id), false))
    }

    /// Content size (without the frame's padding), its cards the zone's gaps apart (`pitch`: the
    /// rows' pitch shared with the zones beside it).
    fn content(&self, ctx: &Ctx, pitch: Option<f64>) -> (f64, f64) {
        if let Some((_, w, h)) = &self.fixed {
            return (*w, *h);
        }
        let gx = ctx.gap(Some(&self.id), true);
        let gy = self.pitch(ctx, pitch) - self.cell.1;
        let deep = self.levels.len();
        let wide = self.levels.iter().map(Vec::len).max().unwrap_or(1);
        let (cols, rows) = if self.across {
            (deep, wide)
        } else {
            (wide, deep)
        };
        let (cw, ch) = self.cell;
        (
            cols as f64 * cw + cols.saturating_sub(1) as f64 * gx,
            rows as f64 * ch + rows.saturating_sub(1) as f64 * gy,
        )
    }

    /// The cards' boxes from the content origin, each level centred.
    fn local(&self, ctx: &Ctx, pitch: Option<f64>) -> Vec<(String, Box2)> {
        if let Some((rel, _, _)) = &self.fixed {
            return rel.iter().map(|(k, v)| (k.clone(), *v)).collect();
        }
        let gx = ctx.gap(Some(&self.id), true);
        let gy = self.pitch(ctx, pitch) - self.cell.1;
        let wide = self.levels.iter().map(Vec::len).max().unwrap_or(1);
        let (cw, ch) = self.cell;
        let mut out = Vec::new();
        for (l, lv) in self.levels.iter().enumerate() {
            let shift = (wide - lv.len()) as f64 / 2.0;
            for (j, &k) in lv.iter().enumerate() {
                let s = shift + j as f64;
                let (x, y) = if self.across {
                    (l as f64 * (cw + gx), s * (ch + gy))
                } else {
                    (s * (cw + gx), l as f64 * (ch + gy))
                };
                out.push((self.cards[k].clone(), Box2 { x, y, w: cw, h: ch }));
            }
        }
        out
    }
}

/// The room a block keeps around its content: left, top, right, bottom.
type Room = (f64, f64, f64, f64);

/// No room around a block's content: a card, or a zone that is not drawn.
const BARE: Room = (0.0, 0.0, 0.0, 0.0);

/// Is block `b` a zone?
fn is_zone(model: &Model, b: &Block) -> bool {
    model.node(&b.id).is_some_and(|n| n.kind == Kind::Zone)
}

/// The rows' pitch of the zones of cards that stand side by side in one row of the mosaic (index
/// into `order` → pitch): the largest of theirs, so that their rows of cards line up from one
/// frame to the next (their first rows already start at one height, `compose`).
fn pitches(
    model: &Model,
    ctx: &Ctx,
    blocks: &[Block],
    order: &[usize],
    rows: &[Vec<usize>],
) -> BTreeMap<usize, f64> {
    let mut out = BTreeMap::new();
    if !on("grid-row-lines") {
        return out;
    }
    for row in rows {
        let zones: Vec<usize> = row
            .iter()
            .copied()
            .filter(|&i| {
                let b = &blocks[order[i]];
                b.fixed.is_none() && is_zone(model, b)
            })
            .collect();
        if zones.len() < 2 {
            continue;
        }
        let most = zones
            .iter()
            .map(|&i| blocks[order[i]].pitch(ctx, None))
            .fold(0.0, f64::max);
        out.extend(zones.into_iter().map(|i| (i, most)));
    }
    out
}

/// Each block's frame, in `order`: its size, and the room it keeps around its content (`pitch`:
/// the rows' pitch shared by neighbouring zones, by index into `order`).
fn frames(
    model: &Model,
    ctx: &Ctx,
    blocks: &[Block],
    order: &[usize],
    pitch: &BTreeMap<usize, f64>,
) -> Vec<(f64, f64, Room)> {
    order
        .iter()
        .enumerate()
        .map(|(k, &i)| {
            let b = &blocks[i];
            let (cw, ch) = b.content(ctx, pitch.get(&k).copied());
            if !is_zone(model, b) {
                return (cw, ch, BARE);
            }
            let p = padding(model, ctx, &b.id);
            let title = ctx.sizes.title_ends.get(&b.id).map_or(0.0, |e| e + ctx.pad);
            ((cw + p.0 + p.2).max(title), ch + p.1 + p.3, p)
        })
        .collect()
}

/// The rows the blocks fill in `order` (indices into it): as many as brings the whole nearest
/// the target.
fn split(model: &Model, ctx: &Ctx, blocks: &[Block], order: &[usize]) -> Vec<Vec<usize>> {
    let sizes: Vec<(f64, f64)> = frames(model, ctx, blocks, order, &BTreeMap::new())
        .iter()
        .map(|f| (f.0, f.1))
        .collect();
    rows(&sizes)
}

/// The mosaic of `blocks` in `order`, in the rows `rows`: every card's and zone's box, absolute.
fn compose(
    model: &Model,
    ctx: &Ctx,
    blocks: &[Block],
    order: &[usize],
    rows: &[Vec<usize>],
) -> BTreeMap<String, Box2> {
    let is_zone = |b: &Block| is_zone(model, b);
    let pitch = pitches(model, ctx, blocks, order, rows);
    let frames = frames(model, ctx, blocks, order, &pitch);
    let sizes: Vec<(f64, f64)> = frames.iter().map(|f| (f.0, f.1)).collect();
    // one gap between all the blocks, along each axis
    let (gx, gy) = ctx.gaps(None);
    let (width, _) = extent(rows, &sizes, (gx, gy));
    let mut out = BTreeMap::new();
    let mut y = 24.0;
    for row in rows {
        // the zones of a row start their cards at one height, under the tallest title: cards of
        // neighbouring zones line up
        let head = row.iter().map(|&i| frames[i].2.1).fold(0.0, f64::max);
        let head_of = |i: usize| {
            if on("grid-heads") {
                head
            } else {
                frames[i].2.1
            }
        };
        let rh = row
            .iter()
            .map(|&i| {
                let (_, h, p) = frames[i];
                if p == BARE { h } else { h + head_of(i) - p.1 }
            })
            .fold(0.0, f64::max);
        let used: f64 =
            row.iter().map(|&i| sizes[i].0).sum::<f64>() + gx * row.len().saturating_sub(1) as f64;
        // the row stretched to the mosaic's width: the extra shared by its drawn zones
        let stretchy: f64 = row
            .iter()
            .filter(|&&i| frames[i].2 != BARE && on("grid-justify"))
            .map(|&i| sizes[i].0)
            .sum();
        let spare = width - used;
        let mut x = 24.0 + if stretchy > 0.0 { 0.0 } else { spare / 2.0 };
        for &i in row {
            let b = &blocks[order[i]];
            let (w0, h0, p) = frames[i];
            let drawn = p != BARE;
            let w = if drawn && stretchy > 0.0 {
                w0 + spare * w0 / stretchy
            } else {
                w0
            };
            let h = if drawn { rh } else { h0 };
            if drawn {
                out.insert(b.id.clone(), Box2 { x, y, w, h });
            }
            let (cw, _) = b.content(ctx, pitch.get(&i).copied());
            let ox = x + p.0 + (w - p.0 - p.2 - cw) / 2.0;
            let oy = y + if drawn { head_of(i) } else { p.1 };
            match &b.fixed {
                // a zone holding zones, laid by `tile`: its boxes are relative to each container
                Some((rel, _, _)) if is_zone(b) => {
                    super::mosaic::place(model, ctx, Some(&b.id), (ox, oy), rel, &mut out);
                }
                _ => {
                    for (id, r) in b.local(ctx, pitch.get(&i).copied()) {
                        out.insert(
                            id,
                            Box2 {
                                x: ox + r.x,
                                y: oy + r.y,
                                ..r
                            },
                        );
                    }
                }
            }
            x += w + gx;
        }
        y += rh + gy;
    }
    out
}

/// The blocks of the drawing, in writing order.
fn blocks_of(model: &Model, scene: &crate::scene::Scene, ctx: &Ctx) -> Vec<Block> {
    let size = |id: &str| ctx.sizes.cards.get(id).map(|s| (s.w, s.h));
    let mut blocks: Vec<Block> = Vec::new();
    for c in model.children(None) {
        let kids: Vec<&skemo_lang::Node> = model.children(Some(&c.id)).collect();
        if c.kind == Kind::Zone && !kids.is_empty() && kids.iter().all(|k| k.kind == Kind::Card) {
            let cards: Vec<String> = kids.iter().map(|k| k.id.clone()).collect();
            // one cell for all: the widest card's width, the tallest one's height
            let cell = cards
                .iter()
                .filter_map(|k| size(k))
                .fold((0.0, 0.0), |m: (f64, f64), s| (m.0.max(s.0), m.1.max(s.1)));
            let levels = levels(model, scene, &cards);
            blocks.push(Block {
                id: c.id.clone(),
                cards,
                levels,
                across: false,
                cell,
                fixed: None,
            });
        } else {
            let mut rel = BTreeMap::new();
            let (w, h) = if c.kind == Kind::Zone {
                tile(model, ctx, Some(&c.id), &mut rel).1
            } else {
                let (w, h) = size(&c.id).unwrap_or((100.0, 40.0));
                rel.insert(
                    c.id.clone(),
                    Box2 {
                        x: 0.0,
                        y: 0.0,
                        w,
                        h,
                    },
                );
                (w, h)
            };
            blocks.push(Block {
                id: c.id.clone(),
                cards: Vec::new(),
                levels: Vec::new(),
                across: false,
                cell: (0.0, 0.0),
                fixed: Some((rel, w, h)),
            });
        }
    }
    blocks
}

/// The order the search starts from: the writing order, the start first. The start draws the eye
/// first: the block the most arrows leave for the others, and the fewest come into.
fn start_first(model: &Model, blocks: &[Block], arrows: &[(String, String, f64)]) -> Vec<usize> {
    let top_of = |id: &str| -> String {
        let mut cur = id;
        while let Some(p) = model.node(cur).and_then(|n| n.parent.as_deref()) {
            cur = p;
        }
        cur.to_string()
    };
    let mut net: BTreeMap<String, f64> = BTreeMap::new();
    for (a, b, w) in arrows {
        let (ta, tb) = (top_of(a), top_of(b));
        if ta != tb {
            *net.entry(ta).or_insert(0.0) += w;
            *net.entry(tb).or_insert(0.0) -= w;
        }
    }
    let mut order: Vec<usize> = (0..blocks.len()).collect();
    if !on("grid-start") {
        return order;
    }
    if let Some(main) = (0..blocks.len()).max_by(|&a, &b| {
        let v = |i: usize| net.get(&blocks[i].id).copied().unwrap_or(0.0);
        v(a).total_cmp(&v(b)).then(b.cmp(&a))
    }) {
        order.retain(|&i| i != main);
        order.insert(0, main);
    }
    order
}

/// The placement of the grid engine: every card's box and every zone's frame, absolute.
pub(super) fn place(
    model: &Model,
    scene: &crate::scene::Scene,
    sizes: &Sizes,
    (pad, head): (f64, f64),
) -> BTreeMap<String, Box2> {
    let ctx = Ctx {
        sizes,
        pad,
        head,
        room: true,
    };
    // the search reads the drawing without the room made: the structure never depends on it
    let bare = ctx.bare();
    let mut blocks = blocks_of(model, scene, &bare);
    let arrows: Vec<(String, String, f64)> = model
        .edges
        .iter()
        .map(|e| (e.from.clone(), e.to.clone(), weight(scene, &e.id)))
        .collect();
    // what the search shortens: the arrows' lengths between the centres of their cards
    let cost = |blocks: &[Block], order: &[usize]| -> f64 {
        let boxes = compose(
            model,
            &bare,
            blocks,
            order,
            &split(model, &bare, blocks, order),
        );
        arrows
            .iter()
            .filter_map(|(a, b, w)| {
                let (p, q) = (boxes.get(a)?, boxes.get(b)?);
                let d = (p.x + p.w / 2.0 - q.x - q.w / 2.0).abs()
                    + (p.y + p.h / 2.0 - q.y - q.h / 2.0).abs();
                Some(w * d)
            })
            .sum()
    };
    let mut order = start_first(model, &blocks, &arrows);
    let mut best = cost(&blocks, &order);
    // a move is kept when it shortens the arrows
    let mut shorter = |blocks: &[Block], order: &[usize]| {
        let c = cost(blocks, order);
        let kept = c < best - 0.5;
        if kept {
            best = c;
        }
        kept
    };
    for _ in 0..40 {
        let mut improved = false;
        // two blocks swapped (the start keeps its place)
        for a in 1..order.len() {
            for b in a + 1..order.len() {
                order.swap(a, b);
                if on("grid-swap-blocks") && shorter(&blocks, &order) {
                    improved = true;
                } else {
                    order.swap(a, b);
                }
            }
        }
        // a block's sequences turned
        for i in 0..blocks.len() {
            if blocks[i].fixed.is_some() {
                continue;
            }
            blocks[i].across = !blocks[i].across;
            if on("grid-turn") && shorter(&blocks, &order) {
                improved = true;
            } else {
                blocks[i].across = !blocks[i].across;
            }
        }
        // two cards of one level swapped
        for i in 0..blocks.len() {
            for l in 0..blocks[i].levels.len() {
                let len = blocks[i].levels[l].len();
                for a in 0..len {
                    for b in a + 1..len {
                        blocks[i].levels[l].swap(a, b);
                        if on("grid-swap-cards") && shorter(&blocks, &order) {
                            improved = true;
                        } else {
                            blocks[i].levels[l].swap(a, b);
                        }
                    }
                }
            }
        }
        if !improved {
            break;
        }
    }
    let rows = split(model, &bare, &blocks, &order);
    if sizes.spacing.is_empty() {
        return compose(model, &bare, &blocks, &order, &rows);
    }
    // the room made, evenly (`Sizes::spacing`): the same blocks in the same rows, the gutters of
    // a whole group wider
    let mut spread = blocks_of(model, scene, &ctx);
    for (s, b) in spread.iter_mut().zip(&blocks) {
        s.levels.clone_from(&b.levels);
        s.across = b.across;
    }
    compose(model, &ctx, &spread, &order, &rows)
}
