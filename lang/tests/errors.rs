//! Diagnostics (F-PARSE-4, 5): one file per error in `fixtures/erreurs/`, rendered as a snapshot.
//! And robustness (F-PARSE-9): no panic on random or mangled sources.

use std::path::Path;

#[test]
fn diagnostics_snapshots() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/erreurs");
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    files.sort();
    for p in files {
        let name = p.file_stem().unwrap().to_str().unwrap().to_string();
        let src = std::fs::read_to_string(&p).unwrap();
        let doc = skemo_lang::parse(&src);
        let out: String = doc
            .diagnostics
            .iter()
            .map(|d| skemo_lang::diag::render(&src, &format!("{name}.skemo"), d))
            .collect();
        insta::assert_snapshot!(name, out);
    }
}

/// Deterministic pseudo-random generator (no dependency).
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

#[test]
fn no_panic_on_mutations() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures");
    // the prototype's files and the diagnostics' files (v0.8 lines, tables, emphasis)
    let seeds: Vec<String> = [dir.clone(), dir.join("erreurs")]
        .iter()
        .flat_map(|d| std::fs::read_dir(d).unwrap())
        .filter_map(|e| {
            let p = e.unwrap().path();
            matches!(
                p.extension().and_then(|x| x.to_str()),
                Some("pug" | "skemo")
            )
            .then(|| std::fs::read_to_string(p).unwrap())
        })
        .collect();
    let alphabet: Vec<char> = "abc#.()=\"' ,-@:\\|/\n\t  \u{e9}\u{1F600}0123456789toli*`^=|"
        .chars()
        .collect();
    let mut rng = Rng(0x5eed);
    let n: usize = std::env::var("LANG_FUZZ_ITERS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(20_000);
    for _ in 0..n {
        let mut s: Vec<char> = seeds[rng.below(seeds.len())].chars().collect();
        for _ in 0..1 + rng.below(8) {
            let i = rng.below(s.len() + 1);
            match rng.below(3) {
                0 if i < s.len() => {
                    s.remove(i);
                }
                1 => s.insert(i, alphabet[rng.below(alphabet.len())]),
                _ if i < s.len() => s[i] = alphabet[rng.below(alphabet.len())],
                _ => {}
            }
        }
        let src: String = s.into_iter().collect();
        let doc = skemo_lang::parse(&src);
        for d in &doc.diagnostics {
            let _ = skemo_lang::diag::render(&src, "x.pug", d);
        }
        for n in &doc.model.nodes {
            let _ = skemo_lang::table::blocks(&n.li);
            for l in &n.li {
                let _ = skemo_lang::inline::runs(&l.text);
            }
        }
        let _ = skemo_lang::fmt::format(&src);
    }
}
