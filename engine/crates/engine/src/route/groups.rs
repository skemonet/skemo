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
    // plain arrows, by source and classes; between two children of a stack, also against the
    // flow (D-050)
    let mut groups: BTreeMap<(&str, String), Vec<&skemo_lang::Edge>> = BTreeMap::new();
    for e in &model.edges {
        let (Some(pb), Some(cb)) = (boxes.get(&e.from), boxes.get(&e.to)) else {
            continue;
        };
        // cross links (`constraint="false"`) fork too, in groups of their own (D-056)
        let plain = e.label.is_empty()
            && e.side.is_none()
            && e.via.is_none()
            && e.join.is_none()
            && model.node(&e.to).is_some_and(|n| n.kind == Kind::Card)
            // a wide card's arrows run straight to it, never as a fork (D-050)
            && !crate::stack::is_wide_card(model, &e.from)
            && !crate::stack::is_wide_card(model, &e.to)
            && (beyond(pb, cb) || against_stack(model, boxes, e));
        if plain {
            let mut key = e.classes.join(".");
            if against_stack(model, boxes, e) {
                key.push_str(AGAINST);
            }
            if !e.constraint {
                key.push_str("|cross");
            }
            groups.entry((e.from.as_str(), key)).or_default().push(e);
        }
    }
    let mut out = BTreeMap::new();
    for ((from, classes), edges) in groups {
        if edges.len() < 2 {
            continue;
        }
        let first = flow(&boxes[&edges[0].to]);
        let one_row = edges.iter().all(|e| {
            let f = flow(&boxes[&e.to]);
            f.0.max(first.0) < f.1.min(first.1)
        });
        if one_row {
            let group = if classes.is_empty() {
                format!("{from}|fork")
            } else {
                format!("{from}|fork|{classes}")
            };
            for e in edges {
                out.insert(e.id.clone(), group.clone());
            }
        }
    }
    out
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
    // targets of each forking source
    let mut targets: BTreeMap<&str, std::collections::BTreeSet<&str>> = BTreeMap::new();
    for e in model
        .edges
        .iter()
        .filter(|e| forks.get(&e.id).is_some_and(|g| !g.ends_with(AGAINST)))
    {
        targets
            .entry(e.from.as_str())
            .or_default()
            .insert(e.to.as_str());
    }
    let mut by_set: BTreeMap<Vec<&str>, Vec<&str>> = BTreeMap::new();
    for (s, t) in &targets {
        by_set
            .entry(t.iter().copied().collect())
            .or_default()
            .push(s);
    }
    let mut out = BTreeMap::new();
    for (tset, sources) in by_set {
        if sources.len() < 2 {
            continue;
        }
        let Some(bar) = bus_bar(model, boxes, &sources, &tset) else {
            continue;
        };
        let group = format!("bus|{}", sources.join(" "));
        for e in model.edges.iter().filter(|e| forks.contains_key(&e.id)) {
            if !sources.contains(&e.from.as_str()) {
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
