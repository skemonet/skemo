//! The language as redefined in v0.8 (FORMAT §3.2.3–§3.2.9, §7): lines `|`, Markdown, tables,
//! category, docking, dates.

use skemo_lang::{Join, parse};

const SRC: &str = r#"diagram(dir="right")
  title Shop — AWS production platform
    | Account 123456789012 · **eu-west-3**
    |.pill prod
  zone#vpc.vpc(tag="module.network") VPC
    | 3 availability zones
    card#lambda.fn.warn(label="Lambda (in VPC)" tag="module.functions") order_events
      | `python3.12` · 256 MB
      |.warn db SG has no rule  @2
      | .env file
      to(db) 5432
    card#db(label="") orders
  zone#sec(at="s") Security
    card#sg(label="Security groups") Ingress
      | Group | Inbound |
      |-------|:-------:|
      | alb   | 443 from ==0.0.0.0/0== |
      | ^^    | 80 |
      | worker | none ||
  card#flows.legend(at="NE") Flows
    |.user user request
"#;

#[test]
fn lines_title_and_zone() {
    let doc = parse(SRC);
    assert!(!doc.has_errors(), "{:?}", doc.diagnostics);
    let t = doc.model.title.as_ref().unwrap();
    assert_eq!(t.sub, vec!["Account 123456789012 · **eu-west-3**"]);
    assert_eq!(t.lines, vec!["prod"]);
    let vpc = doc.model.node("vpc").unwrap();
    assert_eq!(vpc.li[0].text, "3 availability zones");
    let l = doc.model.node("lambda").unwrap();
    assert_eq!(l.li[0].text, "`python3.12` · 256 MB");
    assert_eq!(l.li[1].classes, vec!["warn"]);
    assert_eq!(l.li[1].text, "db SG has no rule");
    assert_eq!(
        l.li[1].anim.as_ref().unwrap().start.as_ref().unwrap().raw,
        "2"
    );
    assert!(l.li[2].classes.is_empty());
    assert_eq!(l.li[2].text, ".env file");
    assert_eq!(l.items.len(), 3);
}

#[test]
fn category_and_dock() {
    let doc = parse(SRC);
    let l = doc.model.node("lambda").unwrap();
    assert_eq!(l.label.as_deref(), Some("Lambda (in VPC)"));
    assert_eq!(l.tag.as_deref(), Some("module.functions"));
    assert_eq!(doc.model.node("db").unwrap().label.as_deref(), Some(""));
    assert_eq!(doc.model.node("sec").unwrap().dock.as_deref(), Some("s"));
    assert_eq!(doc.model.node("flows").unwrap().dock.as_deref(), Some("ne"));
    assert!(doc.model.node("sec").unwrap().at.is_none());
}

#[test]
fn rows() {
    let doc = parse(SRC);
    let sg = doc.model.node("sg").unwrap();
    assert_eq!(sg.li[0].cells.len(), 2);
    assert_eq!(sg.li[1].rule.len(), 2);
    assert_eq!(sg.li[2].cells[1].text, "443 from ==0.0.0.0/0==");
    assert_eq!(sg.li[3].cells[0].join, Some(Join::Up));
    assert_eq!(sg.li[4].cells[2].join, Some(Join::Left));
}

#[test]
fn line_spans_for_the_editor() {
    let src = "card#a A\n  |.warn x y  @2\n  | plain\n  |\n";
    let doc = parse(src);
    let l = &doc.map.lis[&("a".to_string(), 0)];
    assert_eq!(&src[l.text.unwrap().start..l.text.unwrap().end], "x y");
    assert_eq!(&src[l.classes[0].1.start..l.classes[0].1.end], "warn");
    let p = &doc.map.lis[&("a".to_string(), 1)];
    assert_eq!(&src[p.text.unwrap().start..p.text.unwrap().end], "plain");
    assert!(doc.map.lis[&("a".to_string(), 2)].text.is_none());
    let out = skemo_lang::structure::set_line(src, "a", 0, "z").unwrap();
    assert!(out.contains("|.warn z  @2"), "{out}");
    let out = skemo_lang::structure::set_line(src, "a", 2, "w").unwrap();
    assert!(out.ends_with("  | w\n"), "{out}");
}

#[test]
fn old_spellings_are_read() {
    let doc = parse(
        "diagram(layout=\"timeline\" from=\"2026-10\" to=\"2027-06\")\n  title T\n    li ns\n  card#a(start=\"2026-11\") A\n    li.pill x\n",
    );
    assert!(!doc.has_errors(), "{:?}", doc.diagnostics);
    assert_eq!(doc.model.option("start"), Some("2026-10"));
    assert_eq!(doc.model.option("end"), Some("2027-06"));
    assert_eq!(doc.model.title.as_ref().unwrap().lines, vec!["ns"]);
    assert_eq!(doc.model.node("a").unwrap().li[0].classes, vec!["pill"]);
    assert!(doc.warnings().all(|w| w.code != "line-misplaced"));
}

#[test]
fn removed_attributes_are_not_options() {
    let doc = parse("diagram(elk='{\"x\":1}' router=\"avoid\" dir=\"down\")\n  card#a A\n");
    assert!(doc.model.option("elk").is_none());
    assert!(doc.model.option("router").is_none());
    assert_eq!(doc.model.option("dir"), Some("down"));
    assert_eq!(
        doc.warnings()
            .filter(|w| w.code == "unknown-attribute")
            .count(),
        2
    );
}

#[test]
fn frieze_end_is_included() {
    use skemo_lang::dates::parse_frise;
    // Empire -27 → 476 runs to the end of 476
    assert_eq!(parse_frise("476", true).unwrap(), 477.0);
}
