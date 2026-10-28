//! R-12: everything that cannot be done as asked is reported, without panic; and the incremental
//! API (removing obstacles and connectors).

use skemo_router::{Connector, Diagnostic, End, Obstacle, Params, Rect, Router, Side, route};

fn shapes() -> Vec<Obstacle> {
    vec![
        Obstacle::shape("a", Rect::new(0.0, 0.0, 100.0, 50.0)),
        Obstacle::shape("b", Rect::new(300.0, 0.0, 100.0, 50.0)),
    ]
}

fn conn() -> Connector {
    Connector::new("a->b", End::shape("a"), End::shape("b"))
}

#[test]
fn an_enclosed_shape_is_unroutable_and_reported() {
    let mut obs = shapes();
    // four walls closing a ring around b
    for (i, r) in [
        Rect::new(260.0, -40.0, 180.0, 20.0),
        Rect::new(260.0, 70.0, 180.0, 20.0),
        Rect::new(260.0, -40.0, 20.0, 130.0),
        Rect::new(420.0, -40.0, 20.0, 130.0),
    ]
    .into_iter()
    .enumerate()
    {
        obs.push(Obstacle::shape(format!("wall{i}"), r));
    }
    let (routes, diags) = route(&obs, &[conn()], &Params::default());
    assert!(routes.is_empty(), "{routes:?}");
    assert!(
        diags
            .iter()
            .any(|d| matches!(d, Diagnostic::Unroutable { id, .. } if id == "a->b")),
        "{diags:?}"
    );
}

#[test]
fn a_blocked_side_is_ignored_and_reported() {
    let mut obs = shapes();
    // a wall flush against the left side of a: the forced side cannot be used
    obs.push(Obstacle::shape("wall", Rect::new(-30.0, -20.0, 30.0, 90.0)));
    let c = Connector::new("a->b", End::shape_side("a", Side::Left), End::shape("b"));
    let (routes, diags) = route(&obs, &[c], &Params::default());
    assert_eq!(routes.len(), 1, "{diags:?}");
    assert!(
        diags
            .iter()
            .any(|d| matches!(d, Diagnostic::SideIgnored { id, at_start: true } if id == "a->b")),
        "{diags:?}"
    );
}

#[test]
fn an_unreachable_waypoint_is_dropped_with_a_diagnostic() {
    let mut obs = shapes();
    obs.push(Obstacle::shape(
        "block",
        Rect::new(150.0, 100.0, 60.0, 60.0),
    ));
    let mut c = conn();
    c.checkpoints = vec![(180.0, 130.0)]; // inside an obstacle
    let (routes, diags) = route(&obs, &[c], &Params::default());
    assert_eq!(routes.len(), 1);
    assert!(
        diags.iter().any(
            |d| matches!(d, Diagnostic::CheckpointUnreachable { id, index: 0 } if id == "a->b")
        ),
        "{diags:?}"
    );
}

#[test]
fn invalid_input_is_reported_not_fatal() {
    let mut obs = shapes();
    obs.push(Obstacle::shape("bad", Rect::new(f64::NAN, 0.0, 10.0, 10.0)));
    obs.push(Obstacle::shape("a", Rect::new(0.0, 0.0, 10.0, 10.0))); // duplicate id
    let missing = Connector::new("x->y", End::shape("nope"), End::shape("b"));
    let (routes, diags) = route(&obs, &[conn(), missing], &Params::default());
    assert!(routes.iter().any(|r| r.id == "a->b"));
    assert!(
        diags
            .iter()
            .any(|d| matches!(d, Diagnostic::InvalidInput { .. })),
        "{diags:?}"
    );
}

#[test]
fn removing_an_obstacle_or_a_connector_updates_the_routes() {
    let mut r = Router::new(Params::default());
    for o in shapes() {
        r.set_obstacle(o);
    }
    r.set_obstacle(Obstacle::shape(
        "wall",
        Rect::new(180.0, -40.0, 40.0, 130.0),
    ));
    r.set_connector(conn());
    r.update();
    let detour = r.routes().next().unwrap().points.clone();
    assert!(detour.len() > 2, "goes around the wall: {detour:?}");

    r.remove_obstacle("wall");
    let changed = r.update();
    assert_eq!(changed.len(), 1);
    assert_eq!(r.routes().next().unwrap().points.len(), 2, "straight again");
    r.remove_obstacle("unknown"); // no effect
    assert!(r.update().is_empty());

    // modifying a connector re-routes it, removing it drops its route
    let mut c = conn();
    c.checkpoints = vec![(200.0, 200.0)];
    r.set_connector(c);
    assert_eq!(r.update().len(), 1);
    r.remove_connector("a->b");
    r.update();
    assert_eq!(r.routes().count(), 0);
    r.remove_connector("a->b"); // idempotent
    assert!(r.diagnostics().is_empty());
}

#[test]
fn reasons_have_neutral_english_texts() {
    use skemo_router::Reason;
    assert_eq!(Reason::NoPath.to_string(), "no path");
    assert_eq!(
        Reason::NoClearSide("a".into()).to_string(),
        "no side of « a » is clear"
    );
    let obs = shapes();
    let missing = Connector::new("x->y", End::shape("nope"), End::shape("b"));
    let (_, diags) = route(&obs, &[missing], &Params::default());
    assert!(
        diags.iter().any(|d| matches!(d, Diagnostic::Unroutable { reason: Reason::UnknownShape(id), .. } if id == "nope")),
        "{diags:?}"
    );
}
