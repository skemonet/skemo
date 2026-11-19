//! `cards` and `zones` (piles): the cards (frames) behind take room right and down, the top card
//! keeps its siblings' width, and a `count` draws a `×N` badge at the same place in SVG and HTML.

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

/// A pile of zones (D-114): the frames behind stand outside the zone's box, which is the top
/// frame; the slide engine keeps them clear of the zone beside it and of the frame round it.
const ZONES: &str = "diagram
  zone#hub Hub
    card#acm ACM
      to(agent)
  zone#region Region
    zones#fleet(count=\"4\") Clusters
      card#agent klusterlet
      card#apps apps
    card#dns DNS
";

#[test]
fn a_pile_of_zones_keeps_its_frames_behind_clear() {
    let d = build(ZONES);
    let (fleet, region) = (d.boxes["fleet"], d.boxes["region"]);
    let (agent, apps) = (d.boxes["agent"], d.boxes["apps"]);
    // what it holds is in the top frame
    for c in [agent, apps] {
        assert!(
            c.x >= fleet.x && c.x + c.w <= fleet.x + fleet.w,
            "{c:?} {fleet:?}"
        );
        assert!(
            c.y >= fleet.y && c.y + c.h <= fleet.y + fleet.h,
            "{c:?} {fleet:?}"
        );
    }
    // the frames behind (10 px right and down) stay inside the frame round it, with room
    assert!(
        fleet.x + fleet.w + 10.0 < region.x + region.w,
        "{fleet:?} {region:?}"
    );
    assert!(
        fleet.y + fleet.h + 10.0 < region.y + region.h,
        "{fleet:?} {region:?}"
    );
    // and clear of every card and zone that is not its own
    for (id, b) in &d.boxes {
        if ["fleet", "agent", "apps", "region"].contains(&id.as_str()) {
            continue;
        }
        let apart = b.x >= fleet.x + fleet.w + 10.0
            || b.x + b.w <= fleet.x
            || b.y >= fleet.y + fleet.h + 10.0
            || b.y + b.h <= fleet.y;
        assert!(apart, "{id} {b:?} against the pile {fleet:?}");
    }
}

#[test]
fn a_pile_of_zones_draws_its_frames_and_badge() {
    let d = build(ZONES);
    let out = svg::render(&d);
    assert!(out.contains(">×4<"), "count badge");
    let page = html::render(&d, &Default::default());
    let z = page.split("<zone data-id=\"fleet\"").nth(1).unwrap();
    let z = &z[..z.find("</zone>").unwrap()];
    assert!(z.contains("deck") && z.contains("--deck-m:10px"), "{z}");
    assert!(z.contains("box-shadow:5px 5px 0") && z.contains("<span class=\"count\">×4</span>"));
}
