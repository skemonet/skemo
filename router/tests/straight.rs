//! R-20 and R-21: a straight line between two facing shapes of different heights, arrows both
//! ways kept straight, and a port moved beside a shape that sits right in front of the middle of
//! a side (a zone title above a card).

use skemo_router::{Connector, End, Obstacle, Params, Rect, route};

fn conn(id: &str, from: &str, to: &str) -> Connector {
    Connector::new(id, End::shape(from), End::shape(to))
}

#[test]
fn facing_shapes_of_different_heights_get_a_straight_line() {
    // b is taller and lower: the middles differ by 21 px, the sides overlap
    let obstacles = vec![
        Obstacle::shape("a", Rect::new(0.0, 100.0, 120.0, 36.0)),
        Obstacle::shape("b", Rect::new(200.0, 97.0, 120.0, 78.0)),
    ];
    let (routes, _) = route(&obstacles, &[conn("e", "a", "b")], &Params::default());
    assert_eq!(routes[0].points, vec![(120.0, 118.0), (200.0, 118.0)]);
}

#[test]
fn arrows_both_ways_stay_straight_and_apart() {
    let obstacles = vec![
        Obstacle::shape("a", Rect::new(0.0, 100.0, 120.0, 36.0)),
        Obstacle::shape("b", Rect::new(200.0, 86.0, 120.0, 78.0)),
    ];
    let conns = [conn("ab", "a", "b"), conn("ba", "b", "a")];
    let (routes, _) = route(&obstacles, &conns, &Params::default());
    let ys: Vec<f64> = routes
        .iter()
        .map(|r| {
            assert_eq!(r.points.len(), 2, "{}: {:?}", r.id, r.points);
            assert_eq!(r.points[0].1, r.points[1].1);
            r.points[0].1
        })
        .collect();
    assert!((ys[0] - ys[1]).abs() >= 12.0, "{ys:?}");
}

#[test]
fn a_shape_in_front_of_the_middle_moves_the_port_aside() {
    // a title-like shape just above the middle of b's top side; a comes from above
    let obstacles = vec![
        Obstacle::shape("a", Rect::new(0.0, 0.0, 190.0, 36.0)),
        Obstacle::shape("title", Rect::new(-10.0, 120.0, 120.0, 20.0)),
        Obstacle::shape("b", Rect::new(0.0, 148.0, 190.0, 78.0)),
    ];
    let (routes, _) = route(&obstacles, &[conn("e", "a", "b")], &Params::default());
    let p = &routes[0].points;
    let end = *p.last().unwrap();
    assert_eq!(end.1, 148.0, "enters from the top: {p:?}");
    assert!(end.0 >= 110.0, "beside the title: {p:?}");
}
