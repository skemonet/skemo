//! Lines with emphasis and tables (FORMAT §3.2.4, §3.2.5): widths, line breaking and the cells'
//! boxes, shared by the measure and the geometry of a card.

use crate::measure::{border, line_height, tw};
use crate::scene::{CardEl, LiEl, Scene};

/// A piece of a line on one row of text: its text, the element of its style, its width (with the
/// horizontal padding of an inline element such as `code` or `mark`).
#[derive(Debug, Clone, PartialEq)]
pub struct Seg {
    pub text: String,
    pub el: usize,
    pub width: f64,
    /// Room before the text inside its own box (an inline element's left padding).
    pub inset: f64,
}

/// Width of a piece styled by `el`; an element other than the line itself counts its horizontal
/// padding (a `code` or a `mark` with a background).
pub fn seg(scene: &Scene, line_el: usize, el: usize, text: &str) -> Seg {
    let c = scene.style(el);
    let (w, inset) = if el == line_el {
        (tw(c, text), 0.0)
    } else {
        let p = c.padding();
        (tw(c, text) + p[1] + p[3], p[3])
    };
    Seg {
        text: text.to_string(),
        el,
        width: w,
        inset,
    }
}

/// Natural width of pieces on one row.
pub fn parts_width(scene: &Scene, line_el: usize, parts: &[(String, usize)]) -> f64 {
    parts
        .iter()
        .map(|(t, el)| seg(scene, line_el, *el, t).width)
        .sum()
}

/// Breaks pieces into rows of at most `avail` px, at spaces, like the browser; a piece keeps its
/// style across a break.
pub fn break_parts(
    scene: &Scene,
    line_el: usize,
    parts: &[(String, usize)],
    avail: f64,
) -> Vec<Vec<Seg>> {
    // words with the element of their style; `true` when a space comes before the word
    let mut words: Vec<(String, usize, bool)> = Vec::new();
    let mut space = false;
    for (text, el) in parts {
        for (k, w) in text.split(' ').enumerate() {
            if k > 0 {
                space = true;
            }
            if w.is_empty() {
                continue;
            }
            words.push((w.to_string(), *el, space && !words.is_empty()));
            space = false;
        }
        if text.ends_with(' ') {
            space = true;
        }
    }
    let mut rows: Vec<Vec<Seg>> = vec![Vec::new()];
    let mut used = 0.0;
    for (w, el, sp) in words {
        let prev = rows.last().and_then(|r| r.last()).map(|s| s.el);
        // a space between two styles is the line's own (as the natural width counts it)
        let (lead, word) = if !sp {
            (None, w.clone())
        } else if prev == Some(el) {
            (None, format!(" {w}"))
        } else {
            (Some(seg(scene, line_el, line_el, " ")), w.clone())
        };
        let wide = lead.as_ref().map_or(0.0, |l| l.width) + seg(scene, line_el, el, &word).width;
        let row_empty = rows.last().is_none_or(Vec::is_empty);
        if !row_empty && used + wide > avail + 1e-6 {
            rows.push(Vec::new());
            push_seg(scene, line_el, rows.last_mut(), el, &w);
            used = seg(scene, line_el, el, &w).width;
            continue;
        }
        if lead.is_some() {
            push_seg(scene, line_el, rows.last_mut(), line_el, " ");
        }
        push_seg(scene, line_el, rows.last_mut(), el, &word);
        used += wide;
    }
    rows
}

/// Appends text to the last segment of a row when it has the same style, else a new segment.
fn push_seg(scene: &Scene, line_el: usize, row: Option<&mut Vec<Seg>>, el: usize, text: &str) {
    let Some(row) = row else { return };
    match row.last_mut() {
        Some(s) if s.el == el => {
            let t = format!("{}{text}", s.text);
            *s = seg(scene, line_el, el, &t);
        }
        _ => row.push(seg(scene, line_el, el, text)),
    }
}

/// Width taken before a line's text by its state mark (`icon + gap`) and by a legend's sample,
/// and the mark's size.
pub fn icon_lead(scene: &Scene, li: &LiEl) -> (f64, f64) {
    let (w, size) = scene.icon_lead(&crate::scene::Decor {
        icon: li.icon,
        mark: None,
    });
    (
        w + sample_box(scene, li).map_or(0.0, |(sw, _, gap)| sw + gap),
        size,
    )
}

/// Width, height and gap after it of a legend's sample (`--sample-w`, `--sample-gap` on the
/// line; a chip's own `width` and `height`).
pub fn sample_box(scene: &Scene, li: &LiEl) -> Option<(f64, f64, f64)> {
    let c = scene.style(li.el);
    let gap = c.length("--sample-gap").unwrap_or(8.0);
    Some(match li.sample? {
        crate::scene::Sample::Arrow { .. } => (c.length("--sample-w").unwrap_or(34.0), 8.0, gap),
        // as wide as a chip, so that the lines' texts start together
        crate::scene::Sample::Icon(el, _) => {
            let s = scene.style(el).length("--sample-icon").unwrap_or(14.0);
            (s.max(22.0), s, gap)
        }
        crate::scene::Sample::Chip(el) => {
            let sc = scene.style(el);
            (
                sc.length("width").unwrap_or(22.0),
                sc.length("height").unwrap_or(12.0),
                gap,
            )
        }
    })
}

/// Natural width of a text line (without the list's padding): state mark, address, text.
pub fn line_width(scene: &Scene, li: &LiEl) -> f64 {
    let c = scene.style(li.el);
    let p = c.padding();
    let lead = icon_lead(scene, li).0;
    if !li.parts.is_empty() {
        return p[1] + p[3] + lead + parts_width(scene, li.el, &li.parts);
    }
    let mut w = p[1] + p[3] + lead;
    if let Some((m, ip)) = &li.mono {
        let mc = scene.style(*m);
        w += tw(mc, ip) + mc.margin()[1] + mc.margin()[3];
    }
    w + tw(c, &li.rest)
}

/// A cell placed in its table: box (in the table's frame) and the cell.
#[derive(Debug, Clone)]
pub struct CellBox {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    /// Index of the row's line in the card (`CardEl::lis`) and of the cell in the row.
    pub li: usize,
    pub cell: usize,
}

/// A table's layout: every cell's box, every row's band, the table's size.
#[derive(Debug, Clone, Default)]
pub struct TableGeom {
    pub w: f64,
    pub h: f64,
    pub cells: Vec<CellBox>,
    /// Each line of the table (rows and separator): index in `CardEl::lis`, top, height.
    pub rows: Vec<(usize, f64, f64)>,
}

/// Lays out table `ti` of a card: columns as wide as their widest cell, rows as tall as their
/// tallest; a cell over several columns or rows widens or heightens the last of them if needed.
/// Text never wraps in a table.
pub fn table(scene: &Scene, card: &CardEl, ti: usize) -> TableGeom {
    let Some(t) = card.tables.get(ti) else {
        return TableGeom::default();
    };
    let lines: Vec<usize> = (0..card.lis.len())
        .filter(|&k| card.lis[k].table == Some(ti))
        .collect();
    let rows: Vec<usize> = lines
        .iter()
        .copied()
        .filter(|&k| !card.lis[k].rule)
        .collect();
    let mut cols = vec![0.0f64; t.cols.max(1)];
    let mut heights = vec![0.0f64; rows.len()];
    let size = |el: usize, parts: &[(String, usize)]| {
        let c = scene.style(el);
        let (p, b) = (c.padding(), border(c));
        (
            parts_width(scene, el, parts) + p[1] + p[3] + b[1] + b[3],
            line_height(c) + p[0] + p[2] + b[0] + b[2],
        )
    };
    let mut spanned = Vec::new();
    for (r, &k) in rows.iter().enumerate() {
        for c in &card.lis[k].cells {
            let (w, h) = size(c.el, &c.parts);
            if c.colspan == 1 && c.col < cols.len() {
                cols[c.col] = cols[c.col].max(w);
            }
            if c.rowspan == 1 {
                heights[r] = heights[r].max(h);
            }
            if c.colspan > 1 || c.rowspan > 1 {
                spanned.push((r, c.col, c.colspan, c.rowspan, w, h));
            }
        }
    }
    for &(r, col, cs, rs, w, h) in &spanned {
        let last_col = (col + cs).min(cols.len()) - 1;
        let have: f64 = cols[col.min(last_col)..=last_col].iter().sum();
        if w > have {
            cols[last_col] += w - have;
        }
        let last_row = (r + rs).min(heights.len()) - 1;
        let have: f64 = heights[r..=last_row].iter().sum();
        if h > have {
            heights[last_row] += h - have;
        }
    }
    let col_x: Vec<f64> = cols
        .iter()
        .scan(0.0, |x, w| {
            let at = *x;
            *x += w;
            Some(at)
        })
        .collect();
    let row_y: Vec<f64> = heights
        .iter()
        .scan(0.0, |y, h| {
            let at = *y;
            *y += h;
            Some(at)
        })
        .collect();
    let mut out = TableGeom {
        w: cols.iter().sum(),
        h: heights.iter().sum(),
        ..TableGeom::default()
    };
    for &k in &lines {
        let r = rows.iter().position(|&x| x == k);
        let (y, h) = r.map_or((row_y.first().copied().unwrap_or(0.0), 0.0), |r| {
            (row_y[r], heights[r])
        });
        out.rows.push((k, y, h));
        let Some(r) = r else { continue };
        for (ci, c) in card.lis[k].cells.iter().enumerate() {
            let col = c.col.min(cols.len() - 1);
            let last_col = (c.col + c.colspan).min(cols.len()) - 1;
            let last_row = (r + c.rowspan).min(heights.len()) - 1;
            out.cells.push(CellBox {
                x: col_x[col],
                y: row_y[r],
                w: col_x[last_col] + cols[last_col] - col_x[col],
                h: row_y[last_row] + heights[last_row] - row_y[r],
                li: k,
                cell: ci,
            });
        }
    }
    out
}

/// The first line of table `ti` in the card: the table takes its room there.
pub fn first_of_table(card: &CardEl, k: usize) -> bool {
    let t = card.lis[k].table;
    t.is_some() && (k == 0 || card.lis[k - 1].table != t)
}
