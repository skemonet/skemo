//! Scaling (SPEC §7): `cargo run --release --example scale`.
//! Shapes on a loose grid, connectors between shapes drawn at random (fixed seed).
use skemo_router::{Connector, End, Obstacle, Params, Rect, route};
use std::time::Instant;

fn main() {
    let sizes: Vec<(usize, usize)> = std::env::args()
        .skip(1)
        .filter_map(|a| {
            let (n, m) = a.split_once('/')?;
            Some((n.parse().ok()?, m.parse().ok()?))
        })
        .collect();
    let sizes = if sizes.is_empty() {
        vec![(50, 100), (200, 400), (500, 1000)]
    } else {
        sizes
    };
    for (n, m) in sizes {
        let mut s: u64 = 42;
        let mut rnd = |k: u64| {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            s % k
        };
        let cols = (n as f64).sqrt().ceil() as u64;
        let obstacles: Vec<Obstacle> = (0..n as u64)
            .map(|i| {
                let (cx, cy) = ((i % cols) as f64 * 260.0, (i / cols) as f64 * 180.0);
                Obstacle::shape(
                    format!("s{i}"),
                    Rect::new(
                        cx + rnd(40) as f64,
                        cy + rnd(40) as f64,
                        120.0 + rnd(60) as f64,
                        50.0 + rnd(30) as f64,
                    ),
                )
            })
            .collect();
        // "local" (realistic): links to a nearby shape (at most 2 cells away); otherwise random (worst case)
        let local = std::env::var_os("SCALE_RANDOM").is_none();
        let connectors: Vec<Connector> = (0..m)
            .map(|k| {
                let a = rnd(n as u64);
                let mut b = if local {
                    let (ax, ay) = ((a % cols) as i64, (a / cols) as i64);
                    let (dx, dy) = (rnd(5) as i64 - 2, rnd(5) as i64 - 2);
                    let (bx, by) = ((ax + dx).clamp(0, cols as i64 - 1), (ay + dy).max(0));
                    ((by * cols as i64 + bx) as u64).min(n as u64 - 1)
                } else {
                    rnd(n as u64)
                };
                if b == a {
                    b = (a + 1) % n as u64;
                }
                Connector::new(
                    format!("c{k}"),
                    End::shape(format!("s{a}")),
                    End::shape(format!("s{b}")),
                )
            })
            .collect();
        let p0 = Params {
            ideal_nudging: 0.0,
            ..Params::default()
        };
        let t = Instant::now();
        let _ = route(&obstacles, &connectors, &p0);
        println!("  sans R2 : {:.0} ms", t.elapsed().as_secs_f64() * 1000.0);
        let t = Instant::now();
        let (routes, diags) = route(&obstacles, &connectors, &Params::default());
        // fingerprint of the routes (FNV-1a over the coordinates): an optimisation must not change it
        let mut fp: u64 = 0xcbf29ce484222325;
        for r in &routes {
            for p in &r.points {
                for v in [p.0, p.1] {
                    fp = (fp ^ v.to_bits()).wrapping_mul(0x100000001b3);
                }
            }
        }
        println!(
            "{n} shapes / {m} connectors: {:.0} ms, {} routes, {} diagnostics, fingerprint {fp:016x}",
            t.elapsed().as_secs_f64() * 1000.0,
            routes.len(),
            diags.len()
        );
    }
}
