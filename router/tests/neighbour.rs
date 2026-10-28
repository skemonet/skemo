//! Defect seen on the Terraform platform (2026-10-28): an arrow leaving a card by the side that
//! faces a neighbour 20 px away ran straight through that neighbour.

use skemo_router::{Connector, Dir, End, Obstacle, Params, Rect, route};

fn through(p: &[(f64, f64)], r: &Rect) -> bool {
    p.windows(2).any(|w| {
        let ((x1, y1), (x2, y2)) = (w[0], w[1]);
        let (lo_x, hi_x) = (x1.min(x2), x1.max(x2));
        let (lo_y, hi_y) = (y1.min(y2), y1.max(y2));
        lo_x < r.x + r.w && hi_x > r.x && lo_y < r.y + r.h && hi_y > r.y
    })
}

#[test]
fn no_arrow_through_a_close_neighbour() {
    let worker = Rect::new(1252.5, 969.0, 159.0, 119.0);
    let obstacles = vec![
        Obstacle::shape("api", Rect::new(1052.5, 940.25, 180.0, 145.0)),
        Obstacle::shape("worker", worker),
        Obstacle::shape("events", Rect::new(1431.5, 932.0, 154.0, 156.0)),
        Obstacle::shape("uploads", Rect::new(1723.5, 689.0, 280.0, 135.0)),
        Obstacle::shape("queue", Rect::new(1723.5, 872.0, 280.0, 114.0)),
    ];
    let conns = [
        Connector::new("u", End::shape("api"), End::shape("uploads")),
        Connector::new("q", End::shape("api"), End::shape("queue")),
    ];
    let params = Params {
        flow: Some(Dir::Down),
        flow_penalty: 150.0,
        port_spread: 42.0,
        ideal_nudging: 12.0,
        ..Params::default()
    };
    let (routes, _) = route(&obstacles, &conns, &params);
    for r in &routes {
        assert!(!through(&r.points, &worker), "{}: {:?}", r.id, r.points);
    }
    // the two still leave apart, in the corridor above the neighbour
    assert_ne!(routes[0].points[0], routes[1].points[0]);
}
