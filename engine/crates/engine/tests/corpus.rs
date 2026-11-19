//! Properties every graph drawing of the corpus keeps (the prototype's oracle and `tests/data`):
//! no arrow runs through the inside of the cards it links (a turn moved into its own source drew a
//! line across the card's text).

use skemo_engine::{Options, from_source};
use skemo_lang::Kind;
use std::path::Path;
mod elk;

fn corpus() -> Vec<(String, String, Vec<String>)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut found = Vec::new();
    for (dir, ext) in [("../../fixtures", "pug"), ("tests/data", "skemo")] {
        let dir = root.join(dir);
        for e in std::fs::read_dir(&dir).unwrap() {
            let p = e.unwrap().path();
            if p.extension().is_some_and(|x| x == ext) {
                let name = p.file_stem().unwrap().to_str().unwrap().to_string();
                let src = std::fs::read_to_string(&p).unwrap();
                // cards pinned by hand on ELK's drawing: their places mean something there only
                let src = if has_pins(&src) {
                    elk::graph(&src)
                } else {
                    src
                };
                let css = std::fs::read_to_string(dir.join(format!("{name}.css")))
                    .into_iter()
                    .collect();
                found.push((name, src, css));
            }
        }
    }
    found.sort_by(|a, b| a.0.cmp(&b.0));
    found
}

/// A card pinned at a point (`at="x,y"`), not docked on a side (`at="s"`).
fn has_pins(src: &str) -> bool {
    src.match_indices("at=\"").any(|(i, m)| {
        src[i + m.len()..]
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_digit() || c == '-')
    })
}

#[test]
fn no_arrow_runs_through_the_cards_it_links() {
    let mut bad = Vec::new();
    for (name, src, css) in corpus() {
        let (d, _) = from_source(
            &src,
            &Options {
                css,
                ..Options::default()
            },
        )
        .unwrap();
        // a timeline's arrows leave a bar at a date, inside its card's box (title and bar)
        if d.model.option("layout").is_some_and(|l| l != "graph") {
            continue;
        }
        for e in &d.model.edges {
            let Some(r) = d.routes.get(&e.id) else {
                continue;
            };
            for end in [&e.from, &e.to] {
                if d.model.node(end).is_none_or(|n| n.kind != Kind::Card) {
                    continue;
                }
                let Some(b) = d.boxes.get(end) else { continue };
                // strictly inside, 1 px in from the border the ports sit on
                let through = r.windows(2).any(|s| {
                    let (x0, x1) = (s[0].0.min(s[1].0), s[0].0.max(s[1].0));
                    let (y0, y1) = (s[0].1.min(s[1].1), s[0].1.max(s[1].1));
                    x1 > b.x + 1.0
                        && x0 < b.right() - 1.0
                        && y1 > b.y + 1.0
                        && y0 < b.bottom() - 1.0
                });
                if through {
                    bad.push(format!("{name}: {} through {end}", e.id));
                }
            }
        }
    }
    assert!(bad.is_empty(), "{bad:#?}");
}

/// Two arrows leaving one side of a card do not cross under it, even when a third one on that
/// side belongs to a join (the router used to leave the whole side as it was): api's exits to
/// the managed services on the Terraform platform.
#[test]
fn exits_of_a_side_with_a_join_are_uncrossed() {
    let d = terraform();
    let (a, b) = (&d.routes["api->orders-sqs"], &d.routes["api->uploads"]);
    let crossing = |s: &[(f64, f64)], t: &[(f64, f64)]| {
        let (hs, ht) = ((s[0].1 - s[1].1).abs() < 0.5, (t[0].1 - t[1].1).abs() < 0.5);
        if hs == ht {
            return false;
        }
        let (h, v) = if hs { (s, t) } else { (t, s) };
        let (x, y) = (v[0].0, h[0].1);
        h[0].0.min(h[1].0) < x
            && x < h[0].0.max(h[1].0)
            && v[0].1.min(v[1].1) < y
            && y < v[0].1.max(v[1].1)
    };
    let n = a
        .windows(2)
        .map(|s| b.windows(2).filter(|t| crossing(s, t)).count())
        .sum::<usize>();
    assert_eq!(n, 0, "{a:?} {b:?}");
}

/// The Terraform platform of `tests/data`, laid out with its stylesheet.
fn terraform() -> skemo_engine::Diagram {
    data("terraform-shop")
}

/// A drawing of `tests/data` and its stylesheet, laid out.
fn data(name: &str) -> skemo_engine::Diagram {
    data_with(name, str::to_string)
}

/// The same, drawn by ELK: the finishing passes of its routes (D-093, the steps flattened) are
/// ELK's; the slide engine keeps the constitution's rules instead.
fn data_elk(name: &str) -> skemo_engine::Diagram {
    data_with(name, elk::graph)
}

fn data_with(name: &str, engine: fn(&str) -> String) -> skemo_engine::Diagram {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data");
    let src = engine(&std::fs::read_to_string(root.join(format!("{name}.skemo"))).unwrap());
    let css = vec![std::fs::read_to_string(root.join(format!("{name}.css"))).unwrap()];
    from_source(
        &src,
        &Options {
            css,
            ..Options::default()
        },
    )
    .unwrap()
    .0
}

/// Arrows of one card that carry the same label merge when they cross a row of cards on their
/// way, and the label is written once, on the trunk (D-093): backoffice's two `5432`.
#[test]
fn long_arrows_with_one_label_merge_and_say_it_once() {
    let d = terraform();
    let (ra, rb) = (
        &d.routes["backoffice->orders"],
        &d.routes["backoffice->orders-replica"],
    );
    assert_eq!(ra[..2], rb[..2], "one trunk: {ra:?} {rb:?}");
    let shown = d
        .labels
        .keys()
        .filter(|k| k.starts_with("backoffice->"))
        .count();
    assert_eq!(shown, 1, "{:?}", d.labels);
}

/// An end alone on its side lands in its middle, a step rather than an end off the middle
/// (D-093; small steps are no longer straightened by sliding an end): the load balancer's HTTPS
/// arrow to the api.
#[test]
fn a_lone_end_lands_in_the_middle_of_its_side() {
    let d = data_elk("terraform-shop");
    let r = &d.routes["shop-prod->api"];
    let api = d.boxes["api"];
    let end = r.last().unwrap();
    assert!((end.0 - (api.x + api.w / 2.0)).abs() < 1.0, "{r:?} {api:?}");
}

/// Two arrows that share neither their source nor their target are never drawn on one line (the
/// same line within 1.5 px over more than 4 px): they read as one arrow. A crossing is fine.
#[test]
fn unrelated_arrows_never_run_on_one_line() {
    type Run = (String, String, String, bool, f64, f64, f64);
    let mut bad = Vec::new();
    for (name, src, css) in corpus() {
        let (d, _) = from_source(
            &src,
            &Options {
                css,
                ..Options::default()
            },
        )
        .unwrap();
        let mut runs: Vec<Run> = Vec::new();
        for e in &d.model.edges {
            let Some(r) = d.routes.get(&e.id) else {
                continue;
            };
            for w in r.windows(2) {
                let (p, q) = (w[0], w[1]);
                let (v, c, lo, hi) = if (p.0 - q.0).abs() < 0.5 {
                    (true, p.0, p.1.min(q.1), p.1.max(q.1))
                } else if (p.1 - q.1).abs() < 0.5 {
                    (false, p.1, p.0.min(q.0), p.0.max(q.0))
                } else {
                    continue;
                };
                runs.push((e.id.clone(), e.from.clone(), e.to.clone(), v, c, lo, hi));
            }
        }
        for (i, a) in runs.iter().enumerate() {
            for b in &runs[i + 1..] {
                let unrelated = a.0 != b.0 && a.1 != b.1 && a.2 != b.2;
                if unrelated
                    && a.3 == b.3
                    && (a.4 - b.4).abs() < 1.5
                    && a.6.min(b.6) - a.5.max(b.5) > 4.0
                {
                    bad.push(format!("{name}: {} on {}", a.0, b.0));
                }
            }
        }
    }
    bad.dedup();
    assert!(bad.is_empty(), "{bad:#?}");
}

/// No arrow runs through a zone's title or its tag pill (graph drawings): the router goes round
/// them, and spreading the ports of a side used to drag a line across a pill standing over it.
#[test]
fn no_arrow_runs_through_a_zone_title() {
    let mut bad = Vec::new();
    for (name, src, css) in corpus() {
        let (d, _) = from_source(
            &src,
            &Options {
                css,
                ..Options::default()
            },
        )
        .unwrap();
        if d.model.option("layout").is_some_and(|l| l != "graph") {
            continue;
        }
        let titles = skemo_engine::labels::zone_titles(&d.model, &d.scene, &d.boxes);
        for (id, r) in &d.routes {
            for (z, t) in &titles {
                // 3 px inside the box: a line may graze a title
                let through = r.windows(2).any(|s| {
                    let (x0, x1) = (s[0].0.min(s[1].0), s[0].0.max(s[1].0));
                    let (y0, y1) = (s[0].1.min(s[1].1), s[0].1.max(s[1].1));
                    x1 > t.x + 3.0
                        && x0 < t.right() - 3.0
                        && y1 > t.y + 3.0
                        && y0 < t.bottom() - 3.0
                });
                if through {
                    bad.push(format!("{name}: {id} through the title of {z}"));
                }
            }
        }
    }
    assert!(bad.is_empty(), "{bad:#?}");
}

/// Where two arrows cross, the horizontal line is cut on each side of the vertical one, as
/// passing under it (D-093): a crossing then never reads as a junction. One cut per crossing
/// away from the bends, by a mask: the line itself runs on unbroken.
#[test]
fn crossings_get_a_gap() {
    let d = terraform();
    let svg = skemo_engine::svg::render(&d);
    let cuts = svg.matches("fill=\"black\"/>").count();
    let expected: usize = d
        .routes
        .keys()
        .map(|id| skemo_engine::arrow::hops(&d, id, (0.0, 0.0)).len())
        .sum();
    assert!(expected > 0);
    assert_eq!(cuts, expected, "{cuts} cuts, {expected} crossings");
    assert!(!svg.contains(" A5,5 0 0 "), "no bridge left");
    assert_eq!(
        svg.matches("<mask ").count(),
        svg.matches(" mask=\"url(#").count()
    );
}

/// A step in the middle of an arrow (two parallel runs the same way, joined by less than a
/// centimetre) goes on one level when nothing is crossed for it: nightly's arrow to the order
/// events went left, up 35 px, then left again (the platform as a Terraform importer writes it).
#[test]
fn a_step_in_the_middle_of_an_arrow_is_flattened() {
    let d = data_elk("terraform-import");
    let r = &d.routes["nightly->order-events"];
    assert!(r.len() <= 4, "{r:?}");
}

/// A Terraform importer's output: the managed services' `wrap` grid, narrower than ELK's room for it,
/// sits towards the network beside it, and title room widens the network's edge once for its
/// five zones: no wide empty band between the two (370 px before).
#[test]
fn a_narrowed_grid_leaves_no_gap_beside_it() {
    let (_, src, css) = corpus()
        .into_iter()
        .find(|c| c.0 == "terraform-import")
        .unwrap();
    let (d, _) = from_source(
        &src,
        &Options {
            css,
            ..Options::default()
        },
    )
    .unwrap();
    let (s, v) = (&d.boxes["services-eu-west-3"], &d.boxes["vpc-main"]);
    assert!(v.x - s.right() < 60.0, "{s:?} {v:?}");
}

/// No arrow runs through a card it does not link (two arrows leaving a card spread into the card
/// 20 px beside it). `step-ports` is drawn in steps: an arrow passes where a card of another step
/// stands (D-060).
#[test]
fn no_arrow_runs_through_another_card() {
    let mut bad = Vec::new();
    for (name, src, css) in corpus().into_iter().filter(|c| c.0 != "step-ports") {
        let (d, _) = from_source(
            &src,
            &Options {
                css,
                ..Options::default()
            },
        )
        .unwrap();
        if d.model.option("layout").is_some_and(|l| l != "graph") {
            continue;
        }
        let cards: Vec<&str> = d
            .model
            .nodes
            .iter()
            .filter(|n| n.kind == Kind::Card)
            .map(|n| n.id.as_str())
            .collect();
        for e in &d.model.edges {
            let Some(r) = d.routes.get(&e.id) else {
                continue;
            };
            for c in cards.iter().filter(|c| **c != e.from && **c != e.to) {
                let Some(b) = d.boxes.get(*c) else { continue };
                let through = r.windows(2).any(|s| {
                    let (x0, x1) = (s[0].0.min(s[1].0), s[0].0.max(s[1].0));
                    let (y0, y1) = (s[0].1.min(s[1].1), s[0].1.max(s[1].1));
                    x1 > b.x + 1.0
                        && x0 < b.right() - 1.0
                        && y1 > b.y + 1.0
                        && y0 < b.bottom() - 1.0
                });
                if through {
                    bad.push(format!("{name}: {} through {c}", e.id));
                }
            }
        }
    }
    assert!(bad.is_empty(), "{bad:#?}");
}

/// No zone lies over another that is neither inside it nor around it. `reseau-conflit` pins two
/// zones over each other on purpose (the oracle of reported overlaps, D-033).
#[test]
fn no_zone_lies_over_another() {
    let mut bad = Vec::new();
    for (name, src, css) in corpus().into_iter().filter(|c| c.0 != "reseau-conflit") {
        let (d, _) = from_source(
            &src,
            &Options {
                css,
                ..Options::default()
            },
        )
        .unwrap();
        if d.model.option("layout").is_some_and(|l| l != "graph") {
            continue;
        }
        let zones: Vec<&str> = d
            .model
            .nodes
            .iter()
            .filter(|n| n.kind == Kind::Zone && !skemo_engine::layout::invisible(n))
            .map(|n| n.id.as_str())
            .collect();
        let nested = |a: &str, b: &str| d.model.ancestors(a).iter().any(|n| n.id == b);
        for (i, a) in zones.iter().enumerate() {
            for b in &zones[i + 1..] {
                if nested(a, b) || nested(b, a) {
                    continue;
                }
                let (Some(p), Some(q)) = (d.boxes.get(*a), d.boxes.get(*b)) else {
                    continue;
                };
                if p.x < q.right() - 1.0
                    && q.x < p.right() - 1.0
                    && p.y < q.bottom() - 1.0
                    && q.y < p.bottom() - 1.0
                {
                    bad.push(format!("{name}: {a} over {b}"));
                }
            }
        }
    }
    assert!(bad.is_empty(), "{bad:#?}");
}

/// No arrow label lies on a card, nor on another label (a label on a card's corner, 2026-10-28).
/// `reseau-conflit` overlaps on purpose (D-033).
#[test]
fn no_label_lies_on_a_card_or_a_label() {
    let mut bad = Vec::new();
    let mut checked = 0;
    for (name, src, css) in corpus().into_iter().filter(|c| c.0 != "reseau-conflit") {
        let (d, _) = from_source(
            &src,
            &Options {
                css,
                ..Options::default()
            },
        )
        .unwrap();
        if d.model.option("layout").is_some_and(|l| l != "graph") {
            continue;
        }
        let boxes: Vec<(&String, [f64; 4])> = d
            .labels
            .iter()
            .filter_map(|(id, (cx, cy))| {
                let s = d.sizes.labels.get(id)?;
                Some((id, [cx - s.w / 2.0, cy - s.h / 2.0, s.w, s.h]))
            })
            .collect();
        checked += boxes.len();
        let hit = |a: [f64; 4], b: [f64; 4]| {
            a[0] + 1.0 < b[0] + b[2]
                && b[0] + 1.0 < a[0] + a[2]
                && a[1] + 1.0 < b[1] + b[3]
                && b[1] + 1.0 < a[1] + a[3]
        };
        for (i, (id, l)) in boxes.iter().enumerate() {
            for n in d.model.nodes.iter().filter(|n| n.kind == Kind::Card) {
                if let Some(b) = d.boxes.get(&n.id)
                    && hit(*l, [b.x, b.y, b.w, b.h])
                {
                    bad.push(format!("{name}: label of {id} on {}", n.id));
                }
            }
            for (other, m) in &boxes[i + 1..] {
                if hit(*l, *m) {
                    bad.push(format!("{name}: labels of {id} and {other}"));
                }
            }
        }
    }
    assert!(checked > 30, "only {checked} labels checked");
    assert!(bad.is_empty(), "{bad:#?}");
}

/// Every segment of a graph's route is horizontal or vertical (2026-11-09: a port's stub and the
/// segment after it, a few hundred-thousandths of a pixel apart, let a pass slant the line; less
/// than a pixel is the micro steps dropped on purpose).
#[test]
fn no_route_slants() {
    let mut bad = Vec::new();
    for (name, src, css) in corpus() {
        let (d, _) = from_source(
            &src,
            &Options {
                css,
                ..Options::default()
            },
        )
        .unwrap();
        if d.model.option("layout").is_some_and(|l| l != "graph") {
            continue;
        }
        for (id, r) in &d.routes {
            if r.windows(2)
                .any(|s| (s[0].0 - s[1].0).abs() > 1.0 && (s[0].1 - s[1].1).abs() > 1.0)
            {
                bad.push(format!("{name}: {id}"));
            }
        }
    }
    assert!(bad.is_empty(), "slanted routes: {bad:?}");
}
