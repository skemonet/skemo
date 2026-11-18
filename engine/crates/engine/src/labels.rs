//! Placement of arrow labels (E-ROUTE-4): candidate positions along the route, and choice of the
//! one that is least in the way (cards, zone titles, other labels, other arrows).
//! On a tie: the middle of the longest segment, as the prototype.

use crate::geometry;
use crate::layout::Box2;
use crate::measure::{Sizes, line_height, text_style};
use crate::route::{Points, mid_of_longest};
use crate::scene::Scene;
use skemo_lang::Model;
use std::collections::BTreeMap;

fn hits(a: &Box2, b: &Box2) -> bool {
    a.x < b.right() && b.x < a.right() && a.y < b.bottom() && b.y < a.bottom()
}

fn seg_hits(r: &Box2, p: (f64, f64), q: (f64, f64)) -> bool {
    let (x0, x1) = (p.0.min(q.0), p.0.max(q.0));
    let (y0, y1) = (p.1.min(q.1), p.1.max(q.1));
    x1 > r.x && x0 < r.right() && y1 > r.y && y0 < r.bottom()
}

/// Boxes of the zone titles (file coordinates), with the zone's identifier. Also obstacles for the
/// router (`route::router_scene`).
pub fn zone_titles(
    model: &Model,
    scene: &Scene,
    boxes: &BTreeMap<String, Box2>,
) -> Vec<(String, Box2)> {
    let mut out = Vec::new();
    for n in model
        .nodes
        .iter()
        .filter(|n| n.kind == skemo_lang::Kind::Zone && !n.title.is_empty())
    {
        let (Some(b), Some(z)) = (boxes.get(&n.id), scene.zones.get(&n.id)) else {
            continue;
        };
        let text = scene.zone_text(&n.id, &n.title);
        let zt = geometry::zone_title(scene, z, (text, n.tag.as_deref()), b.w, b.h);
        let run = &zt.run;
        let c = scene.style(z.title);
        let (asc, _) = text_style(c).metrics();
        let lh = line_height(c);
        out.push((
            n.id.clone(),
            Box2 {
                x: b.x + zt.span.0 - 2.0,
                y: b.y + run.baseline - asc - 2.0,
                w: zt.span.1 - zt.span.0 + 4.0,
                // a wrapped title (D-099) is a wall over all its lines
                h: lh * (zt.more.len() as f64 + 1.0) + 4.0,
            },
        ));
        // its description lines (v0.8): walls for the lines, like the title
        for s in &zt.subs {
            let sc = scene.style(s.el);
            let (sa, _) = text_style(sc).metrics();
            out.push((
                n.id.clone(),
                Box2 {
                    x: b.x + s.x - 2.0,
                    y: b.y + s.baseline - sa - 2.0,
                    w: s.width + 4.0,
                    h: line_height(sc) + 4.0,
                },
            ));
        }
        // the pill of `tag=` too, wherever it stands (D-050)
        if let Some(p) = &zt.tag {
            out.push((
                n.id.clone(),
                Box2 {
                    x: b.x + p.x - 2.0,
                    y: b.y + p.y - 2.0,
                    w: p.w + 4.0,
                    h: p.h + 4.0,
                },
            ));
        }
    }
    out
}

/// What stands in the drawing, for the labels.
struct Stage<'a> {
    cards: Vec<Box2>,
    /// The visible zones' outlines (an invisible group has none to keep clear of).
    zones: Vec<Box2>,
    titles: Vec<Box2>,
    title_rows: Vec<Box2>,
    /// Where grouped arrows split or meet: a label there hides the junction (D-093).
    junctions: Vec<(f64, f64)>,
    routes: &'a BTreeMap<String, Points>,
}

/// One label being placed.
struct Label<'a> {
    id: &'a str,
    /// The arrows of its join (D-054), itself included.
    joined: Vec<&'a str>,
    route: &'a Points,
    /// The routes of the other arrows of its join.
    mates: Vec<&'a Points>,
    /// The middle of its longest segment, or the spot its join shares.
    default: (f64, f64),
    w: f64,
    h: f64,
}

impl Stage<'_> {
    /// Is `id` a line of another flow than the label's? (the arrows of its join run under the
    /// label they share, D-054)
    fn foreign(&self, l: &Label, id: &str) -> bool {
        id != l.id && !l.joined.contains(&id)
    }

    /// What bounds the places of a label at `rows`: of everything (`all`), else of the cards and
    /// outlines alone.
    fn around(&self, placed: &[Box2], l: &Label, rows: (f64, f64), all: bool) -> Vec<Wall> {
        let mut v = walls(&self.cards, &self.zones, rows);
        if !all {
            return v;
        }
        let (top, bottom) = (rows.0 - 2.0, rows.1 + 2.0);
        let level = |t: &&Box2| t.y < bottom && top < t.bottom();
        v.extend(
            self.titles
                .iter()
                .chain(placed)
                .filter(level)
                .map(|t| (t.x - PAD, t.right() + PAD, t.x)),
        );
        for (_, other) in self.routes.iter().filter(|(id, _)| self.foreign(l, id)) {
            for s in other.windows(2) {
                let (y0, y1) = (s[0].1.min(s[1].1), s[0].1.max(s[1].1));
                if y1 > top && y0 < bottom {
                    let (x0, x1) = (s[0].0.min(s[1].0), s[0].0.max(s[1].0));
                    v.push((x0 - PAD, x1 + PAD, x0));
                }
            }
        }
        v
    }

    /// The cost of label `l` centred at `c`.
    fn cost(&self, placed: &[Box2], l: &Label, c: (f64, f64)) -> f64 {
        let (w, h) = (l.w, l.h);
        let rb = Box2 {
            x: c.0 - w / 2.0 - 2.0,
            y: c.1 - h / 2.0 - 2.0,
            w: w + 4.0,
            h: h + 4.0,
        };
        let count = |v: &[Box2]| v.iter().filter(|b| hits(b, &rb)).count() as f64;
        let mut k = 1000.0 * count(&self.cards);
        k += 300.0
            * self
                .junctions
                .iter()
                .filter(|p| {
                    p.0 > rb.x - 6.0
                        && p.0 < rb.right() + 6.0
                        && p.1 > rb.y - 6.0
                        && p.1 < rb.bottom() + 6.0
                })
                .count() as f64;
        k += 500.0 * count(&self.titles);
        // over another label: the text of both is lost, as over a card
        k += 1000.0 * count(placed);
        k += TITLE_ROW * count(&self.title_rows);
        // straddling the outline of a zone
        let inside = |z: &Box2| {
            rb.x >= z.x && rb.right() <= z.right() && rb.y >= z.y && rb.bottom() <= z.bottom()
        };
        k += ASTRIDE
            * self
                .zones
                .iter()
                .filter(|z| hits(z, &rb) && !inside(z))
                .count() as f64;
        let crossed = self
            .routes
            .iter()
            .filter(|(id, _)| self.foreign(l, id))
            .filter(|(_, other)| other.windows(2).any(|s| seg_hits(&rb, s[0], s[1])))
            .count();
        k += FOREIGN_LINE * crossed as f64;
        k += squeezed(l.route, c, w);
        // the label of a join names the flow they share: on that part (D-054)
        if !l.mates.is_empty() && !l.mates.iter().all(|m| on_route(m, c)) {
            k += 100.0;
        }
        k + 0.001 * ((c.0 - l.default.0).abs() + (c.1 - l.default.1).abs())
    }
}

/// Centre of each label (file coordinates).
pub fn place(
    model: &Model,
    scene: &Scene,
    boxes: &BTreeMap<String, Box2>,
    routes: &BTreeMap<String, Points>,
    sizes: &Sizes,
    groups: &BTreeMap<String, String>,
) -> BTreeMap<String, (f64, f64)> {
    let of_kind = |kind: skemo_lang::Kind| -> Vec<Box2> {
        model
            .nodes
            .iter()
            .filter(|n| {
                n.kind == kind && !(kind == skemo_lang::Kind::Zone && crate::layout::invisible(n))
            })
            .filter_map(|n| boxes.get(&n.id).copied())
            .collect()
    };
    let named = zone_titles(model, scene, boxes);
    let stage = Stage {
        cards: of_kind(skemo_lang::Kind::Card),
        zones: of_kind(skemo_lang::Kind::Zone),
        title_rows: title_rows(&named, boxes),
        titles: named.into_iter().map(|(_, b)| b).collect(),
        junctions: crate::route::junctions(routes, groups),
        routes,
    };
    let mut placed: Vec<Box2> = Vec::new();
    let mut out = BTreeMap::new();
    for e in model.edges.iter().filter(|e| !e.label.is_empty()) {
        let Some(r) = routes.get(&e.id) else { continue };
        let Some(mut default) = mid_of_longest(r) else {
            continue;
        };
        let Some(sz) = sizes.labels.get(&e.id) else {
            out.insert(e.id.clone(), default);
            continue;
        };
        let (w, h) = (sz.w, sz.h);
        if said_by_mate(model, groups, e) {
            continue;
        }
        let joined = mates_of(model, groups, e);
        if let Some(spot) = joined_spot(routes, e, &joined, w) {
            default = spot;
        }
        let mates: Vec<&Points> = joined
            .iter()
            .filter(|id| **id != e.id)
            .filter_map(|id| routes.get(*id))
            .collect();
        let l = Label {
            id: &e.id,
            joined,
            route: r,
            mates,
            default,
            w,
            h,
        };
        let (cands, mut extra) = candidates(r, default, w, h);
        // the places clear of what stands round the line, found exactly
        extra.extend(clear_spots(r, (w, h), &|rows, all| {
            stage.around(&placed, &l, rows, all)
        }));
        // the extra places win only where they avoid something
        let best = cands
            .into_iter()
            .map(|c| (c, stage.cost(&placed, &l, c)))
            .chain(
                extra
                    .into_iter()
                    .map(|c| (c, stage.cost(&placed, &l, c) + 1.0)),
            )
            .min_by(|a, b| a.1.total_cmp(&b.1));
        if crate::trace::wanted(&e.id) {
            eprintln!(
                "trace label {}: {w:.0}x{h:.0} at {best:?} (the middle of its longest segment: {default:?}, cost {:.1})",
                e.id,
                stage.cost(&placed, &l, default)
            );
        }
        let best = best.map_or(default, |(c, _)| c);
        placed.push(Box2 {
            x: best.0 - w / 2.0,
            y: best.1 - h / 2.0,
            w,
            h,
        });
        out.insert(e.id.clone(), best);
    }
    out
}

/// Is the label of `e` already said by an arrow of its join written before it, with the same
/// text and the same steps? The flow they share is named once, on the part they share (D-054).
fn said_by_mate(model: &Model, groups: &BTreeMap<String, String>, e: &skemo_lang::Edge) -> bool {
    let Some(g) = groups.get(&e.id) else {
        return false;
    };
    model.edges.iter().take_while(|f| f.id != e.id).any(|f| {
        groups.get(&f.id) == Some(g) && f.label == e.label && f.step == e.step && f.anim == e.anim
    })
}

/// The arrows of `e`'s group (a join, a fork, D-054, D-093), `e` included; empty when it is in
/// none. A label they share is written once, on the part they share.
fn mates_of<'m>(
    model: &'m Model,
    groups: &BTreeMap<String, String>,
    e: &skemo_lang::Edge,
) -> Vec<&'m str> {
    let Some(g) = groups.get(&e.id) else {
        return Vec::new();
    };
    model
        .edges
        .iter()
        // an arrow without a label runs under the one its group shows
        .filter(|f| groups.get(&f.id) == Some(g) && (f.label == e.label || f.label.is_empty()))
        .map(|f| f.id.as_str())
        .collect()
}

/// The label of arrows joined into one card (a named join, D-051) names their common flow
/// (D-054): it starts from the middle of the longest horizontal segment of the part they share,
/// up to the card, long enough for it (8 px of line on each side), else of its longest shared
/// segment. `None` for other arrows, or when they share nothing.
fn joined_spot(
    routes: &BTreeMap<String, Points>,
    e: &skemo_lang::Edge,
    joined: &[&str],
    w: f64,
) -> Option<(f64, f64)> {
    let r = routes.get(&e.id)?;
    let mates: Vec<&Points> = joined
        .iter()
        .filter(|id| **id != e.id)
        .filter_map(|id| routes.get(*id))
        .collect();
    // the shared part: the segments that lie on every other arrow of the join
    let on_all = |p: (f64, f64)| !mates.is_empty() && mates.iter().all(|m| on_route(m, p));
    let shared: Vec<&[(f64, f64)]> = r
        .windows(2)
        .filter(|s| on_all(s[0]) && on_all(s[1]))
        .collect();
    let len = |s: &&[(f64, f64)]| (s[1].0 - s[0].0).abs() + (s[1].1 - s[0].1).abs();
    let mid = |s: &[(f64, f64)]| ((s[0].0 + s[1].0) / 2.0, (s[0].1 + s[1].1) / 2.0);
    // a horizontal segment long enough for the text, else the longest shared segment
    shared
        .iter()
        .filter(|s| (s[0].1 - s[1].1).abs() < 1e-9 && len(s) >= w + 16.0)
        .max_by(|a, b| len(a).total_cmp(&len(b)))
        .or_else(|| shared.iter().max_by(|a, b| len(a).total_cmp(&len(b))))
        .map(|s| mid(s))
}

/// Does point `p` lie on the polyline `r` (within 0.5 px)?
fn on_route(r: &Points, p: (f64, f64)) -> bool {
    r.windows(2).any(|s| {
        let (a, b) = (s[0], s[1]);
        let within = |v: f64, x: f64, y: f64| v >= x.min(y) - 0.5 && v <= x.max(y) + 0.5;
        within(p.0, a.0, b.0)
            && within(p.1, a.1, b.1)
            && ((a.0 - b.0).abs() < 0.5 || (a.1 - b.1).abs() < 0.5)
    })
}

/// Cost of a label on a zone's title row, clear of the title: tolerated, but read as a second
/// title, so any place that crosses no arrow is preferred (`FOREIGN_LINE`).
const TITLE_ROW: f64 = 40.0;

/// Cost of each other arrow under a label (once, whatever its segments there): the label hides
/// it, or reads as that arrow's (bug 24). Something hidden (the layout method's first rank), so
/// above what is only untidy: a label squeezed on a short segment (200), off a join's shared
/// part (100); under a label over a card or another label (1000), whose text is lost, where the
/// label drawn over a line stays readable. It was 50, under the outline's cost: a label slid
/// onto a parallel line rather than over a frame.
const FOREIGN_LINE: f64 = 250.0;

/// Cost of each zone whose outline a label sits astride: the label cuts the outline (the layout
/// method's first rank too), so above a label squeezed on its segment (200), and under another
/// arrow's line (`FOREIGN_LINE`), which the label would seem to name. It was 100, under the
/// squeezed label's 200: between two cards with an outline in between, the label sat on the
/// outline rather than in the room made for it beside.
const ASTRIDE: f64 = 225.0;

/// Least room kept between an arrow label and a zone's outline it does not cross (px).
pub(crate) const LABEL_ROOM: f64 = 6.0;

/// Room kept between a label and a card, a title, another label or another arrow's line (px):
/// what `place` counts as touching (2 px), and a hair for the rounding.
pub(crate) const PAD: f64 = 2.01;

/// The grid engine's lanes (`grid::lanes`): a line that would run through a label stands beside
/// it, `LANE_EDGE` from what bounds the gap (a card: the router's margin, a zone's border: clear
/// of it), the next ones every `LANE`.
pub(crate) const LANE_EDGE: f64 = 12.0;
pub(crate) const LANE: f64 = 10.0;

/// A stretch along x a label must stay out of: where it starts, where it ends, and where what
/// stands there starts (its margin left out).
pub(crate) type Wall = (f64, f64, f64);

/// The stretches along x that a label on the rows `rows` (its top and its bottom) must stay out
/// of, for the cards (`PAD` away) and the zone outlines (`LABEL_ROOM` away): each side of a zone
/// the label is level with, and the zone's whole width where the label would sit astride its top
/// or its bottom.
pub(crate) fn walls(cards: &[Box2], zones: &[Box2], rows: (f64, f64)) -> Vec<Wall> {
    let (top, bottom) = (rows.0 - 2.0, rows.1 + 2.0);
    let mut out = Vec::new();
    for b in cards.iter().filter(|b| b.y < bottom && top < b.bottom()) {
        out.push((b.x - PAD, b.right() + PAD, b.x));
    }
    for z in zones.iter().filter(|z| z.y < bottom && top < z.bottom()) {
        if top >= z.y && bottom <= z.bottom() {
            out.push((z.x - LABEL_ROOM, z.x + LABEL_ROOM, z.x));
            out.push((z.right() - LABEL_ROOM, z.right() + LABEL_ROOM, z.right()));
        } else {
            out.push((z.x - LABEL_ROOM, z.right() + LABEL_ROOM, z.x));
        }
    }
    out
}

/// The free stretches of `range` between the walls: where each starts, where it ends, and where
/// what ends it starts (the end of the range for the last one).
pub(crate) fn free_spans(range: (f64, f64), mut walls: Vec<Wall>) -> Vec<Wall> {
    walls.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut out = Vec::new();
    let mut from = range.0;
    for (lo, hi, at) in walls {
        if lo >= range.1 {
            break;
        }
        if lo > from {
            out.push((from, lo, at));
        }
        from = from.max(hi);
    }
    if from < range.1 {
        out.push((from, range.1, range.1));
    }
    out
}

/// The rows of a label `h` high centred on `cy`.
fn rows(cy: f64, h: f64) -> (f64, f64) {
    (cy - h / 2.0, cy + h / 2.0)
}

/// Where a label `h` high may stand along an upright segment from `y0` down to `y1`: every
/// `STEP / 2`, 8 px of line kept at each end; the middle of a segment too short for that.
fn heights(y0: f64, y1: f64, h: f64) -> Vec<f64> {
    let (len, need) = (y1 - y0, h + 16.0);
    if len < need {
        return vec![(y0 + y1) / 2.0];
    }
    let mut out = Vec::new();
    let mut d = need / 2.0;
    while d <= len - need / 2.0 {
        out.push(y0 + d);
        d += STEP / 2.0;
    }
    out
}

/// The label's line `y` above a level segment at `line`: the text sits over the arrow (D-049).
fn above(line: f64, h: f64) -> f64 {
    line - h / 2.0 - 3.0
}

/// The centre nearest `x` of a label `w` wide within `(a, b)` (at least `w` wide: `b - a` and
/// `a + w` may round apart, so the bounds are kept in order).
fn centre_in(x: f64, (a, b): (f64, f64), w: f64) -> f64 {
    let lo = a + w / 2.0;
    x.clamp(lo, (b - w / 2.0).max(lo))
}

/// The places of a label `w` by `h` on the route `r` that are clear of the walls `around` gives
/// for its rows (of everything, then of the cards and outlines alone), found exactly where the
/// places every `STEP` px miss a gap just wide enough: on each level segment, in each free
/// stretch between two walls, as near its middle as the stretch lets (`CLEAR` of line kept at
/// each end, else the label's room), and above it; on each upright segment, at each height, slid
/// sideways into the free stretch while it stays over its line.
fn clear_spots(
    r: &Points,
    (w, h): (f64, f64),
    around: &dyn Fn((f64, f64), bool) -> Vec<Wall>,
) -> Spots {
    let mut out = Vec::new();
    for s in r.windows(2) {
        let (p, q) = (s[0], s[1]);
        let (flat, upright) = ((p.1 - q.1).abs() < 1e-9, (p.0 - q.0).abs() < 1e-9);
        if flat && !upright {
            let (x0, x1) = (p.0.min(q.0), p.0.max(q.0));
            let mid = (x0 + x1) / 2.0;
            for (cy, ends) in [(p.1, CLEAR), (p.1, LABEL_ROOM), (above(p.1, h), 0.0)] {
                for all in [true, false] {
                    for (a, b, _) in free_spans((x0 + ends, x1 - ends), around(rows(cy, h), all)) {
                        if b - a >= w {
                            out.push((centre_in(mid, (a, b), w), cy));
                        }
                    }
                }
            }
        } else if upright && !flat {
            let reach = (p.0 - w + 4.0, p.0 + w - 4.0);
            for cy in heights(p.1.min(q.1), p.1.max(q.1), h) {
                for all in [true, false] {
                    for (a, b, _) in free_spans(reach, around(rows(cy, h), all)) {
                        if b - a >= w {
                            out.push((centre_in(p.0, (a, b), w), cy));
                        }
                    }
                }
            }
        }
    }
    out
}

/// A place where the label of an arrow may stand on its line, as `clear_spots` finds them.
pub(crate) struct Slot {
    /// The label's top and its bottom.
    pub rows: (f64, f64),
    /// The gap between two walls (cards, zone outlines) the place lies in, at those rows.
    pub gap: (f64, f64),
    /// How far the label may slide in it: its left edge at the least, its right edge at the most.
    pub slide: (f64, f64),
    /// Where the stretch starts, and where what ends it starts.
    pub ends: (f64, f64),
}

impl Slot {
    /// The room a label `w` wide lacks there (px; none or less: it fits).
    pub fn lack(&self, w: f64) -> f64 {
        w - (self.slide.1 - self.slide.0)
    }
}

const FAR: (f64, f64) = (f64::NEG_INFINITY, f64::INFINITY);

/// The places where the label (`w` by `h`) of the route `r` may stand clear of the cards and of
/// the zone outlines, on its line or above a level segment of it, wide enough or not. The places
/// are those of `clear_spots`: one wide enough here is a place found there.
pub(crate) fn slots(r: &Points, (w, h): (f64, f64), cards: &[Box2], zones: &[Box2]) -> Vec<Slot> {
    let mut out = Vec::new();
    for s in r.windows(2) {
        let (p, q) = (s[0], s[1]);
        let (flat, upright) = ((p.1 - q.1).abs() < 1e-9, (p.0 - q.0).abs() < 1e-9);
        if flat && !upright {
            let (x0, x1) = (p.0.min(q.0), p.0.max(q.0));
            for (cy, ends) in [(p.1, CLEAR), (above(p.1, h), 0.0)] {
                let range = (x0 + ends, x1 - ends);
                let rows = rows(cy, h);
                let gaps = free_spans(FAR, walls(cards, zones, rows));
                for (a, b, at) in free_spans(range, walls(cards, zones, rows)) {
                    let gap = gaps.iter().find(|g| g.0 <= a && b <= g.1);
                    out.push(Slot {
                        rows,
                        gap: gap.map_or((a, b), |g| (g.0, g.1)),
                        slide: (a, b),
                        // the segment's own end stands where the stretch stops short of it
                        ends: (a, if at == range.1 { x1 } else { at }),
                    });
                }
            }
        } else if upright && !flat {
            for cy in heights(p.1.min(q.1), p.1.max(q.1), h) {
                // the corridor the line runs in at that height
                let rows = rows(cy, h);
                let free = free_spans(FAR, walls(cards, zones, rows));
                if let Some((a, b, at)) = free.into_iter().find(|(a, b, _)| *a < p.0 && p.0 < *b) {
                    out.push(Slot {
                        rows,
                        gap: (a, b),
                        slide: (a.max(p.0 - w + 4.0), b.min(p.0 + w - 4.0)),
                        ends: (a, at),
                    });
                }
            }
        }
    }
    out
}

/// The place where the label `label` of the route `r` stands: the gap that holds it, and how
/// far it may slide there along the segment it stands on. `None` when it stands in no gap (on a
/// card, astride an outline) or on no segment of its line.
pub(crate) fn slot_at(r: &Points, label: &Box2, cards: &[Box2], zones: &[Box2]) -> Option<Slot> {
    let rows = (label.y, label.bottom());
    let (cx, cy) = (label.x + label.w / 2.0, label.y + label.h / 2.0);
    let (lo, hi, _) = free_spans(FAR, walls(cards, zones, rows))
        .into_iter()
        .find(|(a, b, _)| *a <= label.x + 0.5 && label.right() <= *b + 0.5)?;
    for s in r.windows(2) {
        let (p, q) = (s[0], s[1]);
        let (flat, upright) = ((p.1 - q.1).abs() < 1e-9, (p.0 - q.0).abs() < 1e-9);
        let slide = if flat && !upright {
            let (x0, x1) = (p.0.min(q.0), p.0.max(q.0));
            let on = (cy - p.1).abs() < 0.5;
            if !(on || (cy - above(p.1, label.h)).abs() < 0.5)
                || label.x < x0 - 0.5
                || label.right() > x1 + 0.5
            {
                continue;
            }
            // as `slots`, and the label where it stands (a place nearer the segment's end)
            let ends = if on { CLEAR } else { 0.0 };
            ((x0 + ends).min(label.x), (x1 - ends).max(label.right()))
        } else if upright && !flat {
            let (y0, y1) = (p.1.min(q.1), p.1.max(q.1));
            if cy < y0 || cy > y1 || (cx - p.0).abs() > label.w / 2.0 {
                continue;
            }
            (p.0 - label.w + 4.0, p.0 + label.w - 4.0)
        } else {
            continue;
        };
        return Some(Slot {
            rows,
            gap: (lo, hi),
            slide: (slide.0.max(lo), slide.1.min(hi)),
            ends: (lo, hi),
        });
    }
    None
}

/// Free line wanted on each side of a label set on a horizontal segment (px).
const CLEAR: f64 = 20.0;

/// Each zone's title row (its title and tag), across the zone's whole width.
fn title_rows(named: &[(String, Box2)], boxes: &BTreeMap<String, Box2>) -> Vec<Box2> {
    let mut rows: BTreeMap<&str, (f64, f64)> = BTreeMap::new();
    for (z, t) in named {
        let r = rows.entry(z.as_str()).or_insert((t.y, t.bottom()));
        *r = (r.0.min(t.y), r.1.max(t.bottom()));
    }
    rows.into_iter()
        .filter_map(|(z, (y0, y1))| {
            boxes.get(z).map(|zb| Box2 {
                x: zb.x,
                y: y0,
                w: zb.w,
                h: y1 - y0,
            })
        })
        .collect()
}

/// Label centres.
type Spots = Vec<(f64, f64)>;

/// How far beside a segment too short for its label the label may go (px from the line).
const REACH: f64 = 48.0;

/// Step between the extra places along a segment (px).
const STEP: f64 = 8.0;

/// Candidate centres of a label: fractions of each segment long enough to carry it, and above
/// each horizontal segment too short to (D-049: a hand drawing writes `yes` over a short arrow
/// rather than cutting it in two stubs); then extra places every `STEP` px along each segment
/// long enough, for a free spot between two obstacles that no fraction hits, and beside each
/// segment too short, up to `REACH` px away (a label wider than the gap between two cards).
fn candidates(r: &Points, default: (f64, f64), w: f64, h: f64) -> (Spots, Spots) {
    let mut cands = vec![default];
    let mut extra = Vec::new();
    for s in r.windows(2) {
        let (p, q) = (s[0], s[1]);
        let len = (q.0 - p.0).abs() + (q.1 - p.1).abs();
        let flat = (p.1 - q.1).abs() < 1e-9;
        if flat && len < w + 2.0 * CLEAR && len >= w {
            cands.push(((p.0 + q.0) / 2.0, p.1 - h / 2.0 - 3.0));
        }
        let need = if flat { w } else { h } + 16.0;
        if len < need {
            // too short to carry it (two cards close together): beside it, a little further each
            // time, for a place clear of the cards
            let mid = ((p.0 + q.0) / 2.0, (p.1 + q.1) / 2.0);
            // on the segment itself: a short step between two cards side by side, whose label
            // then sits in the middle of the gap, over the step (D-093)
            extra.push(mid);
            if !flat {
                // a wide label on a short upright step: slid sideways, still over its line, clear
                // of a parallel line beside it (bug 24)
                let mut dx = STEP;
                while dx < w / 2.0 - 4.0 {
                    extra.extend([(mid.0 - dx, mid.1), (mid.0 + dx, mid.1)]);
                    dx += STEP;
                }
            }
            let mut d = if flat { h } else { w } / 2.0 + 3.0;
            while d <= REACH {
                for sign in [-1.0, 1.0] {
                    extra.push(if flat {
                        (mid.0, mid.1 + sign * d)
                    } else {
                        (mid.0 + sign * d, mid.1)
                    });
                }
                d += STEP;
            }
            continue;
        }
        for t in [0.5, 1.0 / 3.0, 2.0 / 3.0, 0.25, 0.75] {
            cands.push((p.0 + (q.0 - p.0) * t, p.1 + (q.1 - p.1) * t));
        }
        if !flat {
            // a wide label on an upright line between two cards close together: slid sideways,
            // still over the line, to sit in the gap between them
            for t in [0.5, 1.0 / 3.0, 2.0 / 3.0] {
                let y = p.1 + (q.1 - p.1) * t;
                let mut dx = STEP;
                while dx < w / 2.0 - 4.0 {
                    extra.extend([(p.0 - dx, y), (p.0 + dx, y)]);
                    dx += STEP;
                }
            }
        }
        let mut d = need / 2.0;
        while d <= len - need / 2.0 {
            let t = d / len;
            extra.push((p.0 + (q.0 - p.0) * t, p.1 + (q.1 - p.1) * t));
            d += STEP;
        }
    }
    (cands, extra)
}

/// Cost of a label set on a horizontal segment that leaves less than `CLEAR` of line on a side.
fn squeezed(r: &Points, c: (f64, f64), w: f64) -> f64 {
    let short = r.windows(2).any(|s| {
        let (p, q) = (s[0], s[1]);
        let (x0, x1) = (p.0.min(q.0), p.0.max(q.0));
        (p.1 - q.1).abs() < 1e-9
            && (c.1 - p.1).abs() < 0.5
            && c.0 >= x0
            && c.0 <= x1
            && (c.0 - w / 2.0 - x0).min(x1 - c.0 - w / 2.0) < CLEAR
    });
    if short { 200.0 } else { 0.0 }
}
