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
    /// Each row's height: its tallest card's (P7, V5: the other cards of the row align).
    pub rowh: Vec<i32>,
    pub gaps: Gaps,
    pub extra: Extra,
    pub xgap: Vec<i32>,
    pub ygap: Vec<i32>,
    pub xout: Vec<bool>,
    pub yout: Vec<bool>,
    pub colx: Vec<i32>,
    pub rowy: Vec<i32>,
    /// The header of the innermost frames opening above each row.
    pub zone_head: Vec<i32>,
    pub card: Vec<Rect>,
    pub frame: Vec<Option<Rect>>,
    pub width: i32,
    pub height: i32,
    /// The texts of each framed panel (title, description, tag), px: lines never cross them.
    pub texts: Vec<Vec<BoxPx>>,
}

impl Lattice {
    /// The lattice of `tiling` with these gutters; the header above a row is as tall as the
    /// tallest title of the frames opening there needs (rows level across groups, P8).
    pub(crate) fn new(
        inp: &Input,
        tiling: &Tiling,
        gaps: Gaps,
        extra: &Extra,
        walls: &Walls,
    ) -> Lattice {
        let cells = frame_cells(inp, tiling);
        let levels = levels(inp);
        // a level of frames opening on a row: a header; a frame with its title beside its cards
        // (`head="left"`) only its padding above, and a column on the left instead
        let mut top: Vec<Vec<Option<i32>>> = vec![vec![None; levels]; tiling.rows];
        for (i, c) in cells.iter().enumerate() {
            if let Some((_, y0, _, _)) = c {
                let d = inp.chain(i).len() - 1;
                let h = if inp.panels[i].side() {
                    PAD
                } else {
                    inp.head_steps.max(inp.heads[i])
                };
                top[*y0][d] = Some(top[*y0][d].map_or(h, |t| t.max(h)));
            }
        }
        let mut bands = Bands {
            top: top
                .into_iter()
                .map(|r| r.into_iter().map(|h| h.unwrap_or(inp.head_steps)).collect())
                .collect(),
            left: vec![vec![0; levels]; tiling.cols],
        };
        let mut l = Self::with_head(inp, tiling, gaps, extra, &bands, walls);
        for _ in 0..4 {
            let need = l.bands_needed(inp, &cells);
            let mut grew = false;
            for (h, n) in (bands.top.iter_mut().flatten()).zip(need.top.iter().flatten()) {
                grew |= n > h;
                *h = (*h).max(*n);
            }
            for (h, n) in (bands.left.iter_mut().flatten()).zip(need.left.iter().flatten()) {
                grew |= n > h;
                *h = (*h).max(*n);
            }
            if !grew {
                break;
            }
            l = Self::with_head(inp, tiling, gaps, extra, &bands, walls);
        }
        l
    }

    /// The bands the texts ask: a header from the frame's top to a step under the lowest text; a
    /// column (`head="left"`) from the frame's padding to a step right of the widest text. A
    /// caption below the cards (`.bare`) asks none: counted, it moved down with every header it
    /// asked.
    fn bands_needed(&self, inp: &Input, cells: &[Option<(usize, usize, usize, usize)>]) -> Bands {
        let levels = levels(inp);
        let mut need = Bands {
            top: vec![vec![0; levels]; self.rows],
            left: vec![vec![0; levels]; self.cols],
        };
        for (i, f) in self.frame.iter().enumerate() {
            let (Some(f), Some((x0, y0, _, _))) = (f.filter(|_| !inp.panels[i].below()), cells[i])
            else {
                continue;
            };
            let d = inp.chain(i).len() - 1;
            for b in &self.texts[i] {
                if inp.panels[i].side() {
                    let right = b.x + b.w - f64::from(f.x + PAD) * STEP;
                    let n = ((right + STEP) / STEP).ceil() as i32;
                    need.left[x0][d] = need.left[x0][d].max(n);
                } else {
                    let low = b.y + b.h - f64::from(f.y) * STEP;
                    let n = ((low + 6.0) / STEP).ceil() as i32;
                    need.top[y0][d] = need.top[y0][d].max(n);
                }
            }
        }
        need
    }

    fn with_head(
        inp: &Input,
        tiling: &Tiling,
        gaps: Gaps,
        extra: &Extra,
        bands: &Bands,
        walls: &Walls,
    ) -> Lattice {
        let zone_head = &bands.top;
        let (cols, rows) = (tiling.cols, tiling.rows);
        let cw = inp.card_w;
        let rowh = row_heights(inp, tiling);
        let ex = |m: &BTreeMap<usize, i32>, k: usize| m.get(&k).copied().unwrap_or(0);
        let Gutters {
            xgap,
            xout,
            ygap,
            yout,
            edges: (left, top, right, bottom),
            deep,
            piles,
        } = gutters(inp, tiling, gaps, extra, bands);
        let colx = offsets(
            MARGIN + PAD * left + sum(&bands.left, 0, 0, left),
            &vec![cw; cols],
            &xgap,
        );
        let rowy = offsets(
            MARGIN + sum(zone_head, 0, 0, top) + ex(&extra.head, 0),
            &rowh,
            &ygap,
        );
        // a pile takes a step more right and down: the lines keep clear of the cards behind
        // a wide card, a bar, spans its row (its column across `dir`)
        let card = tiling
            .pos
            .iter()
            .zip(&inp.pile)
            .enumerate()
            .map(|(c, (&(cx, cy), &m))| {
                if inp.wide(c) {
                    let (x0, y0, w, h) = tiling.rects[inp.card_panel[c]];
                    return Rect {
                        x: colx[x0],
                        y: rowy[y0],
                        w: colx[x0 + w - 1] + cw - colx[x0],
                        h: rowy[y0 + h - 1] + rowh[y0 + h - 1] - rowy[y0],
                    };
                }
                let more = i32::from(m > 0.0);
                Rect {
                    x: colx[cx],
                    y: rowy[cy],
                    w: cw + more,
                    h: rowh[cy] + more,
                }
            })
            .collect();
        let frame = frames(
            inp,
            tiling,
            (&colx, &rowy),
            (cw, &rowh),
            (bands, &deep),
            extra,
        );
        let width = colx[cols - 1] + cw + PAD * right + piles.0 + MARGIN;
        let height = rowy[rows - 1] + rowh[rows - 1] + PAD * bottom + piles.1 + MARGIN;
        let frames_px: Vec<(usize, BoxPx)> = frame
            .iter()
            .enumerate()
            .filter_map(|(i, f)| f.map(|f| (i, top_frame(inp, i, f))))
            .collect();
        let mut texts = vec![Vec::new(); tiling.rects.len()];
        for (i, b) in walls(&frames_px) {
            texts[i].push(b);
        }
        Lattice {
            cols,
            rows,
            cw,
            rowh,
            gaps,
            extra: extra.clone(),
            xgap,
            ygap,
            xout,
            yout,
            colx,
            rowy,
            zone_head: (0..rows)
                .map(|r| zone_head[r][(deep.1[r] - 1) as usize])
                .collect(),
            card,
            frame,
            width,
            height,
            texts,
        }
    }

    /// The box an end of arrow `e` stands for (`start`: its source): a card, a zone's frame, or
    /// round a group drawn without one (a stack, a bare zone).
    pub(crate) fn end(&self, inp: &Input, e: &super::input::Edge, start: bool) -> Rect {
        let (c, z) = if start {
            (e.from, e.ends.0)
        } else {
            (e.to, e.ends.1)
        };
        let Some(z) = z else {
            return self.card[c];
        };
        if let Some(f) = self.frame[z] {
            return f;
        }
        let inside: Vec<Rect> = (0..inp.cards.len())
            .filter(|&k| {
                let mut at = Some(inp.card_panel[k]);
                while let Some(q) = at {
                    if q == z {
                        return true;
                    }
                    at = inp.panels[q].parent;
                }
                false
            })
            .map(|k| self.card[k])
            .collect();
        let x0 = inside.iter().map(|r| r.x).min().unwrap_or(self.card[c].x);
        let y0 = inside.iter().map(|r| r.y).min().unwrap_or(self.card[c].y);
        let x1 = inside
            .iter()
            .map(|r| r.x + r.w)
            .max()
            .unwrap_or(x0 + self.cw);
        let y1 = inside
            .iter()
            .map(|r| r.y + r.h)
            .max()
            .unwrap_or(y0 + self.card[c].h);
        Rect {
            x: x0 - 1,
            y: y0 - 1,
            w: x1 - x0 + 2,
            h: y1 - y0 + 2,
        }
    }

    /// The frames holding card `c`, its own first, outwards.
    pub(crate) fn zones_of(&self, inp: &Input, c: usize) -> Vec<usize> {
        inp.zones_of(c)
            .into_iter()
            .filter(|&z| self.frame[z].is_some())
            .collect()
    }

    /// The gutter column `x` (steps) lies in, between columns k and k+1.
    pub(crate) fn gutter_x(&self, x: i32) -> Option<usize> {
        (0..self.cols.saturating_sub(1))
            .find(|&c| self.colx[c] + self.cw <= x && x <= self.colx[c + 1])
    }

    /// The gutter or title band row `y` lies in: `(false, k)` the gutter after row k, `(true, r)`
    /// the header above row r.
    pub(crate) fn gutter_y(&self, y: i32) -> Option<(bool, usize)> {
        let head = |r: usize| self.zone_head[r] + self.extra.head.get(&r).copied().unwrap_or(0);
        for r in 0..self.rows.saturating_sub(1) {
            let (lo, hi) = (self.rowy[r] + self.rowh[r], self.rowy[r + 1]);
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
/// Panel `i`'s frame as drawn, px: a pile's (`zones`, D-114) without the step its frames behind
/// take.
pub(crate) fn top_frame(inp: &Input, i: usize, f: Rect) -> BoxPx {
    let mut b = f.px();
    if inp.panels[i].pile_step() > 0 {
        b.w -= STEP;
        b.h -= STEP;
    }
    b
}

fn owners(tiling: &Tiling) -> Vec<usize> {
    let cols = tiling.cols;
    let mut owner = vec![usize::MAX; cols * tiling.rows];
    for (i, &(x, y, w, h)) in tiling.rects.iter().enumerate() {
        // a zone holding no card of its own has no cells
        for cx in x..x + w {
            for cy in y..y + h {
                owner[cy * cols + cx] = i;
            }
        }
    }
    owner
}

/// The headers above each row, one per level of frames, the outermost first (steps): each as
/// tall as the tallest title and description of the frames of that level opening on that row
/// (rows level across groups, P8; a frame round them keeps its own, shorter, header).
type Heads = Vec<Vec<i32>>;

/// The bands round the cells: a header above each row and a column left of each column of cells,
/// one per level of frames.
pub(crate) struct Bands {
    top: Heads,
    left: Heads,
}

/// The levels of frames: the deepest nesting, one at least.
fn levels(inp: &Input) -> usize {
    (0..inp.panels.len())
        .map(|i| inp.chain(i).len())
        .max()
        .unwrap_or(1)
        .max(1)
}

/// The bands of levels `from..to` at row (column) `r` (a missing level: the last one's).
fn sum(h: &Heads, r: usize, from: i32, to: i32) -> i32 {
    (from.max(0)..to.max(0))
        .map(|j| {
            let row = &h[r];
            row.get(j as usize).or(row.last()).copied().unwrap_or(0)
        })
        .sum()
}

/// Where each cell of these sizes starts, from `start`, with `gaps` between them.
fn offsets(start: i32, sizes: &[i32], gaps: &[i32]) -> Vec<i32> {
    let mut out = Vec::with_capacity(sizes.len());
    let mut at = start;
    for (k, size) in sizes.iter().enumerate() {
        out.push(at);
        at += size + gaps.get(k).copied().unwrap_or(0);
    }
    out
}

/// Each row's height: its tallest card's, a wide card's on its first row (P7, V5: the cards of a
/// row share one height); a row without a card, the smallest card's.
fn row_heights(inp: &Input, tiling: &Tiling) -> Vec<i32> {
    let mut h = vec![4; tiling.rows];
    for (c, &(_, cy)) in tiling.pos.iter().enumerate() {
        let r = if inp.wide(c) {
            tiling.rects[inp.card_panel[c]].1
        } else {
            cy
        };
        if let Some(x) = h.get_mut(r) {
            *x = (*x).max(inp.heights[c]);
        }
    }
    h
}

/// Each zone's frame round its own cards and the frames of the zones inside it, the innermost
/// first (a zone is written before the zones inside it).
fn frames(
    inp: &Input,
    tiling: &Tiling,
    (colx, rowy): (&[i32], &[i32]),
    (cw, rowh): (i32, &[i32]),
    (bands, deep): (&Bands, &(Vec<i32>, Vec<i32>)),
    extra: &Extra,
) -> Vec<Option<Rect>> {
    let ex = |m: &BTreeMap<usize, i32>, k: usize| m.get(&k).copied().unwrap_or(0);
    let (cols_deep, rows_deep) = deep;
    let cells = frame_cells(inp, tiling);
    // a pile of frames (`zones`, D-114) takes a step more right and down, its frame and the
    // frames round it that end on the same column (row) with it
    let piled = |i: usize, x1: usize, y1: usize| {
        let mut more = (0, 0);
        for (p, c) in cells.iter().enumerate() {
            let Some(c) = c.filter(|_| inp.panels[p].pile_step() > 0 && inp.chain(p).contains(&i))
            else {
                continue;
            };
            more.0 = more.0.max(i32::from(c.2 == x1));
            more.1 = more.1.max(i32::from(c.3 == y1));
        }
        more
    };
    let mut frame: Vec<Option<Rect>> = vec![None; inp.panels.len()];
    for (i, f) in frame.iter_mut().enumerate() {
        let Some((x0, y0, x1, y1)) = cells[i] else {
            continue;
        };
        // its level: the outermost frame takes the outermost padding of the gutter
        let d = inp.chain(i).len() as i32;
        let (l, r) = (cols_deep[x0] - d + 1, cols_deep[x1] - d + 1);
        let (t, b) = (rows_deep[y0] - d + 1, rows_deep[y1] - d + 1);
        let (lx, ty) = (
            colx[x0] - PAD * l - sum(&bands.left, x0, d - 1, d - 1 + l),
            rowy[y0] - sum(&bands.top, y0, d - 1, d - 1 + t) - ex(&extra.head, y0),
        );
        let (mr, mb) = piled(i, x1, y1);
        let (rx, by) = (
            colx[x1] + cw + PAD * r + mr,
            rowy[y1] + rowh[y1] + PAD * b + mb,
        );
        *f = Some(Rect {
            x: lx,
            y: ty,
            w: rx - lx,
            h: by - ty,
        });
    }
    frame
}

/// The cells each framed panel's frame holds: those of its cards and of the zones inside it
/// (`zones="fit"`: the cells its cards stand in, not the block they were given), first and last
/// column and row; none for a panel without a frame or without cells.
fn frame_cells(inp: &Input, tiling: &Tiling) -> Vec<Option<(usize, usize, usize, usize)>> {
    (0..inp.panels.len())
        .map(|i| {
            inp.panels[i].zone.as_ref()?;
            let mut cells: Option<(usize, usize, usize, usize)> = None;
            for (j, &(x, y, w, h)) in tiling.rects.iter().enumerate() {
                if w == 0 || h == 0 || !inp.chain(j).contains(&i) {
                    continue;
                }
                let (x, y, w, h) = match fitted(inp, tiling, j) {
                    Some(b) if inp.fit => b,
                    _ => (x, y, w, h),
                };
                let (x1, y1) = (x + w - 1, y + h - 1);
                cells = Some(cells.map_or((x, y, x1, y1), |(a, b, c, d)| {
                    (a.min(x), b.min(y), c.max(x1), d.max(y1))
                }));
            }
            cells
        })
        .collect()
}

/// The gutters between the columns and the rows, and the frames at each edge of the drawing.
struct Gutters {
    xgap: Vec<i32>,
    xout: Vec<bool>,
    ygap: Vec<i32>,
    yout: Vec<bool>,
    /// The frames at the left, top, right and bottom edges (one at least).
    edges: (i32, i32, i32, i32),
    /// The frames each column and each row of cells is nested in at most (one at least).
    deep: (Vec<i32>, Vec<i32>),
    /// A pile of frames ends on the last column, on the last row (a step more each, D-114).
    piles: (i32, i32),
}

/// The gutters of `tiling`: between two groups, a padding for each level of frames on either side
/// each side) and the channel; inside a group, the group's gap; above a row of other groups, a
/// title for each frame opening there.
fn gutters(inp: &Input, tiling: &Tiling, gaps: Gaps, extra: &Extra, bands: &Bands) -> Gutters {
    let (cols, rows) = (tiling.cols, tiling.rows);
    let owner = owners(tiling);
    let ex = |m: &BTreeMap<usize, i32>, k: usize| m.get(&k).copied().unwrap_or(0);
    // the zones holding each cell, its own first (none for a hole or a card outside every zone)
    let chains: Vec<Vec<usize>> = owner
        .iter()
        .map(|&o| {
            if o == usize::MAX {
                Vec::new()
            } else {
                inp.chain(o)
            }
        })
        .collect();
    // two cells of different groups: in different zones, or outside every zone (two blocks of one
    // zone, a stack's layers or a `wrap` zone's children, keep the zone's own gap)
    let apart = |a: usize, b: usize| {
        owner[a] != owner[b] && (chains[a] != chains[b] || chains[a].is_empty())
    };
    // the frames a column (a row) of cells is nested in at most, one at least: each frame keeps
    // one padding (one title above) of the gutter for its level, so that the frames of one level
    // line up along a column or a row (S3), whatever they hold
    let cols_deep: Vec<i32> = (0..cols)
        .map(|c| {
            (0..rows)
                .map(|r| chains[r * cols + c].len() as i32)
                .max()
                .unwrap_or(0)
                .max(1)
        })
        .collect();
    let rows_deep: Vec<i32> = (0..rows)
        .map(|r| {
            (0..cols)
                .map(|c| chains[r * cols + c].len() as i32)
                .max()
                .unwrap_or(0)
                .max(1)
        })
        .collect();
    // a gutter between two groups holds a padding for each frame it crosses (at least one on
    // each side) and the channel; one inside a group, the group's gap; above a row of other
    // groups, a title for each frame opening there
    // a pile of frames (`zones`, D-114): a step more after its last column and its last row
    let (mut pile_x, mut pile_y) = (vec![0; cols], vec![0; rows]);
    for (p, c) in frame_cells(inp, tiling).into_iter().enumerate() {
        if let Some((_, _, x1, y1)) = c.filter(|_| inp.panels[p].pile_step() > 0) {
            pile_x[x1] = 1;
            pile_y[y1] = 1;
        }
    }
    let mut xout = Vec::new();
    let mut xgap = Vec::new();
    for k in 1..cols {
        let out = (0..rows).any(|r| apart(r * cols + k - 1, r * cols + k));
        let g = if out {
            PAD * (cols_deep[k - 1] + cols_deep[k])
                + sum(&bands.left, k, 0, cols_deep[k])
                + gaps.chan_x
        } else {
            gaps.gin_x
        };
        xout.push(out);
        xgap.push(g + ex(&extra.x, k - 1) + pile_x[k - 1]);
    }
    let mut yout = Vec::new();
    let mut ygap = Vec::new();
    for k in 1..rows {
        let out = (0..cols).any(|c| apart((k - 1) * cols + c, k * cols + c));
        let g = if out {
            PAD * rows_deep[k - 1] + sum(&bands.top, k, 0, rows_deep[k]) + gaps.chan_y
        } else {
            gaps.gin_y
        };
        yout.push(out);
        ygap.push(g + ex(&extra.y, k - 1) + ex(&extra.head, k) + pile_y[k - 1]);
    }
    // round the drawing: a padding (a title above) for each frame at its edge
    let (left, right) = (cols_deep[0], cols_deep[cols - 1]);
    let (top, bottom) = (rows_deep[0], rows_deep[rows - 1]);
    Gutters {
        xgap,
        xout,
        ygap,
        yout,
        edges: (left, top, right, bottom),
        deep: (cols_deep, rows_deep),
        piles: (pile_x[cols - 1], pile_y[rows - 1]),
    }
}

/// The cells panel `j`'s cards stand in, as one block (none when it holds no card).
fn fitted(inp: &Input, tiling: &Tiling, j: usize) -> Option<(usize, usize, usize, usize)> {
    let cells: Vec<(usize, usize)> = inp.panels[j].cards.iter().map(|&c| tiling.pos[c]).collect();
    let x0 = cells.iter().map(|c| c.0).min()?;
    let y0 = cells.iter().map(|c| c.1).min()?;
    let x1 = cells.iter().map(|c| c.0).max()?;
    let y1 = cells.iter().map(|c| c.1).max()?;
    Some((x0, y0, x1 - x0 + 1, y1 - y0 + 1))
}
