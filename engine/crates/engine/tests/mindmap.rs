//! Mindmap mode (D-034): root at the centre, branches on both sides, sub-branches outward, no card
//! overlaps, tree arrows by the outer and central sides.

use skemo_engine::layout::Box2;
use skemo_engine::{Options, from_source};

const SRC: &str = include_str!("data/mindmap.skemo");

fn build() -> skemo_engine::Diagram {
    from_source(SRC, &Options::default()).unwrap().0
}

fn overlap(a: &Box2, b: &Box2) -> bool {
    a.x < b.right() && b.x < a.right() && a.y < b.bottom() && b.y < a.bottom()
}

#[test]
fn branches_go_both_ways_and_the_root_is_in_the_middle() {
    let d = build();
    let root = d.boxes["skemo"];
    let kids = ["language", "engine", "router", "cli", "editor", "docs"];
    let right = kids
        .iter()
        .filter(|k| d.boxes[**k].x >= root.right())
        .count();
    let left = kids
        .iter()
        .filter(|k| d.boxes[**k].right() <= root.x)
        .count();
    assert_eq!(
        right + left,
        kids.len(),
        "each branch is on one side or the other"
    );
    assert!(
        right >= 2 && left >= 2,
        "balanced: {right} on the right, {left} on the left"
    );
    // the centre of the root is on the vertical axis of the two blocks
    let (top, bottom) = d.boxes.values().fold((f64::MAX, f64::MIN), |(t, b), x| {
        (t.min(x.y), b.max(x.bottom()))
    });
    assert!(((top + bottom) / 2.0 - (root.y + root.h / 2.0)).abs() < 80.0);
}

#[test]
fn no_card_overlaps_another() {
    let d = build();
    let cards: Vec<_> = d.boxes.iter().collect();
    for (i, (ia, a)) in cards.iter().enumerate() {
        for (ib, b) in &cards[i + 1..] {
            assert!(!overlap(a, b), "{ia} chevauche {ib}");
        }
    }
}

#[test]
fn sub_branches_move_away_from_the_centre_and_are_centred_on_their_children() {
    let d = build();
    let root = d.boxes["skemo"];
    for (parent, kids) in [
        ("language", ["parser", "styles", "editing"].as_slice()),
        ("engine", ["measure", "layout", "render"].as_slice()),
    ] {
        let p = d.boxes[parent];
        let outward_right = p.x > root.x;
        for k in kids {
            let c = d.boxes[*k];
            if outward_right {
                assert!(c.x >= p.right(), "{k} right of {parent}");
            } else {
                assert!(c.right() <= p.x, "{k} left of {parent}");
            }
        }
        let first = d.boxes[kids[0]];
        let last = d.boxes[kids[kids.len() - 1]];
        let block_mid = (first.y + last.bottom()) / 2.0;
        assert!(
            (block_mid - (p.y + p.h / 2.0)).abs() < 1.0,
            "{parent} centred on its children"
        );
        // writing order from top to bottom
        assert!(d.boxes[kids[0]].y < d.boxes[kids[1]].y && d.boxes[kids[1]].y < d.boxes[kids[2]].y);
    }
}

#[test]
fn tree_arrows_leave_outwards_arrive_inwards_and_share_a_trunk_per_side() {
    let d = build();
    let root = d.boxes["skemo"];
    let mut starts_right = Vec::new();
    for k in ["language", "engine", "router", "cli", "editor", "docs"] {
        let route = &d.routes[&format!("skemo->{k}")];
        let b = d.boxes[k];
        let (a, z) = (route[0], *route.last().unwrap());
        if b.x >= root.right() {
            assert!(
                (a.0 - root.right()).abs() < 0.01,
                "leaves from the root's right edge"
            );
            assert!((z.0 - b.x).abs() < 0.01, "arrives on the left edge of {k}");
            starts_right.push(a);
        } else {
            assert!(
                (a.0 - root.x).abs() < 0.01,
                "leaves from the root's left edge"
            );
            assert!(
                (z.0 - b.right()).abs() < 0.01,
                "arrives on the right edge of {k}"
            );
        }
    }
    assert!(
        starts_right.windows(2).all(|w| w[0] == w[1]),
        "shared trunk on the right"
    );
}

#[test]
fn loose_cards_are_reported_and_put_below() {
    let src = "diagram(layout=\"mindmap\")\n  card#r Root\n    to(a)\n  card#a A\n  card#z Alone\n";
    let (d, _) = from_source(src, &Options::default()).unwrap();
    assert!(
        d.warnings.iter().any(|w| w.contains("« z »")),
        "{:?}",
        d.warnings
    );
    assert!(d.boxes["z"].y > d.boxes["a"].bottom());
}

#[test]
fn layout_does_not_depend_on_classes_or_theme() {
    let (a, _) = from_source(SRC, &Options::default()).unwrap();
    let dark = Options {
        theme: Some(skemo_lang::style::Theme::Dark),
        ..Options::default()
    };
    let (b, _) = from_source(SRC, &dark).unwrap();
    assert_eq!(a.boxes, b.boxes);
}
