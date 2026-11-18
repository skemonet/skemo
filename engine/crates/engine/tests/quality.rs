//! Each quality property (`quality::defects`) finds the defect it was written for, on the minimal
//! case reported (2026-11-09), and stays quiet on a plain diagram.

use skemo_engine::{Options, quality};
mod elk;
use elk::from_source;

fn broken(src: &str, property: &str) -> Vec<String> {
    let d = from_source(src, &Options::default()).unwrap().0;
    quality::defects(&d).remove(property).unwrap()
}

#[test]
fn a_plain_diagram_breaks_nothing() {
    let d = from_source(
        "diagram(dir=\"down\")\n  card#a A\n    to(b)\n    to(c)\n  card#b B\n  card#c C\n",
        &Options::default(),
    )
    .unwrap()
    .0;
    let all: Vec<_> = quality::defects(&d)
        .into_iter()
        .filter(|(_, v)| !v.is_empty())
        .collect();
    assert!(all.is_empty(), "{all:?}");
}

// bug 1's minimal case: a wide card with no arrow, stretched on the zones' row, lay over both
// (fixed: it takes a row of its own, after them as written); the detector is checked on the card
// moved back by hand
#[test]
fn a_wide_card_over_two_zones_is_an_overlap() {
    let src = "diagram(dir=\"down\")\n  zone#a Zone A\n    card#a1 A1\n    card#a2 A2\n  zone#b Zone B\n    card#b1 B1\n    card#b2 B2\n  card#base.wide Base\n";
    assert!(broken(src, "card-overlap").is_empty());
    let mut d = from_source(src, &Options::default()).unwrap().0;
    let (base, a) = (d.boxes["base"], d.boxes["a"]);
    assert!(base.y >= a.bottom(), "{base:?} {a:?}");
    d.boxes.insert(
        "base".into(),
        skemo_engine::layout::Box2 {
            y: d.boxes["a1"].y,
            ..base
        },
    );
    assert!(!quality::defects(&d)["card-overlap"].is_empty());
}

// bug 3's minimal case: the pill went past its zone's frame (fixed: the pill takes a line of its
// own and the zone widens to hold it); the detector is checked on a frame narrowed by hand
#[test]
fn a_zone_pill_wider_than_its_frame_overflows() {
    let src = "diagram(dir=\"right\")\n  zone#a(tag=\"operator.open-cluster-management.io\") Installation\n    card#x X\n";
    assert!(broken(src, "zone-title-overflow").is_empty());
    let mut d = from_source(src, &Options::default()).unwrap().0;
    let a = d.boxes["a"];
    d.boxes
        .insert("a".into(), skemo_engine::layout::Box2 { w: 150.0, ..a });
    assert_eq!(quality::defects(&d)["zone-title-overflow"], ["a"]);
}

// bug 4's minimal case: the shelf's column of unlinked cards grew along the flow without pushing
// the zone below (fixed); the detector is checked on a zone moved by hand
#[test]
fn a_zone_covering_its_sibling_is_an_overlap() {
    let src = "diagram(dir=\"down\")\n  zone#hub Hub\n    zone#g Global\n      card#mc MC\n        to(set)\n      card#set Set\n    zone#c Bureau\n      card#mw Work\n        to(mc)\n      card#x1 X1\n      card#x2 X2\n      card#x3 X3\n      card#x4 X4\n      card#x5 X5\n";
    assert!(broken(src, "zone-overlap").is_empty());
    let mut d = from_source(src, &Options::default()).unwrap().0;
    let g = d.boxes["g"];
    d.boxes.insert(
        "c".into(),
        skemo_engine::layout::Box2 {
            y: g.y + g.h / 2.0,
            ..g
        },
    );
    assert_eq!(quality::defects(&d)["zone-overlap"], ["g c"]);
}

#[test]
fn a_character_the_font_lacks_is_found() {
    let src = "diagram\n  card#a Carte\n    | \u{270e} à écrire\n";
    assert_eq!(broken(src, "missing-glyph"), ["a \u{270e}"]);
}

#[test]
fn an_unlinked_card_written_first_on_a_later_row_breaks_the_order() {
    let src = "diagram(dir=\"down\")\n  zone#z Zone\n    card#start Départ\n    card#a A\n      to(x constraint=\"false\")\n    card#b B\n      to(a constraint=\"false\")\n      to(x constraint=\"false\")\n  zone#e Autre\n    card#x X\n";
    assert_eq!(broken(src, "writing-order"), ["start"]);
}

// bug 2 (fixed: `a_wide_note_docked_below_spans_the_drawing`); the detector is checked on the note
// narrowed by hand
#[test]
fn a_note_docked_wide_but_narrow_is_found() {
    let src = "diagram(dir=\"down\")\n  card#a A\n  card#b B\n  card#c C\n  card#n.note.wide(at=\"s\")\n    | Un long texte de note qui devrait s'étendre sur toute la largeur du dessin plutôt que de rester en colonne étroite.\n";
    let mut d = from_source(src, &Options::default()).unwrap().0;
    let n = d.boxes["n"];
    d.boxes
        .insert("n".into(), skemo_engine::layout::Box2 { w: 200.0, ..n });
    assert_eq!(quality::defects(&d)["wide-docked-narrow"], ["n"]);
}

#[test]
fn backticks_in_a_label_are_markup_shown() {
    let src = "diagram\n  card#a A\n    to(b) `:2379`\n  card#b B\n";
    assert_eq!(broken(src, "label-markup"), ["a->b"]);
}

fn boxed(src: &str) -> skemo_engine::Diagram {
    from_source(src, &Options::default()).unwrap().0
}

// the router never draws a route through a card, even led there by `via=`: the detectors are
// checked on routes set by hand
#[test]
fn an_arrow_through_a_card_is_found() {
    let mut d = boxed("diagram(dir=\"right\")\n  card#a A\n    to(c)\n  card#b B\n  card#c C\n");
    let (a, b, c) = (d.boxes["a"], d.boxes["b"], d.boxes["c"]);
    let y = b.y + b.h / 2.0;
    d.routes.insert(
        "a->c".into(),
        vec![
            (a.right(), a.y + 10.0),
            (a.right() + 8.0, a.y + 10.0),
            (a.right() + 8.0, y),
            (c.right() + 20.0, y),
            (c.right() + 20.0, c.y + 10.0),
            (c.right(), c.y + 10.0),
        ],
    );
    assert_eq!(quality::defects(&d)["through-card"], ["a->c b"]);
}

#[test]
fn two_unrelated_arrows_on_one_line_are_an_overlay() {
    let mut d = boxed(
        "diagram(dir=\"down\")\n  card#a A\n    to(c)\n  card#b B\n    to(d)\n  card#c C\n  card#d D\n",
    );
    let (a, b, c, dd) = (d.boxes["a"], d.boxes["b"], d.boxes["c"], d.boxes["d"]);
    let y = (a.bottom() + c.y) / 2.0;
    let (xa, xb) = (a.x + a.w / 2.0, b.x + b.w / 2.0);
    d.routes.insert(
        "a->c".into(),
        vec![(xa, a.bottom()), (xa, y), (xb, y), (xb, c.y)],
    );
    d.routes.insert(
        "b->d".into(),
        vec![(xb, b.bottom()), (xb, y), (xa, y), (xa, dd.y)],
    );
    assert_eq!(quality::defects(&d)["overlay"], ["a->c b->d"]);
}

#[test]
fn pinning_an_unlinked_card_where_it_stands_moves_nothing() {
    // the defect of 2026-11-09 (the stability of an edit, D-031): the tidy passes skipped the
    // pinned card and the others re-flowed around it; they now see no card as pinned (D-031)
    let src = "diagram(dir=\"right\")\n  card#plain Plain card\n  card#typed Typed card\n    | first line\n  card#tagged With a tag\n";
    let d = boxed(src);
    let (pin, moved) = quality::stability(&elk::graph(src), &Options::default(), &d);
    assert!(pin.is_empty(), "{pin:?}");
    assert!(moved.is_empty(), "{moved:?}");
}

#[test]
fn pinning_a_card_of_an_inventory_where_it_stands_moves_nothing() {
    // four cards with no arrow are laid as a grid (`inventory_grids`); a pinned one stays in it
    let src = "diagram\n  zone#z Inventory\n    card#a A\n    card#b B\n    card#c C\n    card#d D\n    card#e E\n";
    let d = boxed(src);
    let (pin, _) = quality::stability(&elk::graph(src), &Options::default(), &d);
    assert!(pin.is_empty(), "{pin:?}");
}

#[test]
fn pinning_a_card_of_a_docked_zone_where_it_stands_moves_nothing() {
    // a docked zone is laid out on its own, then moved beside the drawing: its pins are in the
    // drawing's frame, not in its own
    let src = "diagram\n  card#x X\n    to(y)\n  card#y Y\n  zone#k(at=\"s\") Key\n    card#a A\n      to(b)\n    card#b B\n    card#c C\n";
    let d = boxed(src);
    let (pin, _) = quality::stability(&elk::graph(src), &Options::default(), &d);
    assert!(pin.is_empty(), "{pin:?}");
}

#[test]
fn a_linked_chain_pinned_where_it_stands_keeps_its_place() {
    let src = "diagram(dir=\"down\")\n  card#a A\n    to(b)\n  card#b B\n    to(c)\n  card#c C\n";
    let d = boxed(src);
    let (pin, _) = quality::stability(&elk::graph(src), &Options::default(), &d);
    assert!(pin.is_empty(), "{pin:?}");
}

// bugs 13 and 14 of the private corpus: an arrow leaving from under the card beside its source,
// or ending on another card; checked on a route moved by hand
#[test]
fn an_arrow_ending_off_its_card_is_found() {
    let src = "diagram(dir=\"down\")\n  card#a A\n    to(b)\n  card#b B\n  card#c C\n";
    assert!(broken(src, "end-off-card").is_empty());
    let mut d = from_source(src, &Options::default()).unwrap().0;
    let c = d.boxes["c"];
    let r = d.routes.get_mut("a->b").unwrap();
    *r.last_mut().unwrap() = (c.x + c.w / 2.0, c.y);
    assert_eq!(quality::defects(&d)["end-off-card"], ["a->b to"]);
}

// bug 17: a `code` too long for a line ran out of its card; a hyphenated one now wraps after a
// hyphen, as the browser does; one with no place to break still overflows
#[test]
fn a_code_wider_than_its_card_overflows() {
    let src = "diagram\n  card#a collector\n    | ns `a-namespace-name-far-too-long-to-wrap-anywhere-at-all`\n";
    assert!(broken(src, "text-overflow").is_empty());
    let src = "diagram\n  card#a collector\n    | ns `a.dotted.name.far.too.long.to.wrap.anywhere.at.all.really`\n";
    assert_eq!(broken(src, "text-overflow"), ["a"]);
    assert!(broken("diagram\n  card#a A\n    | short `code`\n", "text-overflow").is_empty());
}

#[test]
fn only_a_word_too_wide_for_a_line_breaks_after_a_hyphen() {
    use skemo_engine::lines::hyphen_pieces;
    assert_eq!(
        hyphen_pieces("open-cluster-management"),
        ["open-", "cluster-", "management"]
    );
    // never before a digit, nor at a word's ends
    assert_eq!(hyphen_pieces("eu-west-3a"), ["eu-", "west-3a"]);
    assert_eq!(hyphen_pieces("-x-"), ["-x-"]);
}

// bug 24: a label wider than the gap between its short upright step and a parallel line lay over
// that line (fixed: it slides along its own line, clear of the other)
#[test]
fn a_wide_label_on_a_short_step_clears_the_line_beside_it() {
    let src = "diagram\n  card#m Middle\n    to(c) a fairly long label here\n  card#top A rather wide card on top\n    to(m)\n    to(c)\n  card#c Bottom\n";
    assert!(broken(src, "through-label").is_empty());
}

// bug 25: a note stood far from the card it annotates, its link across the drawing; the detector
// is checked on the note moved away by hand
#[test]
fn a_note_far_from_its_card_is_found() {
    let src = "diagram\n  card#a A\n  card#n.note N\n    to(a).link\n";
    assert!(broken(src, "note-far").is_empty());
    let mut d = from_source(src, &Options::default()).unwrap().0;
    let n = d.boxes["n"];
    d.boxes.insert(
        "n".into(),
        skemo_engine::layout::Box2 {
            x: n.x + 200.0,
            ..n
        },
    );
    assert_eq!(quality::defects(&d)["note-far"], ["n"]);
}

// bug 16: a zone's description line ran out of its frame (fixed: it wraps, D-101); the detector
// is checked on a frame narrowed by hand
#[test]
fn a_zone_description_wider_than_its_frame_overflows() {
    let src = "diagram\n  zone#a Search\n    | An index of the resources of every cluster of the fleet.\n    card#x X\n";
    assert!(broken(src, "zone-title-overflow").is_empty());
    let mut d = from_source(src, &Options::default()).unwrap().0;
    let a = d.boxes["a"];
    d.boxes
        .insert("a".into(), skemo_engine::layout::Box2 { w: 60.0, ..a });
    assert_eq!(quality::defects(&d)["zone-title-overflow"], ["a"]);
}

// bug 25, second cause: no free side beside the card, the note went beyond the row with a link
// across it (fixed: the row opens after the card for it, D-104)
#[test]
fn a_row_opens_for_a_note_with_no_free_side() {
    let src = "diagram\n  zone#z Zone\n    card#s A card with a long title\n    card#top Top\n      to(s)\n      to(p)\n      to(d)\n    card#p Middle card\n    card#n.note Handled by\n      to(p).link\n    card#d Another card\n";
    assert!(broken(src, "note-far").is_empty());
}

// bug 28: two cards of a column one pixel into each other; cards stand on whole pixels (D-100), so
// one pixel is an overlap, not rounding. Checked on a card moved by hand.
#[test]
fn cards_one_pixel_into_each_other_overlap() {
    let src = "diagram\n  card#a A\n    to(b)\n  card#b B\n";
    assert!(broken(src, "card-overlap").is_empty());
    let mut d = from_source(src, &Options::default()).unwrap().0;
    let a = d.boxes["a"];
    d.boxes.insert(
        "b".into(),
        skemo_engine::layout::Box2 {
            y: a.bottom() - 1.0,
            ..d.boxes["b"]
        },
    );
    assert_eq!(quality::defects(&d)["card-overlap"], ["a b"]);
}

// bug 27: a zone written after a sibling it is reached from stood before it along the flow. Checked
// on the zones swapped by hand.
#[test]
fn a_zone_before_one_it_comes_from_breaks_the_order() {
    let src = "diagram\n  zone#a A\n    card#x X\n      to(y)\n  zone#b B\n    card#y Y\n";
    assert!(broken(src, "zone-order").is_empty());
    let mut d = from_source(src, &Options::default()).unwrap().0;
    let (a, b) = (d.boxes["a"], d.boxes["b"]);
    d.boxes.insert(
        "b".into(),
        skemo_engine::layout::Box2 {
            y: a.y - b.h - 40.0,
            ..b
        },
    );
    assert_eq!(quality::defects(&d)["zone-order"], ["a b"]);
}

// "Comment marche ACM" (the user's site): a paragraph drawn on one line as wide as its sentence,
// and cards outside the flow growing the image. Checked on boxes moved by hand.
#[test]
fn a_paragraph_past_the_measure_and_cards_outside_the_flow_are_found() {
    let src =
        "diagram\n  card#a A\n    to(b)\n  card#b B\n  card#t.text(at=\"0,0\") Note\n    | short\n";
    let mut d = from_source(src, &Options::default()).unwrap().0;
    assert!(quality::defects(&d)["long-line"].is_empty());
    let t = d.boxes["t"];
    d.boxes
        .insert("t".into(), skemo_engine::layout::Box2 { w: 900.0, ..t });
    assert_eq!(quality::defects(&d)["long-line"], ["t"]);
    let src = "diagram\n  card#a A\n    to(b)\n  card#b B\n  card#k Key\n";
    let mut d = from_source(src, &Options::default()).unwrap().0;
    let (b, k) = (d.boxes["b"], d.boxes["k"]);
    d.boxes.insert(
        "k".into(),
        skemo_engine::layout::Box2 {
            x: b.right() + 600.0,
            ..k
        },
    );
    assert_eq!(quality::defects(&d)["sprawl"], ["k"]);
}

// bug 2: a `.wide` note docked below kept its own width (280 px under a drawing of 400); it spans
// the drawing, its text wrapped to it
#[test]
fn a_wide_note_docked_below_spans_the_drawing() {
    let src = "diagram(dir=\"down\")\n  card#a A\n  card#b B\n  card#c C\n  card#n.note.wide(at=\"s\")\n    | A long note text that should span the whole width of the drawing rather than stay a narrow column.\n";
    assert!(broken(src, "wide-docked-narrow").is_empty());
}

// the user's "Carte des CRDs": zones in a staircase, cards over 10 % of the drawing. Checked on a
// card moved far by hand.
#[test]
fn a_scattered_drawing_is_found() {
    let src = "diagram\n  card#a A\n    to(b)\n  card#b B\n    to(c)\n  card#c C\n    to(d)\n  card#d D\n";
    assert!(broken(src, "sparse").is_empty());
    let mut d = from_source(src, &Options::default()).unwrap().0;
    let b = d.boxes["d"];
    d.boxes.insert(
        "d".into(),
        skemo_engine::layout::Box2 {
            x: b.x + 1500.0,
            y: b.y + 1500.0,
            ..b
        },
    );
    assert_eq!(quality::defects(&d)["sparse"], ["drawing"]);
}
