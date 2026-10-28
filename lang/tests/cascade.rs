//! CSS cascade: selectors, specificity, `!important`, inline styles, variables, media.

use skemo_lang::style::{MediaContext, StyleTree, Styler, Stylesheet};

/// Computed style of element `n` of a tree `stage > card#k.x > span` (n = 0, 1, 2) with `css`.
fn compute(css: &str, media: MediaContext) -> Vec<skemo_lang::style::Computed> {
    let mut t = StyleTree::new();
    let stage = t.add(None, "div", Some("stage"), &["stage"]);
    let card = t.add(Some(stage), "card", Some("k"), &["x"]);
    t.add(Some(card), "span", None, &["a", "b"]);
    t.add(Some(card), "span", None, &["c"]);
    t.set_attr(card, "data-kind", "net");
    Styler::new(vec![Stylesheet::parse(css)], media).compute(&t)
}

fn get(css: &str, node: usize, prop: &str) -> Option<String> {
    compute(css, MediaContext::default())[node]
        .get(prop)
        .map(String::from)
}

#[test]
fn simple_selectors() {
    assert_eq!(
        get("card { color: red }", 1, "color").as_deref(),
        Some("rgba(255, 0, 0, 1)"),
        "colours are normalised"
    );
    assert_eq!(get(".x { width: 1px }", 1, "width").as_deref(), Some("1px"));
    assert_eq!(get("#k { width: 2px }", 1, "width").as_deref(), Some("2px"));
    assert_eq!(get("* { width: 3px }", 2, "width").as_deref(), Some("3px"));
    assert_eq!(
        get("[data-kind] { width: 4px }", 1, "width").as_deref(),
        Some("4px")
    );
    assert_eq!(
        get("[data-kind=\"net\"] { width: 5px }", 1, "width").as_deref(),
        Some("5px")
    );
    assert_eq!(
        get("[data-kind=\"other\"] { width: 6px }", 1, "width"),
        None
    );
    assert_eq!(
        get(".a.b { width: 7px }", 2, "width").as_deref(),
        Some("7px")
    );
}

#[test]
fn combinators_and_structural_pseudo_classes() {
    assert_eq!(
        get("card span { width: 1px }", 2, "width").as_deref(),
        Some("1px")
    );
    assert_eq!(
        get("div span { width: 1px }", 2, "width").as_deref(),
        Some("1px")
    );
    assert_eq!(
        get("card > span { width: 2px }", 3, "width").as_deref(),
        Some("2px")
    );
    assert_eq!(
        get("div > span { width: 3px }", 2, "width"),
        None,
        "not a direct child"
    );
    assert_eq!(
        get("span:first-child { width: 4px }", 2, "width").as_deref(),
        Some("4px")
    );
    assert_eq!(get("span:first-child { width: 4px }", 3, "width"), None);
    assert_eq!(
        get("span:last-child { width: 5px }", 3, "width").as_deref(),
        Some("5px")
    );
    assert_eq!(
        get("span:not(.c) { width: 6px }", 2, "width").as_deref(),
        Some("6px")
    );
    assert_eq!(get("span:not(.c) { width: 6px }", 3, "width"), None);
    // unknown pseudo-classes and :has() never match
    assert_eq!(get("span:hover { width: 7px }", 2, "width"), None);
    assert_eq!(get("card:has(span) { width: 7px }", 1, "width"), None);
}

#[test]
fn specificity_order_and_important() {
    let css = ".x { width: 1px } #k { width: 2px } card { width: 3px }";
    assert_eq!(
        get(css, 1, "width").as_deref(),
        Some("2px"),
        "id beats class beats tag"
    );
    let later = "card { width: 1px } card { width: 2px }";
    assert_eq!(
        get(later, 1, "width").as_deref(),
        Some("2px"),
        "later wins on a tie"
    );
    let imp = "#k { width: 1px } .x { width: 2px !important }";
    assert_eq!(
        get(imp, 1, "width").as_deref(),
        Some("2px"),
        "!important wins"
    );
}

#[test]
fn inline_styles_win_and_inherit() {
    let mut t = StyleTree::new();
    let stage = t.add(None, "div", Some("stage"), &["stage"]);
    let card = t.add(Some(stage), "card", None, &[]);
    let span = t.add(Some(card), "span", None, &[]);
    t.set_inline(card, "color", "#00ff00");
    t.set_inline(card, "width", "9px");
    let s = Styler::new(
        vec![Stylesheet::parse("card { color: red; width: 1px }")],
        MediaContext::default(),
    );
    let c = s.compute(&t);
    assert_eq!(c[card].get("width"), Some("9px"));
    assert_eq!(c[card].color().to_css(), "#00FF00");
    assert_eq!(c[span].color().to_css(), "#00FF00", "colour is inherited");
    assert_eq!(c[span].get("width"), None, "width is not inherited");
}

#[test]
fn custom_properties_inherit_and_substitute() {
    let css = ".stage { --gap: 6px; --pad: var(--gap) } card { padding: var(--pad); margin: var(--none, 2px) }";
    let c = compute(css, MediaContext::default());
    assert_eq!(c[1].padding(), [6.0; 4]);
    assert_eq!(c[1].margin(), [2.0; 4]);
    assert_eq!(c[2].get("--gap"), Some("6px"), "inherited by descendants");
}

#[test]
fn media_queries_follow_the_context() {
    let css = "card { width: 1px } @media (prefers-color-scheme: dark) { card { width: 2px } } @media print { card { width: 3px } } @media (min-width: 10px) { card { width: 4px } }";
    let at = |media| compute(css, media)[1].get("width").map(String::from);
    assert_eq!(at(MediaContext::default()).as_deref(), Some("1px"));
    assert_eq!(
        at(MediaContext {
            dark: true,
            ..MediaContext::default()
        })
        .as_deref(),
        Some("2px")
    );
    assert_eq!(
        at(MediaContext {
            print: true,
            ..MediaContext::default()
        })
        .as_deref(),
        Some("3px")
    );
}

#[test]
fn garbage_css_is_skipped_not_fatal() {
    let css = "card { width: 1px } {{{ ; } }}} /* unterminated comment";
    assert_eq!(get(css, 1, "width").as_deref(), Some("1px"));
    assert_eq!(get("", 1, "width"), None);
    let long = format!("card {{ width: {}px }}", "9".repeat(2000));
    let _ = get(&long, 1, "width");
}

#[test]
fn pseudo_elements_get_their_own_style() {
    let mut t = StyleTree::new();
    let stage = t.add(None, "div", Some("stage"), &["stage"]);
    let card = t.add(Some(stage), "card", None, &[]);
    let before = t.add_pseudo(card, "before");
    let after = t.add_pseudo(card, "after");
    let s = Styler::new(
        vec![Stylesheet::parse(
            "card::before { content: \"A\" } card::after { content: \"B\" } card { content: none }",
        )],
        MediaContext::default(),
    );
    let c = s.compute(&t);
    assert_eq!(c[before].string("content").as_deref(), Some("A"));
    assert_eq!(c[after].string("content").as_deref(), Some("B"));
    assert_eq!(c[card].get("content"), Some("none"));
}

#[test]
fn variables_resolve_through_chains_and_survive_a_reset() {
    // a chain of var() resolved on one element, a cycle dropped, a variable set then reset
    let css = ".stage { --a: var(--b); --b: var(--c); --c: 4px; --x: var(--y); --y: var(--x); --r: red; --r: initial } card { padding: var(--a); margin: var(--x, 1px) }";
    let c = compute(css, MediaContext::default());
    assert_eq!(c[1].padding(), [4.0; 4]);
    assert_eq!(
        c[1].margin(),
        [1.0; 4],
        "a cycle is invalid, the fallback applies"
    );
    assert_eq!(c[0].get("--r"), None);
}

#[test]
fn rules_are_found_by_id_class_and_tag_in_cascade_order() {
    let css = "#k { width: 1px } .x { width: 2px } card { height: 3px } * { color: red } CARD { min-width: 5px }";
    let c = compute(css, MediaContext::default());
    assert_eq!(c[1].get("width"), Some("1px"), "the id wins");
    assert_eq!(c[1].get("height"), Some("3px"));
    assert_eq!(c[1].get("min-width"), Some("5px"), "tags ignore case");
    assert_eq!(c[2].get("height"), None);
}
