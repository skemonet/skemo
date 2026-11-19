//! Tunnels (D-116): an arrow marked `tunnel="id"` runs into the near end of the tunnel, along
//! it on a lane of its own inside the tube, and out of the far end to its target; drawn as one
//! line, hidden inside the two cards it passes through.

use skemo_engine::{Options, from_source, html, svg};

const SRC: &str = "diagram(dir=\"right\")
  zone#paris Paris
    card#app App
      to(db tunnel=\"vpn\") SQL
      to(s3 tunnel=\"vpn\") Backups
      to(web) Health
    card#gw-a Gateway A
      tunnel#vpn(gw-b) IPsec
  zone#lyon Lyon
    card#gw-b Gateway B
    card#db DB
    card#s3 Storage
  card#web Web
";

fn build(src: &str) -> skemo_engine::Diagram {
    from_source(src, &Options::default()).unwrap().0
}

/// Does the polyline `r` have a point strictly inside box `b`?
fn enters(r: &[(f64, f64)], b: skemo_engine::layout::Box2) -> bool {
    r.iter()
        .any(|&(x, y)| x > b.x + 0.5 && x < b.x + b.w - 0.5 && y > b.y + 0.5 && y < b.y + b.h - 0.5)
}

#[test]
fn an_arrow_through_a_tunnel_passes_through_both_ends() {
    for layout in ["slide", "graph"] {
        let d = build(&SRC.replace(
            "dir=\"right\"",
            &format!("dir=\"right\" layout=\"{layout}\""),
        ));
        for id in ["app->db", "app->s3"] {
            let r = &d.routes[id];
            assert!(enters(r, d.boxes["gw-a"]), "{layout} {id} {r:?}");
            assert!(enters(r, d.boxes["gw-b"]), "{layout} {id} {r:?}");
            // it starts on its source and ends on its target
            let (s, e) = (r[0], *r.last().unwrap());
            let (app, db) = (d.boxes["app"], d.boxes[&id[5..]]);
            assert!(
                s.0 >= app.x - 0.5 && s.0 <= app.x + app.w + 0.5,
                "{layout} {id} {s:?}"
            );
            assert!(
                e.0 >= db.x - 0.5 && e.0 <= db.x + db.w + 0.5,
                "{layout} {id} {e:?}"
            );
        }
        // no leg is left over: one route for each arrow written
        assert_eq!(
            d.routes.len(),
            d.model.edges.len(),
            "{layout} {:?}",
            d.routes.keys()
        );
        assert!(!enters(&d.routes["app->web"], d.boxes["gw-a"]));
        assert_eq!(d.tubes.get("vpn"), Some(&2));
        assert_eq!(d.labels.len(), 4, "{layout} {:?}", d.labels);
    }
}

#[test]
fn the_lanes_run_side_by_side_inside_the_tube() {
    let d = build(SRC);
    let t = &d.routes["vpn"];
    let (a, b) = (d.boxes["gw-a"], d.boxes["gw-b"]);
    // the part of each arrow between the two cards, against the tunnel's line
    let between = |r: &[(f64, f64)]| -> Vec<(f64, f64)> {
        r.iter()
            .copied()
            .filter(|&(x, _)| x >= a.x + a.w - 0.5 && x <= b.x + 0.5)
            .collect()
    };
    let (sql, bak) = (between(&d.routes["app->db"]), between(&d.routes["app->s3"]));
    assert!(!sql.is_empty() && !bak.is_empty());
    let ty = t[0].1;
    assert!(
        t.iter().all(|p| (p.1 - ty).abs() < 0.5),
        "a straight tunnel {t:?}"
    );
    let off = |r: &[(f64, f64)]| r[0].1 - ty;
    // one lane each, GAP apart, centred on the tunnel's line
    assert!((off(&sql) + off(&bak)).abs() < 0.5, "{sql:?} {bak:?}");
    assert!(
        ((off(&sql) - off(&bak)).abs() - 6.0).abs() < 0.5,
        "{sql:?} {bak:?}"
    );
}

#[test]
fn the_tube_is_drawn_first_and_the_arrows_hidden_in_the_cards() {
    let d = build(SRC);
    let s = svg::render(&d);
    let tube = s.find("data-id=\"vpn\"").unwrap();
    let sql = s.find("data-id=\"app-&gt;db\"").unwrap();
    assert!(tube < sql, "the tunnel under its arrows");
    assert!(s.contains("class=\"edge tunnel\""));
    // two lanes: the theme's 16 and one lane more
    let g = &s[tube..sql];
    assert!(g.contains("stroke-width=\"22\""), "{g}");
    // the arrow's mask hides the two cards it crosses
    let after = &s[sql..];
    let mask = &after[..after.find("</g>").unwrap()];
    let cuts = mask.matches("fill=\"black\"").count();
    assert!(cuts >= 2, "{mask}");
    let h = html::render(&d, &html::HtmlOptions::default());
    assert!(
        h.contains("style=\"stroke-width:22px\""),
        "the tube in HTML"
    );
    assert!(h.find("data-id=\"vpn\"").unwrap() < h.find("data-id=\"app-&gt;db\"").unwrap());
}

#[test]
fn an_arrow_from_one_end_needs_no_leg_there() {
    let d = build(
        "diagram(dir=\"right\")\n  card#gw-a A\n    tunnel#t(gw-b)\n    to(db tunnel=\"t\") SQL\n  card#gw-b B\n  card#db DB\n    to(gw-a tunnel=\"t\") back\n",
    );
    let out = &d.routes["gw-a->db"];
    assert!(!enters(out, d.boxes["gw-a"]), "it starts on gw-a: {out:?}");
    assert!(enters(out, d.boxes["gw-b"]), "{out:?}");
    // the way back runs along the tunnel the other way
    let back = &d.routes["db->gw-a"];
    assert!(enters(back, d.boxes["gw-b"]), "{back:?}");
    assert!(!enters(back, d.boxes["gw-a"]), "{back:?}");
    assert_eq!(d.tubes.get("t"), Some(&2));
}
