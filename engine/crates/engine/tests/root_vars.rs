//! The variables the documentation sets on `:root` reach every card and zone (D-096): the theme
//! sets them on `:root` too, so that an author's stylesheet wins.

use skemo_engine::{Diagram, Options, png, svg};
mod elk;
use elk::from_source;

const SRC: &str = "diagram\n  zone#z Z\n    card#a(href=\"https://skemo.net\") A\n      | x\n";

fn with_css(css: &str) -> Diagram {
    let opts = Options {
        css: vec![css.to_string()],
        ..Options::default()
    };
    from_source(SRC, &opts).unwrap().0
}

fn pixels(d: &Diagram) -> Vec<u8> {
    png::render(d, 1.0).unwrap()
}

#[test]
fn zone_margins_on_root_change_the_layout() {
    let base = with_css("");
    let wide = with_css(":root { --zone-pad: 50px; }");
    assert!(
        wide.frame.w > base.frame.w + 30.0,
        "{} {}",
        wide.frame.w,
        base.frame.w
    );
}

#[test]
fn the_link_mark_on_root_hides_every_glyph() {
    assert!(svg::render(&with_css("")).contains("link-mark"));
    assert!(!svg::render(&with_css(":root { --link-mark: none; }")).contains("link-mark"));
    assert!(!svg::render(&with_css("card { --link-mark: none; }")).contains("link-mark"));
}

#[test]
fn colours_and_radius_on_root_are_drawn() {
    let base = pixels(&with_css(""));
    for css in [
        ":root { --bg: #ff0000; }",
        ":root { --surface: #ff0000; }",
        ":root { --ink: #ff0000; }",
        ":root { --card-border: #ff0000; }",
        ":root { --radius: 0px; }",
    ] {
        assert_ne!(pixels(&with_css(css)), base, "{css}");
    }
}
