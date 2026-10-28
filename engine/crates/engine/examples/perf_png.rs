//! Profile of the PNG rendering: `cargo run --release --example perf_png -- fixtures/reseau.pug`
use std::time::Instant;
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let src = std::fs::read_to_string(&a[0]).unwrap();
    let css = a[1..]
        .iter()
        .map(|c| std::fs::read_to_string(c).unwrap())
        .collect();
    let t = Instant::now();
    let (d, _) = skemo_engine::from_source(
        &src,
        &skemo_engine::Options {
            css,
            theme: None,
            base_theme: None,
            high_contrast: false,
            transparent: false,
            ..Default::default()
        },
    )
    .unwrap();
    println!("pipeline     {:6.1} ms", t.elapsed().as_secs_f64() * 1000.0);
    let t = Instant::now();
    let svg = skemo_engine::svg::render_for_raster(&d);
    println!("svg          {:6.1} ms", t.elapsed().as_secs_f64() * 1000.0);
    let t = Instant::now();
    let _ = skemo_engine::png::svg_to_png(
        "<svg xmlns='http://www.w3.org/2000/svg' width='1' height='1'/>",
        1.0,
    );
    println!("polices      {:6.1} ms", t.elapsed().as_secs_f64() * 1000.0);
    let t = Instant::now();
    let _ = skemo_engine::png::svg_to_png(&svg, 2.0).unwrap();
    println!("png (x2)     {:6.1} ms", t.elapsed().as_secs_f64() * 1000.0);
    let nofilter = svg
        .split("filter=\"url(")
        .map(std::string::ToString::to_string)
        .collect::<Vec<_>>();
    let stripped = if nofilter.len() > 1 {
        nofilter
            .iter()
            .enumerate()
            .map(|(i, s)| {
                if i == 0 {
                    s.clone()
                } else {
                    s[s.find('"').unwrap() + 1..].to_string()
                }
            })
            .collect::<String>()
    } else {
        svg.clone()
    };
    let t = Instant::now();
    let _ = skemo_engine::png::svg_to_png(&stripped, 2.0).unwrap();
    println!(
        "png no shadow {:4.1} ms",
        t.elapsed().as_secs_f64() * 1000.0
    );
    variants(&svg);
    // a single shadow layer (the most blurred)
    let one = svg
        .split("<feMergeNode in=\"s0\"/>")
        .collect::<Vec<_>>()
        .join("");
    let t = Instant::now();
    let _ = skemo_engine::png::svg_to_png(&one, 2.0).unwrap();
    println!(
        "no 1st shadow {:4.1} ms",
        t.elapsed().as_secs_f64() * 1000.0
    );
    println!(
        "{}",
        &svg[svg.find("<filter").unwrap_or(0)..][..600.min(svg.len())]
    );
}

#[allow(dead_code)]
fn variants(svg: &str) {
    let t = Instant::now();
    let v1 = svg.replace(
        "x=\"-20%\" y=\"-20%\" width=\"140%\" height=\"160%\"",
        "x=\"-10%\" y=\"-10%\" width=\"120%\" height=\"140%\"",
    );
    let _ = skemo_engine::png::svg_to_png(&v1, 2.0).unwrap();
    println!(
        "reduced region {:4.1} ms",
        t.elapsed().as_secs_f64() * 1000.0
    );
}
