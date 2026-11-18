//! The quality ledger of any set of diagrams (the layout method, phase 0), for corpora kept out of
//! the repository (private maps). The repository's own corpus is checked by `tests/ledger.rs`.
//!
//! ```sh
//! cargo run --profile local -p skemo-engine --example ledger -- <ledger file> [--update] [--items] [--css=FILE] <files or folders>…
//! ```
//!
//! Without `--update`: compares with the ledger file (the ratchet) and exits 1 if a count rose.
//! `--update` writes the ledger file. `--items` also prints what breaks each property. `--css=FILE`
//! (repeatable): a stylesheet applied to every diagram, before its own.

use skemo_engine::quality;
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let update = args.iter().any(|a| a == "--update");
    let items = args.iter().any(|a| a == "--items");
    let shared: Vec<String> = args
        .iter()
        .filter_map(|a| a.strip_prefix("--css="))
        .map(|f| std::fs::read_to_string(f).expect("reading --css"))
        .collect();
    let mut rest = args.iter().filter(|a| !a.starts_with("--"));
    let Some(ledger) = rest.next().map(PathBuf::from) else {
        eprintln!("usage: ledger <ledger file> [--update] [--items] <files or folders>…");
        std::process::exit(2);
    };
    let paths: Vec<PathBuf> = rest
        .map(PathBuf::from)
        .map(|p| p.canonicalize().unwrap_or(p))
        .collect();
    let root = common_root(&paths);
    let mut entries = quality::entries(&paths, &root);
    for e in &mut entries {
        let own = std::mem::take(&mut e.css);
        e.css = shared.iter().cloned().chain(own).collect();
    }
    if items {
        for e in &entries {
            let opts = skemo_engine::Options {
                css: e.css.clone(),
                ..Default::default()
            };
            if let Some(found) = quality::all_defects(&e.source, &opts) {
                for (p, v) in found.into_iter().filter(|(_, v)| !v.is_empty()) {
                    println!("{} {p}: {}", e.name, v.join(", "));
                }
            }
        }
    }
    let now = quality::count(&entries);
    if update {
        std::fs::write(&ledger, quality::format(&now)).expect("writing the ledger");
        print!("{}", quality::format(&now));
        return;
    }
    let old = quality::parse(&std::fs::read_to_string(&ledger).unwrap_or_default());
    let (worse, better) = quality::compare(&old, &now);
    for b in &better {
        println!("better  {b}");
    }
    for w in &worse {
        println!("WORSE   {w}");
    }
    print!(
        "{}",
        quality::format(&now)
            .lines()
            .last()
            .map(|l| format!("{l}\n"))
            .unwrap_or_default()
    );
    if !worse.is_empty() {
        std::process::exit(1);
    }
}

/// The deepest folder holding every path (names in the ledger are relative to it).
fn common_root(paths: &[PathBuf]) -> PathBuf {
    let abs: Vec<PathBuf> = paths
        .iter()
        .filter_map(|p| p.canonicalize().ok())
        .map(|p| {
            if p.is_dir() {
                p
            } else {
                p.parent().map(PathBuf::from).unwrap_or(p)
            }
        })
        .collect();
    let Some(first) = abs.first() else {
        return PathBuf::from(".");
    };
    let mut root = first.clone();
    while !abs.iter().all(|p| p.starts_with(&root)) {
        if !root.pop() {
            break;
        }
    }
    root
}
