//! Animation (F-ANIM-1 to 3): `animation.pug` against `animation.expected.json`.

use serde_json::Value;
use skemo_engine::anim::resolve;
use std::collections::BTreeMap;
use std::path::Path;

#[test]
fn animation_expected() {
    // the oracle's warnings are the prototype's French texts (own process: the language is global)
    skemo_lang::i18n::set_language(skemo_lang::i18n::Lang::Fr);
    let fx = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    let doc = skemo_lang::parse(&std::fs::read_to_string(fx.join("animation.pug")).unwrap());
    assert!(!doc.has_errors());
    let a = resolve(&doc.model);
    let exp: Value =
        serde_json::from_str(&std::fs::read_to_string(fx.join("animation.expected.json")).unwrap())
            .unwrap();
    // states
    let states: Vec<&str> = a.states.iter().map(|s| s.raw.as_str()).collect();
    let want: Vec<&str> = exp["states"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(states, want);
    // avertissements
    let codes: Vec<(String, &str)> = a
        .warnings
        .iter()
        .map(|w| (w.element.clone(), w.code))
        .collect();
    let want: Vec<(String, &str)> = exp["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|w| {
            (
                w["element"].as_str().unwrap().to_string(),
                w["code"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(codes, want);
    assert_eq!(
        a.warnings[0].message,
        exp["warnings"][0]["message"].as_str().unwrap()
    );
    // attributs SVG
    for (id, attrs) in exp["svg_attributes"].as_object().unwrap() {
        let got: BTreeMap<&str, String> = a.attributes(id).into_iter().collect();
        let want: BTreeMap<&str, String> = attrs
            .as_object()
            .unwrap()
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str().unwrap().to_string()))
            .collect();
        assert_eq!(got, want, "attributs de {id}");
    }
    // visibility and focus per state
    let mut prev: Vec<String> = Vec::new();
    for s in &a.states {
        let vis: Vec<String> = a
            .elements
            .keys()
            .filter(|k| a.visible(k, s.value))
            .cloned()
            .collect();
        let mut delta: Vec<String> = vis
            .iter()
            .filter(|k| !prev.contains(k))
            .map(|k| format!("+{k}"))
            .collect();
        delta.extend(
            prev.iter()
                .filter(|k| !vis.contains(k))
                .map(|k| format!("-{k}")),
        );
        let want = &exp["visible_by_state"][&s.raw];
        let mut want: Vec<String> = want
            .as_array()
            .unwrap()
            .iter()
            .map(|v| {
                let v = v.as_str().unwrap();
                if v.starts_with('+') || v.starts_with('-') {
                    v.to_string()
                } else {
                    format!("+{v}")
                }
            })
            .collect();
        want.sort();
        delta.sort();
        assert_eq!(delta, want, "visibility at state {}", s.raw);
        prev = vis;
        let foc: Vec<String> = a
            .elements
            .keys()
            .filter(|k| a.focused(k, s.value))
            .cloned()
            .collect();
        let want: Vec<String> = exp["focus_by_state"]
            .get(&s.raw)
            .and_then(|v| v.as_array())
            .map(|v| v.iter().map(|x| x.as_str().unwrap().to_string()).collect())
            .unwrap_or_default();
        let mut w2 = want.clone();
        w2.sort();
        assert_eq!(foc, w2, "focus at state {}", s.raw);
    }
}

/// Highlights (SK-4 of glitu, D-090): their own attribute, the appearance effect kept, their steps
/// counted, on any element (a line, an arrow, a card without `#id`).
#[test]
fn highlights_have_their_own_attribute_and_count_as_steps() {
    let src = "diagram\n  card#a A @1:up @3:halo\n    | line @4-5:chase\n    to(b) @2:magnify\n  card B @6:halo @7:magnify\n  card#b C\n";
    let (d, warnings) = skemo_engine::from_source(src, &Default::default()).unwrap();
    assert!(
        warnings.is_empty() && d.anim.warnings.is_empty(),
        "{warnings:?} {:?}",
        d.anim.warnings
    );
    let svg = skemo_engine::svg::render(&d);
    assert!(
        svg.contains(r#"data-id="a" data-step="1" data-fx="up" data-highlight="halo:3""#),
        "{svg}"
    );
    assert!(svg.contains(r#"data-highlight="chase:4-5""#));
    assert!(svg.contains(r#"data-highlight="magnify:2""#));
    assert!(svg.contains(r#"data-highlight="halo:6 magnify:7""#));
    let states: Vec<&str> = d.anim.states.iter().map(|s| s.raw.as_str()).collect();
    assert_eq!(states, ["0", "1", "2", "3", "4", "5", "6", "7"]);
    let html = skemo_engine::html::render(&d, &Default::default());
    assert!(html.contains(r#"data-highlight="halo:3""#));
}

#[test]
fn a_highlight_outside_the_visibility_is_a_warning() {
    let doc = skemo_lang::parse("diagram\n  card#a A @3-4 @6:halo\n  card#b B @1\n");
    assert!(!doc.has_errors());
    let a = resolve(&doc.model);
    assert!(
        a.warnings
            .iter()
            .any(|w| w.code == "highlight-outside" && w.element == "a"),
        "{:?}",
        a.warnings
    );
}
