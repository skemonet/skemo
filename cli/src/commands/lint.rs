//! `skemo lint`: diagnostics of the files, contrast of the texts in the three themes, and with
//! `--fix` the files rewritten in canonical form (`--dry`: only reported).

use crate::args::{LintArgs, ThemeArg};
use crate::common::{Fail, Inputs, prepare, read_source, write_out};
use crate::out::Out;
use skemo_lang::tr;

pub fn run(out: &Out, a: &LintArgs) -> Result<(), Fail> {
    let mut failed = false;
    let mut errors = false;
    for f in &a.files {
        let src = match read_source(f) {
            Ok(s) => s,
            Err(e) => {
                out.error(&e);
                failed = true;
                continue;
            }
        };
        if a.canon.fix {
            failed |= fix(out, a, f, &src)?;
        }
        let doc = skemo_lang::parse(&src);
        for d in &doc.diagnostics {
            out.diag(&src, f, d);
        }
        let unknown = unknown_icons(out, &src, f, &doc);
        errors |= doc.has_errors();
        failed |= (a.deny_warnings && unknown)
            || doc.has_errors()
            || (a.deny_warnings && doc.warnings().next().is_some());
        if !doc.has_errors() {
            failed |= layout_warnings(out, a, f) && a.deny_warnings;
        }
        if a.contrast && !doc.has_errors() {
            failed |= report_contrast(out, a, f);
        }
        if let Some(target) = a.fit.as_deref().filter(|_| !doc.has_errors()) {
            super::fit::run(out, a, f, target)?;
        }
    }
    if !errors {
        let n = a.files.len();
        out.note(&if n == 1 {
            tr!(
                "1 diagram checked: no error",
                "1 schéma vérifié : aucune erreur"
            )
        } else {
            tr!(
                "{n} diagrams checked: no error",
                "{n} schémas vérifiés : aucune erreur"
            )
        });
    }
    if failed { Err(Fail::source()) } else { Ok(()) }
}

/// Rewrites `f` in canonical form (`-`: on the standard output), or with `--dry` reports that it
/// is not; true if that failed the file. A file that does not parse is left to the diagnostics.
fn fix(out: &Out, a: &LintArgs, f: &str, src: &str) -> Result<bool, Fail> {
    match skemo_lang::fmt::format(src) {
        Ok(formatted) if formatted == src => Ok(false),
        Ok(_) if a.canon.dry => {
            out.warn(&tr!(
                "{f} is not in canonical form (skemo lint --fix)",
                "{f} n'est pas en forme canonique (skemo lint --fix)"
            ));
            Ok(true)
        }
        Ok(formatted) => write_out(f, formatted.as_bytes()).map(|()| false),
        Err(_) => Ok(false),
    }
}

/// Warnings of the placement (overlaps, `wrap`, `fit`), which the analysis alone does not see;
/// true if there are any.
fn layout_warnings(out: &Out, a: &LintArgs, file: &str) -> bool {
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
fn report_contrast(out: &Out, a: &LintArgs, file: &str) -> bool {
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
