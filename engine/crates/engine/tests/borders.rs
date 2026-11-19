//! Layout by borders and alignment (D-117): `at="n|s|e|w"` inside a group, as Java's
//! `BorderLayout`; `dir` and `align` on a container; `block`, a group drawn as nothing that the
//! other lines go round; a docked element linked to the rest keeps its arrows.

use skemo_engine::{Options, from_source, svg};

fn draw(src: &str) -> skemo_engine::Diagram {
    from_source(src, &Options::default()).unwrap().0
}

const HUB: &str = "diagram
  zone#hub Hub
    card#api(at=\"n\") API gateway
      to(s1)
      to(s2)
    block(dir=\"right\")
      card#s1 Service 1
        to(db)
      card#s2 Service 2
        to(db)
      card#s3 Service 3
    card#db(at=\"s\") Database
    card#log(at=\"e\") Logs
  card#user(at=\"w\") User
    to(api)
";

#[test]
fn docked_children_stand_round_the_rest() {
    let d = draw(HUB);
    assert_eq!(d.layout_json()["layout"]["engine"], "slide");
    let b = |id: &str| d.boxes[id];
    let (api, db, log, user) = (b("api"), b("db"), b("log"), b("user"));
    for s in ["s1", "s2", "s3"] {
        assert!(api.y + api.h <= b(s).y, "north: {api:?} {:?}", b(s));
        assert!(b(s).y + b(s).h <= db.y, "south: {db:?} {:?}", b(s));
        assert!(b(s).x + b(s).w <= log.x, "east: {log:?} {:?}", b(s));
    }
    assert!(
        user.x + user.w <= d.boxes["hub"].x,
        "west of the drawing: {user:?}"
    );
    // the services on one row, in writing order
    assert!(b("s1").y == b("s2").y && b("s2").y == b("s3").y);
    assert!(b("s1").x < b("s2").x && b("s2").x < b("s3").x);
}

#[test]
fn a_docked_element_keeps_its_arrows() {
    let d = draw(HUB);
    assert!(d.routes.contains_key("user->api"), "{:?}", d.routes.keys());
}

#[test]
fn a_block_draws_nothing() {
    let d = draw(HUB);
    let out = svg::render(&d);
    assert!(
        !out.contains("data-id=\"block"),
        "a block's own group drawn"
    );
}

#[test]
fn dir_down_lays_a_column() {
    let d = draw("diagram\n  zone#z(dir=\"down\") Z\n    card#a A\n    card#b B\n    card#c C\n");
    let (a, b, c) = (d.boxes["a"], d.boxes["b"], d.boxes["c"]);
    assert!(a.x == b.x && b.x == c.x, "{a:?} {b:?} {c:?}");
    assert!(a.y < b.y && b.y < c.y);
}

#[test]
fn other_lines_go_round_a_block() {
    let src = "diagram(dir=\"right\")
  stack
    card#a Source
      to(z)
    block(dir=\"down\")
      card#m1 Middle 1
      card#m2 Middle 2
    card#z Target
";
    let d = draw(src);
    let (m1, m2) = (d.boxes["m1"], d.boxes["m2"]);
    let (x0, y0) = (m1.x.min(m2.x), m1.y.min(m2.y));
    let (x1, y1) = (
        (m1.x + m1.w).max(m2.x + m2.w),
        (m1.y + m1.h).max(m2.y + m2.h),
    );
    for &(x, y) in &d.routes["a->z"] {
        assert!(
            !(x0 <= x && x <= x1 && y0 <= y && y <= y1),
            "({x}, {y}) in the block"
        );
    }
}
