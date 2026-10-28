//! `cargo run --release --example reorder -- in.skemo style.css [out.skemo]`: the order search
//! for importers (D-065) on a source, its scores and time; `--score` only scores the source.
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let src = std::fs::read_to_string(&args[0]).unwrap();
    let opts = skemo_engine::Options {
        css: vec![std::fs::read_to_string(&args[1]).unwrap()],
        ..Default::default()
    };
    if args.get(2).is_some_and(|a| a == "--score") {
        println!("{}", skemo_engine::reorder::evaluate(&src, &opts));
        return;
    }
    let t = std::time::Instant::now();
    let (out, before, after) = skemo_engine::reorder::improve(&src, &opts);
    println!("{before} -> {after} in {:.1} s", t.elapsed().as_secs_f64());
    if let Some(o) = args.get(2) {
        std::fs::write(o, out).unwrap();
    }
}
