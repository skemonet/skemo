//! Frieze mode (F-TL-8): sizes and positions against the oracle `frise.layout.json` (±1 px).

use serde_json::Value;
use std::path::Path;

fn check(case: &str, theme: skemo_lang::style::Theme) -> Vec<String> {
    let fx = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    let src = std::fs::read_to_string(fx.join("frise.pug")).unwrap();
    let css = vec![std::fs::read_to_string(fx.join("frise.css")).unwrap()];
    let (d, _) = skemo_engine::from_source(
        &src,
        &skemo_engine::Options {
            css,
            theme: Some(theme),
            // the prototype's theme: the oracle predates D-048 (bars, dates, gaps)
            base_theme: Some(skemo_lang::style::REFERENCE_THEME.to_string()),
            high_contrast: false,
            transparent: false,
            ..Default::default()
        },
    )
    .unwrap();
    let oracle: Value = serde_json::from_str(
        &std::fs::read_to_string(fx.join(format!("{case}.layout.json"))).unwrap(),
    )
    .unwrap();
    let mut bad = Vec::new();
    for (id, s) in oracle["layout"]["sizes"].as_object().unwrap() {
        let g = d.sizes.cards[id];
        let (w, h) = (s["w"].as_f64().unwrap(), s["h"].as_f64().unwrap());
        if (g.w - w).abs() > 1.0 || (g.h - h).abs() > 1.0 {
            bad.push(format!(
                "{case} taille {id} : {}×{} au lieu de {w}×{h}",
                g.w, g.h
            ));
        }
    }
    for (id, v) in oracle["layout"]["nodes"].as_object().unwrap() {
        let o: Vec<f64> = v
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_f64().unwrap())
            .collect();
        let Some(b) = d.boxes.get(id) else {
            bad.push(format!("{case} {id} absent"));
            continue;
        };
        let e = [b.x - o[0], b.y - o[1], b.w - o[2], b.h - o[3]]
            .iter()
            .map(|x| x.abs())
            .fold(0.0, f64::max);
        if e > 1.0 {
            bad.push(format!(
                "{case} {id} : [{:.1}, {:.1}, {:.1}, {:.1}] au lieu de {o:?}",
                b.x, b.y, b.w, b.h
            ));
        }
    }
    bad
}

#[test]
fn frise_parity() {
    let mut bad = check("frise", skemo_lang::style::Theme::Light);
    bad.extend(check("frise-print", skemo_lang::style::Theme::Print));
    assert!(bad.is_empty(), "\n{}", bad.join("\n"));
}

#[test]
fn frieze_and_its_old_name_frise_draw_the_same() {
    let src = |l: &str| {
        format!(
            "diagram(layout=\"{l}\" from=\"-800\" to=\"600\")\n  card#a(date=\"-44\") Caesar\n  card#b(start=\"-27\" end=\"476\") Empire\n"
        )
    };
    let build = |l: &str| {
        skemo_engine::from_source(&src(l), &skemo_engine::Options::default())
            .unwrap()
            .0
    };
    let (a, b) = (build("frieze"), build("frise"));
    assert!(a.frise.is_some());
    assert_eq!(a.boxes, b.boxes);
    // the same drawing; only the ids of the filters, from a hash of the source, differ
    let plain = |d: &skemo_engine::Diagram| {
        let svg = skemo_engine::svg::render(d);
        let at = svg.find("id=\"sk").expect("a filter") + 4;
        svg.replace(&svg[at..at + 10], "sk")
    };
    assert!(plain(&a) == plain(&b), "the two names draw differently");
}
