//! R-11 (determinism) and R-12 (no panic) on degenerate or random inputs.

use skemo_router::{Connector, Diagnostic, End, Obstacle, Params, Rect, route};

/// Deterministic pseudo-random generator (xorshift), without dependency.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n.max(1)
    }

    fn coord(&mut self) -> f64 {
        // ordinary values, but also zero, negative, not finite and very large ones
        match self.below(20) {
            0 => f64::NAN,
            1 => f64::INFINITY,
            2 => 0.0,
            3 => 1e300,
            4 => -(self.below(500) as f64),
            _ => self.below(800) as f64,
        }
    }
}

fn random_scene(rng: &mut Rng) -> (Vec<Obstacle>, Vec<Connector>) {
    let n = 2 + rng.below(8);
    let obstacles: Vec<Obstacle> = (0..n)
        .map(|k| {
            // identifiants parfois en double
            let id = format!("s{}", if rng.below(6) == 0 { 0 } else { k });
            let r = Rect::new(rng.coord(), rng.coord(), rng.coord(), rng.coord());
            if rng.below(4) == 0 {
                Obstacle::region(&id, r)
            } else {
                Obstacle::shape(&id, r)
            }
        })
        .collect();
    let connectors = (0..rng.below(8))
        .map(|k| {
            // ends sometimes unknown
            let end = |rng: &mut Rng| End::shape(format!("s{}", rng.below(n + 2)));
            Connector::new(format!("c{k}"), end(rng), end(rng))
        })
        .collect();
    (obstacles, connectors)
}

#[test]
fn never_panics_on_random_degenerate_input() {
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    for _ in 0..300 {
        let (obstacles, connectors) = random_scene(&mut rng);
        let (routes, _) = route(&obstacles, &connectors, &Params::default());
        for r in routes {
            assert!(
                r.points.iter().all(|p| p.0.is_finite() && p.1.is_finite()),
                "route not finite for {}",
                r.id
            );
        }
    }
}

#[test]
fn same_input_same_output() {
    let mut rng = Rng(42);
    for _ in 0..30 {
        let (obstacles, connectors) = random_scene(&mut rng);
        let a = route(&obstacles, &connectors, &Params::default());
        let b = route(&obstacles, &connectors, &Params::default());
        assert_eq!(a.0, b.0);
    }
}

#[test]
fn invalid_input_is_reported_not_ignored() {
    let obstacles = [
        Obstacle::shape("a", Rect::new(0.0, 0.0, 50.0, 50.0)),
        Obstacle::shape("a", Rect::new(200.0, 0.0, 50.0, 50.0)),
        Obstacle::shape("nan", Rect::new(f64::NAN, 0.0, 10.0, 10.0)),
        Obstacle::shape("b", Rect::new(300.0, 0.0, 50.0, 50.0)),
    ];
    let c = [
        Connector::new("ok", End::shape("a"), End::shape("b")),
        Connector::new("inconnue", End::shape("a"), End::shape("absente")),
    ];
    let (routes, diags) = route(&obstacles, &c, &Params::default());
    assert_eq!(routes.len(), 1);
    assert_eq!(routes[0].id, "ok");
    let invalid = diags
        .iter()
        .filter(|d| matches!(d, Diagnostic::InvalidInput { .. }))
        .count();
    assert_eq!(invalid, 2, "doublon et rectangle non fini : {diags:?}");
    assert!(
        diags
            .iter()
            .any(|d| matches!(d, Diagnostic::Unroutable { id, .. } if id == "inconnue"))
    );
}
