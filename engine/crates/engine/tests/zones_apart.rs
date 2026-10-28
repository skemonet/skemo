//! Sibling zones never laid over each other (2026-10-28, seen on the Terraform import's views and
//! on a hybrid estate): a zone grown by its shelf of cards with no arrow, by a `wrap` grid wider
//! than the room ELK kept, or an empty zone.

use skemo_engine::{Options, from_source};
use skemo_lang::Kind;

/// Pairs of zones, neither inside the other, whose frames overlap.
fn overlapping_zones(src: &str) -> Vec<(String, String)> {
    let d = from_source(src, &Options::default()).unwrap().0;
    let zones: Vec<&str> = d
        .model
        .nodes
        .iter()
        .filter(|n| n.kind == Kind::Zone)
        .map(|n| n.id.as_str())
        .collect();
    let nested = |a: &str, b: &str| d.model.ancestors(a).iter().any(|n| n.id == b);
    let mut out = Vec::new();
    for (i, a) in zones.iter().enumerate() {
        for b in &zones[i + 1..] {
            if nested(a, b) || nested(b, a) {
                continue;
            }
            let (p, q) = (&d.boxes[*a], &d.boxes[*b]);
            if p.x < q.right() && q.x < p.right() && p.y < q.bottom() && q.y < p.bottom() {
                out.push((a.to_string(), b.to_string()));
            }
        }
    }
    out
}

#[test]
fn a_shelf_wider_than_its_zone_pushes_the_next_zone() {
    // two machines with no arrow under a broker: their shelf is wider than the broker
    let src = r#"diagram(dir="down")
  zone#a A
    card#src source
      to(mqtt) uses
  zone#ot OT
    card#mqtt mqtt
    card#vm1 scada gateway
      | line one
      | line two
    card#vm2 historian
      | line one
      | line two
  zone#core Core
    zone#dmz DMZ
      card#p proxy
    zone#app APP
      card#e erp app
"#;
    assert_eq!(overlapping_zones(src), Vec::<(String, String)>::new());
}

#[test]
fn a_grid_wider_than_its_room_pushes_the_next_zone() {
    let src = r#"diagram(dir="down")
  card#cdn CDN
    to(assets constraint="false") static
  zone#region Region
    zone#svc(wrap="2") Managed services
      card#uploads uploads
        | versioned
        to(fn) events
      card#assets assets
        | versioned
      card#fn thumbnails
        to(assets) writes
      card#logs logs
        | access logs
    zone#vpc VPC
      zone#pub Public
        card#bastion bastion
      zone#priv Private
        card#api api
          to(uploads constraint="false") writes
          to(db) 5432
      zone#data Data
        card#db orders
"#;
    assert_eq!(overlapping_zones(src), Vec::<(String, String)>::new());
}

#[test]
fn an_empty_zone_keeps_its_own_room() {
    let src = r#"diagram(dir="down")
  card#lb load balancer
    to(api) 443
  zone#vpc VPC
    zone#data Data subnets
      | 0.0.0.0/0 → nat
    zone#pub Public subnets
      card#bastion bastion
      card#nat nat gateway
    zone#priv Private subnets
      card#api api
"#;
    assert_eq!(overlapping_zones(src), Vec::<(String, String)>::new());
}

#[test]
fn an_inventory_is_laid_as_a_grid() {
    let src = "diagram(dir=\"down\")\n  card#x X\n    to(y)\n  card#y Y\n  zone#ops Ops\n    card#a A\n    card#b B\n    card#c C\n    card#d D\n    card#e E\n";
    let d = from_source(src, &Options::default()).unwrap().0;
    let rows: std::collections::BTreeSet<i64> = ["a", "b", "c", "d", "e"]
        .iter()
        .map(|id| d.boxes[*id].y.round() as i64)
        .collect();
    // five cards with no arrow: three columns, two rows (not one row of five)
    assert_eq!(rows.len(), 2, "{rows:?}");
}
