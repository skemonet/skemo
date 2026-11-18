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
    // a nested zone: drawn by the grid engine, still drawn
    let d = slide(
        "diagram(layout=\"slide\")
  zone#a Alpha
    zone#a1 Inner
      card#x X
        to(y)
    card#y Y
",
    );
    assert!(d.routes.contains_key(&d.model.edges[0].id));
}

#[test]
fn the_layout_says_which_engine_drew_it() {
    let d = slide(MAP);
    assert_eq!(d.layout_json()["layout"]["engine"], "slide");
}
