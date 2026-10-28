//! Emphasis inside lines and table cells (FORMAT §3.2.4): Markdown's `**bold**`, `*italic*`,
//! `` `mono` ``, `==mark==` and `==.class mark==`, with the flanking rules of `CommonMark` (a delimiter
//! opens before a non-space and closes after one; a delimiter with no partner stays text).
//! `_` is not emphasis. Backslash escapes any punctuation (`\|`, `\*`, `` \` ``, `\=`, `\@`).
//!
//! ```
//! use skemo_lang::inline::runs;
//! let r = runs("443 from ==0.0.0.0/0== · **TLS**");
//! assert_eq!(r[1].text, "0.0.0.0/0");
//! assert_eq!(r[1].mark, Some(vec![]));
//! assert!(r[3].strong);
//! assert_eq!(runs("cron(0 2 * * ? *)")[0].text, "cron(0 2 * * ? *)");
//! ```

use serde::{Deserialize, Serialize};

/// A piece of text with one style.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Run {
    pub text: String,
    #[serde(default, skip_serializing_if = "is_no")]
    pub strong: bool,
    #[serde(default, skip_serializing_if = "is_no")]
    pub em: bool,
    #[serde(default, skip_serializing_if = "is_no")]
    pub code: bool,
    /// `==…==`: marked, with the classes written after the opening `==` (`==.ok …==`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mark: Option<Vec<String>>,
}

impl Run {
    fn same_style(&self, o: &Run) -> bool {
        self.strong == o.strong && self.em == o.em && self.code == o.code && self.mark == o.mark
    }
}

#[allow(clippy::trivially_copy_pass_by_ref)] // serde passes a reference
fn is_no(b: &bool) -> bool {
    !*b
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Strong,
    Em,
    Mark,
}

#[derive(Debug, Clone)]
enum Tok {
    Text(String),
    Code(String),
    Delim {
        kind: Kind,
        raw: String,
        open: bool,
        close: bool,
        classes: Vec<String>,
    },
}

/// The text of `s` with its emphasis resolved; plain text gives a single run.
pub fn runs(s: &str) -> Vec<Run> {
    let toks = tokens(s);
    let pairs = pair(&toks);
    let mut out: Vec<Run> = Vec::new();
    let mut active: Vec<(Kind, Vec<String>)> = Vec::new();
    for (i, t) in toks.iter().enumerate() {
        let text = match t {
            Tok::Text(x) => x.clone(),
            Tok::Code(x) => {
                push(&mut out, style(&active, x.clone(), true));
                continue;
            }
            Tok::Delim {
                kind, raw, classes, ..
            } => match pairs[i] {
                Some(Side::Open) => {
                    active.push((*kind, classes.clone()));
                    continue;
                }
                Some(Side::Close) => {
                    if let Some(k) = active.iter().rposition(|(a, _)| a == kind) {
                        active.remove(k);
                    }
                    continue;
                }
                None => raw.clone(),
            },
        };
        push(&mut out, style(&active, text, false));
    }
    if out.is_empty() {
        out.push(Run::default());
    }
    out
}

/// The text of `s` without its delimiters (what is drawn, for measuring and plain outputs).
pub fn plain(s: &str) -> String {
    runs(s).into_iter().map(|r| r.text).collect()
}

fn style(active: &[(Kind, Vec<String>)], text: String, code: bool) -> Run {
    let mut r = Run {
        text,
        code,
        ..Run::default()
    };
    for (k, c) in active {
        match k {
            Kind::Strong => r.strong = true,
            Kind::Em => r.em = true,
            Kind::Mark => r.mark = Some(c.clone()),
        }
    }
    r
}

fn push(out: &mut Vec<Run>, r: Run) {
    if r.text.is_empty() {
        return;
    }
    match out.last_mut() {
        Some(l) if l.same_style(&r) => l.text.push_str(&r.text),
        _ => out.push(r),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Side {
    Open,
    Close,
}

/// Matches openers and closers (the delimiter stack of `CommonMark`, without the "rule of 3").
fn pair(toks: &[Tok]) -> Vec<Option<Side>> {
    let mut sides = vec![None; toks.len()];
    let mut stack: Vec<usize> = Vec::new();
    for (i, t) in toks.iter().enumerate() {
        let Tok::Delim {
            kind, open, close, ..
        } = t
        else {
            continue;
        };
        let opener = if *close {
            stack
                .iter()
                .rposition(|&j| delim_kind(&toks[j]) == Some(*kind))
        } else {
            None
        };
        if let Some(k) = opener {
            sides[stack[k]] = Some(Side::Open);
            sides[i] = Some(Side::Close);
            // openers between the pair can no longer be closed (CommonMark)
            stack.truncate(k);
        } else if *open {
            stack.push(i);
        }
    }
    sides
}

fn delim_kind(t: &Tok) -> Option<Kind> {
    match t {
        Tok::Delim { kind, .. } => Some(*kind),
        _ => None,
    }
}

fn tokens(s: &str) -> Vec<Tok> {
    let cs: Vec<char> = s.chars().collect();
    let mut toks: Vec<Tok> = Vec::new();
    let mut text = String::new();
    let mut i = 0;
    while i < cs.len() {
        let c = cs[i];
        if c == '\\' && cs.get(i + 1).is_some_and(char::is_ascii_punctuation) {
            text.push(cs[i + 1]);
            i += 2;
            continue;
        }
        if c == '`'
            && let Some((code, next)) = code_span(&cs, i)
        {
            flush(&mut toks, &mut text);
            toks.push(Tok::Code(code));
            i = next;
            continue;
        }
        if let Some((tok, next)) = delimiter(&cs, i) {
            flush(&mut toks, &mut text);
            toks.push(tok);
            i = next;
            continue;
        }
        text.push(c);
        i += 1;
    }
    flush(&mut toks, &mut text);
    toks
}

fn flush(toks: &mut Vec<Tok>, text: &mut String) {
    if !text.is_empty() {
        toks.push(Tok::Text(std::mem::take(text)));
    }
}

/// A code span starting at `i` (a run of n backticks closed by a run of exactly n).
fn code_span(cs: &[char], i: usize) -> Option<(String, usize)> {
    let n = cs[i..].iter().take_while(|&&c| c == '`').count();
    let mut j = i + n;
    while j < cs.len() {
        if cs[j] == '`' {
            let m = cs[j..].iter().take_while(|&&c| c == '`').count();
            if m == n {
                let mut code: String = cs[i + n..j].iter().collect();
                if code.len() > 2 && code.starts_with(' ') && code.ends_with(' ') {
                    code = code[1..code.len() - 1].to_string();
                }
                return Some((code, j + m));
            }
            j += m;
        } else {
            j += 1;
        }
    }
    None
}

/// A delimiter `**`, `*` or `==` at `i`, with whether it may open or close.
fn delimiter(cs: &[char], i: usize) -> Option<(Tok, usize)> {
    let c = cs[i];
    let (kind, len) = match c {
        '*' if cs.get(i + 1) == Some(&'*') => (Kind::Strong, 2),
        '*' => (Kind::Em, 1),
        '=' if cs.get(i + 1) == Some(&'=') && cs.get(i + 2) != Some(&'=') => (Kind::Mark, 2),
        _ => return None,
    };
    // a longer run of the same character (`===`, `***` handled as `**` then `*`) stays text
    // for marks; `=` runs of three or more are left alone
    if kind == Kind::Mark && i > 0 && cs[i - 1] == '=' {
        return None;
    }
    let before = i.checked_sub(1).map(|k| cs[k]);
    let after = cs.get(i + len).copied();
    let open = after.is_some_and(|a| !a.is_whitespace());
    let close = before.is_some_and(|b| !b.is_whitespace());
    let mut next = i + len;
    let mut classes = Vec::new();
    if kind == Kind::Mark && open {
        let (cl, end) = mark_classes(cs, next);
        if !cl.is_empty() {
            classes = cl;
            next = end;
        }
    }
    let raw: String = cs[i..next].iter().collect();
    Some((
        Tok::Delim {
            kind,
            raw,
            open,
            close,
            classes,
        },
        next,
    ))
}

/// `.class.other ` right after an opening `==`: the classes and the index after the space.
fn mark_classes(cs: &[char], mut i: usize) -> (Vec<String>, usize) {
    let start = i;
    let mut out = Vec::new();
    while cs.get(i) == Some(&'.')
        && cs
            .get(i + 1)
            .is_some_and(|c| c.is_alphabetic() || *c == '_')
    {
        let s = i + 1;
        let mut e = s;
        while cs
            .get(e)
            .is_some_and(|c| c.is_alphanumeric() || *c == '_' || *c == '-')
        {
            e += 1;
        }
        out.push(cs[s..e].iter().collect());
        i = e;
    }
    // the classes end at a space, which is not drawn
    if !out.is_empty() && cs.get(i) == Some(&' ') {
        (out, i + 1)
    } else {
        (Vec::new(), start)
    }
}

/// Splits `text` into the parts of a table row on its unescaped `|` (FORMAT §3.2.5); escapes
/// are kept for `runs`.
pub(crate) fn split_cells(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut esc = false;
    for c in text.chars() {
        if esc {
            cur.push(c);
            esc = false;
        } else if c == '\\' {
            cur.push(c);
            esc = true;
        } else if c == '|' {
            out.push(std::mem::take(&mut cur));
        } else {
            cur.push(c);
        }
    }
    out.push(cur);
    out
}

/// `true` when `text` (what follows the opening bar) ends with an unescaped `|`.
pub(crate) fn ends_with_bar(text: &str) -> bool {
    let t = text.trim_end();
    if !t.ends_with('|') {
        return false;
    }
    let slashes = t[..t.len() - 1]
        .chars()
        .rev()
        .take_while(|&c| c == '\\')
        .count();
    slashes % 2 == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn show(s: &str) -> String {
        runs(s)
            .iter()
            .map(|r| {
                let mut t = r.text.clone();
                if r.code {
                    t = format!("`{t}`");
                }
                if r.em {
                    t = format!("<i>{t}</i>");
                }
                if r.strong {
                    t = format!("<b>{t}</b>");
                }
                if let Some(c) = &r.mark {
                    t = format!(
                        "<m{}>{t}</m>",
                        c.iter().map(|c| format!(".{c}")).collect::<String>()
                    );
                }
                t
            })
            .collect()
    }

    #[test]
    fn emphasis() {
        assert_eq!(show("a **b** c"), "a <b>b</b> c");
        assert_eq!(show("*x* and **y**"), "<i>x</i> and <b>y</b>");
        assert_eq!(show("from ==0.0.0.0/0=="), "from <m>0.0.0.0/0</m>");
        assert_eq!(show("==.ok passed== ok"), "<m.ok>passed</m> ok");
        assert_eq!(show("==.a.b x=="), "<m.a.b>x</m>");
        assert_eq!(show("==**critical**=="), "<m><b>critical</b></m>");
        assert_eq!(show("image `a*b*c`"), "image `a*b*c`");
        assert_eq!(show("`` a`b ``"), "`a`b`");
    }

    #[test]
    fn stays_text() {
        for s in [
            "cron(0 2 * * ? *)",
            "/api/*",
            "status == 200",
            "retry if code==503",
            "order_events DATABASE_HOST",
            "a === b",
            "unclosed `tick",
            "2 * 3 = 6",
        ] {
            assert_eq!(show(s), s, "{s}");
        }
        assert_eq!(show(r"s3:\*Object\*"), "s3:*Object*");
        assert_eq!(show(r"x\==1 and y==2"), "x==1 and y==2");
        assert_eq!(show(r"a \| b"), "a | b");
        assert_eq!(show("x==1 and y==2"), "x<m>1 and y</m>2");
    }

    #[test]
    fn cells() {
        assert_eq!(split_cells(" a | b \\| c |"), vec![" a ", " b \\| c ", ""]);
        assert!(ends_with_bar(" a | b |  "));
        assert!(!ends_with_bar(" a \\|"));
        assert!(ends_with_bar(" a \\\\|"));
        assert_eq!(plain("**a** ==b=="), "a b");
    }
}
