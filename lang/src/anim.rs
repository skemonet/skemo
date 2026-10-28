//! Animation markers `@` at the end of a line (FORMAT §3.8).

use crate::diag::{Diagnostic, Span};
use crate::model::{Anim, Focus, Step};
use crate::tr;

pub const EFFECTS: [&str; 5] = ["fade", "up", "zoom", "draw", "focus"];

/// What reading a text gives: the text shown, the markers, the markers' positions.
pub struct Marked {
    pub text: String,
    pub anim: Option<Anim>,
    pub markers: Vec<Span>,
}

/// A step value ≥ 0 (0 only makes sense as the start of a range: `@0-3`, see `parse_marker`).
fn step(raw: &str) -> Option<Step> {
    let ok = !raw.is_empty()
        && raw.chars().all(|c| c.is_ascii_digit() || c == '.')
        && raw.matches('.').count() <= 1
        && !raw.starts_with('.')
        && !raw.ends_with('.');
    if !ok {
        return None;
    }
    let value: f64 = raw.parse().ok()?;
    (value >= 0.0).then(|| Step {
        raw: raw.to_string(),
        value,
    })
}

enum Token {
    Appear {
        start: Step,
        until: Option<Step>,
        fx: Option<String>,
    },
    Focus(Focus),
}

/// `tok` starts with `@` followed by a digit: an attempted marker.
fn parse_marker(tok: &str) -> Result<Token, String> {
    let body = &tok[1..];
    let (range, fx) = match body.split_once(':') {
        Some((r, f)) => (r, Some(f)),
        None => (body, None),
    };
    let (a, b) = match range.split_once('-') {
        Some((a, b)) => (a, Some(b)),
        None => (range, None),
    };
    let a = step(a).ok_or_else(|| {
        tr!(
            "« {tok} »: invalid step (a number is expected, e.g. @2 or @2.5)",
            "« {tok} » : étape invalide (nombre attendu, ex. @2 ou @2.5)"
        )
    })?;
    if a.value == 0.0 && b.is_none() {
        return Err(tr!(
            "« {tok} »: step 0 is the initial state, already visible; write « @0-N » to hide the element after step N",
            "« {tok} » : l'étape 0 est l'état initial, déjà visible ; écrire « @0-N » pour masquer l'élément après l'étape N"
        ));
    }
    let b = match b {
        Some(b) => Some(step(b).filter(|b| b.value > 0.0).ok_or_else(|| {
            tr!(
                "« {tok} »: invalid end of range",
                "« {tok} » : fin de plage invalide"
            )
        })?),
        None => None,
    };
    if let Some(b) = &b
        && b.value < a.value
    {
        return Err(tr!(
            "« {tok} »: the end of the range is before its start",
            "« {tok} » : la fin de la plage précède son début"
        ));
    }
    match fx {
        Some("focus") => Ok(Token::Focus(Focus { from: a, to: b })),
        Some(f) if EFFECTS.contains(&f) => Ok(Token::Appear {
            start: a,
            until: b,
            fx: Some(f.to_string()),
        }),
        Some(f) => Err(tr!(
            "« {tok} »: unknown effect « {f} » (fade, up, zoom, draw, focus)",
            "« {tok} » : effet « {f} » inconnu (fade, up, zoom, draw, focus)"
        )),
        None => Ok(Token::Appear {
            start: a,
            until: b,
            fx: None,
        }),
    }
}

/// Reads the final markers of `text` (which starts at byte `base`), removes them from the text and
/// replaces `\@` with `@`.
pub fn read(text: &str, base: usize, diags: &mut Vec<Diagnostic>) -> Marked {
    // tokens (start, end) separated by blanks
    let mut toks = Vec::new();
    let mut start = None;
    for (i, c) in text.char_indices() {
        if c.is_whitespace() {
            if let Some(s) = start.take() {
                toks.push((s, i));
            }
        } else if start.is_none() {
            start = Some(i);
        }
    }
    if let Some(s) = start {
        toks.push((s, text.len()));
    }
    let mut anim = Anim::default();
    let mut markers = Vec::new();
    let mut cut = text.len();
    let mut appear_seen = false;
    for &(s, e) in toks.iter().rev() {
        let tok = &text[s..e];
        let is_attempt = tok.starts_with('@')
            && tok[1..]
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_digit() || c == '.');
        if !is_attempt {
            break;
        }
        let span = Span::new(base + s, base + e);
        markers.push(span);
        cut = s;
        match parse_marker(tok) {
            Ok(Token::Appear { start, until, fx }) => {
                if appear_seen {
                    diags.push(
                        Diagnostic::error(
                            "multiple-appear",
                            span,
                            tr!(
                                "« {tok} »: only one appearance marker per element",
                                "« {tok} » : un seul marqueur d'apparition par élément"
                            ),
                        )
                        .with_help(tr!(
                            "several markers are only allowed for :focus",
                            "plusieurs marqueurs ne sont permis que pour :focus"
                        )),
                    );
                    continue;
                }
                appear_seen = true;
                anim.start = Some(start);
                anim.until = until;
                anim.fx = fx;
            }
            Ok(Token::Focus(f)) => anim.focus.insert(0, f),
            Err(m) => diags.push(Diagnostic::error("bad-marker", span, m)),
        }
    }
    markers.reverse();
    let shown = text[..cut].trim_end().replace("\\@", "@");
    Marked {
        text: shown,
        anim: (!anim.is_empty()).then_some(anim),
        markers,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(t: &str) -> (String, Option<Anim>, usize) {
        let mut d = Vec::new();
        let m = read(t, 0, &mut d);
        (m.text, m.anim, d.len())
    }

    #[test]
    fn markers() {
        let (t, a, e) = r("seg-k8s-01   @1:up @4-6:focus");
        assert_eq!((t.as_str(), e), ("seg-k8s-01", 0));
        let a = a.unwrap();
        assert_eq!(a.start.unwrap().raw, "1");
        assert_eq!(a.fx.as_deref(), Some("up"));
        assert_eq!(a.focus[0].to.as_ref().unwrap().raw, "6");
    }

    #[test]
    fn only_final_tokens() {
        let (t, a, _) = r("rendez-vous @3 demain");
        assert_eq!(t, "rendez-vous @3 demain");
        assert!(a.is_none());
        let (t, a, _) = r("voir \\@2 ou admin@example.com");
        assert_eq!(t, "voir @2 ou admin@example.com");
        assert!(a.is_none());
    }

    #[test]
    fn errors() {
        assert_eq!(r("x @0").2, 1);
        assert_eq!(r("x @4-2").2, 1);
        assert_eq!(r("x @2:wiggle").2, 1);
        assert_eq!(r("x @1 @2").2, 1);
        assert_eq!(r("x @2.5").2, 0);
        assert_eq!(r("x @0-0").2, 1);
    }

    #[test]
    fn visible_from_the_start_until_a_step() {
        let (t, a, e) = r("NodePort @0-2");
        assert_eq!((t.as_str(), e), ("NodePort", 0));
        let a = a.unwrap();
        assert_eq!(a.start.unwrap().value, 0.0);
        assert_eq!(a.until.unwrap().raw, "2");
    }
}
