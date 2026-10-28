//! Syntax: a subset of Pug (FORMAT §2), read line by line, into a tree of tags that keeps the
//! position of each piece in the source (the base of surgical editing).

use crate::diag::{Diagnostic, Span};
use crate::tr;

/// Un attribut `nom="valeur"`, `nom=3` ou `nom` (drapeau).
#[derive(Debug, Clone, PartialEq)]
pub struct Attr {
    pub name: String,
    /// Unquoted value; `None` for a flag.
    pub value: Option<String>,
    /// Attribut complet (nom et valeur).
    pub span: Span,
    pub name_span: Span,
    /// Valeur, guillemets compris.
    pub value_span: Option<Span>,
}

/// A tag and its indented block.
#[derive(Debug, Clone, PartialEq)]
pub struct Tag {
    pub name: String,
    pub name_span: Span,
    pub id: Option<(String, Span)>,
    pub classes: Vec<(String, Span)>,
    pub attrs: Vec<Attr>,
    /// Group `( … )`, parentheses included.
    pub attr_group: Option<Span>,
    /// Text on the tag's line, as is (not split), and its position. For a line `| …` (tag name `|`),
    /// everything after the bar, untrimmed: `|.warn x` and `| .env` differ.
    pub text: Option<(String, Span)>,
    /// The whole line (indentation excluded).
    pub line: Span,
    /// Indentation of the line, in characters.
    pub indent: usize,
    pub children: Vec<Tag>,
}

struct Line<'a> {
    start: usize,
    indent: usize,
    body: &'a str,
    body_start: usize,
}

/// Splits the source into a tree of tags. Never panics: errors go into `diags`.
pub fn parse_tree(src: &str, diags: &mut Vec<Diagnostic>) -> Vec<Tag> {
    let mut lines = Vec::new();
    let mut offset = 0;
    let mut indent_kind: Option<char> = None;
    for raw in src.split_inclusive('\n') {
        let start = offset;
        offset += raw.len();
        let text = raw.trim_end_matches(['\n', '\r']);
        let body = text.trim_start_matches([' ', '\t']);
        if body.is_empty() {
            continue;
        }
        let ws = &text[..text.len() - body.len()];
        for c in ws.chars() {
            match indent_kind {
                None => indent_kind = Some(c),
                Some(k) if k != c => {
                    diags.push(
                        Diagnostic::error(
                            "mixed-indent",
                            Span::new(start, start + ws.len()),
                            tr!("indentation mixing spaces and tabs", "indentation mélangeant espaces et tabulations"),
                        )
                        .with_help(tr!("use only spaces (2 per level recommended) or only tabs", "utiliser uniquement des espaces (2 par niveau recommandés) ou uniquement des tabulations")),
                    );
                    break;
                }
                _ => {}
            }
        }
        lines.push(Line {
            start,
            indent: ws.chars().count(),
            body,
            body_start: start + ws.len(),
        });
    }

    // Tree by indentation: a line's parent is the last line less indented.
    let mut roots: Vec<Tag> = Vec::new();
    let mut stack: Vec<Tag> = Vec::new(); // open tags, from the root to the deepest
    let mut skip_deeper_than: Option<usize> = None; // commentaire de bloc en cours
    for l in &lines {
        if let Some(d) = skip_deeper_than {
            if l.indent > d {
                continue;
            }
            skip_deeper_than = None;
        }
        while stack.last().is_some_and(|t| t.indent >= l.indent) {
            close(&mut stack, &mut roots);
        }
        if l.body.starts_with("//") {
            skip_deeper_than = Some(l.indent);
            continue;
        }
        if let Some(rest) = l.body.strip_prefix('|') {
            // a line (FORMAT §3.2.3): a child tag named `|`, kept in writing order among the
            // other children; it opens no block
            match stack.last_mut() {
                Some(parent) => parent
                    .children
                    .push(text_line(rest, l.body_start, l.indent)),
                None => diags.push(Diagnostic::warning(
                    "orphan-text",
                    Span::new(l.body_start, l.body_start + l.body.len()),
                    tr!(
                        "« | » text outside any tag: ignored",
                        "texte « | » hors de toute balise : ignoré"
                    ),
                )),
            }
            continue;
        }
        let _ = l.start;
        if let Some(mut tag) = parse_line(l.body, l.body_start, diags) {
            tag.indent = l.indent;
            stack.push(tag);
        }
    }
    while !stack.is_empty() {
        close(&mut stack, &mut roots);
    }
    roots
}

/// The tag of a line `|…`: name `|` (the bar), text = everything after the bar.
fn text_line(rest: &str, body_start: usize, indent: usize) -> Tag {
    let s = body_start + 1;
    Tag {
        name: "|".into(),
        name_span: Span::new(body_start, s),
        id: None,
        classes: Vec::new(),
        attrs: Vec::new(),
        attr_group: None,
        text: Some((rest.to_string(), Span::new(s, s + rest.len()))),
        line: Span::new(body_start, s + rest.len()),
        indent,
        children: Vec::new(),
    }
}

fn close(stack: &mut Vec<Tag>, roots: &mut Vec<Tag>) {
    if let Some(t) = stack.pop() {
        match stack.last_mut() {
            Some(p) => p.children.push(t),
            None => roots.push(t),
        }
    }
}

fn is_ident(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '-'
}

/// End of the identifier that starts at `from` in `body`.
fn ident_end(body: &str, from: usize) -> usize {
    let mut e = from;
    while let Some(c) = body[e..].chars().next().filter(|c| is_ident(*c)) {
        e += c.len_utf8();
    }
    e
}

/// Parses a tag line: `name#id.c1(attrs).c2 text`.
pub(crate) fn parse_line(body: &str, base: usize, diags: &mut Vec<Diagnostic>) -> Option<Tag> {
    let i = ident_end(body, 0);
    let name = &body[..i];
    // Pug accepte `#id` ou `.classe` sans nom de balise (div implicite).
    if name.is_empty() && !matches!(body.as_bytes().first(), Some(b'#' | b'.')) {
        diags.push(Diagnostic::error(
            "syntax",
            Span::new(base, base + body.len()),
            tr!(
                "unreadable line: a tag (diagram, title, card, zone, to) or a line « | » is expected",
                "ligne illisible : une balise (diagram, title, card, zone, to) ou une ligne « | » est attendue"
            ),
        ));
        return None;
    }
    let mut tag = Tag {
        name: if name.is_empty() {
            "div".into()
        } else {
            name.to_string()
        },
        name_span: Span::new(base, base + i),
        id: None,
        classes: Vec::new(),
        attrs: Vec::new(),
        attr_group: None,
        text: None,
        line: Span::new(base, base + body.len()),
        indent: 0,
        children: Vec::new(),
    };
    let i = parse_decorations(body, i, base, &mut tag, diags);
    parse_trailing_text(body, i, base, &mut tag, diags);
    Some(tag)
}

/// `#id`, `.class` and `(attrs)` after the name; returns the index where the rest starts.
fn parse_decorations(
    body: &str,
    mut i: usize,
    base: usize,
    tag: &mut Tag,
    diags: &mut Vec<Diagnostic>,
) -> usize {
    loop {
        match body.as_bytes().get(i) {
            Some(b'#' | b'.') => i = parse_id_or_class(body, i, base, tag, diags),
            Some(b'(') => i = parse_attr_group(body, i, base, tag, diags),
            _ => return i,
        }
    }
}

/// `#id` ou `.classe` en `i` ; renvoie l'indice suivant.
fn parse_id_or_class(
    body: &str,
    i: usize,
    base: usize,
    tag: &mut Tag,
    diags: &mut Vec<Diagnostic>,
) -> usize {
    let kind = body.as_bytes()[i];
    let s = i + 1;
    let e = ident_end(body, s);
    if e == s {
        // `card.` (a Pug text block) or punctuation: not handled
        diags.push(Diagnostic::error(
            "syntax",
            Span::new(base + i, base + i + 1),
            tr!(
                "« {} » must be followed by an identifier",
                "« {} » doit être suivi d'un identifiant",
                kind as char
            ),
        ));
        return i + 1;
    }
    let v = body[s..e].to_string();
    let sp = Span::new(base + s, base + e);
    if kind != b'#' {
        tag.classes.push((v, sp));
    } else if tag.id.is_some() {
        diags.push(Diagnostic::error(
            "syntax",
            Span::new(base + i, base + e),
            tr!(
                "two « # » identifiers on the same tag",
                "deux identifiants « # » sur la même balise"
            ),
        ));
    } else {
        tag.id = Some((v, sp));
    }
    e
}

/// Group `( … )` at `i`; a second group on the same tag is an error.
fn parse_attr_group(
    body: &str,
    i: usize,
    base: usize,
    tag: &mut Tag,
    diags: &mut Vec<Diagnostic>,
) -> usize {
    let close = parse_attrs(body, i, base, tag, diags);
    if tag.attr_group.is_some() {
        diags.push(
            Diagnostic::error(
                "multiple-attr-groups",
                Span::new(base + i, base + close),
                tr!("several attribute groups on one tag", "plusieurs groupes d'attributs sur une balise"),
            )
            .with_help(tr!(
                "put the attributes in a single pair of parentheses: to(core side=\"right-top\").dashed",
                "regrouper les attributs dans une seule paire de parenthèses : to(core side=\"right-top\").dashed"
            )),
        );
    } else {
        tag.attr_group = Some(Span::new(base + i, base + close));
    }
    close
}

/// Text of the tag (after a space), or an error if something else follows.
fn parse_trailing_text(
    body: &str,
    i: usize,
    base: usize,
    tag: &mut Tag,
    diags: &mut Vec<Diagnostic>,
) {
    let rest = &body[i..];
    if rest.is_empty() {
        return;
    }
    if let Some(t) = rest.strip_prefix(' ') {
        let trimmed = t.trim();
        if !trimmed.is_empty() {
            let s = i + 1 + (t.len() - t.trim_start().len());
            tag.text = Some((
                trimmed.to_string(),
                Span::new(base + s, base + s + trimmed.len()),
            ));
        }
    } else if rest.starts_with(':') {
        diags.push(Diagnostic::error(
            "syntax",
            Span::new(base + i, base + body.len()),
            tr!(
                "inline nesting « : » is not supported: put the child on the next line, indented",
                "imbrication en ligne « : » non prise en charge : mettre l'enfant sur la ligne suivante, indenté"
            ),
        ));
    } else {
        diags.push(Diagnostic::error(
            "syntax",
            Span::new(base + i, base + body.len()),
            tr!(
                "unexpected character « {} » after the tag",
                "caractère inattendu « {} » après la balise",
                rest.chars().next().unwrap_or(' ')
            ),
        ));
    }
}

/// Reads `( … )` from `open`; returns the index just after `)`.
fn parse_attrs(
    body: &str,
    open: usize,
    base: usize,
    tag: &mut Tag,
    diags: &mut Vec<Diagnostic>,
) -> usize {
    let mut attrs = Vec::new();
    let mut i = open + 1;
    let at = |i: usize| body[i..].chars().next();
    loop {
        while at(i).is_some_and(|c| c == ' ' || c == ',' || c == '\t') {
            i += 1;
        }
        let Some(c) = at(i) else {
            diags.push(Diagnostic::error(
                "syntax",
                Span::new(base + open, base + body.len()),
                tr!("unclosed parenthesis « ( »", "parenthèse « ( » non fermée"),
            ));
            break;
        };
        if c == ')' {
            i += 1;
            break;
        }
        let ns = i;
        while let Some(c) =
            at(i).filter(|c| !matches!(c, '=' | ' ' | ',' | ')' | '\t' | '"' | '\''))
        {
            i += c.len_utf8();
        }
        if i == ns {
            diags.push(Diagnostic::error(
                "syntax",
                Span::new(base + i, base + i + 1),
                tr!(
                    "unexpected character « {c} » in the attributes",
                    "caractère inattendu « {c} » dans les attributs"
                ),
            ));
            i += c.len_utf8();
            continue;
        }
        let name = body[ns..i].to_string();
        let name_span = Span::new(base + ns, base + i);
        if at(i) == Some('=') {
            i += 1;
            let vs = i;
            let value = if let Some(q @ ('"' | '\'')) = at(i) {
                i += 1;
                let mut v = String::new();
                let mut closed = false;
                while let Some(c) = at(i) {
                    i += c.len_utf8();
                    if c == '\\' {
                        if let Some(n) = at(i) {
                            v.push(n);
                            i += n.len_utf8();
                        }
                    } else if c == q {
                        closed = true;
                        break;
                    } else {
                        v.push(c);
                    }
                }
                if !closed {
                    diags.push(Diagnostic::error(
                        "syntax",
                        Span::new(base + vs, base + i),
                        tr!("unterminated string", "chaîne non fermée"),
                    ));
                }
                v
            } else {
                while let Some(c) = at(i).filter(|c| !matches!(c, ' ' | ',' | ')' | '\t')) {
                    i += c.len_utf8();
                }
                body[vs..i].to_string()
            };
            attrs.push(Attr {
                name,
                value: Some(value),
                span: Span::new(base + ns, base + i),
                name_span,
                value_span: Some(Span::new(base + vs, base + i)),
            });
        } else {
            attrs.push(Attr {
                name,
                value: None,
                span: name_span,
                name_span,
                value_span: None,
            });
        }
    }
    tag.attrs.extend(attrs);
    i
}
