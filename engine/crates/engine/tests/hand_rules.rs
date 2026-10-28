//! Hand-drawn rules, second pass (D-049): arrows cross a gap half-way through the free space,
//! a shared trunk and its dot are drawn once, a back edge loops beyond the zone frame, a label
//! sits over a short arrow, sibling chips share a width.

use skemo_engine::{Options, from_source, svg};

fn build(src: &str) -> skemo_engine::Diagram {
    from_source(src, &Options::default()).unwrap().0
}

/// A card forking to two cards held by a zone.
const FORK: &str = "diagram(dir=\"down\")
  card#s Source
    to(a b)
  zone#z Targets
    card#a A
    card#b B
";

#[test]
fn a_fork_bar_runs_half_way_between_its_source_and_the_zone_frame() {
    let d = build(FORK);
    let r = &d.routes["s->a"];
    let bar = r
        .windows(2)
        .find(|w| (w[0].1 - w[1].1).abs() < 1e-6)
        .expect("a horizontal bar")[0]
        .1;
    let middle = f64::midpoint(d.boxes["s"].bottom(), d.boxes["z"].y);
    assert!((bar - middle).abs() <= 0.5, "bar at {bar}, middle {middle}");
}

#[test]
fn a_shared_trunk_and_its_dot_are_drawn_once() {
    let d = build(FORK);
    let out = svg::render(&d);
    assert_eq!(out.matches("class=\"junction\"").count(), 1, "{out}");
    // the second branch starts where it leaves the first one, not at the source
    let start = d.routes["s->a"][0];
    let lines: Vec<&str> = out
        .split("<path class=\"line\" d=\"M")
        .skip(1)
        .map(|s| s.split(' ').next().unwrap_or(""))
        .collect();
    let at_source = lines
        .iter()
        .filter(|m| **m == format!("{},{}", start.0 + d.frame.ox, start.1 + d.frame.oy))
        .count();
    assert_eq!(at_source, 1, "{lines:?}");
}

/// A back edge from a card outside a zone to a card inside it.
const BACK: &str = "diagram(dir=\"right\")
  zone#hr HR
    card#post Post
      to(screen)
    card#screen Screen
      to(decide)
  card#decide Decision
    to(hire) yes
    to(screen) no
  card#hire Hire
";

#[test]
fn a_back_edge_loops_beyond_the_zone_frame() {
    let d = build(BACK);
    let lowest = d.routes["decide->screen"]
        .iter()
        .map(|p| p.1)
        .fold(f64::MIN, f64::max);
    let frame = d.boxes["hr"].bottom();
    assert!(lowest >= frame + 20.0, "loop at {lowest}, frame at {frame}");
}

#[test]
fn a_label_sits_over_a_short_arrow() {
    let d = build(BACK);
    let line = d.routes["decide->hire"][0].1;
    let (_, y) = d.labels["decide->hire"];
    assert!(y < line - 5.0, "label at {y}, line at {line}");
}

#[test]
fn sibling_chips_share_a_width() {
    let d = build(
        "diagram(dir=\"right\")
  card#app App
    to(v10 v20)
  zone#vlans.bare Uplinks / VLAN
    card#v10.chip VLAN 10
    card#v20.chip VLAN 20000
",
    );
    assert_eq!(d.boxes["v10"].w, d.boxes["v20"].w);
}

/// A back edge to a card beside it, with no arrow out of it (an alarm below and left of the topic
/// it feeds): an L, out by the side facing the topic and into its bottom, not a loop round the row.
#[test]
fn a_back_edge_to_a_card_beside_it_turns_one_corner() {
    let d = build(
        "diagram(dir=\"down\")
  card#topic(at=\"300,24\") Topic
  card#alarm(at=\"24,200\") Alarm
    to(topic)
",
    );
    let r = &d.routes["alarm->topic"];
    let (a, t) = (d.boxes["alarm"], d.boxes["topic"]);
    assert_eq!(r.len(), 3, "{r:?}");
    assert!((r[0].0 - a.right()).abs() < 0.5, "{r:?}");
    assert!((r[2].1 - t.bottom()).abs() < 0.5, "{r:?}");
}

/// A label is tolerated on a zone's title row, but a free place on its arrow is preferred: it
/// reads as a second title (the language page's example, `transit` beside `Internal LAN`).
#[test]
fn a_label_leaves_a_zone_title_row_when_its_arrow_has_room() {
    let src = "diagram(dir=\"down\")
  card#fw.t0(tag=\"Edge\") Firewall
    li 10.0.0.1/24
    to(core).dashed transit
  zone#lan.lan Internal LAN
    card#core.t1 Core switch
      | (stack of two)
";
    let d = build(src);
    let (_, y) = d.labels["fw->core"];
    let lan = d.boxes["lan"];
    assert!(y < lan.y, "label at {y}, zone from {}", lan.y);
}

/// Two arrows of one join with the same label name the flow they share once (D-054).
#[test]
fn a_join_says_a_shared_label_once() {
    let d = build(
        "diagram(dir=\"down\")
  card#api Api
    to(db join=\"sql\") 5432
  card#worker Worker
    to(db join=\"sql\") 5432
  card#db Database
",
    );
    assert!(d.labels.contains_key("api->db"), "{:?}", d.labels);
    assert!(!d.labels.contains_key("worker->db"), "{:?}", d.labels);
}

/// A label wider than the gap between two cards goes beside its short arrow, clear of the cards,
/// rather than over both of them.
#[test]
fn a_label_wider_than_the_gap_goes_beside_its_arrow() {
    let d = build(
        "diagram(dir=\"right\")
  card#q(at=\"0,0\") Queue
    to(dlq) dead letters
  card#dlq(at=\"140,0\") DLQ
",
    );
    let (_, y) = d.labels["q->dlq"];
    let (q, dlq) = (d.boxes["q"], d.boxes["dlq"]);
    assert!(dlq.x - q.right() < 60.0, "{q:?} {dlq:?}");
    assert!(y < q.y || y > q.bottom(), "label at {y}, cards {q:?}");
}

/// A back edge between two cards of one zone loops round the row inside that zone: leaving it
/// to go round its frame (as an arrow from outside does) made a long detour out of the zone.
#[test]
fn a_back_edge_inside_a_zone_stays_in_it() {
    let d = build(
        "diagram(dir=\"down\")
  zone#z Services
    card#t Thumbnails
      to(a)
    card#a Assets
      to(u)
    card#u Uploads
      to(t) events
",
    );
    let z = d.boxes["z"];
    let r = &d.routes["u->t"];
    assert!(
        r.iter().all(|p| p.0 < z.right() && p.0 > z.x),
        "{r:?} {z:?}"
    );
}
