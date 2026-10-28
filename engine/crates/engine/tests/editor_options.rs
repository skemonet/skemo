//! What the editor needs from the engine: minimum card size, the frame holds the arrow labels.

use skemo_engine::{Options, from_source};

fn build(src: &str) -> skemo_engine::Diagram {
    from_source(src, &Options::default()).unwrap().0
}

#[test]
fn a_card_size_is_a_minimum() {
    let d = build(
        "diagram\n  card#a(size=\"300,140\") Big\n  card#b(size=\"10,10\") Small text here\n",
    );
    let (a, b) = (d.boxes["a"], d.boxes["b"]);
    assert_eq!((a.w, a.h), (300.0, 140.0));
    assert!(b.w > 10.0 && b.h > 10.0, "{b:?}");
}

#[test]
fn the_frame_holds_the_labels() {
    let d = build(
        "diagram(dir=\"right\")\n  card#a A\n    to(b) a rather long label on this arrow\n  card#b B\n",
    );
    let (x, y) = d.labels.values().next().copied().unwrap();
    let s = d.sizes.labels.values().next().copied().unwrap();
    assert!(x - s.w / 2.0 + d.frame.ox >= 0.0 && x + s.w / 2.0 + d.frame.ox <= d.frame.w);
    assert!(y - s.h / 2.0 + d.frame.oy >= 0.0 && y + s.h / 2.0 + d.frame.oy <= d.frame.h);
}
