//! Timeline mode (F-TL-8): sizes and positions against the oracle `timeline.layout.json` (±1 px).

use serde_json::Value;
use std::path::Path;

fn check(case: &str, theme: skemo_lang::style::Theme) -> Vec<String> {
    let fx = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    let src = std::fs::read_to_string(fx.join("timeline.pug")).unwrap();
    let css = vec![std::fs::read_to_string(fx.join("timeline.css")).unwrap()];
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
fn timeline_parity() {
    let mut bad = check("timeline", skemo_lang::style::Theme::Light);
    bad.extend(check("timeline-sombre", skemo_lang::style::Theme::Dark));
    assert!(bad.is_empty(), "\n{}", bad.join("\n"));
}

/// A dependency to a bar just below that starts where the source's ends leaves a little before
/// the end and comes into the bar from the left: a line straight down landed on the first letter
/// of the target's title.
#[test]
fn a_dependency_down_to_a_bar_starting_there_misses_its_title() {
    let src =
        "diagram(layout=\"timeline\" from=\"2027-01\" to=\"2027-06\" scale=\"month\" lang=\"en\")
  zone#team.lane Team
    card#build.work(start=\"2027-01-15\" end=\"2027-03\") Build
      to(pilot)
    card#pilot.work(start=\"2027-04\" end=\"2027-05-15\") Pilot
";
    let (d, _) = skemo_engine::from_source(src, &skemo_engine::Options::default()).unwrap();
    let r = &d.routes["build->pilot"];
    let pilot = d.boxes["pilot"];
    let last = r[r.len() - 1];
    assert!((last.0 - pilot.x).abs() < 0.5, "{r:?} {pilot:?}");
    assert!(
        r[..r.len() - 1].iter().all(|p| p.0 < pilot.x - 5.0),
        "{r:?} {pilot:?}"
    );
}
