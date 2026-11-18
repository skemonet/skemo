//! Candidate routes for each arrow on the lattice, each keeping the rules a single line can keep
//! on its own (H1.4, H2.4, H2.8, H2.9, H2.23, H2.24); the rules between two lines are the
//! solver's (`pairs`, `solve`).
//!
//! A route leaves its card square from a port of its side (one step of stub, H2.9), runs on
//! lattice nodes outside every card (never on a card's border: `[box-margin]` is one step), never
//! along a zone frame (`[frame-margin]`), never through a zone title, never between two cards of a
//! group that holds neither of its ends (H1.4), and reaches its target square over at least two
//! steps (head and tail, H2.8).

use super::lattice::Lattice;
use super::{Input, Pt, Rect, STEP};
use std::collections::HashSet;

/// A search's distances and parents, per state.
type Searched = (Vec<f64>, Vec<u32>);

/// E S W N
pub(crate) const DIRS: [(i32, i32); 4] = [(1, 0), (0, 1), (-1, 0), (0, -1)];
/// A bend weighs 40 steps of length (bends before length, the constitution's order).
pub(crate) const BEND: f64 = 40.0;
/// Each step inside a group holding neither end (P18, discouraged).
const FOREIGN: f64 = 4.0;
/// Each step right beside a card or a frame (P20: a line runs in the middle of its gap).
const HUG: f64 = 2.0;
/// An end on the side turned away from the other card (a hook, H2.24 as amended on 2026-11-17,
/// A3): three bends' worth, so that room is made rather than a hook drawn (P11, M2).
pub(crate) const W_HOOK: f64 = 120.0;
/// Each step along a zone's title, in its band (A5, counted under P11): the cards move down.
pub(crate) const W_BAND: f64 = 3.0;
/// Each bend past the second (A8): a line of four bends is hard to follow.
const W_MANY: f64 = 20.0;
/// How much dearer than the cheapest arrival a search still explores (ten bends).
const SLACK: f64 = 400.0;
/// What makes another line's node dearer for a route that knows its neighbours.
pub(crate) const P_LINE: f64 = 300.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum Side {
    Top,
    Bottom,
    Left,
    Right,
}

pub(crate) const SIDES: [Side; 4] = [Side::Top, Side::Bottom, Side::Left, Side::Right];

impl Side {
    /// The direction leaving the card by this side.
    pub(crate) fn out(self) -> usize {
        match self {
            Side::Right => 0,
            Side::Bottom => 1,
            Side::Left => 2,
            Side::Top => 3,
        }
    }
}

/// What a node is on a route: an end, passing straight (level or upright), or a bend.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Role {
    End,
    H,
    V,
    Bend,
}

#[derive(Clone, Debug)]
pub(crate) struct Route {
    pub nodes: Vec<Pt>,
    pub pts: Vec<Pt>,
    pub sside: Side,
    pub tside: Side,
    pub cost: f64,
    pub bends: usize,
    /// (node key, index in `nodes`, role), sorted by key.
    pub keys: Vec<(u32, u16, Role)>,
    pub bbox: (i32, i32, i32, i32),
}

impl Route {
    pub(crate) fn new(nodes: Vec<Pt>, sside: Side, tside: Side, cost: f64) -> Route {
        let pts = corners(&nodes);
        let mut keys: Vec<(u32, u16, Role)> = nodes
            .iter()
            .enumerate()
            .map(|(i, &p)| (key(p), i as u16, role_at(&nodes, i)))
            .collect();
        keys.sort_unstable_by_key(|k| k.0);
        let xs = nodes.iter().map(|p| p.0);
        let ys = nodes.iter().map(|p| p.1);
        let bbox = (
            xs.clone().min().unwrap_or(0),
            ys.clone().min().unwrap_or(0),
            xs.max().unwrap_or(0),
            ys.max().unwrap_or(0),
        );
        Route {
            bends: pts.len().saturating_sub(2),
            pts,
            nodes,
            sside,
            tside,
            cost,
            keys,
            bbox,
        }
    }

    pub(crate) fn src(&self) -> Pt {
        self.nodes[0]
    }

    pub(crate) fn dst(&self) -> Pt {
        self.nodes[self.nodes.len() - 1]
    }

    /// The index and role of node `p` on this route.
    pub(crate) fn find(&self, p: Pt) -> Option<(usize, Role)> {
        let k = key(p);
        self.keys
            .binary_search_by_key(&k, |e| e.0)
            .ok()
            .map(|i| (self.keys[i].1 as usize, self.keys[i].2))
    }

    pub(crate) fn has(&self, p: Pt) -> bool {
        self.keys.binary_search_by_key(&key(p), |e| e.0).is_ok()
    }

    /// Every node of `seg` (sorted keys) is on this route.
    pub(crate) fn holds(&self, seg: &[u32]) -> bool {
        let mut i = 0;
        for &k in seg {
            while i < self.keys.len() && self.keys[i].0 < k {
                i += 1;
            }
            if i >= self.keys.len() || self.keys[i].0 != k {
                return false;
            }
        }
        true
    }
}

/// A node as one integer (x and y are below 2^15 on any slide).
pub(crate) fn key(p: Pt) -> u32 {
    ((p.1 as u32 & 0xffff) << 16) | (p.0 as u32 & 0xffff)
}

pub(crate) fn unkey(k: u32) -> Pt {
    ((k & 0xffff) as i32, (k >> 16) as i32)
}

fn role_at(n: &[Pt], i: usize) -> Role {
    if i == 0 || i + 1 == n.len() {
        return Role::End;
    }
    let (a, p, c) = (n[i - 1], n[i], n[i + 1]);
    if a.1 == p.1 && p.1 == c.1 {
        Role::H
    } else if a.0 == p.0 && p.0 == c.0 {
        Role::V
    } else {
        Role::Bend
    }
}

pub(crate) fn corners(nodes: &[Pt]) -> Vec<Pt> {
    let mut pts = vec![nodes[0]];
    for k in 1..nodes.len().saturating_sub(1) {
        let (a, b, c) = (nodes[k - 1], nodes[k], nodes[k + 1]);
        if (b.0 - a.0, b.1 - a.1) != (c.0 - b.0, c.1 - b.1) {
            pts.push(b);
        }
    }
    if nodes.len() > 1 {
        pts.push(nodes[nodes.len() - 1]);
    }
    pts
}

/// Every node between consecutive corners.
pub(crate) fn dense(pts: &[Pt]) -> Vec<Pt> {
    let mut out = vec![pts[0]];
    for &q in &pts[1..] {
        let mut p = *out.last().unwrap_or(&q);
        let dx = (q.0 - p.0).signum();
        let dy = (q.1 - p.1).signum();
        while p != q {
            p = (p.0 + dx, p.1 + dy);
            out.push(p);
        }
    }
    out
}

/// The lattice's nodes as one flat array per property, `w` by `h`.
struct Plane {
    w: i32,
    h: i32,
}

impl Plane {
    fn n(&self) -> usize {
        (self.w * self.h) as usize
    }

    fn idx(&self, x: i32, y: i32) -> usize {
        (y * self.w + x) as usize
    }

    fn set(&self, v: &mut [bool], x: i32, y: i32) {
        if (0..self.w).contains(&x) && (0..self.h).contains(&y) {
            v[self.idx(x, y)] = true;
        }
    }

    /// The cards and the zone texts (3 px round them); when every card sits in a frame, the
    /// outside of the frames' hull too (no loop round the groups, H2.24 read with M2: a line
    /// that cannot get through inside asks for room, P11).
    fn blocked(&self, inp: &Input, l: &Lattice, frames: &[Rect]) -> Vec<bool> {
        let (w, h) = (self.w, self.h);
        let mut block = vec![false; self.n()];
        for r in &l.card {
            for yy in r.y.max(0)..=(r.y + r.h).min(h - 1) {
                for xx in r.x.max(0)..=(r.x + r.w).min(w - 1) {
                    block[self.idx(xx, yy)] = true;
                }
            }
        }
        for b in l.texts.iter().flatten() {
            let y0 = ((b.y - 3.0) / STEP).floor() as i32;
            let y1 = ((b.y + b.h + 3.0) / STEP).ceil() as i32;
            let x0 = ((b.x - 3.0) / STEP).floor() as i32;
            let x1 = ((b.x + b.w + 3.0) / STEP).ceil() as i32;
            for yy in y0.max(0)..=y1.min(h - 1) {
                for xx in x0.max(0)..=x1.min(w - 1) {
                    block[self.idx(xx, yy)] = true;
                }
            }
        }
        if !frames.is_empty() && (0..inp.cards.len()).all(|c| l.zones_of(inp, c).is_some()) {
            let l0 = frames.iter().map(|f| f.x).min().unwrap_or(0);
            let t0 = frames.iter().map(|f| f.y).min().unwrap_or(0);
            let r0 = frames.iter().map(|f| f.x + f.w).max().unwrap_or(0);
            let b0 = frames.iter().map(|f| f.y + f.h).max().unwrap_or(0);
            for yy in 0..h {
                for xx in 0..w {
                    if xx < l0 || xx > r0 || yy < t0 || yy > b0 {
                        block[self.idx(xx, yy)] = true;
                    }
                }
            }
        }
        block
    }

    /// The frame lines, level and upright: no stretch runs along one.
    fn frame_lines(&self, frames: &[Rect]) -> (Vec<bool>, Vec<bool>) {
        let mut hf = vec![false; self.n()];
        let mut vf = vec![false; self.n()];
        for f in frames {
            for xx in f.x..f.x + f.w {
                self.set(&mut hf, xx, f.y);
                self.set(&mut hf, xx, f.y + f.h);
            }
            for yy in f.y..f.y + f.h {
                self.set(&mut vf, f.x, yy);
                self.set(&mut vf, f.x + f.w, yy);
            }
        }
        (hf, vf)
    }

    /// A zone's title band (A5): the frame's width, from its top to a step under its texts.
    fn title_bands(&self, l: &Lattice) -> Vec<bool> {
        let mut band = vec![false; self.n()];
        for (i, f) in l.frame.iter().enumerate() {
            let Some(f) = f else { continue };
            let Some(low) = l.texts[i].iter().map(|b| b.y + b.h).reduce(f64::max) else {
                continue;
            };
            let low = ((low + STEP) / STEP - 1e-9).ceil() as i32;
            for yy in (f.y + 1)..low {
                for xx in (f.x + 1).max(0)..(f.x + f.w).min(self.w) {
                    if (0..self.h).contains(&yy) {
                        band[self.idx(xx, yy)] = true;
                    }
                }
            }
        }
        band
    }

    /// The nodes a step from a card or beside a frame line: running there costs.
    fn hugging(&self, l: &Lattice, frames: &[Rect]) -> Vec<bool> {
        let (w, h) = (self.w, self.h);
        let mut hug = vec![false; self.n()];
        for r in &l.card {
            for yy in (r.y - 1).max(0)..=(r.y + r.h + 1).min(h - 1) {
                for xx in (r.x - 1).max(0)..=(r.x + r.w + 1).min(w - 1) {
                    hug[self.idx(xx, yy)] = true;
                }
            }
        }
        for f in frames {
            for xx in (f.x - 1).max(0)..=(f.x + f.w + 1).min(w - 1) {
                for yy in [f.y - 1, f.y + 1, f.y + f.h - 1, f.y + f.h + 1] {
                    if (0..h).contains(&yy) {
                        hug[self.idx(xx, yy)] = true;
                    }
                }
            }
            for yy in (f.y - 1).max(0)..=(f.y + f.h + 1).min(h - 1) {
                for xx in [f.x - 1, f.x + 1, f.x + f.w - 1, f.x + f.w + 1] {
                    if (0..w).contains(&xx) {
                        hug[self.idx(xx, yy)] = true;
                    }
                }
            }
        }
        hug
    }

    /// Per framed panel: the nodes between two of its cards and the others inside it.
    fn frame_masks(&self, inp: &Input, l: &Lattice) -> Vec<Option<(Vec<bool>, Vec<bool>)>> {
        let mut fmask = Vec::new();
        for (i, f) in l.frame.iter().enumerate() {
            let Some(f) = f else {
                fmask.push(None);
                continue;
            };
            let cards: Vec<Rect> = inp.panels[i].cards.iter().map(|&c| l.card[c]).collect();
            let mut bad = vec![false; self.n()];
            let mut pay = vec![false; self.n()];
            for yy in (f.y + 1)..(f.y + f.h) {
                for xx in (f.x + 1)..(f.x + f.w) {
                    let (mut le, mut ri, mut up, mut dn) = (false, false, false, false);
                    for c in &cards {
                        let iny = c.y < yy && yy < c.y + c.h;
                        let inx = c.x < xx && xx < c.x + c.w;
                        le |= c.x + c.w <= xx && iny;
                        ri |= c.x >= xx && iny;
                        up |= c.y + c.h <= yy && inx;
                        dn |= c.y >= yy && inx;
                    }
                    if (0..self.w).contains(&xx) && (0..self.h).contains(&yy) {
                        if (le && ri) || (up && dn) {
                            bad[self.idx(xx, yy)] = true;
                        } else {
                            pay[self.idx(xx, yy)] = true;
                        }
                    }
                }
            }
            fmask.push(Some((bad, pay)));
        }
        fmask
    }
}

/// Whether a node lies in a zone's title band (A5), without the rest of a grid.
pub(crate) fn title_bands(l: &Lattice) -> impl Fn(Pt) -> bool {
    let pl = Plane {
        w: l.width + 1,
        h: l.height + 1,
    };
    let band = pl.title_bands(l);
    move |p: Pt| (0..pl.w).contains(&p.0) && (0..pl.h).contains(&p.1) && band[pl.idx(p.0, p.1)]
}

/// The routes found so far for an arrow, and their node lists (each route once).
#[derive(Default)]
struct Found {
    seen: HashSet<Vec<Pt>>,
    out: Vec<Route>,
}

pub(crate) struct Grid {
    pub w: i32,
    pub h: i32,
    pub block: Vec<bool>,
    pub band: Vec<bool>,
    hug: Vec<bool>,
    hf: Vec<bool>,
    vf: Vec<bool>,
    /// Per framed panel: the nodes between two of its cards (no line of another group) and the
    /// others inside it (a line of another group pays).
    fmask: Vec<Option<(Vec<bool>, Vec<bool>)>>,
    foreign: std::cell::RefCell<std::collections::HashMap<Vec<usize>, std::rc::Rc<Vec<u8>>>>,
}

impl Grid {
    pub(crate) fn new(inp: &Input, l: &Lattice) -> Grid {
        let pl = Plane {
            w: l.width + 1,
            h: l.height + 1,
        };
        let frames: Vec<Rect> = l.frame.iter().flatten().copied().collect();
        let (hf, vf) = pl.frame_lines(&frames);
        Grid {
            w: pl.w,
            h: pl.h,
            block: pl.blocked(inp, l, &frames),
            band: pl.title_bands(l),
            hug: pl.hugging(l, &frames),
            hf,
            vf,
            fmask: pl.frame_masks(inp, l),
            foreign: std::cell::RefCell::default(),
        }
    }

    pub(crate) fn idx(&self, p: Pt) -> usize {
        (p.1 * self.w + p.0) as usize
    }

    pub(crate) fn inside(&self, p: Pt) -> bool {
        (0..self.w).contains(&p.0) && (0..self.h).contains(&p.1)
    }

    pub(crate) fn free(&self, p: Pt) -> bool {
        self.inside(p) && !self.block[self.idx(p)]
    }

    fn edge_ok(&self, p: Pt, d: usize) -> bool {
        let at = |x: i32, y: i32, v: &Vec<bool>| self.inside((x, y)) && v[self.idx((x, y))];
        match d {
            0 => !at(p.0, p.1, &self.hf),
            2 => !at(p.0 - 1, p.1, &self.hf),
            1 => !at(p.0, p.1, &self.vf),
            _ => !at(p.0, p.1 - 1, &self.vf),
        }
    }

    pub(crate) fn foreign_cached(&self, mut own: Vec<usize>) -> std::rc::Rc<Vec<u8>> {
        own.sort_unstable();
        own.dedup();
        let mut c = self.foreign.borrow_mut();
        c.entry(own.clone())
            .or_insert_with(|| std::rc::Rc::new(self.foreign(&own)))
            .clone()
    }

    /// For an arrow whose ends lie in the frames `own`: 2 a node it may not use (between two
    /// cards of another group), 1 a node it pays for (elsewhere inside another group).
    pub(crate) fn foreign(&self, own: &[usize]) -> Vec<u8> {
        let mut out = vec![0u8; self.block.len()];
        for (i, m) in self.fmask.iter().enumerate() {
            let Some((bad, pay)) = m else { continue };
            if own.contains(&i) {
                continue;
            }
            for (k, o) in out.iter_mut().enumerate() {
                if bad[k] {
                    *o = 2;
                } else if pay[k] && *o == 0 {
                    *o = 1;
                }
            }
        }
        out
    }

    /// What stepping onto node `m` in direction `nd` costs, coming in direction `d`.
    fn step_cost(&self, m: usize, d: usize, nd: usize, fr: &[u8]) -> f64 {
        1.0 + if nd == d { 0.0 } else { BEND }
            + if fr[m] == 1 { FOREIGN } else { 0.0 }
            + if self.hug[m] { HUG } else { 0.0 }
            + if nd.is_multiple_of(2) && self.band[m] {
                W_BAND
            } else {
                0.0
            }
    }
}

/// The lattice points of a side where a line may land, with their cost: the middle half (P19)
/// free, wider only when needed (it costs), never a corner.
pub(crate) fn ports(r: Rect, side: Side, icon: Option<(i32, i32)>) -> Vec<(Pt, f64)> {
    // a node (D-087): its arrows end on its icon, above, below or before it; on the side of its
    // text, level with the icon (they start after the text)
    if let Some((cx, cy)) = icon {
        return vec![(
            match side {
                Side::Top => (r.x + cx, r.y),
                Side::Bottom => (r.x + cx, r.y + r.h),
                Side::Left => (r.x, r.y + cy),
                Side::Right => (r.x + r.w, r.y + cy),
            },
            0.0,
        )];
    }
    let n = if matches!(side, Side::Top | Side::Bottom) {
        r.w
    } else {
        r.h
    };
    let nf = f64::from(n);
    let (q1, q3) = (nf / 4.0, 3.0 * nf / 4.0);
    (2..=n - 2)
        .map(|k| {
            let kf = f64::from(k);
            let mut c = (kf - nf / 2.0).abs();
            if kf < q1 {
                c += 6.0 * (q1 - kf);
            } else if kf > q3 {
                c += 6.0 * (kf - q3);
            }
            let p = match side {
                Side::Top => (r.x + k, r.y),
                Side::Bottom => (r.x + k, r.y + r.h),
                Side::Left => (r.x, r.y + k),
                Side::Right => (r.x + r.w, r.y + k),
            };
            (p, c)
        })
        .collect()
}

fn port_cost(r: Rect, side: Side, icon: Option<(i32, i32)>, p: Pt) -> Option<f64> {
    ports(r, side, icon)
        .into_iter()
        .find(|q| q.0 == p)
        .map(|q| q.1)
}

/// `side` of card `own` is turned away from card `other`: `other` lies wholly beyond the
/// opposite side (a hook, A3; the judge says the same).
pub(crate) fn away(side: Side, own: Rect, other: Rect) -> bool {
    match side {
        Side::Top => other.y >= own.y + own.h,
        Side::Bottom => other.y + other.h <= own.y,
        Side::Left => other.x >= own.x + own.w,
        Side::Right => other.x + other.w <= own.x,
    }
}

/// What a route costs beyond its steps and bends: a hook at either end (A3), bends past the
/// second (A8).
fn extras(s: Rect, t: Rect, nodes: &[Pt], sside: Side, tside: Side) -> f64 {
    let hooks = u8::from(away(sside, s, t)) + u8::from(away(tside, t, s));
    W_HOOK * f64::from(hooks) + W_MANY * (corners(nodes).len().saturating_sub(4) as f64)
}

/// The rules one line keeps alone: no jog shorter than `[event-gap]` (H2.23), no staircase
/// (H2.23), no wild detour (H2.24), a stub and a tail (H2.8, H2.9).
pub(crate) fn unary_ok(pts: &[Pt], shortest: f64) -> bool {
    let n = pts.len() - 1;
    let seg: Vec<i32> = (0..n)
        .map(|k| (pts[k + 1].0 - pts[k].0).abs() + (pts[k + 1].1 - pts[k].1).abs())
        .collect();
    if n >= 1 && (seg[0] < 1 || seg[n - 1] < 2) {
        return false;
    }
    for k in 1..n.saturating_sub(1) {
        let (a, b, c, d) = (pts[k - 1], pts[k], pts[k + 1], pts[k + 2]);
        let u = (b.0 - a.0, b.1 - a.1);
        let v = (d.0 - c.0, d.1 - c.1);
        if seg[k] < 2 && u.0 * v.0 + u.1 * v.1 > 0 {
            return false;
        }
    }
    if n >= 4 {
        let mono =
            |v: Vec<i32>| v.windows(2).all(|w| w[0] <= w[1]) || v.windows(2).all(|w| w[0] >= w[1]);
        if mono(pts.iter().map(|p| p.0).collect()) && mono(pts.iter().map(|p| p.1).collect()) {
            return false;
        }
    }
    f64::from(seg.iter().sum::<i32>()) <= 2.0 * shortest + 20.0
}

fn shortest(s: Rect, t: Rect) -> f64 {
    let c = |r: Rect| {
        (
            f64::from(r.x) + f64::from(r.w) / 2.0,
            f64::from(r.y) + f64::from(r.h) / 2.0,
        )
    };
    let (a, b) = (c(s), c(t));
    (a.0 - b.0).abs() + (a.1 - b.1).abs()
}

/// One arrow's search space: its two cards, the nodes it may not use or pays for, its region.
pub(crate) struct Arrow<'g> {
    g: &'g Grid,
    pub s: Rect,
    pub t: Rect,
    /// A node's icon, where its arrows end (steps from its box's corner).
    si: Option<(i32, i32)>,
    ti: Option<(i32, i32)>,
    fr: std::rc::Rc<Vec<u8>>,
    region: (i32, i32, i32, i32),
}

impl<'g> Arrow<'g> {
    pub(crate) fn new(g: &'g Grid, inp: &Input, l: &Lattice, from: usize, to: usize) -> Arrow<'g> {
        let (s, t) = (l.card[from], l.card[to]);
        let own: Vec<usize> = [l.zones_of(inp, from), l.zones_of(inp, to)]
            .into_iter()
            .flatten()
            .collect();
        let pad = 30;
        let region = (
            (s.x.min(t.x) - pad).max(0),
            (s.y.min(t.y) - pad).max(0),
            ((s.x + s.w).max(t.x + t.w) + pad).min(g.w - 1),
            ((s.y + s.h).max(t.y + t.h) + pad).min(g.h - 1),
        );
        let icon = |c: usize| inp.icon[c].map(|i| i.port(l.card[c].h));
        Arrow {
            g,
            s,
            t,
            si: icon(from),
            ti: icon(to),
            fr: g.foreign_cached(own),
            region,
        }
    }

    fn in_region(&self, p: Pt) -> bool {
        let (x0, y0, x1, y1) = self.region;
        x0 <= p.0 && p.0 <= x1 && y0 <= p.1 && p.1 <= y1
    }

    /// Multi-source search over (node, direction) states from one side's ports.
    fn search(&self, side: Side, penal: &dyn Fn(usize) -> f64) -> (Vec<f64>, Vec<u32>) {
        let g = self.g;
        let o = side.out();
        let mut starts = Vec::new();
        for (p, pc) in ports(self.s, side, self.si) {
            let n1 = (p.0 + DIRS[o].0, p.1 + DIRS[o].1);
            if g.free(n1) {
                starts.push((g.idx(n1) * 4 + o, 1.0 + 0.5 * pc));
            }
        }
        self.run(&starts, penal)
    }

    /// The search from these states (state, cost).
    /// Costs are counted in quarters, whole numbers, in a queue of buckets (Dial's): every
    /// step costs at most `BUCKETS` quarters (a penalty is capped), so the queue is a ring.
    fn run(&self, starts: &[(usize, f64)], penal: &dyn Fn(usize) -> f64) -> (Vec<f64>, Vec<u32>) {
        const BUCKETS: usize = 4096;
        const MAX_PENAL: f64 = 900.0;
        let g = self.g;
        let n = g.block.len() * 4;
        let q = |c: f64| (c * 4.0).round() as u32;
        let mut dist = vec![u32::MAX; n];
        let mut parent = vec![u32::MAX; n];
        let mut ring: Vec<Vec<u32>> = vec![Vec::new(); BUCKETS];
        let mut pending = 0usize;
        // the nodes a route arrives by (two steps out of a port of the target): once the
        // cheapest is reached, a state dearer by SLACK leads to no route worth keeping
        let mut arrive = vec![false; g.block.len()];
        for t in SIDES {
            let ot = t.out();
            for (p, _) in ports(self.t, t, self.ti) {
                let a2 = (p.0 + 2 * DIRS[ot].0, p.1 + 2 * DIRS[ot].1);
                if g.inside(a2) {
                    arrive[g.idx(a2)] = true;
                }
            }
        }
        let mut reached = u32::MAX;
        let mut at = u32::MAX;
        for &(st, c) in starts {
            let c = q(c);
            if c < dist[st] {
                dist[st] = c;
                ring[c as usize % BUCKETS].push(st as u32);
                pending += 1;
                at = at.min(c);
            }
        }
        let slack = q(SLACK);
        while pending > 0 {
            let Some(st) = ring[at as usize % BUCKETS].pop() else {
                at += 1;
                continue;
            };
            pending -= 1;
            let st = st as usize;
            let c = at;
            if c != dist[st] {
                continue;
            }
            if reached != u32::MAX && c > reached + slack {
                break;
            }
            let (m0, d) = (st / 4, st % 4);
            if reached == u32::MAX && arrive[m0] {
                reached = c;
            }
            let p = ((m0 as i32) % g.w, (m0 as i32) / g.w);
            for nd in [d, (d + 1) % 4, (d + 3) % 4] {
                let nq = (p.0 + DIRS[nd].0, p.1 + DIRS[nd].1);
                if !self.in_region(nq) || !g.free(nq) {
                    continue;
                }
                let m = g.idx(nq);
                if self.fr[m] == 2 || !g.edge_ok(p, nd) {
                    continue;
                }
                let cost = c + q(g.step_cost(m, d, nd, &self.fr) + penal(m).min(MAX_PENAL));
                let t = m * 4 + nd;
                if cost < dist[t] {
                    dist[t] = cost;
                    parent[t] = st as u32;
                    ring[cost as usize % BUCKETS].push(t as u32);
                    pending += 1;
                }
            }
        }
        let dist = dist
            .into_iter()
            .map(|d| {
                if d == u32::MAX {
                    f64::INFINITY
                } else {
                    f64::from(d) / 4.0
                }
            })
            .collect();
        (dist, parent)
    }

    /// The candidate routes: the `keep` cheapest, diverse. `penal`: what other lines make dearer
    /// (a route that knows its neighbours); its share is left out of the route's own cost.
    pub(crate) fn candidates(
        &self,
        per_pair: usize,
        rounds: usize,
        keep: usize,
        penal0: &dyn Fn(usize) -> f64,
    ) -> Vec<Route> {
        let g = self.g;
        let short = shortest(self.s, self.t);
        let mut found = Found::default();
        let mut used = vec![0.0f64; g.block.len()];
        for _ in 0..rounds {
            for sside in SIDES {
                let pen = |m: usize| penal0(m) + used[m];
                let tree = self.search(sside, &pen);
                for tside in SIDES {
                    self.arrivals((sside, tside), &tree, per_pair, &pen, short, &mut found);
                }
            }
            for r in &found.out {
                for &p in &r.nodes[2..r.nodes.len() - 2] {
                    used[g.idx(p)] += 6.0;
                }
            }
        }
        diverse(found.out, keep)
    }

    /// The routes from side `sides.0` (searched: `tree`, each state's cost and parent) arriving
    /// at side `sides.1`: the `per_pair` cheapest ports, each through its two-step tail.
    fn arrivals(
        &self,
        (sside, tside): (Side, Side),
        (dist, parent): &(Vec<f64>, Vec<u32>),
        per_pair: usize,
        pen: &dyn Fn(usize) -> f64,
        short: f64,
        found: &mut Found,
    ) {
        let Found { seen, out } = found;
        let g = self.g;
        let o = sside.out();
        let ot = tside.out();
        let inward = (ot + 2) % 4;
        let mut found = Vec::new();
        for (q, qc) in ports(self.t, tside, self.ti) {
            let a1 = (q.0 + DIRS[ot].0, q.1 + DIRS[ot].1);
            let a2 = (q.0 + 2 * DIRS[ot].0, q.1 + 2 * DIRS[ot].1);
            if !g.free(a1) || !g.free(a2) {
                continue;
            }
            let mut best: Option<(f64, usize)> = None;
            for d in 0..4 {
                if d == ot {
                    continue;
                }
                let s = g.idx(a2) * 4 + d;
                if !dist[s].is_finite() {
                    continue;
                }
                let c = dist[s] + if d == inward { 0.0 } else { BEND } + 2.0 + 0.5 * qc;
                if best.is_none_or(|b| c < b.0) {
                    best = Some((c, s));
                }
            }
            if let Some((c, s)) = best {
                found.push((c, s, q, a1));
            }
        }
        found.sort_by(|a, b| a.0.total_cmp(&b.0));
        for (c, s, q, a1) in found.into_iter().take(per_pair) {
            let mut seq = Vec::new();
            let mut st = s as u32;
            while st != u32::MAX {
                let m = (st / 4) as i32;
                seq.push((m % g.w, m / g.w));
                st = parent[st as usize];
            }
            seq.reverse();
            let first = seq[0];
            let p0 = (first.0 - DIRS[o].0, first.1 - DIRS[o].1);
            let mut nodes = vec![p0];
            nodes.extend(seq);
            nodes.push(a1);
            nodes.push(q);
            let uniq: HashSet<Pt> = nodes.iter().copied().collect();
            if uniq.len() != nodes.len() || seen.contains(&nodes) {
                continue;
            }
            seen.insert(nodes.clone());
            let mut cost = c;
            for &p in &nodes[2..nodes.len() - 2] {
                cost -= pen(g.idx(p));
            }
            cost += extras(self.s, self.t, &nodes, sside, tside);
            let r = Route::new(nodes, sside, tside, cost);
            if unary_ok(&r.pts, short) {
                out.push(r);
            }
        }
    }

    fn path(&self, parent: &[u32], s: usize) -> Vec<Pt> {
        let g = self.g;
        let mut seq = Vec::new();
        let mut st = s as u32;
        while st != u32::MAX {
            let m = (st / 4) as i32;
            seq.push((m % g.w, m / g.w));
            st = parent[st as usize];
        }
        seq.reverse();
        seq
    }

    /// Routes sharing a trunk with a sibling's route `mate`: for a join (`join`, the same target),
    /// the line runs to a node of `mate` and ends with it; for a fork, it starts with `mate` and
    /// leaves it at one of its nodes (H2.16, H2.17: forks part late, joins meet early).
    pub(crate) fn siblings(&self, mates: &[&Route], join: bool, keep: usize) -> Vec<Route> {
        let mut out = Vec::new();
        if join {
            let searched: Vec<(Side, Searched)> = SIDES
                .iter()
                .map(|&sd| (sd, self.search(sd, &|_| 0.0)))
                .collect();
            for m in mates {
                out.extend(self.sibling(m, Some(&searched), keep));
            }
        } else {
            for m in mates {
                out.extend(self.sibling(m, None, keep));
            }
        }
        out
    }

    fn sibling(
        &self,
        mate: &Route,
        joined: Option<&[(Side, Searched)]>,
        keep: usize,
    ) -> Vec<Route> {
        let g = self.g;
        let n = mate.nodes.len();
        if n < 6 {
            return Vec::new();
        }
        let dir = |a: Pt, b: Pt| DIRS.iter().position(|&v| v == (b.0 - a.0, b.1 - a.1));
        let mut out: Vec<Route> = Vec::new();
        let mut seen: HashSet<Vec<Pt>> = HashSet::new();
        let mut push = |nodes: Vec<Pt>, ss: Side, ts: Side, out: &mut Vec<Route>| {
            if seen.insert(nodes.clone())
                && let Some(r) = self.route(nodes, ss, ts)
            {
                out.push(r);
            }
        };
        if let Some(searched) = joined {
            for (sside, (dist, parent)) in searched {
                let sside = *sside;
                for i in 2..n - 3 {
                    let v = mate.nodes[i];
                    let Some(dout) = dir(v, mate.nodes[i + 1]) else {
                        continue;
                    };
                    let mut best: Option<(f64, usize)> = None;
                    for d in 0..4 {
                        if d == (dout + 2) % 4 {
                            continue;
                        }
                        let st = g.idx(v) * 4 + d;
                        if dist[st].is_finite() && best.is_none_or(|b| dist[st] < b.0) {
                            best = Some((dist[st], st));
                        }
                    }
                    let Some((_, st)) = best else { continue };
                    let seq = self.path(parent, st);
                    let first = seq[0];
                    let o = sside.out();
                    let mut nodes = vec![(first.0 - DIRS[o].0, first.1 - DIRS[o].1)];
                    nodes.extend(seq);
                    nodes.extend_from_slice(&mate.nodes[i + 1..]);
                    push(nodes, sside, mate.tside, &mut out);
                }
            }
        } else {
            let mut starts = Vec::new();
            for i in 1..n - 3 {
                if let Some(d) = dir(mate.nodes[i - 1], mate.nodes[i]) {
                    starts.push((g.idx(mate.nodes[i]) * 4 + d, i as f64));
                }
            }
            let (dist, parent) = self.run(&starts, &|_| 0.0);
            for tside in SIDES {
                let ot = tside.out();
                for (q, _) in ports(self.t, tside, self.ti) {
                    let a1 = (q.0 + DIRS[ot].0, q.1 + DIRS[ot].1);
                    let a2 = (q.0 + 2 * DIRS[ot].0, q.1 + 2 * DIRS[ot].1);
                    if !g.free(a1) || !g.free(a2) {
                        continue;
                    }
                    let Some((_, st)) = (0..4)
                        .filter(|&d| d != ot)
                        .map(|d| (dist[g.idx(a2) * 4 + d], g.idx(a2) * 4 + d))
                        .filter(|t| t.0.is_finite())
                        .min_by(|a, b| a.0.total_cmp(&b.0))
                    else {
                        continue;
                    };
                    let seq = self.path(&parent, st);
                    let Some(i) = mate.nodes.iter().position(|&p| p == seq[0]) else {
                        continue;
                    };
                    let mut nodes = mate.nodes[..i].to_vec();
                    nodes.extend(seq);
                    nodes.push(a1);
                    nodes.push(q);
                    push(nodes, mate.sside, tside, &mut out);
                }
            }
        }
        out.sort_by(|a, b| a.cost.total_cmp(&b.cost));
        out.truncate(keep);
        out
    }

    /// The cost the candidate search gives these nodes, or `None` when they are not a valid
    /// route on this lattice (a route carried from another lattice, or slid along its side).
    pub(crate) fn cost_of(&self, nodes: &[Pt], sside: Side, tside: Side) -> Option<f64> {
        let g = self.g;
        let pc = port_cost(self.s, sside, self.si, nodes[0])?;
        let qc = port_cost(self.t, tside, self.ti, *nodes.last()?)?;
        if nodes.len() < 4 {
            return None;
        }
        let (o, ot) = (sside.out(), tside.out());
        let n = nodes.len();
        if nodes[1] != (nodes[0].0 + DIRS[o].0, nodes[0].1 + DIRS[o].1)
            || nodes[n - 2] != (nodes[n - 1].0 + DIRS[ot].0, nodes[n - 1].1 + DIRS[ot].1)
        {
            return None;
        }
        let uniq: HashSet<Pt> = nodes.iter().copied().collect();
        if uniq.len() != n {
            return None;
        }
        for &p in &nodes[1..n - 1] {
            if !g.free(p) || self.fr[g.idx(p)] == 2 {
                return None;
            }
        }
        let mut c = 1.0 + 0.5 * pc;
        let mut d = o;
        for w in nodes[1..n - 2].windows(2) {
            let (a, b) = (w[0], w[1]);
            let nd = DIRS.iter().position(|&v| v == (b.0 - a.0, b.1 - a.1))?;
            if nd == (d + 2) % 4 || !g.edge_ok(a, nd) {
                return None;
            }
            c += g.step_cost(g.idx(b), d, nd, &self.fr);
            d = nd;
        }
        if d == ot {
            return None;
        }
        let tail = if d == (ot + 2) % 4 { 0.0 } else { BEND };
        Some(c + tail + 2.0 + 0.5 * qc + extras(self.s, self.t, nodes, sside, tside))
    }

    /// A route from nodes, if valid here and keeping the rules a line keeps alone.
    pub(crate) fn route(&self, nodes: Vec<Pt>, sside: Side, tside: Side) -> Option<Route> {
        let c = self.cost_of(&nodes, sside, tside)?;
        let r = Route::new(nodes, sside, tside, c);
        unary_ok(&r.pts, shortest(self.s, self.t)).then_some(r)
    }

    /// The same routes landing elsewhere on their side: the first (or last) stretch moved along
    /// the side. The search leaves each side by its cheapest port; a neighbour arriving on that
    /// side may need the line a few steps aside.
    pub(crate) fn slides(&self, base: &[Route], keep: usize) -> Vec<Route> {
        let mut seen: HashSet<Vec<Pt>> = base.iter().map(|r| r.nodes.clone()).collect();
        let mut out = Vec::new();
        for r in base.iter().take(8) {
            let p = &r.pts;
            for (end, side) in [(0usize, r.sside), (1, r.tside)] {
                let ax = usize::from(!matches!(side, Side::Top | Side::Bottom));
                for d in [-6, -4, -2, 2, 4, 6] {
                    let Some(q) = slid(p, end, ax, d) else {
                        continue;
                    };
                    let nodes = dense(&q);
                    if seen.contains(&nodes) {
                        continue;
                    }
                    if let Some(nr) = self.route(nodes.clone(), r.sside, r.tside) {
                        seen.insert(nodes);
                        out.push(nr);
                    }
                }
            }
        }
        out.sort_by(|a, b| a.cost.total_cmp(&b.cost));
        let mut ends = HashSet::new();
        out.retain(|r| ends.insert((r.src(), r.dst())));
        out.truncate(keep);
        out
    }
}

fn shift(p: Pt, ax: usize, d: i32) -> Pt {
    if ax == 0 {
        (p.0 + d, p.1)
    } else {
        (p.0, p.1 + d)
    }
}

fn coord(p: Pt, ax: usize) -> i32 {
    if ax == 0 { p.0 } else { p.1 }
}

/// Corners `p` with one end (0 the start, 1 the end) moved `d` steps along axis `ax`.
fn slid(p: &[Pt], end: usize, ax: usize, d: i32) -> Option<Vec<Pt>> {
    let mut q = p.to_vec();
    if p.len() == 2 {
        return Some(p.iter().map(|&v| shift(v, ax, d)).collect());
    }
    let n = p.len();
    let (a, b, c) = if end == 0 {
        (0, 1, 2)
    } else {
        (n - 1, n - 2, n - 3)
    };
    if coord(p[b], ax) == coord(p[c], ax) {
        return None;
    }
    q[a] = shift(p[a], ax, d);
    q[b] = shift(p[b], ax, d);
    let before = coord(p[c], ax) - coord(p[b], ax);
    let after = coord(p[c], ax) - coord(q[b], ax);
    (before * after > 0 && after.abs() >= 2).then_some(q)
}

/// The routes kept: the cheapest of each pair of sides first, then by cost, leaving out a route
/// that runs over most of its nodes on one already kept (near copies leave no real choice).
fn diverse(mut out: Vec<Route>, keep: usize) -> Vec<Route> {
    out.sort_by(|a, b| a.cost.total_cmp(&b.cost));
    let mut kept: Vec<usize> = Vec::new();
    let mut pairs = HashSet::new();
    for (i, r) in out.iter().enumerate() {
        if pairs.insert((r.sside, r.tside)) {
            kept.push(i);
        }
    }
    let sets: Vec<HashSet<Pt>> = out
        .iter()
        .map(|r| r.nodes.iter().copied().collect())
        .collect();
    for i in 0..out.len() {
        if kept.len() >= keep {
            break;
        }
        if kept.contains(&i) {
            continue;
        }
        let near = kept.iter().any(|&j| {
            let common = sets[i].intersection(&sets[j]).count();
            common as f64 > 0.8 * sets[i].len().min(sets[j].len()) as f64
        });
        if !near {
            kept.push(i);
        }
    }
    for i in 0..out.len() {
        if kept.len() >= keep {
            break;
        }
        if !kept.contains(&i) {
            kept.push(i);
        }
    }
    kept.truncate(keep);
    kept.sort_by(|&a, &b| out[a].cost.total_cmp(&out[b].cost));
    let mut slots: Vec<Option<Route>> = out.into_iter().map(Some).collect();
    kept.into_iter().filter_map(|i| slots[i].take()).collect()
}

/// Cards `a` and `b` face each other: side by side or one above the other, their middle halves
/// level (P19), nothing between them (no card, zone text, nor a frame holding neither).
pub(crate) fn facing(inp: &Input, l: &Lattice, a: usize, b: usize) -> bool {
    let (ra, rb) = (l.card[a], l.card[b]);
    let f = |v: i32| f64::from(v);
    for horiz in [true, false] {
        let (g0, g1, m0, m1) = if horiz {
            let (lo, hi) = if ra.x < rb.x { (ra, rb) } else { (rb, ra) };
            (
                f(lo.x + lo.w),
                f(hi.x),
                (f(ra.y) + f(ra.h) / 4.0).max(f(rb.y) + f(rb.h) / 4.0),
                (f(ra.y) + 3.0 * f(ra.h) / 4.0).min(f(rb.y) + 3.0 * f(rb.h) / 4.0),
            )
        } else {
            let (lo, hi) = if ra.y < rb.y { (ra, rb) } else { (rb, ra) };
            (
                f(lo.y + lo.h),
                f(hi.y),
                (f(ra.x) + f(ra.w) / 4.0).max(f(rb.x) + f(rb.w) / 4.0),
                (f(ra.x) + 3.0 * f(ra.w) / 4.0).min(f(rb.x) + 3.0 * f(rb.w) / 4.0),
            )
        };
        if g1 <= g0 || m1 - m0 < 0.1 {
            continue;
        }
        let (x0, y0, x1, y1) = if horiz {
            (g0, m0, g1, m1)
        } else {
            (m0, g0, m1, g1)
        };
        let hit = |x: f64, y: f64, w: f64, h: f64| x < x1 && x + w > x0 && y < y1 && y + h > y0;
        let card_hit = l
            .card
            .iter()
            .enumerate()
            .any(|(c, r)| c != a && c != b && hit(f(r.x), f(r.y), f(r.w), f(r.h)));
        let text_hit = l
            .texts
            .iter()
            .flatten()
            .any(|t| hit(t.x / STEP, t.y / STEP, t.w / STEP, t.h / STEP));
        let own = [l.zones_of(inp, a), l.zones_of(inp, b)];
        let frame_hit = l.frame.iter().enumerate().any(|(i, fr)| {
            fr.is_some_and(|r| !own.contains(&Some(i)) && hit(f(r.x), f(r.y), f(r.w), f(r.h)))
        });
        if !card_hit && !text_hit && !frame_hit {
            return true;
        }
    }
    false
}
