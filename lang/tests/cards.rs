//! `cards`: a card drawn as a pile, for several instances of one thing; `count` gives the `×N`.

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
fn count_is_read_on_cards_only() {
    let doc = skemo_lang::parse("diagram\n  card#a(count=\"2\") A\n  cards#b(count=\"two\") B\n");
    let codes: Vec<&str> = doc.diagnostics.iter().map(|d| d.code).collect();
    assert_eq!(codes, ["bad-count", "bad-count"]);
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
