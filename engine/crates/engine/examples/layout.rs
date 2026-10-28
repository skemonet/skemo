//! `cargo run --example layout -- file.pug [custom.css…]`: layout.json on stdout.
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let src = std::fs::read_to_string(&args[0]).unwrap();
    let css = args[1..]
        .iter()
        .map(|c| std::fs::read_to_string(c).unwrap())
        .collect();
    match skemo_engine::from_source(
        &src,
        &skemo_engine::Options {
            css,
            theme: None,
            base_theme: None,
            high_contrast: false,
            transparent: false,
            ..Default::default()
        },
    ) {
        Ok((d, _)) => {
            for w in &d.warnings {
                eprintln!("avertissement : {w}");
            }
            println!(
                "{}",
                serde_json::to_string_pretty(&d.layout_json()).unwrap()
            );
        }
        Err(e) => eprintln!("{e}"),
    }
}
