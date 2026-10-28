//! The diagram's title, zone tags, stacks and wide cards (D-050).

use skemo_engine::{Options, from_source, html, svg};

fn build(src: &str) -> skemo_engine::Diagram {
    from_source(src, &Options::default()).unwrap().0
}

/// Pods above services above nodes above a network bar, the services' arrows going up.
const K8S: &str = "diagram(dir=\"down\")
  title Kubernetes application architecture
    li namespace mon_namespace
  stack
    zone#pods(tag=\"172.30.0.0/16\") Pods
      card#fe1 frontend-1
      card#fe2 frontend-2
      card#pg postgresql-0
    zone#svcs(tag=\"172.21.0.0/16\") Services
      card#svc Frontend
        to(fe1 fe2)
      card#db Database
        to(pg)
    zone#nodes.bare
      card#cp Control Plane
      card#w1 Worker 1
    card#net.wide Cluster network
      to(cp w1)
";

#[test]
fn a_stack_lays_its_children_in_writing_order() {
    let d = build(K8S);
    let b = &d.boxes;
    assert!(b["pods"].bottom() <= b["svcs"].y, "pods above services");
    assert!(b["svcs"].bottom() <= b["nodes"].y, "services above nodes");
    assert!(b["nodes"].bottom() <= b["net"].y, "nodes above the network");
}

#[test]
fn arrows_against_a_stack_go_straight_back_as_a_fork() {
    let d = build(K8S);
    for id in ["svc->fe1", "svc->fe2"] {
        let r = &d.routes[id];
        // out of the service's top, into the pod's bottom
        assert!((r[0].1 - d.boxes["svc"].y).abs() < 1.0, "{id}: {r:?}");
        assert!(
            (r.last().unwrap().1 - d.boxes[&id[5..]].bottom()).abs() < 1.0,
            "{id}: {r:?}"
        );
    }
    assert_eq!(d.edge_groups.get("svc->fe1"), d.edge_groups.get("svc->fe2"));
    // the service sits in the middle of its two pods
    let mid = |id: &str| d.boxes[id].x + d.boxes[id].w / 2.0;
    assert!((mid("svc") - (mid("fe1") + mid("fe2")) / 2.0).abs() < 1.0);
}

#[test]
fn a_wide_card_spans_its_group_and_its_arrows_drop_straight() {
    let d = build(K8S);
    let (net, pods) = (d.boxes["net"], d.boxes["pods"]);
    assert!(
        net.x <= pods.x + 0.5 && net.right() >= pods.right() - 0.5,
        "{net:?} {pods:?}"
    );
    for id in ["net->cp", "net->w1"] {
        let r = &d.routes[id];
        assert_eq!(r.len(), 2, "{id}: {r:?}");
        assert!((r[0].0 - r[1].0).abs() < 1e-6, "{id}: {r:?}");
        assert!(!d.edge_groups.contains_key(id), "{id} forks");
    }
}

#[test]
fn a_bare_zone_without_title_keeps_no_room() {
    let d = build(K8S);
    let (z, cp, w1) = (d.boxes["nodes"], d.boxes["cp"], d.boxes["w1"]);
    assert!(
        (z.y - cp.y.min(w1.y)).abs() < 0.5 && (z.x - cp.x.min(w1.x)).abs() < 0.5,
        "{z:?}"
    );
}

#[test]
fn the_title_sits_above_the_drawing_with_its_pills() {
    let d = build(K8S);
    let t = d.title_box.expect("a title box");
    let top = d.boxes.values().map(|b| b.y).fold(f64::INFINITY, f64::min);
    assert!(t.bottom() < top, "{t:?} above {top}");
    let out = svg::render(&d);
    assert!(
        out.contains("Kubernetes application architecture")
            && out.contains("namespace mon_namespace")
    );
    let page = html::render(&d, &Default::default());
    assert!(
        page.contains("class=\"diagram-title\"")
            && page.contains("<li class=\"pill\">namespace mon_namespace</li>")
    );
}

#[test]
fn a_zone_tag_is_drawn_after_its_title() {
    let d = build(K8S);
    let out = svg::render(&d);
    assert!(
        out.contains("172.30.0.0/16") && out.contains("172.21.0.0/16"),
        "{out}"
    );
    let page = html::render(&d, &Default::default());
    assert!(page.contains("<span class=\"tag\">172.30.0.0/16</span>"));
    // a stack draws nothing of its own
    assert!(!page.contains("data-id=\"stack1\""));
}
