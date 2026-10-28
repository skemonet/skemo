//! The language v0.8 in the engine (FORMAT §3.2.3–§3.2.9): lines with emphasis, tables,
//! category, states, legends, plain text, docking, subtitles, dates.

use skemo_engine::{Options, from_source, html, svg};

fn build(src: &str) -> skemo_engine::Diagram {
    from_source(src, &Options::default()).unwrap().0
}

fn shop() -> skemo_engine::Diagram {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data");
    let src = std::fs::read_to_string(dir.join("v08-shop.skemo")).unwrap();
    let css = std::fs::read_to_string(dir.join("v08-shop.css")).unwrap();
    let opts = Options {
        css: vec![css],
        ..Options::default()
    };
    from_source(&src, &opts).unwrap().0
}

#[test]
fn emphasis_draws_styled_pieces_and_marks() {
    let d = build("card#a A\n  | plain **bold** ==mark== `code`\n");
    let out = svg::render(&d);
    assert!(
        out.contains(">bold</text>") && out.contains("font-weight=\"700\""),
        "{out}"
    );
    assert!(out.contains(">mark</text>") && out.contains(">code</text>"));
    let page = html::render(&d, &Default::default());
    assert!(page.contains("<strong>bold</strong>") && page.contains("<mark>mark</mark>"));
    assert!(page.contains("<code>code</code>"));
}

#[test]
fn a_table_widens_its_card_and_spans_cells() {
    let d = build(
        "card#t T\n  | A | B | C |\n  |---|---|---|\n  | long first cell | x | y |\n  | ^^ | z | w |\n  | joined || v |\n",
    );
    let el = &d.scene.cards["t"];
    let b = d.boxes["t"];
    let g = skemo_engine::lines::table(&d.scene, el, 0);
    assert!(b.w >= g.w, "{} < {}", b.w, g.w);
    let tall = g
        .cells
        .iter()
        .find(|c| el.lis[c.li].cells[c.cell].rowspan == 2)
        .unwrap();
    let one = g
        .cells
        .iter()
        .find(|c| el.lis[c.li].cells[c.cell].text == "z")
        .unwrap();
    assert!((tall.h - 2.0 * one.h).abs() < 0.5);
    let wide = g
        .cells
        .iter()
        .find(|c| el.lis[c.li].cells[c.cell].colspan == 2)
        .unwrap();
    assert!(wide.w > one.w);
    let page = html::render(&d, &Default::default());
    assert!(
        page.contains("rowspan=\"2\"") && page.contains("colspan=\"2\"") && page.contains("<th")
    );
}

#[test]
fn the_category_sits_above_the_name_and_the_tag_in_the_chip() {
    let d = build("card#a(label=\"Load balancer\" tag=\"module.app\") shop\n");
    let el = &d.scene.cards["a"];
    let h = el.header.as_ref().unwrap();
    assert_eq!(h.kind.as_ref().map(|k| k.1.as_str()), Some("Load balancer"));
    assert_eq!(h.tag.as_ref().map(|t| t.1.as_str()), Some("module.app"));
    assert!(svg::render(&d).contains(">LOAD BALANCER</text>"));
}

#[test]
fn a_state_card_has_a_ring_and_a_mark() {
    let d = shop();
    assert!(d.scene.cards["lambda"].decor.icon.is_some());
    let out = svg::render(&d);
    // the ring: a stroke of the warn colour around the card
    assert!(out.contains("stroke=\"#A75207\""), "ring");
}

#[test]
fn legends_show_samples_and_fill_themselves() {
    let d = shop();
    let l = &d.scene.cards["flows"];
    assert!(
        l.lis
            .iter()
            .all(|li| matches!(li.sample, Some(skemo_engine::scene::Sample::Arrow { .. })))
    );
    let a = build("card#a.k A\n  to(b).e\ncard#b B\ncard#l.legend Key\n");
    assert!(a.scene.cards["l"].lis.is_empty(), "no --label, no line");
    let opts = Options {
        css: vec![".k { --label: \"Kind\"; } .edge.e { --label: \"flow\"; }".into()],
        ..Options::default()
    };
    let (a, _) = from_source(
        "card#a.k A\n  to(b).e\ncard#b B\ncard#l.legend Key\n",
        &opts,
    )
    .unwrap();
    let texts: Vec<&str> = a.scene.cards["l"]
        .lis
        .iter()
        .map(|l| l.rest.as_str())
        .collect();
    assert_eq!(texts, vec!["flow", "Kind"]);
}

#[test]
fn docked_elements_go_around_the_drawing() {
    let d = shop();
    let main_bottom = ["users", "vpc"]
        .iter()
        .map(|k| d.boxes[*k].bottom())
        .fold(0.0, f64::max);
    let main_right = ["users", "vpc"]
        .iter()
        .map(|k| d.boxes[*k].right())
        .fold(0.0, f64::max);
    let (sec, ops, flows) = (d.boxes["sec"], d.boxes["ops"], d.boxes["flows"]);
    assert!(sec.y > main_bottom && ops.y > main_bottom, "below");
    assert!(
        (sec.y - ops.y).abs() < 0.5 && ops.x > sec.right(),
        "side by side"
    );
    assert!(
        flows.x > main_right && flows.y <= d.boxes["vpc"].y + 0.5,
        "top right"
    );
    let note = d
        .model
        .nodes
        .iter()
        .find(|n| n.classes.iter().any(|c| c == "text"))
        .unwrap();
    assert!(
        d.boxes[&note.id].y > sec.bottom(),
        "the footnote on a row of its own"
    );
}

#[test]
fn an_arrow_into_a_docked_element_is_not_drawn() {
    let d = build("card#a A\n  to(x)\nzone#z(at=\"s\") Z\n  card#x X\n");
    assert!(d.model.edges.is_empty() && d.routes.is_empty());
}

#[test]
fn subtitle_and_zone_lines_take_room() {
    let d = shop();
    let out = svg::render(&d);
    assert!(out.contains("Account 123456789012") && out.contains("3 availability zones"));
    let plain = build("zone#z Z\n  card#a A\n");
    let lined = build("zone#z Z\n  | a description line\n  card#a A\n");
    let gap = |d: &skemo_engine::Diagram| d.boxes["a"].y - d.boxes["z"].y;
    assert!(gap(&lined) > gap(&plain) + 10.0);
}

#[test]
fn a_frieze_era_runs_to_the_end_of_its_end_year() {
    let d = build(
        "diagram(layout=\"frieze\" start=\"-600\" end=\"500\" scale=\"century\")\n  card#r(start=\"-509\" end=\"-27\") Republic\n  card#e(start=\"-27\" end=\"476\") Empire\n",
    );
    // the two eras share the year -27 (less than 2 px at century scale): one row
    assert!((d.boxes["r"].y - d.boxes["e"].y).abs() < 0.5);
}

/// A zone's description lines take room in the timeline and frieze modes too: the cards under
/// them move down by about their height.
#[test]
fn zone_description_lines_take_room_in_time_modes() {
    let y = |src: &str, id: &str| {
        let d = skemo_engine::from_source(src, &skemo_engine::Options::default())
            .unwrap()
            .0;
        d.boxes[id].y
    };
    let timeline = "diagram(layout=\"timeline\" start=\"2026-10\" end=\"2027-03\")\n  zone#eng.lane Engine\nLINES    card#e0(start=\"2026-10-16\" end=\"2026-11-07\") E0\n";
    let with = timeline.replace("LINES", "    | the layout\n    | two people\n");
    let without = timeline.replace("LINES", "");
    assert!(y(&with, "e0") - y(&without, "e0") > 30.0);
    let frieze = "diagram(layout=\"frieze\" start=\"1900\" end=\"2000\" scale=\"decade\")\n  card#iii(start=\"1900\" end=\"1940\") Third Republic\n  zone#up Up\n    card#a(date=\"1918\") A\n  zone#down Down\nLINES    card#b(date=\"1919\") B\n";
    let with = frieze.replace("LINES", "    | one line\n");
    let without = frieze.replace("LINES", "");
    assert!(y(&with, "b") - y(&without, "b") > 15.0);
}

/// A legend's line names its class's kind of sample: a frame for a zone type, the icon of a class
/// that sets one, a chip for a card type.
#[test]
fn legend_samples_follow_the_kind_of_class() {
    let src = "diagram\n  zone#z.lan LAN\n    card#a.pod A\n  card#b.t0 B\n  card#k.legend Key\n    |.lan zone\n    |.pod pod\n    |.t0 card\n";
    let opts = Options {
        css: vec![".pod { --c: #2F9E44; --icon: \"box\"; }".into()],
        ..Options::default()
    };
    let d = from_source(src, &opts).unwrap().0;
    let lis = &d.scene.cards["k"].lis;
    assert!(matches!(
        lis[1].sample,
        Some(skemo_engine::scene::Sample::Icon(_, _))
    ));
    assert!(matches!(
        lis[2].sample,
        Some(skemo_engine::scene::Sample::Chip(_))
    ));
    let svg = svg::render(&d);
    assert!(
        svg.contains("width=\"21\" height=\"11\""),
        "the zone sample, drawn inside its edge"
    );
    let page = html::render(&d, &html::HtmlOptions::default());
    assert!(page.contains("swatch zone-sample lan") && page.contains("icon sample-icon pod"));
}

/// Frieze: below the axis, a zone's title and description lines end before the stem of one of its
/// events: the zone widens to the left.
#[test]
fn a_stem_does_not_cross_its_zone_title() {
    let src = "diagram(layout=\"frieze\" start=\"1900\" end=\"2000\" scale=\"decade\")\n  card#iii(start=\"1900\" end=\"1940\") Third Republic\n  zone#up Up\n    card#a(date=\"1903\") A\n  zone#c A rather long zone title\n    | and a line under it\n    card#fair(date=\"1900-04-14\") World's Fair\n";
    let d = build(src);
    let (z, card) = (d.boxes["c"], d.boxes["fair"]);
    let stem = card.x + card.w / 2.0;
    // the title alone is wider than 150 px at the default size
    assert!(z.x + 16.0 + 150.0 < stem, "zone {z:?}, stem at {stem}");
}
