//! The documentation site (`site/`) only shows examples that parse cleanly: every
//! `site/examples/*.skemo` gives no diagnostic at all, neither error nor warning.

use std::path::Path;

#[test]
fn documentation_examples_have_no_diagnostic() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("site/examples");
    let mut checked = 0;
    for entry in std::fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|e| e != "skemo") {
            continue;
        }
        let src = std::fs::read_to_string(&path).unwrap();
        let doc = skemo_lang::parse(&src);
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let report: String = doc
            .diagnostics
            .iter()
            .map(|d| skemo_lang::diag::render(&src, &name, d))
            .collect();
        assert!(report.is_empty(), "{name}:\n{report}");
        checked += 1;
    }
    assert!(
        checked >= 20,
        "only {checked} examples in {}",
        dir.display()
    );
}
