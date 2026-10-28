//! Public API: model, themes, style tree, CSS values and edit errors.

use skemo_lang::edit::{self, EditError, Target};
use skemo_lang::style::{
    MediaContext, Rgba, StyleTree, Styler, Stylesheet, Theme, parse_color, parse_length, styler,
    substitute, unquote,
};
use skemo_lang::{Kind, parse};
use std::collections::BTreeMap;

const SRC: &str = "diagram(dir=\"down\" layout=\"graph\")\n  zone#z.lan Z\n    card#a A\n      to(b) lien\n    card#b B\n  card#c C\n";

#[test]
fn model_navigation() {
    let m = parse(SRC).model;
    assert_eq!(m.option("dir"), Some("down"));
    assert_eq!(m.option("absent"), None);
    let roots: Vec<&str> = m.children(None).map(|n| n.id.as_str()).collect();
    assert_eq!(roots, ["z", "c"]);
    let inside: Vec<&str> = m.children(Some("z")).map(|n| n.id.as_str()).collect();
    assert_eq!(inside, ["a", "b"]);
    assert_eq!(
        m.ancestors("a")
            .iter()
            .map(|n| n.id.as_str())
            .collect::<Vec<_>>(),
        ["z"]
    );
    assert!(m.ancestors("c").is_empty());
    assert!(m.ancestors("inconnu").is_empty());
    assert_eq!(m.node("z").unwrap().kind, Kind::Zone);
    let json = m.to_json();
    assert!(json.contains("\"id\": \"a\"") && json.contains("\"to\": \"b\""));
}

#[test]
fn themes_round_trip_through_their_names() {
    for t in [Theme::Light, Theme::Dark, Theme::Print, Theme::Auto] {
        assert_eq!(Theme::parse(t.name()), Some(t));
        assert_eq!(t.class(), format!("theme-{}", t.name()));
    }
    assert_eq!(Theme::parse("sepia"), None);
}

#[test]
fn style_tree_helpers_and_typed_accessors() {
    let s: Styler = styler(&[
        ".x { --c: #123456; padding: 2px 4px; border-left: thick solid red; max-width: 50%; }",
    ]);
    let mut t = StyleTree::new();
    assert!(t.is_empty());
    let stage = t.add(None, "div", Some("stage"), &["stage"]);
    let card = t.add(Some(stage), "card", Some("k"), &[]);
    t.add_class(card, "x");
    t.add_class(card, "x"); // no duplicate
    t.set_attr(card, "data-k", "1");
    t.set_attr(card, "data-k", "2"); // replaced
    t.set_inline(card, "margin-top", "7px");
    t.set_inline(card, "margin-top", "9px");
    assert_eq!(t.len(), 2);
    let c = s.compute(&t);
    assert_eq!(c[card].padding(), [2.0, 4.0, 2.0, 4.0]);
    assert_eq!(
        c[card].margin()[0],
        9.0,
        "the inline value replaces the first"
    );
    assert_eq!(c[card].border()[3], 5.0, "thick");
    assert_eq!(c[card].string("--c").as_deref(), Some("#123456"));
    assert_eq!(c[card].var_color("--c").unwrap().to_css(), "#123456");
    assert_eq!(c[card].border_color(3).to_css(), "#FF0000");
    assert!(
        c[card].border_color(99).a > 0.0,
        "an index past the end is clamped"
    );
    assert!(!c[card].italic());
    assert!(!c[card].font_family().is_empty());
    assert_eq!(c[card].display(), "block");
}

#[test]
fn colors_parse_in_every_notation() {
    let cur = Rgba {
        r: 1.0,
        g: 2.0,
        b: 3.0,
        a: 1.0,
    };
    assert_eq!(parse_color("CurrentColor", cur), Some(cur));
    assert_eq!(parse_color("transparent", cur), Some(Rgba::TRANSPARENT));
    assert_eq!(parse_color("#abc", cur).unwrap().to_css(), "#AABBCC");
    assert_eq!(
        parse_color("#aabbcc80", cur).unwrap().to_css(),
        "rgba(170, 187, 204, 0.502)"
    );
    assert_eq!(parse_color("#abcd", cur).unwrap().a.round(), 1.0);
    assert_eq!(
        parse_color("rgb(10, 20, 30)", cur).unwrap().to_css(),
        "#0A141E"
    );
    assert_eq!(parse_color("rgb(10 20 30 / 50%)", cur).unwrap().a, 0.5);
    assert_eq!(parse_color("rgba(10, 20, 30, 0.25)", cur).unwrap().a, 0.25);
    assert_eq!(parse_color("rgb(10 20)", cur), None);
    assert_eq!(parse_color("#12", cur), None);
    assert_eq!(parse_color("pas-une-couleur", cur), None);
    let mix = parse_color("color-mix(in srgb, #000000 50%, #ffffff)", cur).unwrap();
    assert!((mix.r - 127.5).abs() < 1.0, "{mix:?}");
    assert_eq!(parse_color("color-mix(in lab, red, blue)", cur), None);
}

#[test]
fn colour_maths() {
    let white = Rgba {
        r: 255.0,
        g: 255.0,
        b: 255.0,
        a: 1.0,
    };
    let black = Rgba {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };
    assert!((white.luminance() - 1.0).abs() < 1e-9 && black.luminance() == 0.0);
    assert!((white.contrast(black) - 21.0).abs() < 1e-6);
    let half = Rgba { a: 0.5, ..black };
    assert_eq!(half.over(white).to_css(), "#808080");
    assert!(half.to_css_precise().starts_with("rgba("));
}

#[test]
fn lengths_in_every_unit() {
    assert_eq!(parse_length("0", 14.0, None), Some(0.0));
    assert_eq!(parse_length("12px", 14.0, None), Some(12.0));
    assert_eq!(parse_length("2em", 14.0, None), Some(28.0));
    assert_eq!(parse_length("1.5rem", 14.0, None), Some(24.0));
    assert_eq!(parse_length("3pt", 14.0, None), Some(4.0));
    assert_eq!(parse_length("50%", 14.0, Some(200.0)), Some(100.0));
    assert_eq!(parse_length("50%", 14.0, None), None);
    assert_eq!(parse_length("abc", 14.0, None), None);
}

#[test]
fn variables_and_strings() {
    let vars: BTreeMap<String, String> = [("--a".to_string(), "10px".to_string())]
        .into_iter()
        .collect();
    assert_eq!(substitute("var(--a)", &vars).as_deref(), Some("10px"));
    assert_eq!(substitute("var(--b, 4px)", &vars).as_deref(), Some("4px"));
    assert_eq!(substitute("var(--b)", &vars), None);
    assert_eq!(unquote("\"abc\""), "abc");
    assert_eq!(unquote("'x'"), "x");
    assert_eq!(unquote("nu"), "nu");
    let _ = (Stylesheet::parse("a{b:c}"), MediaContext::default());
}

#[test]
fn edit_errors_have_messages_and_never_corrupt() {
    let bad = "card#a(\n";
    let t = Target::Node("a".into());
    let e = edit::set_attr(bad, &t, "at", "1,2").unwrap_err();
    assert!(matches!(e, EditError::Invalid));
    assert!(e.to_string().contains("errors"));
    let e = edit::set_attr(SRC, &Target::Node("zzz".into()), "at", "1,2").unwrap_err();
    assert!(matches!(e, EditError::NotFound(ref id) if id == "zzz"));
    assert!(e.to_string().contains("zzz"));
    let e = edit::set_attr(SRC, &t, "bad name", "1").unwrap_err();
    assert!(matches!(e, EditError::BadValue(_)));
    assert!(e.to_string().contains("bad name"));
    // diagram options, classes, and removing what is not there
    let out = edit::set_attr(SRC, &Target::Diagram, "dir", "right").unwrap();
    assert!(out.contains("dir=\"right\""));
    assert_eq!(edit::remove_attr(SRC, &t, "at").unwrap(), SRC);
    assert_eq!(edit::remove_class(SRC, &t, "nope").unwrap(), SRC);
    assert_eq!(edit::snap(13.0, Some(8.0)), 16);
    assert_eq!(edit::snap(13.4, None), 13);
}

#[test]
fn wrap_is_read_on_zones_only() {
    let d = parse("zone#z(wrap=\"3\") Z\n  card#a(wrap=\"2\") A\nzone#y(wrap=\"zero\") Y\n");
    assert_eq!(d.model.node("z").unwrap().wrap, Some(3));
    assert_eq!(d.model.node("a").unwrap().wrap, None);
    assert_eq!(d.model.node("y").unwrap().wrap, None);
    let codes: Vec<&str> = d.warnings().map(|w| w.code).collect();
    assert_eq!(codes, ["bad-wrap", "bad-wrap"]);
    // absent from the JSON when not written (compatibility with the oracle)
    let json = serde_json::to_string(&parse("card#a A\n").model).unwrap();
    assert!(!json.contains("wrap") && !json.contains("tag_anim"));
}

#[test]
fn the_tag_carries_its_own_step_markers() {
    let d = parse("card#p(tag=\"8080 @1:up\") pod\ncard#q(tag=\"admin@corp\") q\n");
    let p = d.model.node("p").unwrap();
    assert_eq!(p.tag.as_deref(), Some("8080"));
    let a = p.tag_anim.as_ref().unwrap();
    assert_eq!(a.start.as_ref().unwrap().raw, "1");
    assert_eq!(a.fx.as_deref(), Some("up"));
    assert!(p.anim.is_none());
    let q = d.model.node("q").unwrap();
    assert_eq!(q.tag.as_deref(), Some("admin@corp"));
    assert!(q.tag_anim.is_none());
}

#[test]
fn frieze_zones_frame_events_but_do_not_nest() {
    let src = "diagram(layout=\"frise\" from=\"2014\" to=\"2016\" scale=\"year\")
  card#lxc(date=\"2014-02\") LXC
  zone#orch Orchestrators
    card#k8s(date=\"2015-07\") Kubernetes
";
    assert!(!skemo_lang::parse(src).has_errors());
    let nested = src.replace("    card#k8s", "    zone#inner Inner\n      card#k8s");
    let doc = skemo_lang::parse(&nested);
    assert!(doc.errors().any(|d| d.code == "zone-in-frise"));
}

#[test]
fn frieze_and_its_old_name_frise_are_one_mode() {
    let src = |l: &str| {
        format!(
            "diagram(layout=\"{l}\" from=\"2014\" to=\"2016\")\n  zone#z Z\n    zone#y Y\n  card#a A\n"
        )
    };
    let codes = |l: &str| {
        let mut c: Vec<String> = skemo_lang::parse(&src(l))
            .diagnostics
            .iter()
            .map(|d| d.code.to_string())
            .collect();
        c.sort();
        c
    };
    let frieze = codes("frieze");
    assert!(frieze.contains(&"zone-in-frise".to_string()), "{frieze:?}");
    assert!(frieze.contains(&"missing-date".to_string()), "{frieze:?}");
    assert_eq!(frieze, codes("frise"));
}
