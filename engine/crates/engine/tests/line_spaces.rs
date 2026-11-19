//! The spaces after a line's bar, past the first, are drawn (as in Pug): non-breaking, in SVG
//! and HTML, and they widen the line.

use skemo_engine::{Options, from_source, html, svg};

#[test]
fn spaces_before_a_line_are_drawn_and_measured() {
    let build = |src: &str| from_source(src, &Options::default()).unwrap().0;
    let d = build(
        "diagram\n  card#a Routes\n    | default\n    |         *via* the gateway of the other site\n",
    );
    let flat = build(
        "diagram\n  card#a Routes\n    | default\n    | *via* the gateway of the other site\n",
    );
    // the card's own width, before the slide engine evens it to its lattice
    let (w, flat_w) = (d.sizes.cards["a"].w, flat.sizes.cards["a"].w);
    assert!(w > flat_w + 20.0, "{w} {flat_w}");
    let s = svg::render(&d);
    assert!(
        s.contains("\u{a0}\u{a0}\u{a0}\u{a0}\u{a0}\u{a0}\u{a0}\u{a0}"),
        "eight kept spaces"
    );
    let h = html::render(&d, &Default::default());
    assert!(h.contains("\u{a0}\u{a0}\u{a0}\u{a0}\u{a0}\u{a0}\u{a0}\u{a0}"));
}
