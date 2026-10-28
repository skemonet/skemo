//! A workstation above a cluster of nodes (D-056): sources on their zone's first row, the zones
//! of a stack at one width, cross links forking, a corridor between the rows of the nodes, and
//! cards of a column keeping one title alignment.

use skemo_engine::{Options, from_source, html};

const SRC: &str = "diagram(dir=\"down\")
  stack
    zone#poste Workstation
      card#kubectl kubectl
        li API client
        to(api)
      card#manifest Manifest
        li YAML file
        to(vol).link
        to(img).link
      card#vol.chip Volumes
      card#img.chip Images
    zone#cluster Cluster
      zone#cp(wrap=\"1\") Control plane
        card#api Kubernetes API
          li Container
          to(k1 k2 constraint=\"false\").dashed
        card#k0 Kubelet
          li Process
        card#d0 Docker
      zone#w1(wrap=\"1\") Worker
        card#a1 Application
          li Container
        card#k1 Kubelet
          li Process
        card#d1 Docker
      zone#w2(wrap=\"1\") Worker
        card#a2 Application
          li Container
        card#k2 Kubelet
          li Process
        card#d2 Docker
";

fn build() -> skemo_engine::Diagram {
    from_source(SRC, &Options::default()).unwrap().0
}

#[test]
fn a_source_whose_arrows_leave_its_zone_sits_on_the_first_row() {
    let d = build();
    let (k, m) = (d.boxes["kubectl"], d.boxes["manifest"]);
    assert!((k.y - m.y).abs() < 1.0, "kubectl {k:?}, manifest {m:?}");
}

#[test]
fn the_zones_of_a_stack_share_one_width() {
    let d = build();
    let (p, c) = (d.boxes["poste"], d.boxes["cluster"]);
    assert!(
        (p.x - c.x).abs() < 0.5 && (p.w - c.w).abs() < 0.5,
        "{p:?} {c:?}"
    );
}

#[test]
fn cross_links_of_one_source_fork_through_a_corridor() {
    let d = build();
    let g = &d.edge_groups;
    assert!(
        g.contains_key("api->k1") && g.get("api->k1") == g.get("api->k2"),
        "{g:?}"
    );
    // the routes stay inside the cluster: through the rows' corridor, not around the top
    let c = d.boxes["cluster"];
    for id in ["api->k1", "api->k2"] {
        assert!(
            d.routes[id].iter().all(|p| p.1 >= c.y && p.1 <= c.bottom()),
            "{id}: {:?}",
            d.routes[id]
        );
    }
    // the rows of the nodes stay aligned
    assert!((d.boxes["k0"].y - d.boxes["k1"].y).abs() < 1.0);
    assert!((d.boxes["d0"].y - d.boxes["d2"].y).abs() < 1.0);
}

#[test]
fn a_card_without_lines_keeps_its_column_alignment() {
    let page = html::render(&build(), &Default::default());
    let docker = page.find("data-id=\"d0\"").unwrap();
    let open = page[..docker].rfind("<div").unwrap();
    assert!(
        !page[open..docker].contains("no-body"),
        "{}",
        &page[open..docker]
    );
}
