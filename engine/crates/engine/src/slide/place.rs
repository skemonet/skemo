//! The placement, bounded: which cell of the slide's grid holds which card.
//!
//! The groups are tiled like a slide layout (a tree of guillotine cuts over a grid of cells: the
//! outline is one rectangle, groups side by side share top and bottom, S3), the slide's shape near
//! 16:9 with few empty cells (S4), linked groups neighbours (S5), one reading direction (S6). Up
//! to five groups every split of every subset is tried; with more, only the splits that keep the
//! groups in their writing order (D-086). The cheapest tilings get their cards placed (a local
//! search over the cells) and each link routed alone on the lattice; the cheapest of those is kept.

use super::grid::{Arrow as Search, Route};
use super::input::{Align, Band};
use super::lattice::{Extra, Gaps, Lattice, Tiling};
use super::pairs::count_crossings;
use super::{Input, Walls};
use std::collections::HashMap;
use std::rc::Rc;

const FULL_UPTO: usize = 5;
const ROUTE_TOP: usize = 6;
const ROUTE_LINKS: usize = 90;
const KEEP_FULL: usize = 30;
const KEEP_SUB5: usize = 16;
const KEEP_ORDERED: usize = 12;
/// The tilings of a zone inside another kept for the one around it.
const KEEP_INNER: usize = 12;
const TILE_W: f64 = 10.0;
const W_CROSSING: f64 = 300.0;
const NO_ROUTE: f64 = 2000.0;
const W_SHAPE: f64 = 800.0;
/// Two sibling groups read in the other order than written (P3, D-086): more than two arrows
/// climbing back, which is what breaking the order would save.
const W_ZONE_ORDER: f64 = 20.0;
/// A note on the other side of its card, or away from it (D-104): it reads as its card's, a
/// preference below the lines' rules.
/// A link between groups set aside across a written `dir`: the writer's direction outweighs
/// the slide's shape (S4).
const W_ASIDE: f64 = 100.0;
/// A column's members a cell apart across `dir` (D-051).
const W_COL: f64 = 200.0;
/// An arrow set aside across a written `dir`.
const SIDEWAYS: f64 = 15.0;
const NOTE_SECOND: f64 = 2.0;
const NOTE_AWAY: f64 = 6.0;
/// Two cards of a group read in the other order than written (P5): less than a crossing (R4).
const W_CARD_ORDER: f64 = 6.0;
pub(crate) const ASPECT: f64 = 16.0 / 9.0;
const GAPS: Gaps = Gaps {
    gin_x: 6,
    gin_y: 6,
    chan_x: 6,
    chan_y: 6,
};

/// A tree with its score and the order it was made in (ties broken by that order).
type Scored = (f64, usize, Rc<Tree>);

#[derive(Debug)]
pub(crate) enum Tree {
    Leaf {
        p: usize,
        w: usize,
        h: usize,
    },
    Cut {
        across: bool,
        a: Rc<Tree>,
        b: Rc<Tree>,
        w: usize,
        h: usize,
    },
}

impl Tree {
    fn w(&self) -> usize {
        match self {
            Tree::Leaf { w, .. } | Tree::Cut { w, .. } => *w,
        }
    }
    fn h(&self) -> usize {
        match self {
            Tree::Leaf { h, .. } | Tree::Cut { h, .. } => *h,
        }
    }
    fn panels(&self, out: &mut Vec<usize>) {
        match self {
            Tree::Leaf { p, .. } => out.push(*p),
            Tree::Cut { a, b, .. } => {
                a.panels(out);
                b.panels(out);
            }
        }
    }
    fn cut(across: bool, a: Rc<Tree>, b: Rc<Tree>) -> Tree {
        let (w, h) = if across {
            (a.w() + b.w(), a.h().max(b.h()))
        } else {
            (a.w().max(b.w()), a.h() + b.h())
        };
        Tree::Cut { across, a, b, w, h }
    }
}

/// The tree stretched to `w` x `h` cells: the extra goes to the last child of each cut.
fn rects(t: &Tree, n: usize) -> Vec<(usize, usize, usize, usize)> {
    fn go(
        t: &Tree,
        x: usize,
        y: usize,
        w: usize,
        h: usize,
        out: &mut Vec<(usize, usize, usize, usize)>,
    ) {
        match t {
            Tree::Leaf { p, .. } => out[*p] = (x, y, w, h),
            Tree::Cut {
                across: true, a, b, ..
            } => {
                go(a, x, y, a.w(), h, out);
                go(b, x + a.w(), y, w - a.w(), h, out);
            }
            Tree::Cut { a, b, .. } => {
                go(a, x, y, w, a.h(), out);
                go(b, x, y + a.h(), w, h - a.h(), out);
            }
        }
    }
    let mut out = vec![(0, 0, 0, 0); n];
    go(t, 0, 0, t.w(), t.h(), &mut out);
    out
}

fn panel_links(inp: &Input) -> Vec<((usize, usize), usize)> {
    let mut m: HashMap<(usize, usize), usize> = HashMap::new();
    for e in inp.edges.iter().filter(|e| e.constraint) {
        let (a, b) = (inp.card_panel[e.from], inp.card_panel[e.to]);
        if a != b {
            *m.entry((a, b)).or_default() += 1;
        }
    }
    let mut v: Vec<_> = m.into_iter().collect();
    v.sort_unstable();
    v
}

/// Group `b` lies against the flow from group `a` (`dir`: down; along x with `across`;
/// backwards with `reverse`): back along the flow, or behind across it while not past `a`.
fn against(
    (ax, ay, aw, ah): (usize, usize, usize, usize),
    (bx, by, bw, bh): (usize, usize, usize, usize),
    (across, reverse): (bool, bool),
) -> bool {
    // along the flow (start, length), then across it
    let ((a0, al), (c0, _), (b0, bl), (d0, dl)) = if across {
        ((ax, aw), (ay, ah), (bx, bw), (by, bh))
    } else {
        ((ay, ah), (ax, aw), (by, bh), (bx, bw))
    };
    let behind = d0 + dl <= c0;
    if reverse {
        b0 >= a0 + al || (behind && b0 + bl > a0)
    } else {
        b0 + bl <= a0 || (behind && b0 < a0 + al)
    }
}

/// Group `b` stands beside group `a` across the flow, not after it along it (`across`: the flow
/// along x).
fn aside(
    (ax, ay, aw, ah): (usize, usize, usize, usize),
    (bx, by, bw, bh): (usize, usize, usize, usize),
    across: bool,
) -> bool {
    let ((a0, al), (b0, bl)) = if across {
        ((ax, aw), (bx, bw))
    } else {
        ((ay, ah), (by, bh))
    };
    b0 < a0 + al && a0 < b0 + bl
}

/// S5 far apart, S7 groups in the way, S6 against the flow (a written `dir`: beside it too);
/// `in_tree`: the panels laid out.
fn link_cost(
    r: &[(usize, usize, usize, usize)],
    links: &[((usize, usize), usize)],
    in_tree: &[bool],
    dir: (bool, bool, bool),
) -> f64 {
    let mut cost = 0.0;
    for &((a, b), k) in links {
        if !in_tree[a] || !in_tree[b] {
            continue;
        }
        let k = k as f64;
        let f = |v: usize| v as f64;
        let (ax, ay, aw, ah) = r[a];
        let (bx, by, bw, bh) = r[b];
        let gx = (f(bx) - f(ax + aw)).max(f(ax) - f(bx + bw)).max(0.0);
        let gy = (f(by) - f(ay + ah)).max(f(ay) - f(by + bh)).max(0.0);
        cost += k * (gx + gy) * 4.0;
        let (acx, acy) = (f(ax) + f(aw) / 2.0, f(ay) + f(ah) / 2.0);
        let (bcx, bcy) = (f(bx) + f(bw) / 2.0, f(by) + f(bh) / 2.0);
        let mut hit = vec![false; r.len()];
        for i in 1..12 {
            let t = f64::from(i) / 12.0;
            let (px, py) = (acx + (bcx - acx) * t, acy + (bcy - acy) * t);
            for (p, &(x, y, w, h)) in r.iter().enumerate() {
                if in_tree[p]
                    && p != a
                    && p != b
                    && f(x) < px
                    && px < f(x + w)
                    && f(y) < py
                    && py < f(y + h)
                {
                    hit[p] = true;
                }
            }
        }
        cost += k * 6.0 * hit.iter().filter(|&&h| h).count() as f64;
        if against(r[a], r[b], (dir.0, dir.1)) {
            // back against a written `dir` weighs more than beside it
            cost += k * if dir.2 { 2.0 * W_ASIDE } else { 8.0 };
        } else if dir.2 && aside(r[a], r[b], dir.0) {
            cost += k * W_ASIDE;
        }
    }
    cost + W_ZONE_ORDER * inversions(r, in_tree) as f64
}

/// Pairs of groups read in the other order than written (P3, D-086): reading goes by rows, a
/// group before another when its top left cell comes first.
fn inversions(r: &[(usize, usize, usize, usize)], in_tree: &[bool]) -> usize {
    let at: Vec<(usize, (usize, usize))> = (0..r.len())
        .filter(|&p| in_tree[p] && r[p].2 > 0)
        .map(|p| (p, (r[p].1, r[p].0)))
        .collect();
    let mut n = 0;
    for (i, &(_, a)) in at.iter().enumerate() {
        n += at[i + 1..].iter().filter(|&&(_, b)| b < a).count();
    }
    n
}

struct Tiler<'a> {
    inp: &'a Input,
    links: Vec<((usize, usize), usize)>,
    n: usize,
}

impl Tiler<'_> {
    fn scored(&self, t: &Tree) -> f64 {
        let mut ps = Vec::new();
        t.panels(&mut ps);
        let mut in_tree = vec![false; self.n];
        for &p in &ps {
            in_tree[p] = true;
        }
        let cards: usize = ps.iter().map(|&p| self.inp.panels[p].cards.len()).sum();
        let waste = (t.w() * t.h() - cards) as f64;
        waste * 3.0
            + link_cost(
                &rects(t, self.n),
                &self.links,
                &in_tree,
                (
                    self.inp.dir.across,
                    self.inp.dir.reverse,
                    self.inp.dir.written,
                ),
            )
    }

    fn leaves(&self, p: usize) -> Vec<Rc<Tree>> {
        let k = self.inp.panels[p].cards.len();
        // a wide zone: its cards on one row (one column across `dir`)
        if self.inp.panels[p].wide && self.inp.panels[p].zone.is_some() {
            let (w, h) = if self.inp.dir.across { (1, k) } else { (k, 1) };
            return vec![Rc::new(Tree::Leaf { p, w, h })];
        }
        (1..=k)
            .filter(|&c| c * k.div_ceil(c) - k < c)
            .map(|c| {
                Rc::new(Tree::Leaf {
                    p,
                    w: c,
                    h: k.div_ceil(c),
                })
            })
            .collect()
    }

    /// The trees of every (A, B) pair cut both ways: three per shape, `keep` in all.
    fn best_of(&self, pairs: &[(Rc<Tree>, Rc<Tree>)], keep: usize, dirs: &[bool]) -> Vec<Rc<Tree>> {
        let mut by_shape: HashMap<(usize, usize), Vec<Scored>> = HashMap::new();
        let mut seq = 0;
        for (a, b) in pairs {
            for &across in dirs {
                let t = Tree::cut(across, a.clone(), b.clone());
                let s = self.scored(&t);
                by_shape
                    .entry((t.w(), t.h()))
                    .or_default()
                    .push((s, seq, Rc::new(t)));
                seq += 1;
            }
        }
        let mut out = Vec::new();
        let mut shapes: Vec<_> = by_shape.into_iter().collect();
        shapes.sort_by_key(|(k, _)| *k);
        for (_, mut lst) in shapes {
            lst.sort_by(|x, y| x.0.total_cmp(&y.0).then(x.1.cmp(&y.1)));
            out.extend(lst.into_iter().take(3));
        }
        out.sort_by(|x, y| x.0.total_cmp(&y.0).then(x.1.cmp(&y.1)));
        out.into_iter().take(keep).map(|t| t.2).collect()
    }

    /// The best trees of the items of `mask`, every split tried (each item: its alternatives).
    fn full(
        &self,
        items: &[Vec<Rc<Tree>>],
        mask: usize,
        memo: &mut HashMap<usize, Vec<Rc<Tree>>>,
    ) -> Vec<Rc<Tree>> {
        if let Some(v) = memo.get(&mask) {
            return v.clone();
        }
        let m = items.len();
        let idx: Vec<usize> = (0..m).filter(|i| mask >> i & 1 == 1).collect();
        let out = if idx.len() == 1 {
            items[idx[0]].clone()
        } else {
            let mut pairs = Vec::new();
            let mut sub = (mask - 1) & mask;
            while sub > 0 {
                let (a, b) = (
                    self.full(items, sub, memo),
                    self.full(items, mask ^ sub, memo),
                );
                for x in &a {
                    for y in &b {
                        pairs.push((x.clone(), y.clone()));
                    }
                }
                sub = (sub - 1) & mask;
            }
            let all = (1 << m) - 1;
            self.best_of(
                &pairs,
                if m <= 4 || mask == all {
                    KEEP_FULL
                } else {
                    KEEP_SUB5
                },
                &[true, false],
            )
        };
        memo.insert(mask, out.clone());
        out
    }

    /// The best trees of the items `lo..hi` in their writing order, split in two at each place.
    fn span(
        &self,
        items: &[Vec<Rc<Tree>>],
        lo: usize,
        hi: usize,
        memo: &mut HashMap<(usize, usize), Vec<Rc<Tree>>>,
    ) -> Vec<Rc<Tree>> {
        if let Some(v) = memo.get(&(lo, hi)) {
            return v.clone();
        }
        let out = if hi - lo == 1 {
            items[lo].clone()
        } else {
            let mut pairs = Vec::new();
            for mid in lo + 1..hi {
                let (a, b) = (
                    self.span(items, lo, mid, memo),
                    self.span(items, mid, hi, memo),
                );
                for x in &a {
                    for y in &b {
                        pairs.push((x.clone(), y.clone()));
                    }
                }
            }
            self.best_of(&pairs, KEEP_ORDERED, &[true, false])
        };
        memo.insert((lo, hi), out.clone());
        out
    }

    /// The tilings of what panel `p` holds (`None`: the whole slide): its own cards, a block, and
    /// each zone inside it, tiled the same way first; up to five items every split is tried, past
    /// that only the splits that keep their writing order (D-086). A stack lays its children one
    /// after the other along `dir`; a wide card or zone is a row of its own (a column across
    /// `dir`), where it is written.
    fn tile(&self, p: Option<usize>) -> Vec<Rc<Tree>> {
        self.tile_kids(p, &self.inp.children(p))
    }

    /// The children of a group docked on its sides (`at=`, D-117) round the rest, as Java's
    /// `BorderLayout`: the north and south bands span the group, the west and east ones the
    /// middle; several on one side in writing order.
    fn border(&self, p: Option<usize>, kids: &[usize]) -> Vec<Rc<Tree>> {
        let inp = self.inp;
        let tiled = |c: usize| {
            let mut alts = self.tile(Some(c));
            alts.truncate(KEEP_INNER);
            alts
        };
        let on = |b: Band| -> Vec<Vec<Rc<Tree>>> {
            kids.iter()
                .filter(|&&c| inp.panels[c].dock == Some(b))
                .map(|&c| tiled(c))
                .collect()
        };
        let rest: Vec<usize> = kids
            .iter()
            .copied()
            .filter(|&c| inp.panels[c].dock.is_none())
            .collect();
        let has_own = p.is_some_and(|p| !inp.panels[p].cards.is_empty());
        let mut middle = on(Band::W);
        if has_own || !rest.is_empty() {
            middle.push(self.tile_kids(p, &rest));
        }
        middle.extend(on(Band::E));
        let mut rows = on(Band::N);
        if !middle.is_empty() {
            rows.push(self.sequence(middle, true));
        }
        rows.extend(on(Band::S));
        self.sequence(rows, false)
    }

    /// The children `kids` of group `p` (and its own cards) tiled.
    fn tile_kids(&self, p: Option<usize>, kids: &[usize]) -> Vec<Rc<Tree>> {
        let inp = self.inp;
        if kids.iter().any(|&c| inp.panels[c].dock.is_some()) {
            return self.border(p, kids);
        }
        let tiled = |c: usize| {
            let mut alts = self.tile(Some(c));
            alts.truncate(KEEP_INNER);
            alts
        };
        // a stack, or a group with `dir=` (D-117): its own cards on one line, then its children,
        // in writing order, one after the other that way
        let along = p.and_then(|p| {
            inp.panels[p].dir.or(inp.panels[p]
                .stack
                .then_some((inp.dir.across, inp.dir.reverse)))
        });
        if let (Some(p), Some((across, reverse))) = (p, along) {
            let mut items: Vec<(usize, Vec<Rc<Tree>>)> = kids
                .iter()
                .map(|&c| (inp.panels[c].order, tiled(c)))
                .collect();
            let k = inp.panels[p].cards.len();
            if k > 0 {
                let (w, h) = if across { (k, 1) } else { (1, k) };
                items.push((inp.panels[p].first, vec![Rc::new(Tree::Leaf { p, w, h })]));
            }
            items.sort_by_key(|i| i.0);
            let mut items: Vec<Vec<Rc<Tree>>> = items.into_iter().map(|i| i.1).collect();
            if reverse {
                items.reverse();
            }
            return self.sequence(items, across);
        }
        // `wrap="N"`: the children in writing order, N a row (a column across `dir`)
        if let Some(p) = p
            && let Some(n) = inp.panels[p].wrap
        {
            let items: Vec<Vec<Rc<Tree>>> = kids.iter().map(|&c| tiled(c)).collect();
            let lines: Vec<Vec<Rc<Tree>>> = items
                .chunks(n)
                .map(|row| self.sequence(row.to_vec(), !inp.dir.across))
                .collect();
            return self.sequence(lines, inp.dir.across);
        }
        let (wide, free): (Vec<usize>, Vec<usize>) =
            kids.iter().partition(|&&c| inp.panels[c].wide);
        let own = p.filter(|&p| !inp.panels[p].cards.is_empty());
        let mut items: Vec<Vec<Rc<Tree>>> = Vec::new();
        if let Some(p) = own {
            items.push(self.leaves(p));
        }
        for &c in &free {
            items.push(tiled(c));
        }
        let block = match items.len() {
            // an empty zone: one cell, its frame and title shown (FORMAT: a default frame)
            0 => p.map_or_else(Vec::new, |p| vec![Rc::new(Tree::Leaf { p, w: 1, h: 1 })]),
            1 => items.pop().unwrap_or_default(),
            n if n <= FULL_UPTO => self.full(&items, (1 << n) - 1, &mut HashMap::new()),
            n => self.span(&items, 0, n, &mut HashMap::new()),
        };
        if wide.is_empty() {
            return block;
        }
        // the rows in writing order: the block where its first card or zone is written
        let block_at = own
            .map(|p| inp.panels[p].first)
            .into_iter()
            .chain(free.iter().map(|&c| inp.panels[c].order))
            .min();
        let mut rows: Vec<(usize, Vec<Rc<Tree>>)> = wide
            .iter()
            .map(|&c| (inp.panels[c].order, tiled(c)))
            .collect();
        if let Some(at) = block_at {
            rows.push((at, block));
        }
        rows.sort_by_key(|r| r.0);
        self.sequence(rows.into_iter().map(|r| r.1).collect(), inp.dir.across)
    }

    /// The items one after the other, side by side when `across`, else one under the other.
    fn sequence(&self, items: Vec<Vec<Rc<Tree>>>, across: bool) -> Vec<Rc<Tree>> {
        let mut it = items.into_iter();
        let Some(mut cur) = it.next() else {
            return Vec::new();
        };
        for next in it {
            let pairs: Vec<(Rc<Tree>, Rc<Tree>)> = cur
                .iter()
                .flat_map(|a| next.iter().map(move |b| (a.clone(), b.clone())))
                .collect();
            cur = self.best_of(&pairs, KEEP_FULL, &[across]);
        }
        cur
    }
}

/// The tilings, the best first.
fn tilings(inp: &Input) -> Vec<(f64, Rc<Tree>)> {
    let n = inp.panels.len();
    let t = Tiler {
        inp,
        links: panel_links(inp),
        n,
    };
    let top = t.tile(None);
    let mut out: Vec<(f64, Rc<Tree>)> = top
        .into_iter()
        .map(|tr| {
            let (w, h) = px_size(inp, tr.w(), tr.h());
            (t.scored(&tr) + ((w / h) / ASPECT).ln().abs() * 25.0, tr)
        })
        .collect();
    out.sort_by(|a, b| a.0.total_cmp(&b.0));
    out
}

fn px_size(inp: &Input, w: usize, h: usize) -> (f64, f64) {
    let (w, h) = (w as f64, h as f64);
    let cw = f64::from(inp.card_w) * 10.0;
    let ch = f64::from(inp.card_h) * 10.0;
    (
        w * cw + (w - 1.0) * 78.0 + 96.0,
        h * ch + (h - 1.0) * 96.0 + 56.0 + 96.0 + 80.0,
    )
}

/// The cards inside their group: a local search over the cells (S5 linked cards face each
/// other, S6 an arrow never climbs back nor runs right to left, holes at the end).
type Cells = HashMap<(usize, usize), usize>;

/// Each panel's cards in its cells: in order first, then swapped (with each other or with an
/// empty cell) while the links get shorter, straighter, uncrossed and with the flow.
fn place_cards(inp: &Input, r: &[(usize, usize, usize, usize)]) -> Vec<(usize, usize)> {
    let mut pos = vec![(0, 0); inp.cards.len()];
    let mut cells_of = Vec::new();
    let mut occ: Cells = HashMap::new();
    for (i, p) in inp.panels.iter().enumerate() {
        let (x, y, w, h) = r[i];
        let cells: Vec<(usize, usize)> = (0..h)
            .flat_map(|cy| (0..w).map(move |cx| (x + cx, y + cy)))
            .collect();
        for (&c, &cell) in p.cards.iter().zip(&cells) {
            pos[c] = cell;
            occ.insert(cell, c);
        }
        cells_of.push(cells);
    }
    let total = |pos: &[(usize, usize)], occ: &Cells| -> f64 {
        let s = links_cost(inp, pos, occ)
            + crossings_cost(inp, pos)
            + order_cost(inp, pos)
            + notes_cost(inp, pos)
            + cols_cost(inp, pos);
        holes_cost(s, r, &cells_of, occ)
    };
    let mut cur = total(&pos, &occ);
    for _ in 0..30 {
        let mut improved = false;
        for (i, p) in inp.panels.iter().enumerate() {
            for &a in &p.cards {
                for &cell in &cells_of[i] {
                    let pa = pos[a];
                    if cell == pa {
                        continue;
                    }
                    let b = occ.get(&cell).copied();
                    swap(&mut pos, &mut occ, a, pa, cell, b);
                    let new = total(&pos, &occ);
                    if new < cur - 1e-9 {
                        cur = new;
                        improved = true;
                    } else {
                        swap(&mut pos, &mut occ, a, cell, pa, b);
                    }
                }
            }
        }
        if !improved {
            break;
        }
    }
    spread(inp, r, &mut pos);
    pos
}

/// A stack's layers (D-050) and a wide zone's row (D-051, D-092): their cards share the width
/// evenly, each in the middle of its share, in their order; a card alone in its layer stands
/// over (under) the card it links to when that one is within its width, else in the middle.
fn spread(inp: &Input, r: &[(usize, usize, usize, usize)], pos: &mut [(usize, usize)]) {
    for (i, p) in inp.panels.iter().enumerate() {
        let parent = p.parent.map(|q| &inp.panels[q]);
        // a layer of a stack or of a group with `dir=` (D-117), a band docked in its group, a
        // wide zone's row: its cards across the layer (`across`: a column)
        let across = if let Some(b) = p.dock {
            matches!(b, Band::E | Band::W)
        } else if let Some(q) = parent.filter(|q| q.stack || q.dir.is_some()) {
            !q.dir.map_or(inp.dir.across, |d| d.0)
        } else if p.wide && p.zone.is_some() {
            inp.dir.across
        } else {
            continue;
        };
        if p.cards.is_empty() {
            continue;
        }
        let align = parent.and_then(|q| q.align);
        let (x, y, w, h) = r[i];
        let (len, line) = if across { (h, w) } else { (w, h) };
        let k = p.cards.len();
        if line != 1 || len <= k {
            continue;
        }
        let at = |j: usize| if across { (x, y + j) } else { (x + j, y) };
        let along = |c: (usize, usize)| if across { c.1 } else { c.0 };
        // `align=` start or end: packed at that end, in their order
        if let Some(a @ (Align::Start | Align::End)) = align {
            let mut cards = p.cards.clone();
            cards.sort_by_key(|&c| along(pos[c]));
            let from = if a == Align::Start { 0 } else { len - k };
            for (n, &c) in cards.iter().enumerate() {
                pos[c] = at(from + n);
            }
            continue;
        }
        if k == 1 {
            let c = p.cards[0];
            let linked = inp.edges.iter().filter_map(|e| {
                let other = if e.from == c {
                    e.to
                } else if e.to == c {
                    e.from
                } else {
                    return None;
                };
                (inp.card_panel[other] != i).then(|| along(pos[other]))
            });
            let start = along(at(0));
            let over = linked
                .filter(|&a| a >= start && a < start + len)
                .min_by_key(|&a| a.abs_diff(start + len / 2));
            pos[c] = at(over.map_or((len - 1) / 2, |a| a - start));
            continue;
        }
        let mut cards = p.cards.clone();
        cards.sort_by_key(|&c| along(pos[c]));
        for (n, &c) in cards.iter().enumerate() {
            pos[c] = at((2 * n + 1) * len / (2 * k));
        }
    }
}

/// Card `a` moves from cell `from` to cell `to`; card `b`, if any, the other way.
fn swap(
    pos: &mut [(usize, usize)],
    occ: &mut Cells,
    a: usize,
    from: (usize, usize),
    to: (usize, usize),
    b: Option<usize>,
) {
    pos[a] = to;
    occ.insert(to, a);
    match b {
        Some(b) => {
            pos[b] = from;
            occ.insert(from, b);
        }
        None => {
            occ.remove(&from);
        }
    }
}

fn cell(pos: &[(usize, usize)], c: usize) -> (i64, i64) {
    (pos[c].0 as i64, pos[c].1 as i64)
}

/// A move of `(dx, dy)` along the flow (`dir`: down by default) and across it.
fn flow(inp: &Input, dx: i64, dy: i64) -> (i64, i64) {
    let sign = if inp.dir.reverse { -1 } else { 1 };
    if inp.dir.across {
        (sign * dx, dy)
    } else {
        (sign * dy, dx)
    }
}

/// Each note (D-104) beside its card: after it across the flow (its right with `dir` down or
/// up), else before it (above with right or left); a note elsewhere pays by its distance.
fn notes_cost(inp: &Input, pos: &[(usize, usize)]) -> f64 {
    let mut s = 0.0;
    if crate::trace::off("slide-notes") {
        return s;
    }
    for &(n, t) in &inp.notes {
        let (dx, dy) = (
            pos[n].0 as i64 - pos[t].0 as i64,
            pos[n].1 as i64 - pos[t].1 as i64,
        );
        // with right or left the first place is above the card, else on its right
        let (first, second) = if inp.dir.across {
            ((0, -1), (0, 1))
        } else {
            ((1, 0), (-1, 0))
        };
        s += if (dx, dy) == first {
            0.0
        } else if (dx, dy) == second {
            NOTE_SECOND
        } else {
            NOTE_AWAY
        };
    }
    s
}

/// Columns (`.col-<name>`, D-051): the members of one column share an axis across `dir`; each
/// column pays the spread of its members' axes, in cells.
fn cols_cost(inp: &Input, pos: &[(usize, usize)]) -> f64 {
    let across = |c: usize| {
        if inp.dir.across {
            pos[c].1 as f64
        } else {
            pos[c].0 as f64
        }
    };
    let mut s = 0.0;
    for col in &inp.cols {
        let axes: Vec<f64> = col
            .iter()
            .map(|m| {
                let lo = m.iter().map(|&c| across(c)).fold(f64::INFINITY, f64::min);
                let hi = m
                    .iter()
                    .map(|&c| across(c))
                    .fold(f64::NEG_INFINITY, f64::max);
                (lo + hi) / 2.0
            })
            .collect();
        let lo = axes.iter().copied().fold(f64::INFINITY, f64::min);
        let hi = axes.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        s += W_COL * (hi - lo);
    }
    s
}

/// Each link: its length, a bend, the cards in its way, against the flow.
fn links_cost(inp: &Input, pos: &[(usize, usize)], occ: &Cells) -> f64 {
    let mut s = 0.0;
    for e in inp.edges.iter().filter(|e| e.constraint) {
        let (a, b) = (cell(pos, e.from), cell(pos, e.to));
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let mut c = ((dx.abs() + dy.abs()) * 10) as f64;
        if dx != 0 && dy != 0 {
            c += 7.0;
        }
        if dx == 0 && dy.abs() > 1 {
            let between = (a.1.min(b.1) + 1..a.1.max(b.1))
                .filter(|&yy| occ.contains_key(&(a.0 as usize, yy as usize)))
                .count();
            c += 14.0 * between as f64;
        }
        if dy == 0 && dx.abs() > 1 {
            let between = (a.0.min(b.0) + 1..a.0.max(b.0))
                .filter(|&xx| occ.contains_key(&(xx as usize, a.1 as usize)))
                .count();
            c += 14.0 * between as f64;
        }
        // against the flow (`dir`): back along it, or across it backwards
        let (along, across) = flow(inp, dx, dy);
        if along < 0 {
            c += 30.0;
        } else if along == 0 && across < 0 {
            c += 25.0;
        } else if along == 0 && inp.dir.written {
            // `dir` written: the flow goes that way, an arrow set aside across it pays
            c += SIDEWAYS;
        }
        s += c;
    }
    s
}

/// Two cards of a group read in the other order than written (P5): by rows, cards numbered in
/// writing order.
fn order_cost(inp: &Input, pos: &[(usize, usize)]) -> f64 {
    let mut n = 0;
    // a note stands by its card, not in the writing order (D-104)
    let note = |c: usize| inp.notes.iter().any(|&(nt, _)| nt == c);
    for p in &inp.panels {
        let cards: Vec<usize> = p.cards.iter().copied().filter(|&c| !note(c)).collect();
        for (i, &a) in cards.iter().enumerate() {
            let ra = (pos[a].1, pos[a].0);
            n += cards[i + 1..]
                .iter()
                .filter(|&&b| (pos[b].1, pos[b].0) < ra)
                .count();
        }
    }
    W_CARD_ORDER * n as f64
}

/// Two links between four different cards crossing as straight lines.
fn crossings_cost(inp: &Input, pos: &[(usize, usize)]) -> f64 {
    let ccw = |p: (i64, i64), q: (i64, i64), r: (i64, i64)| {
        (r.1 - p.1) * (q.0 - p.0) - (q.1 - p.1) * (r.0 - p.0)
    };
    let es: Vec<&super::input::Edge> = inp.edges.iter().filter(|e| e.constraint).collect();
    let mut s = 0.0;
    for i in 0..es.len() {
        for j in i + 1..es.len() {
            let p = |c| cell(pos, c);
            let (a, b, c, d) = (p(es[i].from), p(es[i].to), p(es[j].from), p(es[j].to));
            let distinct = a != b && a != c && a != d && b != c && b != d && c != d;
            if distinct && ccw(a, b, c) * ccw(a, b, d) < 0 && ccw(c, d, a) * ccw(c, d, b) < 0 {
                s += 12.0;
            }
        }
    }
    s
}

/// Empty cells, added to `s`: at the end of their panel (bottom right) costs least.
fn holes_cost(
    mut s: f64,
    r: &[(usize, usize, usize, usize)],
    cells_of: &[Vec<(usize, usize)>],
    occ: &Cells,
) -> f64 {
    for (i, cells) in cells_of.iter().enumerate() {
        let (x, y, w, h) = r[i];
        for &(cx, cy) in cells {
            if !occ.contains_key(&(cx, cy)) {
                s += 0.3 * (((y + h - 1 - cy) as f64) + ((x + w - 1 - cx) as f64) * 0.5);
            }
        }
    }
    s
}

/// What the links cost on this tiling, each taking its cheapest route alone, plus the
/// crossings between those routes; and the drawing's shape.
fn route_cost(inp: &Input, tiling: &Tiling, walls: &Walls) -> f64 {
    let l = Lattice::new(inp, tiling, GAPS, &Extra::default(), walls);
    let g = super::grid::Grid::new(inp, &l);
    let mut best: Vec<Route> = Vec::new();
    let mut cost = 0.0;
    for e in inp.edges.iter().filter(|e| e.constraint) {
        let s = Search::of(&g, inp, &l, e);
        match s.candidates(1, 1, 1, &|_| 0.0).into_iter().next() {
            Some(r) => {
                cost += r.cost;
                best.push(r);
            }
            None => cost += NO_ROUTE,
        }
    }
    let refs: Vec<&Route> = best.iter().collect();
    let cr = count_crossings(&refs) as f64;
    let shape = f64::from(l.width) / f64::from(l.height);
    cost + W_CROSSING * cr + W_SHAPE * (shape / ASPECT).ln().abs()
}

/// The tiling and the cell of each card.
pub(crate) fn placement(inp: &Input, walls: &Walls) -> Tiling {
    let links = panel_links(inp);
    let n = inp.panels.len();
    let n_route = (ROUTE_LINKS / inp.edges.len().max(1)).clamp(2, ROUTE_TOP);
    let mut best: Option<(f64, Tiling)> = None;
    let mut seen = Vec::new();
    let mut tried = Vec::new();
    for (_, t) in tilings(inp) {
        let r = rects(&t, n);
        if seen.contains(&r) {
            continue;
        }
        seen.push(r.clone());
        if seen.len() > n_route {
            break;
        }
        let pos = place_cards(inp, &r);
        let tiling = Tiling {
            cols: t.w(),
            rows: t.h(),
            rects: r,
            pos,
        };
        tried.push(tiling);
    }
    // each tiling's routes, several at once: the costliest part of the placement
    let costs = super::par_map(&tried, &|t: &Tiling| route_cost(inp, t, walls));
    for (tiling, rc) in tried.into_iter().zip(costs) {
        let r = &tiling.rects;
        let own = (tiling.cols * tiling.rows - inp.cards.len()) as f64 * 3.0
            + link_cost(
                r,
                &links,
                &vec![true; n],
                (inp.dir.across, inp.dir.reverse, inp.dir.written),
            );
        // the columns line up across groups only as the tiling allows: it counts too
        let c = rc + TILE_W * (own + cols_cost(inp, &tiling.pos));
        if best.as_ref().is_none_or(|b| c < b.0 - 1e-9) {
            best = Some((c, tiling));
        }
    }
    best.map_or_else(
        || Tiling {
            cols: 1,
            rows: 1,
            rects: vec![(0, 0, 1, 1)],
            pos: vec![(0, 0)],
        },
        |b| b.1,
    )
}
