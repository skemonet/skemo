//! The slide engine (`diagram(layout="slide")`): one model, one search, no passes
//! (`docs/plans/reliable-layout.md` of the maintainers, phase E; the constitution as amended on
//! 2026-11-17, D-111).
//!
//! 1. **Placement** ([`place`]): the groups tiled like a slide layout, the cards in their cells.
//! 2. **Lattice** ([`lattice`]): every size and gap a whole number of 10 px steps, one module for
//!    every card (S1), one header height for every zone (P8).
//! 3. **Candidates** ([`grid`]): for each arrow, its cheapest routes keeping the rules a line
//!    keeps alone (stubs, tails, no staircase, no hook unless nothing else, a title band priced).
//! 4. **The model** ([`solve`], built by [`draw`]): one route per arrow and one place per label,
//!    the rules between two lines ([`pairs`]) and between labels and lines as conflicts, the
//!    preferences in the constitution's order as one objective; solved by a deterministic search
//!    of our own.
//! 5. **Room** ([`room`]): the narrowest even gutters where everything fits; then room made
//!    where a line pays for the lack of it (one gutter wider, or a row's cards lower under their
//!    zone's title), kept only when the whole slide scores better.
//!
//! A diagram the slide engine does not read (nested zones, pinned cards, an arrow to a zone)
//! goes to the grid engine.

mod draw;
mod grid;
mod input;
mod labels;
mod lattice;
mod pairs;
mod place;
mod room;
mod solve;

use crate::layout::Box2;
use crate::measure::Sizes;
use crate::route::Points;
use crate::scene::Scene;
use grid::Route;
pub(crate) use input::{Input, reads};
use input::{read, wrapped};
use lattice::{Extra, Lattice, Tiling};
use room::room_cost;
use skemo_lang::{Kind, Model};
use solve::Arrow;
use std::collections::BTreeMap;

pub(crate) const STEP: f64 = 10.0;
pub(crate) type Pt = (i32, i32);

/// A rectangle in steps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    pub(crate) fn px(self) -> BoxPx {
        BoxPx {
            x: f64::from(self.x) * STEP,
            y: f64::from(self.y) * STEP,
            w: f64::from(self.w) * STEP,
            h: f64::from(self.h) * STEP,
        }
    }
}

pub(crate) type BoxPx = Box2;

/// The zone texts (title, description, tag) of each framed panel, given the frames, px.
pub(crate) type Walls<'a> = dyn Fn(&[(usize, BoxPx)]) -> Vec<(usize, BoxPx)> + 'a;

thread_local! {
    /// The last diagram asked of the slide engine was drawn by it (not handed to the grid).
    static DREW: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static CACHE: std::cell::RefCell<Vec<(String, Drawn)>> = const { std::cell::RefCell::new(Vec::new()) };
    static PLACED: std::cell::RefCell<Vec<(String, Tiling)>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// The last few results kept.
fn remember<T>(v: &mut Vec<(String, T)>, key: String, val: T) {
    v.retain(|(k, _)| *k != key);
    v.push((key, val));
    if v.len() > 4 {
        v.remove(0);
    }
}

/// `SKEMO_SLIDE_TRACE=1`: each stage and its time on standard error (native builds only).
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn trace(what: &str) {
    use std::sync::OnceLock;
    static T0: OnceLock<Option<std::time::Instant>> = OnceLock::new();
    if let Some(t0) = T0.get_or_init(|| {
        std::env::var("SKEMO_SLIDE_TRACE")
            .ok()
            .map(|_| std::time::Instant::now())
    }) {
        eprintln!("slide {:>7.3}s {what}", t0.elapsed().as_secs_f64());
    }
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn trace(_: &str) {}

fn steps(px: f64) -> i32 {
    (px / STEP - 1e-9).ceil() as i32
}

/// The model read as panels of cards, or `None` when the slide engine does not draw it.
/// Each label wrapped at `[label-width]` (L6): its lines and its pill's size.
/// What the slide engine hands to the renderer.
#[derive(Clone)]
pub(crate) struct Drawn {
    /// Labels wrapped on more than one line, and every label's size.
    pub label_lines: BTreeMap<String, Vec<String>>,
    pub label_sizes: BTreeMap<String, crate::measure::Size>,
    pub boxes: BTreeMap<String, Box2>,
    pub routes: BTreeMap<String, Points>,
    pub labels: BTreeMap<String, (f64, f64)>,
}

/// Places, routes and labels `model`, or `None` when the slide engine does not draw it.
/// Whether the last diagram asked of the slide engine was drawn by it.
pub(crate) fn drew() -> bool {
    DREW.with(std::cell::Cell::get)
}

pub(crate) fn arrange(
    model: &Model,
    scene: &Scene,
    sizes: &Sizes,
    head: f64,
    preview: bool,
) -> Option<Drawn> {
    let out = arrange_inner(model, scene, sizes, head, preview);
    DREW.with(|d| d.set(out.is_some()));
    out
}

fn arrange_inner(
    model: &Model,
    scene: &Scene,
    sizes: &Sizes,
    head: f64,
    preview: bool,
) -> Option<Drawn> {
    let wraps = wrapped(model, scene);
    let mut sizes = sizes.clone();
    for (id, (_, sz)) in &wraps {
        sizes.labels.insert(id.clone(), *sz);
    }
    let inp = read(model, scene, &sizes, head)?;
    // the pipeline lays a diagram out more than once (a preview for the titles' wraps, then a
    // check of them): the same input is drawn once, and placed once
    let base = inp.key();
    let texts: Vec<String> = model
        .nodes
        .iter()
        .filter(|n| n.kind == Kind::Zone)
        .map(|n| {
            let subs = scene.zones.get(&n.id).map(|z| {
                z.subs
                    .iter()
                    .map(|(_, t)| t.as_str())
                    .collect::<Vec<_>>()
                    .join("|")
            });
            format!("{}#{:?}", scene.zone_text(&n.id, &n.title), subs)
        })
        .collect();
    let key = format!("{base}|{preview}|{texts:?}");
    if let Some(d) = CACHE.with(|c| {
        c.borrow()
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, d)| d.clone())
    }) {
        return Some(d);
    }
    let walls = |frames: &[(usize, BoxPx)]| -> Vec<(usize, BoxPx)> {
        let mut boxes = BTreeMap::new();
        let mut of = BTreeMap::new();
        for &(i, b) in frames {
            if let Some(z) = &inp.panels[i].zone {
                boxes.insert(z.clone(), b);
                of.insert(z.clone(), i);
            }
        }
        crate::labels::zone_titles(model, scene, &boxes)
            .into_iter()
            .filter_map(|(id, b)| of.get(&id).map(|&i| (i, b)))
            .collect()
    };
    trace("start");
    let tiling = if let Some(t) = PLACED.with(|c| {
        c.borrow()
            .iter()
            .find(|(k, _)| *k == base)
            .map(|(_, t)| t.clone())
    }) {
        t
    } else {
        let t = place::placement(&inp, &walls);
        PLACED.with(|c| remember(&mut c.borrow_mut(), base.clone(), t.clone()));
        t
    };
    trace(&format!("placement {}x{}", tiling.cols, tiling.rows));
    let mut engine = Engine::new(&inp, &walls, tiling);
    if preview {
        let l = Lattice::new(
            &inp,
            &engine.tiling,
            engine.needs(),
            &Extra::default(),
            &walls,
        );
        let out = Drawing::empty(l, inp.edges.len()).output(&inp);
        CACHE.with(|c| remember(&mut c.borrow_mut(), key, out.clone()));
        return Some(out);
    }
    let best = engine.run()?;
    let mut out = best.output(&inp);
    for (id, (lines, sz)) in wraps {
        if lines.len() > 1 {
            out.label_lines.insert(id.clone(), lines);
        }
        out.label_sizes.insert(id, sz);
    }
    CACHE.with(|c| remember(&mut c.borrow_mut(), key, out.clone()));
    Some(out)
}

/// A drawing of the whole slide on one lattice.
struct Drawing {
    l: Lattice,
    /// Every arrow's candidates on this lattice.
    cands: Vec<Vec<Route>>,
    routes: Vec<Option<Route>>,
    /// (arrow, label box px)
    labels: Vec<(usize, BoxPx)>,
    dropped: usize,
    ldropped: usize,
    crossings: usize,
    objective: f64,
}

impl Drawing {
    fn empty(l: Lattice, n: usize) -> Drawing {
        Drawing {
            l,
            cands: vec![Vec::new(); n],
            routes: vec![None; n],
            labels: Vec::new(),
            dropped: 0,
            ldropped: 0,
            crossings: 0,
            objective: 0.0,
        }
    }

    fn score(&self) -> f64 {
        self.objective + room_cost(&self.l)
    }

    fn output(&self, inp: &Input) -> Drawn {
        let mut boxes = BTreeMap::new();
        // a pile: the module and the room of its cards behind (5 px a card), within its step
        let top = |c: usize| {
            let mut b = self.l.card[c].px();
            if let Some(icon) = inp.icon[c] {
                return icon.node_box(b);
            }
            if inp.pile[c] > 0.0 {
                b.w = f64::from(inp.card_w) * STEP + inp.pile[c];
                b.h = f64::from(inp.card_h) * STEP + inp.pile[c];
            }
            b
        };
        for (c, id) in inp.cards.iter().enumerate() {
            boxes.insert(id.clone(), top(c));
        }
        for (i, p) in inp.panels.iter().enumerate() {
            if let (Some(z), Some(f)) = (&p.zone, self.l.frame[i]) {
                boxes.insert(z.clone(), f.px());
            }
        }
        let mut routes = BTreeMap::new();
        for (k, r) in self.routes.iter().enumerate() {
            if let Some(r) = r {
                let mut pts: Points = r
                    .pts
                    .iter()
                    .map(|p| (f64::from(p.0) * STEP, f64::from(p.1) * STEP))
                    .collect();
                // an end on the right or the bottom of a pile meets its cards behind, not the
                // step the lattice gave them
                let e = &inp.edges[k];
                let last = pts.len() - 1;
                for (i, c) in [(0, e.from), (last, e.to)] {
                    let (cell, b) = (self.l.card[c].px(), top(c));
                    let p = &mut pts[i];
                    if (p.0 - (cell.x + cell.w)).abs() < 0.5 {
                        p.0 = b.x + b.w;
                    }
                    if (p.1 - (cell.y + cell.h)).abs() < 0.5 {
                        p.1 = b.y + b.h;
                    }
                }
                // a node's arrows end on its icon (D-087): the last stretch runs on to it
                for (i, c, side) in [(0, e.from, r.sside), (last, e.to, r.tside)] {
                    if let Some(icon) = inp.icon[c] {
                        let b = top(c);
                        to_icon(&mut pts, i, side, icon.within(b), b);
                    }
                }
                routes.insert(e.id.clone(), pts);
            }
        }
        let labels = self
            .labels
            .iter()
            .map(|(k, b)| (inp.edges[*k].id.clone(), (b.x + b.w / 2.0, b.y + b.h / 2.0)))
            .collect();
        Drawn {
            label_lines: BTreeMap::new(),
            label_sizes: BTreeMap::new(),
            boxes,
            routes,
            labels,
        }
    }
}

/// End `i` of `pts`, on side `side` of a node (box `b`), moved onto its icon `ib`, or after its
/// text on the text's side: the stretch that reaches it moves along with it (it stays level or
/// upright).
fn to_icon(pts: &mut Points, i: usize, side: grid::Side, ib: Box2, b: Box2) {
    let j = if i == 0 { 1 } else { i - 1 };
    let (cx, cy) = (ib.x + ib.w / 2.0, ib.y + ib.h / 2.0);
    match side {
        grid::Side::Top | grid::Side::Bottom => {
            if (pts[j].0 - pts[i].0).abs() < 0.5 {
                pts[j].0 = cx;
            }
            pts[i].0 = cx;
            pts[i].1 = if side == grid::Side::Top {
                ib.y
            } else {
                ib.y + ib.h
            };
        }
        grid::Side::Left | grid::Side::Right => {
            if (pts[j].1 - pts[i].1).abs() < 0.5 {
                pts[j].1 = cy;
            }
            pts[i].1 = cy;
            pts[i].0 = if side == grid::Side::Left {
                ib.x
            } else {
                b.right()
            };
        }
    }
}

struct Engine<'a> {
    inp: &'a Input,
    walls: &'a Walls<'a>,
    tiling: Tiling,
    arrows: Vec<Arrow>,
}

impl<'a> Engine<'a> {
    fn new(inp: &'a Input, walls: &'a Walls<'a>, tiling: Tiling) -> Engine<'a> {
        let arrows = inp
            .edges
            .iter()
            .map(|e| Arrow {
                src: e.from,
                dst: e.to,
                look: e.look.clone(),
                label: e.label.clone(),
                face: false,
            })
            .collect();
        Engine {
            inp,
            walls,
            tiling,
            arrows,
        }
    }
}
