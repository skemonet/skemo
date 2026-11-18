//! The quality ledger of the repository's corpus (the layout method, phase 0 and its ratchet: meta
//! repository, `docs/plans/layout-method.md`). Every property of `quality::PROPERTIES` is counted
//! on every diagram; the test fails when a count rises above `tests/ledger.txt` or a diagram is
//! missing from it. When counts only fall (or diagrams are added), look at the diagrams that
//! changed, then `UPDATE_LEDGER=1 cargo test --profile local -p skemo-engine --test ledger`.

use skemo_engine::quality;
mod elk;
use std::path::{Path, PathBuf};

fn corpus() -> Vec<quality::Entry> {
    let engine = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = engine.join("../../..").canonicalize().unwrap();
    let mut paths: Vec<PathBuf> = vec![
        engine.join("../../fixtures"),
        engine.join("tests/data"),
        root.join("site/gallery"),
        root.join("cli/examples/site"),
    ];
    // the site's examples in English (their French copies draw the same)
    for dir in [root.join("lang/site/examples"), root.join("site/examples")] {
        let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "skemo"))
            .collect();
        files.sort();
        paths.extend(files);
    }
    let paths: Vec<PathBuf> = paths
        .into_iter()
        .map(|p| p.canonicalize().unwrap())
        .collect();
    // counted on ELK's drawing, the engine the ledger was frozen on (the slide engine, now the
    // default, is measured by the maintainers' judge)
    quality::entries(&paths, &root)
        .into_iter()
        .map(|e| quality::Entry {
            source: elk::graph(&e.source),
            ..e
        })
        .collect()
}

#[test]
fn no_count_rises_above_the_ledger() {
    let file = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ledger.txt");
    let now = quality::count(&corpus());
    if std::env::var_os("UPDATE_LEDGER").is_some() {
        std::fs::write(&file, quality::format(&now)).unwrap();
        return;
    }
    let ledger = quality::parse(&std::fs::read_to_string(&file).unwrap_or_default());
    let (worse, better) = quality::compare(&ledger, &now);
    assert!(
        worse.is_empty(),
        "counts above the ledger:\n{}\n(better: {})",
        worse.join("\n"),
        better.len()
    );
    assert!(
        better.is_empty(),
        "counts below the ledger: look at those diagrams, then UPDATE_LEDGER=1\n{}",
        better.join("\n")
    );
}
