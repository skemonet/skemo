//! Visual review (SPEC §8): our routes as solid lines, the `expected` reference dashed.
//!
//! cargo run --example review -- fixtures/manuel-via.json > /tmp/review.svg
//! cargo run --release --example review -- --all out     SVGs + contact sheet + measurements
//! cargo run --release --example review -- --clean <fixture.json> > x.svg   routes only, no reference

#[path = "../tests/common/mod.rs"]
mod common;

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::time::Instant;

use common::{Fixture, Metrics, criteria, evaluate, expected_metrics};
use skemo_router::{Diagnostic, ObstacleKind, Point, route};

const OURS: &str = "#0f766e";
const EXPECTED: &str = "#be123c";

fn bbox(fx: &Fixture) -> (f64, f64, f64, f64) {
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for o in &fx.obstacles {
        x0 = x0.min(o.rect.x0());
        y0 = y0.min(o.rect.y0());
        x1 = x1.max(o.rect.x1());
        y1 = y1.max(o.rect.y1());
    }
    (x0 - 40.0, y0 - 70.0, x1 + 40.0, y1 + 30.0)
}

fn poly(s: &mut String, p: &[Point], stroke: &str, extra: &str) {
    let pts: Vec<String> = p.iter().map(|(x, y)| format!("{x:.1},{y:.1}")).collect();
    let _ = writeln!(
        s,
        r#"<polyline points="{}" fill="none" stroke="{stroke}" stroke-width="2" stroke-linejoin="round" {extra}/>"#,
        pts.join(" ")
    );
}

fn body(
    fx: &Fixture,
    ours: &BTreeMap<String, Vec<Point>>,
    title: &str,
    subtitle: &str,
    clean: bool,
) -> String {
    let (x0, y0, _, _) = bbox(fx);
    let mut s = String::new();
    let _ = writeln!(
        s,
        r##"<text x="{:.0}" y="{:.0}" font-size="20" font-weight="bold" fill="#111">{title}</text>"##,
        x0 + 8.0,
        y0 + 26.0
    );
    let _ = writeln!(
        s,
        r##"<text x="{:.0}" y="{:.0}" font-size="13" fill="#444">{subtitle}</text>"##,
        x0 + 8.0,
        y0 + 46.0
    );
    let b = fx.params.shape_buffer;
    for o in fx
        .obstacles
        .iter()
        .filter(|o| o.kind == ObstacleKind::Region)
    {
        let r = o.rect;
        let _ = writeln!(
            s,
            r##"<rect x="{:.1}" y="{:.1}" width="{:.1}" height="{:.1}" fill="#eef2ff" stroke="#6366f1" stroke-dasharray="8 4"/><text x="{:.1}" y="{:.1}" font-size="12" fill="#4338ca">{}</text>"##,
            r.x,
            r.y,
            r.w,
            r.h,
            r.x + 4.0,
            r.y + 14.0,
            o.id
        );
    }
    for o in fx
        .obstacles
        .iter()
        .filter(|o| o.kind == ObstacleKind::Shape)
    {
        let r = o.rect;
        let _ = writeln!(
            s,
            r##"<rect x="{:.1}" y="{:.1}" width="{:.1}" height="{:.1}" fill="none" stroke="#d4d4d8" stroke-dasharray="2 3"/><rect x="{:.1}" y="{:.1}" width="{:.1}" height="{:.1}" fill="#f4f4f5" fill-opacity="0.85" stroke="#52525b"/><text x="{:.1}" y="{:.1}" font-size="13" text-anchor="middle" fill="#18181b">{}</text>"##,
            r.x - b,
            r.y - b,
            r.w + 2.0 * b,
            r.h + 2.0 * b,
            r.x,
            r.y,
            r.w,
            r.h,
            r.x + r.w / 2.0,
            r.y + r.h / 2.0 + 4.0,
            o.id
        );
    }
    for p in fx.expected.values().filter(|_| !clean) {
        poly(
            &mut s,
            p,
            EXPECTED,
            r#"stroke-dasharray="6 4" opacity="0.8""#,
        );
    }
    for c in &fx.connectors {
        if let Some(p) = ours.get(&c.id) {
            poly(&mut s, p, OURS, r#"opacity="0.9""#);
            let (ex, ey) = p[p.len() - 1];
            let _ = writeln!(
                s,
                r#"<circle cx="{ex:.1}" cy="{ey:.1}" r="3.5" fill="{OURS}"/>"#
            );
        }
        for cp in &c.checkpoints {
            let _ = writeln!(
                s,
                r##"<circle cx="{}" cy="{}" r="5" fill="none" stroke="#111" stroke-width="1.5"/>"##,
                cp.0, cp.1
            );
        }
    }
    s
}

fn single(
    fx: &Fixture,
    ours: &BTreeMap<String, Vec<Point>>,
    title: &str,
    subtitle: &str,
    clean: bool,
) -> String {
    let (x0, y0, x1, y1) = bbox(fx);
    let (w, h) = (x1 - x0, y1 - y0);
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"{x0:.0} {y0:.0} {w:.0} {h:.0}\" width=\"{w:.0}\" height=\"{h:.0}\" font-family=\"sans-serif\">\n<rect x=\"{x0:.0}\" y=\"{y0:.0}\" width=\"{w:.0}\" height=\"{h:.0}\" fill=\"#fff\"/>\n{}</svg>\n",
        body(fx, ours, title, subtitle, clean)
    )
}

struct Done {
    fx: Fixture,
    ours: BTreeMap<String, Vec<Point>>,
    diags: Vec<Diagnostic>,
    m: Metrics,
    exp: Metrics,
    micros: f64,
}

fn run(fx: Fixture) -> Done {
    let mut times = Vec::new();
    let mut out = None;
    for _ in 0..21 {
        let t = Instant::now();
        let r = route(&fx.obstacles, &fx.connectors, &fx.params);
        times.push(t.elapsed().as_secs_f64() * 1e6);
        out = Some(r);
    }
    times.sort_by(f64::total_cmp);
    let (routes, diags) = out.unwrap();
    let ours: BTreeMap<_, _> = routes.into_iter().map(|r| (r.id, r.points)).collect();
    let m = evaluate(&fx, &ours);
    let exp = expected_metrics(&fx);
    Done {
        fx,
        ours,
        diags,
        m,
        exp,
        micros: times[times.len() / 2],
    }
}

fn subtitle(d: &Done) -> String {
    format!(
        "solid green = router · red dashed = reference · bends {} (ref {}) · length ×{:.3} · crossings {} (ref {})",
        d.m.bends,
        d.exp.bends,
        d.m.length / d.exp.length,
        d.m.crossings,
        d.exp.crossings
    )
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) != Some("--all") {
        let clean = args.first().map(String::as_str) == Some("--clean");
        let path = args
            .get(usize::from(clean))
            .expect("usage: review [--clean] <fixture.json> | --all <folder>");
        let d = run(Fixture::load(path));
        let sub = if clean { String::new() } else { subtitle(&d) };
        let title = if clean { "" } else { &d.fx.name };
        print!("{}", single(&d.fx, &d.ours, title, &sub, clean));
        return;
    }
    let out = args.get(1).cloned().unwrap_or_else(|| "out".into());
    std::fs::create_dir_all(&out).unwrap();
    let mut table = String::from(
        "| case | routed | R-01 | R-02 | R-03 | R-06 | R-07 | margin | bends (ref) | length/ref | crossings (ref) | min spacing (ref) | time | §7 criteria except spacing |\n|---|---|---|---|---|---|---|---|---|---|---|---|---|---|\n",
    );
    let mut done = Vec::new();
    for fx in Fixture::all() {
        let d = run(fx);
        let crit = criteria(&d.fx, &d.m, &d.exp, false);
        let _ = writeln!(
            table,
            "| {} | {}/{} | {} | {} | {} | {} | {} | {} | {} ({}) | {:.3} | {} ({}) | {} ({}) | {:.2} ms | {} |",
            d.fx.name,
            d.m.routed,
            d.m.total,
            d.m.r01,
            d.m.r02,
            d.m.r03,
            d.m.r06,
            d.m.r07,
            d.m.buffer,
            d.m.bends,
            d.exp.bends,
            d.m.length / d.exp.length,
            d.m.crossings,
            d.exp.crossings,
            d.m.min_spacing.map_or("—".into(), |v| format!("{v:.1}")),
            d.exp.min_spacing.map_or("—".into(), |v| format!("{v:.1}")),
            d.micros / 1000.0,
            if crit.is_empty() {
                "✅".to_string()
            } else {
                crit.join(" ; ")
            }
        );
        for diag in &d.diags {
            let _ = writeln!(table, "\n- {} : {diag:?}", d.fx.name);
        }
        std::fs::write(
            format!("{out}/{}.svg", d.fx.name),
            single(&d.fx, &d.ours, &d.fx.name, &subtitle(&d), false),
        )
        .unwrap();
        done.push(d);
    }
    // planche 3 colonnes
    let (cell, gap) = (620.0, 16.0);
    let mut s = String::new();
    let mut y = gap;
    for row in done.chunks(3) {
        let mut row_h: f64 = 0.0;
        for (c, d) in row.iter().enumerate() {
            let (x0, y0, x1, y1) = bbox(&d.fx);
            let h = (y1 - y0) * cell / (x1 - x0);
            row_h = row_h.max(h);
            let _ = writeln!(
                s,
                "<svg x=\"{:.0}\" y=\"{y:.0}\" width=\"{cell:.0}\" height=\"{h:.0}\" viewBox=\"{x0:.0} {y0:.0} {:.0} {:.0}\"><rect x=\"{x0:.0}\" y=\"{y0:.0}\" width=\"{:.0}\" height=\"{:.0}\" fill=\"#fff\" stroke=\"#ccc\"/>\n{}</svg>",
                gap + c as f64 * (cell + gap),
                x1 - x0,
                y1 - y0,
                x1 - x0,
                y1 - y0,
                body(&d.fx, &d.ours, &d.fx.name, &subtitle(d), false)
            );
        }
        y += row_h + gap;
    }
    let w = 3.0 * (cell + gap) + gap;
    std::fs::write(
        format!("{out}/planche.svg"),
        format!("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w:.0}\" height=\"{y:.0}\" font-family=\"sans-serif\">\n<rect width=\"100%\" height=\"100%\" fill=\"#f8f8f8\"/>\n{s}</svg>\n"),
    )
    .unwrap();
    std::fs::write(format!("{out}/mesures.md"), &table).unwrap();
    print!("{table}");
}
