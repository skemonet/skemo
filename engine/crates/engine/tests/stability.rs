//! Stability (D-031): pinning a card in its place changes nothing else, and the drag preview is
//! identical to the full computation after the drop (idempotence on reopening).

use skemo_engine::diagram::{Options, from_source};
use skemo_engine::session::Session;
use std::path::Path;

const CASES: &[(&str, Option<&str>)] = &[
    ("reseau.pug", None),
    ("reseau-manuel.pug", None),
    ("reseau-manuel-via.pug", None),
    ("reseau-2pins.pug", None),
    ("reseau-entete.pug", None),
    ("process.pug", Some("process.css")),
    ("design.pug", Some("design.css")),
    ("animation.pug", None),
];

fn load(name: &str, css: Option<&str>) -> (String, Options) {
    let fx = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    let src = std::fs::read_to_string(fx.join(name)).unwrap();
    let opts = Options {
        css: css
            .map(|c| std::fs::read_to_string(fx.join(c)).unwrap())
            .into_iter()
            .collect(),
        ..Options::default()
    };
    (src, opts)
}

fn close(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() <= tol
}

/// Pinning a card in its place (up to the pixel rounding): no zone nor other card moves, no arrow
/// is routed again (it may slide by the half-pixel of rounding).
#[test]
fn pin_in_place_changes_nothing() {
    let mut failures = Vec::new();
    for (name, css) in CASES {
        let (src, opts) = load(name, *css);
        let (base, _) = from_source(&src, &opts).unwrap();
        for card in base
            .model
            .nodes
            .iter()
            .filter(|n| n.kind == skemo_lang::Kind::Card)
        {
            let b = base.boxes[&card.id];
            let edited = skemo_lang::edit::move_card(&src, &card.id, b.x, b.y).unwrap();
            let (after, _) = from_source(&edited, &opts).unwrap();
            for (id, a) in &base.boxes {
                let c = after.boxes[id];
                // the pinned card and its zones follow the pixel rounding (≤ 0.5 px)
                if !(close(a.x, c.x, 0.5)
                    && close(a.y, c.y, 0.5)
                    && close(a.w, c.w, 1.0)
                    && close(a.h, c.h, 1.0))
                {
                    failures.push(format!("{name}: pinning {} moves {id}", card.id));
                }
            }
            for (id, r) in &base.routes {
                let s = &after.routes[id];
                if r.len() != s.len() {
                    failures.push(format!("{name} : figer {} retrace {id}", card.id));
                }
            }
        }
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}

/// During a drag, the preview (session) is exactly the drawing the file will give once `at=` is
/// written and read again.
#[test]
fn drag_preview_equals_reload() {
    for (name, css) in CASES {
        let (src, opts) = load(name, *css);
        let mut s = Session::new(&src, opts.clone()).unwrap();
        let cards: Vec<String> = s
            .diagram
            .model
            .nodes
            .iter()
            .filter(|n| n.kind == skemo_lang::Kind::Card)
            .map(|n| n.id.clone())
            .collect();
        let id = &cards[cards.len() / 2];
        let b = s.diagram.boxes[id];
        s.drag(id, b.x + 47.3, b.y + 31.8).unwrap();
        let preview = (s.diagram.boxes.clone(), s.diagram.routes.clone());
        let new_src = s.commit().unwrap();
        let (reloaded, _) = from_source(&new_src, &opts).unwrap();
        assert_eq!(preview.0, reloaded.boxes, "{name}: boxes ({id})");
        assert_eq!(preview.1, reloaded.routes, "{name}: routes ({id})");
    }
}
