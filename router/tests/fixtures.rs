//! Criteria of SPEC §7 on the test sets (R1: everything but the spacing).

mod common;

use std::collections::BTreeMap;

use common::{Fixture, criteria, evaluate, expected_metrics};
use skemo_router::route;

#[test]
fn fixtures_meet_r1_criteria() {
    let mut failures = Vec::new();
    for fx in Fixture::all() {
        let (routes, diags) = route(&fx.obstacles, &fx.connectors, &fx.params);
        let map: BTreeMap<_, _> = routes.into_iter().map(|r| (r.id, r.points)).collect();
        let ours = evaluate(&fx, &map);
        let exp = expected_metrics(&fx);
        for d in &diags {
            failures.push(format!("{} : diagnostic {d:?}", fx.name));
        }
        for e in criteria(&fx, &ours, &exp, false) {
            failures.push(format!("{} : {e}", fx.name));
        }
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}

/// R2: every criterion of SPEC §7, spacing included.
#[test]
fn fixtures_meet_r2_criteria() {
    let mut failures = Vec::new();
    for fx in Fixture::all() {
        let (routes, _) = route(&fx.obstacles, &fx.connectors, &fx.params);
        let map: BTreeMap<_, _> = routes.into_iter().map(|r| (r.id, r.points)).collect();
        let ours = evaluate(&fx, &map);
        let exp = expected_metrics(&fx);
        // timeline (post-v1): gutters of 22 to 26 px between lanes, margin 8 px: three parallel segments
        // cannot be 12 px apart there without touching the lanes. A documented exception (SPEC §7, the
        // engine's F-TL-6); at least 6 px is required.
        let spacing = fx.name != "timeline";
        if !spacing && ours.min_spacing.is_some_and(|s| s < 6.0 - 1e-6) {
            failures.push(format!(
                "{} : espacement {:?} < 6 px",
                fx.name, ours.min_spacing
            ));
        }
        for e in criteria(&fx, &ours, &exp, spacing) {
            failures.push(format!("{} : {e}", fx.name));
        }
        eprintln!(
            "{:14} espacement min {:?}, croisements {} (expected {})",
            fx.name, ours.min_spacing, ours.crossings, exp.crossings
        );
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}

#[test]
fn deterministic() {
    for fx in Fixture::all() {
        let a = route(&fx.obstacles, &fx.connectors, &fx.params);
        let b = route(&fx.obstacles, &fx.connectors, &fx.params);
        assert_eq!(format!("{a:?}"), format!("{b:?}"), "{}", fx.name);
    }
}
