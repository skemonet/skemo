//! Image files as icons (D-046): handed over in `Options::images`, drawn as images in their box,
//! never inlined, carried in the embedded source.

use skemo_engine::{Options, from_source, html, png, source, svg};

const LOGO: &str = "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 24 24\"><circle cx=\"12\" cy=\"12\" r=\"10\" fill=\"#c00\"/></svg>";
const TRAP: &str = "<svg xmlns=\"http://www.w3.org/2000/svg\" xmlns:xlink=\"http://www.w3.org/1999/xlink\" viewBox=\"0 0 24 24\"><script>alert(1)</script><image xlink:href=\"/etc/hostname\" width=\"24\" height=\"24\"/></svg>";

fn with_images(src: &str, files: &[(&str, &str)]) -> skemo_engine::Diagram {
    let opts = Options {
        images: files
            .iter()
            .map(|(n, c)| ((*n).to_string(), c.as_bytes().to_vec()))
            .collect(),
        ..Options::default()
    };
    from_source(src, &opts).unwrap().0
}

#[test]
fn an_image_file_is_drawn_as_an_image_in_the_icon_box() {
    let d = with_images(
        "card#a(icon=\"img/logo.svg\") A\n",
        &[("img/logo.svg", LOGO)],
    );
    assert!(d.warnings.is_empty(), "{:?}", d.warnings);
    let out = svg::render(&d);
    assert!(out.contains("<image class=\"icon\""), "{out}");
    assert!(out.contains("data:image/svg+xml;base64,"));
    assert!(!out.contains("<circle cx=\"12\""), "never inlined");
    let page = html::render(&d, &Default::default());
    assert!(page.contains("<image href=\"data:image/svg+xml;base64,"));
    // same room as a built-in icon: the layout does not depend on where the icon comes from
    let builtin = with_images("card#a(icon=\"database\") A\n", &[]);
    assert_eq!(d.boxes["a"], builtin.boxes["a"]);
    assert!(png::svg_to_png(&svg::render_for_raster(&d), 1.0).is_ok());
}

#[test]
fn a_missing_or_refused_image_draws_nothing_and_warns() {
    let d = with_images("card#a(icon=\"img/none.svg\") A\n", &[]);
    assert!(
        d.warnings.iter().any(|w| w.contains("img/none.svg")),
        "{:?}",
        d.warnings
    );
    let plain = with_images("card#a A\n", &[]);
    assert_eq!(d.boxes["a"], plain.boxes["a"], "no room reserved");
    let d = with_images("card#a(icon=\"../x.svg\") A\n", &[("../x.svg", LOGO)]);
    assert!(
        d.warnings.iter().any(|w| w.contains("../x.svg")),
        "{:?}",
        d.warnings
    );
    let d = with_images("card#a(icon=\"x.png\") A\n", &[("x.png", "not a png")]);
    assert!(
        d.warnings.iter().any(|w| w.contains("x.png")),
        "{:?}",
        d.warnings
    );
}

#[test]
fn an_svg_image_resolves_no_file_when_rasterised() {
    let d = with_images("card#a(icon=\"t.svg\") A\n", &[("t.svg", TRAP)]);
    // renders (the reference is simply ignored), and the script never reaches the output as markup
    assert!(png::svg_to_png(&svg::render_for_raster(&d), 1.0).is_ok());
    assert!(!svg::render(&d).contains("<script>alert"));
}

#[test]
fn the_embedded_source_carries_the_images() {
    let mut b = source::Bundle::new("card#a(icon=\"img/logo.svg\") A\n");
    b.images
        .push(source::NamedBytes::new("img/logo.svg", LOGO.as_bytes()));
    let svg = source::embed_text("<svg xmlns=\"http://www.w3.org/2000/svg\"></svg>", &b);
    let back = source::extract(svg.as_bytes()).unwrap();
    assert_eq!(back.images[0].name, "img/logo.svg");
    assert_eq!(back.images[0].bytes().unwrap(), LOGO.as_bytes());
}
