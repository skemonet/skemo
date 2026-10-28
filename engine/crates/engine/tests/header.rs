//! Default "header" look: the name is in the coloured band; `look="classic"` renders the old look
//! (title under a coloured rule).

use skemo_engine::{Options, from_source};

const SRC: &str = "diagram(dir=\"right\")\n  card#a(tag=\"DHCP\") Alpha\n    li 10.0.0.1/24\n    to(b)\n  card#b Beta\n";

#[test]
fn name_sits_in_the_band_by_default() {
    let (d, _) = from_source(SRC, &Options::default()).unwrap();
    let a = &d.scene.cards["a"];
    let h = a.header.as_ref().expect("header by default");
    assert_eq!(h.type_text, "Alpha");
    assert_eq!(h.tag.as_ref().map(|t| t.1.as_str()), Some("DHCP"));
    assert!(a.title_text.is_empty());
    let b = d.scene.cards["b"].header.as_ref().expect("no tag either");
    assert_eq!((b.type_text.as_str(), b.tag.is_none()), ("Beta", true));
}

#[test]
fn classic_look_is_kept_as_an_alternative() {
    let src = SRC.replace(
        "diagram(dir=\"right\")",
        "diagram(dir=\"right\" look=\"classic\")",
    );
    let (d, _) = from_source(&src, &Options::default()).unwrap();
    let a = &d.scene.cards["a"];
    assert!(a.header.is_none());
    assert_eq!(a.title_text, "Alpha");
}

#[test]
fn per_type_classic_look() {
    let css = ".old { --look: classic; }".to_string();
    let src = "diagram\n  card#a.old Alpha\n  card#b Beta\n";
    let (d, _) = from_source(
        src,
        &Options {
            css: vec![css],
            ..Options::default()
        },
    )
    .unwrap();
    assert!(d.scene.cards["a"].header.is_none());
    assert!(d.scene.cards["b"].header.is_some());
}

#[test]
fn old_header_look_is_kept_as_an_alternative() {
    let src = SRC.replace(
        "diagram(dir=\"right\")",
        "diagram(dir=\"right\" look=\"header\")",
    );
    let (d, _) = from_source(&src, &Options::default()).unwrap();
    let a = &d.scene.cards["a"];
    let h = a.header.as_ref().unwrap();
    assert!(!h.band, "old header: name under the band");
    assert_eq!(a.title_text, "Alpha");
    assert_eq!(h.tag.as_ref().map(|t| t.1.as_str()), Some("DHCP"));
    // without tag nor label: classic card, as before
    assert!(d.scene.cards["b"].header.is_none());
}

/// A band card without lines keeps round bottom corners when text sizes are scaled: its band,
/// a fraction of a pixel shorter than the card rounded to whole pixels, still fills it.
#[test]
fn a_band_card_without_lines_has_round_corners_at_any_text_scale() {
    for scale in ["1", "1.3", "1.35", "1.5"] {
        let opts = skemo_engine::Options {
            css: vec![format!(".stage {{ --font-scale: {scale}; }}")],
            ..skemo_engine::Options::default()
        };
        let src = "diagram(dir=\"down\")\n  card#d.runtime(icon=\"container\") Docker\n";
        let d = skemo_engine::from_source(src, &opts).unwrap().0;
        let svg = skemo_engine::svg::render(&d);
        let at = svg.find("data-id=\"d\"").unwrap();
        let band = svg[at..]
            .split("<path d=\"")
            .nth(3)
            .and_then(|p| p.split('"').next())
            .unwrap();
        assert!(!band.contains("A0,0"), "scale {scale}: square band {band}");
    }
}
