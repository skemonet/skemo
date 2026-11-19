//! The slide engine (`diagram(layout="slide")`, one model and one search): what it promises on a
//! small map of zones.

use skemo_engine::{Options, from_source, quality};

/// Three groups, labelled arrows between them and inside one, a fork drawn alike with one label,
/// a label too long for one line, a legend docked top right.
const MAP: &str = "diagram(layout=\"slide\")
  title Mini map
  zone#a Alpha
    | A description long enough to wrap on a second line under the title of the zone
    card#hub Hub
      | installs everything
      to(op) read by
      to(eng)
    card#op Operator
      | keeps the parts up to date
      to(idx) deploys and watches every part it installed
  zone#b Beta
    card#eng Engine
      | the registry's operator
      to(reg) installs
      to(idx) installs
    card#reg Registry
      | every cluster
  zone#c Gamma
    card#idx Indexer
      | ranks the inventory
    card#bkp Backup
      | every hour
      to(hub) saves
  card#key.legend(at=\"ne\")
    | legend
";

fn slide(src: &str) -> skemo_engine::Diagram {
    from_source(src, &Options::default()).unwrap().0
}

#[test]
fn every_arrow_is_drawn_and_named() {
    let d = slide(MAP);
    for e in &d.model.edges {
        let r = d
            .routes
            .get(&e.id)
            .unwrap_or_else(|| panic!("{} not drawn", e.id));
        assert!(r.len() >= 2, "{}", e.id);
        // orthogonal: every stretch level or upright
        for w in r.windows(2) {
            assert!(
                (w[0].0 - w[1].0).abs() < 0.01 || (w[0].1 - w[1].1).abs() < 0.01,
                "{}: {r:?}",
                e.id
            );
        }
        if !e.label.is_empty() {
            assert!(d.labels.contains_key(&e.id), "label of {} left out", e.id);
        }
    }
}

#[test]
fn a_long_label_is_wrapped_to_the_label_width() {
    let d = slide(MAP);
    let id = d
        .model
        .edges
        .iter()
        .find(|e| e.label.starts_with("deploys"))
        .map(|e| e.id.clone())
        .unwrap();
    let lines = d.label_lines.get(&id).expect("wrapped");
    assert!((2..=3).contains(&lines.len()), "{lines:?}");
    assert!(d.sizes.labels[&id].w <= 140.0 + 0.5);
}

#[test]
fn cards_share_one_module_and_sit_on_the_lattice() {
    let d = slide(MAP);
    let cards: Vec<_> = ["hub", "op", "eng", "reg", "idx", "bkp"]
        .iter()
        .map(|c| d.boxes[*c])
        .collect();
    for b in &cards {
        assert!((b.w - cards[0].w).abs() < 0.01 && (b.h - cards[0].h).abs() < 0.01);
        assert!((b.x / 10.0).fract().abs() < 1e-9 && (b.y / 10.0).fract().abs() < 1e-9);
    }
    for z in ["a", "b", "c"] {
        let f = d.boxes[z];
        let held = d
            .model
            .nodes
            .iter()
            .filter(|n| n.parent.as_deref() == Some(z));
        for n in held {
            let b = d.boxes[&n.id];
            assert!(
                b.x >= f.x && b.right() <= f.right() && b.y >= f.y && b.bottom() <= f.bottom(),
                "{} out of {z}",
                n.id
            );
        }
    }
}

#[test]
fn the_same_source_draws_the_same_slide() {
    let (a, b) = (slide(MAP), slide(MAP));
    assert_eq!(a.routes, b.routes);
    assert_eq!(a.labels, b.labels);
}

#[test]
fn nothing_hidden() {
    let d = slide(MAP);
    let defects = quality::defects(&d);
    for p in [
        "end-off-card",
        "zone-overlap",
        "card-outside-zone",
        "card-overlap",
        "zone-title-overflow",
        "text-overflow",
        "through-card",
        "label-overlap",
    ] {
        assert!(defects[p].is_empty(), "{p}: {:?}", defects[p]);
    }
}

#[test]
fn what_it_does_not_read_goes_to_the_grid_engine() {
    // an invisible card (a bare one without a title): drawn by the grid engine, still drawn
    let d = slide(
        "diagram(layout=\"slide\")
  zone#a Alpha
    card#x X
      to(y)
    card#y Y
    card#z.bare
",
    );
    assert!(d.routes.contains_key(&d.model.edges[0].id));
    assert_eq!(d.layout_json()["layout"]["engine"], "grid");
}

#[test]
fn zones_inside_zones_are_framed_one_inside_the_other() {
    let d = slide(
        "diagram(layout=\"slide\")
  zone#hub Hub
    zone#g Global
      card#mc MC
        to(set)
      card#set Set
    zone#c Office
      card#w Work
        to(mc)
      card#x X
    card#own Own
      to(w)
  card#out Outside
    to(own)
",
    );
    assert_eq!(d.layout_json()["layout"]["engine"], "slide");
    let inside = |a: &str, b: &str| {
        let (a, b) = (d.boxes[a], d.boxes[b]);
        a.x > b.x && a.right() < b.right() && a.y > b.y && a.bottom() < b.bottom()
    };
    for (card, zone) in [
        ("mc", "g"),
        ("set", "g"),
        ("w", "c"),
        ("x", "c"),
        ("own", "hub"),
    ] {
        assert!(inside(card, zone), "{card} out of {zone}");
    }
    assert!(inside("g", "hub") && inside("c", "hub"));
    assert!(!inside("out", "hub"));
    // the two inner zones apart
    let (g, c) = (d.boxes["g"], d.boxes["c"]);
    assert!(g.right() < c.x || c.right() < g.x || g.bottom() < c.y || c.bottom() < g.y);
    let defects = quality::defects(&d);
    for p in [
        "zone-overlap",
        "card-outside-zone",
        "card-overlap",
        "through-card",
    ] {
        assert!(defects[p].is_empty(), "{p}: {:?}", defects[p]);
    }
}

#[test]
fn the_layout_says_which_engine_drew_it() {
    let d = slide(MAP);
    assert_eq!(d.layout_json()["layout"]["engine"], "slide");
}

/// Does the route `r` pass through point `p` (on a vertex or along a stretch)?
fn on_route(r: &[(f64, f64)], p: (f64, f64)) -> bool {
    r.windows(2).any(|w| {
        let (a, b) = (w[0], w[1]);
        let (x0, x1) = (a.0.min(b.0), a.0.max(b.0));
        let (y0, y1) = (a.1.min(b.1), a.1.max(b.1));
        p.0 >= x0 - 0.5 && p.0 <= x1 + 0.5 && p.1 >= y0 - 0.5 && p.1 <= y1 + 0.5
    })
}

#[test]
fn an_arrow_keeps_its_waypoints_and_its_sides() {
    let base = "diagram
  card#a A
    to(b)
    to(c)
  card#b B
  card#c C
";
    let d = slide(base);
    assert_eq!(d.layout_json()["layout"]["engine"], "slide");
    let (a, c) = (d.boxes["a"], d.boxes["c"]);
    // a waypoint below both cards, on the lattice
    let w = (
        ((a.x + a.w / 2.0) / 10.0).round() * 10.0,
        ((a.bottom().max(c.bottom()) + 60.0) / 10.0).round() * 10.0,
    );
    let src = base.replace(
        "    to(c)\n",
        &format!("    to(c via=\"{},{}\" side=\"bottom-bottom\")\n", w.0, w.1),
    );
    let d = slide(&src);
    let r = &d.routes["a->c"];
    assert!(on_route(r, w), "{w:?} not on {r:?}");
    let (a, c) = (d.boxes["a"], d.boxes["c"]);
    assert!(
        (r[0].1 - a.bottom()).abs() < 0.5,
        "leaves a by its bottom: {r:?}"
    );
    let end = r[r.len() - 1];
    assert!(
        (end.1 - c.bottom()).abs() < 0.5,
        "enters c by its bottom: {r:?}"
    );
}

#[test]
fn an_empty_zone_is_drawn_as_a_card() {
    let d = slide(
        "diagram\n  zones#z(count=\"3\") Empty\n    | to be filled\n  card#a A\n    to(b)\n  card#b B\n",
    );
    assert_eq!(d.layout_json()["layout"]["engine"], "slide");
    let (z, a) = (d.boxes["z"], d.boxes["a"]);
    assert!(z.w > 0.0 && z.h > 0.0, "{z:?}");
    let node = d.model.node("z").unwrap();
    assert_eq!(node.kind, skemo_lang::Kind::Card);
    assert_eq!(node.deck.and_then(|k| k.count), Some(3));
    assert!(z.w >= a.w, "a card's module: {z:?} {a:?}");
}

#[test]
fn dir_right_lays_the_flow_left_to_right() {
    let d =
        slide("diagram(dir=\"right\")\n  card#a A\n    to(b)\n  card#b B\n    to(c)\n  card#c C\n");
    assert_eq!(d.layout_json()["layout"]["engine"], "slide");
    let (a, b, c) = (d.boxes["a"], d.boxes["b"], d.boxes["c"]);
    assert!(a.x < b.x && b.x < c.x, "{a:?} {b:?} {c:?}");
}

#[test]
fn a_trunk_join_leaves_its_card_once() {
    let d = slide(
        "diagram\n  card#a A\n    to(b c d join=\"trunk\")\n  card#b B\n  card#c C\n  card#d D\n",
    );
    let starts: Vec<(f64, f64)> = ["a->b", "a->c", "a->d"]
        .iter()
        .map(|id| d.routes[*id][0])
        .collect();
    assert!(starts.windows(2).all(|w| w[0] == w[1]), "{starts:?}");
}

#[test]
fn wrap_lays_n_children_a_row_in_writing_order() {
    let d = slide(
        "diagram\n  zone#z(wrap=\"2\") Z\n    card#a A\n    card#b B\n    card#c C\n    card#e E\n  card#x X\n    to(a)\n",
    );
    assert_eq!(d.layout_json()["layout"]["engine"], "slide");
    let at = |id: &str| (d.boxes[id].y, d.boxes[id].x);
    // two a row, in writing order
    assert!(at("a").0 == at("b").0 && at("a").1 < at("b").1);
    assert!(at("c").0 == at("e").0 && at("c").1 < at("e").1);
    assert!(at("a").0 < at("c").0);
}

#[test]
fn a_zone_placed_and_sized_by_hand_keeps_that_frame() {
    let d = slide(
        "diagram\n  zone#z(at=\"600,400\" size=\"500,300\") Z\n    card#a A\n  card#x X\n    to(a)\n",
    );
    assert_eq!(d.layout_json()["layout"]["engine"], "slide");
    let z = d.boxes["z"];
    assert_eq!((z.x, z.y, z.w, z.h), (600.0, 400.0, 500.0, 300.0));
    let a = d.boxes["a"];
    assert!(a.x > z.x && a.y > z.y, "{a:?} {z:?}");
}

#[test]
fn zones_fit_keeps_each_frame_to_its_cards() {
    let src = "diagram(zones=\"fit\")\n  zone#z Z\n    card#a A\n  zone#w W\n    card#b B\n    card#c C\n    card#e E\n    card#f F\n  card#x X\n    to(a b)\n";
    let d = slide(src);
    let (z, a) = (d.boxes["z"], d.boxes["a"]);
    // one card: the frame round it, its padding on either side
    assert!((z.w - a.w - 60.0).abs() < 0.5, "{z:?} {a:?}");
}

#[test]
fn an_arrow_to_a_zone_ends_on_its_frame() {
    let d = slide("diagram\n  card#x X\n    to(z)\n  zone#z Z\n    card#a A\n    card#b B\n");
    assert_eq!(d.layout_json()["layout"]["engine"], "slide");
    let z = d.boxes["z"];
    let r = &d.routes["x->z"];
    let end = *r.last().unwrap();
    let on_side = |v: f64, a: f64| (v - a).abs() < 0.5;
    let on_frame =
        (on_side(end.0, z.x) || on_side(end.0, z.right())) && end.1 >= z.y && end.1 <= z.bottom()
            || (on_side(end.1, z.y) || on_side(end.1, z.bottom()))
                && end.0 >= z.x
                && end.0 <= z.right();
    assert!(on_frame, "{r:?} {z:?}");
}

#[test]
fn an_arrow_from_a_zone_leaves_its_frame() {
    // `to` written in a zone: the arrow leaves the zone's frame, here into another zone
    let d = slide("diagram\n  zone#a A\n    to(b) VPN\n    card#x X\n  zone#b B\n    card#y Y\n");
    assert_eq!(d.layout_json()["layout"]["engine"], "slide");
    let (a, b) = (d.boxes["a"], d.boxes["b"]);
    let r = &d.routes["a->b"];
    let (start, end) = (r[0], *r.last().unwrap());
    let on = |p: (f64, f64), z: skemo_engine::layout::Box2| {
        let near = |v: f64, w: f64| (v - w).abs() < 0.5;
        (near(p.0, z.x) || near(p.0, z.right())) && p.1 >= z.y && p.1 <= z.bottom()
            || (near(p.1, z.y) || near(p.1, z.bottom())) && p.0 >= z.x && p.0 <= z.right()
    };
    assert!(on(start, a) && on(end, b), "{r:?} {a:?} {b:?}");
}

#[test]
fn the_members_of_a_column_share_an_axis() {
    let d = slide(
        "diagram\n  zone#a A\n    card#p.col-k P\n    card#q Q\n    card#r R\n  zone#b B\n    card#s S\n    card#t T\n    card#u.col-k U\n  card#x X\n    to(p u)\n",
    );
    assert_eq!(d.layout_json()["layout"]["engine"], "slide");
    let (p, u) = (d.boxes["p"], d.boxes["u"]);
    assert!((p.x - u.x).abs() < 0.5, "{p:?} {u:?}");
}

/// A bare zone's caption stands below its cards: it asks no header of the framed zones (it used to
/// be counted as a header text, moved down with every header it asked, and tripled them).
#[test]
fn a_bare_zone_caption_asks_no_header() {
    let src = "diagram
  zone#a.wide A
    card#x X
  zone#b.wide.bare B
    card#y Y
  zone#c.wide C
    card#z Z
";
    let d = slide(src);
    for (z, c) in [("a", "x"), ("c", "z")] {
        let (zb, cb) = (d.boxes[z], d.boxes[c]);
        assert!(cb.y - zb.y <= 50.0, "{z}: header {} px", cb.y - zb.y);
    }
}

/// `head="left"` (D-117): the zone's title and description in a column left of what it holds, the
/// cards beside them rather than under them.
#[test]
fn a_zone_s_head_on_the_left() {
    let src = "diagram
  zone#z(head=\"left\") Zone
    | a description line
    | another one
    card#a A
    card#b B
";
    let d = slide(src);
    let (z, a) = (d.boxes["z"], d.boxes["a"]);
    assert!(a.y - z.y <= 40.0, "the cards near the top: {z:?} {a:?}");
    assert!(
        a.x - z.x >= 100.0,
        "the cards right of the text: {z:?} {a:?}"
    );
}

/// The settings a type gives in CSS (D-117): `--head` on a zone's type, not inherited by the zones
/// inside it; `--side` on an arrow's class.
#[test]
fn css_gives_head_and_side() {
    let src = "diagram
  card#r R
  zone#z.box Zone
    | a description line
    zone#in Inner
      card#a A
        to(r).up
";
    let opts = Options {
        css: vec![".box { --head: left; }\n.up { --side: n-s; }".to_string()],
        ..Options::default()
    };
    let d = from_source(src, &opts).unwrap().0;
    assert_eq!(d.model.node("z").unwrap().head.as_deref(), Some("left"));
    assert_eq!(d.model.node("in").unwrap().head, None);
    assert_eq!(d.model.edges[0].side.as_deref(), Some("top-bottom"));
}
