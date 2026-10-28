//! Docked elements (`at="s"`, `at="ne"`…, FORMAT §3.2.9): a top-level card or zone with a compass
//! point leaves the drawing. The drawing is laid out and routed as if it were absent; each docked
//! element is laid out on its own, as a small diagram; then they are set around the drawing, on a
//! grid of three columns and three rows (`nw n ne / w · e / sw s se`) whose empty rows and
//! columns take no room. The drawing never moves.

use crate::diagram::Arranged;
use crate::layout::Box2;
use crate::measure::Sizes;
use skemo_lang::Model;
use std::collections::BTreeMap;

/// A docked element: its compass point, whether it takes a row of its own (`.wide`), and the
/// model of its subtree.
#[derive(Debug, Clone)]
pub struct Part {
    pub point: String,
    pub wide: bool,
    pub model: Model,
}

/// The drawing's model without the docked elements, and one model per docked element.
#[derive(Debug, Clone)]
pub struct Split {
    pub main: Model,
    pub parts: Vec<Part>,
}

/// The top-level node holding `id` (itself at the top level).
fn top<'a>(model: &'a Model, id: &str) -> Option<&'a skemo_lang::Node> {
    let mut n = model.node(id)?;
    for _ in 0..=model.nodes.len() {
        match n.parent.as_deref().and_then(|p| model.node(p)) {
            Some(p) => n = p,
            None => break,
        }
    }
    Some(n)
}

/// The docked element holding `id`, if any.
fn dock_of(model: &Model, id: &str) -> Option<String> {
    top(model, id)
        .filter(|t| t.dock.is_some())
        .map(|t| t.id.clone())
}

/// The model without the arrows between a docked element and anything outside it (the parser's
/// warning `docked-arrow`: they are not drawn).
pub fn without_crossing(mut model: Model) -> Model {
    if !model.nodes.iter().any(|n| n.dock.is_some()) {
        return model;
    }
    let keep: Vec<bool> = model
        .edges
        .iter()
        .map(|e| dock_of(&model, &e.from) == dock_of(&model, &e.to))
        .collect();
    let mut k = keep.into_iter();
    model.edges.retain(|_| k.next().unwrap_or(true));
    model
}

/// Splits a model with docked elements; `None` when there is none.
pub fn split(model: &Model) -> Option<Split> {
    let docked: Vec<&skemo_lang::Node> = model
        .nodes
        .iter()
        .filter(|n| n.parent.is_none() && n.dock.is_some())
        .collect();
    if docked.is_empty() {
        return None;
    }
    let restrict = |keep: &dyn Fn(&str) -> bool| {
        let mut m = model.clone();
        m.nodes.retain(|n| keep(&n.id));
        m.edges.retain(|e| keep(&e.from) && keep(&e.to));
        m.title = None;
        m
    };
    let main = restrict(&|id| dock_of(model, id).is_none());
    let parts = docked
        .iter()
        .map(|d| {
            let mut m = restrict(&|id| dock_of(model, id).as_deref() == Some(&d.id));
            // laid out as a plain graph, whatever the drawing's mode
            for k in [
                "layout", "fit", "start", "end", "from", "to", "scale", "width", "today",
            ] {
                m.options.remove(k);
            }
            for n in m.nodes.iter_mut().filter(|n| n.id == d.id) {
                n.dock = None;
            }
            Part {
                point: d.dock.clone().unwrap_or_default(),
                wide: d.classes.iter().any(|c| c == "wide"),
                model: m,
            }
        })
        .collect();
    Some(Split { main, parts })
}

/// What an arrangement covers: boxes, routes and labels (with their measured sizes).
fn extent(a: &Arranged, sizes: &Sizes) -> Box2 {
    let mut framed = a.boxes.clone();
    framed.extend(crate::diagram::label_boxes(&a.labels, sizes));
    let f = crate::diagram::frame(&framed, &a.routes);
    let pad = crate::diagram::FRAME_PAD;
    Box2 {
        x: pad - f.ox,
        y: pad - f.oy,
        w: f.w - 2.0 * pad,
        h: f.h - 2.0 * pad,
    }
}

/// Moves an arrangement by `(dx, dy)`.
fn shift(a: &mut Arranged, (dx, dy): (f64, f64)) {
    for b in a.boxes.values_mut() {
        b.x += dx;
        b.y += dy;
    }
    for r in a.routes.values_mut() {
        for p in r.iter_mut() {
            p.0 += dx;
            p.1 += dy;
        }
    }
    for l in a.labels.values_mut() {
        l.0 += dx;
        l.1 += dy;
    }
}

/// One element to set: its size and where it goes once placed.
struct Item {
    k: usize,
    w: f64,
    h: f64,
    wide: bool,
}

/// Rows of a band (`n` or `s`): side by side in writing order, a new row when the next one would
/// pass `cap`; a `.wide` element on a row of its own. Each row: its items, width and height.
fn rows(items: &[Item], cap: f64, gap: f64) -> Vec<(Vec<usize>, f64, f64)> {
    let mut out: Vec<(Vec<usize>, f64, f64)> = Vec::new();
    for (i, it) in items.iter().enumerate() {
        let fits = out.last().is_some_and(|(r, w, _)| {
            !r.is_empty() && !it.wide && !r.iter().any(|&j| items[j].wide) && w + gap + it.w <= cap
        });
        if fits {
            if let Some((r, w, h)) = out.last_mut() {
                r.push(i);
                *w += gap + it.w;
                *h = h.max(it.h);
            }
        } else {
            out.push((vec![i], it.w, it.h));
        }
    }
    out
}

/// Width and height of a column of items one under the other.
fn column(items: &[Item], gap: f64) -> (f64, f64) {
    let w = items.iter().map(|i| i.w).fold(0.0, f64::max);
    let h = items.iter().map(|i| i.h).sum::<f64>() + gap * items.len().saturating_sub(1) as f64;
    (w, h)
}

/// Sets the docked elements around the drawing and merges everything into one arrangement.
/// Like Java's `BorderLayout`: the `n` and `s` bands span the whole width; the side columns hold
/// `w` and `e`, with the corners at their top (`nw`, `ne`) or bottom (`sw`, `se`), beside the
/// drawing.
pub(crate) fn compose(
    mut main: Arranged,
    parts: Vec<(Part, Arranged)>,
    sizes: &Sizes,
    gap: f64,
) -> Arranged {
    let m = extent(&main, sizes);
    let mut by: BTreeMap<&str, Vec<Item>> = BTreeMap::new();
    let exts: Vec<Box2> = parts.iter().map(|(_, a)| extent(a, sizes)).collect();
    for (k, (p, _)) in parts.iter().enumerate() {
        by.entry(p.point.as_str()).or_default().push(Item {
            k,
            w: exts[k].w,
            h: exts[k].h,
            wide: p.wide,
        });
    }
    let empty: Vec<Item> = Vec::new();
    let get = |p: &str| by.get(p).unwrap_or(&empty);
    let side = |ps: [&str; 3]| -> (f64, f64) {
        let w = ps
            .iter()
            .flat_map(|p| get(p).iter().map(|i| i.w))
            .fold(0.0, f64::max);
        let h = ps.iter().map(|p| column(get(p), gap).1).sum::<f64>()
            + gap
                * ps.iter()
                    .filter(|p| !get(p).is_empty())
                    .count()
                    .saturating_sub(1) as f64;
        (w, h)
    };
    let (west_w, west_h) = side(["nw", "w", "sw"]);
    let (east_w, east_h) = side(["ne", "e", "se"]);
    let mid_h = m.h.max(west_h).max(east_h);
    let x_left = if west_w > 0.0 {
        m.x - gap - west_w
    } else {
        m.x
    };
    let x_right = if east_w > 0.0 {
        m.x + m.w + gap + east_w
    } else {
        m.x + m.w
    };
    let full = x_right - x_left;
    let widest = ["n", "s"]
        .iter()
        .flat_map(|p| get(p).iter().map(|i| i.w))
        .fold(0.0, f64::max);
    // side by side up to half again the drawing's width, then a new row
    let cap = (full * 1.5).max(widest);
    let (n_rows, s_rows) = (rows(get("n"), cap, gap), rows(get("s"), cap, gap));
    let band_h = |r: &[(Vec<usize>, f64, f64)]| {
        r.iter().map(|x| x.2).sum::<f64>() + gap * r.len().saturating_sub(1) as f64
    };
    let mut at: Vec<(f64, f64)> = vec![(0.0, 0.0); parts.len()];
    let mut place_rows = |r: &[(Vec<usize>, f64, f64)], items: &[Item], y0: f64| {
        let mut y = y0;
        for (row, _, h) in r {
            let mut x = x_left;
            for &i in row {
                at[items[i].k] = (x, y);
                x += items[i].w + gap;
            }
            y += h + gap;
        }
    };
    place_rows(&n_rows, get("n"), m.y - gap - band_h(&n_rows));
    place_rows(&s_rows, get("s"), m.y + mid_h + gap);
    // a side column: its top corner's elements from the top, then its side's, then its bottom
    // corner's ending at the bottom of the middle row
    let mut column_at = |ps: [&str; 3], x: f64| {
        let mut y = m.y;
        for p in &ps[..2] {
            for i in get(p) {
                at[i.k] = (x, y);
                y += i.h + gap;
            }
        }
        let mut y = m.y + mid_h;
        for i in get(ps[2]).iter().rev() {
            y -= i.h;
            at[i.k] = (x, y);
            y -= gap;
        }
    };
    column_at(["nw", "w", "sw"], m.x - gap - west_w);
    column_at(["ne", "e", "se"], m.x + m.w + gap);
    for (k, (_, mut a)) in parts.into_iter().enumerate() {
        shift(&mut a, (at[k].0 - exts[k].x, at[k].1 - exts[k].y));
        main.boxes.extend(a.boxes);
        main.routes.extend(a.routes);
        main.labels.extend(a.labels);
        main.edge_groups.extend(a.edge_groups);
    }
    main
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_and_drops_crossing_arrows() {
        let doc = skemo_lang::parse(
            "card#a A\n  to(b)\n  to(x)\ncard#b B\nzone#z(at=\"s\") Z\n  card#x X\n    to(y)\n  card#y Y\n",
        );
        let m = without_crossing(doc.model.clone());
        assert_eq!(m.edges.len(), 2);
        let s = split(&m).unwrap();
        assert_eq!(s.main.nodes.len(), 2);
        assert_eq!(s.parts.len(), 1);
        assert_eq!(s.parts[0].point, "s");
        assert_eq!(s.parts[0].model.nodes.len(), 3);
        assert_eq!(s.parts[0].model.edges.len(), 1);
    }
}
