//! Stability bench (D-031): `cargo run --release --example stability [-- file.skemo[:css]…]`.
//!
//! For each card of each diagram (graph mode):
//! - **pin in place**: `at=` at its current position; nothing should move;
//! - **move** by (+60, +40): only its arrows should move.
//!
//! Counts what moves without being linked to it: arrows not linked to the card whose two cards
//! stayed in place, other cards, zones that do not hold it.

use skemo_engine::diagram::{Diagram, Options, from_source};
use skemo_engine::layout::Box2;
use std::path::Path;

const DEFAULT: &[(&str, Option<&str>)] = &[
    ("reseau.pug", None),
    ("reseau-manuel.pug", None),
    ("reseau-manuel-via.pug", None),
    ("reseau-2pins.pug", None),
    ("reseau-conflit.pug", None),
    ("reseau-entete.pug", None),
    ("process.pug", Some("process.css")),
    ("design.pug", Some("design.css")),
    ("animation.pug", None),
];

thread_local! {
    static MAX_SLIDE: std::cell::Cell<f64> = const { std::cell::Cell::new(0.0) };
    static SLIDES: std::cell::RefCell<Vec<f64>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Crossings between arrows (perpendicular segments that strictly cut each other).
fn crossings(d: &Diagram) -> usize {
    let routes: Vec<&Vec<(f64, f64)>> = d.routes.values().collect();
    let mut n = 0;
    for (i, a) in routes.iter().enumerate() {
        for b in &routes[i + 1..] {
            for s in a.windows(2) {
                for t in b.windows(2) {
                    let (h, v) = if (s[0].1 - s[1].1).abs() < 1e-6 {
                        (s, t)
                    } else {
                        (t, s)
                    };
                    if (h[0].1 - h[1].1).abs() > 1e-6 || (v[0].0 - v[1].0).abs() > 1e-6 {
                        continue;
                    }
                    let (x0, x1) = (h[0].0.min(h[1].0), h[0].0.max(h[1].0));
                    let (y0, y1) = (v[0].1.min(v[1].1), v[0].1.max(v[1].1));
                    if v[0].0 > x0 + 0.5
                        && v[0].0 < x1 - 0.5
                        && h[0].1 > y0 + 0.5
                        && h[0].1 < y1 - 0.5
                    {
                        n += 1;
                    }
                }
            }
        }
    }
    n
}

fn moved_box(a: &Box2, b: &Box2) -> bool {
    (a.x - b.x).abs() > 0.5
        || (a.y - b.y).abs() > 0.5
        || (a.w - b.w).abs() > 0.5
        || (a.h - b.h).abs() > 0.5
}

/// An arrow that keeps its shape (same number of points) and shifts by at most `max` px.
fn slid(a: &[(f64, f64)], b: &[(f64, f64)], max: f64) -> bool {
    a.len() == b.len()
        && a.iter()
            .zip(b)
            .all(|(p, q)| (p.0 - q.0).abs() <= max && (p.1 - q.1).abs() <= max)
}

fn moved_route(a: &[(f64, f64)], b: &[(f64, f64)]) -> bool {
    a.len() != b.len()
        || a.iter()
            .zip(b)
            .any(|(p, q)| (p.0 - q.0).abs() > 0.5 || (p.1 - q.1).abs() > 0.5)
}

/// What moved without being linked to `card`: (arrows, cards, zones).
fn unrelated(
    before: &Diagram,
    after: &Diagram,
    card: &str,
) -> (Vec<String>, Vec<String>, Vec<String>) {
    let m = &before.model;
    let ancestors: Vec<String> = m.ancestors(card).iter().map(|n| n.id.clone()).collect();
    let mut edges = Vec::new();
    for e in &m.edges {
        if e.from == card || e.to == card {
            continue;
        }
        let still = |id: &str| match (before.boxes.get(id), after.boxes.get(id)) {
            (Some(a), Some(b)) => !moved_box(a, b),
            _ => false,
        };
        // only the arrows whose two cards did not move
        if !still(&e.from) || !still(&e.to) {
            continue;
        }
        if let (Some(a), Some(b)) = (before.routes.get(&e.id), after.routes.get(&e.id))
            && moved_route(a, b)
        {
            let tag = if slid(a, b, 16.0) { "~" } else { "" };
            if !tag.is_empty() {
                let d = a
                    .iter()
                    .zip(b)
                    .map(|(p, q)| (p.0 - q.0).abs().max((p.1 - q.1).abs()))
                    .fold(0.0, f64::max);
                MAX_SLIDE.with(|m| m.set(m.get().max(d)));
                SLIDES.with(|v| v.borrow_mut().push(d));
            }
            edges.push(format!("{tag}{}", e.id));
        }
    }
    let (mut cards, mut zones) = (Vec::new(), Vec::new());
    for n in &m.nodes {
        if n.id == card || ancestors.contains(&n.id) {
            continue;
        }
        if let (Some(a), Some(b)) = (before.boxes.get(&n.id), after.boxes.get(&n.id))
            && moved_box(a, b)
        {
            if n.kind == skemo_lang::Kind::Zone {
                zones.push(n.id.clone());
            } else {
                cards.push(n.id.clone());
            }
        }
    }
    (edges, cards, zones)
}

fn main() {
    let fx = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cases: Vec<(String, Option<String>)> = if args.is_empty() {
        DEFAULT
            .iter()
            .map(|(s, c)| {
                (
                    fx.join(s).display().to_string(),
                    c.map(|c| fx.join(c).display().to_string()),
                )
            })
            .collect()
    } else {
        args.iter()
            .map(|a| match a.split_once(':') {
                Some((s, c)) => (s.to_string(), Some(c.to_string())),
                None => (a.clone(), None),
            })
            .collect()
    };
    let verbose = std::env::var_os("STABILITY_VERBOSE").is_some();
    let (mut tot_pin, mut tot_move, mut tot_cards, mut tot_x) = (0usize, 0usize, 0usize, 0usize);
    let mut tot_slid = 0usize;
    println!(
        "{:24} {:>6} | {:>22} | {:>28}",
        "diagram", "cards", "pin: arrows(+slid)/cards/zones", "move: same"
    );
    for (src_path, css_path) in cases {
        let src = std::fs::read_to_string(&src_path).unwrap();
        let opts = Options {
            css: css_path
                .iter()
                .map(|c| std::fs::read_to_string(c).unwrap())
                .collect(),
            ..Options::default()
        };
        let Ok((base, _)) = from_source(&src, &opts) else {
            eprintln!("{src_path} : erreur");
            continue;
        };
        let cards: Vec<String> = base
            .model
            .nodes
            .iter()
            .filter(|n| n.kind == skemo_lang::Kind::Card)
            .map(|n| n.id.clone())
            .collect();
        let (mut pin, mut mv) = ([0usize; 4], [0usize; 4]);
        for c in &cards {
            let b = base.boxes[c];
            for (k, (dx, dy)) in [(0.0, 0.0), (60.0, 40.0)].into_iter().enumerate() {
                let edited = skemo_lang::edit::move_card(&src, c, b.x + dx, b.y + dy).unwrap();
                let (after, _) = from_source(&edited, &opts).unwrap();
                let (e, cs, zs) = unrelated(&base, &after, c);
                let acc = if k == 0 { &mut pin } else { &mut mv };
                acc[0] += e.iter().filter(|x| !x.starts_with('~')).count();
                acc[3] += e.iter().filter(|x| x.starts_with('~')).count();
                acc[1] += cs.len();
                acc[2] += zs.len();
                if verbose && !(e.is_empty() && cs.is_empty() && zs.is_empty()) {
                    eprintln!(
                        "  {} {c}: arrows {e:?} cards {cs:?} zones {zs:?}",
                        if k == 0 { "pin" } else { "move" }
                    );
                }
            }
        }
        let name = Path::new(&src_path)
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();
        println!(
            "{name:24} {:>6} {:>6} | {:>22} | {:>28}",
            cards.len(),
            crossings(&base),
            format!("{}+{}~/{}/{}", pin[0], pin[3], pin[1], pin[2]),
            format!("{}+{}~/{}/{}", mv[0], mv[3], mv[1], mv[2])
        );
        tot_pin += pin[0];
        tot_move += mv[0];
        tot_slid += pin[3] + mv[3];
        tot_cards += cards.len();
        tot_x += crossings(&base);
    }
    println!(
        "total: {tot_cards} cards, {tot_x} crossings; unlinked arrows routed again: {tot_pin} when pinning, {tot_move} when moving; slid (≤ 16 px): {tot_slid}"
    );
    let mut v = SLIDES.with(|v| v.borrow().clone());
    v.sort_by(f64::total_cmp);
    if !v.is_empty() {
        println!(
            "slides: median {:.1} px, max {:.1} px",
            v[v.len() / 2],
            MAX_SLIDE.with(std::cell::Cell::get)
        );
    }
}
