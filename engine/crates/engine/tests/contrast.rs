//! E-STYLE-7 / F-STYLE-15: contrast of every text, on every test set, in the 3 themes (the
//! "print converted to grey" test is covered: the ratio depends on luminance only).

use skemo_lang::style::Theme;
use std::path::Path;

const CASES: [(&str, Option<&str>); 8] = [
    ("reseau.pug", None),
    ("reseau-manuel.pug", None),
    ("reseau-manuel-via.pug", None),
    ("reseau-2pins.pug", None),
    ("reseau-entete.pug", None),
    ("process.pug", Some("process.css")),
    ("design.pug", Some("design.css")),
    ("animation.pug", None),
];

#[test]
fn contrast_all_themes() {
    let fx = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    let mut report = Vec::new();
    for (pug, css) in CASES {
        let src = std::fs::read_to_string(fx.join(pug)).unwrap();
        let css: Vec<String> = css
            .into_iter()
            .map(|c| std::fs::read_to_string(fx.join(c)).unwrap())
            .collect();
        for theme in [Theme::Light, Theme::Dark, Theme::Print] {
            let (d, _) = skemo_engine::from_source(
                &src,
                &skemo_engine::Options {
                    css: css.clone(),
                    theme: Some(theme),
                    base_theme: None,
                    high_contrast: false,
                    transparent: false,
                    ..Default::default()
                },
            )
            .unwrap();
            for i in skemo_engine::contrast::check(&d) {
                report.push(format!(
                    "{pug} [{}] {} ({}) : {} < {} ({} on {})",
                    theme.name(),
                    i.element,
                    i.what,
                    i.ratio,
                    i.required,
                    i.text,
                    i.background
                ));
            }
        }
    }
    // Known defects (the prototype's theme, example CSS), for the user to decide on: see
    // tests/contrast.known.txt. Any new regression fails the test.
    let known_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/contrast.known.txt");
    let known: Vec<String> = std::fs::read_to_string(&known_path)
        .unwrap_or_default()
        .lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .map(String::from)
        .collect();
    let new: Vec<&String> = report.iter().filter(|r| !known.contains(r)).collect();
    let fixed: Vec<&String> = known.iter().filter(|k| !report.contains(k)).collect();
    if !fixed.is_empty() {
        eprintln!(
            "defects fixed (to remove from contrast.known.txt):\n{}",
            fixed
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
    assert!(
        new.is_empty(),
        "{} new contrast defect(s):\n{}",
        new.len(),
        new.iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// Enforced contrast: no defect left, and nothing but colours changes.
#[test]
fn high_contrast_fixes_everything_and_keeps_layout() {
    let fx = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    for (pug, css) in CASES {
        let src = std::fs::read_to_string(fx.join(pug)).unwrap();
        let css: Vec<String> = css
            .into_iter()
            .map(|c| std::fs::read_to_string(fx.join(c)).unwrap())
            .collect();
        for theme in [Theme::Light, Theme::Dark, Theme::Print] {
            let build = |high_contrast| {
                skemo_engine::from_source(
                    &src,
                    &skemo_engine::Options {
                        css: css.clone(),
                        theme: Some(theme),
                        base_theme: None,
                        high_contrast,
                        transparent: false,
                        ..Default::default()
                    },
                )
                .unwrap()
                .0
            };
            let (std, high) = (build(false), build(true));
            assert_eq!(
                std.layout_json(),
                high.layout_json(),
                "{pug} [{}]: the placement changes",
                theme.name()
            );
            let left = skemo_engine::contrast::check(&high);
            assert!(
                left.is_empty(),
                "{pug} [{}]: defects left: {:?}",
                theme.name(),
                left.iter().map(|i| &i.element).collect::<Vec<_>>()
            );
        }
    }
}
