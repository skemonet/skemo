//! D-099 (2026-11-09, the README's example): a zone title wraps rather than pushing its zone out,
//! the zone stays centred on its cards, the layout keeps the router's margin from a title, and a
//! triangle `a → b → c` with `a → c` is drawn with its long arrow straight and mirrored hooks.

use skemo_engine::{Diagram, Options};
mod elk;
use elk::from_source;

const README: &str = r#"diagram(dir="down")

  card#internet.cloud Internet
    to(fw) WAN

  card#fw.firewall Firewall
    | 203.0.113.10/29
    to(web)
    to(base)

  zone#internal.internal Internal network
    card#web.server Web server
      | 10.0.0.10
      to(base)
    card#base.server Database
      | 10.0.0.20
"#;

fn diagram(src: &str) -> Diagram {
    from_source(src, &Options::default()).unwrap().0
}

fn mid(d: &Diagram, id: &str) -> f64 {
    d.boxes[id].x + d.boxes[id].w / 2.0
}

/// The horizontal steps of a route (x after − x before at each horizontal segment).
fn steps(d: &Diagram, id: &str) -> Vec<f64> {
    d.routes[id]
        .windows(2)
        .filter(|w| (w[0].1 - w[1].1).abs() < 0.5 && (w[0].0 - w[1].0).abs() > 0.5)
        .map(|w| w[1].0 - w[0].0)
        .collect()
}

#[test]
fn a_zone_title_in_the_way_of_an_arrow_wraps() {
    let d = diagram(README);
    assert_eq!(
        d.scene.title_wraps.get("internal").map(String::as_str),
        Some("Internal\nnetwork")
    );
    let svg = skemo_engine::svg::render(&d);
    assert!(
        svg.contains(">INTERNAL<") && svg.contains(">NETWORK<"),
        "{svg}"
    );
    let html = skemo_engine::html::render(&d, &Default::default());
    assert!(html.contains("Internal<br>network"));
    // no arrow comes in from above: the title stays on one line
    let d = diagram("diagram\n  zone#z Internal network\n    card#b B\n");
    assert!(d.scene.title_wraps.is_empty());
}

// bug 3 (the private corpus, 2026-11-09): a title or a pill wider than its zone went past the
// frame, under the legend or the zone beside it
#[test]
fn a_title_wider_than_its_zone_wraps_inside_its_frame() {
    let d = diagram("diagram\n  zone#z Hub, namespace of the managed cluster\n    card#b B\n");
    assert!(d.scene.title_wraps["z"].contains('\n'));
    assert!(skemo_engine::quality::defects(&d)["zone-title-overflow"].is_empty());
    // the zone keeps the width its card gives: the title adapts to the zone
    let (z, b) = (d.boxes["z"], d.boxes["b"]);
    assert!(z.w <= b.w + 61.0, "{z:?} {b:?}");
}

#[test]
fn a_pill_wider_than_its_zone_takes_a_line_and_widens_it() {
    let d = diagram(
        "diagram(dir=\"right\")\n  zone#a(tag=\"operator.open-cluster-management.io\") Installation\n    card#x X\n",
    );
    assert_eq!(
        d.scene.title_wraps.get("a").map(String::as_str),
        Some("Installation\n")
    );
    assert!(skemo_engine::quality::defects(&d)["zone-title-overflow"].is_empty());
    let html = skemo_engine::html::render(&d, &Default::default());
    assert!(html.contains("Installation<br><span class=\"tag\" style=\"margin-left:0\">"));
    // widened as much on both sides: its card stays in its middle
    let (z, x) = (d.boxes["a"], d.boxes["x"]);
    assert!(
        ((x.x - z.x) - (z.right() - x.right())).abs() < 1.0,
        "{z:?} {x:?}"
    );
}

// bug 16: a zone's description line wider than the zone ran out of its frame; it wraps (D-101)
#[test]
fn a_zone_description_wraps_inside_its_frame() {
    let d = diagram(
        "diagram\n  zone#a Search\n    | An index of the resources of every cluster of the fleet.\n    card#x X\n",
    );
    assert!(d.scene.zones["a"].subs[0].1.contains('\n'));
    assert!(skemo_engine::quality::defects(&d)["zone-title-overflow"].is_empty());
    let html = skemo_engine::html::render(&d, &Default::default());
    assert!(html.contains("<div class=\"sub\">An index"), "{html}");
}

// bug 12: an arrow coming down onto a card of a zone went round its description line (a wall
// for the lines that D-064 did not count); it now comes straight down
#[test]
fn an_arrow_into_a_zone_with_a_description_comes_straight() {
    let d = diagram(
        "diagram\n  card#a A\n    to(b)\n  zone#z Zone\n    | a line of description\n    card#b B\n",
    );
    assert_eq!(d.routes["a->b"].len(), 2, "{:?}", d.routes["a->b"]);
}

// the site's `cloud` example: an arrow comes into the subnet from above (D-064), whose zone
// widens for its title and pill rather than wrap them (a pill alone on its line weighs more)
#[test]
fn a_title_with_a_pill_that_fits_its_frame_keeps_one_line() {
    let d = diagram(
        "diagram(dir=\"down\")\n  card#i Internet\n    to(lb)\n  zone#pub(tag=\"10.0.0.0/24\") Public subnet\n    card#lb Load balancer\n    card#nat NAT gateway\n",
    );
    assert!(d.scene.title_wraps.is_empty(), "{:?}", d.scene.title_wraps);
}

#[test]
fn a_zone_widened_for_its_title_stays_centred_on_its_cards() {
    let d = diagram(README);
    let z = d.boxes["internal"];
    let left = ["web", "base"]
        .iter()
        .map(|k| d.boxes[*k].x)
        .fold(f64::INFINITY, f64::min);
    let right = ["web", "base"]
        .iter()
        .map(|k| d.boxes[*k].right())
        .fold(f64::NEG_INFINITY, f64::max);
    assert!(
        ((left - z.x) - (z.right() - right)).abs() < 1.0,
        "{z:?} {left} {right}"
    );
}

#[test]
fn a_triangle_has_a_straight_long_arrow_and_mirrored_hooks() {
    let d = diagram(README);
    // the long arrow: straight
    assert!(
        steps(&d, "fw->base").is_empty(),
        "{:?}",
        d.routes["fw->base"]
    );
    // the way through the middle card: one step aside, one step back, as long
    let (go, back) = (steps(&d, "fw->web"), steps(&d, "web->base"));
    assert_eq!((go.len(), back.len()), (1, 1), "{go:?} {back:?}");
    assert!((go[0] + back[0]).abs() < 0.5, "{go:?} {back:?}");
    // the first and last cards on one axis, the middle one aside
    assert!((mid(&d, "fw") - mid(&d, "base")).abs() < 0.5);
    assert!(mid(&d, "web") < mid(&d, "fw") - 20.0);
    // the long arrow keeps the router's margin from the middle card
    let x = d.routes["fw->base"][0].0;
    assert!(x - d.boxes["web"].right() >= 13.5, "{x}");
}

#[test]
fn the_hooks_mirror_each_other_outside_a_zone_too() {
    let src = r#"diagram(dir="down")
  card#a A wide card on top
    to(b)
    to(c)
  card#b B
    to(c)
  card#c C wide card at the bottom
"#;
    let d = diagram(src);
    assert!(steps(&d, "a->c").is_empty(), "{:?}", d.routes["a->c"]);
    let (go, back) = (steps(&d, "a->b"), steps(&d, "b->c"));
    assert_eq!((go.len(), back.len()), (1, 1), "{go:?} {back:?}");
    assert!((go[0] + back[0]).abs() < 0.5, "{go:?} {back:?}");
}

#[test]
fn a_chain_without_the_long_arrow_stays_in_one_column() {
    let plain = diagram(
        "diagram(dir=\"down\")\n  card#a A\n    to(b)\n  card#b B\n    to(c)\n  card#c C\n",
    );
    assert!((mid(&plain, "a") - mid(&plain, "b")).abs() < 0.5);
    assert!((mid(&plain, "b") - mid(&plain, "c")).abs() < 0.5);
}

#[test]
fn a_line_under_a_zone_title_does_not_step() {
    // the layout keeps the router's margin from the title: the port lies where the line runs
    let src = "diagram(dir=\"down\")\n  card#a Firewall\n    to(b)\n  zone#z Internal LAN\n    card#b Core switch\n";
    let d = diagram(src);
    assert!(steps(&d, "a->b").is_empty(), "{:?}", d.routes["a->b"]);
}
