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
