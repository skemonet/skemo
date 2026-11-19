//! `tunnel` (D-116): a tunnel between two cards, written in one of them; `tunnel="id"` on a `to`
//! sends the arrow through it.

const SRC: &str = "diagram
  card#app App
    to(db tunnel=\"vpn\") SQL
    to(web) Health
  card#gw-a A
    tunnel#vpn(gw-b).wan IPsec
  card#gw-b B
  card#db DB
  card#web Web
";

#[test]
fn a_tunnel_is_a_link_that_arrows_pass_through() {
    let doc = skemo_lang::parse(SRC);
    assert!(doc.diagnostics.is_empty(), "{:?}", doc.diagnostics);
    let m = doc.model;
    let t = m.edges.iter().find(|e| e.id == "vpn").unwrap();
    assert!(t.tunnel);
    assert_eq!((t.from.as_str(), t.to.as_str()), ("gw-a", "gw-b"));
    assert_eq!(t.classes, ["wan"]);
    assert_eq!(t.label, "IPsec");
    let sql = m.edges.iter().find(|e| e.to == "db").unwrap();
    assert_eq!(sql.through.as_deref(), Some("vpn"));
    assert!(!sql.tunnel);
    // nothing new in the JSON of a plain arrow
    let web = m.edges.iter().find(|e| e.to == "web").unwrap();
    let json = serde_json::to_string(web).unwrap();
    assert!(
        !json.contains("tunnel") && !json.contains("through"),
        "{json}"
    );
}

#[test]
fn a_tunnel_badly_written_is_warned_and_drawn_as_written() {
    let doc = skemo_lang::parse(
        "diagram\n  card#a A\n    to(b tunnel=\"x\")\n    to(c tunnel)\n    tunnel(z)\n  zone#z Z\n    card#b B\n  card#c C\n    tunnel#t(a tunnel=\"t\")\n  zone#y Y\n    tunnel(c)\n",
    );
    let codes: Vec<&str> = doc.diagnostics.iter().map(|d| d.code).collect();
    assert_eq!(
        codes,
        [
            "bad-tunnel",
            "bad-tunnel",
            "bad-tunnel",
            "bad-tunnel",
            "tunnel-outside-card"
        ],
        "{:?}",
        doc.diagnostics
    );
    let m = doc.model;
    assert!(m.edges.iter().all(|e| e.through.is_none()));
    // `to(c tunnel)` is no arrow to a card named `tunnel`
    assert!(m.edges.iter().all(|e| e.to != "tunnel"));
    // a tunnel to a zone is an arrow
    assert!(!m.edges.iter().find(|e| e.to == "z").unwrap().tunnel);
    assert!(m.edges.iter().find(|e| e.id == "t").unwrap().tunnel);
}

#[test]
fn a_tab_or_a_space_formats_a_tunnel_like_an_arrow() {
    let out = skemo_lang::fmt::format(
        "diagram\n  card#a\tA\n    tunnel#t(b).wan  IPsec\n    to(b   tunnel=\"t\")\n  card#b B\n",
    );
    assert_eq!(
        out.unwrap(),
        "diagram\n  card#a A\n    tunnel#t(b).wan IPsec\n    to(b tunnel=\"t\")\n  card#b B\n"
    );
}
