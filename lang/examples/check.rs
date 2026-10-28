//! Checks diagram files: `cargo run --example check -- file.skemo… [--json]`.
//! Exit code 1 if there is at least one error.

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let json = args.iter().any(|a| a == "--json");
    let mut failed = false;
    for f in args.iter().filter(|a| !a.starts_with("--")) {
        let src = match std::fs::read_to_string(f) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("{f} : lecture impossible ({e})");
                failed = true;
                continue;
            }
        };
        let doc = skemo_lang::parse(&src);
        for d in &doc.diagnostics {
            eprint!("{}", skemo_lang::diag::render(&src, f, d));
        }
        failed |= doc.has_errors();
        if json {
            println!("{}", doc.model.to_json());
        }
    }
    std::process::exit(i32::from(failed));
}
