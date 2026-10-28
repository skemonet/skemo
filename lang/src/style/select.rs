//! CSS selectors (a subset): element, `*`, `.class`, `#id`, `[attr]`, `[attr="v"]`, `:not(…)`,
//! `:first-child`, `:last-child`, `::before`/`::after`, descendant and `>` combinators. `:has()` and
//! unknown pseudo-classes never match.

use super::css::split_top;

#[derive(Debug, Clone, PartialEq)]
pub enum Simple {
    Tag(String),
    Universal,
    Class(String),
    Id(String),
    Attr(String, Option<String>),
    Not(Vec<Compound>),
    FirstChild,
    LastChild,
    Never,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Compound {
    pub simples: Vec<Simple>,
    /// Pseudo-element aimed at (`before`, `after`).
    pub pseudo: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Combinator {
    Descendant,
    Child,
}

/// A complex selector, read from left to right: `compounds[0] comb[0] compounds[1] …`.
#[derive(Debug, Clone, PartialEq)]
pub struct Selector {
    pub compounds: Vec<Compound>,
    pub combinators: Vec<Combinator>,
    pub specificity: (u32, u32, u32),
}

/// Element as the selector engine sees it.
pub trait Element {
    fn tag(&self) -> &str;
    fn id(&self) -> Option<&str>;
    fn has_class(&self, c: &str) -> bool;
    fn attr(&self, name: &str) -> Option<&str>;
    fn parent(&self) -> Option<Self>
    where
        Self: Sized;
    fn is_first_child(&self) -> bool;
    fn is_last_child(&self) -> bool;
}

fn ident_end(s: &str) -> usize {
    s.char_indices()
        .find(|(_, c)| !(c.is_alphanumeric() || *c == '-' || *c == '_'))
        .map_or(s.len(), |(i, _)| i)
}

fn parse_compound(s: &str) -> Option<Compound> {
    let mut c = Compound::default();
    let mut i = 0;
    let b = s.as_bytes();
    while i < s.len() {
        match b[i] {
            b'*' => {
                c.simples.push(Simple::Universal);
                i += 1;
            }
            b'.' | b'#' => {
                let e = i + 1 + ident_end(&s[i + 1..]);
                if e == i + 1 {
                    return None;
                }
                let v = s[i + 1..e].to_string();
                c.simples.push(if b[i] == b'.' {
                    Simple::Class(v)
                } else {
                    Simple::Id(v)
                });
                i = e;
            }
            b'[' => {
                let e = i + s[i..].find(']')?;
                let inner = &s[i + 1..e];
                match inner.split_once('=') {
                    Some((n, v)) => {
                        let v = v.trim().trim_matches(['"', '\'']).to_string();
                        c.simples.push(Simple::Attr(n.trim().to_string(), Some(v)));
                    }
                    None => c.simples.push(Simple::Attr(inner.trim().to_string(), None)),
                }
                i = e + 1;
            }
            b':' => {
                let double = b.get(i + 1) == Some(&b':');
                let ns = i + if double { 2 } else { 1 };
                let ne = ns + ident_end(&s[ns..]);
                let name = s[ns..ne].to_ascii_lowercase();
                let mut end = ne;
                let mut arg = None;
                if b.get(ne) == Some(&b'(') {
                    let mut depth = 0;
                    let mut j = ne;
                    while j < s.len() {
                        match b[j] {
                            b'(' => depth += 1,
                            b')' => {
                                depth -= 1;
                                if depth == 0 {
                                    break;
                                }
                            }
                            _ => {}
                        }
                        j += 1;
                    }
                    arg = Some(&s[ne + 1..j.min(s.len())]);
                    end = (j + 1).min(s.len());
                }
                match (double || name == "before" || name == "after", name.as_str()) {
                    (true, "before" | "after") => c.pseudo = Some(name.clone()),
                    (false, "not") => {
                        let inner: Vec<Compound> = split_top(arg.unwrap_or(""), ',')
                            .into_iter()
                            .filter_map(|a| parse_compound(a.trim()))
                            .collect();
                        c.simples.push(Simple::Not(inner));
                    }
                    (false, "first-child") => c.simples.push(Simple::FirstChild),
                    (false, "last-child") => c.simples.push(Simple::LastChild),
                    (false, "root") => c.simples.push(Simple::Tag(":root".into())),
                    _ => c.simples.push(Simple::Never),
                }
                i = end;
            }
            _ => {
                let e = i + ident_end(&s[i..]);
                if e == i {
                    return None;
                }
                c.simples.push(Simple::Tag(s[i..e].to_ascii_lowercase()));
                i = e;
            }
        }
    }
    Some(c)
}

fn specificity_of(c: &Compound, sp: &mut (u32, u32, u32)) {
    for s in &c.simples {
        match s {
            Simple::Id(_) => sp.0 += 1,
            Simple::Class(_)
            | Simple::Attr(..)
            | Simple::FirstChild
            | Simple::LastChild
            | Simple::Never => sp.1 += 1,
            Simple::Tag(t) if t == ":root" => sp.1 += 1,
            Simple::Tag(_) => sp.2 += 1,
            Simple::Not(inner) => {
                // specificity of the most specific argument
                let best = inner
                    .iter()
                    .map(|c| {
                        let mut s = (0, 0, 0);
                        specificity_of(c, &mut s);
                        s
                    })
                    .max()
                    .unwrap_or((0, 0, 0));
                sp.0 += best.0;
                sp.1 += best.1;
                sp.2 += best.2;
            }
            Simple::Universal => {}
        }
    }
    if c.pseudo.is_some() {
        sp.2 += 1;
    }
}

pub fn parse_selector(s: &str) -> Option<Selector> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    // split into pieces separated by blanks or ">", outside brackets and parentheses
    let mut parts: Vec<String> = Vec::new();
    let mut combs: Vec<Combinator> = Vec::new();
    let mut cur = String::new();
    let mut depth = 0;
    let mut pending: Option<Combinator> = None;
    for ch in s.chars() {
        match ch {
            '(' | '[' => {
                depth += 1;
                cur.push(ch);
            }
            ')' | ']' => {
                depth -= 1;
                cur.push(ch);
            }
            ' ' | '\t' | '\n' if depth == 0 => {
                if !cur.is_empty() {
                    parts.push(std::mem::take(&mut cur));
                    pending.get_or_insert(Combinator::Descendant);
                }
            }
            '>' if depth == 0 => {
                if !cur.is_empty() {
                    parts.push(std::mem::take(&mut cur));
                }
                pending = Some(Combinator::Child);
            }
            '+' | '~' if depth == 0 => return None, // sibling combinators: not handled
            _ => {
                if let Some(c) = pending.take()
                    && !parts.is_empty()
                {
                    combs.push(c);
                }
                cur.push(ch);
            }
        }
    }
    if !cur.is_empty() {
        parts.push(cur);
    }
    if parts.is_empty() || combs.len() + 1 != parts.len() {
        return None;
    }
    let compounds: Option<Vec<Compound>> = parts.iter().map(|p| parse_compound(p)).collect();
    let compounds = compounds?;
    let mut specificity = (0, 0, 0);
    for c in &compounds {
        specificity_of(c, &mut specificity);
    }
    Some(Selector {
        compounds,
        combinators: combs,
        specificity,
    })
}

pub fn parse_selector_list(s: &str) -> Vec<Selector> {
    split_top(s, ',')
        .into_iter()
        .filter_map(parse_selector)
        .collect()
}

fn matches_compound<E: Element>(c: &Compound, e: &E) -> bool {
    c.simples.iter().all(|s| match s {
        Simple::Universal => true,
        Simple::Tag(t) if t == ":root" => e.parent().is_none(),
        Simple::Tag(t) => e.tag().eq_ignore_ascii_case(t),
        Simple::Class(k) => e.has_class(k),
        Simple::Id(i) => e.id() == Some(i.as_str()),
        Simple::Attr(n, v) => match (e.attr(n), v) {
            (Some(_), None) => true,
            (Some(a), Some(v)) => a == v,
            _ => false,
        },
        Simple::Not(inner) => !inner
            .iter()
            .any(|c| c.pseudo.is_none() && matches_compound(c, e)),
        Simple::FirstChild => e.is_first_child(),
        Simple::LastChild => e.is_last_child(),
        Simple::Never => false,
    })
}

impl Selector {
    /// Pseudo-element the selector aims at (`None` = the element itself).
    pub fn pseudo(&self) -> Option<&str> {
        self.compounds.last().and_then(|c| c.pseudo.as_deref())
    }

    /// Match with `e` (the pseudo-element, if any, is checked by the caller).
    pub fn matches<E: Element>(&self, e: &E) -> bool {
        let n = self.compounds.len();
        if !matches_compound(&self.compounds[n - 1], e) {
            return false;
        }
        self.match_from(n - 1, e)
    }

    /// `e` already matches compound `k`; checks the left part.
    fn match_from<E: Element>(&self, k: usize, e: &E) -> bool {
        if k == 0 {
            return true;
        }
        let comb = self.combinators[k - 1];
        let target = &self.compounds[k - 1];
        let mut cur = e.parent();
        while let Some(p) = cur {
            if matches_compound(target, &p) && self.match_from(k - 1, &p) {
                return true;
            }
            if comb == Combinator::Child {
                return false;
            }
            cur = p.parent();
        }
        false
    }
}
