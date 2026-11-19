//! `cards` and `zones`: a card or a zone drawn as a pile, for several instances of one thing;
//! `count` gives the `×N`.

const SRC: &str = "diagram
  cards#api.service(count=\"4\" tag=\"ECS\") api
    li 4 tasks
    to(db)
  cards#w worker
  card#db DB
";

#[test]
fn a_pile_is_a_card_with_a_deck() {
    let doc = skemo_lang::parse(SRC);
    assert!(doc.diagnostics.is_empty(), "{:?}", doc.diagnostics);
    let m = doc.model;
    let api = m.node("api").unwrap();
    assert_eq!(api.kind, skemo_lang::Kind::Card);
    assert_eq!(api.deck, Some(skemo_lang::Deck { count: Some(4) }));
    assert_eq!(api.deck.unwrap().layers(), 3);
    assert_eq!(api.li.len(), 1);
    assert_eq!(m.edges.len(), 1);
    // without a count: three cards drawn, no badge
    let w = m.node("w").unwrap().deck.unwrap();
    assert_eq!((w.count, w.layers()), (None, 3));
    assert_eq!(skemo_lang::Deck { count: Some(2) }.layers(), 2);
    assert!(m.node("db").unwrap().deck.is_none());
    let json = serde_json::to_string(m.node("db").unwrap()).unwrap();
    assert!(!json.contains("deck"));
}

#[test]
fn a_pile_of_zones_is_a_zone_with_a_deck() {
    let doc = skemo_lang::parse(
        "diagram\n  zones#fleet(count=\"5\" tag=\"spoke\") Clusters\n    | managed\n    card#a agent\n  zones#r Regions\n",
    );
    assert!(doc.diagnostics.is_empty(), "{:?}", doc.diagnostics);
    let m = doc.model;
    let fleet = m.node("fleet").unwrap();
    assert_eq!(fleet.kind, skemo_lang::Kind::Zone);
    assert_eq!(fleet.deck, Some(skemo_lang::Deck { count: Some(5) }));
    assert_eq!(fleet.li.len(), 1);
    assert_eq!(m.node("a").unwrap().parent.as_deref(), Some("fleet"));
    assert_eq!(
        m.node("r").unwrap().deck,
        Some(skemo_lang::Deck { count: None })
    );
    assert!(
        skemo_lang::fmt::format("diagram\n  zones#z(count=\"2\") Z\n")
            .unwrap()
            .contains("zones#z(count=\"2\") Z")
    );
}

#[test]
fn count_is_read_on_piles_only() {
    let doc = skemo_lang::parse(
        "diagram\n  card#a(count=\"2\") A\n  cards#b(count=\"two\") B\n  zone#c(count=\"2\") C\n",
    );
    let codes: Vec<&str> = doc.diagnostics.iter().map(|d| d.code).collect();
    assert_eq!(codes, ["bad-count", "bad-count", "bad-count"]);
    assert!(doc.model.node("c").unwrap().deck.is_none());
    assert!(doc.model.node("a").unwrap().deck.is_none());
    assert_eq!(doc.model.node("b").unwrap().deck.unwrap().count, None);
}

#[test]
fn fmt_keeps_the_pile() {
    let out = skemo_lang::fmt::format(SRC).unwrap();
    assert!(
        out.contains("cards#api.service(count=\"4\" tag=\"ECS\") api"),
        "{out}"
    );
}

#[test]
fn an_arrow_leaves_a_card_or_a_zone_never_the_diagram() {
    let doc = skemo_lang::parse("diagram\n  zones#z Z\n    to(c)\n    card#a A\n  card#c C\n");
    assert!(doc.diagnostics.is_empty(), "{:?}", doc.diagnostics);
    let e = &doc.model.edges[0];
    assert_eq!((e.from.as_str(), e.to.as_str()), ("z", "c"));
    let doc = skemo_lang::parse("diagram\n  to(c)\n  card#c C\n");
    assert_eq!(doc.diagnostics[0].code, "to-outside-card");
}
