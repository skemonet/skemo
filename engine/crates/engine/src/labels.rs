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
        let zt = geometry::zone_title(scene, z, (&n.title, n.tag.as_deref()), b.w, b.h);
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
                h: lh + 4.0,
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

/// Centre of each label (file coordinates).
pub fn place(
    model: &Model,
    scene: &Scene,
    boxes: &BTreeMap<String, Box2>,
    routes: &BTreeMap<String, Points>,
    sizes: &Sizes,
) -> BTreeMap<String, (f64, f64)> {
    let cards: Vec<Box2> = model
        .nodes
        .iter()
        .filter(|n| n.kind == skemo_lang::Kind::Card)
        .filter_map(|n| boxes.get(&n.id).copied())
        .collect();
    let named = zone_titles(model, scene, boxes);
    let title_rows = title_rows(&named, boxes);
    let titles: Vec<Box2> = named.into_iter().map(|(_, b)| b).collect();
    // an invisible group (a stack, an untitled bare zone) has no outline to keep clear of
    let zones: Vec<Box2> = model
        .nodes
        .iter()
        .filter(|n| n.kind == skemo_lang::Kind::Zone && !crate::layout::invisible(n))
        .filter_map(|n| boxes.get(&n.id).copied())
        .collect();
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
        if said_by_mate(model, e) {
            continue;
        }
        if let Some(spot) = joined_spot(model, routes, e, w) {
            default = spot;
        }
        let joined: Vec<&str> = model
            .edges
            .iter()
            .filter(|f| crate::route::fan_in(e) && f.to == e.to && f.join == e.join)
            .map(|f| f.id.as_str())
            .collect();
        let mates: Vec<&Points> = joined
            .iter()
            .filter(|id| **id != e.id)
            .filter_map(|id| routes.get(*id))
            .collect();
        let (cands, extra) = candidates(r, default, w, h);
        let cost = |c: (f64, f64)| -> f64 {
            let rb = Box2 {
                x: c.0 - w / 2.0 - 2.0,
                y: c.1 - h / 2.0 - 2.0,
                w: w + 4.0,
                h: h + 4.0,
            };
            let mut k = 0.0;
            k += 1000.0 * cards.iter().filter(|b| hits(b, &rb)).count() as f64;
            k += 500.0 * titles.iter().filter(|b| hits(b, &rb)).count() as f64;
            k += 300.0 * placed.iter().filter(|b| hits(b, &rb)).count() as f64;
            k += TITLE_ROW * title_rows.iter().filter(|b| hits(b, &rb)).count() as f64;
            // straddling the outline of a zone
            let inside = |z: &Box2| {
                rb.x >= z.x && rb.right() <= z.right() && rb.y >= z.y && rb.bottom() <= z.bottom()
            };
            k += 100.0 * zones.iter().filter(|z| hits(z, &rb) && !inside(z)).count() as f64;
            for (id, other) in routes {
                // the arrows of its join run under the label they share (D-054)
                if *id == e.id || joined.contains(&id.as_str()) {
                    continue;
                }
                k += 50.0
                    * other
                        .windows(2)
                        .filter(|s| seg_hits(&rb, s[0], s[1]))
                        .count() as f64;
            }
            k += squeezed(r, c, w);
            // the label of a join names the flow they share: on that part (D-054)
            if !mates.is_empty() && !mates.iter().all(|m| on_route(m, c)) {
                k += 100.0;
            }
            k + 0.001 * ((c.0 - default.0).abs() + (c.1 - default.1).abs())
        };
        // the extra places win only where they avoid something
        let best = cands
            .into_iter()
            .map(|c| (c, cost(c)))
            .chain(extra.into_iter().map(|c| (c, cost(c) + 1.0)))
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map_or(default, |(c, _)| c);
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
fn said_by_mate(model: &Model, e: &skemo_lang::Edge) -> bool {
    crate::route::fan_in(e)
        && model.edges.iter().take_while(|f| f.id != e.id).any(|f| {
            f.to == e.to
                && f.join == e.join
                && f.label == e.label
                && f.step == e.step
                && f.anim == e.anim
        })
}

/// The label of arrows joined into one card (a named join, D-051) names their common flow
/// (D-054): it starts from the middle of the longest horizontal segment of the part they share,
/// up to the card, long enough for it (8 px of line on each side), else of its longest shared
/// segment. `None` for other arrows, or when they share nothing.
fn joined_spot(
    model: &Model,
    routes: &BTreeMap<String, Points>,
    e: &skemo_lang::Edge,
    w: f64,
) -> Option<(f64, f64)> {
    if !crate::route::fan_in(e) {
        return None;
    }
    let r = routes.get(&e.id)?;
    let mates: Vec<&Points> = model
        .edges
        .iter()
        .filter(|f| f.id != e.id && f.to == e.to && f.join == e.join)
        .filter_map(|f| routes.get(&f.id))
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
/// title, so any place that crosses no arrow is preferred (an arrow crossed costs 50).
const TITLE_ROW: f64 = 40.0;

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
