//! Diagnostics: position, message (in the chosen language, see `i18n`), suggestion; rendered as
//! text in the manner of `ariadne`.

use crate::tr;
use serde::Serialize;
use std::fmt::Write as _;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
}

/// Range of bytes in the source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Span {
            start,
            end: end.max(start),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Diagnostic {
    pub severity: Severity,
    /// Stable code (`unknown-target`, `duplicate-id`…), for the tests and the editor.
    pub code: &'static str,
    pub message: String,
    pub span: Span,
    pub help: Option<String>,
}

impl Diagnostic {
    pub fn error(code: &'static str, span: Span, message: impl Into<String>) -> Self {
        Diagnostic {
            severity: Severity::Error,
            code,
            message: message.into(),
            span,
            help: None,
        }
    }
    pub fn warning(code: &'static str, span: Span, message: impl Into<String>) -> Self {
        Diagnostic {
            severity: Severity::Warning,
            code,
            message: message.into(),
            span,
            help: None,
        }
    }
    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }
    pub fn is_error(&self) -> bool {
        self.severity == Severity::Error
    }
}

/// Line and column (from 1, column in characters) of a byte of the source.
pub fn line_col(src: &str, offset: usize) -> (usize, usize) {
    let offset = floor_char(src, offset.min(src.len()));
    let before = &src[..offset];
    let line = before.matches('\n').count() + 1;
    let line_start = before.rfind('\n').map_or(0, |i| i + 1);
    let col = src[line_start..offset].chars().count() + 1;
    (line, col)
}

fn floor_char(s: &str, mut i: usize) -> usize {
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

/// Rendu texte d'un diagnostic :
///
/// ```text
/// error[unknown-target]: unknown target « cor »
///   ┌─ reseau.skemo:7:8
///   │
/// 7 │     to(cor).wan transit
///   │        ^^^
///   = help: did you mean « core »?
/// ```
pub fn render(src: &str, file: &str, d: &Diagnostic) -> String {
    let mut out = String::new();
    let kind = match d.severity {
        Severity::Error => tr!("error", "erreur"),
        Severity::Warning => tr!("warning", "avertissement"),
    };
    let (line, col) = line_col(src, d.span.start);
    let _ = writeln!(out, "{kind}[{}]{} {}", d.code, tr!(":", " :"), d.message);
    let num = line.to_string();
    let pad = " ".repeat(num.len());
    let _ = writeln!(out, "{pad} ┌─ {file}:{line}:{col}");
    let _ = writeln!(out, "{pad} │");
    let text = src.lines().nth(line - 1).unwrap_or("");
    let _ = writeln!(out, "{num} │ {text}");
    let start = floor_char(src, d.span.start.min(src.len()));
    let line_start = src[..start].rfind('\n').map_or(0, |i| i + 1);
    let line_end = line_start + text.len();
    let end = floor_char(src, d.span.end.min(line_end).max(start));
    let width = src[start..end].chars().count().max(1);
    let lead: String = src[line_start..start]
        .chars()
        .map(|c| if c == '\t' { '\t' } else { ' ' })
        .collect();
    let _ = writeln!(out, "{pad} │ {lead}{}", "^".repeat(width));
    if let Some(h) = &d.help {
        let _ = writeln!(out, "{pad} = {} {h}", tr!("help:", "aide :"));
    }
    out
}

/// Edit distance (Levenshtein), for the suggestions.
pub fn distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut cur = vec![i + 1; b.len() + 1];
        for (j, cb) in b.iter().enumerate() {
            cur[j + 1] = (prev[j] + usize::from(ca != *cb))
                .min(prev[j + 1] + 1)
                .min(cur[j] + 1);
        }
        prev = cur;
    }
    prev[b.len()]
}

/// The nearest candidate, if it is near enough to be a typo.
pub fn suggest<'a>(word: &str, candidates: impl IntoIterator<Item = &'a str>) -> Option<&'a str> {
    let max = (word.chars().count() / 3).max(1);
    candidates
        .into_iter()
        .map(|c| (distance(word, c), c))
        .filter(|(d, _)| *d <= max)
        .min_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(b.1)))
        .map(|(_, c)| c)
}
