//! Structural edits for an editor (add or remove cards, zones and arrows, rename, move a card
//! into a zone): like `edit`, each one touches only the lines concerned; comments, order and
//! indentation of everything else stay as written.
//!
//! ```
//! use skemo_lang::structure::{add_edge, add_node, NewNode};
//! let src = "diagram\n  card#a A\n";
//! let src = add_node(src, None, &NewNode::card("b", "B")).unwrap();
//! let src = add_edge(&src, "a", "b").unwrap();
//! assert_eq!(src, "diagram\n  card#a A\n    to(b)\n  card#b B\n");
//! ```

use crate::edit::{EditError, Target, locate, quote, splice};
use crate::parse::{Located, parse};
use crate::tr;

/// A card or a zone to add.
#[derive(Debug, Clone, PartialEq)]
pub struct NewNode {
    /// `card`, `cards`, `zone` or `stack`.
    pub tag: String,
    pub id: String,
    pub classes: Vec<String>,
    /// Attributes in writing order (`at`, `size`…).
    pub attrs: Vec<(String, String)>,
    pub title: String,
}

impl NewNode {
    pub fn card(id: &str, title: &str) -> NewNode {
        NewNode {
            tag: "card".into(),
            id: id.into(),
            classes: Vec::new(),
            attrs: Vec::new(),
            title: title.into(),
        }
    }
    pub fn zone(id: &str, title: &str) -> NewNode {
        NewNode {
            tag: "zone".into(),
            ..NewNode::card(id, title)
        }
    }
    pub fn with_class(mut self, c: &str) -> NewNode {
        self.classes.push(c.into());
        self
    }
    pub fn with_attr(mut self, name: &str, value: &str) -> NewNode {
        self.attrs.push((name.into(), value.into()));
        self
    }
    /// The tag as written on its line.
    fn line(&self) -> String {
        let mut s = format!("{}#{}", self.tag, self.id);
        for c in &self.classes {
            s.push('.');
            s.push_str(c);
        }
        if !self.attrs.is_empty() {
            let a: Vec<String> = self
                .attrs
                .iter()
                .map(|(k, v)| format!("{k}={}", quote(v)))
                .collect();
            s.push_str(&format!("({})", a.join(" ")));
        }
        let title = one_line(&self.title);
        if !title.is_empty() {
            s.push(' ');
            s.push_str(&title);
        }
        s
    }
}

fn one_line(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// The first id `prefix1`, `prefix2`… that no card, zone or arrow of `src` uses.
pub fn fresh_id(src: &str, prefix: &str) -> String {
    let doc = parse(src);
    let used =
        |id: &str| doc.model.node(id).is_some() || doc.model.edges.iter().any(|e| e.id == id);
    (1..)
        .map(|k| format!("{prefix}{k}"))
        .find(|id| !used(id))
        .unwrap_or_else(|| prefix.to_string())
}

/// Start of the line of a located tag (its indentation included).
fn line_start(l: &Located) -> usize {
    l.line.start - l.indent
}

/// End of the tag's block: the end of its last line or of the last deeper line that follows
/// (blank lines inside the block are kept with it, trailing ones are not).
fn block_end(src: &str, l: &Located) -> usize {
    let mut end = src[l.line.end..]
        .find('\n')
        .map_or(src.len(), |i| l.line.end + i);
    let mut pos = end;
    while pos < src.len() {
        let next = pos + 1;
        let stop = src[next..].find('\n').map_or(src.len(), |i| next + i);
        let line = &src[next..stop];
        let indent = line.len() - line.trim_start().len();
        if !line.trim().is_empty() {
            if indent <= l.indent {
                break;
            }
            end = stop;
        }
        pos = stop;
    }
    end
}

/// The indentation of the children of a tag: that of its first child line, else two more.
fn child_indent(src: &str, l: &Located) -> usize {
    let after = src[l.line.end..].find('\n').map(|i| l.line.end + i + 1);
    if let Some(start) = after {
        let line = src[start..].lines().next().unwrap_or("");
        let indent = line.len() - line.trim_start().len();
        if !line.trim().is_empty() && indent > l.indent {
            return indent;
        }
    }
    l.indent + 2
}

/// Inserts `text` (one or more lines, without indentation) as the last child of `parent` (a node
/// id), or of the diagram (`None`: under `diagram(...)` if written, else at the end of the file).
fn append_child(src: &str, parent: Option<&str>, text: &str) -> Result<String, EditError> {
    if parse(src).has_errors() {
        return Err(EditError::Invalid);
    }
    append_unchecked(src, parent, text)
}

/// `append_child` on a source that may not parse yet (a block cut out to be moved).
fn append_unchecked(src: &str, parent: Option<&str>, text: &str) -> Result<String, EditError> {
    let doc = parse(src);
    let host = match parent {
        Some(p) => Some(
            doc.map
                .nodes
                .get(p)
                .cloned()
                .ok_or_else(|| EditError::NotFound(p.to_string()))?,
        ),
        None => doc.map.diagram.clone(),
    };
    let (at, indent) = match &host {
        Some(l) => (block_end(src, l), child_indent(src, l)),
        None => (src.trim_end_matches('\n').len(), 0),
    };
    let pad = " ".repeat(indent);
    let lines: Vec<String> = text.lines().map(|t| format!("{pad}{t}")).collect();
    let mut out = splice(src, at, at, &format!("\n{}", lines.join("\n")));
    if !out.ends_with('\n') {
        out.push('\n');
    }
    // an insertion at the very start of an empty file leaves a leading newline
    if src.trim().is_empty() {
        out = out.trim_start_matches('\n').to_string();
    }
    check(out)
}

/// The result must still parse without error.
fn check(out: String) -> Result<String, EditError> {
    if parse(&out).has_errors() {
        Err(EditError::Invalid)
    } else {
        Ok(out)
    }
}

/// Adds a card or a zone as the last child of `parent` (a zone id) or of the diagram (`None`).
pub fn add_node(src: &str, parent: Option<&str>, node: &NewNode) -> Result<String, EditError> {
    if !valid_id(&node.id) {
        return Err(EditError::BadValue(tr!(
            "invalid id: « {} »",
            "identifiant invalide : « {} »",
            node.id
        )));
    }
    if parse(src).model.node(&node.id).is_some() {
        return Err(EditError::BadValue(tr!(
            "the id « {} » is taken",
            "l'identifiant « {} » est déjà pris",
            node.id
        )));
    }
    append_child(src, parent, &node.line())
}

/// Adds an arrow `to(to)` under card `from`.
pub fn add_edge(src: &str, from: &str, to: &str) -> Result<String, EditError> {
    let doc = parse(src);
    if doc.model.node(to).is_none() {
        return Err(EditError::NotFound(to.to_string()));
    }
    append_child(src, Some(from), &format!("to({to})"))
}

/// Removes the lines `[start, end)` and the newline before them.
fn cut(src: &str, start: usize, end: usize) -> String {
    let from = if start > 0 && src.as_bytes()[start - 1] == b'\n' {
        start - 1
    } else {
        start
    };
    let out = splice(src, from, end, "");
    if from == 0 {
        out.trim_start_matches('\n').to_string()
    } else {
        out
    }
}

/// Removes a flag attribute (a target of `to(a b)`) with the space that separates it.
fn remove_flag(src: &str, span: crate::diag::Span) -> String {
    let before = src[..span.start].trim_end_matches([' ', '\t']).len();
    if before < span.start && !src[..before].ends_with('(') {
        return splice(src, before, span.end, "");
    }
    let after =
        span.end + (src[span.end..].len() - src[span.end..].trim_start_matches([' ', '\t']).len());
    splice(src, span.start, after, "")
}

/// Removes arrow `edge`: its `to(...)` line (with the lines under it), or only its target when
/// the tag has several (`to(a b)`).
pub fn remove_edge(src: &str, edge: &str) -> Result<String, EditError> {
    locate(src, &Target::Edge(edge.to_string()))?;
    check(remove_edge_unchecked(src, edge)?)
}

/// Removes card or zone `id` with everything under it, and the arrows that point to it.
pub fn remove_node(src: &str, id: &str) -> Result<String, EditError> {
    let l = locate(src, &Target::Node(id.to_string()))?;
    let mut out = cut(src, line_start(&l), block_end(src, &l));
    // arrows into it (or into a node inside it) from elsewhere
    loop {
        let doc = parse(&out);
        let gone = |n: &str| doc.model.node(n).is_none();
        let Some(e) = doc.model.edges.iter().find(|e| gone(&e.to)) else {
            break;
        };
        out = remove_edge_unchecked(&out, &e.id)?;
    }
    check(out)
}

/// `remove_edge` on a source that does not parse (its target is gone).
fn remove_edge_unchecked(src: &str, edge: &str) -> Result<String, EditError> {
    let doc = parse(src);
    let l = doc
        .map
        .edges
        .get(edge)
        .cloned()
        .ok_or_else(|| EditError::NotFound(edge.to_string()))?;
    let target = doc
        .model
        .edges
        .iter()
        .find(|e| e.id == edge)
        .map(|e| e.to.clone())
        .unwrap_or_default();
    let flags: Vec<_> = l.attrs.iter().filter(|a| a.value.is_none()).collect();
    if flags.len() > 1
        && let Some(a) = flags.iter().find(|a| a.name == target)
    {
        return Ok(remove_flag(src, a.span));
    }
    Ok(cut(src, line_start(&l), block_end(src, &l)))
}

/// Replaces the text of a located tag (title, label, line), or writes one after its head.
fn set_text(src: &str, l: &Located, text: &str) -> Result<String, EditError> {
    let text = one_line(text);
    let out = if let Some(t) = l.text {
        // an empty text also drops the space before it
        let start = if text.is_empty() {
            src[..t.start].trim_end_matches([' ', '\t']).len()
        } else {
            t.start
        };
        splice(src, start, t.end, &text)
    } else if text.is_empty() {
        src.to_string()
    } else {
        let head = l.attr_group.map_or_else(
            || {
                l.classes
                    .iter()
                    .map(|(_, s)| s.end)
                    .chain(l.id.map(|s| s.end))
                    .fold(l.name.end, usize::max)
            },
            |g| g.end,
        );
        splice(src, head, head, &format!(" {text}"))
    };
    check(out)
}

/// Sets the title of a card or zone (the text after its tag).
pub fn set_title(src: &str, id: &str, title: &str) -> Result<String, EditError> {
    let l = locate(src, &Target::Node(id.to_string()))?;
    set_text(src, &l, title)
}

/// Sets the label of an arrow (the text after `to(...)`); empty removes it. A tag
/// `to(a b)` carries both arrows: they share the label.
pub fn set_label(src: &str, edge: &str, label: &str) -> Result<String, EditError> {
    let l = locate(src, &Target::Edge(edge.to_string()))?;
    set_text(src, &l, label)
}

fn line_of(src: &str, id: &str, idx: usize) -> Result<Located, EditError> {
    let doc = parse(src);
    if doc.has_errors() {
        return Err(EditError::Invalid);
    }
    doc.map
        .lis
        .get(&(id.to_string(), idx))
        .cloned()
        .ok_or_else(|| EditError::NotFound(format!("{id} | {idx}")))
}

/// Replaces the text of line `idx` (`|`) of card or zone `id`.
pub fn set_line(src: &str, id: &str, idx: usize, text: &str) -> Result<String, EditError> {
    let l = line_of(src, id, idx)?;
    set_text(src, &l, text)
}

/// Removes line `idx` (`|`) of card or zone `id`.
pub fn remove_line(src: &str, id: &str, idx: usize) -> Result<String, EditError> {
    let l = line_of(src, id, idx)?;
    check(cut(src, line_start(&l), block_end(src, &l)))
}

/// Adds a line (`|`) to card or zone `id`, after its last line (or right under its tag, before
/// its arrows and children).
pub fn add_line(src: &str, id: &str, text: &str) -> Result<String, EditError> {
    let card = locate(src, &Target::Node(id.to_string()))?;
    let doc = parse(src);
    let last = (0..)
        .map_while(|k| doc.map.lis.get(&(id.to_string(), k)))
        .last()
        .cloned();
    let (at, indent) = match &last {
        Some(l) => (block_end(src, l), l.indent),
        None => (
            src[card.line.end..]
                .find('\n')
                .map_or(src.len(), |i| card.line.end + i),
            child_indent(src, &card),
        ),
    };
    let text = one_line(text);
    let line = if text.is_empty() {
        "|".to_string()
    } else {
        format!("| {text}")
    };
    let mut out = splice(src, at, at, &format!("\n{}{line}", " ".repeat(indent)));
    if !out.ends_with('\n') {
        out.push('\n');
    }
    check(out)
}

/// Changes the tag of a node (`zone` to `stack` and back, `card` to `cards`…): only the tag
/// name is rewritten. A stack has no title: a zone's title is kept in the text but ignored, with
/// the parser's warning.
pub fn set_tag(src: &str, id: &str, tag: &str) -> Result<String, EditError> {
    if !matches!(tag, "card" | "cards" | "zone" | "stack") {
        return Err(EditError::BadValue(tr!(
            "unknown tag: « {tag} »",
            "balise inconnue : « {tag} »"
        )));
    }
    let l = locate(src, &Target::Node(id.to_string()))?;
    check(splice(src, l.name.start, l.name.end, tag))
}

/// Moves card or zone `id` (with everything under it) to the end of zone `parent`, or of the
/// diagram (`None`), re-indented. A zone cannot go into itself.
pub fn move_into(src: &str, id: &str, parent: Option<&str>) -> Result<String, EditError> {
    let doc = parse(src);
    if let Some(p) = parent {
        let inside = std::iter::successors(Some(p.to_string()), |n| {
            doc.model.node(n).and_then(|x| x.parent.clone())
        })
        .any(|n| n == id);
        if inside {
            return Err(EditError::BadValue(tr!(
                "« {id} » cannot go into itself",
                "« {id} » ne peut pas entrer en lui-même"
            )));
        }
    }
    if doc.model.node(id).and_then(|n| n.parent.as_deref()) == parent {
        return Ok(src.to_string());
    }
    let l = locate(src, &Target::Node(id.to_string()))?;
    let (start, end) = (line_start(&l), block_end(src, &l));
    let block: Vec<String> = src[start..end]
        .lines()
        .map(|t| {
            let i = t.len() - t.trim_start().len();
            format!(
                "{}{}",
                " ".repeat(i.saturating_sub(l.indent)),
                t.trim_start()
            )
        })
        .collect();
    let rest = cut(src, start, end);
    append_unchecked(&rest, parent, &block.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SRC: &str = "//- c\ndiagram(dir=\"down\")\n  card#a A\n    | x\n    to(b c) go\n  zone#z Z\n    card#b B\n  card#c C\n";

    #[test]
    fn adds_a_card_at_the_top_level_and_in_a_zone() {
        let out = add_node(SRC, None, &NewNode::card("d", "D").with_attr("at", "8,16")).unwrap();
        assert!(
            out.ends_with("  card#c C\n  card#d(at=\"8,16\") D\n"),
            "{out}"
        );
        let out = add_node(SRC, Some("z"), &NewNode::card("e", "E").with_class("note")).unwrap();
        assert!(
            out.contains("    card#b B\n    card#e.note E\n  card#c C"),
            "{out}"
        );
    }

    #[test]
    fn adds_to_an_empty_file_and_without_diagram() {
        assert_eq!(
            add_node("", None, &NewNode::card("a", "A")).unwrap(),
            "card#a A\n"
        );
        assert_eq!(
            add_node("card#a A\n", None, &NewNode::zone("z", "Z")).unwrap(),
            "card#a A\nzone#z Z\n"
        );
    }

    #[test]
    fn refuses_a_taken_or_invalid_id() {
        assert!(add_node(SRC, None, &NewNode::card("a", "A")).is_err());
        assert!(add_node(SRC, None, &NewNode::card("a b", "A")).is_err());
        assert_eq!(fresh_id(SRC, "card"), "card1");
    }

    #[test]
    fn adds_an_arrow_under_its_source() {
        let out = add_edge(SRC, "c", "a").unwrap();
        assert!(out.ends_with("  card#c C\n    to(a)\n"), "{out}");
        assert!(add_edge(SRC, "c", "nope").is_err());
    }

    #[test]
    fn removes_one_target_of_two_then_the_line() {
        let one = remove_edge(SRC, "a->b").unwrap();
        assert!(one.contains("    to(c) go\n"), "{one}");
        let none = remove_edge(&one, "a->c").unwrap();
        assert!(!none.contains("to("), "{none}");
        assert!(none.contains("  card#a A\n    | x\n  zone#z Z"), "{none}");
    }

    #[test]
    fn removes_a_node_its_block_and_the_arrows_into_it() {
        let out = remove_node(SRC, "z").unwrap();
        assert!(!out.contains("zone#z") && !out.contains("card#b"), "{out}");
        assert!(out.contains("    to(c) go\n"), "{out}");
        assert!(parse(&out).diagnostics.iter().all(|d| !d.is_error()));
    }

    #[test]
    fn sets_or_adds_a_title() {
        let out = set_title(SRC, "a", "New  name").unwrap();
        assert!(out.contains("  card#a New name\n"), "{out}");
        let out = set_title("card#a(at=\"1,2\")\n", "a", "T").unwrap();
        assert_eq!(out, "card#a(at=\"1,2\") T\n");
    }

    #[test]
    fn sets_or_removes_an_arrow_label() {
        let out = set_label(SRC, "a->b", "HTTPS").unwrap();
        assert!(out.contains("    to(b c) HTTPS\n"), "{out}");
        let none = set_label(&out, "a->c", "").unwrap();
        assert!(none.contains("    to(b c)\n"), "{none}");
        let new = set_label("card#a A\n  to(b)\ncard#b B\n", "a->b", "x").unwrap();
        assert_eq!(new, "card#a A\n  to(b) x\ncard#b B\n");
    }

    #[test]
    fn adds_changes_and_removes_lines() {
        let out = add_line(SRC, "a", "second").unwrap();
        assert!(
            out.contains("    | x\n    | second\n    to(b c) go"),
            "{out}"
        );
        let out = set_line(&out, "a", 0, "first").unwrap();
        assert!(out.contains("    | first\n    | second\n"), "{out}");
        let out = remove_line(&out, "a", 0).unwrap();
        assert!(
            out.contains("  card#a A\n    | second\n    to(b c)"),
            "{out}"
        );
        // a card without lines gets one right under its tag, before its arrows
        let c = add_line("card#a A\n  to(b)\ncard#b B\n", "a", "10.0.0.1").unwrap();
        assert_eq!(c, "card#a A\n  | 10.0.0.1\n  to(b)\ncard#b B\n");
        let b = add_line("card#a A\ncard#b B\n", "b", "y").unwrap();
        assert_eq!(b, "card#a A\ncard#b B\n  | y\n");
        assert!(set_line(SRC, "a", 5, "z").is_err());
    }

    #[test]
    fn turns_a_zone_into_a_stack_and_back() {
        let out = set_tag(SRC, "z", "stack").unwrap();
        assert!(out.contains("  stack#z Z\n"), "{out}");
        assert_eq!(set_tag(&out, "z", "zone").unwrap(), SRC);
        assert!(set_tag(SRC, "z", "div").is_err());
    }

    #[test]
    fn moves_a_card_into_a_zone_and_out() {
        let into = move_into(SRC, "c", Some("z")).unwrap();
        assert!(into.contains("    card#b B\n    card#c C\n"), "{into}");
        let out = move_into(&into, "b", None).unwrap();
        assert!(out.ends_with("  card#b B\n"), "{out}");
        assert!(move_into(SRC, "z", Some("z")).is_err());
        // a card with its lines keeps them, re-indented
        let a = move_into(SRC, "a", Some("z")).unwrap();
        assert!(
            a.contains("    card#a A\n      | x\n      to(b c) go\n"),
            "{a}"
        );
    }
}
