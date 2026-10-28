//! Reference outputs: SVG (3 themes), SVG with enforced contrast, HTML, PNG and contact sheet of
//! the steps of each example, compared with `tests/golden.txt` (64-bit FNV-1a fingerprint of each
//! output).
//!
//! Safety net of refactorings: nothing should change unless decided. After a wanted change:
//! `UPDATE_GOLDEN=1 cargo test --release --test golden`, then read the diff of `golden.txt` (which
//! outputs moved) before committing it.

use skemo_engine::{Options, from_source, html, png, svg};
use skemo_lang::style::Theme;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

fn fnv1a(data: &[u8]) -> u64 {
    data.iter().fold(0xcbf2_9ce4_8422_2325, |h, &b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    })
}

fn fixtures() -> Vec<(String, String, Vec<String>)> {
    // the prototype's oracle (`.pug`, + `.css` of the same name) and our own examples
    // (`tests/data/*.skemo`)
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut found = Vec::new();
    for (dir, ext) in [("../../fixtures", "pug"), ("tests/data", "skemo")] {
        let dir = root.join(dir);
        for e in std::fs::read_dir(&dir).unwrap() {
            let p = e.unwrap().path();
            if p.extension().is_some_and(|x| x == ext) {
                let name = p.file_stem().unwrap().to_str().unwrap().to_string();
                let src = std::fs::read_to_string(&p).unwrap();
                let css = std::fs::read_to_string(dir.join(format!("{name}.css")))
                    .into_iter()
                    .collect();
                found.push((name, src, css));
            }
        }
    }
    found.sort_by(|a, b| a.0.cmp(&b.0));
    found
}

fn outputs() -> BTreeMap<String, u64> {
    let mut out = BTreeMap::new();
    for (name, src, css) in fixtures() {
        for theme in [Theme::Light, Theme::Dark, Theme::Print] {
            let opts = Options {
                css: css.clone(),
                theme: Some(theme),
                ..Options::default()
            };
            let (d, _) = from_source(&src, &opts).unwrap();
            let svg = svg::render(&d);
            out.insert(
                format!("{name}.{}.svg", theme.name()),
                fnv1a(svg.as_bytes()),
            );
            if theme == Theme::Light {
                let raster = svg::render_for_raster(&d);
                let png = png::svg_to_png(&raster, 2.0).unwrap();
                out.insert(format!("{name}.light.png"), fnv1a(&png));
                let html = html::render(&d, &html::HtmlOptions::default());
                out.insert(format!("{name}.light.html"), fnv1a(html.as_bytes()));
            }
        }
        let opts = Options {
            css,
            high_contrast: true,
            ..Options::default()
        };
        let (d, _) = from_source(&src, &opts).unwrap();
        out.insert(
            format!("{name}.high-contrast.svg"),
            fnv1a(svg::render(&d).as_bytes()),
        );
    }
    out
}

#[test]
fn outputs_match_golden() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden.txt");
    let now = outputs();
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        let mut text = String::new();
        for (k, v) in &now {
            let _ = writeln!(text, "{k} {v:016x}");
        }
        std::fs::write(&path, text).unwrap();
        return;
    }
    let expected: BTreeMap<String, String> = std::fs::read_to_string(&path)
        .expect("tests/golden.txt manquant : UPDATE_GOLDEN=1 cargo test --release --test golden")
        .lines()
        .filter_map(|l| l.split_once(' '))
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    let mut diffs = Vec::new();
    for (k, v) in &now {
        match expected.get(k) {
            Some(e) if *e == format!("{v:016x}") => {}
            Some(_) => diffs.push(format!("changed: {k}")),
            None => diffs.push(format!("nouveau : {k}")),
        }
    }
    diffs.extend(
        expected
            .keys()
            .filter(|k| !now.contains_key(*k))
            .map(|k| format!("disparu : {k}")),
    );
    assert!(
        diffs.is_empty(),
        "{} output(s) differ from the reference (UPDATE_GOLDEN=1 if wanted):\n{}",
        diffs.len(),
        diffs.join("\n")
    );
}
