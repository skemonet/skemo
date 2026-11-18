//! Diagnostics for a stylesheet (glitu M7): what the reader would silently ignore, with its place.
//!
//! - `css-syntax`: a `{` never closed, a `}` without `{`, a declaration outside a rule, a
//!   declaration without `:` inside one;
//! - `css-property`: a property name that is a typo of one skemo or CSS knows (`colr`,
//!   `--lable`). Other unknown names are left alone: the HTML output passes the whole stylesheet
//!   to the browser, and an author's own variables (`--brand`) are free.
//!
//! All are warnings: the stylesheet is still used, as the reader reads it.

use crate::diag::{Diagnostic, Span, suggest};
use crate::tr;

/// Custom properties skemo reads or its themes define (FORMAT §4, `lang/theme/`).
const SKEMO: &[&str] = &[
    "--accent",
    "--accent-fill",
    "--accent-ink",
    "--accent-soft",
    "--amber",
    "--bar-w",
    "--bg",
    "--bg-2",
    "--board",
    "--c",
    "--card-border",
    "--code-bg",
    "--curve",
    "--date",
    "--dim-opacity",
    "--dock-gap",
    "--error",
    "--event-anchor",
    "--event-date",
    "--fill-mix",
    "--flow",
    "--flow-speed",
    "--focus-ring",
    "--font",
    "--font-scale",
    "--frise-w",
    "--grid",
    "--grid-major",
    "--icon",
    "--icon-color",
    "--icon-gap",
    "--icon-size",
    "--ink",
    "--ink-2",
    "--label",
    "--line",
    "--line-2",
    "--link-mark",
    "--look",
    "--mono",
    "--muted",
    "--ok",
    "--on-c",
    "--pulse",
    "--pulse-color",
    "--pulse-size",
    "--pulse-speed",
    "--radius",
    "--sample-gap",
    "--sample-icon",
    "--sample-w",
    "--shadow",
    "--shape",
    "--surface",
    "--surface-2",
    "--tl-deps",
    "--tl-gap-x",
    "--tl-gap-y",
    "--transition",
    "--warn",
    "--z-bg",
    "--z-border",
    "--z-ink",
    "--zone-head",
    "--zone-pad",
];

/// Standard CSS properties (the HTML output passes the whole stylesheet to the browser).
const CSS: &[&str] = &[
    "accent-color",
    "align-content",
    "align-items",
    "align-self",
    "all",
    "animation",
    "animation-delay",
    "animation-direction",
    "animation-duration",
    "animation-fill-mode",
    "animation-iteration-count",
    "animation-name",
    "animation-play-state",
    "animation-timing-function",
    "appearance",
    "aspect-ratio",
    "backdrop-filter",
    "backface-visibility",
    "background",
    "background-attachment",
    "background-blend-mode",
    "background-clip",
    "background-color",
    "background-image",
    "background-origin",
    "background-position",
    "background-position-x",
    "background-position-y",
    "background-repeat",
    "background-size",
    "block-size",
    "border",
    "border-block",
    "border-block-color",
    "border-block-end",
    "border-block-end-color",
    "border-block-end-style",
    "border-block-end-width",
    "border-block-start",
    "border-block-start-color",
    "border-block-start-style",
    "border-block-start-width",
    "border-block-style",
    "border-block-width",
    "border-bottom",
    "border-bottom-color",
    "border-bottom-left-radius",
    "border-bottom-right-radius",
    "border-bottom-style",
    "border-bottom-width",
    "border-collapse",
    "border-color",
    "border-end-end-radius",
    "border-end-start-radius",
    "border-image",
    "border-image-outset",
    "border-image-repeat",
    "border-image-slice",
    "border-image-source",
    "border-image-width",
    "border-inline",
    "border-inline-color",
    "border-inline-end",
    "border-inline-end-color",
    "border-inline-end-style",
    "border-inline-end-width",
    "border-inline-start",
    "border-inline-start-color",
    "border-inline-start-style",
    "border-inline-start-width",
    "border-inline-style",
    "border-inline-width",
    "border-left",
    "border-left-color",
    "border-left-style",
    "border-left-width",
    "border-radius",
    "border-right",
    "border-right-color",
    "border-right-style",
    "border-right-width",
    "border-spacing",
    "border-start-end-radius",
    "border-start-start-radius",
    "border-style",
    "border-top",
    "border-top-color",
    "border-top-left-radius",
    "border-top-right-radius",
    "border-top-style",
    "border-top-width",
    "border-width",
    "bottom",
    "box-decoration-break",
    "box-shadow",
    "box-sizing",
    "break-after",
    "break-before",
    "break-inside",
    "caption-side",
    "caret-color",
    "clear",
    "clip",
    "clip-path",
    "clip-rule",
    "color",
    "color-scheme",
    "column-count",
    "column-fill",
    "column-gap",
    "column-rule",
    "column-rule-color",
    "column-rule-style",
    "column-rule-width",
    "column-span",
    "column-width",
    "columns",
    "contain",
    "container",
    "container-name",
    "container-type",
    "content",
    "content-visibility",
    "counter-increment",
    "counter-reset",
    "counter-set",
    "cursor",
    "cx",
    "cy",
    "d",
    "direction",
    "display",
    "dominant-baseline",
    "empty-cells",
    "fill",
    "fill-opacity",
    "fill-rule",
    "filter",
    "flex",
    "flex-basis",
    "flex-direction",
    "flex-flow",
    "flex-grow",
    "flex-shrink",
    "flex-wrap",
    "float",
    "flood-color",
    "flood-opacity",
    "font",
    "font-display",
    "font-family",
    "font-feature-settings",
    "font-kerning",
    "font-optical-sizing",
    "font-size",
    "font-size-adjust",
    "font-stretch",
    "font-style",
    "font-synthesis",
    "font-variant",
    "font-variant-caps",
    "font-variant-east-asian",
    "font-variant-ligatures",
    "font-variant-numeric",
    "font-variation-settings",
    "font-weight",
    "gap",
    "grid",
    "grid-area",
    "grid-auto-columns",
    "grid-auto-flow",
    "grid-auto-rows",
    "grid-column",
    "grid-column-end",
    "grid-column-gap",
    "grid-column-start",
    "grid-gap",
    "grid-row",
    "grid-row-end",
    "grid-row-gap",
    "grid-row-start",
    "grid-template",
    "grid-template-areas",
    "grid-template-columns",
    "grid-template-rows",
    "hanging-punctuation",
    "height",
    "hyphens",
    "image-rendering",
    "inline-size",
    "inset",
    "inset-block",
    "inset-block-end",
    "inset-block-start",
    "inset-inline",
    "inset-inline-end",
    "inset-inline-start",
    "isolation",
    "justify-content",
    "justify-items",
    "justify-self",
    "left",
    "letter-spacing",
    "lighting-color",
    "line-break",
    "line-clamp",
    "line-height",
    "list-style",
    "list-style-image",
    "list-style-position",
    "list-style-type",
    "margin",
    "margin-block",
    "margin-block-end",
    "margin-block-start",
    "margin-bottom",
    "margin-inline",
    "margin-inline-end",
    "margin-inline-start",
    "margin-left",
    "margin-right",
    "margin-top",
    "marker",
    "marker-end",
    "marker-mid",
    "marker-start",
    "mask",
    "mask-clip",
    "mask-composite",
    "mask-image",
    "mask-mode",
    "mask-origin",
    "mask-position",
    "mask-repeat",
    "mask-size",
    "mask-type",
    "max-block-size",
    "max-height",
    "max-inline-size",
    "max-width",
    "min-block-size",
    "min-height",
    "min-inline-size",
    "min-width",
    "mix-blend-mode",
    "object-fit",
    "object-position",
    "offset",
    "offset-distance",
    "offset-path",
    "offset-rotate",
    "opacity",
    "order",
    "orphans",
    "outline",
    "outline-color",
    "outline-offset",
    "outline-style",
    "outline-width",
    "overflow",
    "overflow-anchor",
    "overflow-wrap",
    "overflow-x",
    "overflow-y",
    "overscroll-behavior",
    "overscroll-behavior-x",
    "overscroll-behavior-y",
    "padding",
    "padding-block",
    "padding-block-end",
    "padding-block-start",
    "padding-bottom",
    "padding-inline",
    "padding-inline-end",
    "padding-inline-start",
    "padding-left",
    "padding-right",
    "padding-top",
    "page-break-after",
    "page-break-before",
    "page-break-inside",
    "paint-order",
    "perspective",
    "perspective-origin",
    "place-content",
    "place-items",
    "place-self",
    "pointer-events",
    "position",
    "print-color-adjust",
    "quotes",
    "r",
    "resize",
    "right",
    "rotate",
    "row-gap",
    "rx",
    "ry",
    "scale",
    "scroll-behavior",
    "scroll-margin",
    "scroll-margin-bottom",
    "scroll-margin-left",
    "scroll-margin-right",
    "scroll-margin-top",
    "scroll-padding",
    "scroll-padding-bottom",
    "scroll-padding-left",
    "scroll-padding-right",
    "scroll-padding-top",
    "scroll-snap-align",
    "scroll-snap-type",
    "scrollbar-color",
    "scrollbar-gutter",
    "scrollbar-width",
    "shape-image-threshold",
    "shape-margin",
    "shape-outside",
    "shape-rendering",
    "speak",
    "src",
    "stop-color",
    "stop-opacity",
    "stroke",
    "stroke-dasharray",
    "stroke-dashoffset",
    "stroke-linecap",
    "stroke-linejoin",
    "stroke-miterlimit",
    "stroke-opacity",
    "stroke-width",
    "tab-size",
    "table-layout",
    "text-align",
    "text-align-last",
    "text-anchor",
    "text-combine-upright",
    "text-decoration",
    "text-decoration-color",
    "text-decoration-line",
    "text-decoration-skip-ink",
    "text-decoration-style",
    "text-decoration-thickness",
    "text-emphasis",
    "text-indent",
    "text-justify",
    "text-orientation",
    "text-overflow",
    "text-rendering",
    "text-shadow",
    "text-transform",
    "text-underline-offset",
    "text-underline-position",
    "text-wrap",
    "top",
    "touch-action",
    "transform",
    "transform-box",
    "transform-origin",
    "transform-style",
    "transition",
    "transition-behavior",
    "transition-delay",
    "transition-duration",
    "transition-property",
    "transition-timing-function",
    "translate",
    "unicode-bidi",
    "unicode-range",
    "user-select",
    "vector-effect",
    "vertical-align",
    "view-transition-name",
    "visibility",
    "white-space",
    "white-space-collapse",
    "widows",
    "width",
    "will-change",
    "word-break",
    "word-spacing",
    "word-wrap",
    "writing-mode",
    "x",
    "y",
    "z-index",
    "zoom",
];

/// The diagnostics of stylesheet `src` (positions in `src`). `others`: the diagram's other
/// stylesheets, whose `var(--x)` make `--x` an author's variable here too.
pub fn check(src: &str, others: &[&str]) -> Vec<Diagnostic> {
    let clean = blank_comments(src);
    let mut out = Vec::new();
    let mut vars = used_vars(&clean);
    for o in others {
        vars.extend(used_vars(&blank_comments(o)));
    }
    block(&clean, 0, clean.len(), &vars, &mut out);
    out
}

/// `src` with every comment replaced by spaces (newlines kept): positions do not move.
fn blank_comments(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let mut rest = src;
    while let Some(i) = rest.find("/*") {
        out.push_str(&rest[..i]);
        let end = rest[i + 2..]
            .find("*/")
            .map_or(rest.len(), |e| i + 2 + e + 2);
        for c in rest[i..end].chars() {
            if c == '\n' {
                out.push('\n');
            } else {
                out.extend(std::iter::repeat_n(' ', c.len_utf8()));
            }
        }
        rest = &rest[end..];
    }
    out.push_str(rest);
    out
}

/// Custom properties the stylesheet reads with `var(--x)`: an author's own variables.
fn used_vars(src: &str) -> Vec<String> {
    src.match_indices("var(")
        .filter_map(|(i, _)| {
            let r = src[i + 4..].trim_start();
            let end = r.find([',', ')', ' ']).unwrap_or(r.len());
            r[..end].starts_with("--").then(|| r[..end].to_string())
        })
        .collect()
}

fn warn(out: &mut Vec<Diagnostic>, code: &'static str, at: (usize, usize), m: String) {
    out.push(Diagnostic::warning(code, Span::new(at.0, at.1), m));
}

/// Rules and `@` blocks of `src[from..to]` (the whole sheet, or an `@media` block).
fn block(src: &str, from: usize, to: usize, vars: &[String], out: &mut Vec<Diagnostic>) {
    let mut i = from;
    while i < to {
        let Some(rel) = src[i..to].find(['{', ';', '}']) else {
            stray(src, (i, to), out);
            return;
        };
        let at = i + rel;
        let prelude = src[i..at].trim();
        match src.as_bytes()[at] {
            b';' => {
                if !prelude.starts_with('@') {
                    stray(src, (i, at), out);
                }
                i = at + 1;
            }
            b'}' => {
                warn(
                    out,
                    "css-syntax",
                    (at, at + 1),
                    tr!("`}}` without `{{`: ignored", "« }} » sans « {{ » : ignorée"),
                );
                i = at + 1;
            }
            _ => {
                let close = close_of(src, at, to);
                if close == to {
                    warn(
                        out,
                        "css-syntax",
                        (at, at + 1),
                        tr!(
                            "`{{` never closed: the rest of the stylesheet is ignored",
                            "« {{ » jamais fermée : la suite de la feuille est ignorée"
                        ),
                    );
                }
                if prelude.to_ascii_lowercase().starts_with("@media") {
                    block(src, at + 1, close, vars, out);
                } else if !prelude.starts_with('@') {
                    declarations(src, (at + 1, close), vars, out);
                }
                i = close + 1;
            }
        }
    }
}

/// Text outside any rule: a declaration there is ignored.
fn stray(src: &str, (from, to): (usize, usize), out: &mut Vec<Diagnostic>) {
    let text = src[from..to].trim();
    if text.is_empty() {
        return;
    }
    let start = from + (src[from..to].len() - src[from..to].trim_start().len());
    warn(
        out,
        "css-syntax",
        (start, start + text.len()),
        tr!(
            "`{text}` is outside any rule: ignored",
            "« {text} » est hors de toute règle : ignoré"
        ),
    );
}

/// The `}` that closes the `{` at `open` (strings skipped), or `to` when there is none.
fn close_of(src: &str, open: usize, to: usize) -> usize {
    let b = src.as_bytes();
    let (mut depth, mut i, mut quote) = (0, open, None);
    while i < to {
        match (quote, b[i]) {
            (Some(_), b'\\') => i += 1,
            (Some(q), c) if c == q => quote = None,
            (None, q @ (b'"' | b'\'')) => quote = Some(q),
            (None, b'{') => depth += 1,
            (None, b'}') => {
                depth -= 1;
                if depth == 0 {
                    return i;
                }
            }
            _ => {}
        }
        i += 1;
    }
    to
}

/// The declarations of a rule's body `src[from..to]`.
fn declarations(src: &str, (from, to): (usize, usize), vars: &[String], out: &mut Vec<Diagnostic>) {
    let body = &src[from..to];
    if body.contains('{') {
        return; // nested rules: not read by skemo, left to the browser
    }
    let mut start = from;
    for part in crate::style::css::split_top(body, ';') {
        let lead = part.len() - part.trim_start().len();
        let (s, text) = (start + lead, part.trim());
        start += part.len() + 1;
        if text.is_empty() {
            continue;
        }
        let Some((name, _)) = text.split_once(':') else {
            warn(
                out,
                "css-syntax",
                (s, s + text.len()),
                tr!(
                    "`{text}`: `name: value` expected; ignored",
                    "« {text} » : « nom: valeur » attendu ; ignoré"
                ),
            );
            continue;
        };
        property(name.trim(), s, vars, out);
    }
}

/// A property name that is a typo of a known one.
fn property(name: &str, at: usize, vars: &[String], out: &mut Vec<Diagnostic>) {
    let lower = name.to_ascii_lowercase();
    let custom = name.starts_with("--");
    if lower.starts_with('-') && !custom {
        return; // vendor prefix
    }
    let known: &[&str] = if custom { SKEMO } else { CSS };
    if known.contains(&lower.as_str()) || vars.iter().any(|v| v == name) {
        return;
    }
    let near = if custom {
        near_custom(name)
    } else {
        suggest(&lower, known.iter().copied())
    };
    if let Some(s) = near {
        warn(
            out,
            "css-property",
            (at, at + name.len()),
            tr!(
                "unknown property `{name}`: did you mean `{s}`?",
                "propriété « {name} » inconnue : vouliez-vous « {s} » ?"
            ),
        );
    }
}

/// A skemo property one slip away (a letter wrong, missing, extra, or two swapped) from `name`,
/// for names of 5 letters or more: shorter ones are too often an author's own variable.
fn near_custom(name: &str) -> Option<&'static str> {
    let n: Vec<char> = name.trim_start_matches('-').chars().collect();
    if n.len() < 5 {
        return None;
    }
    SKEMO.iter().copied().find(|k| {
        let k: Vec<char> = k.trim_start_matches('-').chars().collect();
        one_slip(&n, &k)
    })
}

fn one_slip(a: &[char], b: &[char]) -> bool {
    let p = a.iter().zip(b).take_while(|(x, y)| x == y).count();
    let (a, b) = (&a[p..], &b[p..]);
    match (a.len(), b.len()) {
        (0, 0) => false,
        (x, y) if x == y => {
            a[1..] == b[1..] || (x >= 2 && a[0] == b[1] && a[1] == b[0] && a[2..] == b[2..])
        }
        (x, y) if x == y + 1 => a[1..] == *b,
        (x, y) if y == x + 1 => *a == b[1..],
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn codes(src: &str) -> Vec<(&'static str, String)> {
        check(src, &[])
            .into_iter()
            .map(|d| (d.code, src[d.span.start..d.span.end].to_string()))
            .collect()
    }

    #[test]
    fn a_clean_sheet_has_nothing_to_say() {
        let src = "/* theme */\n.web { --c: #2F7FD8; --brand: red; color: var(--brand); }\n\
                   @media (prefers-color-scheme: dark) { .web { fill: #000 } }\n\
                   @font-face { font-family: X; src: url(x.woff2); }\n\
                   .a:hover { cursor: pointer; -webkit-user-select: none; }\n";
        assert_eq!(codes(src), vec![]);
    }

    #[test]
    fn typos_of_known_properties_are_reported() {
        let c = codes(
            ".a { colr: red; --lable: \"X\"; font-wieght: 700; --mine: 1; --lines: 2; --brand: 3; }",
        );
        assert_eq!(
            c,
            vec![
                ("css-property", "colr".into()),
                ("css-property", "--lable".into()),
                ("css-property", "font-wieght".into()),
                ("css-property", "--lines".into()),
            ]
        );
    }

    #[test]
    fn broken_structure_is_reported_where_it_is() {
        let c = codes("color: red;\n.a { color red; }\n}\n.b { fill: blue;\n");
        assert_eq!(
            c,
            vec![
                ("css-syntax", "color: red".into()),
                ("css-syntax", "color red".into()),
                ("css-syntax", "}".into()),
                ("css-syntax", "{".into()),
            ]
        );
    }

    #[test]
    fn a_variable_another_sheet_reads_is_an_authors() {
        assert_eq!(
            check(".a { --lines: 2 }", &[".b { stroke: var(--lines) }"]),
            vec![]
        );
        assert_eq!(check(".a { --lines: 2 }", &[]).len(), 1);
    }

    #[test]
    fn comments_keep_positions() {
        let src = "/* é */ .a { colr: red }";
        let d = check(src, &[]);
        assert_eq!(&src[d[0].span.start..d[0].span.end], "colr");
    }
}
