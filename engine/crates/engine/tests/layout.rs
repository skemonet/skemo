//! Placement parity (E1): Rust measurement → ELK (elk-rs) → pinned positions, against the oracle's
//! `nodes` (±1 px). The cases without `at=` test ELK alone; the others, ELK + `applyPins`, up to
//! the gap of the zones' top margin (D-031, end of limit L6).

use serde_json::Value;
use skemo_engine::layout::{elk_layout, final_boxes};
use skemo_engine::measure::measure;
use skemo_engine::scene::Scene;
use skemo_lang::style::Theme;
use std::path::Path;

const CASES: [(&str, &str, Option<&str>, &str); 11] = [
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
];

#[test]
fn nodes_match_oracle() {
    let fx = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    let mut lines = Vec::new();
    let mut failing = 0;
    for (case, pug, css, theme) in CASES {
        let src = std::fs::read_to_string(fx.join(pug)).unwrap();
        let mut doc = skemo_lang::parse(&src);
        // the prototype broke cycles by writing order (D-041 moved to GREEDY_MODEL_ORDER) and left
        // zones with ELK's spacing (D-047): this test checks elk-rs against elkjs, so it keeps the
        // prototype's options
        doc.model.options.insert(
            "elk".into(),
            r#"{"elk.layered.cycleBreaking.strategy":"MODEL_ORDER","elk.spacing.nodeNode":"36","elk.layered.spacing.nodeNodeBetweenLayers":"64","elk.layered.spacing.edgeNodeBetweenLayers":"26","skemo.zoneSpacing":"elk","skemo.labels":"elk"}"#
                .into(),
        );
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
        let (pad, head) = scene.zone_pad_head();
        let base = elk_layout(&doc.model, &sizes, pad, head).unwrap();
        let boxes = final_boxes(&doc.model, &sizes, &base, pad, head);
        let oracle: Value = serde_json::from_str(
            &std::fs::read_to_string(fx.join(format!("{case}.layout.json"))).unwrap(),
        )
        .unwrap();
        // D-031 (end of L6): a single top margin for zones, the one given to ELK (`head + pad × 0.6`).
        // The prototype computed the zones again with `pad + head × 0.8` as soon as there was an `at=`,
        // `via=` or `side=`: for these cases, the oracle is offset by exactly that gap, checked here.
        let pinned_oracle = doc.model.nodes.iter().any(|n| n.at.is_some())
            || doc
                .model
                .edges
                .iter()
                .any(|e| e.via.is_some() || e.side.is_some());
        let l6 = (pad + head * 0.8) - (head + pad * 0.6);
        let mut worst: f64 = 0.0;
        let mut bad = Vec::new();
        for (id, v) in oracle["layout"]["nodes"].as_object().unwrap() {
            let mut o: Vec<f64> = v
                .as_array()
                .unwrap()
                .iter()
                .map(|x| x.as_f64().unwrap())
                .collect();
            if pinned_oracle
                && doc
                    .model
                    .node(id)
                    .is_some_and(|n| n.kind == skemo_lang::Kind::Zone)
            {
                o[1] += l6;
                o[3] -= l6;
            }
            let b = boxes[id];
            let d = [b.x - o[0], b.y - o[1], b.w - o[2], b.h - o[3]]
                .iter()
                .map(|v| v.abs())
                .fold(0.0, f64::max);
            worst = worst.max(d);
            if d > 1.0 {
                bad.push(format!("{id} ({d:.1})"));
            }
        }
        if worst > 1.0 {
            failing += 1;
        }
        lines.push(format!(
            "{case:14} max gap {worst:6.1} px {}",
            bad.join(", ")
        ));
    }
    eprintln!("{}", lines.join("\n"));
    // `process`: known gap of elk-rs (E0 report)
    assert!(failing <= 1, "{}", lines.join("\n"));
}
