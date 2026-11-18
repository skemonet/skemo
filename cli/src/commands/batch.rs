//! `skemo render --batch`: many diagrams in one process (`cli/SPEC.md` §2.5). Requests on the
//! standard input, one JSON object per line; one answer per request, in the same order, on the
//! standard output. A refused diagram answers `"svg": null` and the process goes on.
//!
//! ```text
//! {"id":"slide4-1","source":"diagram\n  card#a A\n","css":["themes/crystal/skemo.css"],"font_scale":1.35}
//! {"id":"slide4-1","svg":"<svg …>","diagnostics":[]}
//! ```
//!
//! The options of the command line (`--transparent`, `--theme`, `--css`, `--root`…) apply to every
//! request; a request's `css`, `font_scale`, `theme`, `transparent`, `no_source` replace them.

use crate::args::{RenderArgs, ThemeArg};
use crate::common::{Fail, Inputs, prepare, with_source};
use crate::out::{Out, Place, collect};
use serde_json::{Value, json};
use skemo_lang::tr;
use std::io::{BufRead, Write};
use std::path::PathBuf;

/// The options a request may give.
#[derive(Default, Debug)]
struct Request {
    id: Value,
    source: Option<String>,
    css: Option<Vec<PathBuf>>,
    font_scale: Option<f32>,
    theme: Option<ThemeArg>,
    transparent: Option<bool>,
    no_source: Option<bool>,
}

pub fn run(_out: &Out, a: &RenderArgs) -> Result<(), Fail> {
    if !a.file.is_empty() {
        return Err(Fail::usage(tr!(
            "--batch reads its requests on the standard input: give no diagram file",
            "--batch lit ses demandes sur l'entrée standard : ne pas donner de fichier"
        )));
    }
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout().lock();
    for line in stdin.lock().lines() {
        let line = line.map_err(|e| Fail::internal(crate::common::io(&e)))?;
        if line.trim().is_empty() {
            continue;
        }
        let answer = answer(a, &line);
        writeln!(stdout, "{answer}")
            .and_then(|()| stdout.flush())
            .map_err(|e| Fail::internal(tr!("writing: {e}", "écriture : {e}")))?;
    }
    Ok(())
}

/// The answer to one line: the SVG, or `null`, and the diagnostics.
fn answer(a: &RenderArgs, line: &str) -> Value {
    let out = Out {
        json: true,
        ..Out::default()
    };
    let ((id, svg), mut diagnostics) = collect(|| {
        let r = match parse(line) {
            Ok(r) => r,
            Err(m) => {
                out.error_at("bad-request", &m, Place::file("-"));
                return (r_id(line), None);
            }
        };
        match render(&out, a, &r) {
            Ok(s) => (r.id, Some(s)),
            Err(Fail(code, msg)) => {
                if let Some(m) = msg {
                    out.error_at(crate::code_of(code), &m, Place::file("-"));
                }
                (r.id, None)
            }
        }
    });
    let mut svg = svg;
    if a.deny_warnings && svg.is_some() && !diagnostics.is_empty() {
        diagnostics.push(json!({
            "severity": "error", "code": "deny-warnings", "file": "-",
            "message": tr!("warnings with --deny-warnings", "avertissements avec --deny-warnings"),
        }));
        svg = None;
    }
    json!({ "id": id, "svg": svg, "diagnostics": diagnostics })
}

/// The `id` of a request, even an invalid one, so that the answer can be matched.
fn r_id(line: &str) -> Value {
    serde_json::from_str::<Value>(line)
        .ok()
        .and_then(|v| v.get("id").cloned())
        .unwrap_or(Value::Null)
}

fn parse(line: &str) -> Result<Request, String> {
    let v: Value = serde_json::from_str(line).map_err(|e| {
        tr!(
            "a request is one JSON object per line: {e}",
            "une demande est un objet JSON par ligne : {e}"
        )
    })?;
    let o = v.as_object().ok_or_else(|| {
        tr!(
            "a request is a JSON object",
            "une demande est un objet JSON"
        )
    })?;
    let bad = |k: &str| {
        tr!(
            "request: `{k}` has the wrong type",
            "demande : « {k} » n'a pas le bon type"
        )
    };
    let mut r = Request {
        id: o.get("id").cloned().unwrap_or(Value::Null),
        ..Request::default()
    };
    for (k, val) in o {
        match k.as_str() {
            "id" => {}
            "source" => r.source = Some(val.as_str().ok_or_else(|| bad(k))?.to_string()),
            "css" => {
                let list = val.as_array().ok_or_else(|| bad(k))?;
                let paths = list
                    .iter()
                    .map(|p| p.as_str().map(PathBuf::from).ok_or_else(|| bad(k)))
                    .collect::<Result<Vec<_>, _>>()?;
                r.css = Some(paths);
            }
            #[allow(clippy::cast_possible_truncation)]
            "font_scale" => r.font_scale = Some(val.as_f64().ok_or_else(|| bad(k))? as f32),
            "theme" => {
                let t = val.as_str().ok_or_else(|| bad(k))?;
                r.theme =
                    Some(<ThemeArg as clap::ValueEnum>::from_str(t, false).map_err(|_| bad(k))?);
            }
            "transparent" => r.transparent = Some(val.as_bool().ok_or_else(|| bad(k))?),
            "no_source" => r.no_source = Some(val.as_bool().ok_or_else(|| bad(k))?),
            _ => {
                return Err(tr!(
                    "request: unknown key `{k}` (id, source, css, font_scale, theme, transparent, no_source)",
                    "demande : clé « {k} » inconnue (id, source, css, font_scale, theme, transparent, no_source)"
                ));
            }
        }
    }
    if r.source.is_none() {
        return Err(tr!(
            "request: `source` is missing",
            "demande : « source » manque"
        ));
    }
    Ok(r)
}

/// The SVG of one request, as `skemo render - -o - --format svg` writes it.
fn render(out: &Out, a: &RenderArgs, r: &Request) -> Result<String, Fail> {
    let css = r.css.as_ref().unwrap_or(&a.css);
    let inputs = Inputs::new("-", css, a.base_theme.as_ref())
        .text(r.source.as_deref())
        .theme(r.theme.or(a.theme))
        .high_contrast(a.contrast == crate::args::ContrastArg::High)
        .transparent(r.transparent.unwrap_or(a.transparent))
        .font_scale(r.font_scale.or(a.font_scale))
        .fonts(&a.fonts.font)
        .root(a.tool.root.as_deref());
    let (d, _, bundle) = prepare(out, inputs)?;
    let src = (!r.no_source.unwrap_or(a.no_source)).then_some(&bundle);
    let svg = skemo_engine::svg::render_with(&d, a.fonts.embed_fonts, a.svg.motion);
    String::from_utf8(with_source(svg.into_bytes(), src))
        .map_err(|_| Fail::internal("SVG is not UTF-8"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_are_checked_key_by_key() {
        let r = parse(
            r#"{"id":7,"source":"diagram\n","css":["a.css"],"font_scale":1.5,"theme":"dark"}"#,
        )
        .unwrap();
        assert_eq!(r.id, json!(7));
        assert_eq!(r.css.unwrap(), [PathBuf::from("a.css")]);
        assert_eq!(r.font_scale, Some(1.5));
        assert_eq!(r.theme, Some(ThemeArg::Dark));
        assert!(parse(r#"{"id":1}"#).unwrap_err().contains("source"));
        assert!(
            parse(r#"{"source":"x","colour":1}"#)
                .unwrap_err()
                .contains("colour")
        );
        assert!(parse(r#"{"source":3}"#).is_err());
        assert!(parse("[1]").is_err());
        assert!(parse("not json").is_err());
        assert_eq!(r_id(r#"{"id":"s1","source":3}"#), json!("s1"));
        assert_eq!(r_id("nope"), Value::Null);
    }
}
