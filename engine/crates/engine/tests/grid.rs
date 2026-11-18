//! The grid engine (`diagram(layout="grid")`, the mosaic and its routing rules): what it promises
//! on a small map of zones.

use skemo_engine::session::Session;
use skemo_engine::{Options, from_source, quality};
use skemo_lang::edit::move_card;

/// Groups side by side, a description taking room under one title, a right tag, labelled arrows
/// between groups and inside them, a nested zone, a legend docked top right.
const MAP: &str = "diagram(layout=\"grid\")
  title Mini map
    |.pill level 2
  zone#a.tag-right(tag=\"open-cluster-management-observability\") Alpha
    | A description long enough to wrap on a second line under the title of the zone
    card#hub Hub
      | installs everything
      to(op) read by
      to(eng)
    card#op Operator
      | keeps the parts up to date
      to(idx) deploys
      to(pl)
  zone#b Beta
    card#eng Engine
      | the registry's operator
      to(reg) installs
    card#reg Registry
      | every cluster
  zone#c Gamma
    card#pl Placement
      | chooses clusters
      to(dec) decides
    card#dec Decision
      | the chosen ones
      to(reg)
  zone#d Delta
    zone#d1 Inner one
      card#idx Indexer
        | ranks the inventory
    zone#d2 Inner two
      card#bkp Backup
        | every hour
        to(hub) saves
  card#key.legend(at=\"ne\")
    | legend
";

fn grid(src: &str) -> skemo_engine::Diagram {
    from_source(src, &Options::default()).unwrap().0
}

#[test]
fn the_mosaic_hides_nothing() {
    let d = grid(MAP);
    let defects = quality::defects(&d);
    for p in [
        "end-off-card",
        "zone-overlap",
        "card-outside-zone",
        "card-overlap",
        "zone-title-overflow",
        "text-overflow",
        "through-card",
        "label-overlap",
    ] {
        assert!(defects[p].is_empty(), "{p}: {:?}", defects[p]);
    }
}

#[test]
fn every_arrow_is_routed_with_a_tail() {
    let d = grid(MAP);
    assert_eq!(d.routes.len(), d.model.edges.len());
    for (id, r) in &d.routes {
        assert!(r.len() >= 2, "{id}: {r:?}");
        let (p, q) = (r[r.len() - 2], r[r.len() - 1]);
        let tail = (p.0 - q.0).abs() + (p.1 - q.1).abs();
        // a straight arrow between two close cards is all tail; otherwise the head has a line
        // behind it
        assert!(r.len() == 2 || tail >= 14.0, "{id}: tail {tail} in {r:?}");
    }
}

#[test]
fn the_first_cards_of_a_row_of_zones_line_up() {
    let d = grid(MAP);
    // Alpha's description takes room under its title; Beta's and Gamma's first cards still stand
    // as high as Alpha's when the three share a row of the mosaic
    let top = |id: &str| d.boxes[id].y;
    let (a, b) = (d.boxes["a"], d.boxes["b"]);
    if (a.y - b.y).abs() < 0.5 {
        assert!((top("hub").min(top("op")) - top("eng").min(top("reg"))).abs() < 1.0);
    }
}

#[test]
fn no_line_runs_through_a_zone_that_holds_neither_end() {
    let d = grid(MAP);
    let holds = |z: &str, id: &str| d.model.ancestors(id).iter().any(|n| n.id == z);
    for e in &d.model.edges {
        let r = &d.routes[&e.id];
        for n in d
            .model
            .nodes
            .iter()
            .filter(|n| n.kind == skemo_lang::Kind::Zone)
        {
            if holds(&n.id, &e.from) || holds(&n.id, &e.to) {
                continue;
            }
            let b = d.boxes[&n.id];
            for w in r.windows(2) {
                let (p, q) = (w[0], w[1]);
                let inside = |v: f64, lo: f64, hi: f64| lo + 1.0 < v && v < hi - 1.0;
                let mid = ((p.0 + q.0) / 2.0, (p.1 + q.1) / 2.0);
                assert!(
                    !(inside(mid.0, b.x, b.right()) && inside(mid.1, b.y, b.bottom())),
                    "{} through {}: {r:?}",
                    e.id,
                    n.id
                );
            }
        }
    }
}

/// The hard guarantee of editing (D-031; rank 1 since D-108): a pin names a place, it changes
/// nothing else.
#[test]
fn pinning_a_card_where_it_stands_moves_nothing() {
    let d = grid(MAP);
    let (pin, _) = quality::stability(MAP, &Options::default(), &d);
    assert!(pin.is_empty(), "{pin:?}");
    // not even the card: the grid sets cards on fractions of a pixel, `at` is written in whole
    // pixels, and the pin that names the grid's own place is that place
    for id in ["hub", "reg", "dec", "idx"] {
        let b = d.boxes[id];
        let pinned = grid(&move_card(MAP, id, b.x, b.y).unwrap());
        assert_eq!(pinned.boxes, d.boxes, "{id}");
        assert_eq!(pinned.routes, d.routes, "{id}");
    }
}

#[test]
fn a_moved_card_keeps_its_size_and_its_zones_grow_to_it() {
    let d = grid(MAP);
    // Indexer stands in Inner one, in Delta: both frames follow it
    let b = d.boxes["idx"];
    let moved = grid(&move_card(MAP, "idx", b.x + 400.0, b.y + 300.0).unwrap());
    let c = moved.boxes["idx"];
    assert_eq!((c.x, c.y), ((b.x + 400.0).round(), (b.y + 300.0).round()));
    assert_eq!((c.w, c.h), (b.w, b.h));
    let (inner, outer) = (moved.boxes["d1"], moved.boxes["d"]);
    let holds = |z: &skemo_engine::layout::Box2, c: &skemo_engine::layout::Box2| {
        z.x < c.x && c.right() < z.right() && z.y < c.y && c.bottom() < z.bottom()
    };
    assert!(holds(&inner, &c), "{inner:?} {c:?}");
    assert!(holds(&outer, &inner), "{outer:?} {inner:?}");
    assert!(quality::defects(&moved)["card-outside-zone"].is_empty());
    // the zones that do not hold it stay
    for z in ["a", "b", "c", "d2"] {
        assert_eq!(moved.boxes[z], d.boxes[z], "{z}");
    }
}

/// The editor's preview of a drag is the drawing the file gives once `at=` is written.
#[test]
fn a_drag_previews_what_the_file_then_draws() {
    // in the grid and in the graph alike; the docked legend stays where it is
    for layout in ["grid", "graph"] {
        let src = MAP.replace("layout=\"grid\"", &format!("layout=\"{layout}\""));
        let mut s = Session::new(&src, Options::default()).unwrap();
        assert_eq!(s.diagram.grid, layout == "grid");
        // (a drag that widens the drawing is another matter: the preview leaves a docked
        // element where it stood, the file sets it around the wider drawing)
        let b = s.diagram.boxes["reg"];
        s.drag("reg", b.x + 7.3, b.y + 231.8).unwrap();
        let preview = (s.diagram.boxes.clone(), s.diagram.routes.clone());
        let reloaded = grid(&s.commit().unwrap());
        assert_eq!(preview.0, reloaded.boxes, "{layout}");
        assert_eq!(preview.1, reloaded.routes, "{layout}");
    }
}

/// A bare zone draws nothing, yet the zones it holds are tiles like any other.
#[test]
fn zones_in_a_bare_zone_are_laid_out() {
    let src = "diagram(layout=\"grid\")
  zone#row.bare
    zone#a Alpha
      card#x X
        to(y)
      card#w W
    zone#b Beta
      card#y Y
        to(z) reads
  zone#c Gamma
    card#z Z
      to(x)
";
    let d = grid(src);
    let defects = quality::defects(&d);
    for p in [
        "zone-overlap",
        "card-outside-zone",
        "card-overlap",
        "end-off-card",
    ] {
        assert!(defects[p].is_empty(), "{p}: {:?}", defects[p]);
    }
    assert_eq!(d.routes.len(), 3);
}

#[test]
fn the_layout_says_which_engine_placed_it() {
    assert_eq!(grid(MAP).layout_json()["layout"]["engine"], "grid");
    let graph = MAP.replace("layout=\"grid\"", "layout=\"graph\"");
    let (d, warnings) = from_source(&graph, &Options::default()).unwrap();
    assert!(!d.grid);
    assert_eq!(d.layout_json()["layout"]["engine"], "elk");
    assert!(warnings.iter().all(|w| w.code != "unknown-layout"));
}

/// The manual method's §7: the tail behind a head, the lines apart and the gaps of a group even
/// hold together; none yields to another.
#[test]
fn room_is_made_evenly_and_no_rule_yields() {
    let d = grid(MAP);
    let defects = quality::defects(&d);
    for p in ["uneven-gaps", "close-lines", "short-tail", "label-on-frame"] {
        assert!(defects[p].is_empty(), "{p}: {:?}", defects[p]);
    }
}

/// A label too long for the gap between two zones widens every gap between the zones, not that
/// one alone, and the gaps inside the zones stay what they were.
#[test]
fn a_long_label_widens_every_gap_of_its_group() {
    let src = "diagram(layout=\"grid\")
  zone#z1 One
    card#a1 A1
      to(a2)
      to(b1) a label far longer than the gap between two zones
    card#a2 A2
      to(a3)
    card#a3 A3
  zone#z2 Two
    card#b1 B1
      to(b2)
    card#b2 B2
      to(b3)
    card#b3 B3
  zone#z3 Three
    card#c1 C1
      to(c2)
    card#c2 C2
      to(c3)
    card#c3 C3
";
    let d = grid(src);
    let gap = |l: &str, r: &str| d.boxes[r].x - d.boxes[l].right();
    let under = |t: &str, b: &str| d.boxes[b].y - d.boxes[t].bottom();
    // the three zones stand in a row, their cards one under the other
    assert!(
        (d.boxes["z1"].y - d.boxes["z3"].y).abs() < 0.5,
        "{:?}",
        d.boxes
    );
    assert!(gap("z1", "z2") > 100.0, "{}", gap("z1", "z2"));
    assert!((gap("z1", "z2") - gap("z2", "z3")).abs() < 0.5);
    assert!(
        (under("a1", "a2") - 28.0).abs() < 0.5,
        "{}",
        under("a1", "a2")
    );
    let defects = quality::defects(&d);
    for p in [
        "uneven-gaps",
        "label-on-frame",
        "label-overlap",
        "through-label",
    ] {
        assert!(defects[p].is_empty(), "{p}: {:?}", defects[p]);
    }
}

/// A label too long for a zone, on an arrow between two of its cards one under the other, widens
/// that zone's padding, the same on both sides.
#[test]
fn a_long_label_in_a_zone_widens_its_padding_on_both_sides() {
    let src = "diagram(layout=\"grid\")
  zone#z Zone
    card#a A
      to(b) a label far longer than the zone is wide
    card#b B
  zone#y Other
    card#d D
      to(e)
    card#e E
";
    let d = grid(src);
    let (z, a) = (d.boxes["z"], d.boxes["a"]);
    assert!(
        (d.boxes["a"].x - d.boxes["b"].x).abs() < 0.5,
        "{:?}",
        d.boxes
    );
    assert!(a.x - z.x > 60.0, "{}", a.x - z.x);
    assert!(((a.x - z.x) - (z.right() - a.right())).abs() < 0.5);
    let defects = quality::defects(&d);
    for p in [
        "uneven-gaps",
        "label-on-frame",
        "label-overlap",
        "through-label",
    ] {
        assert!(defects[p].is_empty(), "{p}: {:?}", defects[p]);
    }
}

/// Two arrows between the same two cards, one each way, both labelled: their straight lines move
/// apart until the labels stand side by side, each over its own line.
#[test]
fn two_labels_on_lines_side_by_side_stand_apart() {
    let src = "diagram(layout=\"grid\")
  zone#z Things
    card#a First card
      | makes the reports
      to(gen) one each
    card#b Second card
      | a longer line of text that wraps
      to(gen) one each, state
    card#p Project
      | limits what may be deployed, and where
      to(gen).read project
    card#gen Generated
      | several objects of several kinds, made by the others
      to(p) default
";
    let d = grid(src);
    let defects = quality::defects(&d);
    for p in ["label-overlap", "through-label"] {
        assert!(defects[p].is_empty(), "{p}: {:?}", defects[p]);
    }
}

#[test]
fn rows_of_cards_line_up_from_one_zone_to_the_next() {
    // the left zone's cards are taller and its arrows labelled (its gutters widen): the right
    // zone's rows still stand at the same heights
    let src = "diagram(layout=\"grid\")
  zone#l Left
    card#l1 First
      | a description long enough to wrap on a second line, and a third one too
      to(l2) one per cluster, state
    card#l2 Second
      | made by the first
      to(l3) default
    card#l3 Third
      | limits the second
  zone#r Right
    card#r1 Source
      | short
      to(r2)
    card#r2 Middle
      | short
      to(r3)
    card#r3 End
      | short
";
    let d = grid(src);
    for (a, b) in [("l1", "r1"), ("l2", "r2"), ("l3", "r3")] {
        let (ya, yb) = (d.boxes[a].y, d.boxes[b].y);
        assert!((ya - yb).abs() < 0.5, "{a} at {ya}, {b} at {yb}");
    }
}
