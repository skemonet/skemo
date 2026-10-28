//! Defects reported by an agent styling diagrams (2026-10-28): see-through
//! backgrounds, shadows under them, `--curve` outside mind maps, fonts.

use skemo_engine::{Options, from_source, svg};

fn with_css(src: &str, css: &str) -> skemo_engine::Diagram {
    let opts = Options {
        css: vec![css.to_string()],
        ..Options::default()
    };
    from_source(src, &opts).unwrap().0
}

const SRC: &str =
    "zone#z Group\n  card#a.s Alpha\n    | one\n    to(b)\n  cards#b.s(count=\"3\") API\n";

#[test]
fn a_transparent_box_shows_only_its_border() {
    let d = with_css(
        SRC,
        "zone, card, card.look-band { background: transparent; } .s { --c: #2b5bd7; }",
    );
    let out = svg::render(&d);
    // the border is a ring, not a filled outline under a missing background
    assert!(out.contains("fill-rule=\"evenodd\""), "ring");
    // the zone's background is not drawn at all
    let zone = &out[out.find("<g class=\"zone\"").unwrap()..];
    let first = &zone[..zone.find("/>").unwrap()];
    assert!(first.ends_with("fill=\"none\""), "{first}");
}

#[test]
fn a_translucent_box_hides_its_shadow_and_the_pile_behind() {
    let css = ".s { --c: #2b5bd7; } card, card.look-band { background: rgba(255,255,255,.2); box-shadow: 0 8px 20px rgba(0,0,0,.5); }";
    let d = with_css(SRC, css);
    let out = svg::render(&d);
    assert!(
        out.contains("<clipPath") && out.contains("clip-rule=\"evenodd\""),
        "clipped"
    );
    let raster = svg::render_for_raster(&d);
    assert!(
        raster.contains("<clipPath"),
        "the PNG's shadows are clipped too"
    );
}

#[test]
fn an_opaque_box_is_drawn_as_before() {
    let d = with_css(SRC, "");
    assert!(!svg::render(&d).contains("clip-rule=\"evenodd\""));
}

#[test]
fn a_curve_outside_a_mind_map_is_ignored_with_a_warning() {
    let d = with_css(SRC, ".edge .line { --curve: on; }");
    assert!(
        d.warnings.iter().any(|w| w.contains("--curve")),
        "{:?}",
        d.warnings
    );
    assert!(!svg::render(&d).contains(" C"), "no Bézier segment");
}

#[test]
fn a_missing_font_is_named() {
    let d = with_css(SRC, ".stage { --font: 'Nowhere Sans', sans-serif; }");
    assert!(
        d.warnings.iter().any(|w| w.contains("Nowhere Sans")),
        "{:?}",
        d.warnings
    );
    let ok = with_css(SRC, "");
    assert!(
        !ok.warnings.iter().any(|w| w.contains("font")),
        "{:?}",
        ok.warnings
    );
}

#[test]
fn a_font_file_tells_its_family_weight_and_style() {
    let f = &skemo_engine::fonts::FILES;
    assert_eq!(
        skemo_engine::fonts::describe(f[5].data),
        Some(("Inter".into(), 700, false))
    );
    assert_eq!(
        skemo_engine::fonts::describe(f[1].data).map(|d| d.2),
        Some(true)
    );
    assert_eq!(
        skemo_engine::fonts::describe(f[7].data).map(|d| d.0),
        Some("JetBrains Mono".into())
    );
    assert!(skemo_engine::fonts::describe(b"not a font").is_none());
}
