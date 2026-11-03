//! `skemo check`: diagnostics of the files, and contrast of the texts in the three themes.

use crate::args::{CheckArgs, ThemeArg};
use crate::common::{Fail, Inputs, prepare, read_source};
use crate::out::Out;
use skemo_lang::tr;

pub fn run(out: &Out, a: &CheckArgs) -> Result<(), Fail> {
    let mut failed = false;
    for f in &a.files {
        let src = match read_source(f) {
            Ok(s) => s,
            Err(e) => {
                out.error(&e);
                failed = true;
                continue;
            }
        };
        let doc = skemo_lang::parse(&src);
        for d in &doc.diagnostics {
            out.diag(&src, f, d);
        }
        let unknown = unknown_icons(out, &src, f, &doc);
        failed |= (a.deny_warnings && unknown)
            || doc.has_errors()
            || (a.deny_warnings && doc.warnings().next().is_some());
        if !doc.has_errors() {
            failed |= layout_warnings(out, a, f) && a.deny_warnings;
        }
        if a.contrast && !doc.has_errors() {
            failed |= report_contrast(out, a, f);
        }
    }
    if failed { Err(Fail::source()) } else { Ok(()) }
}

/// Warnings of the placement (overlaps, `wrap`, `fit`), which the analysis alone does not see;
/// true if there are any.
fn layout_warnings(out: &Out, a: &CheckArgs, file: &str) -> bool {
    let inputs = Inputs::new(file, &a.css, a.base_theme.as_ref());
    let Ok((d, _, _)) = prepare(&out.silenced(), inputs) else {
        return false;
    };
    for w in &d.warnings {
        out.warn(w);
    }
    !d.warnings.is_empty()
}

/// Warns about `icon="…"` outside the built-in set (the engine ignores them when rendering); true
/// if there are any.
fn unknown_icons(out: &Out, src: &str, file: &str, doc: &skemo_lang::Document) -> bool {
    let mut found = false;
    for n in &doc.model.nodes {
        let Some(name) = n.icon.as_deref().filter(|i| *i != "none") else {
            continue;
        };
        if skemo_engine::icons::get(name).is_some() {
            continue;
        }
        let span = doc
            .map
            .nodes
            .get(&n.id)
            .and_then(|l| l.attrs.iter().find(|a| a.name == "icon"))
            .map_or(skemo_lang::diag::Span::default(), |a| a.span);
        let mut d = skemo_lang::Diagnostic::warning(
            "unknown-icon",
            span,
            tr!("unknown icon « {name} »", "icône « {name} » inconnue"),
        );
        if let Some(s) = skemo_engine::icons::suggest(name) {
            d = d.with_help(tr!("did you mean « {s} »?", "vouliez-vous « {s} » ?"));
        }
        out.diag(src, file, &d);
        found = true;
    }
    found
}

/// Contrast defects of the file in the three themes; true if there are any.
fn report_contrast(out: &Out, a: &CheckArgs, file: &str) -> bool {
    let quiet = out.silenced();
    let mut found = false;
    for th in [ThemeArg::Light, ThemeArg::Dark, ThemeArg::Print] {
        let inputs = Inputs::new(file, &a.css, a.base_theme.as_ref()).theme(Some(th));
        let Ok((d, _, _)) = prepare(&quiet, inputs) else {
            continue;
        };
        for i in skemo_engine::contrast::check(&d) {
            found = true;
            out.error(&format!(
                "{file} [{}] {}",
                d.theme.name(),
                tr!(
                    "insufficient contrast: {} ({}, classes {}) {} < {} — text {} on {}",
                    "contraste insuffisant : {} ({}, classes {}) {} < {} — texte {} sur {}",
                    i.element,
                    skemo_engine::contrast::describe(i.what),
                    classes_label(&i.classes),
                    i.ratio,
                    i.required,
                    i.text,
                    i.background
                )
            ));
        }
    }
    found
}

fn classes_label(classes: &[String]) -> String {
    if classes.is_empty() {
        tr!("none", "aucune")
    } else {
        classes.join(" ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classes_are_listed_or_named_none() {
        assert_eq!(classes_label(&[]), "none");
        assert_eq!(classes_label(&["a".into(), "b".into()]), "a b");
    }
}
