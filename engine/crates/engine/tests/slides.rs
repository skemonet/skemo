//! Feedback from diagrams made for slides (D-041): frieze stems and axis end, writing order in a
//! zone, arrows written after their target, `wrap`, `fit`, `--font-scale`, animated chip, steps
//! and fonts in the SVG.

use skemo_engine::{Options, from_source};

fn build(src: &str) -> skemo_engine::Diagram {
    from_source(src, &Options::default()).unwrap().0
}

fn with_css(src: &str, css: &str) -> skemo_engine::Diagram {
    let opts = Options {
        css: vec![css.to_string()],
        ..Options::default()
    };
    from_source(src, &opts).unwrap().0
}

const FRISE: &str = "diagram(layout=\"frieze\" start=\"2010\" end=\"2017\" scale=\"year\")
  card#a(start=\"2010\" end=\"2014\") Phase one
  card#e1(date=\"2011\") Launch
  card#e2(date=\"2012\") Second
  card#e3(date=\"2013\") Third
";

#[test]
fn frieze_axis_end_is_included() {
    let d = build(FRISE);
    let fr = d.frise.as_ref().unwrap();
    // `end="2017"` is included (v0.8): 8 years at 60 px, the axis ends on the 2018 graduation
    assert!((fr.width - 8.0 * 60.0).abs() < 1e-6, "{}", fr.width);
    assert_eq!(fr.ticks.last().unwrap().1, "2018");
}

#[test]
fn frieze_stems_below_the_axis_start_on_it() {
    let d = build(FRISE);
    let fr = d.frise.as_ref().unwrap();
    let below: Vec<_> = fr.stems.iter().filter(|s| s.y2 > 0.0).collect();
    assert!(!below.is_empty());
    for s in below {
        // on the axis, or just under a year label that sits on the stem's way
        let on_tick = fr.ticks.iter().any(|(x, _)| (x - s.x).abs() < 20.0);
        assert!(
            s.y1 == 0.0 || (on_tick && s.y1 < 25.0),
            "stem {} starts at {}",
            s.id,
            s.y1
        );
    }
}

#[test]
fn frieze_without_eras_keeps_no_empty_band_row() {
    let src = FRISE.replace("  card#a(start=\"2010\" end=\"2014\") Phase one\n", "");
    let d = build(&src);
    assert!(d.frise.as_ref().unwrap().band_bottom <= 22.0);
}

#[test]
fn frieze_bands_and_labels_grow_with_the_font_scale() {
    let base = build(FRISE);
    let big = with_css(FRISE, ".stage { --font-scale: 1.6; }");
    assert!(big.boxes["a"].h > base.boxes["a"].h);
    assert!(big.frise.unwrap().band_bottom > base.frise.unwrap().band_bottom);
}

const PODS: &str = "zone#ns Namespace
  card#p1 pod-1
  card#p2 pod-2
  card#p3 pod-3
card#n.note Ports 8080
  to(p3)
card#svc Service
  to(p1)
  to(p2)
  to(p3)
";

#[test]
fn a_zone_keeps_the_writing_order_when_a_note_is_added() {
    let d = build(PODS);
    let x = |id: &str| d.boxes[id].x;
    assert!(x("p1") < x("p2") && x("p2") < x("p3"), "{:?}", d.boxes);
}

#[test]
fn an_arrow_written_after_its_target_still_points_down() {
    let d = build(PODS);
    assert!(d.boxes["svc"].bottom() <= d.boxes["ns"].y, "{:?}", d.boxes);
}

#[test]
fn routes_stay_orthogonal() {
    let d = build(PODS);
    for (id, r) in &d.routes {
        for w in r.windows(2) {
            let (a, b) = (w[0], w[1]);
            assert!(
                (a.0 - b.0).abs() < 0.01 || (a.1 - b.1).abs() < 0.01,
                "{id}: slanted segment {a:?} → {b:?}"
            );
        }
    }
}

const WRAP: &str = "card#lb Load balancer
  to(p1)
zone#ns(wrap=\"3\") Namespace
  card#p1 pod-1
  card#p2 pod-2
  card#p3 pod-3
  card#p4 pod-4
  card#p5 pod-5
  card#p6 pod-6
  card#p7 pod-7
";

#[test]
fn wrap_lays_a_zone_out_on_a_grid() {
    let d = build(WRAP);
    let b = |i: usize| d.boxes[&format!("p{i}")];
    for row in [[1, 2, 3], [4, 5, 6]] {
        assert!(row.iter().all(|&i| (b(i).y - b(row[0]).y).abs() < 1.0));
        assert!(b(row[0]).x < b(row[1]).x && b(row[1]).x < b(row[2]).x);
    }
    assert!(b(4).y > b(1).bottom() && b(7).y > b(4).bottom());
    // columns aligned
    for (top, below) in [(1, 4), (2, 5), (3, 6), (1, 7)] {
        assert!((b(top).x - b(below).x).abs() < 1.0);
    }
    // no route for the invisible wrap edges
    assert!(d.routes.keys().all(|k| !k.starts_with("__wrap")));
}

#[test]
fn wrap_across_the_direction_makes_columns() {
    let d = build(&format!("diagram(dir=\"right\")\n{WRAP}"));
    let b = |i: usize| d.boxes[&format!("p{i}")];
    assert!((b(1).x - b(2).x).abs() < 1.0 && b(1).y < b(2).y);
    assert!(b(4).x > b(1).right());
}

#[test]
fn fit_suggests_settings_that_show_the_text_larger() {
    use skemo_engine::fit::{TOLERANCE, apply, parse_target, size_of, suggestions};
    let src = WRAP.replace("(wrap=\"3\")", "");
    let src = format!("diagram(fit=\"16:9\")\n{src}");
    let d = build(&src);
    assert!(
        d.warnings
            .iter()
            .any(|w| w.contains("fit=") && w.contains("larger")),
        "{:?}",
        d.warnings
    );
    let target = parse_target("16:9").unwrap();
    let opts = Options::default();
    let now = target.scale(size_of(&src, &opts).unwrap());
    let list = suggestions(&src, &opts, target);
    assert!(
        list.iter().all(|s| s.scale > now),
        "only candidates that enlarge the text"
    );
    let best = list.first().expect("a suggestion");
    assert!(best.scale >= now * TOLERANCE);
    let applied = apply(&src, &best.settings).unwrap();
    assert_eq!(
        size_of(&applied, &opts),
        Some(best.size),
        "what is suggested is what you get"
    );
}

#[test]
fn fit_in_a_box_states_the_text_size() {
    let src = WRAP.replace("(wrap=\"3\")", "");
    let d = build(&format!("diagram(fit=\"1280x720\")\n{src}"));
    assert!(
        d.warnings.iter().any(|w| w.contains(" px → ")),
        "{:?}",
        d.warnings
    );
}

#[test]
fn fit_suggests_nothing_when_no_setting_enlarges_the_text() {
    // two stacked cards in a square: side by side would be wider, hence smaller; the diagram
    // is still off the target, which is said (D-045), without a setting to try
    let d = build("diagram(fit=\"1:1\")\ncard#a A\n  to(b)\ncard#b B\n");
    let fit: Vec<&String> = d.warnings.iter().filter(|w| w.contains("fit=")).collect();
    assert_eq!(fit.len(), 1, "{:?}", d.warnings);
    assert!(!fit[0].contains("--write"), "{fit:?}");
}

#[test]
fn font_scale_enlarges_the_text_and_the_cards() {
    let src = "card#a A rather long title that is wider than the minimum width of a card\n  li a line of text\n";
    let base = build(src);
    let big = with_css(src, ".stage { --font-scale: 1.4; }");
    assert!(big.boxes["a"].w > base.boxes["a"].w * 1.2);
    assert!(big.boxes["a"].h > base.boxes["a"].h + 8.0);
    let html = skemo_engine::html::render(&big, &skemo_engine::html::HtmlOptions::default());
    assert!(html.contains("var(--font-scale, 1)"));
}

#[test]
fn a_custom_class_beats_the_frieze_title_rule() {
    let src = FRISE.replace("card#e1(", "card#e1.jalon(");
    let base = build(&src);
    let big = with_css(&src, ".jalon .title { font-size: 20px; }");
    assert!(big.sizes.cards["e1"].h > base.sizes.cards["e1"].h);
}

const TAG: &str = "card#pod(tag=\"8080 @1\") pod-a
  li 10.0.0.1 @2
";

#[test]
fn the_chip_appears_at_its_own_step() {
    let d = build(TAG);
    assert_eq!(d.model.nodes[0].tag.as_deref(), Some("8080"));
    assert_eq!(d.anim.elements["pod/tag"].appear.raw, "1");
    let svg = skemo_engine::svg::render(&d);
    assert!(svg.contains("<g class=\"tag\" data-step=\"1\""), "{svg}");
    let html = skemo_engine::html::render(&d, &skemo_engine::html::HtmlOptions::default());
    assert!(html.contains("<span class=\"tag\" data-step=\"1\""));
}

#[test]
fn svg_lists_its_steps_and_can_embed_its_fonts() {
    let d = build(TAG);
    let svg = skemo_engine::svg::render(&d);
    let states = svg
        .split("data-states=\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap();
    assert!(
        states.contains("&quot;1&quot;") && states.contains("&quot;2&quot;"),
        "{states}"
    );
    assert!(!svg.contains("@font-face"));
    assert!(svg.contains("font-family=\"Inter, 'Helvetica Neue', Arial, sans-serif\""));
    let embedded = skemo_engine::svg::render_with_fonts(&d);
    assert!(embedded.contains("@font-face{font-family:'Inter'"));
    let still = build("card#a A\n");
    assert!(!skemo_engine::svg::render(&still).contains("data-states"));
}

#[test]
fn a_still_image_shows_the_last_step() {
    // D-069: lines that replace each other are not drawn on top of each other
    let d = build(
        "card#svc Service\n  li type: NodePort @0-1\n  li type: ClusterIP @2\ncard#pod Pod @1\n  to(svc)\n",
    );
    let raster = skemo_engine::svg::render_for_raster(&d);
    assert!(raster.contains("ClusterIP") && !raster.contains("NodePort"));
    assert!(raster.contains(">Pod<"), "{raster}");
    let svg = skemo_engine::svg::render(&d);
    assert!(svg.contains("ClusterIP") && svg.contains("NodePort"));
}
