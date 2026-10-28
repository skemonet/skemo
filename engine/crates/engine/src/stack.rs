//! Stacks and wide cards (D-050). A `stack` lays its children one after the other in writing
//! order along `dir`, whatever the arrows between them; a `.wide` card spans its group across
//! `dir`, and its arrows run straight to it.

use crate::layout::{Box2, Placement};
use crate::measure::Sizes;
use crate::route::Points;
use crate::tidy::geom::Axes;
use skemo_lang::{Kind, Model};
use std::collections::BTreeMap;

/// Gap between two children of a stack (px), the rhythm of rows (D-048).
pub const GAP: f64 = 48.0;

/// `true` for a `.wide` card or zone.
pub fn is_wide(model: &Model, id: &str) -> bool {
    model
        .node(id)
        .is_some_and(|n| n.classes.iter().any(|c| c == "wide"))
}

/// `true` for a `.wide` card (its arrows run straight to it).
pub fn is_wide_card(model: &Model, id: &str) -> bool {
    is_wide(model, id) && model.node(id).is_some_and(|n| n.kind == Kind::Card)
}

/// Gap between the cards of a wide zone's row (px).
const ROW_GAP: f64 = 40.0;

/// The node itself, then its ancestors.
fn chain<'a>(model: &'a Model, id: &'a str) -> Vec<&'a str> {
    std::iter::once(id)
        .chain(model.ancestors(id).into_iter().map(|n| n.id.as_str()))
        .collect()
}

/// The stack that holds the two ends of an arrow in two different children, if any.
pub fn across_stack<'a>(model: &'a Model, e: &skemo_lang::Edge) -> Option<&'a str> {
    let to = chain(model, &e.to);
    let common = chain(model, &e.from).into_iter().find(|a| to.contains(a))?;
    let n = model.node(common)?;
    (n.stack && common != e.from && common != e.to).then_some(n.id.as_str())
}

/// The child of `stack` that holds `id`, by its rank in writing order.
fn rank_in(model: &Model, stack: &str, id: &str) -> Option<usize> {
    let c = chain(model, id);
    model
        .children(Some(stack))
        .position(|k| c.contains(&k.id.as_str()))
}

/// The model as the layout sees it: an arrow between two children of a stack that goes against
/// the stack's written order is turned round, so that the layout aligns the cards it links
/// (a service under its pods) while the stack's order holds. Drawing keeps its direction.
pub fn layout_view(model: &mut Model) {
    let turned: Vec<String> = model
        .edges
        .iter()
        .filter(|e| {
            across_stack(model, e)
                .is_some_and(|s| rank_in(model, s, &e.from) > rank_in(model, s, &e.to))
        })
        .map(|e| e.id.clone())
        .collect();
    for e in model.edges.iter_mut().filter(|e| turned.contains(&e.id)) {
        std::mem::swap(&mut e.from, &mut e.to);
    }
}

/// The representative of `i` in a union of groups.
fn root(g: &mut [usize], i: usize) -> usize {
    let mut r = i;
    while g[r] != r {
        r = g[r];
    }
    g[i] = r;
    r
}

/// The children of a stack (ranks into `kids`) linked by arrows, in groups: each group keeps the
/// layout's alignment between its children and is centred as a whole.
fn linked_groups(model: &Model, stack: &str, kids: &[&str]) -> Vec<Vec<usize>> {
    let mut group: Vec<usize> = (0..kids.len()).collect();
    let rank = |id: &str| {
        let c = chain(model, id);
        kids.iter().position(|k| c.contains(k))
    };
    for e in model.edges.iter().filter(|e| e.constraint) {
        if across_stack(model, e) != Some(stack) {
            continue;
        }
        if let (Some(a), Some(b)) = (rank(&e.from), rank(&e.to)) {
            let (ra, rb) = (root(&mut group, a), root(&mut group, b));
            group[ra] = rb;
        }
    }
    let mut out: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for i in 0..kids.len() {
        let r = root(&mut group, i);
        out.entry(r).or_default().push(i);
    }
    out.into_values().collect()
}

/// Depth of a node in the tree of groups.
fn depth(model: &Model, id: &str) -> usize {
    model.ancestors(id).len()
}

/// Lays the children of every stack one after the other (D-050), deepest stacks first: in
/// writing order along `dir` (reversed with `up` and `left`, so that the first child is at the
/// top or on the left as written), `GAP` apart. Across, the children linked by arrows keep the
/// layout's alignment and each such group is centred on the widest; wide cards take the whole
/// width. A child is moved with everything inside it; the layout's hints of the arrows that
/// leave a moved child are dropped (the router traces them anew).
pub fn arrange(model: &Model, placement: &mut Placement) {
    let ax = Axes::of(model);
    let mut stacks: Vec<&str> = model
        .nodes
        .iter()
        .filter(|n| n.stack && n.at.is_none())
        .map(|n| n.id.as_str())
        .collect();
    stacks.sort_by_key(|s| std::cmp::Reverse(depth(model, s)));
    for s in stacks {
        let mut kids: Vec<&str> = model
            .children(Some(s))
            .filter(|c| placement.boxes.contains_key(&c.id))
            .map(|c| c.id.as_str())
            .collect();
        if kids.is_empty() {
            continue;
        }
        if ax.reverse {
            kids.reverse();
        }
        let start = kids
            .iter()
            .map(|k| ax.split(&placement.boxes[*k]))
            .fold((f64::MAX, f64::MAX), |(f, c), ((kf, _), (kc, _))| {
                (f.min(kf), c.min(kc))
            });
        // across: each group of linked children as a block, wide cards left out (they take the
        // whole width)
        let groups = linked_groups(model, s, &kids);
        let span = |g: &[usize]| {
            g.iter()
                .filter(|i| !is_wide(model, kids[**i]))
                .map(|i| ax.split(&placement.boxes[kids[*i]]).1)
                .fold((f64::MAX, f64::MIN), |(lo, hi), (c, cs)| {
                    (lo.min(c), hi.max(c + cs))
                })
        };
        let widest = groups
            .iter()
            .map(|g| span(g))
            .filter(|(lo, hi)| hi > lo)
            .map(|(lo, hi)| hi - lo)
            .fold(0.0, f64::max);
        let mut across_shift = vec![0.0; kids.len()];
        for g in &groups {
            let (lo, hi) = span(g);
            if hi > lo {
                let to = start.1 + (widest - (hi - lo)) / 2.0;
                for i in g {
                    across_shift[*i] = to - lo;
                }
            }
        }
        let mut cursor = start.0;
        for (i, k) in kids.iter().enumerate() {
            let ((f, fs), (c, _)) = ax.split(&placement.boxes[*k]);
            let to_c = if is_wide(model, k) || widest == 0.0 {
                start.1
            } else {
                c + across_shift[i]
            };
            let d = if ax.across {
                (cursor - f, to_c - c)
            } else {
                (to_c - c, cursor - f)
            };
            shift(model, placement, k, d);
            if is_wide_card(model, k) && widest > 0.0 {
                let b = placement.boxes[*k];
                let (fl, _) = ax.split(&b);
                placement
                    .boxes
                    .insert((*k).to_string(), ax.join(fl, (start.1, widest)));
            }
            cursor += fs + GAP;
        }
        let size = (cursor - GAP - start.0, widest.max(1.0));
        placement
            .boxes
            .insert(s.to_string(), ax.join((start.0, size.0), (start.1, size.1)));
    }
}

/// Moves a node and everything inside it by `(dx, dy)`.
fn shift(model: &Model, placement: &mut Placement, id: &str, (dx, dy): (f64, f64)) {
    if dx.abs() < 1e-9 && dy.abs() < 1e-9 {
        return;
    }
    let inside: Vec<String> = model
        .nodes
        .iter()
        .filter(|n| chain(model, &n.id).contains(&id))
        .map(|n| n.id.clone())
        .collect();
    for n in &inside {
        if let Some(b) = placement.boxes.get_mut(n) {
            b.x += dx;
            b.y += dy;
        }
    }
    // hints: moved with an arrow wholly inside, dropped for one that leaves
    for e in &model.edges {
        let (a, b) = (inside.contains(&e.from), inside.contains(&e.to));
        if a && b {
            if let Some((pts, label)) = placement.routes.get_mut(&e.id) {
                for p in pts.iter_mut() {
                    p.0 += dx;
                    p.1 += dy;
                }
                if let Some(l) = label {
                    l.0 += dx;
                    l.1 += dy;
                }
            }
        } else if a || b {
            placement.routes.remove(&e.id);
        }
    }
}

/// Wide cards and zones span their group across `dir` (D-050): the other children of their zone
/// or stack, or every other top-level node. A wide zone's cards on one row share its width,
/// `ROW_GAP` apart, when that makes none of them narrower. Set on the placement and on the
/// measured sizes. `pad`: a zone's margin.
pub fn stretch(
    model: &Model,
    placement: &mut Placement,
    sizes: &mut Sizes,
    (pad, head, cards_only): (f64, f64, bool),
) {
    let ax = Axes::of(model);
    // a zone's extent is its frame (its cards and margins), as drawn
    let frames = crate::layout::zone_frames(model, &placement.boxes, placement, pad, head);
    for n in model
        .nodes
        .iter()
        .filter(|n| is_wide(model, &n.id) && n.at.is_none())
    {
        let Some(b) = placement.boxes.get(&n.id).copied() else {
            continue;
        };
        let extent_of = |wide: bool| -> Vec<Box2> {
            model
                .children(n.parent.as_deref())
                .filter(|c| c.id != n.id && is_wide(model, &c.id) == wide)
                .filter_map(|c| frames.get(&c.id).or(placement.boxes.get(&c.id)).copied())
                .collect()
        };
        // the group's other nodes; when they are all wide too, their own extent (D-058)
        let mut others = extent_of(false);
        if others.is_empty() {
            others = extent_of(true);
        }
        if others.is_empty() {
            continue;
        }
        let lo = others
            .iter()
            .map(|o| ax.split(o).1.0)
            .fold(f64::MAX, f64::min);
        let hi = others
            .iter()
            .map(|o| {
                let (_, (c, cs)) = ax.split(o);
                c + cs
            })
            .fold(f64::MIN, f64::max);
        let (fl, _) = ax.split(&b);
        let wide = ax.join(fl, (lo, hi - lo));
        if n.kind == Kind::Zone && cards_only {
            continue;
        }
        if n.kind == Kind::Zone {
            let inset = if crate::layout::invisible(n) {
                0.0
            } else {
                pad
            };
            fill(model, placement, sizes, &n.id, (lo + inset, hi - inset));
            continue;
        }
        placement.boxes.insert(n.id.clone(), wide);
        if let Some(s) = sizes.cards.get_mut(&n.id) {
            s.w = wide.w;
            s.h = wide.h;
        }
    }
}

/// The cards of a zone, on one row, spread over `(lo, hi)` across `dir`: equal widths (even, so
/// that a pinned card keeps its straight arrows), `ROW_GAP` apart.
fn fill(
    model: &Model,
    placement: &mut Placement,
    sizes: &mut Sizes,
    zone: &str,
    (lo, hi): (f64, f64),
) {
    let ax = Axes::of(model);
    let mut cards: Vec<(String, Box2)> = model
        .children(Some(zone))
        .filter(|c| c.kind == Kind::Card && c.at.is_none())
        .filter_map(|c| placement.boxes.get(&c.id).map(|b| (c.id.clone(), *b)))
        .collect();
    if cards.is_empty() {
        return;
    }
    let first = ax.split(&cards[0].1).0;
    let one_row = cards.iter().all(|(_, b)| {
        let (f, fs) = ax.split(b).0;
        f < first.0 + first.1 && f + fs > first.0
    });
    let n = cards.len() as f64;
    // cards that carry columns (D-054) are each as wide as the widest block of any of their
    // columns, `COL_GAP` apart like the blocks, so that every block fits over its card
    let need = column_needs(model, placement);
    let carried: Vec<f64> = cards
        .iter()
        .filter_map(|(id, _)| model.node(id).and_then(column_of))
        .filter_map(|c| need.get(c).copied())
        .collect();
    let gap = if carried.is_empty() { ROW_GAP } else { COL_GAP };
    let fits = (((hi - lo - (n - 1.0) * gap) / n) / 2.0).floor() * 2.0;
    let each = fits.max((carried.iter().fold(0.0, |a: f64, b| a.max(*b)) / 2.0).ceil() * 2.0);
    let widest = cards
        .iter()
        .map(|(_, b)| ax.split(b).1.1)
        .fold(0.0, f64::max);
    if !one_row || each < widest {
        return;
    }
    cards.sort_by(|a, b| ax.split(&a.1).1.0.total_cmp(&ax.split(&b.1).1.0));
    let used = n * each + (n - 1.0) * gap;
    let mut c = lo + ((hi - lo) - used) / 2.0;
    for (id, b) in cards {
        let (fl, _) = ax.split(&b);
        let nb = ax.join(fl, (c, each));
        placement.boxes.insert(id.clone(), nb);
        if let Some(s) = sizes.cards.get_mut(&id) {
            s.w = nb.w;
            s.h = nb.h;
        }
        c += each + gap;
    }
}

/// The width each column needs (D-054): its widest block, outside the `.wide` zone that carries
/// it.
fn column_needs<'a>(model: &'a Model, placement: &Placement) -> BTreeMap<&'a str, f64> {
    let ax = Axes::of(model);
    let mut cols: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for n in model.nodes.iter().filter(|n| n.at.is_none()) {
        if let Some(c) = column_of(n)
            && placement.boxes.contains_key(&n.id)
        {
            cols.entry(c).or_default().push(n.id.as_str());
        }
    }
    let mut need: BTreeMap<&str, f64> = BTreeMap::new();
    for b in blocks(ax, &cols, &placement.boxes) {
        if !anchored(model, &b.ids) {
            let w = need.entry(b.col).or_insert(0.0);
            *w = w.max(b.span.1 - b.span.0);
        }
    }
    need
}

/// Arrows to or from a wide card run straight (D-050): from the middle of the other card's
/// facing side to the bar, when the other card stands over the bar (10 px from its ends) and no
/// card lies between them.
pub fn plug(model: &Model, boxes: &BTreeMap<String, Box2>, routes: &mut BTreeMap<String, Points>) {
    let ax = Axes::of(model);
    for e in &model.edges {
        let (wide, other, from_wide) =
            if is_wide_card(model, &e.from) && !is_wide_card(model, &e.to) {
                (&e.from, &e.to, true)
            } else if is_wide_card(model, &e.to) && !is_wide_card(model, &e.from) {
                (&e.to, &e.from, false)
            } else {
                continue;
            };
        let (Some(w), Some(o)) = (boxes.get(wide), boxes.get(other)) else {
            continue;
        };
        let ((wf, wfs), (wc, wcs)) = ax.split(w);
        let ((of, ofs), (oc, ocs)) = ax.split(o);
        let mid = oc + ocs / 2.0;
        if mid < wc + 10.0 || mid > wc + wcs - 10.0 {
            continue;
        }
        // the facing sides, along the flow
        let (a, b) = if of + ofs <= wf {
            (of + ofs, wf)
        } else if wf + wfs <= of {
            (of, wf + wfs)
        } else {
            continue;
        };
        let pt = |f: f64| if ax.across { (f, mid) } else { (mid, f) };
        let (p, q) = (pt(a), pt(b));
        let (lo, hi) = (a.min(b), a.max(b));
        let blocked = model.nodes.iter().any(|n| {
            n.kind == Kind::Card
                && n.id != *wide
                && n.id != *other
                && boxes.get(&n.id).is_some_and(|r| {
                    let ((rf, rfs), (rc, rcs)) = ax.split(r);
                    rc < mid && rc + rcs > mid && rf < hi && rf + rfs > lo
                })
        });
        if !blocked {
            let line = if from_wide { vec![q, p] } else { vec![p, q] };
            routes.insert(e.id.clone(), line);
        }
    }
}

/// Gap kept between two column blocks of one row (px).
const COL_GAP: f64 = 48.0;

/// The column of a node: its first `.col-<name>` class (D-050).
fn column_of(n: &skemo_lang::Node) -> Option<&str> {
    n.classes.iter().find_map(|c| c.strip_prefix("col-"))
}

/// A column's members on one row: their flow and across extents (start, end).
struct Block<'a> {
    col: &'a str,
    ids: Vec<&'a str>,
    flow: (f64, f64),
    span: (f64, f64),
}

/// Columns (D-050): nodes with one `.col-<name>` class share an axis across `dir`, whatever
/// zone or stack they sit in (a service and its pods over the machine that hosts them). In each
/// row (members overlapping along the flow), a column's members form a block; every block is
/// centred on the axis of its column's widest block. Blocks of one row keep `COL_GAP` between
/// them, in the order of their axes; a block that would run into a card outside the columns
/// stays where it is.
pub fn columns(model: &Model, placement: &mut Placement) {
    let ax = Axes::of(model);
    let mut cols: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for n in model.nodes.iter().filter(|n| n.at.is_none()) {
        if let Some(c) = column_of(n)
            && placement.boxes.contains_key(&n.id)
        {
            cols.entry(c).or_default().push(n.id.as_str());
        }
    }
    if cols.is_empty() {
        return;
    }
    let members: Vec<&str> = cols.values().flatten().copied().collect();
    let first = blocks(ax, &cols, &placement.boxes);
    let mut axis = column_axes(model, &cols, &first);
    // no wide zone carries the columns: they keep an even rhythm (D-059)
    if !first.iter().any(|b| anchored(model, &b.ids)) {
        axis = even_axes(&first, &axis);
    }
    place_rows(model, ax, placement, (&first, &axis), &members);
    // a block its row could not set on its column's axis (pushed by a neighbour) gives the column
    // its place, and a second pass brings the column's other rows over it: pods follow their
    // service (D-058). Not for a column held by a block in a wide zone.
    let placed = blocks(ax, &cols, &placement.boxes);
    let mut moved = false;
    for b in &placed {
        let mid = f64::midpoint(b.span.0, b.span.1);
        let held = placed
            .iter()
            .any(|o| o.col == b.col && anchored(model, &o.ids));
        if !held && (mid - axis[b.col]).abs() > 0.5 {
            axis.insert(b.col, mid);
            moved = true;
        }
    }
    if moved {
        place_rows(model, ax, placement, (&placed, &axis), &members);
    }
}

/// Columns side by side at an even rhythm (D-059), as tight as their rows allow: in the order of
/// their axes, two columns are as close as the blocks they have side by side on any row permit
/// (`COL_GAP` apart); when blocks of one row skip a column, the room they lack is shared evenly
/// by the gaps between them. The whole stays centred where the columns stood. Columns of one width
/// then stand in mirror round the middle (two sites on either side of their ingress).
fn even_axes<'a>(blocks: &[Block<'a>], axis: &BTreeMap<&'a str, f64>) -> BTreeMap<&'a str, f64> {
    let mut order: Vec<&str> = axis.keys().copied().collect();
    order.sort_by(|p, q| axis[p].total_cmp(&axis[q]));
    let n = order.len();
    if n < 2 {
        return axis.clone();
    }
    let rank = |c: &str| order.iter().position(|o| *o == c).unwrap_or(0);
    // needed distance between the axes of columns i < j that have blocks side by side on a row
    let mut need: BTreeMap<(usize, usize), f64> = BTreeMap::new();
    for b in blocks {
        let mut row: Vec<&Block> = blocks
            .iter()
            .filter(|o| o.flow.0 < b.flow.1 && o.flow.1 > b.flow.0)
            .collect();
        row.sort_by_key(|o| rank(o.col));
        for w in row.windows(2) {
            let (i, j) = (rank(w[0].col), rank(w[1].col));
            if i < j {
                let d = (w[0].span.1 - w[0].span.0 + w[1].span.1 - w[1].span.0) / 2.0 + COL_GAP;
                let e = need.entry((i, j)).or_insert(0.0);
                *e = e.max(d);
            }
        }
    }
    let mut gap = vec![0.0_f64; n - 1];
    for ((i, j), d) in &need {
        if j - i == 1 {
            gap[*i] = gap[*i].max(*d);
        }
    }
    for _ in 0..n {
        for ((i, j), d) in &need {
            let have: f64 = gap[*i..*j].iter().sum();
            if have < *d - 1e-9 {
                let add = (d - have) / (j - i) as f64;
                for g in &mut gap[*i..*j] {
                    *g += add;
                }
            }
        }
    }
    let span: f64 = gap.iter().sum();
    let centre = f64::midpoint(axis[order[0]], axis[order[n - 1]]);
    let mut x = centre - span / 2.0;
    let mut out = BTreeMap::new();
    for (k, c) in order.iter().enumerate() {
        out.insert(*c, x);
        if k < n - 1 {
            x += gap[k];
        }
    }
    out
}

/// Places the blocks row by row, each row in the order of the columns' axes.
fn place_rows(
    model: &Model,
    ax: Axes,
    placement: &mut Placement,
    (blocks, axis): (&[Block], &BTreeMap<&str, f64>),
    members: &[&str],
) {
    let mut done = vec![false; blocks.len()];
    for i in 0..blocks.len() {
        if done[i] {
            continue;
        }
        let mut row: Vec<usize> = (0..blocks.len())
            .filter(|j| {
                !done[*j]
                    && blocks[*j].flow.0 < blocks[i].flow.1
                    && blocks[*j].flow.1 > blocks[i].flow.0
            })
            .collect();
        row.sort_by(|p, q| axis[blocks[*p].col].total_cmp(&axis[blocks[*q].col]));
        let mut floor = f64::MIN;
        for j in row {
            done[j] = true;
            floor = place_block(
                model,
                ax,
                placement,
                (&blocks[j], axis[blocks[j].col]),
                (members, floor),
            );
        }
    }
}

/// The blocks of every column: members grouped while they overlap along the flow.
fn blocks<'a>(
    ax: Axes,
    cols: &BTreeMap<&'a str, Vec<&'a str>>,
    boxes: &BTreeMap<String, Box2>,
) -> Vec<Block<'a>> {
    let mut blocks: Vec<Block> = Vec::new();
    for (c, ids) in cols {
        for id in ids {
            let ((f, fs), (a, as_)) = ax.split(&boxes[*id]);
            match blocks
                .iter_mut()
                .find(|b| b.col == *c && f < b.flow.1 && f + fs > b.flow.0)
            {
                Some(b) => {
                    b.ids.push(id);
                    b.flow = (b.flow.0.min(f), b.flow.1.max(f + fs));
                    b.span = (b.span.0.min(a), b.span.1.max(a + as_));
                }
                None => blocks.push(Block {
                    col: c,
                    ids: vec![id],
                    flow: (f, f + fs),
                    span: (a, a + as_),
                }),
            }
        }
    }
    blocks
}

/// A block in a wide zone is the reference: it gives its column's axis and stays put.
fn anchored(model: &Model, ids: &[&str]) -> bool {
    ids.iter().any(|id| {
        model
            .ancestors(id)
            .iter()
            .any(|z| z.classes.iter().any(|c| c == "wide"))
    })
}

/// Each column's axis: the middle of its anchored block, otherwise of its widest.
fn column_axes<'a>(
    model: &Model,
    cols: &BTreeMap<&'a str, Vec<&'a str>>,
    blocks: &[Block],
) -> BTreeMap<&'a str, f64> {
    cols.keys()
        .map(|c| {
            let mine = || blocks.iter().filter(|b| b.col == *c);
            let width = |b: &&Block| b.span.1 - b.span.0;
            let pick = mine()
                .find(|b| anchored(model, &b.ids))
                .or_else(|| mine().max_by(|p, q| width(p).total_cmp(&width(q))));
            (*c, pick.map_or(0.0, |b| f64::midpoint(b.span.0, b.span.1)))
        })
        .collect()
}

/// Centres a block on its column's `axis`, at `floor` or beyond (the end of the previous block of
/// its row, plus `COL_GAP`), unless it is anchored or would run into a card outside the columns
/// (`members`). Returns the floor for the next block of the row.
fn place_block(
    model: &Model,
    ax: Axes,
    placement: &mut Placement,
    (block, axis): (&Block, f64),
    (members, floor): (&[&str], f64),
) -> f64 {
    let (lo, hi) = block.span;
    if anchored(model, &block.ids) {
        return floor.max(hi + COL_GAP);
    }
    let w = hi - lo;
    let to = (axis - w / 2.0).max(floor);
    let d = to - lo;
    let moved: Vec<(String, Box2)> = block
        .ids
        .iter()
        .map(|id| ((*id).to_string(), ax.moved(&placement.boxes[*id], d)))
        .collect();
    let overlaps = |a: &Box2, o: &Box2| {
        a.x < o.right() && o.x < a.right() && a.y < o.bottom() && o.y < a.bottom()
    };
    let hits = moved.iter().any(|(id, nb)| {
        model.nodes.iter().any(|n| {
            n.kind == Kind::Card
                && !members.contains(&n.id.as_str())
                && n.id != *id
                && placement.boxes.get(&n.id).is_some_and(|o| overlaps(nb, o))
        })
    });
    if hits {
        return floor.max(hi + COL_GAP);
    }
    for (id, nb) in moved {
        if model.node(&id).is_some_and(|n| n.kind == Kind::Zone) {
            let b = placement.boxes[&id];
            shift(model, placement, &id, (nb.x - b.x, nb.y - b.y));
        } else {
            placement.boxes.insert(id.clone(), nb);
            placement.routes.retain(|k, _| {
                model
                    .edges
                    .iter()
                    .all(|e| e.id != *k || (e.from != id && e.to != id))
            });
        }
    }
    to + w + COL_GAP
}
