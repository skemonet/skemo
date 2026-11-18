//! Automatic groups of arrows: forks (one card to a row of cards) and buses.

use super::sides::{AGAINST, after_side, against_stack, opposite};
use crate::layout::Box2;
use skemo_lang::{Kind, Model};
use skemo_router::Side;
use std::collections::BTreeMap;

/// Automatic forks (D-047): the arrows of one card to at least two cards of the next row (all
/// beyond it in the drawing's direction, on one row) are drawn as an organisation chart, one
/// trunk then branches, as with `join="trunk"`. Only plain arrows: no label, no `side=`, no
/// `via=`, no `join=`, all with the same classes, in the layout. The other arrows of the card do
/// not prevent the fork (D-048, R8): a labelled arrow next to two plain ones keeps its own port
/// while the plain ones share a trunk. Edge id → group.
pub(super) fn auto_forks(
    model: &Model,
    boxes: &BTreeMap<String, Box2>,
) -> BTreeMap<String, String> {
    fans(model, boxes, false)
}

/// Automatic joins (D-091), the mirror of the forks: the plain arrows of cards of one row into
/// one card beyond them, as with a named `join=`, before `keep_crowds` decides which merge.
/// Arrows already in a fork stay in it. Edge id → group.
pub(super) fn auto_joins(
    model: &Model,
    boxes: &BTreeMap<String, Box2>,
    forks: &BTreeMap<String, String>,
) -> BTreeMap<String, String> {
    let mut out = fans(model, boxes, true);
    out.retain(|id, _| !forks.contains_key(id));
    // a join left with one arrow is no join
    let mut count: BTreeMap<String, usize> = BTreeMap::new();
    for g in out.values() {
        *count.entry(g.clone()).or_default() += 1;
    }
    out.retain(|_, g| count[g.as_str()] > 1);
    out
}

/// More connectors than this on one side of a card make its plain arrows merge (D-093).
const CROWD: usize = 4;

/// About this many arrows merge in a crowded side, those in its middle (D-093).
const MERGED: usize = 3;

/// Which automatic forks and joins are drawn merged (D-093): only where merging clears a
/// crowd. A group merges whole when its arrows cross a row of cards on their way (long parallel
/// lines: length comes before number); otherwise only on a side with more than `CROWD`
/// connectors, and then the `MERGED` arrows in the middle of that side. A tree (no card with two
/// arrows in) keeps every fork, as an organisation chart does. A fork and a join over the same
/// cards (a diamond: build → test, lint → image) take the same choice on both sides.
pub(super) fn keep_crowds(
    model: &Model,
    boxes: &BTreeMap<String, Box2>,
    forks: &BTreeMap<String, String>,
    joins: &BTreeMap<String, String>,
) -> (BTreeMap<String, String>, BTreeMap<String, String>) {
    let tree = model
        .nodes
        .iter()
        .all(|n| model.edges.iter().filter(|e| e.to == n.id).count() <= 1);
    let fk = members(model, forks);
    let jn = members(model, joins);
    let mut kept_f: BTreeMap<&str, Vec<&str>> = fk
        .iter()
        .map(|(g, es)| {
            let k = if tree {
                es.iter().map(|e| e.id.as_str()).collect()
            } else {
                kept(model, boxes, es, false)
            };
            (g.as_str(), k)
        })
        .collect();
    let mut kept_j: BTreeMap<&str, Vec<&str>> = jn
        .iter()
        .map(|(g, es)| (g.as_str(), kept(model, boxes, es, true)))
        .collect();
    // diamonds: the fork's targets are the join's sources
    for (g, fe) in &fk {
        let targets: std::collections::BTreeSet<&str> = fe.iter().map(|e| e.to.as_str()).collect();
        for (h, je) in &jn {
            let sources: std::collections::BTreeSet<&str> =
                je.iter().map(|e| e.from.as_str()).collect();
            if sources != targets {
                continue;
            }
            let whole =
                kept_f[g.as_str()].len() == fe.len() || kept_j[h.as_str()].len() == je.len();
            let (f_ids, j_ids): (Vec<&str>, Vec<&str>) = if whole {
                (
                    fe.iter().map(|e| e.id.as_str()).collect(),
                    je.iter().map(|e| e.id.as_str()).collect(),
                )
            } else {
                (Vec::new(), Vec::new())
            };
            kept_f.insert(g.as_str(), f_ids);
            kept_j.insert(h.as_str(), j_ids);
        }
    }
    let out = |all: &BTreeMap<String, String>, kept: &BTreeMap<&str, Vec<&str>>| {
        all.iter()
            .filter(|(id, g)| {
                kept.get(g.as_str())
                    .is_some_and(|k| k.contains(&id.as_str()))
            })
            .map(|(id, g)| (id.clone(), g.clone()))
            .collect()
    };
    (out(forks, &kept_f), out(joins, &kept_j))
}

/// The arrows of each group, in writing order.
fn members<'m>(
    model: &'m Model,
    groups: &BTreeMap<String, String>,
) -> BTreeMap<String, Vec<&'m skemo_lang::Edge>> {
    let mut out: BTreeMap<String, Vec<&skemo_lang::Edge>> = BTreeMap::new();
    for e in &model.edges {
        if let Some(g) = groups.get(&e.id) {
            out.entry(g.clone()).or_default().push(e);
        }
    }
    out
}

/// The arrows of one group (a fork, or a join `into`) that merge.
fn kept<'e>(
    model: &Model,
    boxes: &BTreeMap<String, Box2>,
    edges: &[&'e skemo_lang::Edge],
    into: bool,
) -> Vec<&'e str> {
    if edges.len() < 2 {
        return Vec::new();
    }
    if row_between(model, boxes, edges, into) {
        return edges.iter().map(|e| e.id.as_str()).collect();
    }
    let shared = ends(edges[0], into).0;
    let side = side_connectors(model, boxes, shared, into);
    if side.len() <= CROWD {
        return Vec::new();
    }
    // the run of `MERGED` consecutive connectors of the group nearest the side's middle
    let mine = |id: &str| edges.iter().any(|e| e.id == id);
    let mid = (side.len() - 1) as f64 / 2.0;
    let mut best: Option<(f64, usize, usize)> = None;
    for w in (2..=MERGED).rev() {
        for start in 0..=side.len().saturating_sub(w) {
            if side[start..start + w].iter().all(|id| mine(id)) {
                let off = ((start as f64 + (w - 1) as f64 / 2.0) - mid).abs();
                if best.is_none_or(|(o, bw, _)| bw < w || (bw == w && off < o)) {
                    best = Some((off, w, start));
                }
            }
        }
        if best.is_some() {
            break;
        }
    }
    best.map(|(_, w, start)| {
        edges
            .iter()
            .filter(|e| side[start..start + w].contains(&e.id.as_str()))
            .map(|e| e.id.as_str())
            .collect()
    })
    .unwrap_or_default()
}

/// Spans of a box along the drawing's direction, and its middle across it.
fn axes(model: &Model) -> impl Fn(&Box2) -> ((f64, f64), f64) {
    let across = matches!(model.option("dir").unwrap_or("down"), "right" | "left");
    move |b: &Box2| {
        if across {
            ((b.x, b.right()), b.y + b.h / 2.0)
        } else {
            ((b.y, b.bottom()), b.x + b.w / 2.0)
        }
    }
}

/// The arrows on the side of `shared` that faces a fork's targets (or a join's sources, `into`),
/// in or out, ordered across that side.
fn side_connectors<'m>(
    model: &'m Model,
    boxes: &BTreeMap<String, Box2>,
    shared: &str,
    into: bool,
) -> Vec<&'m str> {
    let ax = axes(model);
    let Some(sb) = boxes.get(shared) else {
        return Vec::new();
    };
    let back = matches!(model.option("dir"), Some("up" | "left"));
    let ((s0, s1), _) = ax(sb);
    // the other end lies before the shared card (a join) or after it (a fork), along the flow
    let facing = |b: &Box2| {
        let ((o0, o1), _) = ax(b);
        let after = if back { o1 <= s0 } else { o0 >= s1 };
        let before = if back { o0 >= s1 } else { o1 <= s0 };
        if into { before } else { after }
    };
    let mut out: Vec<(f64, &str)> = model
        .edges
        .iter()
        .filter_map(|e| {
            let other = if e.from == shared {
                &e.to
            } else if e.to == shared {
                &e.from
            } else {
                return None;
            };
            let b = boxes.get(other.as_str())?;
            facing(b).then(|| (ax(b).1, e.id.as_str()))
        })
        .collect();
    out.sort_by(|a, b| a.0.total_cmp(&b.0));
    out.into_iter().map(|(_, id)| id).collect()
}

/// Does a card stand between the shared card of a fork or join (`into`) and its other ends,
/// along the drawing's direction, strictly inside the gap (a whole row the arrows cross)?
fn row_between(
    model: &Model,
    boxes: &BTreeMap<String, Box2>,
    edges: &[&skemo_lang::Edge],
    into: bool,
) -> bool {
    let ax = axes(model);
    let shared = ends(edges[0], into).0;
    let Some(sb) = boxes.get(shared) else {
        return false;
    };
    let (t, _) = ax(sb);
    let mut gap: Option<(f64, f64)> = None;
    for e in edges {
        let Some(b) = boxes.get(ends(e, into).1) else {
            return false;
        };
        let (s, _) = ax(b);
        // the empty stretch between this end and the shared card, whichever comes first
        let g = if s.1 <= t.0 { (s.1, t.0) } else { (t.1, s.0) };
        gap = Some(gap.map_or(g, |(lo, hi)| (lo.max(g.0), hi.min(g.1))));
    }
    let Some((lo, hi)) = gap.filter(|(lo, hi)| hi > lo) else {
        return false;
    };
    model.nodes.iter().any(|n| {
        n.kind == Kind::Card
            && n.id != shared
            && !edges.iter().any(|e| ends(e, into).1 == n.id)
            && boxes.get(&n.id).is_some_and(|b| {
                let (c, _) = ax(b);
                c.0 >= lo && c.1 <= hi
            })
    })
}

/// Group kind of the automatic joins; never a name given in `join=` (a NUL in it).
const AUTO_JOIN: &str = "join|\u{0}";

/// Is an arrow of this group routed from its target (a join, named or automatic), so that the
/// arrows into one card share a trunk like a fork out of it?
pub fn turned(group: &str) -> bool {
    group.contains("|join|")
}

/// Plain arrows sharing one end (the source for a fork, the target for a join, `into`) and the
/// same classes, whose other ends lie beyond the shared one on one row. Edge id → group.
fn fans(model: &Model, boxes: &BTreeMap<String, Box2>, into: bool) -> BTreeMap<String, String> {
    let dir = model.option("dir").unwrap_or("down");
    let across = matches!(dir, "right" | "left");
    let flow = |b: &Box2| {
        if across {
            (b.x, b.right())
        } else {
            (b.y, b.bottom())
        }
    };
    let beyond = |p: &Box2, c: &Box2| match dir {
        "up" | "left" => flow(c).1 <= flow(p).0,
        _ => flow(c).0 >= flow(p).1,
    };
    let ends = |e| ends(e, into);
    // plain arrows, by shared end and classes; for forks between two children of a stack, also
    // against the flow (D-050)
    let mut groups: BTreeMap<(&str, String), Vec<&skemo_lang::Edge>> = BTreeMap::new();
    for e in &model.edges {
        let (Some(pb), Some(cb)) = (boxes.get(&e.from), boxes.get(&e.to)) else {
            continue;
        };
        let against = !into && against_stack(model, boxes, e);
        // cross links (`constraint="false"`) fan too, in groups of their own (D-056)
        // a label is allowed when the arrows of the group share it: it is written once (D-093)
        let plain = e.side.is_none()
            && e.via.is_none()
            && e.join.is_none()
            && model.node(ends(e).1).is_some_and(|n| n.kind == Kind::Card)
            // a wide card's arrows run straight to it, never as a fork (D-050)
            && !crate::stack::is_wide_card(model, &e.from)
            && !crate::stack::is_wide_card(model, &e.to)
            && (beyond(pb, cb) || against);
        if plain {
            let mut key = e.classes.join(".");
            if !e.label.is_empty() {
                key.push_str("|label:");
                key.push_str(&e.label);
            }
            if against {
                key.push_str(AGAINST);
            }
            if !e.constraint {
                key.push_str("|cross");
            }
            groups.entry((ends(e).0, key)).or_default().push(e);
        }
    }
    let mut out = BTreeMap::new();
    for ((shared, classes), edges) in groups {
        if edges.len() < 2 {
            continue;
        }
        let first = flow(&boxes[ends(edges[0]).1]);
        let one_row = edges.iter().all(|e| {
            let f = flow(&boxes[ends(e).1]);
            f.0.max(first.0) < f.1.min(first.1)
        });
        if one_row {
            let kind = if into { AUTO_JOIN } else { "fork" };
            let group = if classes.is_empty() {
                format!("{shared}|{kind}")
            } else {
                format!("{shared}|{kind}|{classes}")
            };
            for e in edges {
                out.insert(e.id.clone(), group.clone());
            }
        }
    }
    out
}

/// The shared end of an arrow in a fork (its source) or a join (its target, `into`), then the
/// other.
fn ends(e: &skemo_lang::Edge, into: bool) -> (&str, &str) {
    if into {
        (&e.to, &e.from)
    } else {
        (&e.from, &e.to)
    }
}

/// An arrow drawn through a bus (D-047): its group, its two waypoints on the bus, its sides.
pub(super) struct Bus {
    pub(super) group: String,
    pub(super) via: Vec<(f64, f64)>,
    pub(super) sides: (Side, Side),
}

/// Buses (D-047): when several cards of one row each fork to the same cards of the next row
/// (every one to every one, `auto_forks`), their arrows share one bar half-way between the rows
/// instead of crossing: down from each source to the bar, along it, down into each target, all
/// in one group so that they stay on top of each other. Only if no other card lies in the band
/// between the two rows over the bar's extent.
pub(super) fn auto_buses(
    model: &Model,
    boxes: &BTreeMap<String, Box2>,
    forks: &BTreeMap<String, String>,
) -> BTreeMap<String, Bus> {
    let dir = model.option("dir").unwrap_or("down");
    let across = matches!(dir, "right" | "left");
    let mid = |b: &Box2| {
        if across {
            b.y + b.h / 2.0
        } else {
            b.x + b.w / 2.0
        }
    };
    let point = |f: f64, a: f64| if across { (f, a) } else { (a, f) };
    let sides = (after_side(model), opposite(after_side(model)));
    // targets of each fork, by source and kind: a bus joins forks of one kind only, same classes
    // and label (the part of the fork's group after its source), never a solid line on a dashed
    // one
    let kind = |id: &str| {
        forks[id]
            .split_once("|fork")
            .map_or_else(String::new, |(_, k)| k.to_string())
    };
    let mut targets: BTreeMap<(&str, String), std::collections::BTreeSet<&str>> = BTreeMap::new();
    for e in model
        .edges
        .iter()
        .filter(|e| forks.get(&e.id).is_some_and(|g| !g.ends_with(AGAINST)))
    {
        targets
            .entry((e.from.as_str(), kind(&e.id)))
            .or_default()
            .insert(e.to.as_str());
    }
    let mut by_set: BTreeMap<(Vec<&str>, String), Vec<&str>> = BTreeMap::new();
    for ((s, k), t) in &targets {
        by_set
            .entry((t.iter().copied().collect(), k.clone()))
            .or_default()
            .push(s);
    }
    let mut out = BTreeMap::new();
    for ((tset, k), sources) in by_set {
        if sources.len() < 2 {
            continue;
        }
        let Some(bar) = bus_bar(model, boxes, &sources, &tset) else {
            continue;
        };
        let group = format!("bus|{}{k}", sources.join(" "));
        for e in model.edges.iter().filter(|e| forks.contains_key(&e.id)) {
            if !sources.contains(&e.from.as_str()) || kind(&e.id) != k {
                continue;
            }
            let via = vec![
                point(bar, mid(&boxes[&e.from])),
                point(bar, mid(&boxes[&e.to])),
            ];
            out.insert(
                e.id.clone(),
                Bus {
                    group: group.clone(),
                    via,
                    sides,
                },
            );
        }
    }
    out
}

/// Position of the bar of a bus along the drawing's direction: half-way between the row of the
/// sources and the row of the targets; `None` if the rows are too close, in the wrong order, or a
/// card lies in the band between them over the bar's extent.
fn bus_bar(
    model: &Model,
    boxes: &BTreeMap<String, Box2>,
    sources: &[&str],
    targets: &[&str],
) -> Option<f64> {
    let dir = model.option("dir").unwrap_or("down");
    let across = matches!(dir, "right" | "left");
    let reverse = matches!(dir, "up" | "left");
    let flow = |b: &Box2| {
        if across {
            (b.x, b.right())
        } else {
            (b.y, b.bottom())
        }
    };
    let span = |b: &Box2| {
        if across {
            (b.y, b.bottom())
        } else {
            (b.x, b.right())
        }
    };
    let sb: Vec<Box2> = sources.iter().map(|s| boxes[*s]).collect();
    let tb: Vec<Box2> = targets.iter().map(|t| boxes[*t]).collect();
    let max = |v: &mut dyn Iterator<Item = f64>| v.fold(f64::NEG_INFINITY, f64::max);
    let min = |v: &mut dyn Iterator<Item = f64>| v.fold(f64::INFINITY, f64::min);
    let (near, far) = if reverse {
        (
            min(&mut sb.iter().map(|b| flow(b).0)),
            max(&mut tb.iter().map(|b| flow(b).1)),
        )
    } else {
        (
            max(&mut sb.iter().map(|b| flow(b).1)),
            min(&mut tb.iter().map(|b| flow(b).0)),
        )
    };
    if (far - near).abs() < 24.0 || (far > near) == reverse {
        return None;
    }
    let mids: Vec<f64> = sb
        .iter()
        .chain(&tb)
        .map(|b| (span(b).0 + span(b).1) / 2.0)
        .collect();
    let (lo, hi) = (
        min(&mut mids.iter().copied()),
        max(&mut mids.iter().copied()),
    );
    let (b0, b1) = (near.min(far), near.max(far));
    let blocked = model.nodes.iter().any(|n| {
        n.kind == Kind::Card
            && !sources.contains(&n.id.as_str())
            && !targets.contains(&n.id.as_str())
            && boxes.get(&n.id).is_some_and(|b| {
                let ((f0, f1), (a0, a1)) = (flow(b), span(b));
                f0 < b1 && f1 > b0 && a0 < hi && a1 > lo
            })
    });
    (!blocked).then_some((near + far) / 2.0)
}
