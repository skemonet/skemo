//! R-15: a hint route (ELK's) usable as is is taken; otherwise it is ignored and the route is
//! computed as usual.

use skemo_router::{Connector, End, Obstacle, Params, Point, Rect, route};

fn obstacles() -> Vec<Obstacle> {
    vec![
        Obstacle::shape("a", Rect::new(0.0, 0.0, 100.0, 50.0)),
        Obstacle::shape("b", Rect::new(300.0, 0.0, 100.0, 50.0)),
    ]
}

fn run(hint: Option<Vec<Point>>, checkpoints: Vec<Point>, extra: Option<Obstacle>) -> Vec<Point> {
    let mut obs = obstacles();
    obs.extend(extra);
    let mut c = Connector::new("a->b", End::shape("a"), End::shape("b"));
    c.hint = hint;
    c.checkpoints = checkpoints;
    let (routes, diags) = route(&obs, &[c], &Params::default());
    assert!(diags.is_empty(), "{diags:?}");
    routes[0].points.clone()
}

fn is_orthogonal(p: &[Point]) -> bool {
    p.windows(2).all(|w| w[0].0 == w[1].0 || w[0].1 == w[1].1)
}

#[test]
fn a_usable_hint_is_kept() {
    let hint = vec![(100.0, 25.0), (300.0, 25.0)];
    assert_eq!(run(Some(hint.clone()), vec![], None), hint);
}

#[test]
fn a_hint_that_crosses_an_obstacle_is_dropped() {
    let wall = Obstacle::shape("wall", Rect::new(180.0, -40.0, 40.0, 130.0));
    let hint = vec![(100.0, 25.0), (300.0, 25.0)];
    let got = run(Some(hint.clone()), vec![], Some(wall));
    assert_ne!(got, hint);
    assert!(is_orthogonal(&got), "{got:?}");
    assert!(
        got.iter()
            .all(|p| !(p.0 > 180.0 && p.0 < 220.0 && p.1 > -40.0 && p.1 < 90.0))
    );
}

#[test]
fn an_oblique_or_detached_hint_is_dropped() {
    let oblique = vec![(100.0, 25.0), (300.0, 40.0)];
    let got = run(Some(oblique.clone()), vec![], None);
    assert_ne!(got, oblique);
    assert!(is_orthogonal(&got), "{got:?}");
    // not touching the shape: the end is far from the border
    let detached = vec![(150.0, 25.0), (300.0, 25.0)];
    assert_ne!(run(Some(detached.clone()), vec![], None), detached);
}

#[test]
fn a_hint_is_ignored_when_waypoints_are_requested() {
    let hint = vec![(100.0, 25.0), (300.0, 25.0)];
    let got = run(Some(hint.clone()), vec![(200.0, 150.0)], None);
    assert_ne!(got, hint);
    assert!(got.iter().any(|p| p.1 >= 150.0), "{got:?}");
}

#[test]
fn non_finite_or_degenerate_hints_do_not_panic() {
    for h in [
        vec![(f64::NAN, 0.0), (300.0, 25.0)],
        vec![(100.0, 25.0)],
        vec![],
        vec![(100.0, 25.0), (100.0, 25.0)],
    ] {
        let got = run(Some(h), vec![], None);
        assert!(got.len() >= 2 && is_orthogonal(&got), "{got:?}");
    }
}

#[test]
fn a_sub_pixel_gap_to_the_border_is_absorbed() {
    // ELK rounds: the end sits 0.4 px off the border, the segment stays perpendicular
    let hint = vec![(100.4, 25.0), (300.0, 25.0)];
    assert_eq!(
        run(Some(hint), vec![], None),
        vec![(100.0, 25.0), (300.0, 25.0)]
    );
}
