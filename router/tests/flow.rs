//! Layered drawings (D-047): trunks that branch in the drawing's direction, a preferred flow,
//! ports spread on roomy sides, no two arrows on one line at a card.

use skemo_router::{Connector, Dir, End, Obstacle, Params, Rect, Route, route};

fn routes(obstacles: &[Obstacle], conns: &[Connector], params: &Params) -> Vec<Route> {
    let (r, diags) = route(obstacles, conns, params);
    assert!(diags.is_empty(), "{diags:?}");
    r
}

fn conn(id: &str, from: &str, to: &str, group: Option<&str>) -> Connector {
    let mut c = Connector::new(id, End::shape(from), End::shape(to));
    c.group = group.map(str::to_string);
    c
}

#[test]
fn a_trunk_branches_downwards_when_every_target_is_below() {
    // one target right below, the other far to the right: the centroid is to the right, the
    // trunk still goes down
    let obstacles = [
        Obstacle::shape("s", Rect::new(0.0, 0.0, 190.0, 40.0)),
        Obstacle::shape("a", Rect::new(0.0, 100.0, 190.0, 40.0)),
        Obstacle::shape("b", Rect::new(900.0, 100.0, 190.0, 40.0)),
    ];
    let conns = [
        conn("s->a", "s", "a", Some("g")),
        conn("s->b", "s", "b", Some("g")),
    ];
    for r in routes(&obstacles, &conns, &Params::default()) {
        assert!(
            (r.points[0].1 - 40.0).abs() < 1e-6,
            "{} leaves at {:?}",
            r.id,
            r.points[0]
        );
    }
}

#[test]
fn the_flow_prefers_bottom_out_and_top_in() {
    // b is below and to the right: without a flow the cheapest is out by the right side
    let obstacles = [
        Obstacle::shape("a", Rect::new(0.0, 0.0, 190.0, 40.0)),
        Obstacle::shape("b", Rect::new(400.0, 120.0, 190.0, 40.0)),
    ];
    let conns = [conn("a->b", "a", "b", None)];
    let params = Params {
        flow: Some(Dir::Down),
        flow_penalty: 150.0,
        ..Params::default()
    };
    let r = &routes(&obstacles, &conns, &params)[0];
    let (s, e) = (r.points[0], *r.points.last().unwrap());
    assert!((s.1 - 40.0).abs() < 1e-6, "out at the bottom: {s:?}");
    assert!((e.1 - 120.0).abs() < 1e-6, "in at the top: {e:?}");
}

#[test]
fn ports_spread_on_a_roomy_side() {
    let obstacles = [
        Obstacle::shape("s", Rect::new(200.0, 0.0, 190.0, 40.0)),
        Obstacle::shape("a", Rect::new(0.0, 160.0, 120.0, 40.0)),
        Obstacle::shape("b", Rect::new(480.0, 160.0, 120.0, 40.0)),
    ];
    let conns = [conn("s->a", "s", "a", None), conn("s->b", "s", "b", None)];
    let gap = |spread: f64| {
        let p = Params {
            flow: Some(Dir::Down),
            flow_penalty: 150.0,
            port_spread: spread,
            ..Params::default()
        };
        let r = routes(&obstacles, &conns, &p);
        assert!(
            r.iter().all(|r| (r.points[0].1 - 40.0).abs() < 1e-6),
            "both out at the bottom"
        );
        (r[0].points[0].0 - r[1].points[0].0).abs()
    };
    assert!(gap(0.0) < 20.0, "default: ideal spacing, {}", gap(0.0));
    assert!((gap(42.0) - 42.0).abs() < 1.0, "spread: {}", gap(42.0));
}

#[test]
fn an_arrow_never_leaves_on_the_line_another_one_arrives_on() {
    // two cards one above the other, each also linked across to a far card: the arrow leaving
    // the upper card and the one arriving at the lower card must not share a vertical line
    let obstacles = [
        Obstacle::shape("w1", Rect::new(0.0, 0.0, 190.0, 40.0)),
        Obstacle::shape("w2", Rect::new(500.0, 0.0, 190.0, 40.0)),
        Obstacle::shape("sa", Rect::new(0.0, 120.0, 190.0, 40.0)),
        Obstacle::shape("sb", Rect::new(500.0, 120.0, 190.0, 40.0)),
    ];
    let conns = [
        conn("w1->sa", "w1", "sa", None),
        conn("w1->sb", "w1", "sb", None),
        conn("w2->sa", "w2", "sa", None),
        conn("w2->sb", "w2", "sb", None),
    ];
    let params = Params {
        flow: Some(Dir::Down),
        flow_penalty: 150.0,
        port_spread: 42.0,
        ..Params::default()
    };
    let rs = routes(&obstacles, &conns, &params);
    for a in &rs {
        for b in &rs {
            if a.id == b.id {
                continue;
            }
            let (s, t) = (
                (a.points[0], a.points[1]),
                (b.points[b.points.len() - 1], b.points[b.points.len() - 2]),
            );
            if (s.0.0 - t.0.0).abs() < 1.0
                && (s.0.0 - s.1.0).abs() < 1e-6
                && (t.0.0 - t.1.0).abs() < 1e-6
            {
                let lo = s.0.1.min(s.1.1).max(t.0.1.min(t.1.1));
                let hi = s.0.1.max(s.1.1).min(t.0.1.max(t.1.1));
                assert!(hi - lo <= 1.0, "{} and {} share a line", a.id, b.id);
            }
        }
    }
}
