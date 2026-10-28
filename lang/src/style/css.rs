//! Reading a stylesheet: rules, selectors, declarations, `@media` blocks.
//! Minimal on purpose: the subset the engine needs (SPEC §4). Unknown `@…` rules (`@font-face`,
//! `@keyframes`…) are ignored for resolution; the whole CSS is still passed as is to the HTML
//! rendering.

use super::select::{Selector, parse_selector_list};

/// Context of a rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Media {
    All,
    Print,
    Dark,
    /// Condition not handled: the rule never applies to resolution.
    Other,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Declaration {
    pub name: String,
    pub value: String,
    pub important: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Rule {
    pub selectors: Vec<Selector>,
    pub declarations: Vec<Declaration>,
    pub media: Media,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Stylesheet {
    pub rules: Vec<Rule>,
}

fn strip_comments(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let mut rest = src;
    loop {
        // beware of strings: a "/*" in a string is not a comment
        let mut i = 0;
        let b = rest.as_bytes();
        let mut found = None;
        while i < b.len() {
            match b[i] {
                q @ (b'"' | b'\'') => {
                    i += 1;
                    while i < b.len() && b[i] != q {
                        if b[i] == b'\\' {
                            i += 1;
                        }
                        i += 1;
                    }
                    i += 1;
                }
                b'/' if b.get(i + 1) == Some(&b'*') => {
                    found = Some(i);
                    break;
                }
                _ => i += 1,
            }
        }
        if let Some(s) = found {
            out.push_str(&rest[..s]);
            out.push(' ');
            rest = match rest[s + 2..].find("*/") {
                Some(e) => &rest[s + 2 + e + 2..],
                None => "",
            };
        } else {
            out.push_str(rest);
            return out;
        }
    }
}

/// Index of the closing brace matching the one at `open`, strings skipped.
fn matching_brace(s: &str, open: usize) -> usize {
    let b = s.as_bytes();
    let mut depth = 0;
    let mut i = open;
    while i < b.len() {
        match b[i] {
            b'"' | b'\'' => {
                let q = b[i];
                i += 1;
                while i < b.len() && b[i] != q {
                    if b[i] == b'\\' {
                        i += 1;
                    }
                    i += 1;
                }
            }
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return i;
                }
            }
            _ => {}
        }
        i += 1;
    }
    s.len()
}

/// Splits on `sep` at level 0 (outside parentheses and strings).
pub fn split_top(s: &str, sep: char) -> Vec<&str> {
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut quote: Option<char> = None;
    let mut start = 0;
    let mut esc = false;
    for (i, c) in s.char_indices() {
        if esc {
            esc = false;
            continue;
        }
        match (quote, c) {
            (_, '\\') => esc = true,
            (Some(q), c) if c == q => quote = None,
            (None, '"' | '\'') => quote = Some(c),
            (None, '(' | '[') => depth += 1,
            (None, ')' | ']') => depth -= 1,
            (None, c) if c == sep && depth == 0 => {
                out.push(&s[start..i]);
                start = i + c.len_utf8();
            }
            _ => {}
        }
    }
    out.push(&s[start..]);
    out
}

pub(crate) fn parse_declarations(block: &str) -> Vec<Declaration> {
    let mut out = Vec::new();
    for d in split_top(block, ';') {
        let Some((name, value)) = d.split_once(':') else {
            continue;
        };
        let name = name.trim();
        if name.is_empty() {
            continue;
        }
        let mut value = value.trim().to_string();
        let mut important = false;
        if let Some(i) = value.to_ascii_lowercase().rfind("!important") {
            important = true;
            value.truncate(i);
            value = value.trim().to_string();
        }
        let name = if name.starts_with("--") {
            name.to_string()
        } else {
            name.to_ascii_lowercase()
        };
        out.push(Declaration {
            name,
            value,
            important,
        });
    }
    out
}

fn media_of(prelude: &str) -> Media {
    let p = prelude.to_ascii_lowercase();
    let p = p.trim_start_matches("@media").trim();
    if p == "print" {
        Media::Print
    } else if p.replace(' ', "").contains("prefers-color-scheme:dark") {
        Media::Dark
    } else if p == "screen" || p == "all" {
        Media::All
    } else {
        Media::Other
    }
}

fn parse_block(src: &str, media: Media, out: &mut Vec<Rule>) {
    let mut i = 0;
    while i < src.len() {
        let Some(rel) = src[i..].find(['{', ';', '}']) else {
            break;
        };
        let at = i + rel;
        let prelude = src[i..at].trim();
        match src.as_bytes()[at] {
            b';' | b'}' => {
                // `@import …;` or a stray declaration: ignored
                i = at + 1;
            }
            _ => {
                let close = matching_brace(src, at);
                let body = &src[at + 1..close.min(src.len())];
                if prelude.starts_with('@') {
                    if prelude.to_ascii_lowercase().starts_with("@media") {
                        let m = match (media, media_of(prelude)) {
                            (Media::All, m) | (m, Media::All) => m,
                            _ => Media::Other,
                        };
                        parse_block(body, m, out);
                    }
                    // other @ rules: ignored
                } else {
                    let selectors = parse_selector_list(prelude);
                    if !selectors.is_empty() {
                        out.push(Rule {
                            selectors,
                            declarations: parse_declarations(body),
                            media,
                        });
                    }
                }
                i = close + 1;
            }
        }
    }
}

impl Stylesheet {
    /// Reads a stylesheet; never panics (an unreadable rule is ignored).
    pub fn parse(src: &str) -> Stylesheet {
        let clean = strip_comments(src);
        let mut rules = Vec::new();
        parse_block(&clean, Media::All, &mut rules);
        Stylesheet { rules }
    }
}
