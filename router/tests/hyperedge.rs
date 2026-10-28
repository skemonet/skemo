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
