//! R-18: the connectors of one group share a port and a trunk; without a group, they are spread
//! along the side (R-09) as usual.

use skemo_router::{Connector, End, Obstacle, Params, Rect, Route, route};

fn scene(grouped: bool) -> Vec<Route> {
    let obstacles = [
        Obstacle::shape("src", Rect::new(200.0, 0.0, 200.0, 60.0)),
        Obstacle::shape("a", Rect::new(0.0, 260.0, 120.0, 60.0)),
        Obstacle::shape("b", Rect::new(240.0, 260.0, 120.0, 60.0)),
        Obstacle::shape("c", Rect::new(480.0, 260.0, 120.0, 60.0)),
    ];
    let connectors: Vec<Connector> = ["a", "b", "c"]
        .iter()
        .map(|t| {
            let mut c = Connector::new(format!("src->{t}"), End::shape("src"), End::shape(*t));
            c.group = grouped.then(|| "src|trunk".to_string());
            c
        })
        .collect();
    let (routes, diags) = route(&obstacles, &connectors, &Params::default());
    assert!(diags.is_empty(), "{diags:?}");
    routes
}

#[test]
fn grouped_connectors_share_the_same_trunk() {
    let routes = scene(true);
    let start = routes[0].points[0];
    // same port, and a first vertical stretch on the same line
    for r in &routes {
        assert_eq!(r.points[0], start, "same port: {}", r.id);
        assert_eq!(r.points[1].0, start.0, "first stretch vertical: {}", r.id);
    }
    // the common trunk has a length: the first (nearest) bend is far from the shape
    let bend = routes
        .iter()
        .map(|r| r.points[1].1)
        .fold(f64::INFINITY, f64::min);
    assert!(bend > start.1 + 20.0, "tronc non nul : {bend}");
    // ... then they part towards their targets
    let ends: Vec<_> = routes.iter().map(|r| *r.points.last().unwrap()).collect();
    assert!(ends[0] != ends[1] && ends[1] != ends[2]);
}

#[test]
fn ungrouped_connectors_are_spread_along_the_side() {
    let routes = scene(false);
    let mut starts: Vec<f64> = routes.iter().map(|r| r.points[0].0).collect();
    starts.sort_by(f64::total_cmp);
    starts.dedup();
    assert_eq!(starts.len(), 3, "trois accroches distinctes : {starts:?}");
}

#[test]
fn trunk_branches_are_still_orthogonal_and_clear_of_shapes() {
    for r in scene(true) {
        for w in r.points.windows(2) {
            assert!(
                w[0].0 == w[1].0 || w[0].1 == w[1].1,
                "segment oblique dans {}",
                r.id
            );
        }
    }
}

#[test]
fn branches_enter_by_the_side_facing_the_trunk_and_split_at_one_point() {
    // targets close to the source and spread wide: the shortest way in is by their inner side
    let obstacles = [
        Obstacle::shape("src", Rect::new(165.0, 240.0, 190.0, 60.0)),
        Obstacle::shape("a", Rect::new(0.0, 40.0, 190.0, 60.0)),
        Obstacle::shape("b", Rect::new(330.0, 40.0, 190.0, 60.0)),
    ];
    let connectors: Vec<Connector> = ["a", "b"]
        .iter()
        .map(|t| {
            let mut c = Connector::new(format!("src->{t}"), End::shape("src"), End::shape(*t));
            c.group = Some("src|trunk".to_string());
            c
        })
        .collect();
    let (routes, diags) = route(&obstacles, &connectors, &Params::default());
    assert!(diags.is_empty(), "{diags:?}");
    for r in &routes {
        let end = *r.points.last().unwrap();
        assert!((end.1 - 100.0).abs() < 1e-6, "{} enters at {end:?}", r.id);
    }
    // the horizontal legs of the two branches are at the same height
    let split: Vec<f64> = routes.iter().map(|r| r.points[1].1).collect();
    assert!((split[0] - split[1]).abs() < 1e-6, "{split:?}");
}

/// A source with a card right under it and two targets below that card, side by side: the
/// branches go round the card together (`late`: `Params::late_split`). Returns the routes.
fn round_a_card(late: bool) -> Vec<Route> {
    let obstacles = [
        Obstacle::shape("src", Rect::new(300.0, 0.0, 200.0, 60.0)),
        Obstacle::shape("mid", Rect::new(300.0, 120.0, 200.0, 60.0)),
        Obstacle::shape("a", Rect::new(60.0, 300.0, 200.0, 60.0)),
        Obstacle::shape("b", Rect::new(300.0, 300.0, 200.0, 60.0)),
    ];
    let connectors: Vec<Connector> = ["a", "b"]
        .iter()
        .map(|t| {
            let mut c = Connector::new(format!("src->{t}"), End::shape("src"), End::shape(*t));
            c.group = Some("src|trunk".to_string());
            c
        })
        .collect();
    let params = Params {
        late_split: late,
        ..Params::default()
    };
    let (routes, diags) = route(&obstacles, &connectors, &params);
    assert!(diags.is_empty(), "{diags:?}");
    routes
}

/// The last point of `r` on the polyline `on`, walking from its start: where `r` leaves it.
fn leaves(r: &Route, on: &Route) -> (f64, f64) {
    let lies = |p: (f64, f64)| {
        on.points.windows(2).any(|w| {
            let (a, b) = (w[0], w[1]);
            (p.0 - a.0) * (p.0 - b.0) <= 1e-6
                && (p.1 - a.1) * (p.1 - b.1) <= 1e-6
                && ((a.0 - b.0).abs() < 1e-6 || (a.1 - b.1).abs() < 1e-6)
        })
    };
    let mut last = r.points[0];
    for w in r.points.windows(2) {
        if !lies(w[1]) {
            // the run from w[0] towards w[1] may still lie on `on` for a while
            let steps = 64;
            for k in 1..=steps {
                let t = k as f64 / steps as f64;
                let p = (
                    w[0].0 + (w[1].0 - w[0].0) * t,
                    w[0].1 + (w[1].1 - w[0].1) * t,
                );
                if lies(p) {
                    last = p;
                } else {
                    return last;
                }
            }
        }
        last = w[1];
    }
    last
}

#[test]
fn a_branch_leaves_the_trunk_as_late_as_its_way_allows() {
    let routes = round_a_card(true);
    let split = leaves(&routes[1], &routes[0]);
    // the branches part below the card between the source and the targets, not at the source
    assert!(split.1 > 180.0, "split at {split:?}: {routes:?}");
    let early = round_a_card(false);
    let split = leaves(&early[1], &early[0]);
    assert!(
        split.1 < 120.0,
        "without it, at the source: {split:?}: {early:?}"
    );
}
