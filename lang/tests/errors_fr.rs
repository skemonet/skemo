//! The same diagnostics in French (the language is chosen once for this process: the other tests
//! run in English). Each message therefore exists in both languages.

use skemo_lang::i18n::{Lang, set_language};
use std::path::Path;

#[test]
fn french_diagnostics_snapshots() {
    set_language(Lang::Fr);
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/erreurs");
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    files.sort();
    assert!(!files.is_empty());
    for p in files {
        let name = p.file_stem().unwrap().to_str().unwrap().to_string();
        let src = std::fs::read_to_string(&p).unwrap();
        let doc = skemo_lang::parse(&src);
        let out: String = doc
            .diagnostics
            .iter()
            .map(|d| skemo_lang::diag::render(&src, &format!("{name}.skemo"), d))
            .collect();
        insta::assert_snapshot!(format!("fr-{name}"), out);
    }
}
