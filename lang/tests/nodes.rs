//! `node` (D-087): a card drawn as its icon with its name and lines beside it; no chip nor
//! category.

const SRC: &str = "diagram
  node#user.person(icon=\"user\") Customer
    | web and mobile
    to(api) HTTPS
  card#api API
";

#[test]
fn a_node_is_a_card_marked_node() {
    let doc = skemo_lang::parse(SRC);
    assert!(doc.diagnostics.is_empty(), "{:?}", doc.diagnostics);
    let m = doc.model;
    let user = m.node("user").unwrap();
    assert_eq!(user.kind, skemo_lang::Kind::Card);
    assert!(user.node);
    assert_eq!((user.li.len(), user.icon.as_deref()), (1, Some("user")));
    assert_eq!(m.edges[0].from, "user");
    assert!(!m.node("api").unwrap().node);
    let json = serde_json::to_string(m.node("api").unwrap()).unwrap();
    assert!(!json.contains("\"node\""), "{json}");
}

#[test]
fn tag_and_label_are_not_read_on_a_node() {
    let doc = skemo_lang::parse("diagram\n  node#a(tag=\"ECS\" label=\"Service\") A\n");
    let codes: Vec<&str> = doc.diagnostics.iter().map(|d| d.code).collect();
    assert_eq!(codes, ["unknown-attribute", "unknown-attribute"]);
    let a = doc.model.node("a").unwrap();
    assert_eq!((a.tag.as_deref(), a.label.as_deref()), (None, None));
}

#[test]
fn the_class_node_is_the_engines() {
    let doc = skemo_lang::parse("diagram\n  card#a.node A\n");
    let codes: Vec<&str> = doc.diagnostics.iter().map(|d| d.code).collect();
    assert_eq!(codes, ["part-class"]);
}

#[test]
fn fmt_and_edits_keep_the_node() {
    let out = skemo_lang::fmt::format(SRC).unwrap();
    assert!(
        out.contains("node#user.person(icon=\"user\") Customer"),
        "{out}"
    );
    let back = skemo_lang::structure::set_tag(SRC, "user", "card").unwrap();
    assert!(back.contains("  card#user.person"), "{back}");
    let to = skemo_lang::structure::set_tag(SRC, "api", "node").unwrap();
    assert!(skemo_lang::parse(&to).model.node("api").unwrap().node);
}

/// `nodes` (D-117): a node standing for several, drawn once with its `×N` badge.
#[test]
fn nodes_is_a_node_with_a_count() {
    let doc = skemo_lang::parse("diagram\n  nodes#c(icon=\"cable\" count=\"3\") Cables\n");
    assert!(doc.diagnostics.is_empty(), "{:?}", doc.diagnostics);
    let c = doc.model.node("c").unwrap();
    assert!(c.node);
    assert_eq!(c.deck.and_then(|d| d.count), Some(3));
}
