//! Parity with the prototype: each `fixtures/*.pug` gives the model of `fixtures/*.model.json`
//! (structural equality on the prototype's fields; the fields the Rust version adds are ignored).

use serde_json::Value;
use std::path::Path;

/// `got` holds everything `want` holds (extra keys are tolerated).
fn covers(got: &Value, want: &Value, path: &str, errs: &mut Vec<String>) {
    match (got, want) {
        (Value::Object(g), Value::Object(w)) => {
            for (k, wv) in w {
                match g.get(k) {
                    Some(gv) => covers(gv, wv, &format!("{path}.{k}"), errs),
                    None => errs.push(format!("{path}.{k} manquant")),
                }
            }
        }
        (Value::Array(g), Value::Array(w)) if g.len() == w.len() => {
            for (i, (gv, wv)) in g.iter().zip(w).enumerate() {
                covers(gv, wv, &format!("{path}[{i}]"), errs);
            }
        }
        (Value::Number(g), Value::Number(w)) if g.as_f64() == w.as_f64() => {}
        _ if got == want => {}
        _ => errs.push(format!("{path} : obtenu {got}, attendu {want}")),
    }
}

#[test]
fn model_parity() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures");
    let mut n = 0;
    for e in std::fs::read_dir(&dir).unwrap() {
        let p = e.unwrap().path();
        if p.extension().and_then(|x| x.to_str()) != Some("pug") {
            continue;
        }
        let expected = p.with_extension("model.json");
        let src = std::fs::read_to_string(&p).unwrap();
        let doc = skemo_lang::parse(&src);
        let errs: Vec<_> = doc
            .errors()
            .map(|d| skemo_lang::diag::render(&src, p.to_str().unwrap(), d))
            .collect();
        assert!(errs.is_empty(), "{}", errs.join("\n"));
        if !expected.exists() {
            continue;
        }
        let want: Value =
            serde_json::from_str(&std::fs::read_to_string(&expected).unwrap()).unwrap();
        let got = serde_json::to_value(&doc.model).unwrap();
        let mut diffs = Vec::new();
        covers(
            &got,
            &want,
            p.file_name().unwrap().to_str().unwrap(),
            &mut diffs,
        );
        assert!(diffs.is_empty(), "{}", diffs.join("\n"));
        n += 1;
    }
    assert!(n >= 10, "{n} jeux seulement");
}

#[test]
fn animation_texts() {
    let src = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/animation.pug"),
    )
    .unwrap();
    let doc = skemo_lang::parse(&src);
    assert!(!doc.has_errors());
    let m = &doc.model;
    assert_eq!(m.node("seg").unwrap().title, "seg-k8s-01");
    assert_eq!(m.node("seg").unwrap().items[0], "10.1.20.0/28");
    assert_eq!(m.node("dns").unwrap().title, "DNS 10.1.0.53");
    assert_eq!(m.node("uplinks").unwrap().title, "Uplinks / VLAN");
    assert_eq!(m.node("naos").unwrap().title, "naos-04");
    assert_eq!(
        m.node("t0").unwrap().items[1],
        "voir @2 ou admin@example.com"
    );
    let seg = m.node("seg").unwrap().anim.clone().unwrap();
    assert_eq!(seg.start.unwrap().raw, "1");
    assert_eq!(seg.focus.len(), 1);
    assert_eq!(
        m.node("ports")
            .unwrap()
            .anim
            .as_ref()
            .unwrap()
            .start
            .as_ref()
            .unwrap()
            .raw,
        "2.5"
    );
}

/// L3: edit round trips on every example (moving then releasing each free card gives the source
/// back, and the diff touches one line only); `fmt` idempotent and loses no meaning.
#[test]
fn edit_round_trips_and_fmt() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures");
    for e in std::fs::read_dir(&dir).unwrap() {
        let p = e.unwrap().path();
        if p.extension().and_then(|x| x.to_str()) != Some("pug") {
            continue;
        }
        let src = std::fs::read_to_string(&p).unwrap();
        let doc = skemo_lang::parse(&src);
        for n in doc.model.nodes.iter().filter(|n| {
            n.kind == skemo_lang::Kind::Card && n.at.is_none() && !n.id.starts_with("card")
        }) {
            let moved = skemo_lang::edit::move_card(&src, &n.id, 12.0, 34.0).unwrap();
            let changed = src
                .lines()
                .zip(moved.lines())
                .filter(|(a, b)| a != b)
                .count();
            assert_eq!(changed, 1, "{}: {}", p.display(), n.id);
            assert_eq!(
                skemo_lang::parse(&moved).model.node(&n.id).unwrap().at,
                Some([12.0, 34.0])
            );
            assert_eq!(
                skemo_lang::edit::release_card(&moved, &n.id).unwrap(),
                src,
                "{}: {}",
                p.display(),
                n.id
            );
        }
        let f = skemo_lang::fmt::format(&src).unwrap();
        assert_eq!(
            skemo_lang::fmt::format(&f).unwrap(),
            f,
            "fmt non idempotent : {}",
            p.display()
        );
        let (a, b) = (skemo_lang::parse(&src).model, skemo_lang::parse(&f).model);
        assert_eq!(a.nodes.len(), b.nodes.len());
        assert_eq!(a.edges.len(), b.edges.len());
        for (x, y) in a.nodes.iter().zip(&b.nodes) {
            assert_eq!(
                (&x.id, &x.title, &x.items, x.at, &x.classes),
                (&y.id, &y.title, &y.items, y.at, &y.classes),
                "{}",
                p.display()
            );
            assert_eq!(
                x.anim
                    .as_ref()
                    .and_then(|a| a.start.as_ref())
                    .map(|s| s.value),
                y.anim
                    .as_ref()
                    .and_then(|a| a.start.as_ref())
                    .map(|s| s.value)
            );
        }
        for (x, y) in a.edges.iter().zip(&b.edges) {
            assert_eq!(
                (&x.id, &x.label, &x.via, &x.side, &x.classes),
                (&y.id, &y.label, &y.via, &y.side, &y.classes),
                "{}",
                p.display()
            );
        }
    }
}

#[test]
fn join_trunk_is_kept_in_the_model_and_invisible_when_absent() {
    let doc = skemo_lang::parse("card#a Root\n  to(b c join=\"trunk\")\ncard#b One\ncard#c Two\n");
    assert!(!doc.has_errors());
    assert!(
        doc.model
            .edges
            .iter()
            .all(|e| e.join.as_deref() == Some("trunk"))
    );
    let plain = skemo_lang::parse("card#a Root\n  to(b)\ncard#b One\n");
    let json = serde_json::to_string(&plain.model).unwrap();
    assert!(
        !json.contains("join"),
        "le champ ne doit pas apparaître quand il est absent"
    );
}
