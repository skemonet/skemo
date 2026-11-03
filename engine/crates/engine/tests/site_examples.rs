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
