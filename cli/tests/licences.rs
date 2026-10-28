//! `src/crates.txt` (the crates `skemo licences` names) follows `Cargo.lock`: every crate it names
//! is in the lock at that version. After a dependency change: `python3 tools/gen-licences.py`.

use std::collections::BTreeSet;

#[test]
fn crate_list_follows_the_lock() {
    let root = env!("CARGO_MANIFEST_DIR");
    // the crate's own lock, or the enclosing workspace's
    let lock = std::fs::read_to_string(format!("{root}/Cargo.lock"))
        .or_else(|_| std::fs::read_to_string(format!("{root}/../Cargo.lock")))
        .unwrap();
    let mut locked = BTreeSet::new();
    let mut name = String::new();
    for l in lock.lines() {
        if let Some(n) = l.strip_prefix("name = ") {
            name = n.trim_matches('"').to_owned();
        } else if let Some(v) = l.strip_prefix("version = ") {
            locked.insert(format!("{name} {}", v.trim_matches('"')));
        }
    }
    let listed = std::fs::read_to_string(format!("{root}/src/crates.txt")).unwrap();
    let stale: Vec<&str> = listed
        .lines()
        .filter(|l| {
            let mut w = l.split(' ');
            let key = format!("{} {}", w.next().unwrap_or(""), w.next().unwrap_or(""));
            !locked.contains(&key)
        })
        .collect();
    assert!(
        stale.is_empty(),
        "not in Cargo.lock (run python3 tools/gen-licences.py): {stale:?}"
    );
}
