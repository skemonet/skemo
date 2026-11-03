//! Defects seen while drawing the site's examples (2026-11-03, `docs/research/site-examples/` in
//! the meta repository).

use skemo_engine::{Options, from_source};

fn build(src: &str) -> skemo_engine::Diagram {
    from_source(src, &Options::default()).unwrap().0
}

fn cx(d: &skemo_engine::Diagram, id: &str) -> f64 {
    let b = d.boxes[id];
    b.x + b.w / 2.0
}

/// The manager with teams written between two others: ELK set it last, the writing order brought
/// it back to the middle and its teams stayed under the last manager.
const ORG: &str = "diagram(dir=\"down\")
  card#ceo CEO
    to(cto cpo cfo join=\"trunk\")
  card#cto Alex Moreau
    to(platform apps join=\"trunk\")
  card#cpo Sam Leroy
    to(design research join=\"trunk\")
  card#cfo Robin Petit
  card#platform Platform team
  card#apps Apps team
  card#design Design
  card#research User research
";

#[test]
fn teams_stay_under_their_manager_when_the_row_is_reordered() {
    let d = build(ORG);
    // managers in writing order
    assert!(cx(&d, "cto") < cx(&d, "cpo") && cx(&d, "cpo") < cx(&d, "cfo"));
    for (m, a, b) in [("cto", "platform", "apps"), ("cpo", "design", "research")] {
        let mid = (cx(&d, a) + cx(&d, b)) / 2.0;
        assert!((cx(&d, m) - mid).abs() < 1.0, "{m} over its teams");
    }
    // the teams of the middle manager end before the last manager's left edge
    assert!(d.boxes["research"].right() < d.boxes["cfo"].x + d.boxes["cfo"].w);
}

/// The cloud example: Internet, outside the VPC, above its only target, the load balancer of a
/// subnet whose title would have been crossed before the zones made room for it (D-064).
const CLOUD: &str = "diagram(dir=\"down\")
  card#users(icon=\"globe\") Internet
    to(lb) HTTPS 443
  zone#vpc(tag=\"10.0.0.0/16\") Production VPC
    | eu-west-1 · 3 availability zones
    zone#public(tag=\"10.0.0.0/24\") Public subnet
      card#lb(icon=\"network\") Load balancer
        | TLS 1.3
        to(app) 8080
      card#nat NAT gateway
        | outbound only
    zone#private(tag=\"10.0.1.0/24\") Private subnet
      card#app App servers
        to(nat constraint=\"false\").dashed updates
";

#[test]
fn a_card_outside_a_zone_stands_above_its_only_target_in_it() {
    let d = build(CLOUD);
    assert!(
        (cx(&d, "users") - cx(&d, "lb")).abs() < 1.0,
        "straight above"
    );
    // stacked subnets as wide as each other (D-085)
    assert_eq!(d.boxes["public"].w, d.boxes["private"].w);
}

/// The app servers reach out through the NAT of the public subnet, which feeds them: without
/// `constraint="false"`, the subnets keep their writing order (public above private).
#[test]
fn zones_linked_both_ways_keep_their_writing_order() {
    let d = build(&CLOUD.replace("to(nat constraint=\"false\")", "to(nat)"));
    assert!(
        d.boxes["public"].bottom() <= d.boxes["private"].y,
        "public above private"
    );
    assert!((cx(&d, "users") - cx(&d, "lb")).abs() < 1.0);
}
