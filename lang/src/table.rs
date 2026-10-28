//! Tables (FORMAT §3.2.5): rows of a card are read here into cells, and consecutive rows are
//! resolved into tables (header, alignment, column and row spans) for the engine.
//!
//! ```
//! let doc = skemo_lang::parse("card#sg Ingress\n  | Group | Inbound |\n  |---|---|\n  | alb | 443 |\n  | ^^ | 80 |\n");
//! let blocks = skemo_lang::table::blocks(&doc.model.node("sg").unwrap().li);
//! let skemo_lang::table::Block::Table(t) = &blocks[0] else { panic!() };
//! assert!(t.header && t.cols == 2);
//! assert_eq!(t.rows[1].cells[0].rowspan, 2);
//! ```

use crate::inline::{ends_with_bar, split_cells};
use crate::model::{Align, Cell, Join, Li};

/// A row read from the text after the opening bar: its cells, or the alignments of a separator
/// row. `None` when the line is not a row.
pub(crate) enum RowKind {
    Cells(Vec<Cell>),
    Rule(Vec<Align>),
}

/// Reads a line's text (after the bar, markers removed) as a row, if it is one: it ends with an
/// unescaped `|`, so the line holds at least two bars.
pub(crate) fn read_row(text: &str) -> Option<RowKind> {
    if !ends_with_bar(text) {
        return None;
    }
    let t = text.trim_end();
    let parts = split_cells(&t[..t.len() - 1]);
    if let Some(al) = rule(&parts) {
        return Some(RowKind::Rule(al));
    }
    Some(RowKind::Cells(parts.iter().map(|p| cell(p)).collect()))
}

/// A separator row: every part is `---`, `:---`, `---:` or `:---:`.
fn rule(parts: &[String]) -> Option<Vec<Align>> {
    parts
        .iter()
        .map(|p| {
            let p = p.trim();
            let core = p.trim_start_matches(':').trim_end_matches(':');
            if core.is_empty() || !core.chars().all(|c| c == '-') {
                return None;
            }
            Some(match (p.starts_with(':'), p.ends_with(':')) {
                (true, true) => Align::Center,
                (false, true) => Align::Right,
                _ => Align::Left,
            })
        })
        .collect()
}

/// One cell: `^^` joins the cell above, nothing at all the cell on the left; `.class` glued to
/// the bar gives the cell classes.
fn cell(raw: &str) -> Cell {
    if raw.is_empty() {
        return Cell {
            join: Some(Join::Left),
            ..Cell::default()
        };
    }
    if raw.trim() == "^^" {
        return Cell {
            join: Some(Join::Up),
            ..Cell::default()
        };
    }
    let (classes, rest) = glued_classes(raw);
    Cell {
        text: rest.trim().to_string(),
        classes,
        join: None,
    }
}

/// Classes glued to the start of `raw` (`.error lambda` → `["error"]`, `"lambda"`); a space or
/// anything else first gives none.
pub(crate) fn glued_classes(raw: &str) -> (Vec<String>, &str) {
    let mut out = Vec::new();
    let mut rest = raw;
    while let Some(r) = rest.strip_prefix('.') {
        let first = r.chars().next();
        if !first.is_some_and(|c| c.is_alphabetic() || c == '_') {
            break;
        }
        let end = r
            .find(|c: char| !(c.is_alphanumeric() || c == '_' || c == '-'))
            .unwrap_or(r.len());
        out.push(r[..end].to_string());
        rest = &r[end..];
    }
    if out.is_empty() {
        return (out, raw);
    }
    match rest.chars().next() {
        None => (out, ""),
        Some(' ') => (out, &rest[1..]),
        // `.5 GB`, `.env-file:` … not classes after all
        Some(_) => (Vec::new(), raw),
    }
}

/// A card's lines, in order: plain lines and tables.
#[derive(Debug, Clone, PartialEq)]
pub enum Block {
    /// A text line: its index in `li`.
    Line(usize),
    Table(Table),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Table {
    /// Columns, spans included.
    pub cols: usize,
    /// The first row is a header (a separator row follows it).
    pub header: bool,
    /// Alignment per column (left by default).
    pub align: Vec<Align>,
    pub rows: Vec<Row>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    /// Index of the row's line in `li` (its animation markers are there).
    pub li: usize,
    /// The cells that start in this row, left to right.
    pub cells: Vec<TCell>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TCell {
    pub col: usize,
    pub colspan: usize,
    pub rowspan: usize,
    /// Text with its emphasis (`inline::runs`).
    pub text: String,
    pub classes: Vec<String>,
}

/// Groups a card's lines into plain lines and tables (consecutive rows; a separator row only
/// counts right after the first row of a table).
pub fn blocks(li: &[Li]) -> Vec<Block> {
    let mut out = Vec::new();
    let mut k = 0;
    while k < li.len() {
        if !is_row(&li[k]) {
            out.push(Block::Line(k));
            k += 1;
            continue;
        }
        let n = li[k..].iter().take_while(|l| is_row(l)).count();
        out.push(Block::Table(resolve(li, k, n)));
        k += n;
    }
    out
}

fn is_row(l: &Li) -> bool {
    !l.cells.is_empty() || !l.rule.is_empty()
}

fn resolve(li: &[Li], first: usize, n: usize) -> Table {
    let header = n > 1 && li[first].rule.is_empty() && !li[first + 1].rule.is_empty();
    let align = if header {
        li[first + 1].rule.clone()
    } else {
        Vec::new()
    };
    let mut rows: Vec<Row> = Vec::new();
    for (k, l) in li.iter().enumerate().skip(first).take(n) {
        if !l.rule.is_empty() {
            continue; // the header's separator, or one out of place (warned by the parser)
        }
        let mut row = Row {
            li: k,
            cells: Vec::new(),
        };
        for (col, c) in l.cells.iter().enumerate() {
            match c.join {
                Some(Join::Left) if !row.cells.is_empty() => {
                    if let Some(last) = row.cells.last_mut() {
                        last.colspan += 1;
                    }
                }
                Some(Join::Up) if !rows.is_empty() => {
                    if let Some(above) = covering(&mut rows, col) {
                        above.rowspan += 1;
                    }
                }
                _ => row.cells.push(TCell {
                    col,
                    colspan: 1,
                    rowspan: 1,
                    text: c.text.clone(),
                    classes: c.classes.clone(),
                }),
            }
        }
        rows.push(row);
    }
    let cols = rows
        .iter()
        .flat_map(|r| r.cells.iter().map(|c| c.col + c.colspan))
        .chain(std::iter::once(align.len()))
        .max()
        .unwrap_or(0);
    Table {
        cols,
        header,
        align,
        rows,
    }
}

/// The cell of an earlier row that covers column `col` down to the row before the current one.
fn covering(rows: &mut [Row], col: usize) -> Option<&mut TCell> {
    let depth = rows.len();
    for (r, row) in rows.iter_mut().enumerate().rev() {
        for c in &mut row.cells {
            if (c.col..c.col + c.colspan).contains(&col) && r + c.rowspan == depth {
                return Some(c);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table(src: &str) -> Table {
        let doc = crate::parse(src);
        let n = doc.model.nodes.iter().find(|n| !n.li.is_empty()).unwrap();
        match blocks(&n.li).into_iter().find_map(|b| match b {
            Block::Table(t) => Some(t),
            Block::Line(_) => None,
        }) {
            Some(t) => t,
            None => panic!("no table"),
        }
    }

    #[test]
    fn spans_and_header() {
        let t = table(
            "card#c C\n  | G | In | At |\n  |:--|:-:|--:|\n  | alb | 443 | lb |\n  | api | 8080 | ecs |\n  | ^^ | 9090 | ecs |\n  | worker | none ||\n  |.error lambda | none | fn |\n",
        );
        assert!(t.header);
        assert_eq!(t.cols, 3);
        assert_eq!(t.align, vec![Align::Left, Align::Center, Align::Right]);
        assert_eq!(t.rows.len(), 6);
        assert_eq!(t.rows[2].cells[0].rowspan, 2);
        assert_eq!(t.rows[3].cells.len(), 2);
        assert_eq!(t.rows[4].cells[1].colspan, 2);
        assert_eq!(t.rows[5].cells[0].classes, vec!["error"]);
        assert_eq!(t.rows[5].cells[0].text, "lambda");
    }

    #[test]
    fn text_then_table_then_text() {
        let doc = crate::parse(
            "card#c C\n  | before\n  | a | b |\n  | c | d |\n  |\n  | e |\n  | after\n",
        );
        let b = blocks(&doc.model.node("c").unwrap().li);
        assert!(matches!(b[0], Block::Line(0)));
        assert!(matches!(&b[1], Block::Table(t) if t.rows.len() == 2 && !t.header));
        assert!(matches!(b[2], Block::Line(3)));
        assert!(matches!(&b[3], Block::Table(t) if t.cols == 1));
        assert!(matches!(b[4], Block::Line(5)));
    }

    #[test]
    fn classes_glued() {
        assert_eq!(glued_classes(".warn x"), (vec!["warn".to_string()], "x"));
        assert_eq!(glued_classes(" .env"), (Vec::new(), " .env"));
        assert_eq!(glued_classes(".5 GB"), (Vec::new(), ".5 GB"));
        assert_eq!(glued_classes(".env:"), (Vec::new(), ".env:"));
        assert_eq!(glued_classes(".a.b"), (vec!["a".into(), "b".into()], ""));
    }
}
