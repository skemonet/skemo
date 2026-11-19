//! Quality properties of a laid-out diagram (the layout method, phase 0: meta repository,
//! `docs/plans/layout-method.md` §2). Each property is true or false for one element, pair or line
//! of the drawing; `defects` lists what breaks each one, so that a corpus can be counted (the
//! ledger, `tests/ledger.rs`) and a change refused when a count rises. Never a weighted score.
//!
//! Graph layouts only for now: timelines, friezes and mind maps place their cards by other rules.

use crate::diagram::{Diagram, Options};
use crate::layout::{Box2, invisible};
use crate::tidy::geom::Axes;
use skemo_lang::{Kind, Model};
use std::collections::BTreeMap;

/// The properties, by family (the method's §2), in the order of the ledger's columns.
pub const PROPERTIES: [&str; 36] = [
    // wrong drawing: an arrow that leaves or reaches the wrong place
    "end-off-card",
    // hidden content
    "zone-overlap",
    "card-outside-zone",
    "card-overlap",
    "zone-title-overflow",
    "text-overflow",
    "missing-glyph",
    "wide-docked-narrow",
    "through-card",
    "through-title",
    "label-overlap",
    "label-on-frame",
    "through-label",
    // stability (D-031): what an edit moves that it should not
    "pin-moves",
    "move-moves",
    // ports and crossings
    "slant",
    "overlay",
    "close-lines",
    "end-crossing",
    "sibling-crossing",
    "through-zone",
    "detour",
    "hugging",
    // bends (the manual method's §6)
    "staircase",
    "short-tail",
    "port-jog",
    // alignment and order
    "row-top",
    "writing-order",
    "zone-order",
    "note-far",
    "sprawl",
    "sparse",
    "uneven-gaps",
    // text
    "label-markup",
    "needless-wrap",
    "long-line",
];

/// A note stands beside the card it annotates, 24 px away (FORMAT, D-048 R7): farther than this
/// between their boxes, it reads as a card of its own and its link runs across the drawing (bug 25).
const NOTE_FAR: f64 = 48.0;

/// Widest card counted as readable text (px): past it a paragraph runs on one line across the image.
const MEASURE: f64 = 600.0;
/// Least share of a drawing its cards cover (an eighth): below, it reads as scattered.
const SPARSE: f64 = 0.125;
/// Cards outside the flow may leave the image this many times the area of the rest, no more.
const SPRAWL: f64 = 1.3;

/// Smallest overlap counted (px): less is rounding.
const TOUCH: f64 = 1.0;
/// The margin a line keeps from a box it runs along (the user's 10 px rule).
const HUG: f64 = 10.0;
/// Shortest stretch of a line along a box that counts as running along it (px).
const HUG_LEN: f64 = 20.0;
/// A route is a detour beyond `DETOUR_K` times the distance between its ends plus `DETOUR_PX`.
/// Fixed once, never tuned (the method's §2).
const DETOUR_K: f64 = 2.0;
const DETOUR_PX: f64 = 200.0;
/// Two lines side by side closer than this read as one thick line (px; the router spaces the
/// ports of one side 8 px apart).
const CLOSE: f64 = 6.0;
/// Shortest stretch two lines run side by side that counts (px).
const CLOSE_LEN: f64 = 8.0;
/// Shortest last stretch that shows an arrow's direction behind its head (px).
const TAIL: f64 = 24.0;
/// A Z whose middle stretch is shorter than this is a jog between two ports nearly in line (px).
const JOG: f64 = 40.0;

/// What breaks each property of a laid-out diagram: property → the elements, pairs or lines
/// concerned (one string each). Every property of `PROPERTIES` but the stability pair is present,
/// an empty list when it holds; `all_defects` adds the pair (two more layouts per card).
pub fn defects(d: &Diagram) -> BTreeMap<&'static str, Vec<String>> {
    let mut out: BTreeMap<&'static str, Vec<String>> =
        PROPERTIES.iter().map(|p| (*p, Vec::new())).collect();
    if !crate::diagram::automatic(&d.model) {
        return out;
    }
    let mut put = |p: &'static str, v: Vec<String>| {
        out.insert(p, v);
    };
    put("end-off-card", end_off_card(d));
    put("zone-overlap", zone_overlap(d));
    put("card-outside-zone", card_outside_zone(d));
    put("card-overlap", card_overlap(d));
    put("zone-title-overflow", zone_title_overflow(d));
    put("text-overflow", text_overflow(d));
    put("missing-glyph", missing_glyph(&d.model, &d.scene));
    put("wide-docked-narrow", wide_docked_narrow(d));
    put("through-card", through_card(d));
    put("through-title", through_title(d));
    put("label-overlap", label_overlap(d));
    put("label-on-frame", label_on_frame(d));
    put("through-label", through_label(d));
    put("slant", slant(d));
    put("overlay", overlay(d));
    put("close-lines", close_lines(d));
    put("end-crossing", end_crossing(d));
    put("sibling-crossing", sibling_crossing(d));
    put("through-zone", through_zone(d));
    put("detour", detour(d));
    put("hugging", hugging(d));
    put("staircase", staircase(d));
    put("short-tail", short_tail(d));
    put("port-jog", port_jog(d));
    put("row-top", row_top(d));
    put("writing-order", writing_order(d));
    put("zone-order", zone_order(d));
    put("note-far", note_far(d));
    put("sprawl", sprawl(d));
    put("sparse", sparse(d));
    put("uneven-gaps", uneven_gaps(d));
    put("label-markup", label_markup(d));
    put("needless-wrap", needless_wrap(d));
    put("long-line", long_line(d));
    out
}

/// Visible zones (not a stack, not a bare zone without title).
fn zones(d: &Diagram) -> Vec<(&str, Box2)> {
    d.model
        .nodes
        .iter()
        .filter(|n| n.kind == Kind::Zone && !invisible(n))
        .filter_map(|n| d.boxes.get(&n.id).map(|b| (n.id.as_str(), *b)))
        .collect()
}

fn cards(d: &Diagram) -> Vec<(&str, Box2)> {
    d.model
        .nodes
        .iter()
        .filter(|n| n.kind == Kind::Card)
        .filter_map(|n| d.boxes.get(&n.id).map(|b| (n.id.as_str(), *b)))
        .collect()
}

fn nested(m: &Model, inner: &str, outer: &str) -> bool {
    m.ancestors(inner).iter().any(|n| n.id == outer)
}

/// Area of the intersection, as its two sides (0 when apart).
fn overlap(a: &Box2, b: &Box2) -> (f64, f64) {
    (
        (a.right().min(b.right()) - a.x.max(b.x)).max(0.0),
        (a.bottom().min(b.bottom()) - a.y.max(b.y)).max(0.0),
    )
}

fn meet(a: &Box2, b: &Box2) -> bool {
    let (w, h) = overlap(a, b);
    w > TOUCH && h > TOUCH
}

/// Do two cards overlap? They stand on whole pixels (D-100): one pixel over is no rounding (bug 28,
/// two cards of a column one pixel into each other).
fn cards_meet(a: &Box2, b: &Box2) -> bool {
    let (w, h) = overlap(a, b);
    w > 0.5 && h > 0.5
}

fn zone_overlap(d: &Diagram) -> Vec<String> {
    let z = zones(d);
    let mut out = Vec::new();
    for (i, (a, ab)) in z.iter().enumerate() {
        for (b, bb) in &z[i + 1..] {
            if !nested(&d.model, a, b) && !nested(&d.model, b, a) && meet(ab, bb) {
                out.push(format!("{a} {b}"));
            }
        }
    }
    out
}

fn card_outside_zone(d: &Diagram) -> Vec<String> {
    let mut out = Vec::new();
    for (c, cb) in cards(d) {
        for z in d.model.ancestors(c) {
            if invisible(z) {
                continue;
            }
            let Some(zb) = d.boxes.get(&z.id) else {
                continue;
            };
            let inside = cb.x >= zb.x - 0.5
                && cb.y >= zb.y - 0.5
                && cb.right() <= zb.right() + 0.5
                && cb.bottom() <= zb.bottom() + 0.5;
            if !inside {
                out.push(format!("{c} {}", z.id));
            }
        }
    }
    out
}

fn card_overlap(d: &Diagram) -> Vec<String> {
    let c = cards(d);
    let mut out = Vec::new();
    for (i, (a, ab)) in c.iter().enumerate() {
        for (b, bb) in &c[i + 1..] {
            if cards_meet(ab, bb) {
                out.push(format!("{a} {b}"));
            }
        }
        for (z, zb) in zones(d) {
            if !nested(&d.model, a, z) && meet(ab, &zb) {
                out.push(format!("{a} {z}"));
            }
        }
    }
    let titles = crate::labels::zone_titles(&d.model, &d.scene, &d.boxes);
    for (a, ab) in &c {
        for (z, tb) in &titles {
            if meet(ab, tb) {
                out.push(format!("{a} title:{z}"));
            }
        }
    }
    out
}

fn zone_title_overflow(d: &Diagram) -> Vec<String> {
    let mut out = Vec::new();
    for (id, b) in zones(d) {
        let (Some(n), Some(z)) = (d.model.node(id), d.scene.zones.get(id)) else {
            continue;
        };
        if n.title.is_empty() {
            continue;
        }
        let text = d.scene.zone_text(id, &n.title);
        let t = crate::geometry::zone_title(&d.scene, z, (text, n.tag.as_deref()), b.w, b.h);
        // the pill, and the description lines under the title (bug 16)
        let right = t.subs.iter().map(|r| r.x + r.width).fold(
            t.tag.as_ref().map_or(t.span.1, |p| t.span.1.max(p.x + p.w)),
            f64::max,
        );
        // as far from the frame's right edge as the title starts from its left edge (bug 23:
        // the last letter touched the frame)
        let past = right > b.w - t.span.0.max(0.0) + 0.5;
        // `.tag-right`: the pill at the row's end runs over the title (the user, on the POC's
        // "Comment marche ACM": "OBSE" left of `open-cluster-management-observability`)
        let under = z.tag_right && t.tag.as_ref().is_some_and(|p| p.x < t.span.1 + 0.5);
        if past || under {
            out.push(id.to_string());
        }
    }
    out
}

/// Characters the default font does not draw (drawn blank), in the titles, lines and labels:
/// `<id> <characters>`.
pub(crate) fn missing_glyph(model: &skemo_lang::Model, scene: &crate::scene::Scene) -> Vec<String> {
    let style = crate::measure::text_style(scene.style(scene.stage));
    let mut texts: Vec<(&str, &str)> = Vec::new();
    for n in &model.nodes {
        texts.push((&n.id, &n.title));
        for li in &n.li {
            texts.push((&n.id, &li.text));
        }
    }
    for e in &model.edges {
        texts.push((&e.id, &e.label));
    }
    let mut out = Vec::new();
    for (id, t) in texts {
        let mut missing: Vec<char> = t
            .chars()
            .filter(|c| !c.is_whitespace() && !style.draws(*c))
            .collect();
        missing.dedup();
        if !missing.is_empty() {
            out.push(format!("{id} {}", missing.iter().collect::<String>()));
        }
    }
    out
}

/// A `.wide` element docked on a side (`at="s"`…) narrower than the drawing it spans.
fn wide_docked_narrow(d: &Diagram) -> Vec<String> {
    let rest: Vec<Box2> = d
        .model
        .nodes
        .iter()
        .filter(|n| n.parent.is_none() && n.dock.is_none())
        .filter_map(|n| d.boxes.get(&n.id).copied())
        .collect();
    let (x0, x1) = rest
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), r| {
            (a.min(r.x), b.max(r.right()))
        });
    let (y0, y1) = rest
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), r| {
            (a.min(r.y), b.max(r.bottom()))
        });
    let mut out = Vec::new();
    for n in d.model.nodes.iter().filter(|n| {
        n.classes.iter().any(|c| c == "wide") && n.dock.as_deref().is_some_and(|s| !s.is_empty())
    }) {
        let Some(b) = d.boxes.get(&n.id) else {
            continue;
        };
        let across = matches!(n.dock.as_deref(), Some("e" | "w"));
        let (have, need) = if across {
            (b.h, y1 - y0)
        } else {
            (b.w, x1 - x0)
        };
        if need.is_finite() && have < need - 1.0 {
            out.push(n.id.clone());
        }
    }
    out
}

/// Is `p` within box `b` (its border included)?
fn inside(b: &Box2, p: (f64, f64)) -> bool {
    p.0 >= b.x - 0.5 && p.0 <= b.x + b.w + 0.5 && p.1 >= b.y - 0.5 && p.1 <= b.y + b.h + 0.5
}

fn segments(r: &[(f64, f64)]) -> impl Iterator<Item = ((f64, f64), (f64, f64))> + '_ {
    r.windows(2).map(|w| (w[0], w[1]))
}

fn slant(d: &Diagram) -> Vec<String> {
    // the turn of an arrow through a tunnel inside a card it crosses is hidden (D-116)
    let hidden = |id: &str, a: (f64, f64), b: (f64, f64)| {
        d.model.edges.iter().find(|e| e.id == id).is_some_and(|e| {
            crate::tunnel::cuts(&d.model, e)
                .iter()
                .filter_map(|c| d.boxes.get(*c))
                .any(|bx| inside(bx, a) && inside(bx, b))
        })
    };
    d.routes
        .iter()
        .filter(|(id, r)| {
            segments(r).any(|(a, b)| {
                (a.0 - b.0).abs() > 1.0 && (a.1 - b.1).abs() > 1.0 && !hidden(id, a, b)
            })
        })
        .map(|(id, _)| id.clone())
        .collect()
}

/// Crossings between two orthogonal polylines, collinear points merged first (a crossing where a
/// stub ends lies between two segments).
fn crossings(a: &[(f64, f64)], b: &[(f64, f64)]) -> usize {
    let merged = |p: &[(f64, f64)]| {
        let mut v: Vec<(f64, f64)> = Vec::new();
        for &q in p {
            if v.len() >= 2 {
                let (u, w) = (v[v.len() - 2], v[v.len() - 1]);
                let line = ((u.0 - w.0).abs() < 0.5 && (w.0 - q.0).abs() < 0.5)
                    || ((u.1 - w.1).abs() < 0.5 && (w.1 - q.1).abs() < 0.5);
                if line {
                    v.pop();
                }
            }
            v.push(q);
        }
        v
    };
    let (a, b) = (merged(a), merged(b));
    let mut n = 0;
    for (p, q) in segments(&a) {
        for (r, t) in segments(&b) {
            let h1 = (p.1 - q.1).abs() < 0.5;
            let h2 = (r.1 - t.1).abs() < 0.5;
            if h1 == h2 {
                continue;
            }
            let (ha, hb, va, vb) = if h1 { (p, q, r, t) } else { (r, t, p, q) };
            let inside_h = va.0 > ha.0.min(hb.0) + 0.5 && va.0 < ha.0.max(hb.0) - 0.5;
            let inside_v = ha.1 > va.1.min(vb.1) + 0.5 && ha.1 < va.1.max(vb.1) - 0.5;
            if inside_h && inside_v {
                n += 1;
            }
        }
    }
    n
}

/// Two arrows of one card (sharing an end) that cross: their ports could be swapped.
fn end_crossing(d: &Diagram) -> Vec<String> {
    let e = &d.model.edges;
    let mut out = Vec::new();
    for (i, a) in e.iter().enumerate() {
        for b in &e[i + 1..] {
            let shared = [&a.from, &a.to]
                .iter()
                .any(|x| **x == b.from || **x == b.to);
            if !shared
                || d.edge_groups
                    .get(&a.id)
                    .is_some_and(|g| d.edge_groups.get(&b.id) == Some(g))
            {
                continue;
            }
            let (Some(ra), Some(rb)) = (d.routes.get(&a.id), d.routes.get(&b.id)) else {
                continue;
            };
            if crossings(ra, rb) > 0 {
                out.push(format!("{} {}", a.id, b.id));
            }
        }
    }
    out
}

/// Two arrows that cross, into (or out of) two sibling cards of one row, in the opposite order of
/// their other ends: swapping the siblings would remove the crossing.
fn sibling_crossing(d: &Diagram) -> Vec<String> {
    let ax = Axes::of(&d.model);
    let e = &d.model.edges;
    let mut out = Vec::new();
    for (i, a) in e.iter().enumerate() {
        for b in &e[i + 1..] {
            for (ta, tb, sa, sb) in [
                (&a.to, &b.to, &a.from, &b.from),
                (&a.from, &b.from, &a.to, &b.to),
            ] {
                if ta == tb || sa == sb {
                    continue;
                }
                let (Some(na), Some(nb)) = (d.model.node(ta), d.model.node(tb)) else {
                    continue;
                };
                if na.parent != nb.parent || na.kind != Kind::Card || nb.kind != Kind::Card {
                    continue;
                }
                let (Some(ba), Some(bb), Some(xa), Some(xb)) = (
                    d.boxes.get(ta),
                    d.boxes.get(tb),
                    d.boxes.get(sa),
                    d.boxes.get(sb),
                ) else {
                    continue;
                };
                if !ax.same_row(ba, bb) {
                    continue;
                }
                let order = |p: &Box2, q: &Box2| ax.mid(p) < ax.mid(q);
                if order(ba, bb) == order(xa, xb) || (ax.mid(xa) - ax.mid(xb)).abs() < 1.0 {
                    continue;
                }
                let (Some(ra), Some(rb)) = (d.routes.get(&a.id), d.routes.get(&b.id)) else {
                    continue;
                };
                if crossings(ra, rb) > 0 {
                    out.push(format!("{} {}", a.id, b.id));
                }
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

fn detour(d: &Diagram) -> Vec<String> {
    d.routes
        .iter()
        .filter(|(_, r)| {
            let (Some(a), Some(b)) = (r.first(), r.last()) else {
                return false;
            };
            let len: f64 = segments(r)
                .map(|(p, q)| (p.0 - q.0).abs() + (p.1 - q.1).abs())
                .sum();
            let direct = (a.0 - b.0).abs() + (a.1 - b.1).abs();
            len > DETOUR_K * direct + DETOUR_PX
        })
        .map(|(id, _)| id.clone())
        .collect()
}

/// A line running along a card (not one of its own ends) or a zone frame, closer than 10 px.
fn hugging(d: &Diagram) -> Vec<String> {
    let edges: BTreeMap<&str, (&str, &str)> = d
        .model
        .edges
        .iter()
        .map(|e| (e.id.as_str(), (e.from.as_str(), e.to.as_str())))
        .collect();
    let boxes: Vec<(&str, Box2, bool)> = cards(d)
        .into_iter()
        .map(|(id, b)| (id, b, true))
        .chain(zones(d).into_iter().map(|(id, b)| (id, b, false)))
        .collect();
    let mut out = Vec::new();
    for (id, r) in &d.routes {
        let ends = edges.get(id.as_str()).copied().unwrap_or(("", ""));
        let mut hit: Vec<&str> = Vec::new();
        for (p, q) in segments(r) {
            let horizontal = (p.1 - q.1).abs() < 0.5;
            let vertical = (p.0 - q.0).abs() < 0.5;
            if horizontal == vertical {
                continue;
            }
            for (b, bb, is_card) in &boxes {
                if *b == ends.0 || *b == ends.1 || hit.contains(b) {
                    continue;
                }
                let (pos, lo, hi, e0, e1, s0, s1) = if horizontal {
                    (
                        p.1,
                        p.0.min(q.0),
                        p.0.max(q.0),
                        bb.y,
                        bb.bottom(),
                        bb.x,
                        bb.right(),
                    )
                } else {
                    (
                        p.0,
                        p.1.min(q.1),
                        p.1.max(q.1),
                        bb.x,
                        bb.right(),
                        bb.y,
                        bb.bottom(),
                    )
                };
                let along = hi.min(s1) - lo.max(s0);
                if along < HUG_LEN {
                    continue;
                }
                // distance to the nearer edge: outside a card, either side of a zone frame
                let dist = (pos - e0).abs().min((pos - e1).abs());
                let inside = pos > e0 && pos < e1;
                if dist < HUG && dist > 0.0 - 0.5 && (!*is_card || !inside) {
                    hit.push(b);
                }
            }
        }
        out.extend(hit.into_iter().map(|b| format!("{id} {b}")));
    }
    out
}

/// Cards that count in a row: not notes, legends or plain text, not docked, not pinned.
fn in_rows(d: &Diagram, id: &str) -> bool {
    d.model.node(id).is_some_and(|n| {
        n.kind == Kind::Card
            && n.dock.is_none()
            && n.at.is_none()
            && !n
                .classes
                .iter()
                .any(|c| matches!(c.as_str(), "note" | "legend" | "text" | "info"))
    })
}

/// Rows of the sibling cards of each container, in writing order within a row.
fn rows(d: &Diagram) -> Vec<Vec<(usize, &str, Box2)>> {
    let ax = Axes::of(&d.model);
    let mut by_parent: BTreeMap<Option<&str>, Vec<(usize, &str, Box2)>> = BTreeMap::new();
    for (k, n) in d.model.nodes.iter().enumerate() {
        if !in_rows(d, &n.id) {
            continue;
        }
        if let Some(b) = d.boxes.get(&n.id) {
            by_parent
                .entry(n.parent.as_deref())
                .or_default()
                .push((k, n.id.as_str(), *b));
        }
    }
    let mut out = Vec::new();
    for sibs in by_parent.into_values() {
        let mut rows: Vec<Vec<(usize, &str, Box2)>> = Vec::new();
        for s in sibs {
            match rows
                .iter_mut()
                .find(|r| r.iter().any(|o| ax.same_row(&o.2, &s.2)))
            {
                Some(r) => r.push(s),
                None => rows.push(vec![s]),
            }
        }
        out.extend(rows);
    }
    out
}

/// Cards of a row whose start along the flow differs from the row's first (dir down: tops).
fn row_top(d: &Diagram) -> Vec<String> {
    let ax = Axes::of(&d.model);
    let mut out = Vec::new();
    for row in rows(d) {
        let start = |b: &Box2| {
            if ax.reverse {
                ax.flow(b).1
            } else {
                ax.flow(b).0
            }
        };
        let first = row.iter().map(|s| start(&s.2)).fold(f64::NAN, |m: f64, v| {
            if m.is_nan() {
                v
            } else if ax.reverse {
                m.max(v)
            } else {
                m.min(v)
            }
        });
        out.extend(
            row.iter()
                .filter(|s| (start(&s.2) - first).abs() > 1.0)
                .map(|s| s.1.to_string()),
        );
    }
    out
}

/// Siblings of one row out of their writing order, and a card with no arrow sent to a row after a
/// sibling written later.
/// Notes standing farther than `NOTE_FAR` from the card they annotate (bug 25), by note.
fn note_far(d: &Diagram) -> Vec<String> {
    let mut out = Vec::new();
    for e in &d.model.edges {
        let Some((note, target)) = crate::tidy::notes::note_link(&d.model, e) else {
            continue;
        };
        let (Some(a), Some(b)) = (d.boxes.get(note), d.boxes.get(target)) else {
            continue;
        };
        let dx = (a.x - b.right()).max(b.x - a.right()).max(0.0);
        let dy = (a.y - b.bottom()).max(b.y - a.bottom()).max(0.0);
        if dx.hypot(dy) > NOTE_FAR {
            out.push(note.to_string());
        }
    }
    out
}

/// Sibling zones out of writing order along the flow (bug 27, D-041, D-086): zone `a`, written
/// before its sibling `b`, reaches it by arrows (through sibling zones, each step from a zone to one
/// written after it), yet `b` stands wholly before `a` along the flow. By pair `a b`.
fn zone_order(d: &Diagram) -> Vec<String> {
    let ax = Axes::of(&d.model);
    let m = &d.model;
    let mut out = Vec::new();
    let mut by_parent: BTreeMap<Option<&str>, Vec<&str>> = BTreeMap::new();
    for n in m
        .nodes
        .iter()
        .filter(|n| n.kind == Kind::Zone && !invisible(n))
    {
        by_parent
            .entry(n.parent.as_deref())
            .or_default()
            .push(n.id.as_str());
    }
    for sibs in by_parent.values() {
        let k = sibs.len();
        // forward steps: an arrow from inside zone i into zone j, written after it
        let mut reach = vec![vec![false; k]; k];
        for e in m.edges.iter().filter(|e| e.constraint) {
            let at = |id: &str| {
                sibs.iter()
                    .position(|z| crate::tidy::geom::within(m, id, z))
            };
            if let (Some(i), Some(j)) = (at(&e.from), at(&e.to))
                && i < j
            {
                reach[i][j] = true;
            }
        }
        for via in 0..k {
            for i in 0..k {
                for j in 0..k {
                    if reach[i][via] && reach[via][j] {
                        reach[i][j] = true;
                    }
                }
            }
        }
        for i in 0..k {
            for j in i + 1..k {
                let (Some(a), Some(b)) = (d.boxes.get(sibs[i]), d.boxes.get(sibs[j])) else {
                    continue;
                };
                if reach[i][j] && ax.beyond(b, a) {
                    out.push(format!("{} {}", sibs[i], sibs[j]));
                }
            }
        }
    }
    out
}

/// A card past a reading measure, neither `.wide` nor holding a table: a paragraph drawn on one line,
/// the image as wide as the sentence (the user's "Comment marche ACM": a text card 1 002 px wide).
fn long_line(d: &Diagram) -> Vec<String> {
    d.model
        .nodes
        .iter()
        .filter(|n| n.kind == Kind::Card && !n.classes.iter().any(|c| c == "wide"))
        .filter(|n| {
            d.scene
                .cards
                .get(&n.id)
                .is_some_and(|c| c.tables.is_empty())
        })
        .filter(|n| d.boxes.get(&n.id).is_some_and(|b| b.w > MEASURE))
        .map(|n| n.id.clone())
        .collect()
}

/// Cards outside the flow (top level, no arrow, not docked: a legend, a paragraph, a link) that
/// leave the image `SPRAWL` times the area of the rest or more: each such card standing outside the
/// rest's bounds (the user's "Comment marche ACM", 50 of the 81 views of their site).
fn sprawl(d: &Diagram) -> Vec<String> {
    let linked = |id: &str| d.model.edges.iter().any(|e| e.from == id || e.to == id);
    let free: Vec<&str> = d
        .model
        .nodes
        .iter()
        .filter(|n| {
            n.kind == Kind::Card && n.parent.is_none() && n.dock.is_none() && !linked(&n.id)
        })
        .map(|n| n.id.as_str())
        .collect();
    let bounds = |keep: &dyn Fn(&str) -> bool| {
        d.boxes
            .iter()
            .filter(|(id, _)| keep(id))
            .fold(None::<Box2>, |acc, (_, b)| {
                Some(acc.map_or(*b, |a| {
                    let x = a.x.min(b.x);
                    let y = a.y.min(b.y);
                    Box2 {
                        x,
                        y,
                        w: a.right().max(b.right()) - x,
                        h: a.bottom().max(b.bottom()) - y,
                    }
                }))
            })
    };
    let (Some(all), Some(rest)) = (bounds(&|_| true), bounds(&|id| !free.contains(&id))) else {
        return Vec::new();
    };
    if all.w * all.h < SPRAWL * rest.w * rest.h {
        return Vec::new();
    }
    free.iter()
        .filter(|id| {
            d.boxes.get(**id).is_some_and(|b| {
                b.x >= rest.right()
                    || b.right() <= rest.x
                    || b.y >= rest.bottom()
                    || b.bottom() <= rest.y
            })
        })
        .map(|id| (*id).to_string())
        .collect()
}

/// A drawing whose cards cover less than `SPARSE` of its area (the cards outside the flow left
/// out, `sprawl` counts them): zones in a staircase, a row far wider than the rest (the user's
/// "Carte des CRDs": 10 %, the median of their site 19 %). One per diagram.
fn sparse(d: &Diagram) -> Vec<String> {
    let linked = |id: &str| d.model.edges.iter().any(|e| e.from == id || e.to == id);
    let outside = |n: &skemo_lang::Node| {
        n.kind == Kind::Card && n.parent.is_none() && (n.dock.is_some() || !linked(&n.id))
    };
    let kept: Vec<(&skemo_lang::Node, Box2)> = d
        .model
        .nodes
        .iter()
        .filter(|n| !outside(n))
        .filter_map(|n| d.boxes.get(&n.id).map(|b| (n, *b)))
        .collect();
    let cards: Vec<&Box2> = kept
        .iter()
        .filter(|(n, _)| n.kind == Kind::Card)
        .map(|(_, b)| b)
        .collect();
    if cards.len() < 4 {
        return Vec::new();
    }
    let x0 = kept.iter().map(|(_, b)| b.x).fold(f64::INFINITY, f64::min);
    let y0 = kept.iter().map(|(_, b)| b.y).fold(f64::INFINITY, f64::min);
    let x1 = kept
        .iter()
        .map(|(_, b)| b.right())
        .fold(f64::NEG_INFINITY, f64::max);
    let y1 = kept
        .iter()
        .map(|(_, b)| b.bottom())
        .fold(f64::NEG_INFINITY, f64::max);
    let covered: f64 = cards.iter().map(|b| b.w * b.h).sum();
    if covered < SPARSE * (x1 - x0) * (y1 - y0) {
        vec!["drawing".to_string()]
    } else {
        Vec::new()
    }
}

/// The bands a set of spans makes along an axis: the spans that meet merged, in order.
fn bands(mut spans: Vec<(f64, f64)>) -> Vec<(f64, f64)> {
    spans.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut out: Vec<(f64, f64)> = Vec::new();
    for s in spans {
        match out.last_mut() {
            Some(l) if s.0 <= l.1 + 0.5 => l.1 = l.1.max(s.1),
            _ => out.push(s),
        }
    }
    out
}

/// The gutters of a group along one axis (`across`: x): its children cut into rows (the bands
/// they make along the other axis), then the free strips between the columns of each row.
fn gutters(children: &[Box2], across: bool) -> Vec<f64> {
    let along = |b: &Box2| {
        if across {
            (b.x, b.right())
        } else {
            (b.y, b.bottom())
        }
    };
    let other = |b: &Box2| {
        if across {
            (b.y, b.bottom())
        } else {
            (b.x, b.right())
        }
    };
    let mut out = Vec::new();
    for row in bands(children.iter().map(other).collect()) {
        let inside = |b: &&Box2| other(b).0 >= row.0 - 0.5 && other(b).1 <= row.1 + 0.5;
        let columns = bands(children.iter().filter(inside).map(along).collect());
        out.extend(columns.windows(2).map(|w| w[1].0 - w[0].1));
    }
    out
}

/// The room made evenly (the manual method's §7): the cards of a group, and the groups, stand
/// the same distance apart; a gutter wider than its neighbours is counted, by group and axis
/// (`zone x`, `drawing y`). The cards outside the grid are left out: a docked element, a pinned
/// card (D-031) and a note, which stands beside its card (D-048 R7).
fn uneven_gaps(d: &Diagram) -> Vec<String> {
    let notes: Vec<&str> = d
        .model
        .edges
        .iter()
        .filter_map(|e| crate::tidy::notes::note_link(&d.model, e).map(|(n, _)| n))
        .collect();
    let in_grid = |n: &&skemo_lang::Node| {
        n.dock.is_none() && n.at.is_none() && !notes.contains(&n.id.as_str())
    };
    let groups = std::iter::once(None).chain(
        d.model
            .nodes
            .iter()
            .filter(|n| n.kind == Kind::Zone)
            .map(|n| Some(n.id.as_str())),
    );
    let mut out = Vec::new();
    for group in groups {
        let children: Vec<Box2> = d
            .model
            .children(group)
            .filter(in_grid)
            .filter_map(|n| d.boxes.get(&n.id).copied())
            .collect();
        for across in [true, false] {
            let gaps = gutters(&children, across);
            let least = gaps.iter().copied().fold(f64::INFINITY, f64::min);
            let most = gaps.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            if most - least > TOUCH {
                let axis = if across { "x" } else { "y" };
                out.push(format!("{} {axis}", group.unwrap_or("drawing")));
            }
        }
    }
    out
}

fn writing_order(d: &Diagram) -> Vec<String> {
    let ax = Axes::of(&d.model);
    let mut out = Vec::new();
    for row in rows(d) {
        for (i, a) in row.iter().enumerate() {
            for b in &row[i + 1..] {
                if a.0 < b.0 && ax.mid(&a.2) > ax.mid(&b.2) + 1.0 {
                    out.push(format!("{} {}", a.1, b.1));
                }
            }
        }
    }
    let linked = |id: &str| d.model.edges.iter().any(|e| e.from == id || e.to == id);
    let mut by_parent: BTreeMap<Option<&str>, Vec<(usize, &str, Box2)>> = BTreeMap::new();
    for (k, n) in d.model.nodes.iter().enumerate() {
        if in_rows(d, &n.id)
            && let Some(b) = d.boxes.get(&n.id)
        {
            by_parent
                .entry(n.parent.as_deref())
                .or_default()
                .push((k, n.id.as_str(), *b));
        }
    }
    for sibs in by_parent.values() {
        for a in sibs.iter().filter(|a| !linked(a.1)) {
            if sibs.iter().any(|b| b.0 > a.0 && ax.beyond(&b.2, &a.2)) {
                out.push(a.1.to_string());
            }
        }
    }
    out
}

/// A segment of a route strictly inside a box, `inset` px in from its border.
fn cuts_box(r: &[(f64, f64)], b: &Box2, inset: f64) -> bool {
    segments(r).any(|(p, q)| {
        let (x0, x1) = (p.0.min(q.0), p.0.max(q.0));
        let (y0, y1) = (p.1.min(q.1), p.1.max(q.1));
        x1 > b.x + inset && x0 < b.right() - inset && y1 > b.y + inset && y0 < b.bottom() - inset
    })
}

/// Are `a` and `b` ever shown together? (an animated diagram shows its steps one at a time)
fn together(d: &Diagram, a: &str, b: &str) -> bool {
    !d.anim.is_animated()
        || d.anim
            .states
            .iter()
            .any(|s| d.anim.visible(a, s.value) && d.anim.visible(b, s.value))
}

/// An arrow running through a card, its own ends included (1 px in from the border its ports sit
/// on; a turn moved into a source drew a line across its text).
fn through_card(d: &Diagram) -> Vec<String> {
    let mut out = Vec::new();
    for e in &d.model.edges {
        let Some(r) = d.routes.get(&e.id) else {
            continue;
        };
        // an arrow through a tunnel crosses the tunnel's cards on purpose (D-116)
        let crossed = crate::tunnel::cuts(&d.model, e);
        for (c, b) in cards(d) {
            if together(d, &e.id, c) && !crossed.contains(&c) && cuts_box(r, &b, 1.0) {
                out.push(format!("{} {c}", e.id));
            }
        }
    }
    out
}

/// An arrow running through a zone that holds neither of its ends (the manual method's §7, foreign
/// groups): it reads as belonging to that zone. A zone kept as a corridor for the arrows into its
/// rows (`layout::corridor`, D-056) is crossed on purpose.
fn through_zone(d: &Diagram) -> Vec<String> {
    let mut out = Vec::new();
    for e in &d.model.edges {
        let Some(r) = d.routes.get(&e.id) else {
            continue;
        };
        for (z, b) in zones(d) {
            let holds = |end: &str| end == z || nested(&d.model, end, z);
            if holds(&e.from)
                || holds(&e.to)
                || crate::layout::corridor(&d.model, z)
                || !together(d, &e.id, z)
            {
                continue;
            }
            if cuts_box(r, &b, 1.0) {
                out.push(format!("{} {z}", e.id));
            }
        }
    }
    out
}

/// An arrow running through a zone's title or its tag pill (3 px in: a line may graze a title).
fn through_title(d: &Diagram) -> Vec<String> {
    let titles = crate::labels::zone_titles(&d.model, &d.scene, &d.boxes);
    let mut out = Vec::new();
    for (id, r) in &d.routes {
        for (z, t) in &titles {
            if cuts_box(r, t, 3.0) {
                out.push(format!("{id} title:{z}"));
            }
        }
    }
    out
}

/// Two arrows sharing no end, nor a group, drawn on one line (within 1.5 px over more than 4 px):
/// they read as one arrow. A crossing is fine.
fn overlay(d: &Diagram) -> Vec<String> {
    // (edge index, vertical, position across, start, end along)
    let mut runs: Vec<(usize, bool, f64, f64, f64)> = Vec::new();
    for (k, e) in d.model.edges.iter().enumerate() {
        let Some(r) = d.routes.get(&e.id) else {
            continue;
        };
        for (p, q) in segments(r) {
            if (p.0 - q.0).abs() < 0.5 {
                runs.push((k, true, p.0, p.1.min(q.1), p.1.max(q.1)));
            } else if (p.1 - q.1).abs() < 0.5 {
                runs.push((k, false, p.1, p.0.min(q.0), p.0.max(q.0)));
            }
        }
    }
    let e = &d.model.edges;
    let mut out = Vec::new();
    for (i, a) in runs.iter().enumerate() {
        for b in &runs[i + 1..] {
            let (ea, eb) = (&e[a.0], &e[b.0]);
            let unrelated = a.0 != b.0
                && ea.from != eb.from
                && ea.to != eb.to
                && ea.from != eb.to
                && ea.to != eb.from
                && !d
                    .edge_groups
                    .get(&ea.id)
                    .is_some_and(|g| d.edge_groups.get(&eb.id) == Some(g));
            if unrelated
                && a.1 == b.1
                && (a.2 - b.2).abs() < 1.5
                && a.4.min(b.4) - a.3.max(b.3) > 4.0
            {
                out.push(format!("{} {}", ea.id, eb.id));
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

/// Two arrows drawn side by side, 1.5 px to [`CLOSE`] apart over more than [`CLOSE_LEN`]: neither
/// one line (a junction, or an `overlay`) nor two, they read as one thick line (the manual
/// method's §7, "lines that cannot be confused").
fn close_lines(d: &Diagram) -> Vec<String> {
    // (edge, upright, position across, start, end along)
    let mut runs: Vec<(&str, bool, f64, f64, f64)> = Vec::new();
    for e in &d.model.edges {
        let Some(r) = d.routes.get(&e.id) else {
            continue;
        };
        for (p, q) in stretches(r) {
            if (p.0 - q.0).abs() < 0.5 {
                runs.push((&e.id, true, p.0, p.1.min(q.1), p.1.max(q.1)));
            } else if (p.1 - q.1).abs() < 0.5 {
                runs.push((&e.id, false, p.1, p.0.min(q.0), p.0.max(q.0)));
            }
        }
    }
    let mut out = Vec::new();
    for (i, a) in runs.iter().enumerate() {
        for b in &runs[i + 1..] {
            let apart = (a.2 - b.2).abs();
            let beside = a.4.min(b.4) - a.3.max(b.3);
            // the lanes of one tube run close on purpose (D-116)
            let tube = crate::tunnel::tube_of(&d.model, a.0)
                .is_some_and(|t| crate::tunnel::tube_of(&d.model, b.0) == Some(t));
            if a.0 != b.0
                && a.1 == b.1
                && !tube
                && (1.5..CLOSE).contains(&apart)
                && beside > CLOSE_LEN
            {
                out.push(format!("{} {}", a.0, b.0));
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

/// A straight stretch of a route: its two ends.
type Stretch = ((f64, f64), (f64, f64));

/// A route as its straight stretches: steps of no length dropped, steps in line merged.
fn stretches(r: &[(f64, f64)]) -> Vec<Stretch> {
    let mut out: Vec<Stretch> = Vec::new();
    for (p, q) in segments(r) {
        if (p.0 - q.0).abs() + (p.1 - q.1).abs() < 0.5 {
            continue;
        }
        let upright = |a: (f64, f64), b: (f64, f64)| (a.0 - b.0).abs() < 0.5;
        let level = |a: (f64, f64), b: (f64, f64)| (a.1 - b.1).abs() < 0.5;
        match out.last_mut() {
            Some(l) if (upright(l.0, l.1) && upright(p, q)) || (level(l.0, l.1) && level(p, q)) => {
                l.1 = q;
            }
            _ => out.push((p, q)),
        }
    }
    out
}

/// A route with three bends or more that never turns back, in x nor in y: an L or a Z joins the
/// same ends (the manual method's §6).
fn staircase(d: &Diagram) -> Vec<String> {
    let one_way =
        |steps: Vec<f64>| steps.iter().all(|v| *v > 0.0) || steps.iter().all(|v| *v < 0.0);
    d.routes
        .iter()
        .filter(|(_, r)| {
            let s = stretches(r);
            let along = |f: fn(&Stretch) -> f64| -> Vec<f64> {
                s.iter().map(f).filter(|v| v.abs() > 0.5).collect()
            };
            s.len() >= 4 && one_way(along(|(p, q)| q.0 - p.0)) && one_way(along(|(p, q)| q.1 - p.1))
        })
        .map(|(id, _)| id.clone())
        .collect()
}

/// An arrow's head squeezed against its last bend: a last stretch shorter than [`TAIL`] does not
/// show where the arrow comes from (the manual method's §6). A straight arrow between two close
/// cards has no bend to hide.
fn short_tail(d: &Diagram) -> Vec<String> {
    d.routes
        .iter()
        .filter(|(_, r)| {
            let s = stretches(r);
            let last = s
                .last()
                .map_or(0.0, |(p, q)| (p.0 - q.0).abs() + (p.1 - q.1).abs());
            s.len() >= 2 && last < TAIL - 0.5
        })
        .map(|(id, _)| id.clone())
        .collect()
}

/// A Z whose middle stretch is shorter than [`JOG`]: two ports nearly in line, joined by a jog
/// that one port moved, or one card, would straighten (the manual method's §6).
fn port_jog(d: &Diagram) -> Vec<String> {
    d.routes
        .iter()
        .filter(|(_, r)| {
            let s = stretches(r);
            s.len() == 3 && (s[1].0.0 - s[1].1.0).abs() + (s[1].0.1 - s[1].1.1).abs() < JOG
        })
        .map(|(id, _)| id.clone())
        .collect()
}

/// An arrow whose first point is not on its source card, or whose last point is not on its target
/// (bugs 13 and 14 of the private corpus: an arrow leaving from under the card beside its source,
/// or ending on another card): the drawing then says something false. 2 px of rounding allowed.
fn end_off_card(d: &Diagram) -> Vec<String> {
    let on = |p: (f64, f64), b: &Box2| {
        p.0 >= b.x - 2.0 && p.0 <= b.right() + 2.0 && p.1 >= b.y - 2.0 && p.1 <= b.bottom() + 2.0
    };
    let mut out = Vec::new();
    for e in &d.model.edges {
        let (Some(r), Some(a), Some(b)) = (
            d.routes.get(&e.id),
            d.boxes.get(&e.from),
            d.boxes.get(&e.to),
        ) else {
            continue;
        };
        let (Some(first), Some(last)) = (r.first(), r.last()) else {
            continue;
        };
        if !on(*first, a) {
            out.push(format!("{} from", e.id));
        }
        if !on(*last, b) {
            out.push(format!("{} to", e.id));
        }
    }
    out
}

/// A card's text, or the box drawn under a `code` or a `mark`, wider than the card (bug 17: a
/// long `code` that cannot wrap ran out of its card and of the image).
fn text_overflow(d: &Diagram) -> Vec<String> {
    let mut out = Vec::new();
    for (id, b) in cards(d) {
        let Some(c) = d.scene.cards.get(id) else {
            continue;
        };
        let g = crate::geometry::card(&d.scene, c, b.w, b.h);
        let right = g
            .title
            .iter()
            .chain(g.kind.iter())
            .chain(g.lis.iter().flat_map(|l| l.runs.iter()))
            .map(|r| r.x + r.width)
            .chain(g.lis.iter().flat_map(|l| l.fills.iter()).map(|f| f.x + f.w))
            .fold(f64::NEG_INFINITY, f64::max);
        if right > b.w + 0.5 {
            out.push(id.to_string());
        }
    }
    out
}

/// An arrow label on a card, or on another label.
fn label_overlap(d: &Diagram) -> Vec<String> {
    let boxes = label_boxes(d);
    let mut out = Vec::new();
    for (i, (id, l)) in boxes.iter().enumerate() {
        for (c, b) in cards(d) {
            if together(d, id, c) && meet(l, &b) {
                out.push(format!("{id} {c}"));
            }
        }
        for (other, m) in &boxes[i + 1..] {
            if together(d, id, other) && meet(l, m) {
                out.push(format!("{id} {other}"));
            }
        }
    }
    out
}

/// An arrow label crossed by the edge of a zone's frame: the frame's line runs through its text
/// (the user, on the POC's "Comment marche ACM": a label astride a firewall zone's border).
fn label_on_frame(d: &Diagram) -> Vec<String> {
    let mut out = Vec::new();
    for (id, l) in label_boxes(d) {
        for (z, b) in zones(d) {
            let across_x = |x: f64| l.x + TOUCH < x && x < l.right() - TOUCH;
            let across_y = |y: f64| l.y + TOUCH < y && y < l.bottom() - TOUCH;
            let spans_y = l.y < b.bottom() && b.y < l.bottom();
            let spans_x = l.x < b.right() && b.x < l.right();
            let hit = (spans_y && (across_x(b.x) || across_x(b.right())))
                || (spans_x && (across_y(b.y) || across_y(b.bottom())));
            if hit && together(d, id, z) {
                out.push(format!("{id} {z}"));
            }
        }
    }
    out
}

/// The box of each arrow label (file coordinates).
fn label_boxes(d: &Diagram) -> Vec<(&String, Box2)> {
    d.labels
        .iter()
        .filter_map(|(id, (cx, cy))| {
            let s = d.sizes.labels.get(id)?;
            Some((
                id,
                Box2 {
                    x: cx - s.w / 2.0,
                    y: cy - s.h / 2.0,
                    w: s.w,
                    h: s.h,
                },
            ))
        })
        .collect()
}

/// A line drawn across the label of another arrow (bug 24): the label hides the line, or the
/// line crosses its text, and one no longer knows which arrow the label names. The arrows of one
/// group (a join sharing its label, D-051) do not count.
fn through_label(d: &Diagram) -> Vec<String> {
    let labels = label_boxes(d);
    let mut out = Vec::new();
    for (id, r) in &d.routes {
        for (l, b) in &labels {
            let same_group = d
                .edge_groups
                .get(id.as_str())
                .is_some_and(|g| d.edge_groups.get(l.as_str()) == Some(g));
            if *l != id && !same_group && together(d, id, l) && cuts_box(r, b, TOUCH) {
                out.push(format!("{id} label:{l}"));
            }
        }
    }
    out
}

/// A zone title wrapped on more lines than its room needs (bug 20): the room runs from where the
/// title starts to its frame's right edge, less the same margin as on the left (bug 23), and to
/// the first line coming down past it, less the router's margin (D-064, D-099); words are put
/// on lines as `layout::title_wraps` does.
fn needless_wrap(d: &Diagram) -> Vec<String> {
    let margin = crate::route::params().shape_buffer;
    let titles = crate::labels::zone_titles(&d.model, &d.scene, &d.boxes);
    let mut out = Vec::new();
    for (id, b) in zones(d) {
        let (Some(n), Some(z)) = (d.model.node(id), d.scene.zones.get(id)) else {
            continue;
        };
        let text = d.scene.zone_text(id, &n.title);
        if !text.contains('\n') {
            continue;
        }
        let tag = n.tag.as_deref();
        let geo = |t: &str, tag: Option<&str>| {
            crate::geometry::zone_title(&d.scene, z, (t, tag), 4000.0, 400.0)
        };
        let end_of = |t: &str, tag: Option<&str>| {
            let t = geo(t, tag);
            t.tag.map_or(t.span.1, |p| t.span.1.max(p.x + p.w)) + 2.0
        };
        let start = geo(&n.title, tag).span.0;
        // the title's own box (the first of the zone's boxes: title, then descriptions, pill)
        let Some((_, tb)) = titles.iter().find(|(t, _)| t == id) else {
            continue;
        };
        let wall = d
            .routes
            .values()
            .flat_map(|r| segments(r))
            .filter(|(p, q)| (p.0 - q.0).abs() < 0.5)
            .filter(|(p, q)| p.1.max(q.1) > tb.y && p.1.min(q.1) < tb.bottom())
            .map(|(p, _)| p.0)
            .filter(|x| *x > tb.x)
            .fold(f64::INFINITY, f64::min);
        let room = (b.w - start).min(wall - margin - b.x);
        let mut lines: Vec<String> = Vec::new();
        for w in n.title.split_whitespace() {
            match lines.last_mut() {
                Some(l) if end_of(&format!("{l} {w}"), None) <= room => {
                    l.push(' ');
                    l.push_str(w);
                }
                _ => lines.push(w.to_string()),
            }
        }
        let mut need = lines.len();
        if tag.is_some() && end_of(&lines.join("\n"), tag) > room + 0.5 {
            need += 1;
        }
        let drawn = text.matches('\n').count() + 1;
        if need < drawn {
            out.push(format!("{id} {drawn}>{need}"));
        }
    }
    out
}

fn moved_box(a: &Box2, b: &Box2) -> bool {
    (a.x - b.x).abs() > 0.5
        || (a.y - b.y).abs() > 0.5
        || (a.w - b.w).abs() > 0.5
        || (a.h - b.h).abs() > 0.5
}

/// What moved in `after` without being linked to `card`: an arrow between two cards that stayed,
/// a card other than `card`, a zone not holding it. `moved`: the card's new place when it was
/// moved; an arrow that ran where it now stands (within the router's margin) has to give way
/// (nothing hidden ranks above stability, layout method §4) and is not counted.
fn unrelated_moves(
    before: &Diagram,
    after: &Diagram,
    card: &str,
    moved: Option<Box2>,
) -> Vec<String> {
    let m = &before.model;
    let still = |id: &str| match (before.boxes.get(id), after.boxes.get(id)) {
        (Some(a), Some(b)) => !moved_box(a, b),
        _ => false,
    };
    let margin = crate::route::params().shape_buffer;
    let in_the_way = |r: &[(f64, f64)]| moved.is_some_and(|b| cuts_box(r, &b, -margin));
    let mut out = Vec::new();
    for e in &m.edges {
        // an arrow through a tunnel belongs to the tunnel's cards too (D-116)
        let ends = crate::tunnel::ends(m, e);
        if e.from == card
            || e.to == card
            || ends.contains(&card)
            || !still(&e.from)
            || !still(&e.to)
            || ends.iter().any(|c| !still(c))
        {
            continue;
        }
        if before.routes.get(&e.id).is_some_and(|r| in_the_way(r)) {
            continue;
        }
        if let (Some(a), Some(b)) = (before.routes.get(&e.id), after.routes.get(&e.id))
            && (a.len() != b.len()
                || a.iter()
                    .zip(b)
                    .any(|(p, q)| (p.0 - q.0).abs() > 0.5 || (p.1 - q.1).abs() > 0.5))
        {
            out.push(format!("{card}>{}", e.id));
        }
    }
    let ancestors: Vec<&str> = m.ancestors(card).iter().map(|n| n.id.as_str()).collect();
    for n in &m.nodes {
        if n.id == card || ancestors.contains(&n.id.as_str()) {
            continue;
        }
        if let (Some(a), Some(b)) = (before.boxes.get(&n.id), after.boxes.get(&n.id))
            && moved_box(a, b)
        {
            out.push(format!("{card}>{}", n.id));
        }
    }
    out
}

/// Stability of an edit (D-031; `pin-moves` in the method's rank 1, `move-moves` its last since
/// D-108), from the source: for each card of a graph that is neither pinned nor docked,
/// `pin-moves` lists what moves when the card is pinned where it stands (nothing should),
/// `move-moves` what moves, other than its own arrows and the arrows it now stands on, when it is
/// moved by (60, 40). Two layouts per card.
pub fn stability(source: &str, opts: &Options, d: &Diagram) -> (Vec<String>, Vec<String>) {
    let (mut pin, mut mv) = (Vec::new(), Vec::new());
    if !crate::diagram::automatic(&d.model) {
        return (pin, mv);
    }
    for n in d
        .model
        .nodes
        .iter()
        .filter(|n| n.kind == Kind::Card && n.at.is_none() && n.dock.is_none())
    {
        let Some(b) = d.boxes.get(&n.id) else {
            continue;
        };
        for (k, (dx, dy)) in [(0.0, 0.0), (60.0, 40.0)].into_iter().enumerate() {
            let Ok(edited) = skemo_lang::edit::move_card(source, &n.id, b.x + dx, b.y + dy) else {
                continue;
            };
            let Ok((after, _)) = crate::diagram::from_source(&edited, opts) else {
                continue;
            };
            let to = after.boxes.get(&n.id).copied().filter(|_| k == 1);
            let moved = unrelated_moves(d, &after, &n.id, to);
            if k == 0 { &mut pin } else { &mut mv }.extend(moved);
        }
    }
    (pin, mv)
}

/// Every property of a source, the stability pair included; `None` when it does not parse.
pub fn all_defects(source: &str, opts: &Options) -> Option<BTreeMap<&'static str, Vec<String>>> {
    let (d, _) = crate::diagram::from_source(source, opts).ok()?;
    let mut out = defects(&d);
    let (pin, mv) = stability(source, opts, &d);
    out.insert("pin-moves", pin);
    out.insert("move-moves", mv);
    Some(out)
}

fn label_markup(d: &Diagram) -> Vec<String> {
    d.model
        .edges
        .iter()
        .filter(|e| e.label.contains('`'))
        .map(|e| e.id.clone())
        .collect()
}

/// A diagram of a corpus: its name in the ledger, its source and its stylesheets.
pub struct Entry {
    pub name: String,
    pub source: String,
    pub css: Vec<String>,
}

/// Counts per diagram and property (only the properties broken at least once are kept).
pub type Counts = BTreeMap<String, BTreeMap<String, usize>>;

/// The counts of a corpus. A diagram that does not parse counts as `error`.
pub fn count(entries: &[Entry]) -> Counts {
    let mut out = Counts::new();
    for e in entries {
        let opts = Options {
            css: e.css.clone(),
            ..Default::default()
        };
        let row = match all_defects(&e.source, &opts) {
            Some(found) => found
                .into_iter()
                .filter(|(_, v)| !v.is_empty())
                .map(|(p, v)| (p.to_string(), v.len()))
                .collect(),
            None => BTreeMap::from([("error".to_string(), 1)]),
        };
        out.insert(e.name.clone(), row);
    }
    out
}

/// The ledger as text: one line per diagram, `name: property=count …` (or `clean`), then the totals.
pub fn format(counts: &Counts) -> String {
    let mut s = String::new();
    let mut totals: BTreeMap<&str, usize> = BTreeMap::new();
    for (name, row) in counts {
        let cells: Vec<String> = row.iter().map(|(p, n)| format!("{p}={n}")).collect();
        let body = if cells.is_empty() {
            "clean".to_string()
        } else {
            cells.join(" ")
        };
        s.push_str(&format!("{name}: {body}\n"));
        for (p, n) in row {
            *totals.entry(p).or_default() += n;
        }
    }
    let t: Vec<String> = PROPERTIES
        .iter()
        .map(|p| format!("{p}={}", totals.get(p).copied().unwrap_or(0)))
        .collect();
    s.push_str(&format!("# total: {}\n", t.join(" ")));
    s
}

/// The ledger read back (`format`'s lines; `#` lines ignored).
pub fn parse(text: &str) -> Counts {
    let mut out = Counts::new();
    for line in text.lines().filter(|l| !l.starts_with('#')) {
        let Some((name, body)) = line.rsplit_once(": ") else {
            continue;
        };
        let row = body
            .split_whitespace()
            .filter_map(|c| c.split_once('='))
            .filter_map(|(p, n)| n.parse().ok().map(|n| (p.to_string(), n)))
            .collect();
        out.insert(name.to_string(), row);
    }
    out
}

/// The ratchet: what got worse (a count above the ledger's, a diagram missing from it) and what
/// got better, one line each.
pub fn compare(ledger: &Counts, now: &Counts) -> (Vec<String>, Vec<String>) {
    let (mut worse, mut better) = (Vec::new(), Vec::new());
    for (name, row) in now {
        let Some(old) = ledger.get(name) else {
            worse.push(format!("{name}: not in the ledger"));
            continue;
        };
        let props: std::collections::BTreeSet<&String> = row.keys().chain(old.keys()).collect();
        for p in props {
            let (a, b) = (
                old.get(p).copied().unwrap_or(0),
                row.get(p).copied().unwrap_or(0),
            );
            if b > a {
                worse.push(format!("{name}: {p} {a} → {b}"));
            } else if b < a {
                better.push(format!("{name}: {p} {a} → {b}"));
            }
        }
    }
    for name in ledger.keys().filter(|n| !now.contains_key(*n)) {
        better.push(format!("{name}: gone from the corpus"));
    }
    (worse, better)
}

/// The stylesheets a diagram includes with `diagram(css="a.css b.css")`, read next to it (the
/// command line does the same, `cli/src/common.rs`).
pub fn included_css(source: &str, dir: &std::path::Path) -> Vec<String> {
    skemo_lang::parse(source)
        .model
        .option("css")
        .map(|v| {
            v.split_whitespace()
                .filter_map(|n| std::fs::read_to_string(dir.join(n)).ok())
                .collect()
        })
        .unwrap_or_default()
}

/// The diagrams under `paths` (files, or folders searched recursively for `.skemo` and `.pug`),
/// named by their path from `root`; each with the stylesheet of the same name beside it, if any,
/// then the ones it includes.
pub fn entries(paths: &[std::path::PathBuf], root: &std::path::Path) -> Vec<Entry> {
    fn walk(p: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        if p.is_dir() {
            let mut kids: Vec<_> = std::fs::read_dir(p)
                .into_iter()
                .flatten()
                .flatten()
                .map(|e| e.path())
                .collect();
            kids.sort();
            for k in kids {
                walk(&k, out);
            }
        } else if p.extension().is_some_and(|x| x == "skemo" || x == "pug") {
            out.push(p.to_path_buf());
        }
    }
    let mut files = Vec::new();
    for p in paths {
        walk(p, &mut files);
    }
    files
        .into_iter()
        .filter_map(|f| {
            let source = std::fs::read_to_string(&f).ok()?;
            let dir = f.parent().unwrap_or(std::path::Path::new("."));
            let mut css: Vec<String> = std::fs::read_to_string(f.with_extension("css"))
                .into_iter()
                .collect();
            css.extend(included_css(&source, dir));
            let name = f.strip_prefix(root).unwrap_or(&f).display().to_string();
            Some(Entry { name, source, css })
        })
        .collect()
}
