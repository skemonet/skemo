//! The mosaic: a container's children as tiles in rows, in writing order.
//!
//! No arrow shapes it. In a zone holding only cards, the cards take one size (the largest) and
//! fill a grid whose number of columns brings the zone nearest [`TARGET`]; elsewhere every child
//! is a tile, the tiles fill rows in writing order (the number of rows the one that brings the
//! container nearest [`TARGET`]), the zones of a row take its height and the rows the container's
//! width (a justified mosaic: zones share their edges). The grid engine lays this way the zones
//! that hold zones (`blocks`), whose cards it does not order by their arrows yet.
//!
//! The gaps are even: one gutter between all the children of a container, along each axis
//! ([`Ctx::gap`]). The room the lines and the labels lack widens the gutters of a whole container
//! (`Sizes::spacing`), never one gap among the others; the structure (how many rows, how many
//! columns) is the one of the drawing without that room, so that making room only spreads it.

use crate::layout::Box2;
use crate::measure::Sizes;
use skemo_lang::{Kind, Model};
use std::collections::BTreeMap;

/// Share of the screen a mosaic aims at (width over height).
pub(super) const TARGET: f64 = 1.6;
/// One gap between tiles, zones and cards alike, before any room is made.
pub(super) const GUTTER: f64 = 28.0;

/// A width and a height.
type Wh = (f64, f64);

/// What every container's mosaic reads: the measured sizes, the zones' padding and the room of
/// their title.
#[derive(Clone, Copy)]
pub(super) struct Ctx<'a> {
    pub sizes: &'a Sizes,
    pub pad: f64,
    pub head: f64,
    /// With the room made for the lines and the labels (`Sizes::spacing`); without it, the
    /// drawing as first laid, which decides the structure.
    pub room: bool,
}

impl Ctx<'_> {
    /// The same drawing without the room made.
    pub fn bare(&self) -> Self {
        Self {
            room: false,
            ..*self
        }
    }

    /// The room made in `group` (`None`: the drawing) along x (`across`) or y: what its gutters
    /// gain, or what its padding must be at least.
    fn made(&self, group: Option<&str>, across: bool, padding: bool) -> f64 {
        if !self.room || self.sizes.spacing.is_empty() {
            return 0.0;
        }
        let key = (group.unwrap_or("").to_string(), across, padding);
        self.sizes.spacing.get(&key).copied().unwrap_or(0.0)
    }

    /// The gap between two children of `group` (`None`: the drawing), along x (`across`) or y:
    /// the same between all of them (`SKEMO_JITTER` adds to it, to measure the noise).
    pub fn gap(&self, group: Option<&str>, across: bool) -> f64 {
        GUTTER + crate::trace::jitter() + self.made(group, across, false)
    }

    /// Both gaps of `group`: along x, along y.
    pub fn gaps(&self, group: Option<&str>) -> Wh {
        (self.gap(group, true), self.gap(group, false))
    }
}

/// The room a zone keeps around its content: left, top, right, bottom. The room made in its
/// padding is the same on both sides; along y, it is the room above its first row.
pub(super) fn padding(model: &Model, ctx: &Ctx, id: &str) -> (f64, f64, f64, f64) {
    let Some(n) = model.node(id) else {
        return (0.0, 0.0, 0.0, 0.0);
    };
    if crate::layout::invisible(n) {
        return (0.0, 0.0, 0.0, 0.0);
    }
    let (pad, head) = (ctx.pad, ctx.head);
    let extra = ctx.sizes.zone_heads.get(id).copied().unwrap_or(0.0);
    let side = pad.max(ctx.made(Some(id), true, true));
    let top = (head + extra + pad * 0.6).max(ctx.made(Some(id), false, true));
    (side, top, side, pad)
}

/// How far a `w` × `h` container stands from [`TARGET`].
fn aspect((w, h): Wh) -> f64 {
    (w / h.max(1.0) / TARGET).ln().abs()
}

/// The size of the tiles `sizes` set in `rows`, `gap` apart (across, down).
pub(super) fn extent(rows: &[Vec<usize>], sizes: &[Wh], gap: Wh) -> Wh {
    let between = |k: usize| k.saturating_sub(1) as f64;
    let w = rows
        .iter()
        .map(|r| r.iter().map(|&i| sizes[i].0).sum::<f64>() + gap.0 * between(r.len()))
        .fold(0.0, f64::max);
    let h = rows
        .iter()
        .map(|r| r.iter().map(|&i| sizes[i].1).fold(0.0, f64::max))
        .sum::<f64>()
        + gap.1 * between(rows.len());
    (w, h)
}

/// Tiles of `sizes` in rows, in their order: the number of rows that brings the whole nearest
/// [`TARGET`], the tiles [`GUTTER`] apart. Returns the rows (indices into `sizes`).
pub(super) fn rows(sizes: &[Wh]) -> Vec<Vec<usize>> {
    let n = sizes.len();
    let total: f64 = sizes.iter().map(|s| s.0).sum::<f64>() + GUTTER * n.saturating_sub(1) as f64;
    let split = |rows: usize| -> Vec<Vec<usize>> {
        let goal = total / rows as f64;
        let mut out: Vec<Vec<usize>> = vec![Vec::new()];
        let mut w = 0.0;
        for (i, size) in sizes.iter().enumerate() {
            let add = size.0 + if w > 0.0 { GUTTER } else { 0.0 };
            if w > 0.0 && w + add / 2.0 > goal && out.len() < rows {
                out.push(Vec::new());
                w = 0.0;
            }
            w += size.0 + if w > 0.0 { GUTTER } else { 0.0 };
            if let Some(r) = out.last_mut() {
                r.push(i);
            }
        }
        out
    };
    let measure = |rows: &Vec<Vec<usize>>| extent(rows, sizes, (GUTTER, GUTTER));
    (1..=n.max(1))
        .map(split)
        .min_by(|a, b| aspect(measure(a)).total_cmp(&aspect(measure(b))))
        .unwrap_or_default()
}

/// Natural size of a child: a card's measure, a zone's tiles with its frame. Without the room
/// made, then with it.
fn natural(model: &Model, ctx: &Ctx, id: &str, rel: &mut BTreeMap<String, Box2>) -> (Wh, Wh) {
    match model.node(id) {
        Some(n) if n.kind == Kind::Zone => {
            let (bare, full) = tile(model, ctx, Some(id), rel);
            let framed = |ctx: &Ctx, (cw, ch): Wh| {
                let (l, t, r, b) = padding(model, ctx, id);
                let title = ctx.sizes.title_ends.get(id).map_or(0.0, |e| e + ctx.pad);
                ((cw + l + r).max(title), ch + t + b)
            };
            (framed(&ctx.bare(), bare), framed(ctx, full))
        }
        _ => {
            let s = ctx
                .sizes
                .cards
                .get(id)
                .map_or((100.0, 40.0), |s| (s.w, s.h));
            (s, s)
        }
    }
}

/// Lays out the children of `parent` as a mosaic; their boxes, relative to its content origin, go
/// to `rel`. Returns the content's size: without the room made (what decides the rows and the
/// columns), then with it (what is laid).
pub(super) fn tile(
    model: &Model,
    ctx: &Ctx,
    parent: Option<&str>,
    rel: &mut BTreeMap<String, Box2>,
) -> (Wh, Wh) {
    let kids: Vec<&skemo_lang::Node> = model.children(parent).collect();
    if kids.is_empty() {
        return ((0.0, 0.0), (0.0, 0.0));
    }
    let both: Vec<(Wh, Wh)> = kids
        .iter()
        .map(|k| natural(model, ctx, &k.id, rel))
        .collect();
    let bare: Vec<Wh> = both.iter().map(|b| b.0).collect();
    let sizes: Vec<Wh> = both.iter().map(|b| b.1).collect();
    let (gx, gy) = ctx.gaps(parent);
    let n = kids.len();
    if kids.iter().all(|k| k.kind == Kind::Card) {
        // one size for all, a grid of columns nearest the target
        let cw = sizes.iter().map(|s| s.0).fold(0.0, f64::max);
        let ch = sizes.iter().map(|s| s.1).fold(0.0, f64::max);
        let grid = |c: usize, gap: Wh| {
            let r = n.div_ceil(c);
            (
                c as f64 * cw + (c - 1) as f64 * gap.0,
                r as f64 * ch + (r - 1) as f64 * gap.1,
            )
        };
        let plain = (GUTTER, GUTTER);
        let best = (1..=n)
            .min_by(|&a, &b| aspect(grid(a, plain)).total_cmp(&aspect(grid(b, plain))))
            .unwrap_or(1);
        for (i, k) in kids.iter().enumerate() {
            let (r, c) = (i / best, i % best);
            rel.insert(
                k.id.clone(),
                Box2 {
                    x: c as f64 * (cw + gx),
                    y: r as f64 * (ch + gy),
                    w: cw,
                    h: ch,
                },
            );
        }
        return (grid(best, plain), grid(best, (gx, gy)));
    }
    // tiles in rows, in writing order, the number of rows nearest the target
    let rows = rows(&bare);
    let (width, height) = extent(&rows, &sizes, (gx, gy));
    let mut y = 0.0;
    for row in &rows {
        let rh = row.iter().map(|&i| sizes[i].1).fold(0.0, f64::max);
        let used: f64 = row.iter().map(|&i| sizes[i].0).sum::<f64>() + gx * (row.len() - 1) as f64;
        // the row stretched to the width: the extra shared by its zones (cards keep their width)
        let zones: f64 = row
            .iter()
            .filter(|&&i| kids[i].kind == Kind::Zone)
            .map(|&i| sizes[i].0)
            .sum();
        let spare = width - used;
        let mut x = if zones > 0.0 { 0.0 } else { spare / 2.0 };
        for &i in row {
            let (w0, h0) = sizes[i];
            let zone = kids[i].kind == Kind::Zone;
            let w = if zone && zones > 0.0 {
                w0 + spare * w0 / zones
            } else {
                w0
            };
            let h = if zone { rh } else { h0 };
            rel.insert(kids[i].id.clone(), Box2 { x, y, w, h });
            x += w + gx;
        }
        y += rh + gy;
    }
    (extent(&rows, &bare, (GUTTER, GUTTER)), (width, height))
}

/// Absolute boxes of the mosaic laid by [`tile`], top-down.
pub(super) fn place(
    model: &Model,
    ctx: &Ctx,
    parent: Option<&str>,
    at: (f64, f64),
    rel: &BTreeMap<String, Box2>,
    out: &mut BTreeMap<String, Box2>,
) {
    for c in model.children(parent) {
        let Some(r) = rel.get(&c.id) else { continue };
        let b = Box2 {
            x: at.0 + r.x,
            y: at.1 + r.y,
            ..*r
        };
        out.insert(c.id.clone(), b);
        if c.kind == Kind::Zone {
            let (l, t, r, _) = padding(model, ctx, &c.id);
            // a zone wider than its content (stretched to its row, or as wide as its title)
            // keeps it centred across, at the top
            let content = model
                .children(Some(&c.id))
                .filter_map(|k| rel.get(&k.id))
                .map(Box2::right)
                .fold(0.0, f64::max);
            let shift = ((b.w - l - r - content) / 2.0).max(0.0);
            place(
                model,
                ctx,
                Some(&c.id),
                (b.x + l + shift, b.y + t),
                rel,
                out,
            );
        }
    }
}
