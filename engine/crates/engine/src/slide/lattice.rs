//! The slide on a lattice of 10 px (`[track]`): cards in the cells of a tiling, every size and gap
//! a whole number of steps. Coordinates are in steps unless a name ends in `_px`.

use super::{BoxPx, Input, Rect, STEP, Walls};
use std::collections::BTreeMap;

/// From a card to its frame (`[zone-pad]`), in steps.
pub(crate) const PAD: i32 = 3;
/// Around the drawing: room for a line going round when not every card is framed.
pub(crate) const MARGIN: i32 = 5;

/// The number of steps of each kind of gutter: between two cards of a group (`gin_*`) and the
/// free channel between two groups, beside their padding (`chan_*`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Gaps {
    pub gin_x: i32,
    pub gin_y: i32,
    pub chan_x: i32,
    pub chan_y: i32,
}

impl Gaps {
    pub(crate) fn plus(self, n: i32) -> Gaps {
        Gaps {
            gin_x: self.gin_x + n,
            gin_y: self.gin_y + n,
            chan_x: self.chan_x + n,
            chan_y: self.chan_y + n,
        }
    }
}

/// Room made in one place rather than on every gutter of a kind (P11): `x[k]` more steps in the
/// gutter after column k, `y[k]` the same after row k, `head[r]` the cards of row r lower under
/// their zone's title (the frame's top stays: the cards move down inside it).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Extra {
    pub x: BTreeMap<usize, i32>,
    pub y: BTreeMap<usize, i32>,
    pub head: BTreeMap<usize, i32>,
}

/// Where a lattice puts the cards: the tiling's rectangle of cells per panel and each card's cell.
#[derive(Clone, Debug)]
pub(crate) struct Tiling {
    pub cols: usize,
    pub rows: usize,
    pub rects: Vec<(usize, usize, usize, usize)>,
    pub pos: Vec<(usize, usize)>,
}

#[derive(Clone, Debug)]
pub(crate) struct Lattice {
    pub cols: usize,
    pub rows: usize,
    pub cw: i32,
    pub ch: i32,
    pub gaps: Gaps,
    pub extra: Extra,
    pub xgap: Vec<i32>,
    pub ygap: Vec<i32>,
    pub xout: Vec<bool>,
    pub yout: Vec<bool>,
    pub colx: Vec<i32>,
    pub rowy: Vec<i32>,
    pub zone_head: i32,
    pub card: Vec<Rect>,
    pub frame: Vec<Option<Rect>>,
    pub width: i32,
    pub height: i32,
    /// The texts of each framed panel (title, description, tag), px: lines never cross them.
    pub texts: Vec<Vec<BoxPx>>,
}

impl Lattice {
    /// The lattice of `tiling` with these gutters; the zone header is as tall as the tallest
    /// title needs (one height for every zone: rows level across groups, P8).
    pub(crate) fn new(
        inp: &Input,
        tiling: &Tiling,
        gaps: Gaps,
        extra: &Extra,
        walls: &Walls,
    ) -> Lattice {
        let mut head = inp.head_steps;
        let mut l = Self::with_head(inp, tiling, gaps, extra, head, walls);
        for _ in 0..3 {
            let need = l.head_needed();
            if need <= head {
                break;
            }
            head = need;
            l = Self::with_head(inp, tiling, gaps, extra, head, walls);
        }
        l
    }

    /// The header the texts ask: from the frame's top to a step under the lowest text.
    fn head_needed(&self) -> i32 {
        let mut need = 0;
        for (i, f) in self.frame.iter().enumerate() {
            let Some(f) = f else { continue };
            for b in &self.texts[i] {
                let low = b.y + b.h - f64::from(f.y) * STEP;
                need = need.max(((low + 6.0) / STEP).ceil() as i32);
            }
        }
        need
    }

    fn with_head(
        inp: &Input,
        tiling: &Tiling,
        gaps: Gaps,
        extra: &Extra,
        zone_head: i32,
        walls: &Walls,
    ) -> Lattice {
        let (cols, rows) = (tiling.cols, tiling.rows);
        let (cw, ch) = (inp.card_w, inp.card_h);
        let owner = owners(tiling);
        let ex = |m: &BTreeMap<usize, i32>, k: usize| m.get(&k).copied().unwrap_or(0);
        // a gutter between two groups holds both paddings and the channel; one inside a group,
        // the group's gap; above a row of other groups, their titles too
        let xout: Vec<bool> = (1..cols)
            .map(|k| (0..rows).any(|r| owner[r * cols + k - 1] != owner[r * cols + k]))
            .collect();
        let xgap: Vec<i32> = xout
            .iter()
            .enumerate()
            .map(|(i, &out)| {
                let g = if out {
                    2 * PAD + gaps.chan_x
                } else {
                    gaps.gin_x
                };
                g + ex(&extra.x, i)
            })
            .collect();
        let yout: Vec<bool> = (1..rows)
            .map(|k| (0..cols).any(|c| owner[(k - 1) * cols + c] != owner[k * cols + c]))
            .collect();
        let ygap: Vec<i32> = yout
            .iter()
            .enumerate()
            .map(|(i, &out)| {
                let g = if out {
                    PAD + gaps.chan_y + zone_head
                } else {
                    gaps.gin_y
                };
                g + ex(&extra.y, i) + ex(&extra.head, i + 1)
            })
            .collect();
        let colx = offsets(MARGIN + PAD, cw, &xgap, cols);
        let rowy = offsets(MARGIN + zone_head + ex(&extra.head, 0), ch, &ygap, rows);
        // a pile takes a step more right and down: the lines keep clear of the cards behind
        let card = tiling
            .pos
            .iter()
            .zip(&inp.pile)
            .map(|(&(cx, cy), &m)| {
                let more = i32::from(m > 0.0);
                Rect {
                    x: colx[cx],
                    y: rowy[cy],
                    w: cw + more,
                    h: ch + more,
                }
            })
            .collect();
        let frame: Vec<Option<Rect>> = tiling
            .rects
            .iter()
            .enumerate()
            .map(|(i, &(x0, y0, w, h))| {
                inp.panels[i].zone.as_ref()?;
                let l = colx[x0] - PAD;
                let r = colx[x0 + w - 1] + cw + PAD;
                let t = rowy[y0] - zone_head - ex(&extra.head, y0);
                let b = rowy[y0 + h - 1] + ch + PAD;
                Some(Rect {
                    x: l,
                    y: t,
                    w: r - l,
                    h: b - t,
                })
            })
            .collect();
        let width = colx[cols - 1] + cw + PAD + MARGIN;
        let height = rowy[rows - 1] + ch + PAD + MARGIN;
        let frames_px: Vec<(usize, BoxPx)> = frame
            .iter()
            .enumerate()
            .filter_map(|(i, f)| f.map(|f| (i, f.px())))
            .collect();
        let mut texts = vec![Vec::new(); tiling.rects.len()];
        for (i, b) in walls(&frames_px) {
            texts[i].push(b);
        }
        Lattice {
            cols,
            rows,
            cw,
            ch,
            gaps,
            extra: extra.clone(),
            xgap,
            ygap,
            xout,
            yout,
            colx,
            rowy,
            zone_head,
            card,
            frame,
            width,
            height,
            texts,
        }
    }

    /// The frames holding card `c` (its panel's, when framed).
    pub(crate) fn zones_of(&self, inp: &Input, c: usize) -> Option<usize> {
        let p = inp.card_panel[c];
        self.frame[p].map(|_| p)
    }

    /// The gutter column `x` (steps) lies in, between columns k and k+1.
    pub(crate) fn gutter_x(&self, x: i32) -> Option<usize> {
        (0..self.cols.saturating_sub(1))
            .find(|&c| self.colx[c] + self.cw <= x && x <= self.colx[c + 1])
    }

    /// The gutter or title band row `y` lies in: `(false, k)` the gutter after row k, `(true, r)`
    /// the header above row r.
    pub(crate) fn gutter_y(&self, y: i32) -> Option<(bool, usize)> {
        let head = |r: usize| self.zone_head + self.extra.head.get(&r).copied().unwrap_or(0);
        for r in 0..self.rows.saturating_sub(1) {
            let (lo, hi) = (self.rowy[r] + self.ch, self.rowy[r + 1]);
            if lo <= y && y <= hi {
                let head_top = hi - head(r + 1);
                return Some(if self.yout[r] && y >= head_top {
                    (true, r + 1)
                } else {
                    (false, r)
                });
            }
        }
        (self.rowy[0] - head(0) <= y && y <= self.rowy[0]).then_some((true, 0))
    }
}

/// The panel owning each cell of the tiling, row by row.
fn owners(tiling: &Tiling) -> Vec<usize> {
    let cols = tiling.cols;
    let mut owner = vec![usize::MAX; cols * tiling.rows];
    for (i, &(x, y, w, h)) in tiling.rects.iter().enumerate() {
        for cx in x..x + w {
            for cy in y..y + h {
                owner[cy * cols + cx] = i;
            }
        }
    }
    owner
}

/// Where each of `n` cells of size `size` starts, from `start`, with `gaps` between them.
fn offsets(start: i32, size: i32, gaps: &[i32], n: usize) -> Vec<i32> {
    let mut out = Vec::with_capacity(n);
    let mut at = start;
    for k in 0..n {
        out.push(at);
        at += size + gaps.get(k).copied().unwrap_or(0);
    }
    out
}
