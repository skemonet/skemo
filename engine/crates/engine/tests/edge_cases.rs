//! End-to-end edge cases: empty diagrams, pinned zones, animation warnings.

use skemo_engine::{Options, from_source, html, svg};

fn build(src: &str) -> skemo_engine::Diagram {
    from_source(src, &Options::default()).unwrap().0
}

fn codes(d: &skemo_engine::Diagram) -> Vec<&'static str> {
    d.anim.warnings.iter().map(|w| w.code).collect()
}

#[test]
fn empty_and_minimal_diagrams_render() {
    for src in [
        "diagram\n",
        "diagram(dir=\"right\")\n",
        "card#a A\n",
        "zone#z Z\n",
    ] {
        let d = build(src);
        assert!(d.frame.w > 0.0 && d.frame.h > 0.0, "{src:?}");
        assert!(svg::render(&d).starts_with("<svg"), "{src:?}");
        assert!(
            html::render(&d, &html::HtmlOptions::default()).contains("<card")
                || src.starts_with("diagram")
                || src.starts_with("zone")
        );
    }
    let empty = build("diagram\n");
    assert!(empty.boxes.is_empty() && empty.routes.is_empty());
    assert!(!empty.anim.is_animated());
}

#[test]
fn a_zone_with_at_and_size_keeps_exactly_that_frame() {
    let d = build("zone#z(at=\"10,20\" size=\"300,200\") Z\n  card#a A\n");
    let z = d.boxes["z"];
    assert_eq!((z.x, z.y, z.w, z.h), (10.0, 20.0, 300.0, 200.0));
}

#[test]
fn nested_zones_enclose_their_children() {
    let d = build("zone#outer O\n  zone#inner I\n    card#a A\n  card#b B\n");
    let (o, i, a) = (d.boxes["outer"], d.boxes["inner"], d.boxes["a"]);
    assert!(i.x <= a.x && i.right() >= a.right() && i.y <= a.y && i.bottom() >= a.bottom());
    assert!(o.x <= i.x && o.right() >= i.right() && o.y <= i.y && o.bottom() >= i.bottom());
}

#[test]
fn a_zone_without_cards_gets_a_default_frame() {
    let d = build("zone#z Vide\ncard#a A\n");
    let z = d.boxes["z"];
    assert!(z.w > 0.0 && z.h > 0.0);
}

#[test]
fn animation_warnings_are_reported() {
    // a range that ends with no later step
    let d = build("diagram\n  card#a A  @1-2\n  card#b B  @1\n");
    assert!(
        codes(&d).contains(&"no-step-after"),
        "{:?}",
        d.anim.warnings
    );
    // a focus outside the visibility period
    let d = build("diagram\n  card#a A  @2-3 @5:focus\n  card#b B  @6\n");
    assert!(
        codes(&d).contains(&"focus-outside"),
        "{:?}",
        d.anim.warnings
    );
    // a well-formed animation has none
    let d = build("diagram\n  card#a A  @1\n  card#b B  @2\n");
    assert!(d.anim.warnings.is_empty(), "{:?}", d.anim.warnings);
    assert!(d.anim.is_animated());
}

#[test]
fn unknown_options_and_odd_values_do_not_panic() {
    for src in [
        "diagram(dir=\"sideways\")\n  card#a A\n",
        "diagram(layout=\"nope\")\n  card#a A\n",
        "card#a(at=\"x,y\") A\n",
        "card#a(at=\"1e999,5\") A\n",
        "card#a A\n  to(a)\n",
        "card#a A\n  to(a)\n  to(a)\n",
    ] {
        let r = from_source(src, &Options::default());
        if let Ok((d, _)) = r {
            assert!(svg::render(&d).starts_with("<svg"), "{src:?}");
        }
    }
}
