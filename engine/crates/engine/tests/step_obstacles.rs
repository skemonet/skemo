//! In a diagram drawn in steps, an arrow is routed among what is shown at the same time (D-060):
//! a card that only appears later is no obstacle to it, and the cards do not move.

use skemo_engine::{Options, from_source};

/// `ic`, a wide card shown from step 3, stands right between `a` and `b`.
const SRC: &str = "diagram(dir=\"down\")
  card#a(at=\"0,0\") A
    to(b) @1-2
  card#b(at=\"0,240\") B
  card#c(at=\"300,0\") C
    to(d) @3
  card#d(at=\"300,240\") D
  card#ic(at=\"-60,110\" size=\"520,50\") Ingress controller @3
";

#[test]
fn an_arrow_crosses_the_room_of_a_card_not_yet_shown() {
    let d = from_source(SRC, &Options::default()).unwrap().0;
    let r = &d.routes["a->b"];
    assert_eq!(r.len(), 2, "straight down, through the room of ic: {r:?}");
}

#[test]
fn an_arrow_shown_with_the_card_goes_round_it() {
    let d = from_source(SRC, &Options::default()).unwrap().0;
    let r = &d.routes["c->d"];
    let ic = d.boxes["ic"];
    let through = r.windows(2).any(|s| {
        let (x0, x1) = (s[0].0.min(s[1].0), s[0].0.max(s[1].0));
        let (y0, y1) = (s[0].1.min(s[1].1), s[0].1.max(s[1].1));
        x0 < ic.right() && x1 > ic.x && y0 < ic.bottom() && y1 > ic.y
    });
    assert!(!through, "{r:?} crosses {ic:?}");
}

/// A slide generator's report: an arrow shown at one step only (`@2-2`), a card from the next one (`@3`).
#[test]
fn an_arrow_of_one_step_crosses_a_card_of_the_next() {
    let src = SRC.replace("to(b) @1-2", "to(b).dashed @2-2");
    let d = from_source(&src, &Options::default()).unwrap().0;
    let r = &d.routes["a->b"];
    assert_eq!(r.len(), 2, "straight down, through the room of ic: {r:?}");
}

/// The card in a zone of its own (as a slide generator writes rows): the zone shows nothing before its
/// cards, so it is no obstacle either.
#[test]
fn a_zone_whose_cards_come_later_is_crossed() {
    let src = SRC.replace("to(b) @1-2", "to(b).dashed @2-2").replace(
        "  card#ic(at=\"-60,110\" size=\"520,50\") Ingress controller @3\n",
        "  zone#row.bare\n    card#ic(at=\"-60,110\" size=\"520,50\") Ingress controller @3\n",
    );
    let d = from_source(&src, &Options::default()).unwrap().0;
    let r = &d.routes["a->b"];
    assert_eq!(r.len(), 2, "straight down, through the room of ic: {r:?}");
}

/// The same generator's second report (its Ingress slides, `tests/data/step-ports.skemo`): `w1` is not face to
/// face with `sa`, so its line turns; it crossed the
/// room of a card shown later only when straight (the router's graph kept a hole there), and went
/// round the whole stack instead.
#[test]
fn a_bent_arrow_crosses_the_room_of_a_card_not_yet_shown() {
    let src = include_str!("data/step-ports.skemo");
    let d = from_source(src, &Options::default()).unwrap().0;
    let r = &d.routes["w1->sa"];
    let stack = d.boxes["stack1"];
    assert!(
        r.iter().all(|p| p.0 > stack.x && p.0 < stack.right()),
        "through the room of ic, not round the stack: {r:?} {stack:?}"
    );
}
