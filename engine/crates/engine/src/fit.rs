//! Target box (`diagram(fit="16:9")` or `fit="1280x720"`): a check and suggestions, never an
//! automatic change of the layout (stability first, D-031). What counts is the size at which the
//! text is shown once the diagram is fitted in the box (D-043), not the closeness of the
//! proportions: a candidate with the right ratio but twice as large shows smaller text. The suggestions are written settings
//! (`dir`, `wrap` on a zone) that the author, or `skemo fit --write`, puts in the source. A
//! candidate whose arrows get worse (a card hidden behind others by `wrap`, many more bends) is
//! never suggested, however large its text.
//!
//! Each candidate is the source with one setting changed, laid out for real: what is suggested
//! is exactly what the author gets.

use crate::diagram::{Options, from_source};
use skemo_lang::edit::{self, remove_attr, set_attr};
use skemo_lang::{Kind, tr};

/// A suggestion is worth making when the text gets at least this much larger.
pub const TOLERANCE: f64 = 1.15;

/// At most this many candidates are laid out (each one is a full layout).
const MAX_CANDIDATES: usize = 24;

/// Base text size of the default theme (px), to state the size obtained in a target box.
const BASE_TEXT: f64 = 14.0;

/// One written setting: `name="value"` on the diagram or on a zone.
#[derive(Debug, Clone, PartialEq)]
pub struct Setting {
    /// `None` = the `diagram` line, otherwise a zone id.
    pub zone: Option<String>,
    pub name: String,
    pub value: String,
}

impl Setting {
    pub fn describe(&self) -> String {
        match &self.zone {
            None => format!("diagram({}=\"{}\")", self.name, self.value),
            Some(z) => format!("zone#{z}({}=\"{}\")", self.name, self.value),
        }
    }
}

/// The box the diagram is shown in: a ratio (`16:9`), or a size in px (`1280x720`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Target {
    pub ratio: f64,
    pub size: Option<(f64, f64)>,
}

impl Target {
    /// Scale at which a diagram of `w × h` px is shown, fitted in the box. Without a size, the
    /// box is `ratio × 1` and only comparisons between candidates mean something.
    pub fn scale(&self, (w, h): (f64, f64)) -> f64 {
        let (bw, bh) = self.size.unwrap_or((self.ratio, 1.0));
        (bw / w).min(bh / h)
    }
}

/// A candidate: its settings, its size (px), the scale it is shown at in the target and the
/// number of bends of its arrows.
#[derive(Debug, Clone, PartialEq)]
pub struct Suggestion {
    pub settings: Vec<Setting>,
    pub size: (f64, f64),
    pub scale: f64,
    pub bends: usize,
}

/// What a layout looks like, for comparing candidates: size (px), bends of all the arrows, and
/// cards of `wrap` zones that their arrows cannot reach straight (`tidy::hidden_by_wrap`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shape {
    pub size: (f64, f64),
    pub bends: usize,
    pub hidden: usize,
}

impl Suggestion {
    pub fn aspect(&self) -> f64 {
        self.size.0 / self.size.1
    }
}

/// `"16:9"`, `"4:3"`, `"1.78"` (ratio) or `"1280x720"` (box in px).
pub fn parse_target(s: &str) -> Option<Target> {
    let s = s.trim();
    let num = |t: &str| {
        t.trim()
            .parse::<f64>()
            .ok()
            .filter(|v| v.is_finite() && *v > 0.0)
    };
    let t = if let Some((w, h)) = s.split_once(['x', '×']) {
        let (w, h) = (num(w)?, num(h)?);
        Target {
            ratio: w / h,
            size: Some((w, h)),
        }
    } else {
        let ratio = match s.split_once(':') {
            Some((w, h)) => num(w)? / num(h)?,
            None => num(s)?,
        };
        Target { ratio, size: None }
    };
    (t.ratio.is_finite() && t.ratio > 0.0).then_some(t)
}

/// How far `aspect` is from `target` (1 = exact; symmetric: 2 means twice too wide or too tall).
pub fn misfit(aspect: f64, target: f64) -> f64 {
    (aspect / target).ln().abs().exp()
}

/// Size (px) of the source as written, `fit` aside (`None` if it does not build).
pub fn size_of(src: &str, opts: &Options) -> Option<(f64, f64)> {
    let src = remove_attr(src, &edit::Target::Diagram, "fit").unwrap_or_else(|_| src.to_string());
    let (d, _) = from_source(&src, opts).ok()?;
    (d.frame.w > 0.0 && d.frame.h > 0.0).then_some((d.frame.w, d.frame.h))
}

/// Shape of the source as written, `fit` aside (`None` if it does not build).
pub fn shape_of(src: &str, opts: &Options) -> Option<Shape> {
    let src = remove_attr(src, &edit::Target::Diagram, "fit").unwrap_or_else(|_| src.to_string());
    let (d, _) = from_source(&src, opts).ok()?;
    if d.frame.w <= 0.0 || d.frame.h <= 0.0 {
        return None;
    }
    Some(Shape {
        size: (d.frame.w, d.frame.h),
        bends: d.routes.values().map(|r| r.len().saturating_sub(2)).sum(),
        hidden: crate::tidy::hidden_by_wrap(&d.model, &d.boxes)
            .iter()
            .map(|h| h.cards.len())
            .sum(),
    })
}

/// A candidate's arrows are acceptable: no card hidden by `wrap`, and not many more bends than
/// the source (the text may be larger, the diagram must stay readable).
fn readable(cand: &Shape, now: &Shape) -> bool {
    cand.hidden == 0 && cand.bends <= now.bends + 2.max(now.bends / 4)
}

/// Writes the settings into the source (`diagram` line added if there is none).
pub fn apply(src: &str, settings: &[Setting]) -> Option<String> {
    let mut out = src.to_string();
    for s in settings {
        let target = match &s.zone {
            None => edit::Target::Diagram,
            Some(z) => edit::Target::Node(z.clone()),
        };
        out = match set_attr(&out, &target, &s.name, &s.value) {
            Ok(o) => o,
            Err(_) if s.zone.is_none() => {
                format!("diagram({}=\"{}\")\n{out}", s.name, s.value)
            }
            Err(_) => return None,
        };
    }
    Some(out)
}

fn flipped(dir: Option<&str>) -> &'static str {
    match dir {
        Some("right") => "down",
        Some("left") => "up",
        Some("up") => "left",
        _ => "right",
    }
}

/// Candidate settings: the other direction, and `wrap` on each zone with at least 3 children
/// (alone, and with the other direction).
fn candidates(src: &str) -> Vec<Vec<Setting>> {
    let doc = skemo_lang::parse(src);
    let model = &doc.model;
    let dir = Setting {
        zone: None,
        name: "dir".into(),
        value: flipped(model.option("dir")).into(),
    };
    let mut out = vec![vec![dir.clone()]];
    for z in model.nodes.iter().filter(|n| n.kind == Kind::Zone) {
        let k = model.children(Some(&z.id)).count();
        if k < 3 {
            continue;
        }
        let current = z.wrap.map_or(k, |w| w as usize);
        for w in (1..k).filter(|w| *w != current) {
            let wrap = Setting {
                zone: Some(z.id.clone()),
                name: "wrap".into(),
                value: w.to_string(),
            };
            out.push(vec![wrap.clone()]);
            out.push(vec![dir.clone(), wrap]);
        }
    }
    out.truncate(MAX_CANDIDATES);
    out
}

/// Candidates that show the text larger in the target than the source as written and keep the
/// arrows readable (`readable`), best first; among those within 5 % of the best, the fewest
/// settings first, then the fewest bends.
pub fn suggestions(src: &str, opts: &Options, target: Target) -> Vec<Suggestion> {
    let Some(shape) = shape_of(src, opts) else {
        return Vec::new();
    };
    let now = target.scale(shape.size);
    let base = remove_attr(src, &edit::Target::Diagram, "fit").unwrap_or_else(|_| src.to_string());
    let mut out: Vec<Suggestion> = candidates(&base)
        .into_iter()
        .filter_map(|settings| {
            let s = apply(&base, &settings)?;
            let cand = shape_of(&s, opts)?;
            let scale = target.scale(cand.size);
            (scale > now * 1.02 && readable(&cand, &shape)).then_some(Suggestion {
                settings,
                size: cand.size,
                scale,
                bends: cand.bends,
            })
        })
        .collect();
    let best = out.iter().map(|s| s.scale).fold(0.0, f64::max);
    let near = |s: &Suggestion| s.scale >= best / 1.05;
    out.sort_by(|a, b| {
        near(b)
            .cmp(&near(a))
            .then(if near(a) && near(b) {
                a.settings
                    .len()
                    .cmp(&b.settings.len())
                    .then(a.bends.cmp(&b.bends))
            } else {
                std::cmp::Ordering::Equal
            })
            .then(b.scale.total_cmp(&a.scale))
    });
    out
}

/// Text size obtained in a target given in px (`None` for a bare ratio).
pub fn text_px(target: Target, size: (f64, f64)) -> Option<f64> {
    target.size.map(|_| BASE_TEXT * target.scale(size))
}

/// Warning for `diagram(fit=…)` when the proportions are more than `TOLERANCE` away from the
/// target and no written setting helps: the author has to rework the diagram (or the target).
fn off_target(raw: &str, target: Target, size: (f64, f64)) -> Option<String> {
    let aspect = size.0 / size.1;
    if misfit(aspect, target.ratio) < TOLERANCE {
        return None;
    }
    let shape = if aspect >= 1.0 {
        format!("{aspect:.1}:1")
    } else {
        format!("1:{:.1}", 1.0 / aspect)
    };
    let px = text_px(target, size).map_or(String::new(), |p| format!(", text {p:.1} px"));
    Some(tr!(
        "fit=\"{raw}\": the diagram is {shape}{px}, and no `dir` or `wrap` setting fits it better; rework its layout or change the target",
        "fit=\"{raw}\" : le schéma est en {shape}{px}, et aucun réglage `dir` ou `wrap` ne l'ajuste mieux ; revoir sa disposition ou changer la cible"
    ))
}

/// Warning for `diagram(fit=…)` when a written setting would show the text clearly larger.
pub fn check(src: &str, opts: &Options, size: (f64, f64)) -> Option<String> {
    let doc = skemo_lang::parse(src);
    let raw = doc.model.option("fit")?;
    let Some(target) = parse_target(raw) else {
        return Some(tr!(
            "fit=\"{raw}\": a ratio like \"16:9\" or a box like \"1280x720\" is expected; ignored",
            "fit=\"{raw}\" : un rapport comme \"16:9\" ou une boîte comme \"1280x720\" est attendu ; ignoré"
        ));
    };
    if doc.model.option("layout").is_some_and(|l| l != "graph") {
        return None;
    }
    let now = target.scale(size);
    let best = suggestions(src, opts, target).into_iter().next();
    let Some(best) = best.filter(|b| b.scale / now >= TOLERANCE) else {
        return off_target(raw, target, size);
    };
    let gain = best.scale / now;
    let what: Vec<String> = best.settings.iter().map(Setting::describe).collect();
    let px = match (text_px(target, size), text_px(target, best.size)) {
        (Some(a), Some(b)) => format!(" ({a:.1} px → {b:.1} px)"),
        _ => String::new(),
    };
    Some(tr!(
        "fit=\"{raw}\": with {} the text would be shown {gain:.2}× larger{px}; or `skemo fit --write`",
        "fit=\"{raw}\" : avec {} le texte serait affiché {gain:.2}× plus grand{px} ; ou `skemo fit --write`",
        what.join(" + ")
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn targets() {
        assert_eq!(parse_target("16:9").unwrap().ratio, 16.0 / 9.0);
        assert_eq!(parse_target("1.5").unwrap().size, None);
        let t = parse_target("1280x720").unwrap();
        assert_eq!(t.size, Some((1280.0, 720.0)));
        assert!(
            (t.scale((2560.0, 720.0)) - 0.5).abs() < 1e-9,
            "limited by the width"
        );
        assert!(
            (t.scale((640.0, 1440.0)) - 0.5).abs() < 1e-9,
            "limited by the height"
        );
        assert_eq!(parse_target("0:9"), None);
        assert_eq!(parse_target("wide"), None);
        assert!((misfit(2.0, 1.0) - 2.0).abs() < 1e-9);
        assert!((misfit(0.5, 1.0) - 2.0).abs() < 1e-9);
    }

    #[test]
    fn apply_adds_a_diagram_line_when_missing() {
        let s = apply(
            "card#a A\n",
            &[Setting {
                zone: None,
                name: "dir".into(),
                value: "right".into(),
            }],
        )
        .unwrap();
        assert_eq!(s, "diagram(dir=\"right\")\ncard#a A\n");
    }
}
