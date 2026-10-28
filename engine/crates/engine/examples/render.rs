//! `cargo run --example render -- fichier.pug sortie.svg [--theme dark] [perso.css…]`
fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut theme = None;
    if let Some(i) = args.iter().position(|a| a == "--theme") {
        theme = skemo_lang::style::Theme::parse(&args[i + 1]);
        args.drain(i..i + 2);
    }
    let src = std::fs::read_to_string(&args[0]).unwrap();
    let css = args[2..]
        .iter()
        .map(|c| std::fs::read_to_string(c).unwrap())
        .collect();
    let (d, _) = skemo_engine::from_source(
        &src,
        &skemo_engine::Options {
            css,
            theme,
            base_theme: None,
            high_contrast: false,
            transparent: false,
            ..Default::default()
        },
    )
    .unwrap_or_else(|e| panic!("{e}"));
    for w in &d.warnings {
        eprintln!("avertissement : {w}");
    }
    if args[1].ends_with(".html") {
        std::fs::write(
            &args[1],
            skemo_engine::html::render(&d, &skemo_engine::html::HtmlOptions::default()),
        )
        .unwrap();
    } else if args[1].ends_with(".png") {
        std::fs::write(&args[1], skemo_engine::png::render(&d, 2.0).unwrap()).unwrap();
    } else {
        std::fs::write(&args[1], skemo_engine::svg::render(&d)).unwrap();
    }
}
