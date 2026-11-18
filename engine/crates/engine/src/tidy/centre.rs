//! Centring, like a tree drawn by hand: a lone zone on what it links to, a parent on its
//! children, a child under its parents. A node moves only across the flow, and only if the place
//! is free (`fits`).

use super::geom::{
    Axes, Boxes, constrained, extent, fits, in_grid, is_card, only_parent, rows, within,
};
use crate::layout::Box2;
use skemo_lang::{Edge, Kind, Model};

/// A top-level zone alone on its row is centred, across the direction, on the span of
/// the nodes it is linked to (its own arrows and those of what it contains). ELK's hierarchical
/// placement leaves such a zone against one side of its targets ("Services" above two workers).
/// It moves only if the place is free (`fits`): alone on its row as ELK placed it, a zone may
/// still meet a card an earlier pass moved beside it (bug 8: a leaf lifted next to it). Two
/// passes, so that a node linked to a moved one follows it.
pub fn center_lonely(model: &Model, boxes: &mut Boxes) {
    let ax = Axes::of(model);
    let tops: Vec<&str> = model.children(None).map(|n| n.id.as_str()).collect();
    for _ in 0..2 {
        for row in rows(&tops, boxes, ax, 1) {
            let [id] = row[..] else { continue };
            if model.node(id).is_none_or(|n| n.kind != Kind::Zone) {
                continue;
            }
            let (lo, hi) = extent(model.edges.iter().filter(|e| e.constraint).filter_map(|e| {
                let other = match (within(model, &e.from, id), within(model, &e.to, id)) {
                    (true, false) => &e.to,
                    (false, true) => &e.from,
                    _ => return None,
                };
                boxes.get(other.as_str()).map(|b| ax.span(b))
            }));
            let Some(b) = boxes.get(id) else {
                continue;
            };
            if !lo.is_finite() {
                continue;
            }
            let d = (lo + hi) / 2.0 - ax.mid(b);
            let inside: Vec<&str> = model
                .nodes
                .iter()
                .filter(|n| within(model, &n.id, id))
                .map(|n| n.id.as_str())
                .collect();
            if d.abs() > 0.5 && fits(model, id, &ax.moved(b, d), &inside, boxes) {
                ax.shift_across(model, id, d, boxes);
            }
        }
    }
}

/// Parents centred on their children (D-047), like a tree drawn by hand: a card whose children
/// (cards it links to, at least two, on one row beyond it in the drawing's direction, with no
/// other parent) are spread under it moves across to the middle of their span; a card with one
/// child it is the only parent of stands straight in line with it (a spine, D-048), beside the
/// title of a zone the arrow enters; otherwise it moves onto the child in its column; each time
/// if that place is free and its zones stay clear of their neighbours. Children are handled
/// before their parents, so that a chain of single parents stays straight. Arrows outside the
/// layout (`constraint="false"`) do not count; cells of a `wrap` grid stay in the grid.
pub fn center_parents(model: &Model, boxes: &mut Boxes, titles: &[(String, Box2)]) {
    let ax = Axes::of(model);
    let edges = constrained(model);
    let mut cards: Vec<&skemo_lang::Node> = model
        .nodes
        .iter()
        .filter(|n| n.kind == Kind::Card && !in_grid(model, &n.id))
        .collect();
    // children first: the farthest along the drawing's direction
    let start = |id: &str| boxes.get(id).map_or(0.0, |b| ax.flow(b).0);
    cards.sort_by(|a, b| ax.upstream_first(start(&a.id), start(&b.id)).reverse());
    for p in cards {
        let Some(pb) = boxes.get(&p.id).copied() else {
            continue;
        };
        let Some((lo, hi)) = parent_target(model, &edges, titles, ax, (&p.id, &pb), boxes) else {
            continue;
        };
        let d = (lo + hi) / 2.0 - ax.mid(&pb);
        if d.abs() < 0.5 {
            continue;
        }
        let moved = ax.moved(&pb, d);
        if fits(model, &p.id, &moved, &[p.id.as_str()], boxes) {
            boxes.insert(p.id.clone(), moved);
        }
    }
}

/// A card linked one-to-one to a card inside a zone it is not in (Internet above a load balancer in
/// a subnet), centred on it again once the zones have made room for their titles (D-064): before,
/// the straight arrow would have crossed a title (`clear_of_titles`), and the card stayed where ELK
/// put it, the arrow bending. `titles`: the zone titles on the widened frames. The card moves
/// only if the place is free and `framed` accepts it (its zones, framed by their own rule, come
/// no nearer their neighbours).
pub fn spines_into_zones(
    model: &Model,
    boxes: &mut Boxes,
    titles: &[(String, Box2)],
    framed: &dyn Fn(&str, &Box2, &Boxes) -> bool,
) {
    let ax = Axes::of(model);
    let edges = constrained(model);
    let zones_of = |id: &str| {
        let mut out = Vec::new();
        let mut n = model.node(id).and_then(|n| n.parent.as_deref());
        while let Some(z) = n {
            out.push(z);
            n = model.node(z).and_then(|n| n.parent.as_deref());
        }
        out
    };
    for p in model
        .nodes
        .iter()
        .filter(|n| n.kind == Kind::Card && !in_grid(model, &n.id))
    {
        let out: Vec<&&Edge> = edges.iter().filter(|e| e.from == p.id).collect();
        let [e] = out[..] else { continue };
        let (Some(pb), Some(cb)) = (boxes.get(&p.id).copied(), boxes.get(&e.to).copied()) else {
            continue;
        };
        let entered = zones_of(&e.to).iter().any(|z| !within(model, &p.id, z));
        if !is_card(model, &e.to) || !entered || !only_parent(&edges, &p.id, &e.to) {
            continue;
        }
        if !ax.beyond(&pb, &cb) {
            continue;
        }
        let Some(m) = clear_of_titles(model, titles, ax, (&p.id, &e.to), &cb) else {
            continue;
        };
        let d = m - ax.mid(&pb);
        let moved = ax.moved(&pb, d);
        if d.abs() >= 0.5
            && fits(model, &p.id, &moved, &[p.id.as_str()], boxes)
            && framed(&p.id, &moved, boxes)
        {
            boxes.insert(p.id.clone(), moved);
        }
    }
}

/// The span (across the flow) parent `p` (box `pb`) should be centred on, or `None`.
fn parent_target(
    model: &Model,
    edges: &[&Edge],
    titles: &[(String, Box2)],
    ax: Axes,
    (p, pb): (&str, &Box2),
    boxes: &Boxes,
) -> Option<(f64, f64)> {
    // (child box, this card is its only parent)
    let linked: Vec<(Box2, bool)> = edges
        .iter()
        .filter(|e| e.from == p)
        .filter_map(|e| {
            let cb = boxes.get(&e.to)?;
            (is_card(model, &e.to) && ax.beyond(pb, cb))
                .then_some((*cb, only_parent(edges, p, &e.to)))
        })
        .collect();
    let kids: Vec<Box2> = linked.iter().filter(|k| k.1).map(|k| k.0).collect();
    // at least two own children, all on the row of the first one: their middle
    if kids.len() >= 2 && kids.iter().all(|k| ax.same_row(k, &kids[0])) {
        return Some(extent(kids.iter().map(|b| ax.span(b))));
    }
    if let [(b, true)] = linked[..] {
        // a one-to-one link: the arrow is straight, as by hand (a spine), beside the title of a
        // zone it enters
        let m = clear_of_titles(model, titles, ax, (p, &linked_id(model, edges, p)), &b)?;
        return Some((m, m));
    }
    // otherwise the child in its column (the straight arrow of a hand drawing); not a child
    // another of its parents on this card's row stands in front of too: that child is centred on
    // its parents (`center_children`), and one parent moving alone would break the even spacing
    // of the row
    let cross =
        |a: &Box2, b: &Box2| ax.span(a).1.min(ax.span(b).1) - ax.span(a).0.max(ax.span(b).0);
    let overlap = |b: &Box2| cross(b, pb);
    let row_mates = |child: &str| {
        let Some(cb) = boxes.get(child) else {
            return false;
        };
        edges.iter().any(|e| {
            e.to == child
                && e.from != p
                && boxes
                    .get(&e.from)
                    .is_some_and(|q| ax.same_row(q, pb) && cross(q, cb) > 0.0)
        })
    };
    edges
        .iter()
        .filter(|e| e.from == p && is_card(model, &e.to) && !row_mates(&e.to))
        .filter_map(|e| boxes.get(&e.to))
        .filter(|b| ax.beyond(pb, b) && overlap(b) > 0.0)
        .max_by(|a, b| overlap(a).total_cmp(&overlap(b)))
        .map(|b| ax.span(b))
}

/// The only constrained child of `p` (the one-to-one case of `center_parents`).
fn linked_id(model: &Model, edges: &[&Edge], p: &str) -> String {
    edges
        .iter()
        .find(|e| e.from == p && is_card(model, &e.to))
        .map(|e| e.to.clone())
        .unwrap_or_default()
}

/// Where a straight arrow from `parent` may enter `child` (box `b`), across the flow: its middle,
/// or, when the title of a zone the arrow enters lies there, just past that title, if that is
/// still well inside the card. `None` when no such place exists.
fn clear_of_titles(
    model: &Model,
    titles: &[(String, Box2)],
    ax: Axes,
    (parent, child): (&str, &str),
    b: &Box2,
) -> Option<f64> {
    let (lo, hi) = ax.span(b);
    let mut m = (lo + hi) / 2.0;
    for (z, t) in titles {
        if !within(model, child, z) || within(model, parent, z) {
            continue;
        }
        let (a, e) = ax.span(t);
        if m > a - 6.0 && m < e + 6.0 {
            m = e + 12.0;
        }
    }
    (m <= hi - 12.0).then_some(m)
}

/// Cards centred under their parents, top-down, like a hand drawing (D-048, R2): a card fed by
/// two or more cards of one row before it sits in the middle of their span (Core between two
/// uplinks, an API under two web servers); the children a card is the only parent of, on one row,
/// are shifted together under it when it has moved (a spine and its fork stay centred). Each move
/// happens only if the place is free and inside the card's zone. Arrows outside the layout
/// (`constraint="false"`) do not count.
pub fn center_children(model: &Model, boxes: &mut Boxes) {
    let ax = Axes::of(model);
    let edges = constrained(model);
    let mut cards: Vec<&str> = model
        .nodes
        .iter()
        .filter(|n| n.kind == Kind::Card && boxes.contains_key(&n.id))
        .map(|n| n.id.as_str())
        .collect();
    // parents first: the nearest to the start of the drawing
    cards.sort_by(|a, b| ax.upstream_first(ax.flow(&boxes[*a]).0, ax.flow(&boxes[*b]).0));
    // a card may be held by a neighbour that moves later on its row: up to three rounds, until
    // nothing moves
    for _ in 0..3 {
        let round = boxes.clone();
        for id in &cards {
            center_under_parents(model, &edges, ax, id, boxes);
        }
        for id in &cards {
            under_trunk(model, &edges, ax, id, boxes);
            center_own_children(model, &edges, ax, id, boxes);
        }
        if *boxes == round {
            break;
        }
    }
}

/// Boxes of the cards linked to `id` from one row before it (or after it, `out`); `None` unless
/// they all lie on one row.
fn row_neighbours(
    model: &Model,
    edges: &[&Edge],
    ax: Axes,
    (id, out): (&str, bool),
    boxes: &Boxes,
) -> Option<Vec<(String, Box2)>> {
    let b = boxes.get(id)?;
    let mut seen = Vec::new();
    let mut found: Vec<(String, Box2)> = Vec::new();
    for e in edges {
        let (me, other) = if out {
            (&e.from, &e.to)
        } else {
            (&e.to, &e.from)
        };
        if me != id || seen.contains(other) {
            continue;
        }
        seen.push(other.clone());
        let ob = boxes.get(other.as_str())?;
        let ordered = if out {
            ax.beyond(b, ob)
        } else {
            ax.beyond(ob, b)
        };
        if !ordered || !is_card(model, other) {
            return None;
        }
        found.push((other.clone(), *ob));
    }
    let first = found.first()?.1;
    found
        .iter()
        .all(|(_, k)| ax.same_row(k, &first))
        .then_some(found)
}

/// The child of a fork that its parent's trunk falls on (D-091): the middle of three children,
/// a child standing almost under its parent. It moves across so that the trunk runs straight into
/// its middle, as a hand drawing does: one junction on the bar, and its arrow enters the middle
/// of its side. Only a child this card is the only parent of, if the place is free.
fn under_trunk(model: &Model, edges: &[&Edge], ax: Axes, id: &str, boxes: &mut Boxes) {
    let Some(kids) = row_neighbours(model, edges, ax, (id, true), boxes) else {
        return;
    };
    if kids.len() < 2 {
        return;
    }
    let trunk = ax.mid(&boxes[id]);
    for (c, cb) in kids {
        let (lo, hi) = ax.span(&cb);
        if in_grid(model, &c) || !only_parent(edges, id, &c) || trunk <= lo || trunk >= hi {
            continue;
        }
        let d = trunk - ax.mid(&cb);
        // its own children follow it, so that its fork stays centred; the move is made only if
        // they all find room
        let own = row_neighbours(model, edges, ax, (&c, true), boxes).unwrap_or_default();
        if d.abs() < 0.5 || own.iter().any(|(k, _)| !only_parent(edges, &c, k)) {
            continue;
        }
        let moving: Vec<(String, Box2)> = std::iter::once((c.clone(), cb)).chain(own).collect();
        let ids: Vec<&str> = moving.iter().map(|(k, _)| k.as_str()).collect();
        if moving
            .iter()
            .all(|(k, b)| fits(model, k, &ax.moved(b, d), &ids, boxes))
        {
            for (k, b) in &moving {
                boxes.insert(k.clone(), ax.moved(b, d));
            }
        }
    }
}

/// A card fed by two or more parents of one row, each with no other child, moves to the middle
/// of their span.
fn center_under_parents(model: &Model, edges: &[&Edge], ax: Axes, id: &str, boxes: &mut Boxes) {
    if in_grid(model, id) {
        return;
    }
    let Some(parents) = row_neighbours(model, edges, ax, (id, false), boxes) else {
        return;
    };
    // only parents whose one child is this card (not a bus feeding several cards)
    let sole = parents
        .iter()
        .all(|(p, _)| edges.iter().all(|e| e.from != *p || e.to == id));
    if parents.len() < 2 || !sole {
        return;
    }
    let (lo, hi) = extent(parents.iter().map(|(_, b)| ax.span(b)));
    let b = boxes[id];
    let d = (lo + hi) / 2.0 - ax.mid(&b);
    let m = ax.moved(&b, d);
    if d.abs() >= 0.5 && fits(model, id, &m, &[id], boxes) {
        boxes.insert(id.to_string(), m);
    }
}

/// The children card `id` is the only parent of, on one row, move together under it.
fn center_own_children(model: &Model, edges: &[&Edge], ax: Axes, id: &str, boxes: &mut Boxes) {
    let Some(kids) = row_neighbours(model, edges, ax, (id, true), boxes) else {
        return;
    };
    // only children this card is the sole parent of, not cells of a grid
    if kids.is_empty()
        || kids.iter().any(|(k, _)| !only_parent(edges, id, k))
        || kids.iter().any(|(k, _)| in_grid(model, k))
    {
        return;
    }
    let (lo, hi) = extent(kids.iter().map(|(_, b)| ax.span(b)));
    let d = ax.mid(&boxes[id]) - (lo + hi) / 2.0;
    if d.abs() < 0.5 {
        return;
    }
    let ids: Vec<&str> = kids.iter().map(|(k, _)| k.as_str()).collect();
    let free = kids
        .iter()
        .all(|(k, b)| fits(model, k, &ax.moved(b, d), &ids, boxes));
    if free {
        for (k, b) in &kids {
            boxes.insert(k.clone(), ax.moved(b, d));
        }
    }
}

/// Largest hook of the triangle rule (`triangle_hooks`): beyond it, a long name would push the
/// middle card far aside, and the layout stays as the other passes left it.
const HOOK_MAX: f64 = 48.0;

/// D-099 (the user's choice 4b, 2026-11-09): a triangle `a → b`, `a → c`, `b → c`, laid in one
/// column (`b` after `a`, `c` after `b`), is drawn with its long arrow straight and two equal
/// hooks on the way through `b`. Both lines leave `a` from its middle half, mirror images around
/// its middle (D-092), and both enter `c` the same way: `c` stands so that `a → c` runs straight
/// from port to port, and `b` stands beside that line, the router's margin away, as close to its
/// own port as the line allows. Then `a → b` steps aside by some amount and `b → c` steps back by
/// as much: the steps mirror each other around the column's middle. When `b` is narrow enough to
/// stand under its port, both lines through it are straight. Only for that pattern: `b` linked
/// to `a` and `c` only, `c` entered by `a` and `b` only, `a` leaving forward to `b` and `c` only;
/// no grid; each move only if the place is free (`fits`), and never a hook
/// longer than `HOOK_MAX`.
pub fn triangle_hooks(model: &Model, boxes: &mut Boxes) {
    let ax = Axes::of(model);
    let edges = constrained(model);
    let p = crate::route::graph_params(model);
    // the gap between the two ports of a side (the router's `even_targets` on a free middle half)
    let spread = |w: f64| {
        (w / 2.0)
            .min(p.port_spread)
            .max(p.ideal_nudging.max(p.port_spacing))
    };
    let free = |id: &str, boxes: &Boxes| {
        model.node(id).is_some_and(|n| n.kind == Kind::Card)
            && !in_grid(model, id)
            && boxes.contains_key(id)
    };
    let into = |id: &str| -> Vec<&str> {
        edges
            .iter()
            .filter(|e| e.to == id)
            .map(|e| e.from.as_str())
            .collect()
    };
    let from = |id: &str| -> Vec<&str> {
        edges
            .iter()
            .filter(|e| e.from == id)
            .map(|e| e.to.as_str())
            .collect()
    };
    let same = |mut v: Vec<&str>, w: &[&str]| {
        v.sort_unstable();
        let mut w = w.to_vec();
        w.sort_unstable();
        v == w
    };
    for n in &model.nodes {
        let a = n.id.as_str();
        if !boxes.contains_key(a) || n.kind != Kind::Card {
            continue;
        }
        let ab = boxes[a];
        let ahead: Vec<&str> = from(a)
            .into_iter()
            .filter(|t| boxes.get(*t).is_some_and(|tb| ax.beyond(&ab, tb)))
            .collect();
        if ahead.len() != 2 {
            continue;
        }
        for (b, c) in [(ahead[0], ahead[1]), (ahead[1], ahead[0])] {
            if !free(b, boxes) || !free(c, boxes) {
                continue;
            }
            if !same(into(b), &[a]) || !same(from(b), &[c]) || !same(into(c), &[a, b]) {
                continue;
            }
            let (bb, cb) = (boxes[b], boxes[c]);
            let overlap = |x: &Box2, y: &Box2| {
                let (x, y) = (ax.span(x), ax.span(y));
                x.0 < y.1 && y.0 < x.1
            };
            if !ax.beyond(&bb, &cb)
                || !overlap(&ab, &bb)
                || !overlap(&bb, &cb)
                || !overlap(&ab, &cb)
            {
                continue;
            }
            let (sa, sc) = (spread(ax.span_len(&ab)), spread(ax.span_len(&cb)));
            // `b` keeps the side it is on; the straight line `a → c` runs on the other one
            let s = if ax.mid(&bb) <= ax.mid(&cb) {
                1.0
            } else {
                -1.0
            };
            let line = ax.mid(&ab) + s * sa / 2.0;
            let port_b = ax.mid(&ab) - s * sa / 2.0;
            let mut hook = ax.span_len(&bb) / 2.0 + p.shape_buffer - sa;
            if hook <= 0.0 {
                hook = 0.0;
            } else {
                hook = hook.max(p.min_jog);
            }
            if hook > HOOK_MAX {
                continue;
            }
            let db = port_b - s * hook - ax.mid(&bb);
            let dc = line - s * sc / 2.0 - ax.mid(&cb);
            let (nb, nc) = (ax.moved(&bb, db), ax.moved(&cb, dc));
            let group = [b, c];
            if fits(model, b, &nb, &group, boxes) && fits(model, c, &nc, &group, boxes) {
                boxes.insert(b.to_string(), nb);
                boxes.insert(c.to_string(), nc);
                break;
            }
        }
    }
}
