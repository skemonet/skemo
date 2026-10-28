//! Details of the Kubernetes example (D-051): line pills, named joins into one card, badge
//! sides, wide zones, columns across zones, zone titles below their cards.

use skemo_engine::{Options, from_source, geometry, html};

fn build(src: &str) -> skemo_engine::Diagram {
    build_css(src, &[])
}

fn build_css(src: &str, css: &[&str]) -> skemo_engine::Diagram {
    let opts = Options {
        css: css.iter().map(|c| (*c).to_string()).collect(),
        ..Options::default()
    };
    from_source(src, &opts).unwrap().0
}

fn card_geom(d: &skemo_engine::Diagram, id: &str) -> geometry::CardGeom {
    let b = d.boxes[id];
    geometry::card(&d.scene, &d.scene.cards[id], b.w, b.h)
}

#[test]
fn a_pill_line_is_drawn_as_a_box_around_its_text() {
    let d = build("card#a A\n  li 10.0.0.1\n  li.pill 8080 · http\n");
    let g = card_geom(&d, "a");
    assert!(g.lis[0].boxed.is_none());
    let (x, y, w, h) = g.lis[1].boxed.expect("a pill");
    let run = &g.lis[1].runs[0];
    assert!(run.x > x && run.x + run.width < x + w, "{run:?} in {x} {w}");
    assert!(
        h > g.lis[0].h && y >= g.lis[0].y + g.lis[0].h,
        "{:?}",
        g.lis
    );
    let page = html::render(&d, &Default::default());
    assert!(page.contains("<li class=\"pill\">"), "{page}");
}

/// Two cards whose arrows to `c` share the join `http`, and a third card on its own.
const JOIN: &str = "diagram(dir=\"down\")
  card#a A
    to(c join=\"http\")
  card#b B
    to(c join=\"http\")
  card#c C
";

#[test]
fn arrows_with_one_named_join_share_a_trunk_into_their_card() {
    let d = build(JOIN);
    let g = &d.edge_groups;
    assert!(
        g.contains_key("a->c") && g.get("a->c") == g.get("b->c"),
        "{g:?}"
    );
    // both end at the same point of c
    let (ra, rb) = (&d.routes["a->c"], &d.routes["b->c"]);
    assert_eq!(ra.last(), rb.last(), "{ra:?} {rb:?}");
    // the shared end and its head are drawn once
    let heads = |id: &str| {
        let e = d.model.edges.iter().find(|e| e.id == id).unwrap();
        // in the drawing's frame, as the renderers hand it
        let line: Vec<(f64, f64)> = d.routes[id]
            .iter()
            .map(|p| (p.0 + d.frame.ox, p.1 + d.frame.oy))
            .collect();
        skemo_engine::arrow::unshared(&d, e, &line).1
    };
    assert_ne!(heads("a->c"), heads("b->c"));
}

#[test]
fn a_badge_goes_to_the_side_its_class_names() {
    let src =
        |cls: &str| format!("diagram(look=\"band\")\n  card#a{cls}(tag=\"8080\") A long name\n");
    let tag_x = |cls: &str| {
        let d = build(&src(cls));
        let h = card_geom(&d, "a").head.expect("a band");
        (h.tag.expect("a tag").x, h.type_run.x)
    };
    let (right, name) = tag_x("");
    assert!(right > name, "the tag after the name by default");
    let (left, name) = tag_x(".tag-left");
    assert!(left < name, "{left} before {name}");
}

#[test]
fn a_zone_tag_on_the_right_ends_the_title_row() {
    let src = |cls: &str| format!("zone#z{cls}(tag=\"10.0.0.0/8\") Net\n  card#a A\n  card#b B\n");
    let tag_x = |cls: &str| {
        let d = build(&src(cls));
        let z = d.boxes["z"];
        let t = &d.scene.zones["z"];
        let n = d.model.node("z").unwrap();
        let zt = geometry::zone_title(&d.scene, t, (&n.title, n.tag.as_deref()), z.w, z.h);
        zt.tag.expect("a tag").x
    };
    assert!(tag_x(".tag-right") > tag_x(""));
}

/// A wide zone of three cards under a zone of four, in a stack.
const WIDE: &str = "diagram(dir=\"down\")
  stack
    zone#top Top
      card#t1 One
      card#t2 Two
      card#t3 Three
      card#t4 Four
    zone#low.bare.wide
      card#a A
      card#b B
      card#c C
";

#[test]
fn a_wide_zone_spreads_its_row_over_the_group() {
    let d = build(WIDE);
    let w: Vec<f64> = ["a", "b", "c"].iter().map(|id| d.boxes[*id].w).collect();
    assert!(w.iter().all(|x| (x - w[0]).abs() < 1e-6), "{w:?}");
    let (top, a, c) = (d.boxes["top"], d.boxes["a"], d.boxes["c"]);
    assert!(
        a.x - top.x < 40.0 && top.right() - c.right() < 40.0,
        "{top:?} {a:?} {c:?}"
    );
}

#[test]
fn cards_of_one_column_share_an_axis_across_zones() {
    let src = "diagram(dir=\"down\")
  stack
    zone#up Up
      card#p.col-x Pod
      card#q.col-y Other
    zone#low.bare.wide
      card#m.col-y Machine Y
      card#n.col-x Machine X
";
    let d = build(src);
    let mid = |id: &str| d.boxes[id].x + d.boxes[id].w / 2.0;
    assert!(
        (mid("p") - mid("n")).abs() < 1.0,
        "{} {}",
        mid("p"),
        mid("n")
    );
    assert!(
        (mid("q") - mid("m")).abs() < 1.0,
        "{} {}",
        mid("q"),
        mid("m")
    );
}

#[test]
fn a_zone_title_set_below_keeps_its_room_below() {
    let src = "zone#z Services\n  card#a A\n";
    let css = "zone#z > .title { top: auto; bottom: 12px; }";
    let gap = |d: &skemo_engine::Diagram| {
        (
            d.boxes["a"].y - d.boxes["z"].y,
            d.boxes["z"].bottom() - d.boxes["a"].bottom(),
        )
    };
    let (above, below) = gap(&build(src));
    let (above2, below2) = gap(&build_css(src, &[css]));
    assert!(
        above2 < above && below2 > below,
        "{above} {below} → {above2} {below2}"
    );
}

#[test]
fn a_line_without_a_head_touches_its_card() {
    let d = build("diagram(dir=\"down\")\n  card#a A\n    to(b).nohead\n  card#b B\n");
    let pts: Vec<(f64, f64)> = d.routes["a->b"]
        .iter()
        .map(|p| (p.0 + d.frame.ox, p.1 + d.frame.oy))
        .collect();
    let a = skemo_engine::arrow::arrow(&pts);
    let line = skemo_engine::arrow::drawn_line(&d, "a->b", &a, &pts);
    assert_eq!(line.last(), pts.last(), "the line runs to the card");
    assert_ne!(a.line.last(), pts.last(), "a headed line stops at its head");
}

/// Two pods per machine for the first two machines, one for the third (D-054).
const COLUMNS: &str = "diagram(dir=\"down\")
  stack
    zone#pods Pods
      card#p1.col-a frontend-pod-number-one
      card#p2.col-a frontend-pod-number-two
      card#p3.col-b backend-pod-number-one
      card#p4.col-b backend-pod-number-two
      card#p5.col-c database-pod
    zone#svcs Services
      card#s1.col-a Frontend
        to(p1 p2)
      card#s2.col-b Backend
        to(p3 p4)
      card#s3.col-c Database
        to(p5)
    zone#nodes.bare.wide
      card#w1.col-a Worker 1
      card#w2.col-b Worker 2
      card#w3.col-c Worker 3
";

#[test]
fn every_block_of_a_column_fits_over_its_machine() {
    let d = build(COLUMNS);
    let b = &d.boxes;
    for (w, ids) in [("w1", ["p1", "p2"]), ("w2", ["p3", "p4"])] {
        let lo = b[ids[0]].x.min(b[ids[1]].x);
        let hi = b[ids[0]].right().max(b[ids[1]].right());
        assert!(
            lo >= b[w].x - 0.5 && hi <= b[w].right() + 0.5,
            "{ids:?} over {w}: {lo}..{hi} vs {:?}",
            b[w]
        );
    }
    // a service sits in the middle of its two pods
    let mid = |id: &str| b[id].x + b[id].w / 2.0;
    assert!((mid("s2") - (mid("p3") + mid("p4")) / 2.0).abs() < 1.0);
    // the machines share one width
    assert!((b["w1"].w - b["w3"].w).abs() < 1e-6);
}

#[test]
fn the_labels_of_two_joins_sit_on_their_shared_trunks() {
    let src = "diagram(dir=\"down\")
  stack
    zone#pods Pods
      card#a1 A1
        to(sa join=\"http\" constraint=\"false\") HTTP
      card#a2 A2
        to(sa join=\"http\" constraint=\"false\")
      card#b1 B1
        to(sb join=\"sql\" constraint=\"false\") SQL
      card#b2 B2
        to(sb join=\"sql\" constraint=\"false\")
    zone#svcs Services
      card#sa Service A
      card#sb Service B
";
    let d = build(src);
    let on_both = |id: &str, mate: &str| {
        let p = d.labels[id];
        let on = |r: &Vec<(f64, f64)>| {
            r.windows(2).any(|s| {
                let within = |v: f64, a: f64, b: f64| v >= a.min(b) - 0.5 && v <= a.max(b) + 0.5;
                within(p.0, s[0].0, s[1].0) && within(p.1, s[0].1, s[1].1)
            })
        };
        on(&d.routes[id]) && on(&d.routes[mate])
    };
    assert!(on_both("a1->sa", "a2->sa"), "{:?} {:?}", d.labels, d.routes);
    assert!(on_both("b1->sb", "b2->sb"), "{:?} {:?}", d.labels, d.routes);
}
