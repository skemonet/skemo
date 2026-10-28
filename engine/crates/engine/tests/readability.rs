//! Readable arrows (D-044): zone titles are not crossed, cards of different heights are linked
//! by straight lines, `wrap` warns when it hides cards from their arrows, and `fit` never
//! suggests a setting that does.

use skemo_engine::{Options, from_source};

fn build(src: &str) -> skemo_engine::Diagram {
    from_source(src, &Options::default()).unwrap().0
}

/// The diagram that prompted these rules: a taller card (a line reserved for step 2) between
/// two short ones, and a `wrap` zone fed from the left.
const NET: &str = "diagram(dir=\"right\")
card#internet Internet
  to(magicarpe)
zone#lb Loadbalancer
  card#magicarpe Magicarpe @2:focus
    li Out of order @2
    to(switch)
card#switch Switch
  to(evoli aquali voltali pyroli join=\"trunk\")
zone#web(wrap=\"2\") Web
  card#evoli Evoli
  card#aquali Aquali
  card#voltali Voltali
  card#pyroli Pyroli
";

fn bends(d: &skemo_engine::Diagram, id: &str) -> usize {
    d.routes[id].len() - 2
}

#[test]
fn a_taller_card_is_reached_by_straight_lines() {
    let d = build(NET);
    assert_eq!(bends(&d, "internet->magicarpe"), 0, "{:?}", d.routes);
    assert_eq!(bends(&d, "magicarpe->switch"), 0, "{:?}", d.routes);
}

#[test]
fn arrows_go_beside_zone_titles() {
    let d = build(&NET.replace("dir=\"right\"", "dir=\"down\""));
    let titles = skemo_engine::labels::zone_titles(&d.model, &d.scene, &d.boxes);
    assert!(!titles.is_empty());
    for (id, r) in &d.routes {
        for w in r.windows(2) {
            let (x0, x1) = (w[0].0.min(w[1].0), w[0].0.max(w[1].0));
            let (y0, y1) = (w[0].1.min(w[1].1), w[0].1.max(w[1].1));
            for (z, t) in &titles {
                let hit = x1 > t.x && x0 < t.right() && y1 > t.y && y0 < t.bottom();
                assert!(!hit, "{id} crosses the title of {z}: {r:?}");
            }
        }
    }
    // and still enters the card from the top, the side facing its source
    let r = &d.routes["internet->magicarpe"];
    let end = r[r.len() - 1];
    assert!((end.1 - d.boxes["magicarpe"].y).abs() < 0.5, "{r:?}");
}

#[test]
fn wrap_warns_when_it_hides_cards_from_their_arrows() {
    let d = build(NET);
    let w: Vec<_> = d.warnings.iter().filter(|w| w.contains("wrap=")).collect();
    assert_eq!(w.len(), 1, "{:?}", d.warnings);
    assert!(
        w[0].contains("voltali") && w[0].contains("pyroli"),
        "{}",
        w[0]
    );
    assert!(
        !w[0].contains("evoli,") && !w[0].contains("aquali"),
        "{}",
        w[0]
    );
    // one column in the direction of the arrows: nothing hidden
    let d = build(&NET.replace("(wrap=\"2\")", ""));
    assert!(
        d.warnings.iter().all(|w| !w.contains("wrap=")),
        "{:?}",
        d.warnings
    );
}

#[test]
fn fit_never_suggests_hiding_cards() {
    use skemo_engine::fit::{apply, parse_target, shape_of, suggestions};
    let src = NET.replace("(wrap=\"2\")", "");
    let opts = Options::default();
    for s in suggestions(&src, &opts, parse_target("16:9").unwrap()) {
        let applied = apply(&src, &s.settings).unwrap();
        let shape = shape_of(&applied, &opts).unwrap();
        assert_eq!(shape.hidden, 0, "{:?}", s.settings);
        assert_eq!(shape.bends, s.bends);
    }
    assert!(shape_of(NET, &opts).unwrap().hidden > 0);
}
