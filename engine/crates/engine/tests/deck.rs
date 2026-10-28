//! `cards` (a pile): the cards behind take room right and down, the top card keeps its siblings'
//! width, and a `count` draws a `×N` badge at the same place in SVG and HTML.

use skemo_engine::{Options, from_source, html, svg};

const SRC: &str = "diagram(dir=\"down\")
  card#lb LB
    to(api w one)
  cards#api(count=\"4\") api
    li 4 tasks
  cards#w(count=\"2\") worker
    li 2 tasks
  card#one single
    li 1 task
";

fn build(src: &str) -> skemo_engine::Diagram {
    from_source(src, &Options::default()).unwrap().0
}

#[test]
fn a_pile_takes_the_room_of_its_cards_behind() {
    let d = build(SRC);
    let (api, w, one) = (d.boxes["api"], d.boxes["w"], d.boxes["one"]);
    // siblings share a width: the top cards are as wide as the plain card
    assert_eq!(api.w - 10.0, one.w, "{api:?} {one:?}");
    assert_eq!(w.w - 5.0, one.w, "{w:?} {one:?}");
    assert_eq!(api.h - 10.0, one.h);
}

#[test]
fn svg_draws_the_cards_behind_and_the_count() {
    let d = build(SRC);
    let out = svg::render(&d);
    assert!(out.contains(">×4<") && out.contains(">×2<"), "count badges");
    // the plain card has none
    assert_eq!(out.matches(">×").count(), 2);
}

#[test]
fn html_gives_the_pile_its_shadows_and_badge() {
    let d = build(SRC);
    let page = html::render(&d, &Default::default());
    let api = page.split("<card data-id=\"api\"").nth(1).unwrap();
    let api = &api[..api.find("</card>").unwrap()];
    assert!(api.contains("deck"), "{api}");
    assert!(api.contains("--deck-m:10px") && api.contains("box-shadow:5px 5px 0"));
    assert!(api.contains("<span class=\"count\">×4</span>"));
}

#[test]
fn a_pile_without_count_draws_three_cards_and_no_badge() {
    let d = build("diagram\n  cards#p pods\n  card#q other\n");
    assert_eq!(d.boxes["p"].w - 10.0, d.boxes["q"].w);
    assert!(!svg::render(&d).contains(">×"));
}
