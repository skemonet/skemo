//! The diagram's title, stacks and zone tags (D-050): the model, and the warnings in English
//! (`title_stack_fr.rs` has them in French).

const SRC: &str = "diagram(dir=\"down\")
  title Kubernetes application architecture
    li namespace mon_namespace
  stack#all
    zone#pods(tag=\"172.30.0.0/16\") Pods
      card#fe1 frontend-1
    zone#svcs Services
      card#svc Frontend
        to(fe1)
";

#[test]
fn title_stack_and_zone_tag_reach_the_model() {
    let doc = skemo_lang::parse(SRC);
    assert!(doc.diagnostics.is_empty(), "{:?}", doc.diagnostics);
    let m = doc.model;
    let t = m.title.as_ref().unwrap();
    assert_eq!(t.text, "Kubernetes application architecture");
    assert_eq!(t.lines, vec!["namespace mon_namespace".to_string()]);
    let all = m.node("all").unwrap();
    assert!(all.stack);
    assert_eq!(all.kind, skemo_lang::Kind::Zone);
    assert_eq!(m.node("pods").unwrap().parent.as_deref(), Some("all"));
    assert_eq!(
        m.node("pods").unwrap().tag.as_deref(),
        Some("172.30.0.0/16")
    );
    assert!(!m.node("pods").unwrap().stack);
}

#[test]
fn stack_and_title_stay_out_of_the_json_when_absent() {
    let m = skemo_lang::parse("card#a A\n").model;
    let json = serde_json::to_string(&m).unwrap();
    assert!(
        !json.contains("stack") && !json.contains("title\":{"),
        "{json}"
    );
}

/// Misplaced or repeated titles, text on a stack, foreign content in a title.
pub const BAD: &str = "diagram
  title One
    card#x X
  title Two
  stack Named
    card#a A
      title Inside
";

#[test]
fn warnings_in_english() {
    let doc = skemo_lang::parse(BAD);
    let out: String = doc
        .diagnostics
        .iter()
        .map(|d| skemo_lang::diag::render(BAD, "bad.skemo", d))
        .collect();
    insta::assert_snapshot!("title-stack-warnings", out);
}

#[test]
fn a_line_carries_its_classes() {
    let m = skemo_lang::parse("card#a A\n  li 10.0.0.1\n  li.pill.port 8080 · http\n").model;
    let a = m.node("a").unwrap();
    assert!(a.li[0].classes.is_empty());
    assert_eq!(
        a.li[1].classes,
        vec!["pill".to_string(), "port".to_string()]
    );
    assert_eq!(a.li[1].text, "8080 · http");
}

#[test]
fn a_named_join_shared_by_arrows_to_one_card_holds() {
    let doc = skemo_lang::parse(
        "card#a A\n  to(c join=\"http\")\ncard#b B\n  to(c join=\"http\")\ncard#c C\n",
    );
    assert!(doc.diagnostics.is_empty(), "{:?}", doc.diagnostics);
    assert!(
        doc.model
            .edges
            .iter()
            .all(|e| e.join.as_deref() == Some("http"))
    );
}
