//! Tidy layered drawings (D-047): rows as far apart in zones as outside, parents centred on their
//! children, arrows that leave and enter in the drawing's direction, forks, buses, junctions,
//! no two arrows sharing a line, stroke widths as the theme says.

use skemo_engine::{Options, from_source, svg};

fn build(src: &str) -> skemo_engine::Diagram {
    from_source(src, &Options::default()).unwrap().0
}

/// Two workers both linked to two services, each service with two leaves, in a zone.
const TREE: &str = "diagram(dir=\"down\")
  zone#z Zone
    card#w1 W1
      to(sa sb)
    card#w2 W2
      to(sa sb)
    card#sa SA
      to(a1 a2)
    card#sb SB
      to(b1 b2)
    card#a1 A1
    card#a2 A2
    card#b1 B1
    card#b2 B2
";

fn cx(d: &skemo_engine::Diagram, id: &str) -> f64 {
    let b = d.boxes[id];
    b.x + b.w / 2.0
}

#[test]
fn rows_in_a_zone_are_as_far_apart_as_outside() {
    let d = build(TREE);
    for (upper, lower) in [("w1", "sa"), ("sa", "a1")] {
        let gap = d.boxes[lower].y - d.boxes[upper].bottom();
        // D-048: 48 px between rows, inside a zone as outside
        assert!(gap >= 46.0, "{upper} and {lower}: {gap} px apart");
    }
}

#[test]
fn parents_are_centred_on_their_children() {
    let d = build(TREE);
    assert!((cx(&d, "sa") - (cx(&d, "a1") + cx(&d, "a2")) / 2.0).abs() < 1.0);
    assert!((cx(&d, "sb") - (cx(&d, "b1") + cx(&d, "b2")) / 2.0).abs() < 1.0);
    // a card over the child in its column
    assert!((cx(&d, "w1") - cx(&d, "sa")).abs() < 1.0);
    assert!((cx(&d, "w2") - cx(&d, "sb")).abs() < 1.0);
}

#[test]
fn arrows_leave_downwards_and_enter_from_above() {
    let d = build(TREE);
    for e in &d.model.edges {
        let r = &d.routes[&e.id];
        let (a, b) = (r[0], *r.last().unwrap());
        assert!(
            (a.1 - d.boxes[&e.from].bottom()).abs() < 1.0,
            "{} leaves at {a:?}",
            e.id
        );
        assert!(
            (b.1 - d.boxes[&e.to].y).abs() < 1.0,
            "{} enters at {b:?}",
            e.id
        );
    }
}

#[test]
fn a_card_forks_to_its_children_entering_them_in_the_middle() {
    let d = build(TREE);
    let (r1, r2) = (&d.routes["sa->a1"], &d.routes["sa->a2"]);
    assert_eq!(r1[0], r2[0], "one trunk");
    assert_eq!(r1[1], r2[1], "split at one point");
    for (r, t) in [(r1, "a1"), (r2, "a2")] {
        assert!(
            (r.last().unwrap().0 - cx(&d, t)).abs() < 1.0,
            "into the middle of {t}"
        );
    }
}

#[test]
fn every_one_to_every_one_is_a_bus() {
    let d = build(TREE);
    let ids = ["w1->sa", "w1->sb", "w2->sa", "w2->sb"];
    // the crossing routes run along one bar; the straight ones go through it
    let bar = |id: &str| {
        d.routes[id]
            .windows(2)
            .find(|w| (w[0].1 - w[1].1).abs() < 1e-6 && (w[0].0 - w[1].0).abs() > 1.0)
            .map(|w| w[0].1)
    };
    let (y1, y2) = (bar("w1->sb").unwrap(), bar("w2->sa").unwrap());
    assert!((y1 - y2).abs() < 1e-6, "one bar: {y1} {y2}");
    for id in ids {
        let r = &d.routes[id];
        let (top, bottom) = (
            r[0].1.min(r.last().unwrap().1),
            r[0].1.max(r.last().unwrap().1),
        );
        assert!(top < y1 && y1 < bottom, "{id} through the bar");
    }
    let svg = svg::render(&d);
    assert!(
        svg.matches("class=\"junction\"").count() >= 4,
        "junction dots"
    );
}

#[test]
fn no_two_arrows_share_a_line_out_of_a_card() {
    // the same graph without the zone and with the cards in another order
    let src = TREE
        .replace("  zone#z Zone\n", "")
        .replace("\n    ", "\n  ")
        .replace("\n      ", "\n    ");
    for d in [build(TREE), build(&src)] {
        for (i, (ia, a)) in d.routes.iter().enumerate() {
            for (ib, b) in d.routes.iter().skip(i + 1) {
                let same_group = d.edge_groups.contains_key(ia.as_str())
                    && d.edge_groups.get(ia) == d.edge_groups.get(ib);
                if same_group {
                    continue;
                }
                let (sa, sb) = ((a[0], a[1]), (b[b.len() - 1], b[b.len() - 2]));
                let vertical = |s: ((f64, f64), (f64, f64))| (s.0.0 - s.1.0).abs() < 1e-6;
                if vertical(sa) && vertical(sb) && (sa.0.0 - sb.0.0).abs() < 1.0 {
                    let (lo, hi) = (
                        sa.0.1.min(sa.1.1).max(sb.0.1.min(sb.1.1)),
                        sa.0.1.max(sa.1.1).min(sb.0.1.max(sb.1.1)),
                    );
                    assert!(hi - lo <= 1.0, "{ia} and {ib} overlap");
                }
            }
        }
    }
}

#[test]
fn hyperedges_to_far_targets_still_branch_downwards_and_finish() {
    // trunks aiming at a target far to the side used to leave by the side; with a card set in
    // a column under the source, a router pass used to loop for ever on it
    let src = TREE
        .replace("to(sa sb)", "to(sa sb join=\"trunk\")")
        .replace("to(a1 a2)", "to(a1 a2 join=\"trunk\")");
    let t = std::time::Instant::now();
    let d = build(&src);
    assert!(t.elapsed().as_secs() < 5);
    let r = &d.routes["w1->sb"];
    assert!(
        (r[0].1 - d.boxes["w1"].bottom()).abs() < 1.0,
        "trunk downwards: {r:?}"
    );
}

#[test]
fn arrows_take_the_theme_stroke_width() {
    let svg = svg::render(&build(TREE));
    assert!(
        svg.contains("stroke-width=\"1.4\""),
        "the theme's 1.4, not 1"
    );
}

#[test]
fn a_wrap_grid_stays_compact() {
    // its rows come from invisible edges, not from arrows: no room for arrows is needed
    let d = build(
        "zone#ns(wrap=\"1\") Stack
  card#a A
  card#b B
  card#c C
",
    );
    let gap = d.boxes["b"].y - d.boxes["a"].bottom();
    assert!(gap < 30.0, "{gap}");
}

/// Horizontal middle of a card's title text in the SVG: the `x` of the `<text>` holding `title`,
/// plus half its rendered width (taken from the layout's run).
fn title_centre_offset(d: &skemo_engine::Diagram, id: &str) -> f64 {
    let el = &d.scene.cards[id];
    let g = skemo_engine::geometry::card(&d.scene, el, d.boxes[id].w, d.boxes[id].h);
    let run = g
        .head
        .as_ref()
        .map(|h| h.type_run.clone())
        .or_else(|| g.title.first().cloned())
        .unwrap();
    run.x + run.width / 2.0 - d.boxes[id].w / 2.0
}

#[test]
fn a_card_without_lines_has_its_title_centred() {
    for look in ["band", "classic"] {
        let d = build(&format!(
            "diagram(look=\"{look}\")\ncard#a A rather long name\ncard#b Short\ncard#c With lines\n  li a line\n"
        ));
        // centred in the padded box (the band has 13 px of padding on the left, 10 on the right)
        assert!(title_centre_offset(&d, "b").abs() < 3.0, "{look}: centred");
        assert!(
            title_centre_offset(&d, "c") < -10.0,
            "{look}: a card with lines keeps it left"
        );
        let html = skemo_engine::html::render(&d, &Default::default());
        assert!(
            html.contains("no-body"),
            "{look}: the class reaches the HTML"
        );
    }
}

/// A leaf hanging from a card above a wide zone (a CDN beside the region): ELK puts it beyond the
/// zone, the arrow going round it; it moves beside its parent, on the side of its writing order.
#[test]
fn a_leaf_pushed_aside_by_a_zone_sits_beside_its_parent() {
    let d = build(
        "diagram(dir=\"down\")
  card#cdn CDN
  zone#region Region
    card#a A
      to(sink)
    card#b B
    card#c C
      to(sink)
    card#d D
      to(sink)
  card#front Front
    to(cdn)
    to(b)
  card#sink Sink
",
    );
    let (leaf, parent) = (d.boxes["cdn"], d.boxes["front"]);
    assert!((leaf.y - parent.y).abs() < 0.5, "{leaf:?} {parent:?}");
    assert!(
        (parent.x - leaf.right() - 40.0).abs() < 0.5,
        "{leaf:?} {parent:?}"
    );
    assert_eq!(
        d.routes["front->cdn"].len(),
        2,
        "{:?}",
        d.routes["front->cdn"]
    );
}

/// Cards with no arrow in a zone with a flow go on a shelf below the zone's content, in writing
/// order (an internet gateway and an endpoint in a VPC): they no longer take a place in its rows.
#[test]
fn cards_with_no_arrow_go_on_a_shelf_below_their_zone() {
    let d = build(
        "diagram(dir=\"down\")
  zone#vpc VPC
    card#igw Gateway
    card#lb Balancer
      to(app)
    card#app App
    card#s3 Endpoint
",
    );
    let b = &d.boxes;
    let bottom = b["lb"].bottom().max(b["app"].bottom());
    assert!(b["igw"].y > bottom && b["s3"].y > bottom, "{b:?}");
    assert!(
        (b["igw"].y - b["s3"].y).abs() < 0.5 && b["igw"].x < b["s3"].x,
        "{b:?}"
    );
}

/// A `wrap` grid keeps its rows in writing order when arrows between its cells go against it:
/// ELK broke the cycle they make with the grid's own edges, and its columns started on different
/// rows (or two cells of a column landed on one row, one over the other).
#[test]
fn a_wrap_grid_keeps_its_rows_when_arrows_go_against_the_order() {
    let d = build(
        "diagram(dir=\"down\")
  zone#z(wrap=\"2\") Z
    card#t T
      to(s)
    card#n N
    card#s S
    card#q Q
      to(r)
    card#r R
    card#u U
      to(t)
",
    );
    let b = &d.boxes;
    for row in [["t", "n"], ["s", "q"], ["r", "u"]] {
        assert!((b[row[0]].y - b[row[1]].y).abs() < 0.5, "{row:?} {b:?}");
    }
    assert!(
        b["t"].bottom() < b["s"].y && b["s"].bottom() < b["r"].y,
        "{b:?}"
    );
    assert!(b["t"].x < b["n"].x, "{b:?}");
}

/// Two cards side by side linked by a labelled arrow leave room for the label between them: a
/// label squeezed into a narrow gap ran over both cards and hid the arrow.
#[test]
fn cards_side_by_side_leave_room_for_their_arrow_label() {
    let d = build(
        "diagram(dir=\"down\")
  card#top Top
    to(orders replica)
  zone#data Data
    card#replica Replica
    card#orders Orders
      to(replica constraint=\"false\") replica
",
    );
    let (r, o) = (d.boxes["replica"], d.boxes["orders"]);
    let w = d.sizes.labels["orders->replica"].w;
    let gap = if r.right() <= o.x {
        o.x - r.right()
    } else {
        r.x - o.right()
    };
    assert!(gap >= w + 15.5, "gap {gap}, label {w}");
}
