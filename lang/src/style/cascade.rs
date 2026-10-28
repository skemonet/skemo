//! Cascade, inheritance and computed values on a tree of elements built by the engine.
//!
//! Semantics followed (CSS Cascade 4, Custom Properties 1):
//! - origins: user-agent sheet (minimal) < author sheets; author `!important` above;
//! - for one origin: specificity, then writing order;
//! - custom properties always inherit, and `var()` is substituted in them **on the element where
//!   they are declared** (pitfall P27);
//! - shorthands are expanded after `var()` substitution.

use super::css::Rule;
use super::css::{Declaration, Media, Stylesheet, split_top};
use super::select::{Element, Selector, Simple};
use super::values::{Rgba, parse_color, parse_length};
use std::collections::BTreeMap;

/// User-agent sheet: only what matters for the geometry of the elements the engine creates.
const UA: &str = "div, ul, li, card, zone, p { display: block; } span { display: inline; }
ul { margin: 1em 0; padding-left: 40px; } li { display: list-item; }";

/// Inherited properties (besides the custom properties).
const INHERITED: [&str; 13] = [
    "color",
    "font-family",
    "font-size",
    "font-style",
    "font-weight",
    "line-height",
    "letter-spacing",
    "text-align",
    "text-transform",
    "white-space",
    "visibility",
    "list-style",
    "word-spacing",
];

#[derive(Debug, Clone, Default)]
struct StyleNode {
    tag: String,
    id: Option<String>,
    classes: Vec<String>,
    attrs: Vec<(String, String)>,
    parent: Option<usize>,
    children: Vec<usize>,
    /// Pseudo-element (`before`): `parent` is then the host element.
    pseudo: Option<String>,
    /// Inline style (`style="…"`), above the author sheets.
    inline: Vec<Declaration>,
}

/// Tree of elements (the virtual DOM the engine wants styled). Parents are added before their
/// children.
#[derive(Debug, Clone, Default)]
pub struct StyleTree {
    nodes: Vec<StyleNode>,
}

impl StyleTree {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds an element; returns its index.
    pub fn add(
        &mut self,
        parent: Option<usize>,
        tag: &str,
        id: Option<&str>,
        classes: &[&str],
    ) -> usize {
        let i = self.nodes.len();
        self.nodes.push(StyleNode {
            tag: tag.to_ascii_lowercase(),
            id: id.map(String::from),
            classes: classes
                .iter()
                .map(std::string::ToString::to_string)
                .collect(),
            parent,
            ..Default::default()
        });
        if let Some(p) = parent {
            self.nodes[p].children.push(i);
        }
        i
    }

    /// Adds the `::before` (or `::after`) pseudo-element of `host`.
    pub fn add_pseudo(&mut self, host: usize, pseudo: &str) -> usize {
        let i = self.nodes.len();
        self.nodes.push(StyleNode {
            tag: String::new(),
            parent: Some(host),
            pseudo: Some(pseudo.into()),
            ..Default::default()
        });
        i
    }

    pub fn add_class(&mut self, i: usize, c: &str) {
        if let Some(n) = self.nodes.get_mut(i)
            && !n.classes.iter().any(|k| k == c)
        {
            n.classes.push(c.to_string());
        }
    }

    pub fn set_attr(&mut self, i: usize, name: &str, value: &str) {
        if let Some(n) = self.nodes.get_mut(i) {
            n.attrs.retain(|(k, _)| k != name);
            n.attrs.push((name.to_string(), value.to_string()));
        }
    }

    /// Adds an inline style declaration (like `el.style.setProperty`).
    pub fn set_inline(&mut self, i: usize, name: &str, value: &str) {
        if let Some(n) = self.nodes.get_mut(i) {
            n.inline.retain(|d| d.name != name);
            n.inline.push(Declaration {
                name: name.to_string(),
                value: value.to_string(),
                important: false,
            });
        }
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}

#[derive(Clone, Copy)]
struct NodeRef<'a> {
    tree: &'a StyleTree,
    i: usize,
}

impl Element for NodeRef<'_> {
    fn tag(&self) -> &str {
        &self.tree.nodes[self.i].tag
    }
    fn id(&self) -> Option<&str> {
        self.tree.nodes[self.i].id.as_deref()
    }
    fn has_class(&self, c: &str) -> bool {
        self.tree.nodes[self.i].classes.iter().any(|k| k == c)
    }
    fn attr(&self, name: &str) -> Option<&str> {
        let n = &self.tree.nodes[self.i];
        match name {
            "id" => n.id.as_deref(),
            _ => n
                .attrs
                .iter()
                .find(|(k, _)| k == name)
                .map(|(_, v)| v.as_str()),
        }
    }
    fn parent(&self) -> Option<Self> {
        self.tree.nodes[self.i].parent.map(|p| NodeRef {
            tree: self.tree,
            i: p,
        })
    }
    fn is_first_child(&self) -> bool {
        match self.tree.nodes[self.i].parent {
            Some(p) => self.tree.nodes[p].children.first() == Some(&self.i),
            None => true,
        }
    }
    fn is_last_child(&self) -> bool {
        match self.tree.nodes[self.i].parent {
            Some(p) => self.tree.nodes[p].children.last() == Some(&self.i),
            None => true,
        }
    }
}

/// Conditions de rendu.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MediaContext {
    pub print: bool,
    pub dark: bool,
}

/// Computed style of an element: properties (longhands, `var()` substituted) and variables.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Computed {
    pub props: BTreeMap<String, String>,
    pub vars: BTreeMap<String, String>,
}

struct Entry<'a> {
    key: (u8, (u32, u32, u32), usize),
    decl: &'a Declaration,
}

/// Style resolution for a set of sheets (theme then custom CSS, in order).
pub struct Styler {
    ua: Stylesheet,
    sheets: Vec<Stylesheet>,
    /// The rules that apply in this media, in cascade order: (sheet, rule, origin, order). The
    /// sheet is 0 for the user-agent sheet, `k + 1` for `sheets[k]`; `order` counts every rule.
    rules: Vec<(usize, usize, u8, usize)>,
    /// Indices in `rules` by the id (`#x`), class (`.x`) or tag (`x`) one of a rule's selectors
    /// requires of the element it styles; the rules with a selector that requires none of them.
    by_key: std::collections::HashMap<String, Vec<usize>>,
    any: Vec<usize>,
}

impl Styler {
    pub fn new(sheets: Vec<Stylesheet>, media: MediaContext) -> Self {
        let ua = Stylesheet::parse(UA);
        let mut rules = Vec::new();
        let mut by_key: std::collections::HashMap<String, Vec<usize>> = Default::default();
        let mut any = Vec::new();
        let all = std::iter::once((0u8, &ua)).chain(sheets.iter().map(|s| (1u8, s)));
        let mut order = 0usize;
        for (k, (origin, sheet)) in all.enumerate() {
            for (r, rule) in sheet.rules.iter().enumerate() {
                order += 1;
                if !media_ok(media, rule.media) {
                    continue;
                }
                let at = rules.len();
                rules.push((k, r, origin, order));
                let mut keys: Vec<Option<String>> = rule.selectors.iter().map(key_of).collect();
                keys.sort();
                keys.dedup();
                for key in keys {
                    match key {
                        Some(key) => by_key.entry(key).or_default().push(at),
                        None => any.push(at),
                    }
                }
            }
        }
        Styler {
            ua,
            sheets,
            rules,
            by_key,
            any,
        }
    }

    fn rule(&self, (sheet, r, _, _): (usize, usize, u8, usize)) -> &Rule {
        let sheet = if sheet == 0 {
            &self.ua
        } else {
            &self.sheets[sheet - 1]
        };
        &sheet.rules[r]
    }

    /// The rules that may style `el`, in cascade order: those indexed by its id, its classes and
    /// its tag, and those without such a requirement.
    fn candidates(&self, el: &NodeRef) -> Vec<usize> {
        let node = &el.tree.nodes[el.i];
        let mut keys: Vec<String> = vec![node.tag.to_ascii_lowercase()];
        keys.extend(node.id.iter().map(|i| format!("#{i}")));
        keys.extend(node.classes.iter().map(|c| format!(".{c}")));
        let mut out = self.any.clone();
        for k in &keys {
            if let Some(v) = self.by_key.get(k) {
                out.extend_from_slice(v);
            }
        }
        out.sort_unstable();
        out.dedup();
        out
    }

    fn matching<'a>(&'a self, tree: &'a StyleTree, i: usize) -> Vec<Entry<'a>> {
        let node = &tree.nodes[i];
        let (target, pseudo) = match &node.pseudo {
            Some(p) => (node.parent.unwrap_or(i), Some(p.as_str())),
            None => (i, None),
        };
        let el = NodeRef { tree, i: target };
        let mut out = Vec::new();
        for at in self.candidates(&el) {
            let (_, _, origin, order) = self.rules[at];
            let rule = self.rule(self.rules[at]);
            let best = rule
                .selectors
                .iter()
                .filter(|s| s.pseudo() == pseudo && s.matches(&el))
                .map(|s| s.specificity)
                .max();
            if let Some(sp) = best {
                for d in &rule.declarations {
                    let o = if d.important { origin + 2 } else { origin };
                    out.push(Entry {
                        key: (o, sp, order),
                        decl: d,
                    });
                }
            }
        }
        for d in &node.inline {
            let o = if d.important { 3 } else { 1 };
            out.push(Entry {
                key: (o, (u32::MAX, 0, 0), usize::MAX),
                decl: d,
            });
        }
        out.sort_by_key(|e| e.key);
        out
    }

    /// Computes the style of every element of the tree.
    pub fn compute(&self, tree: &StyleTree) -> Vec<Computed> {
        let mut out: Vec<Computed> = Vec::with_capacity(tree.nodes.len());
        for i in 0..tree.nodes.len() {
            let entries = self.matching(tree, i);
            let c = compute_one(&entries, tree.nodes[i].parent.and_then(|p| out.get(p)));
            out.push(c);
        }
        out
    }
}

fn media_ok(media: MediaContext, m: Media) -> bool {
    match m {
        Media::All => true,
        Media::Print => media.print,
        Media::Dark => media.dark,
        Media::Other => false,
    }
}

/// What a selector requires of the element it styles, as an index key: its id (`#x`), else one
/// of its classes (`.x`), else its tag (`x`, lower case); `None` when it requires none of them.
fn key_of(s: &Selector) -> Option<String> {
    let last = s.compounds.last()?;
    let find = |f: &dyn Fn(&Simple) -> Option<String>| last.simples.iter().find_map(f);
    find(&|x| match x {
        Simple::Id(i) => Some(format!("#{i}")),
        _ => None,
    })
    .or_else(|| {
        find(&|x| match x {
            Simple::Class(c) => Some(format!(".{c}")),
            _ => None,
        })
    })
    .or_else(|| {
        find(&|x| match x {
            Simple::Tag(t) if t != ":root" => Some(t.to_ascii_lowercase()),
            _ => None,
        })
    })
}

/// Replaces the `var(--x, fallback)` of `value` from `vars`. `None` = invalid (variable absent
/// without fallback, or a cycle).
pub fn substitute(value: &str, vars: &BTreeMap<String, String>) -> Option<String> {
    substitute_depth(value, &|n| vars.get(n).cloned(), 0)
}

/// `substitute`, the variables read through `lookup` (`None` = absent).
fn substitute_depth(
    value: &str,
    lookup: &dyn Fn(&str) -> Option<String>,
    depth: usize,
) -> Option<String> {
    if depth > 32 {
        return None;
    }
    let Some(pos) = find_var(value) else {
        return Some(value.to_string());
    };
    let open = pos + 4;
    // matching closing parenthesis
    let mut d = 1;
    let mut close = None;
    for (k, c) in value[open..].char_indices() {
        match c {
            '(' => d += 1,
            ')' => {
                d -= 1;
                if d == 0 {
                    close = Some(open + k);
                    break;
                }
            }
            _ => {}
        }
    }
    let close = close?;
    let inner = &value[open..close];
    let (name, fallback) = match split_top(inner, ',').as_slice() {
        [n] => (n.trim().to_string(), None),
        [n, ..] => (
            n.trim().to_string(),
            Some(inner[inner.find(',').unwrap_or(0) + 1..].trim().to_string()),
        ),
        [] => return None,
    };
    let replacement = match lookup(&name) {
        Some(v) => v,
        None => substitute_depth(&fallback?, lookup, depth + 1)?,
    };
    let rest = substitute_depth(&value[close + 1..], lookup, depth + 1)?;
    Some(format!("{}{}{}", &value[..pos], replacement, rest))
}

fn find_var(s: &str) -> Option<usize> {
    let lower = s.to_ascii_lowercase();
    let mut from = 0;
    while let Some(k) = lower[from..].find("var(") {
        let at = from + k;
        let prev = lower[..at].chars().next_back();
        if !prev.is_some_and(|c| c.is_alphanumeric() || c == '-') {
            return Some(at);
        }
        from = at + 4;
    }
    None
}

fn sides(v: &str) -> [String; 4] {
    let t: Vec<&str> = split_top(v.trim(), ' ')
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect();
    let g = |i: usize| {
        t.get(i)
            .map(std::string::ToString::to_string)
            .unwrap_or_default()
    };
    match t.len() {
        1 => [g(0), g(0), g(0), g(0)],
        2 => [g(0), g(1), g(0), g(1)],
        3 => [g(0), g(1), g(2), g(1)],
        _ => [g(0), g(1), g(2), g(3)],
    }
}

const SIDE_NAMES: [&str; 4] = ["top", "right", "bottom", "left"];
const BORDER_STYLES: [&str; 10] = [
    "none", "hidden", "solid", "dashed", "dotted", "double", "groove", "ridge", "inset", "outset",
];

/// Splits a border value: (width, style, colour).
fn border_parts(v: &str) -> (Option<String>, Option<String>, Option<String>) {
    let (mut w, mut s, mut c) = (None, None, None);
    let mut rest = Vec::new();
    for t in split_top(v.trim(), ' ')
        .into_iter()
        .filter(|t| !t.is_empty())
    {
        let l = t.to_ascii_lowercase();
        if BORDER_STYLES.contains(&l.as_str()) {
            s = Some(l);
        } else if w.is_none()
            && (parse_length(t, 16.0, None).is_some()
                || ["thin", "medium", "thick"].contains(&l.as_str()))
        {
            w = Some(t.to_string());
        } else {
            rest.push(t);
        }
    }
    if !rest.is_empty() {
        c = Some(rest.join(" "));
    }
    (w, s, c)
}

/// Expands a shorthand (after substitution) into longhands.
fn expand(name: &str, v: &str, out: &mut Vec<(String, String)>) {
    let push = |out: &mut Vec<(String, String)>, n: String, v: String| out.push((n, v));
    match name {
        "padding" | "margin" => {
            for (s, val) in SIDE_NAMES.iter().zip(sides(v)) {
                push(out, format!("{name}-{s}"), val);
            }
        }
        "border-width" | "border-style" | "border-color" => {
            let kind = &name[7..];
            for (s, val) in SIDE_NAMES.iter().zip(sides(v)) {
                push(out, format!("border-{s}-{kind}"), val);
            }
        }
        "border" | "border-top" | "border-right" | "border-bottom" | "border-left" => {
            let (w, s, c) = border_parts(v);
            let targets: Vec<&str> = if name == "border" {
                SIDE_NAMES.to_vec()
            } else {
                vec![&name[7..]]
            };
            for side in targets {
                push(
                    out,
                    format!("border-{side}-width"),
                    w.clone().unwrap_or_else(|| "medium".into()),
                );
                push(
                    out,
                    format!("border-{side}-style"),
                    s.clone().unwrap_or_else(|| "none".into()),
                );
                push(
                    out,
                    format!("border-{side}-color"),
                    c.clone().unwrap_or_else(|| "currentColor".into()),
                );
            }
        }
        "font" => {
            // [style] [variant] [weight] size[/line-height] family
            let toks: Vec<&str> = split_top(v.trim(), ' ')
                .into_iter()
                .filter(|t| !t.is_empty())
                .collect();
            let mut style = "normal".to_string();
            let mut weight = "normal".to_string();
            let mut k = 0;
            while k < toks.len() {
                let l = toks[k].to_ascii_lowercase();
                if l == "italic" || l == "oblique" {
                    style = l;
                } else if l == "bold" || l == "bolder" || l == "lighter" || l.parse::<u32>().is_ok()
                {
                    weight = l;
                } else if l == "normal" || l == "small-caps" {
                } else {
                    break;
                }
                k += 1;
            }
            if k >= toks.len() {
                return;
            }
            let (size, lh) = match toks[k].split_once('/') {
                Some((s, l)) => (s.to_string(), Some(l.to_string())),
                None => (toks[k].to_string(), None),
            };
            push(out, "font-style".into(), style);
            push(out, "font-weight".into(), weight);
            push(out, "font-size".into(), size);
            push(
                out,
                "line-height".into(),
                lh.unwrap_or_else(|| "normal".into()),
            );
            push(out, "font-family".into(), toks[k + 1..].join(" "));
        }
        "inset" => {
            for (s, val) in SIDE_NAMES.iter().zip(sides(v)) {
                push(out, s.to_string(), val);
            }
        }
        _ => push(out, name.to_string(), v.to_string()),
    }
}

fn compute_one(entries: &[Entry], parent: Option<&Computed>) -> Computed {
    let mut c = Computed::default();
    // 1. custom properties: inherited, then declared, then var() resolved here
    let mut raw_vars: BTreeMap<String, String> = parent.map(|p| p.vars.clone()).unwrap_or_default();
    let mut declared = Vec::new();
    for e in entries.iter().filter(|e| e.decl.name.starts_with("--")) {
        let v = e.decl.value.trim().to_string();
        if v.eq_ignore_ascii_case("initial") {
            raw_vars.remove(&e.decl.name);
        } else if !v.eq_ignore_ascii_case("inherit") {
            raw_vars.insert(e.decl.name.clone(), v);
            declared.push(e.decl.name.clone());
        }
    }
    // resolve in this element's context (the other variables declared here included), without
    // self-reference; a value without `var()` stays as written
    let resolved: Vec<(&String, Option<String>)> = declared
        .iter()
        .filter_map(|name| {
            let raw = raw_vars.get(name).filter(|raw| find_var(raw).is_some())?;
            let ctx = Resolved::new(&raw_vars, name);
            Some((name, substitute_depth(raw, &|n| ctx.get(n, ROUNDS), 0)))
        })
        .collect();
    let mut vars = raw_vars;
    for (name, v) in resolved {
        match v {
            Some(v) => vars.insert(name.clone(), v),
            None => vars.remove(name),
        };
    }
    c.vars = vars;
    // 2. inheritance of the inherited properties
    if let Some(p) = parent {
        for k in INHERITED {
            if let Some(v) = p.props.get(k) {
                c.props.insert(k.to_string(), v.clone());
            }
        }
    }
    // 3. declarations in cascade order
    for e in entries.iter().filter(|e| !e.decl.name.starts_with("--")) {
        let Some(v) = substitute(&e.decl.value, &c.vars) else {
            continue;
        };
        let mut longs = Vec::new();
        expand(&e.decl.name, &v, &mut longs);
        for (n, v) in longs {
            let l = v.trim().to_ascii_lowercase();
            if l == "inherit" {
                match parent.and_then(|p| p.props.get(&n)) {
                    Some(pv) => c.props.insert(n, pv.clone()),
                    None => c.props.remove(&n),
                };
            } else if l == "initial" || l == "unset" && !INHERITED.contains(&n.as_str()) {
                c.props.remove(&n);
            } else if l == "unset" {
                // inherited property: keep the inherited value
            } else {
                c.props.insert(n, v.trim().to_string());
            }
        }
    }
    // 4. absolute values for what inherits as a computed value
    let parent_fs = parent.map_or(16.0, Computed::font_size);
    let fs = match c.props.get("font-size") {
        Some(v) => parse_length(v, parent_fs, Some(parent_fs)).unwrap_or(parent_fs),
        None => parent_fs,
    };
    c.props.insert("font-size".into(), format!("{fs}px"));
    if let Some(ls) = c.props.get("letter-spacing").cloned()
        && let Some(px) = parse_length(&ls, fs, None)
    {
        c.props.insert("letter-spacing".into(), format!("{px}px"));
    }
    if let Some(lh) = c.props.get("line-height").cloned()
        && (lh.trim().ends_with("em") || lh.trim().ends_with('%'))
        && let Some(px) = parse_length(&lh, fs, Some(fs))
    {
        c.props.insert("line-height".into(), format!("{px}px"));
    }
    // colour: computed (the parent's currentColor)
    let parent_color = parent.map_or(Rgba::BLACK, Computed::color);
    if let Some(col) = c.props.get("color").cloned()
        && let Some(rgba) = parse_color(&col, parent_color)
    {
        c.props.insert("color".into(), rgba.to_css_precise());
    }
    c
}

/// Rounds of resolution of the variables between themselves: a chain of `var()` deeper than this
/// keeps its last `var()` unresolved.
const ROUNDS: u8 = 4;

/// The variables of an element resolved between themselves, without `skip` (the variable being
/// resolved): at round `r`, each value holding a `var()` is substituted with the values of round
/// `r - 1` (without itself), and dropped if that fails. Computed lazily, for the names a value
/// actually reads, and remembered.
struct Resolved<'a> {
    base: &'a BTreeMap<String, String>,
    skip: &'a str,
    memo: std::cell::RefCell<BTreeMap<(String, u8), Option<String>>>,
}

impl<'a> Resolved<'a> {
    fn new(base: &'a BTreeMap<String, String>, skip: &'a str) -> Self {
        Resolved {
            base,
            skip,
            memo: std::cell::RefCell::default(),
        }
    }

    /// The value of `name` after `round` rounds, `None` when absent or dropped.
    fn get(&self, name: &str, round: u8) -> Option<String> {
        if name == self.skip {
            return None;
        }
        if round == 0 {
            return self.base.get(name).cloned();
        }
        let key = (name.to_string(), round);
        if let Some(v) = self.memo.borrow().get(&key) {
            return v.clone();
        }
        let prev = self.get(name, round - 1);
        let v = match prev {
            Some(p) if find_var(&p).is_some() => substitute_depth(
                &p,
                &|n| {
                    if n == name {
                        None
                    } else {
                        self.get(n, round - 1)
                    }
                },
                0,
            ),
            other => other,
        };
        self.memo.borrow_mut().insert(key, v.clone());
        v
    }
}

impl Rgba {
    /// Exact form (not rounded), to pass a computed colour on without loss.
    pub fn to_css_precise(&self) -> String {
        format!("rgba({}, {}, {}, {})", self.r, self.g, self.b, self.a)
    }
}

/// Typed accessors.
impl Computed {
    pub fn get(&self, name: &str) -> Option<&str> {
        if name.starts_with("--") {
            self.vars.get(name).map(String::as_str)
        } else {
            self.props.get(name).map(String::as_str)
        }
    }

    pub fn font_size(&self) -> f64 {
        self.get("font-size")
            .and_then(|v| parse_length(v, 16.0, None))
            .unwrap_or(16.0)
    }

    /// Line height in px; `None` = `normal` (depends on the font).
    pub fn line_height(&self) -> Option<f64> {
        let v = self.get("line-height")?.trim();
        if v.eq_ignore_ascii_case("normal") {
            return None;
        }
        match v.parse::<f64>() {
            Ok(n) => Some(n * self.font_size()),
            Err(_) => parse_length(v, self.font_size(), Some(self.font_size())),
        }
    }

    pub fn font_weight(&self) -> u16 {
        match self
            .get("font-weight")
            .map(|s| s.trim().to_ascii_lowercase())
        {
            Some(w) if w == "bold" => 700,
            Some(w) if w == "normal" => 400,
            Some(w) => w.parse().unwrap_or(400),
            None => 400,
        }
    }

    pub fn italic(&self) -> bool {
        self.get("font-style")
            .is_some_and(|s| s.contains("italic") || s.contains("oblique"))
    }

    /// Font families, in order, without quotes.
    pub fn font_family(&self) -> Vec<String> {
        self.get("font-family")
            .map(|f| {
                split_top(f, ',')
                    .into_iter()
                    .map(|s| s.trim().trim_matches(['"', '\'']).to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn letter_spacing(&self) -> f64 {
        self.get("letter-spacing")
            .and_then(|v| parse_length(v, self.font_size(), None))
            .unwrap_or(0.0)
    }

    pub fn length(&self, name: &str) -> Option<f64> {
        self.get(name)
            .and_then(|v| parse_length(v, self.font_size(), None))
    }

    /// Paddings [top, right, bottom, left].
    pub fn padding(&self) -> [f64; 4] {
        SIDE_NAMES.map(|s| self.length(&format!("padding-{s}")).unwrap_or(0.0))
    }

    pub fn margin(&self) -> [f64; 4] {
        SIDE_NAMES.map(|s| self.length(&format!("margin-{s}")).unwrap_or(0.0))
    }

    /// Largeurs de bordure effectives [haut, droite, bas, gauche] (0 si style `none`).
    pub fn border(&self) -> [f64; 4] {
        SIDE_NAMES.map(|s| {
            let style = self.get(&format!("border-{s}-style")).unwrap_or("none");
            if style == "none" || style == "hidden" {
                return 0.0;
            }
            match self.get(&format!("border-{s}-width")).map(str::trim) {
                Some("thin") => 1.0,
                Some("thick") => 5.0,
                Some("medium") | None => 3.0,
                Some(v) => parse_length(v, self.font_size(), None).unwrap_or(3.0),
            }
        })
    }

    pub fn border_color(&self, side: usize) -> Rgba {
        let s = SIDE_NAMES[side.min(3)];
        self.get(&format!("border-{s}-color"))
            .and_then(|v| parse_color(v, self.color()))
            .unwrap_or_else(|| self.color())
    }

    pub fn color(&self) -> Rgba {
        self.get("color")
            .and_then(|v| parse_color(v, Rgba::BLACK))
            .unwrap_or(Rgba::BLACK)
    }

    /// Colour of any property (`background-color`, `fill`…), `currentColor` resolved.
    pub fn color_of(&self, name: &str) -> Option<Rgba> {
        self.get(name).and_then(|v| parse_color(v, self.color()))
    }

    /// Colour of a variable (`--c`, `--accent-fill`…).
    pub fn var_color(&self, name: &str) -> Option<Rgba> {
        self.vars
            .get(name)
            .and_then(|v| parse_color(v, self.color()))
    }

    /// Text of a variable or of a content property: `"T0"` → `T0`. `None` if absent or `none`.
    pub fn string(&self, name: &str) -> Option<String> {
        let v = self.get(name)?.trim();
        if v.is_empty() || v == "none" || v == "normal" {
            return None;
        }
        Some(unquote(v))
    }

    pub fn min_width(&self) -> f64 {
        self.length("min-width").unwrap_or(0.0)
    }

    pub fn max_width(&self) -> Option<f64> {
        self.length("max-width")
    }

    pub fn display(&self) -> &str {
        self.get("display").unwrap_or("inline")
    }
}

/// Removes a CSS string's quotes and escapes.
pub fn unquote(v: &str) -> String {
    let v = v.trim();
    let inner = if v.len() >= 2
        && ((v.starts_with('"') && v.ends_with('"')) || (v.starts_with('\'') && v.ends_with('\'')))
    {
        &v[1..v.len() - 1]
    } else {
        v
    };
    let mut out = String::new();
    let mut esc = false;
    for ch in inner.chars() {
        if esc {
            out.push(ch);
            esc = false;
        } else if ch == '\\' {
            esc = true;
        } else {
            out.push(ch);
        }
    }
    out
}
