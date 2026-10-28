//! `Connector::ignore`: a connector crosses the shapes it ignores as if they were absent (a shape
//! not shown at the same time, in a diagram drawn in steps), while the others still go round them.

use skemo_router::{Connector, End, Obstacle, Params, Rect, route};

fn scene() -> Vec<Obstacle> {
    vec![
        Obstacle::shape("a", Rect::new(0.0, 0.0, 120.0, 40.0)),
        Obstacle::shape("b", Rect::new(0.0, 240.0, 120.0, 40.0)),
        // right between them
        Obstacle::shape("c", Rect::new(-40.0, 110.0, 200.0, 50.0)),
    ]
}

#[test]
fn an_ignored_shape_does_not_block_the_line() {
    let mut c = Connector::new("e", End::shape("a"), End::shape("b"));
    c.ignore = vec!["c".into()];
    let (routes, _) = route(&scene(), &[c], &Params::default());
    let p = &routes[0].points;
    assert_eq!(p.len(), 2, "straight through: {p:?}");
}

#[test]
fn a_shape_still_blocks_the_other_connectors() {
    let c = Connector::new("e", End::shape("a"), End::shape("b"));
    let (routes, _) = route(&scene(), &[c], &Params::default());
    let p = &routes[0].points;
    assert!(p.len() > 2, "round c: {p:?}");
}

#[test]
fn a_connector_never_ignores_its_own_ends() {
    let mut c = Connector::new("e", End::shape("a"), End::shape("b"));
    c.ignore = vec!["a".into(), "b".into(), "c".into()];
    let (routes, _) = route(&scene(), &[c], &Params::default());
    let p = &routes[0].points;
    // out of a's bottom, into b's top
    assert!(
        (p[0].1 - 40.0).abs() < 1e-6 && (p[p.len() - 1].1 - 240.0).abs() < 1e-6,
        "{p:?}"
    );
}

/// Two shapes not face to face: the line turns, and an ignored shape between them is no hole in
/// the graph either (it once was: only a straight line could cross it).
#[test]
fn a_bent_line_crosses_an_ignored_shape_too() {
    let obstacles = vec![
        Obstacle::shape("a", Rect::new(0.0, 0.0, 100.0, 40.0)),
        Obstacle::shape("b", Rect::new(300.0, 240.0, 100.0, 40.0)),
        // across the whole width between them
        Obstacle::shape("c", Rect::new(-200.0, 110.0, 800.0, 50.0)),
    ];
    let mut c = Connector::new("e", End::shape("b"), End::shape("a"));
    c.ignore = vec!["c".into()];
    let (routes, _) = route(&obstacles, &[c], &Params::default());
    let p = &routes[0].points;
    assert!(p.len() <= 4, "one step through c, not round it: {p:?}");
    assert!(p.iter().all(|q| q.0 > -150.0 && q.0 < 550.0), "{p:?}");
}
