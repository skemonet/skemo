//! Canonical formatting (F-PARSE-8): indentation of 2 spaces, `#id` then classes then attributes
//! (for `to`: attributes then classes, as in `FORMAT.md`), targets first, then
//! `at size step fx side via`, then the other attributes in writing order; values in double
//! quotes; the old `step=`/`fx=` notation turned into an `@` marker (F-PARSE-13).
//! v0.8 (FORMAT §7): `li` lines become `|` lines (a plain `li` under `title` was a pill:
//! `|.pill`), `from`/`to` on `diagram` become `start`/`end`, `layout="frise"` becomes `frieze`.
//! Comments kept. Idempotent.

use crate::diag::Diagnostic;
use crate::parse::parse;
use crate::syntax::{Attr, Tag, parse_line};

const ORDER: [&str; 10] = [
    "at",
    "size",
    "icon",
    "href",
    "step",
    "fx",
    "side",
    "via",
    "join",
    "constraint",
];
const RESERVED: [&str; 18] = [
    "id",
    "class",
    "at",
    "size",
    "via",
    "step",
    "fx",
    "label",
    "from",
    "side",
    "tag",
    "date",
    "join",
    "start",
    "end",
    "icon",
    "href",
    "constraint",
];

fn quote(v: &str) -> String {
    if v.contains('"') && !v.contains('\'') {
        format!("'{v}'")
    } else {
        format!("\"{}\"", v.replace('"', "\\\""))
    }
}

/// The v0.8 spelling of a `diagram` attribute (FORMAT §7).
fn v08_name(a: &Attr) -> Attr {
    let mut a = a.clone();
    match a.name.as_str() {
        "from" => a.name = "start".into(),
        "to" => a.name = "end".into(),
        "layout" if a.value.as_deref() == Some("frise") => a.value = Some("frieze".into()),
        _ => {}
    }
    a
}

/// A line `|…`: the bar, then classes glued to it or a space and the text; rows keep their
/// inner spacing (a table aligned by hand stays aligned).
fn bar_line(rest: &str) -> String {
    let t = rest.trim_end();
    if t.trim().is_empty() {
        "|".to_string()
    } else if t.starts_with([' ', '\t']) {
        format!("| {}", t.trim_start())
    } else {
        format!("|{t}")
    }
}

/// An old `li` line as a `|` line: `li.c text @2` → `|.c text @2`; a plain `li` under `title`
/// was a pill.
fn li_line(t: &Tag, in_title: bool) -> String {
    let mut t = t.clone();
    t.attrs.retain(|a| a.name == "step" || a.name == "fx");
    if in_title && t.classes.is_empty() {
        t.classes
            .push(("pill".into(), crate::diag::Span::default()));
    }
    let line = tag_line(&t);
    let rest = line.strip_prefix("li").unwrap_or(&line);
    bar_line(rest)
}

fn attr_text(a: &Attr) -> String {
    match &a.value {
        Some(v) => format!("{}={}", a.name, quote(v)),
        None => a.name.clone(),
    }
}

/// Canonical tag line.
fn tag_line(t: &Tag) -> String {
    let mut out = String::new();
    if t.name != "div" {
        out.push_str(&t.name);
    }
    if let Some((id, _)) = &t.id {
        out.push('#');
        out.push_str(id);
    }
    let classes: String = t.classes.iter().map(|(c, _)| format!(".{c}")).collect();
    // ancienne notation → marqueur
    let step = t
        .attrs
        .iter()
        .find(|a| a.name == "step")
        .and_then(|a| a.value.clone());
    let fx = t
        .attrs
        .iter()
        .find(|a| a.name == "fx")
        .and_then(|a| a.value.clone());
    let is_diagram = t.name == "diagram";
    let mut attrs: Vec<&Attr> = Vec::new();
    let renamed: Vec<Attr> = t.attrs.iter().map(v08_name).collect();
    if is_diagram {
        attrs.extend(renamed.iter());
    } else {
        attrs.extend(
            t.attrs
                .iter()
                .filter(|a| a.value.is_none() && !RESERVED.contains(&a.name.as_str())),
        );
        for k in ORDER {
            if k == "step" || k == "fx" {
                continue;
            }
            attrs.extend(t.attrs.iter().filter(|a| a.name == k));
        }
        attrs.extend(t.attrs.iter().filter(|a| {
            !ORDER.contains(&a.name.as_str())
                && (a.value.is_some() || RESERVED.contains(&a.name.as_str()))
        }));
    }
    let group = if attrs.is_empty() {
        String::new()
    } else {
        format!(
            "({})",
            attrs
                .iter()
                .map(|a| attr_text(a))
                .collect::<Vec<_>>()
                .join(" ")
        )
    };
    if t.name == "to" {
        out.push_str(&group);
        out.push_str(&classes);
    } else {
        out.push_str(&classes);
        out.push_str(&group);
    }
    let mut text = t
        .text
        .as_ref()
        .map(|(s, _)| s.trim().to_string())
        .unwrap_or_default();
    if !is_diagram && (step.is_some() || fx.is_some()) {
        let mut m = format!("@{}", step.unwrap_or_else(|| "0".into()));
        if let Some(f) = fx {
            m.push(':');
            m.push_str(&f);
        }
        if !text.is_empty() {
            text.push(' ');
        }
        text.push_str(&m);
    }
    if !text.is_empty() {
        out.push(' ');
        out.push_str(&text);
    }
    out
}

/// Formats `src`. Refuses a file that holds errors (returns the diagnostics).
pub fn format(src: &str) -> Result<String, Vec<Diagnostic>> {
    let doc = parse(src);
    if doc.has_errors() {
        return Err(doc.errors().cloned().collect());
    }
    let mut out = String::new();
    let mut stack: Vec<(usize, String)> = Vec::new(); // original indentation and names of the open tags
    let mut comment: Option<(usize, usize)> = None; // (indentation d'origine, niveau) d'un bloc de commentaire
    let mut blank_pending = false;
    for raw in src.lines() {
        let line = raw.trim_end();
        let body = line.trim_start_matches([' ', '\t']);
        if body.is_empty() {
            blank_pending = !out.is_empty();
            continue;
        }
        let indent = line.len() - body.len();
        if let Some((ci, cl)) = comment {
            if indent > ci {
                // content of a comment block: kept as is, under the comment
                if blank_pending {
                    out.push('\n');
                    blank_pending = false;
                }
                out.push_str(&"  ".repeat(cl + 1));
                out.push_str(body);
                out.push('\n');
                continue;
            }
            comment = None;
        }
        while stack.last().is_some_and(|(i, _)| *i >= indent) {
            stack.pop();
        }
        let in_title = stack.last().is_some_and(|(_, n)| n == "title");
        let level = stack.len();
        if blank_pending {
            out.push('\n');
            blank_pending = false;
        }
        out.push_str(&"  ".repeat(level));
        if body.starts_with("//") {
            out.push_str(body);
            comment = Some((indent, level));
        } else if let Some(rest) = body.strip_prefix('|') {
            out.push_str(&bar_line(rest));
        } else {
            let mut d = Vec::new();
            match parse_line(body, 0, &mut d) {
                Some(t) if t.name == "li" => out.push_str(&li_line(&t, in_title)),
                Some(t) => {
                    out.push_str(&tag_line(&t));
                    stack.push((indent, t.name));
                    out.push('\n');
                    continue;
                }
                None => out.push_str(body),
            }
            stack.push((indent, String::new()));
        }
        out.push('\n');
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::format;

    #[test]
    fn canonical() {
        let src = "diagram(dir=\"down\")\n\n    card#fw.t0(step=1 at=\"1,2\") Pare-feu\n        li x\n        to(core).wan(side='left-top')\n    card#core Core\n";
        // deux groupes d'attributs : erreur, refus
        assert!(format(src).is_err());
        let src = "diagram(dir=\"down\")\n\n    card#fw.t0(step=1 fx=\"up\" at=\"1,2\") Pare-feu\n        li x\n        to(core side='left-top').wan transit\n    //- note\n        sous la note\n    card#core Core\n";
        let out = format(src).unwrap();
        assert_eq!(
            out,
            "diagram(dir=\"down\")\n\n  card#fw.t0(at=\"1,2\") Pare-feu @1:up\n    | x\n    to(core side=\"left-top\").wan transit\n  //- note\n    sous la note\n  card#core Core\n"
        );
        assert_eq!(format(&out).unwrap(), out);
    }

    #[test]
    fn to_v08() {
        let src = "diagram(layout=\"frise\" from=\"-800\" to=\"600\")\n  title T\n    li ns\n    li.pill v1\n  card#a(date=\"1\") A\n    li.warn x @2\n    li\n    li(step=3) y\n    |  spaced  \n    |.ok fine\n    | a | b  |\n    |---|---|\n";
        let out = format(src).unwrap();
        assert_eq!(
            out,
            "diagram(layout=\"frieze\" start=\"-800\" end=\"600\")\n  title T\n    |.pill ns\n    |.pill v1\n  card#a(date=\"1\") A\n    |.warn x @2\n    |\n    | y @3\n    | spaced\n    |.ok fine\n    | a | b  |\n    |---|---|\n"
        );
        assert_eq!(format(&out).unwrap(), out);
    }
}
