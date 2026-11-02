//! Arrows in motion (D-079, FORMAT §4.3): `.flow`, dashes that run towards the head, and `.pulse`,
//! one glowing dot that runs along the line from the source to the head, then starts again. The
//! theme asks for them with `--flow: on` and `--pulse: on` on an arrow's line; `--flow-speed`,
//! `--pulse-speed` (px per second), `--pulse-size` and `--pulse-color` tune them.
//!
//! One clock for every output: a still image draws a moving line as a plain solid line and no dot;
//! the HTML and the SVG asked for with `--motion` play CSS keyframes; the frames of an animated
//! image are drawn at their instant. Every pulse loops on one period, the time the slowest dot
//! takes to cross its arrow: the dots of all arrows leave their sources together, at their speed,
//! and a dot that has arrived waits, off the line, for the next round.

use crate::diagram::Diagram;
use crate::svg::{esc, n};
use skemo_lang::style::{Computed, Rgba, parse_length};
use std::collections::BTreeSet;
use std::fmt::Write as _;

/// The instant an image shows.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Clock {
    /// A still image: moving lines solid, no dot.
    Still,
    /// CSS keyframes, played by the browser (HTML, SVG with `--motion`).
    Css,
    /// The instant `t` (s) of a frame of an animated image.
    At(f64),
}

/// Default speeds (px per second) and the dot's diameter (px). 11 px/s is the theme's former
/// one dash period per second.
pub const FLOW_SPEED: f64 = 11.0;
pub const PULSE_SPEED: f64 = 120.0;
pub const PULSE_SIZE: f64 = 6.0;

/// A line whose dashes run: their period (px) and speed (px/s).
pub struct Flow {
    pub period: f64,
    pub speed: f64,
}

impl Flow {
    /// The time one dash period takes to run by.
    pub fn seconds(&self) -> f64 {
        self.period / self.speed
    }

    /// The dash offset at instant `t`: negative moves the dashes towards the end of the path.
    pub fn offset(&self, t: f64) -> f64 {
        -(t * self.speed).rem_euclid(self.period)
    }
}

/// A line with a pulse: the dot's diameter, colour and speed.
pub struct Pulse {
    pub size: f64,
    pub color: Rgba,
    pub speed: f64,
}

fn on(c: &Computed, name: &str) -> bool {
    c.get(name).map(str::trim) == Some("on")
}

/// A speed in px per second, written as a length (`120px`) or a bare number.
fn speed(c: &Computed, name: &str, default: f64) -> f64 {
    c.get(name)
        .map(str::trim)
        .and_then(|v| {
            v.parse::<f64>()
                .ok()
                .or_else(|| parse_length(v, c.font_size(), None))
        })
        .filter(|s| s.is_finite() && *s > 0.0)
        .unwrap_or(default)
}

/// The dashes a style asks for (`stroke-dasharray`), in px; empty for none.
pub fn dashes(c: &Computed) -> Vec<f64> {
    c.get("stroke-dasharray")
        .filter(|v| v.trim() != "none")
        .map(|da| {
            da.replace(',', " ")
                .split_whitespace()
                .filter_map(|t| {
                    t.parse::<f64>()
                        .ok()
                        .or_else(|| parse_length(t, c.font_size(), None))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The flow of a line (`--flow: on`), when it has dashes to move.
pub fn flow(c: &Computed) -> Option<Flow> {
    if !on(c, "--flow") {
        return None;
    }
    let mut period: f64 = dashes(c).iter().sum();
    if dashes(c).len() % 2 == 1 {
        // an odd list repeats, as in CSS
        period *= 2.0;
    }
    (period.is_finite() && period > 0.0).then(|| Flow {
        period,
        speed: speed(c, "--flow-speed", FLOW_SPEED),
    })
}

/// The pulse of a line (`--pulse: on`); its colour defaults to the line's.
pub fn pulse(c: &Computed) -> Option<Pulse> {
    if !on(c, "--pulse") {
        return None;
    }
    let size = c
        .length("--pulse-size")
        .filter(|s| s.is_finite() && *s > 0.0)
        .unwrap_or(PULSE_SIZE);
    let color = c
        .var_color("--pulse-color")
        .or_else(|| c.color_of("stroke"))
        .unwrap_or_else(|| c.color());
    Some(Pulse {
        size,
        color,
        speed: speed(c, "--pulse-speed", PULSE_SPEED),
    })
}

/// The line a pulse runs along, in the drawing's frame: the whole arrow, a join's shared trunk
/// included, to where its line ends.
pub fn pulse_line(d: &Diagram, id: &str) -> Option<Vec<(f64, f64)>> {
    let r = d.routes.get(id).filter(|r| r.len() >= 2)?;
    let pts: Vec<(f64, f64)> = r
        .iter()
        .map(|p| (p.0 + d.frame.ox, p.1 + d.frame.oy))
        .collect();
    let a = crate::arrow::arrow(&pts);
    Some(crate::arrow::drawn_line(d, id, &a, &pts))
}

/// Length of a drawn line with its bridges (each a half circle instead of its diameter).
pub fn length(line: &[(f64, f64)], hops: usize) -> f64 {
    crate::route::length(line) + hops as f64 * (std::f64::consts::PI - 2.0) * crate::arrow::HOP
}

/// The loop of the diagram's motion (s): the time the slowest pulse takes to cross its arrow,
/// stretched to a whole number of the slowest flow's periods so that its dashes loop with it;
/// `None` when nothing moves.
pub fn period(d: &Diagram) -> Option<f64> {
    let (mut pulses, mut flows) = (0.0f64, 0.0f64);
    for e in &d.model.edges {
        let Some(el) = d.scene.edges.get(&e.id) else {
            continue;
        };
        if d.routes.get(&e.id).is_none_or(|r| r.len() < 2) {
            continue;
        }
        let c = d.scene.style(el.line);
        if let Some(f) = flow(c) {
            flows = flows.max(f.seconds());
        }
        if let (Some(p), Some(line)) = (pulse(c), pulse_line(d, &e.id)) {
            let hops = crate::arrow::hops(d, &e.id, (d.frame.ox, d.frame.oy)).len();
            pulses = pulses.max(length(&line, hops) / p.speed);
        }
    }
    match (pulses > 0.0, flows > 0.0) {
        (false, false) => None,
        (false, true) => Some(flows),
        (true, false) => Some(pulses),
        (true, true) => Some((pulses / flows).ceil() * flows),
    }
}

/// The keyframes a rendering uses: each shifts a line's dashes by one distance, named after it.
#[derive(Debug, Default)]
pub struct Keyframes {
    prefix: String,
    shifts: BTreeSet<String>,
}

impl Keyframes {
    pub fn new(prefix: &str) -> Self {
        Keyframes {
            prefix: prefix.to_string(),
            shifts: BTreeSet::new(),
        }
    }

    /// The `style` attribute that shifts a line's dashes by `dist` px every `secs` seconds.
    pub fn animate(&mut self, dist: f64, secs: f64) -> String {
        let k = n(dist);
        let name = format!("{}-shift-{}", self.prefix, k.replace('.', "_"));
        self.shifts.insert(k);
        format!(
            " style=\"animation:{name} {}s linear infinite\"",
            (secs * 1000.0).round() / 1000.0
        )
    }

    /// The CSS of the keyframes used, and the rule that stops them for a reader who asks for
    /// less motion; empty when none is used.
    pub fn css(&self) -> String {
        if self.shifts.is_empty() {
            return String::new();
        }
        let mut s = String::new();
        for k in &self.shifts {
            let _ = write!(
                s,
                "@keyframes {}-shift-{}{{to{{stroke-dashoffset:-{k}px}}}}",
                self.prefix,
                k.replace('.', "_")
            );
        }
        s.push_str(
            "@media (prefers-reduced-motion:reduce){.edge .line{animation:none!important}.edge .pulse{display:none}}",
        );
        s
    }
}

/// `true` when the theme's background is dark: a pulse's dot then gets a bright centre (on a
/// light background a bright centre reads as a ring).
pub fn dark(d: &Diagram) -> bool {
    d.scene
        .style(d.scene.stage)
        .var_color("--bg")
        .is_some_and(|c| c.luminance() < 0.2)
}

/// The dot of a pulse along the path `path` (`len` px long), as SVG: two faint wide strokes for
/// the halo, the dot in its colour and, on a `dark` background, a bright centre; each one dash of
/// nothing with round caps, so that the dot follows
/// the path's corners and bridges. Within one period the dot runs `period × speed`, past the head
/// for every arrow but the slowest: off the path, it shows nothing until the next round. Nothing
/// in a still image.
pub fn pulse_svg(
    p: &Pulse,
    path: &str,
    len: f64,
    clock: Clock,
    period: f64,
    dark: bool,
    kf: &mut Keyframes,
) -> String {
    let travel = period * p.speed;
    let motion = match clock {
        Clock::Still => return String::new(),
        Clock::Css => kf.animate(travel, period),
        Clock::At(t) => format!(
            " stroke-dashoffset=\"{}\"",
            n(-(t.rem_euclid(period)) * p.speed)
        ),
    };
    // one dot per arrow: the gap is longer than the run and the line together
    let gap = travel + len + p.size * 4.0 + 1.0;
    let core = mix(p.color, Rgba::WHITE, 0.6);
    let mut out = String::from("<g class=\"pulse\" fill=\"none\" stroke-linecap=\"round\">");
    let centre = dark.then_some((0.45, 1.0, core));
    let layers = [
        (3.4, 0.14, p.color),
        (2.2, 0.3, p.color),
        (1.0, 1.0, p.color),
    ];
    for (width, opacity, colour) in layers.into_iter().chain(centre) {
        let _ = write!(
            out,
            "<path d=\"{path}\" stroke=\"{}\" stroke-opacity=\"{}\" stroke-width=\"{}\" stroke-dasharray=\"0.01 {}\"{motion}/>",
            esc(&opaque(colour).to_css()),
            n(opacity * colour.a.clamp(0.0, 1.0)),
            n(p.size * width),
            n(gap)
        );
    }
    out.push_str("</g>");
    out
}

/// `a` moved towards `b` by `t` (0–1), alpha kept.
fn mix(a: Rgba, b: Rgba, t: f64) -> Rgba {
    Rgba {
        r: a.r + (b.r - a.r) * t,
        g: a.g + (b.g - a.g) * t,
        b: a.b + (b.b - a.b) * t,
        a: a.a,
    }
}

fn opaque(c: Rgba) -> Rgba {
    Rgba { a: 1.0, ..c }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn diagram(src: &str) -> Diagram {
        crate::diagram::from_source(src, &Default::default())
            .unwrap()
            .0
    }

    const TWO: &str = "diagram\n  card#a A\n    to(b).pulse\n  card#b B\n";

    #[test]
    fn nothing_moves_without_a_class() {
        assert!(period(&diagram("diagram\n  card#a A\n    to(b)\n  card#b B\n")).is_none());
    }

    #[test]
    fn a_pulse_loops_on_its_crossing_time() {
        let d = diagram(TWO);
        let line = pulse_line(&d, &d.model.edges[0].id).unwrap();
        let t = period(&d).unwrap();
        assert!((t - length(&line, 0) / PULSE_SPEED).abs() < 1e-9, "{t}");
    }

    #[test]
    fn flows_stretch_the_loop_to_whole_periods() {
        let d = diagram(
            "diagram\n  card#a A\n    to(b).pulse\n    to(c).flow\n  card#b B\n  card#c C\n",
        );
        let t = period(&d).unwrap();
        let k = t / (11.0 / FLOW_SPEED);
        assert!(
            (k - k.round()).abs() < 1e-9,
            "{t} is no whole number of flow periods"
        );
    }

    #[test]
    fn a_still_image_draws_no_dot() {
        let p = Pulse {
            size: 5.0,
            color: Rgba::BLACK,
            speed: 100.0,
        };
        let mut kf = Keyframes::new("sk");
        assert!(pulse_svg(&p, "M0,0 L100,0", 100.0, Clock::Still, 1.0, false, &mut kf).is_empty());
        assert!(kf.css().is_empty());
        let at = pulse_svg(
            &p,
            "M0,0 L100,0",
            100.0,
            Clock::At(0.5),
            1.0,
            false,
            &mut kf,
        );
        assert!(at.contains("stroke-dashoffset=\"-50\""), "{at}");
        let css = pulse_svg(&p, "M0,0 L100,0", 100.0, Clock::Css, 1.0, false, &mut kf);
        assert!(css.contains("animation:sk-shift-100 1s"), "{css}");
        assert!(kf.css().contains("@keyframes sk-shift-100"));
        assert!(kf.css().contains("prefers-reduced-motion"));
    }

    const BOTH: &str =
        "diagram\n  card#a A\n    to(b).pulse\n    to(c).flow\n  card#b B\n  card#c C\n";

    #[test]
    fn the_svg_is_still_unless_asked() {
        let d = diagram(BOTH);
        let still = crate::svg::render(&d);
        assert!(
            !still.contains("class=\"pulse\""),
            "a still SVG draws no dot"
        );
        assert!(
            !still.contains("stroke-dasharray"),
            "a still SVG draws the flow solid"
        );
        assert!(!still.contains("@keyframes"));
        let moving = crate::svg::render_with(&d, false, true);
        assert!(moving.contains("class=\"pulse\""));
        assert!(moving.contains("@keyframes") && moving.contains("prefers-reduced-motion"));
        assert!(
            moving.contains("stroke-dasharray=\"6 5\""),
            "the flow keeps its dashes"
        );
    }

    #[test]
    fn the_html_plays_both() {
        let html = crate::html::render(&diagram(BOTH), &crate::html::HtmlOptions::default());
        assert!(html.contains("class=\"pulse\"") && html.contains("-shift-11 1s linear"));
    }

    #[cfg(feature = "png")]
    #[test]
    fn frames_make_one_loop_without_steps() {
        let d = diagram(TWO);
        let o = crate::animated::GifOptions {
            fps: 10,
            ..Default::default()
        };
        let frames = crate::animated::frames(&d, &o);
        let want = (period(&d).unwrap() * 10.0).round().max(1.0) as usize;
        assert_eq!(frames.len(), want);
        assert!(frames[0].0.contains("stroke-dashoffset=\"0\""));
        let total: f64 = frames.iter().map(|f| f.1).sum();
        assert!((total - period(&d).unwrap()).abs() < 1e-9);
    }

    #[test]
    fn speeds_read_lengths_and_numbers() {
        let css = ".edge.fast { --pulse-speed: 300px; --pulse-size: 8px; }";
        let (d, _) = crate::diagram::from_source(
            "diagram\n  card#a A\n    to(b).pulse.fast\n  card#b B\n",
            &crate::Options {
                css: vec![css.to_string()],
                ..Default::default()
            },
        )
        .unwrap();
        let el = &d.scene.edges[&d.model.edges[0].id];
        let p = pulse(d.scene.style(el.line)).unwrap();
        assert_eq!((p.speed, p.size), (300.0, 8.0));
    }
}
