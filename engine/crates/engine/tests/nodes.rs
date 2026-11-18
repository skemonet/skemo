//! `node` (D-087): the icon in a column of its own, one size for every node, centred on the text;
//! the looks leave nodes alone; arrows end on the icon.

use skemo_engine::{Options, from_source, html, svg};

const SRC: &str = "diagram(dir=\"right\" look=\"header\")
  node#user(icon=\"user\") Customer
    to(api)
  card#api.t2(tag=\"ECS\") API
    | 3 tasks
    to(db)
  node#db(icon=\"database\") Orders
    | PostgreSQL 16
    | Multi-AZ, encrypted
";

fn build(src: &str) -> skemo_engine::Diagram {
    let (d, warnings) = from_source(src, &Options::default()).unwrap();
    assert!(warnings.is_empty(), "{warnings:?}");
    d
}

#[test]
fn every_node_icon_has_one_size_and_a_column() {
    let d = build(SRC);
    // a one-line node is as tall as its icon; a node of three lines, as its text
    assert_eq!(d.boxes["user"].h, 32.0, "{:?}", d.boxes["user"]);
    assert!(d.boxes["db"].h > 50.0, "{:?}", d.boxes["db"]);
    let out = svg::render(&d);
    let icons: Vec<&str> = out
        .match_indices("class=\"icon\"")
        .map(|(k, _)| &out[k..k + 120])
        .collect();
    assert_eq!(icons.len(), 2, "{icons:?}");
    // 32 px on a 24 px drawing, with the thinner line of large icons
    assert!(icons.iter().all(|i| i.contains("scale(1.33)")), "{icons:?}");
    assert!(
        icons.iter().all(|i| i.contains("stroke-width=\"1.88\"")),
        "{icons:?}"
    );
}

#[test]
fn the_looks_leave_a_node_alone() {
    let d = build(SRC);
    let page = html::render(&d, &Default::default());
    let db = page.split("<card data-id=\"db\"").nth(1).unwrap();
    let db = &db[..db.find("</card>").unwrap()];
    assert!(
        db.contains("node") && !db.contains("look-header") && !db.contains("class=\"head\""),
        "{db}"
    );
    // the icon outside the title, in the column the left padding makes (32 + 12 px)
    assert!(db.contains("padding-left:44px"), "{db}");
    assert!(
        db.find("class=\"icon\"") < db.find("class=\"title\""),
        "{db}"
    );
    let api = page.split("<card data-id=\"api\"").nth(1).unwrap();
    assert!(api[..api.find('>').unwrap()].contains("look-header"));
}

#[test]
fn arrows_end_at_the_middle_of_the_icon() {
    // on the icon: before it level with its middle, or above or below it at its middle (the
    // slide engine places the node where its links ask)
    let d = build(SRC);
    let b = d.boxes["db"];
    let end = *d.routes["api->db"].last().unwrap();
    let (top, mid) = (b.y + (b.h - 32.0) / 2.0, b.y + b.h / 2.0);
    let before = (end.0 - b.x).abs() < 1.0 && (end.1 - mid).abs() < 1.0;
    let on_top = ((end.1 - top).abs() < 1.0 || (end.1 - top - 32.0).abs() < 1.0)
        && end.0 > b.x
        && end.0 < b.x + 32.0;
    assert!(before || on_top, "{end:?} {b:?}");
    // drawn down: from above, on the top of the icon (centred on the text), at its middle
    let d = build(
        "diagram(dir=\"down\")\n  card#api API\n    to(db)\n  node#db(icon=\"database\") Orders\n    | PostgreSQL 16\n",
    );
    let b = d.boxes["db"];
    let end = *d.routes["api->db"].last().unwrap();
    let top = b.y + (b.h - 32.0) / 2.0;
    assert!(
        (end.0 - (b.x + 16.0)).abs() < 1.0 && (end.1 - top).abs() < 1.0,
        "{end:?} {b:?}"
    );
}
