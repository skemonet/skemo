//! Icons and links (FORMAT §3.2.1): layout, warnings, SVG and HTML outputs.

use skemo_engine::{Options, from_source, html, svg};

const ICONS: &str = include_str!("data/icons.skemo");
const LINKS: &str = include_str!("data/links.skemo");

fn size(src: &str, id: &str) -> (f64, f64) {
    let (d, _) = from_source(src, &Options::default()).unwrap();
    let b = &d.boxes[id];
    (b.w, b.h)
}

#[test]
fn an_unknown_icon_reserves_no_space_and_warns() {
    let (d, _) = from_source(ICONS, &Options::default()).unwrap();
    assert_eq!(size(ICONS, "bad"), size(ICONS, "off"));
    let w: Vec<_> = d
        .warnings
        .iter()
        .filter(|w| w.contains("databse"))
        .collect();
    assert_eq!(w.len(), 1, "{:?}", d.warnings);
    assert!(w[0].contains("database"), "suggestion: {}", w[0]);
}

#[test]
fn an_icon_widens_a_long_card_by_size_plus_gap_only() {
    let plain = "card#a A card whose name is long enough to exceed the minimum width\n";
    let with =
        "card#a(icon=\"user\") A card whose name is long enough to exceed the minimum width\n";
    let (wp, hp) = size(plain, "a");
    let (wi, hi) = size(with, "a");
    assert_eq!(wi - wp, 22.0, "16 px of icon + 6 px of gap");
    assert_eq!(hp, hi);
}

#[test]
fn an_icon_or_a_link_changes_only_its_own_card() {
    let plain =
        "diagram(dir=\"right\")\n  card#a A\n    to(b)\n  card#b B\n    to(c)\n  card#c C\n";
    let linked = plain.replace("card#c C", "card#c(href=\"#a\" icon=\"user\") C");
    let (p, _) = from_source(plain, &Options::default()).unwrap();
    let (l, _) = from_source(&linked, &Options::default()).unwrap();
    for id in ["a", "b"] {
        assert_eq!(
            (p.boxes[id].w, p.boxes[id].h),
            (l.boxes[id].w, l.boxes[id].h)
        );
    }
    assert!(l.boxes["c"].w >= p.boxes["c"].w);
}

#[test]
fn the_type_default_icon_applies_and_none_removes_it() {
    let css = vec![".app { --icon: \"server\"; }".to_string()];
    // `a` feeds `b`, so that they are not siblings sharing a width (D-048)
    let src = "card#a.app An app name long enough to be wider than the minimum width of a card\n  to(b)\ncard#b.app(icon=\"none\") An app name long enough to be wider than the minimum width of a card\n";
    let (d, _) = from_source(
        src,
        &Options {
            css,
            ..Options::default()
        },
    )
    .unwrap();
    assert_eq!(d.boxes["a"].w - d.boxes["b"].w, 22.0);
}

#[test]
fn svg_draws_icons_and_wraps_links() {
    let (d, _) = from_source(LINKS, &Options::default()).unwrap();
    let s = svg::render(&d);
    assert!(s.contains("<a href=\"https://wiki.corp/firewall\">"));
    assert!(s.contains("<a href=\"#fw\">"));
    assert!(s.contains("<a href=\"https://wiki.corp/uplinks\">"));
    assert_eq!(s.matches("</a>").count(), 5, "card, zone, arrow x3 liens");
    assert!(s.contains("class=\"link-mark\""));
    assert!(s.contains("class=\"icon\""));
    assert!(!s.contains("javascript:"));
    // un lien n'ajoute jamais d'`id` : `<a href>` seul
    assert!(!s.contains("<a id=") && !s.contains("<a href=\"#fw\" id="));
}

#[test]
fn link_mark_can_be_hidden_and_frees_its_space() {
    let with = "card#a(href=\"#a\") A card whose name is long enough to exceed the minimum width\n";
    let (wi, _) = size(with, "a");
    let css = vec!["card { --link-mark: none; }".to_string()];
    let (d, _) = from_source(
        with,
        &Options {
            css,
            ..Options::default()
        },
    )
    .unwrap();
    assert_eq!(wi - d.boxes["a"].w, 16.0, "12 px de glyphe + 4 px de marge");
    assert!(!svg::render(&d).contains("link-mark"));
}

#[test]
fn unsafe_hrefs_are_refused() {
    for bad in [
        "javascript:alert(1)",
        "data:text/html,x",
        "vbscript:x",
        "file:///x",
        "",
    ] {
        let src = format!("card#a(href=\"{bad}\") A\n");
        assert!(from_source(&src, &Options::default()).is_err(), "{bad}");
    }
}

#[test]
fn html_links_and_icons() {
    let (d, _) = from_source(LINKS, &Options::default()).unwrap();
    let h = html::render(&d, &html::HtmlOptions::default());
    assert!(h.contains("href=\"https://wiki.corp/firewall\" rel=\"noopener\" target=\"_blank\""));
    assert!(h.contains("href=\"docs/core.md\" rel=\"noopener\"></a>"));
    assert!(h.contains("<svg class=\"link-mark\""));
    let (d, _) = from_source(ICONS, &Options::default()).unwrap();
    let h = html::render(&d, &html::HtmlOptions::default());
    assert!(h.contains("<svg class=\"icon\""));
}
