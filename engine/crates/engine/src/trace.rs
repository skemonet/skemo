//! `SKEMO_TRACE=id,id…` (the layout method, phase 0): prints on the standard error the route of
//! each named arrow after every routing pass, and the box of each named card or zone after every
//! tidy pass that moves it, to find by experiment the stage that causes a defect. Arrows are named
//! by their id (`a->b`, as in `skemo layout --json`); an id ending with `*` names every arrow or
//! node starting with it. The router prints its own passes (`router/src/nudge.rs`).

use crate::layout::Box2;
use std::collections::BTreeMap;
use std::sync::OnceLock;

fn names() -> &'static [String] {
    static N: OnceLock<Vec<String>> = OnceLock::new();
    N.get_or_init(|| {
        std::env::var("SKEMO_TRACE")
            .map(|v| {
                v.split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            })
            .unwrap_or_default()
    })
}

/// Is `id` traced?
pub fn wanted(id: &str) -> bool {
    names().iter().any(|n| match n.strip_suffix('*') {
        Some(p) => id.starts_with(p),
        None => id == n,
    })
}

/// `SKEMO_JITTER=px` (the layout method's noise band, D-108's handoff): pixels added to every
/// gutter of the grid engine. A change no rule asks for: what the counts do under it is noise,
/// the band a change must beat. 0 by default.
pub fn jitter() -> f64 {
    static J: OnceLock<f64> = OnceLock::new();
    *J.get_or_init(|| {
        std::env::var("SKEMO_JITTER")
            .ok()
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(0.0)
    })
}

/// Is anything traced? (a cheap test before building what to print)
pub fn on() -> bool {
    !names().is_empty()
}

fn pt(p: (f64, f64)) -> String {
    format!("({:.1}, {:.1})", p.0, p.1)
}

/// The traced routes after routing stage `stage`.
pub fn routes(stage: &str, routes: &BTreeMap<String, Vec<(f64, f64)>>) {
    if !on() {
        return;
    }
    for (id, r) in routes.iter().filter(|(id, _)| wanted(id)) {
        let pts: Vec<String> = r.iter().map(|p| pt(*p)).collect();
        eprintln!("trace route {stage:<12} {id}: {}", pts.join(" "));
    }
}

/// The traced boxes that tidy pass `stage` moved.
pub fn boxes(stage: &str, before: &BTreeMap<String, Box2>, after: &BTreeMap<String, Box2>) {
    if !on() {
        return;
    }
    for (id, b) in after.iter().filter(|(id, _)| wanted(id)) {
        if before.get(id) != Some(b) {
            let was = before.get(id).map_or("-".to_string(), |a| {
                format!("{:.1},{:.1} {:.0}×{:.0}", a.x, a.y, a.w, a.h)
            });
            eprintln!(
                "trace box   {stage:<12} {id}: {was} → {:.1},{:.1} {:.0}×{:.0}",
                b.x, b.y, b.w, b.h
            );
        }
    }
}

/// Is pass `name` switched off by `SKEMO_OFF`?
pub fn off(name: &str) -> bool {
    static OFF: OnceLock<Vec<String>> = OnceLock::new();
    OFF.get_or_init(|| {
        std::env::var("SKEMO_OFF")
            .map(|v| {
                v.split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            })
            .unwrap_or_default()
    })
    .iter()
    .any(|n| n == name)
}
