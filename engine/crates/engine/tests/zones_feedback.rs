//! Feedback from a Kubernetes deck (D-045): cards of a zone stacked on one another by the
//! column alignment, and the features asked for alongside.

use skemo_engine::{Options, from_source};
use skemo_lang::Kind;

fn build(src: &str) -> skemo_engine::Diagram {
    from_source(src, &Options::default()).unwrap().0
}

/// Pairs of cards whose boxes overlap.
fn overlaps(d: &skemo_engine::Diagram) -> Vec<(String, String)> {
    let cards: Vec<&str> = d
        .model
        .nodes
        .iter()
        .filter(|n| n.kind == Kind::Card)
        .map(|n| n.id.as_str())
        .collect();
    let mut out = Vec::new();
    for (i, a) in cards.iter().enumerate() {
        for b in &cards[i + 1..] {
            let (p, q) = (&d.boxes[*a], &d.boxes[*b]);
            if p.x < q.right() && q.x < p.right() && p.y < q.bottom() && q.y < p.bottom() {
                out.push((a.to_string(), b.to_string()));
            }
        }
    }
    out
}

#[test]
fn siblings_above_a_common_target_stay_apart() {
    let d = build(
        "diagram(dir=\"down\")
  card#src Source
    to(w1 w2)
  zone#z Zone
    card#w1 W1
      to(t)
    card#w2 W2
      to(t)
    card#t Target
",
    );
    assert_eq!(overlaps(&d), Vec::<(String, String)>::new());
}

#[test]
fn two_fans_in_a_zone_stay_apart() {
    let d = build(
        "diagram(dir=\"down\")
  zone#z Zone
    card#w1 W1
      to(sa sb)
    card#w2 W2
      to(sa sb)
    card#sa SA
      to(a1 a2)
    card#sb SB
      to(b1 b2)
    card#a1 A1
    card#a2 A2
    card#b1 B1
    card#b2 B2
",
    );
    assert_eq!(overlaps(&d), Vec::<(String, String)>::new());
}

#[test]
fn column_alignment_never_brings_a_card_onto_a_chip() {
    let d = build(
        "diagram(dir=\"down\")
  zone#poste Workstation
    card#kubectl(icon=\"terminal\") kubectl
      li API client
      to(api)
    card#manifest(icon=\"file-text\") Manifest
      li YAML file
      to(vol).link
      to(net).link
      to(img).link
    card#vol.chip(icon=\"hard-drive\") Volumes
    card#net.chip(icon=\"network\") Network
    card#img.chip(icon=\"container\") Images
  zone#cluster Cluster
    card#api(icon=\"box\") Kubernetes API
      li Container
",
    );
    assert_eq!(overlaps(&d), Vec::<(String, String)>::new());
}

#[test]
fn fit_warns_when_the_target_is_missed_and_nothing_helps() {
    // a single column of cards, far taller than 2:1 wide, that no setting brings closer
    let src = "diagram(dir=\"down\" fit=\"2:1\")
  card#a Alpha
    to(b)
  card#b Beta
    to(c)
  card#c Gamma
    to(d)
  card#d Delta
";
    let d = build(src);
    assert!(
        d.warnings.iter().any(|w| w.contains("fit=\"2:1\"")),
        "{:?}",
        d.warnings
    );
    // close enough to the target: silent
    let wide = build(&src.replace("2:1", "1:2.3"));
    assert!(
        !wide.warnings.iter().any(|w| w.contains("fit=")),
        "{:?}",
        wide.warnings
    );
}

#[test]
fn an_unconstrained_arrow_does_not_push_its_target_down() {
    let src = "diagram(dir=\"down\")
  card#client Client
    to(svc)
  card#dns CoreDNS
  card#svc Service
    to(p1 p2)
  card#p1 Pod 1
    to(dns) resolves
  card#p2 Pod 2
    to(dns) resolves
";
    let chained = build(src);
    assert!(chained.boxes["dns"].y > chained.boxes["p1"].bottom());
    let free = build(&src.replace("to(dns)", "to(dns constraint=\"false\")"));
    assert!(
        free.boxes["dns"].y < free.boxes["p1"].bottom(),
        "{:?}",
        free.boxes["dns"]
    );
    // still drawn and routed
    assert!(free.routes.contains_key("p1->dns") && free.routes.contains_key("p2->dns"));
}

#[test]
fn a_stack_of_one_per_row_reached_by_arrows_is_not_a_warning() {
    let src = "diagram(dir=\"down\")
  card#client Client
    to(app)
    to(db)
  zone#srv(wrap=\"1\") Server
    card#app App
    card#rt Runtime
    card#db Database
";
    let d = build(src);
    assert!(
        d.warnings.iter().all(|w| !w.contains("wrap")),
        "{:?}",
        d.warnings
    );
    assert_eq!(overlaps(&d), Vec::<(String, String)>::new());
    // a real grid still warns (D-044)
    let grid = build(&src.replace("wrap=\"1\"", "wrap=\"2\""));
    assert!(
        grid.warnings.iter().any(|w| w.contains("wrap")),
        "{:?}",
        grid.warnings
    );
}

#[test]
fn a_line_visible_from_the_start_until_a_step() {
    let d = build(
        "card#svc Service
  li type: NodePort    @0-1
  li type: ClusterIP   @2
card#p Pod             @1
",
    );
    let (old, new) = ("svc/li[0]", "svc/li[1]");
    assert!(d.anim.visible(old, 0.0) && d.anim.visible(old, 1.0) && !d.anim.visible(old, 2.0));
    assert!(!d.anim.visible(new, 1.0) && d.anim.visible(new, 2.0));
    let states: Vec<f64> = d.anim.states.iter().map(|s| s.value).collect();
    assert_eq!(states, vec![0.0, 1.0, 2.0]);
}

#[test]
fn trunk_branches_enter_their_targets_by_the_facing_side() {
    let d = build(
        "diagram(dir=\"down\")
  card#f1(at=\"50,40\") frontend-1
  card#f2(at=\"300,40\") frontend-2
  card#svc(at=\"165,240\") Frontend
    to(f1 f2 join=\"trunk\")
",
    );
    for t in ["f1", "f2"] {
        let r = &d.routes[&format!("svc->{t}")];
        let end = r.last().unwrap();
        let b = &d.boxes[t];
        assert!(
            (end.1 - b.bottom()).abs() < 1.0,
            "{t} entered at {end:?}, box {b:?}"
        );
    }
}

const SERVERS: &str = "diagram(dir=\"down\")
  card#kubectl kubectl
    to(api)
  zone#cluster Cluster
    zone#cp(wrap=\"1\") Control plane
      card#api Kubernetes API server, a long name
        to(k1 k2 constraint=\"false\").dashed
      card#k0 Kubelet
      card#d0 Docker
    zone#w1(wrap=\"1\") Worker 1
      card#a1 Application
      card#k1 Kubelet
      card#d1 Docker
    zone#w2(wrap=\"1\") Worker 2
      card#a2 Application
      card#k2 Kubelet
      card#d2 Docker
";

#[test]
fn a_stack_is_a_clean_column() {
    let d = build(SERVERS);
    let (api, k0, d0) = (&d.boxes["api"], &d.boxes["k0"], &d.boxes["d0"]);
    assert!(api.w > 200.0, "the long name widens the card: {api:?}");
    for b in [k0, d0] {
        assert_eq!((b.x, b.w), (api.x, api.w), "same left edge and width");
    }
}

#[test]
fn sibling_zones_of_one_row_share_their_top() {
    let d = build(SERVERS);
    let tops: Vec<f64> = ["cp", "w1", "w2"].iter().map(|z| d.boxes[*z].y).collect();
    assert!(tops.iter().all(|t| (t - tops[0]).abs() < 1e-6), "{tops:?}");
}

#[test]
fn lines_never_shown_together_share_their_place() {
    let one = build("card#s Site A\n  li NodePort\n");
    let swap = build("card#s Site A\n  li NodePort @0-1\n  li ClusterIP @2\n");
    let both = build("card#s Site A\n  li NodePort @0-2\n  li ClusterIP @2\n");
    assert_eq!(swap.boxes["s"].h, one.boxes["s"].h);
    assert!(
        both.boxes["s"].h > one.boxes["s"].h,
        "visible together at step 2"
    );
    let html = skemo_engine::html::render(&swap, &Default::default());
    assert!(html.contains("grid-area:1/1"), "stacked in HTML");
}

#[test]
fn zones_stretch_to_the_widest_of_their_column() {
    let src = "diagram(dir=\"up\" zones=\"stretch\")
  zone#pods Pods
    card#f1 frontend-1
    card#f2 frontend-2
    card#b1 backend-1
  zone#svcs Services
    card#sf Frontend
      to(f1 f2 join=\"trunk\")
";
    let d = build(src);
    let (p, s) = (&d.boxes["pods"], &d.boxes["svcs"]);
    assert_eq!((p.x, p.w), (s.x, s.w));
    let plain = build(&src.replace(" zones=\"stretch\"", ""));
    assert!(plain.boxes["svcs"].w < plain.boxes["pods"].w);
    for c in ["f1", "f2", "b1", "sf"] {
        assert_eq!(d.boxes[c], plain.boxes[c], "{c} does not move");
    }
}

#[test]
fn a_stretched_zone_never_covers_a_card_beside_it() {
    // an internet gateway in the VPC, beside the private subnets, with an arrow from the NAT
    let d = build(
        "diagram(dir=\"down\" zones=\"stretch\")
  zone#vpc VPC
    card#igw IGW
    zone#pub public
      card#nat NAT
        to(igw) egress
      card#lb LB
        to(api)
    zone#priv private
      card#api api
        to(db)
    zone#data data
      card#db db
",
    );
    let g = d.boxes["igw"];
    for z in ["pub", "priv", "data"] {
        let b = d.boxes[z];
        let covers = b.x < g.right() && g.x < b.right() && b.y < g.bottom() && g.y < b.bottom();
        assert!(!covers, "{z} {b:?} covers the gateway {g:?}");
    }
}

#[test]
fn a_frieze_zone_frames_its_events_on_one_side() {
    let d = build(
        "diagram(layout=\"frise\" from=\"2014\" to=\"2016\" scale=\"year\" width=\"800\")
  card#lxc(date=\"2014-02\") LXC
  card#docker(date=\"2014-06\") Docker
  zone#orch Orchestrators
    card#compose(date=\"2014-10\") Docker Compose
    card#k8s(date=\"2015-07\") Kubernetes
    card#swarm(date=\"2015-11\") Docker Swarm
",
    );
    let z = d.boxes["orch"];
    let kids = ["compose", "k8s", "swarm"].map(|k| d.boxes[k]);
    for b in kids {
        assert!(b.x > z.x && b.right() < z.right() && b.y > z.y && b.bottom() < z.bottom());
        assert!(b.bottom() < 0.0, "all above the axis: {b:?}");
    }
    assert!(z.bottom() <= 0.0, "the frame stops at the axis: {z:?}");
    for out in ["lxc", "docker"] {
        let b = d.boxes[out];
        let inside = b.x < z.right() && z.x < b.right() && b.y < z.bottom() && z.y < b.bottom();
        assert!(!inside, "{out} outside the frame");
    }
}
