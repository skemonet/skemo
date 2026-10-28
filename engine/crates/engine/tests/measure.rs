//! Measurement parity (E-MEAS-4): sizes of the cards against the oracle's `layout.sizes` (±2 px).

use serde_json::Value;
use skemo_engine::measure::measure;
use skemo_engine::scene::Scene;
use skemo_lang::style::Theme;
use std::path::Path;

/// (oracle case, source, custom CSS, theme)
pub const CASES: [(&str, &str, Option<&str>, &str); 15] = [
    ("auto-elk", "reseau.pug", None, "light"),
    ("manuel-avoid", "reseau-manuel.pug", None, "light"),
    ("manuel-via", "reseau-manuel-via.pug", None, "light"),
    ("deux-pins", "reseau-2pins.pug", None, "light"),
    ("conflit", "reseau-conflit.pug", None, "light"),
    ("process", "process.pug", Some("process.css"), "light"),
    ("design", "design.pug", Some("design.css"), "light"),
    ("design-sombre", "design.pug", Some("design.css"), "dark"),
    ("design-print", "design.pug", Some("design.css"), "print"),
    ("entete", "reseau-entete.pug", None, "light"),
    ("entete-sombre", "reseau-entete.pug", None, "dark"),
    ("auto-graphviz", "reseau.pug", None, "light"),
    ("auto-dagre", "reseau.pug", None, "light"),
    ("manuel-neato", "reseau-manuel.pug", None, "light"),
    ("manuel-straight", "reseau-manuel.pug", None, "light"),
];

#[test]
fn sizes_match_oracle() {
    let fx = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    let mut report = Vec::new();
    let mut worst: f64 = 0.0;
    let mut n = 0;
    for (case, pug, css, theme) in CASES {
        let src = std::fs::read_to_string(fx.join(pug)).unwrap();
        let doc = skemo_lang::parse(&src);
        let css: Vec<String> = css
            .into_iter()
            .map(|c| std::fs::read_to_string(fx.join(c)).unwrap())
            .collect();
        let scene = Scene::build_with_base(
            &doc.model,
            skemo_lang::style::REFERENCE_THEME,
            &css,
            Theme::parse(theme).unwrap(),
        );
        let sizes = measure(&scene, &doc.model);
        let oracle: Value = serde_json::from_str(
            &std::fs::read_to_string(fx.join(format!("{case}.layout.json"))).unwrap(),
        )
        .unwrap();
        for (id, s) in oracle["layout"]["sizes"].as_object().unwrap() {
            let got = sizes.cards[id];
            let (w, h) = (s["w"].as_f64().unwrap(), s["h"].as_f64().unwrap());
            let d = (got.w - w).abs().max((got.h - h).abs());
            worst = worst.max(d);
            n += 1;
            if d > 0.0 {
                report.push(format!(
                    "{case}/{id} : obtenu {}×{}, oracle {w}×{h}",
                    got.w, got.h
                ));
            }
        }
    }
    eprintln!("{n} cards, max gap {worst} px\n{}", report.join("\n"));
    assert!(worst <= 2.0, "gap > 2 px:\n{}", report.join("\n"));
}
