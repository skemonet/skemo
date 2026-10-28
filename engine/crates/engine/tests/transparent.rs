//! "Transparent background" option: no background rectangle any more, transparent PNG corner;
//! without the option, the theme's background is opaque. Shadows stay semi-transparent.

use resvg::tiny_skia::Pixmap;
use skemo_engine::{Options, animated, from_source, png, svg};

const SRC: &str = "diagram(dir=\"down\")\n  card#a Un\n    to(b)\n  card#b Deux\n";

fn corner_alpha(transparent: bool) -> u8 {
    let (d, _) = from_source(
        SRC,
        &Options {
            transparent,
            ..Options::default()
        },
    )
    .unwrap();
    let pixmap = Pixmap::decode_png(&png::render(&d, 1.0).unwrap()).unwrap();
    pixmap.pixel(0, 0).unwrap().alpha()
}

#[test]
fn png_corner_is_transparent_only_on_request() {
    assert_eq!(corner_alpha(false), 255);
    assert_eq!(corner_alpha(true), 0);
}

#[test]
fn svg_has_no_stage_rect_when_transparent() {
    let build = |transparent| {
        from_source(
            SRC,
            &Options {
                transparent,
                ..Options::default()
            },
        )
        .unwrap()
        .0
    };
    assert!(svg::render(&build(false)).contains("class=\"stage\""));
    assert!(!svg::render(&build(true)).contains("class=\"stage\""));
}

#[test]
fn step_board_is_transparent_too() {
    let (d, _) = from_source(
        SRC,
        &Options {
            transparent: true,
            ..Options::default()
        },
    )
    .unwrap();
    let pixmap = Pixmap::decode_png(&animated::board(&d, 1.0).unwrap()).unwrap();
    assert_eq!(pixmap.pixel(0, 0).unwrap().alpha(), 0);
}
