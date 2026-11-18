//! Messages to the user on standard error: text, with or without colours, or one JSON object per
//! line (`--diagnostics json`, `cli/SPEC.md` §2.2).

use serde_json::{Map, Value, json};
use skemo_lang::tr;
use std::cell::RefCell;
use std::sync::OnceLock;

/// Warning codes left out (`--allow`, `cli/SPEC.md` §2.8): never shown, never counted.
static ALLOWED: OnceLock<Vec<String>> = OnceLock::new();

/// Sets the codes of `--allow`, once, before any message.
pub fn set_allowed(codes: Vec<String>) {
    let _ = ALLOWED.set(codes.into_iter().map(|c| c.trim().to_string()).collect());
}

/// True if warnings of this code are left out. Errors never are.
pub fn allowed(code: &str) -> bool {
    ALLOWED.get().is_some_and(|v| v.iter().any(|c| c == code))
}

thread_local! {
    /// While set (`skemo render --batch`), the JSON lines go here instead of the standard error.
    static SINK: RefCell<Option<Vec<Value>>> = const { RefCell::new(None) };
}

/// Runs `f` with the JSON diagnostics collected rather than printed; returns them with its result.
pub fn collect<T>(f: impl FnOnce() -> T) -> (T, Vec<Value>) {
    SINK.with(|s| *s.borrow_mut() = Some(Vec::new()));
    let r = f();
    let got = SINK.with(|s| s.borrow_mut().take()).unwrap_or_default();
    (r, got)
}

/// Prints a JSON line on the standard error, or keeps it in the sink.
fn emit(line: &str) {
    let kept = SINK.with(|s| {
        let mut s = s.borrow_mut();
        match s.as_mut() {
            Some(v) => {
                v.push(serde_json::from_str(line).unwrap_or(Value::Null));
                true
            }
            None => false,
        }
    });
    if !kept {
        eprintln!("{line}");
    }
}

/// Output of the diagnostics and the messages.
#[derive(Clone, Copy, Default)]
pub struct Out {
    pub color: bool,
    pub quiet: bool,
    /// `--diagnostics json`: every message is a JSON line; notes are left out.
    pub json: bool,
}

/// Where a problem lies: a file (`-` for the source on the standard input) and, when it has one,
/// a byte range of that file's text.
#[derive(Clone, Copy)]
pub struct Place<'a> {
    pub file: &'a str,
    pub at: Option<(&'a str, skemo_lang::Span)>,
}

impl<'a> Place<'a> {
    /// A file as a whole (a missing image, a stylesheet that cannot be read).
    pub fn file(file: &'a str) -> Self {
        Place { file, at: None }
    }
}

/// One diagnostic as a JSON object: severity, stable code, message, help, file, range (lines and
/// columns from 1, columns in characters, the end just after the last character).
pub fn json_line(
    error: bool,
    code: &str,
    message: &str,
    help: Option<&str>,
    place: Place,
) -> String {
    let mut o = Map::new();
    o.insert(
        "severity".into(),
        json!(if error { "error" } else { "warning" }),
    );
    o.insert("code".into(), json!(code));
    o.insert("message".into(), json!(message));
    if let Some(h) = help {
        o.insert("help".into(), json!(h));
    }
    o.insert("file".into(), json!(place.file));
    if let Some((src, span)) = place.at {
        let (l, c) = skemo_lang::diag::line_col(src, span.start);
        let (el, ec) = skemo_lang::diag::line_col(src, span.end);
        o.insert("line".into(), json!(l));
        o.insert("column".into(), json!(c));
        o.insert("end_line".into(), json!(el));
        o.insert("end_column".into(), json!(ec));
    }
    Value::Object(o).to_string()
}

impl Out {
    /// A diagnostic from `lang` (warnings are left out in quiet mode). `file` is the name shown:
    /// `-` for the standard input.
    pub fn diag(&self, src: &str, file: &str, d: &skemo_lang::Diagnostic) {
        if !d.is_error() && (self.quiet || allowed(d.code)) {
            return;
        }
        if self.json {
            let place = Place {
                file,
                at: Some((src, d.span)),
            };
            emit(&json_line(
                d.is_error(),
                d.code,
                &d.message,
                d.help.as_deref(),
                place,
            ));
            return;
        }
        let shown = if file == "-" { "<stdin>" } else { file };
        let mut s = skemo_lang::diag::render(src, shown, d);
        if self.color {
            let (word, code) = if d.is_error() {
                (tr!("error", "erreur"), "1;31")
            } else {
                (tr!("warning", "avertissement"), "1;33")
            };
            s = s.replacen(&word, &format!("\x1b[{code}m{word}\x1b[0m"), 1);
        }
        eprint!("{s}");
    }

    /// A warning of the command or the engine, without a stable code of its own.
    pub fn warn(&self, msg: &str) {
        self.warn_at("warning", msg, Place::file("-"));
    }

    /// A warning with its code and place.
    pub fn warn_at(&self, code: &str, msg: &str, place: Place) {
        if self.quiet || allowed(code) {
            return;
        }
        if self.json {
            emit(&json_line(false, code, msg, None, place));
            return;
        }
        eprintln!(
            "{}{} {msg}",
            self.tag(&tr!("warning", "avertissement"), "1;33"),
            tr!(":", " :")
        );
    }

    /// Information line (files written…), omitted in quiet mode and in JSON.
    pub fn note(&self, msg: &str) {
        if !self.quiet && !self.json {
            eprintln!("{msg}");
        }
    }

    /// An error that ends the command; its code follows the exit code when it has none of its own.
    pub fn error(&self, msg: &str) {
        self.error_at("error", msg, Place::file("-"));
    }

    pub fn error_at(&self, code: &str, msg: &str, place: Place) {
        if self.json {
            emit(&json_line(true, code, msg, None, place));
            return;
        }
        eprintln!(
            "{}{} {msg}",
            self.tag(&tr!("error", "erreur"), "1;31"),
            tr!(":", " :")
        );
    }

    /// Tag word, coloured if asked.
    fn tag(&self, word: &str, code: &str) -> String {
        if self.color {
            format!("\x1b[{code}m{word}\x1b[0m")
        } else {
            word.to_string()
        }
    }

    /// Same output, without the warnings.
    pub fn silenced(self) -> Out {
        Out {
            quiet: true,
            ..self
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_are_colored_only_on_request() {
        let plain = Out::default();
        let colored = Out {
            color: true,
            ..Out::default()
        };
        assert_eq!(plain.tag("erreur", "1;31"), "erreur");
        assert_eq!(colored.tag("erreur", "1;31"), "\x1b[1;31merreur\x1b[0m");
    }

    #[test]
    fn silenced_keeps_color() {
        let o = Out {
            color: true,
            ..Out::default()
        }
        .silenced();
        assert!(o.quiet && o.color);
    }

    #[test]
    fn json_lines_count_columns_in_characters_from_one() {
        let src = "diagram\n  card#é A\n    to(db)\n";
        let at = src.find("db").unwrap();
        let span = skemo_lang::Span::new(at, at + 2);
        let line = json_line(
            true,
            "unknown-target",
            "unknown target",
            Some("declare it"),
            Place {
                file: "-",
                at: Some((src, span)),
            },
        );
        let v: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(v["line"], 3);
        assert_eq!(v["column"], 8);
        assert_eq!(v["end_column"], 10);
        assert_eq!(v["help"], "declare it");
        let v: Value =
            serde_json::from_str(&json_line(false, "image", "x", None, Place::file("a.png")))
                .unwrap();
        assert_eq!(v["severity"], "warning");
        assert!(v.get("line").is_none() && v.get("help").is_none());
    }
}
