//! Timeline mode (E7, F-TL-1 to 8; FORMAT §3.6): position = date, lanes = zones.
//!
//! Ported from the prototype (`layoutTimeline`, `ticksFor`, `drawTimeline`).

use crate::layout::Box2;
use crate::measure::{Size, Sizes, card_size};
use crate::scene::{Extras, Scene};
use skemo_lang::dates::{civil_from_days, days_from_civil, parse_timeline, weekday};
use skemo_lang::{Kind, Model};
use std::collections::BTreeMap;

pub(crate) const MONTHS_FR: [&str; 12] = [
    "janv.", "févr.", "mars", "avr.", "mai", "juin", "juil.", "août", "sept.", "oct.", "nov.",
    "déc.",
];
pub(crate) const MONTHS_EN: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];
/// Height of the axis, stacking gaps and gaps between lanes (prototype).
pub const AXIS: f64 = 40.0;
const GAPX: f64 = 12.0;
const GAPY: f64 = 22.0;
const LANE_GAP: f64 = 26.0;

#[derive(Debug, Clone)]
pub struct Tick {
    pub x: f64,
    pub label: String,
    pub major: bool,
}

/// Time scale: bounds (days), density (px per day).
#[derive(Debug, Clone, Copy)]
pub struct Scale {
    pub d0: f64,
    pub d1: f64,
    pub ppd: f64,
}

impl Scale {
    pub fn x(&self, t: f64) -> f64 {
        (t - self.d0) * self.ppd
    }
}

/// Decorations of the timeline (file coordinates).
#[derive(Debug, Clone)]
pub struct Deco {
    pub ticks: Vec<Tick>,
    /// Diamonds of the axis: (id of the milestone, x).
    pub marks: Vec<(String, f64)>,
    pub today: Option<(f64, String)>,
    pub axis_h: f64,
    pub bottom: f64,
    pub width: f64,
}

fn ppd(scale: &str) -> f64 {
    match scale {
        "day" => 36.0,
        "week" => 9.0,
        "quarter" => 1.4,
        "year" => 0.45,
        _ => 4.0,
    }
}

/// (start, end) of a card in days; milestone: end = start.
pub fn span(n: &skemo_lang::Node) -> Option<(f64, f64)> {
    match (&n.date, &n.start) {
        (Some(d), _) => {
            let t = parse_timeline(d, false).ok()?;
            Some((t, t))
        }
        (None, Some(s)) => Some((
            parse_timeline(s, false).ok()?,
            parse_timeline(n.end.as_deref().unwrap_or(s), true).ok()?,
        )),
        _ => None,
    }
}

/// Scale of the diagram (default bounds: extreme dates ± 7 days).
pub fn scale(model: &Model) -> Scale {
    let all: Vec<f64> = model
        .nodes
        .iter()
        .filter(|n| n.kind == Kind::Card)
        .filter_map(span)
        .flat_map(|(a, b)| [a, b])
        .collect();
    let lo = all.iter().copied().fold(f64::INFINITY, f64::min);
    let hi = all.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    // `start` and `end` (`from`/`to` before v0.8, read as them by `lang`)
    let d0 = model
        .option("start")
        .and_then(|s| parse_timeline(s, false).ok())
        .unwrap_or(if lo.is_finite() { lo - 7.0 } else { 0.0 });
    let d1 = model
        .option("end")
        .and_then(|s| parse_timeline(s, true).ok())
        .unwrap_or(if hi.is_finite() { hi + 7.0 } else { d0 + 30.0 });
    let d1 = if d1 > d0 { d1 } else { d0 + 1.0 };
    let ppd = match model
        .option("width")
        .and_then(|w| w.trim().parse::<f64>().ok())
    {
        Some(w) if w > 0.0 => w / (d1 - d0),
        _ => ppd(model.option("scale").unwrap_or("month")),
    };
    Scale { d0, d1, ppd }
}

/// Mode classes and width of the bars, to lay before the measurement.
pub fn extras(model: &Model, sc: &Scale) -> Extras {
    let mut e = Extras::default();
    for n in model.nodes.iter().filter(|n| n.kind == Kind::Card) {
        if n.date.is_some() {
            e.classes.insert(n.id.clone(), vec!["milestone".into()]);
        } else if let Some((a, b)) = span(n) {
            e.classes.insert(n.id.clone(), vec!["phase".into()]);
            let bar = (sc.x(b) - sc.x(a)).max(6.0);
            e.inline
                .insert(n.id.clone(), vec![("--bar-w".into(), format!("{bar}px"))]);
        }
    }
    e
}

/// Graduations in the current language, as the prototype's `ticksFor` (French labels).
pub fn ticks(scale_name: &str, d0: f64, d1: f64) -> Vec<(f64, String, bool)> {
    ticks_in(skemo_lang::i18n::language(), scale_name, d0, d1)
}

/// Graduations in the language `lang`.
pub fn ticks_in(
    lang: skemo_lang::i18n::Lang,
    scale_name: &str,
    d0: f64,
    d1: f64,
) -> Vec<(f64, String, bool)> {
    let months = if lang == skemo_lang::i18n::Lang::Fr {
        &MONTHS_FR
    } else {
        &MONTHS_EN
    };
    let quarter = if lang == skemo_lang::i18n::Lang::Fr {
        "T"
    } else {
        "Q"
    };
    let mut out = Vec::new();
    if scale_name == "day" || scale_name == "week" {
        let mut t = d0.ceil() as i64;
        while (t as f64) <= d1 {
            let (_, m, d) = civil_from_days(t);
            if scale_name == "day" {
                out.push((t as f64, d.to_string(), d == 1));
            } else if weekday(t) == 0 {
                out.push((
                    t as f64,
                    format!("{d} {}", months[(m - 1) as usize]),
                    d <= 7,
                ));
            }
            t += 1;
        }
        return out;
    }
    let (mut y, m0, _) = civil_from_days(d0.floor() as i64);
    let step: u32 = match scale_name {
        "quarter" => 3,
        "year" => 12,
        _ => 1,
    };
    let mut m = if step > 1 {
        ((m0 - 1) / step) * step
    } else {
        m0 - 1
    }; // 0..11
    loop {
        let t = days_from_civil(y, m + 1, 1) as f64;
        if t > d1 {
            break;
        }
        if t >= d0 {
            let label = match scale_name {
                "quarter" => format!("{quarter}{} {y}", m / 3 + 1),
                "year" => y.to_string(),
                _ if m == 0 => format!("{} {y}", months[0]),
                _ => months[m as usize].to_string(),
            };
            out.push((t, label, m == 0));
        }
        m += step;
        if m >= 12 {
            m -= 12;
            y += 1;
        }
    }
    out
}

/// Stacking row: right edge, height, cards (id, x).
type Row = (f64, f64, Vec<(String, f64)>);

/// Couloir : zone de premier niveau (`None` : cartes hors zone) et ses cartes.
type Lane<'a> = (Option<String>, Vec<&'a skemo_lang::Node>);

fn card_in_zone(model: &Model, card: &skemo_lang::Node, zone: &skemo_lang::Node) -> bool {
    model.ancestors(&card.id).iter().any(|a| a.id == zone.id)
}

/// Placement of the timeline: sizes (periods with imposed width), boxes, decorations.
pub fn layout(
    model: &Model,
    scene: &Scene,
    sizes: &mut Sizes,
    sc: &Scale,
    pad: f64,
    head: f64,
) -> (BTreeMap<String, Box2>, Deco) {
    let cards: Vec<&skemo_lang::Node> = model
        .nodes
        .iter()
        .filter(|n| n.kind == Kind::Card && span(n).is_some())
        .collect();
    size_periods(scene, sizes, sc, &cards);
    let lanes = lanes(model, &cards);
    let mut boxes = BTreeMap::new();
    let w_axis = sc.x(sc.d1);
    let mut y = AXIS + 10.0;
    // D-048: a theme drawing periods as thin bars packs them closer (the prototype: 12 and 22)
    let stage = scene.style(scene.stage);
    let gap = |name: &str, default: f64| {
        stage
            .get(name)
            .and_then(|v| skemo_lang::style::parse_length(v.trim(), stage.font_size(), None))
            .unwrap_or(default)
    };
    let gaps = (gap("--tl-gap-x", GAPX), gap("--tl-gap-y", GAPY));
    for (lane, lcards) in &lanes {
        let spec = LaneSpec {
            id: lane.as_deref(),
            top: y,
            // the lane's description lines under its title (v0.8) take room too
            inner: if let Some(id) = lane.as_deref() {
                head + pad * 0.5 + sizes.zone_heads.get(id).copied().unwrap_or(0.0)
            } else {
                0.0
            },
            pad,
            w_axis,
            gaps,
        };
        y = place_lane(&spec, lcards, sc, sizes, &mut boxes) + LANE_GAP;
    }
    enclose_nested_zones(model, &cards, &mut boxes);
    let deco = decorations(model, &cards, sc, w_axis, y - LANE_GAP);
    (boxes, deco)
}

/// Sizes: period = max(bar, natural width), measured at that width (pitfalls P17, P18).
fn size_periods(scene: &Scene, sizes: &mut Sizes, sc: &Scale, cards: &[&skemo_lang::Node]) {
    for n in cards {
        let Some(el) = scene.cards.get(&n.id) else {
            continue;
        };
        if n.date.is_none() {
            let (a, b) = span(n).unwrap_or((0.0, 0.0));
            let bar = (sc.x(b) - sc.x(a)).max(6.0);
            let nat = card_size(scene, el, None).w;
            let w = bar.max(nat);
            let s = card_size(scene, el, Some(w));
            sizes.cards.insert(n.id.clone(), Size { w, h: s.h });
        }
    }
}

/// Lanes: top-level zones, in order; cards outside zones = implicit lane at the top.
fn lanes<'a>(model: &'a Model, cards: &[&'a skemo_lang::Node]) -> Vec<Lane<'a>> {
    let mut lanes: Vec<Lane> = Vec::new();
    let loose: Vec<&skemo_lang::Node> = cards
        .iter()
        .copied()
        .filter(|n| n.parent.is_none())
        .collect();
    if !loose.is_empty() {
        lanes.push((None, loose));
    }
    for z in model
        .nodes
        .iter()
        .filter(|n| n.kind == Kind::Zone && n.parent.is_none())
    {
        let inside = cards
            .iter()
            .copied()
            .filter(|c| card_in_zone(model, c, z))
            .collect();
        lanes.push((Some(z.id.clone()), inside));
    }
    lanes
}

/// Geometry of a lane: possible zone, top, inner margin, zone margin, axis width.
struct LaneSpec<'a> {
    id: Option<&'a str>,
    top: f64,
    inner: f64,
    pad: f64,
    w_axis: f64,
    /// Gaps between cards of a row and between rows (`--tl-gap-x`, `--tl-gap-y` of the theme).
    gaps: (f64, f64),
}

/// Stacks the cards of a lane (first free row), places their box and the zone's.
/// Returns the bottom of the lane.
fn place_lane(
    lane: &LaneSpec,
    lcards: &[&skemo_lang::Node],
    sc: &Scale,
    sizes: &Sizes,
    boxes: &mut BTreeMap<String, Box2>,
) -> f64 {
    let size = |id: &str| {
        sizes
            .cards
            .get(id)
            .copied()
            .unwrap_or(Size { w: 170.0, h: 48.0 })
    };
    let mut sorted = lcards.to_vec();
    sorted.sort_by(|a, b| {
        span(a)
            .map_or(0.0, |s| s.0)
            .total_cmp(&span(b).map_or(0.0, |s| s.0))
    });
    let mut rows: Vec<Row> = Vec::new();
    for c in sorted {
        let x = sc.x(span(c).map_or(0.0, |s| s.0));
        let s = size(&c.id);
        let k = rows
            .iter()
            .position(|r| r.0 + lane.gaps.0 <= x + crate::frise::TOUCH)
            .unwrap_or_else(|| {
                rows.push((f64::NEG_INFINITY, 0.0, Vec::new()));
                rows.len() - 1
            });
        rows[k].0 = x + s.w;
        rows[k].1 = rows[k].1.max(s.h);
        rows[k].2.push((c.id.clone(), x));
    }
    let mut ry = lane.top + lane.inner;
    for (_, h, rc) in &rows {
        for (id, x) in rc {
            let s = size(id);
            boxes.insert(
                id.clone(),
                Box2 {
                    x: *x,
                    y: ry,
                    w: s.w,
                    h: s.h,
                },
            );
        }
        ry += h + lane.gaps.1;
    }
    let bottom = ry - lane.gaps.1 + if lane.id.is_some() { lane.pad } else { 0.0 };
    if let Some(id) = lane.id {
        boxes.insert(
            id.to_string(),
            Box2 {
                x: -lane.pad,
                y: lane.top,
                w: lane.w_axis + 2.0 * lane.pad + 60.0,
                h: bottom - lane.top,
            },
        );
    }
    bottom
}

/// Nested zones: plain bounding box of their cards.
fn enclose_nested_zones(
    model: &Model,
    cards: &[&skemo_lang::Node],
    boxes: &mut BTreeMap<String, Box2>,
) {
    for z in model
        .nodes
        .iter()
        .filter(|n| n.kind == Kind::Zone && n.parent.is_some())
    {
        let bs: Vec<Box2> = cards
            .iter()
            .filter(|c| card_in_zone(model, c, z))
            .filter_map(|c| boxes.get(&c.id).copied())
            .collect();
        if bs.is_empty() {
            continue;
        }
        let x1 = bs.iter().map(|b| b.x).fold(f64::INFINITY, f64::min) - 8.0;
        let y1 = bs.iter().map(|b| b.y).fold(f64::INFINITY, f64::min) - 8.0;
        let x2 = bs
            .iter()
            .map(super::layout::Box2::right)
            .fold(f64::NEG_INFINITY, f64::max)
            + 8.0;
        let y2 = bs
            .iter()
            .map(super::layout::Box2::bottom)
            .fold(f64::NEG_INFINITY, f64::max)
            + 8.0;
        boxes.insert(
            z.id.clone(),
            Box2 {
                x: x1,
                y: y1,
                w: x2 - x1,
                h: y2 - y1,
            },
        );
    }
}

/// Axis: graduations, diamonds of the milestones, "today" line.
fn decorations(
    model: &Model,
    cards: &[&skemo_lang::Node],
    sc: &Scale,
    w_axis: f64,
    bottom: f64,
) -> Deco {
    let scale_name = model.option("scale").unwrap_or("month");
    let ticks = ticks_in(crate::label_lang(model), scale_name, sc.d0, sc.d1)
        .into_iter()
        .map(|(t, label, major)| Tick {
            x: sc.x(t),
            label,
            major,
        })
        .collect();
    let marks = cards
        .iter()
        .filter(|n| n.date.is_some())
        .filter_map(|n| Some((n.id.clone(), sc.x(span(n)?.0))))
        .collect();
    let today = model
        .option("today")
        .and_then(|t| parse_timeline(t, false).ok())
        .map(|t| (sc.x(t), today_label(crate::label_lang(model)).to_string()));
    Deco {
        ticks,
        marks,
        today,
        axis_h: AXIS,
        bottom,
        width: w_axis,
    }
}

/// Label of the `today` line in the diagram's language.
fn today_label(lang: skemo_lang::i18n::Lang) -> &'static str {
    if lang == skemo_lang::i18n::Lang::Fr {
        "aujourd’hui"
    } else {
        "today"
    }
}

/// Box of the axis, for the framing only.
pub fn axis_box(deco: &Deco, pad: f64) -> Box2 {
    Box2 {
        x: -pad,
        y: 0.0,
        w: deco.width + 2.0 * pad + 60.0,
        h: 1.0,
    }
}

/// How far before the end of its bar a dependency leaves when the target's bar starts right
/// there (px).
const LEAD: f64 = 12.0;

/// Dependencies drawn like a hand-made roadmap (D-048, T3), when the theme asks for it
/// (`--tl-deps: drop`): from the end of the source's bar (or its diamond) straight down or up to
/// the level of the target, then into the target's start. No detour around lanes: the lines are
/// thin and dashed, and cross what lies between. Arrows with `side=` or `via=` keep the router.
pub fn dependency_routes(
    model: &Model,
    scene: &Scene,
    boxes: &BTreeMap<String, Box2>,
) -> BTreeMap<String, Vec<(f64, f64)>> {
    let mut out = BTreeMap::new();
    let stage = scene.style(scene.stage);
    if stage.get("--tl-deps").map(str::trim) != Some("drop") {
        return out;
    }
    for e in model
        .edges
        .iter()
        .filter(|e| e.side.is_none() && e.via.is_none())
    {
        let (Some(sb), Some(tb)) = (boxes.get(&e.from), boxes.get(&e.to)) else {
            continue;
        };
        let (Some(sn), Some(tn)) = (model.node(&e.from), model.node(&e.to)) else {
            continue;
        };
        let bar = |n: &skemo_lang::Node, b: &Box2| {
            scene
                .cards
                .get(&n.id)
                .and_then(|c| scene.style(c.el).get("--bar-w"))
                .and_then(|v| v.trim().trim_end_matches("px").parse::<f64>().ok())
                .map(|w| (w, b.bottom() - 4.0))
        };
        // start: the end of the bar, or the diamond of a milestone
        let mut s = match bar(sn, sb) {
            Some((w, y)) => (sb.x + w, y),
            None => (sb.x + 7.0, sb.y + 13.0),
        };
        // end: the start of the target's bar, or its diamond, entered from the side facing the start
        let (tx, ty) = match bar(tn, tb) {
            Some((_, y)) => (tb.x, y),
            None => (tb.x + 7.0, tb.y + 13.0),
        };
        // a bar below that starts where the source's ends: the line would come down on the
        // first letter of its title; it leaves a little before the end and comes in from the left
        if (tx - s.0).abs() < 2.0
            && ty > s.1
            && bar(tn, tb).is_some()
            && bar(sn, sb).is_some_and(|(w, _)| w >= 2.0 * LEAD)
        {
            s.0 -= LEAD;
        }
        let t = if (tx - s.0).abs() < 2.0 {
            (s.0, if ty > s.1 { ty - 8.0 } else { ty + 8.0 })
        } else if tx > s.0 {
            (tx - if bar(tn, tb).is_some() { 0.0 } else { 8.0 }, ty)
        } else {
            (tx + 8.0, ty)
        };
        let pts = if (t.0 - s.0).abs() < 2.0 || (t.1 - s.1).abs() < 2.0 {
            vec![s, t]
        } else {
            vec![s, (s.0, t.1), t]
        };
        out.insert(e.id.clone(), pts);
    }
    out
}
