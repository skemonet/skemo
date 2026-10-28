//! Frieze mode (E8, F-FR-1 to 6, 8, 9; FORMAT §3.7): a central axis, eras as bands under it, events
//! above or below linked to their date by a stem.
//!
//! Ported from the prototype (`layoutFrise`, `drawFrise`).

use crate::layout::Box2;
use crate::measure::{Size, Sizes, tw};
use crate::scene::{Extras, Scene};
use skemo_lang::dates::{parse_frise, to_astronomical, to_historical};
use skemo_lang::i18n::Lang;
use skemo_lang::{Kind, Model};
use std::collections::BTreeMap;

/// Minimum height of an era band; more if the era titles are larger (`--font-scale`).
const BAND_H: f64 = 28.0;
const BAND_GAP: f64 = 4.0;
/// Minimum room for the year labels under the axis; more if they are larger.
const LABEL_H: f64 = 22.0;
const STEM: f64 = 30.0;
const GAPX: f64 = 14.0;
const GAPY: f64 = 14.0;
/// Top of the year labels, under the axis (same offset in the SVG and HTML renderers).
pub const TICK_LABEL_TOP: f64 = 5.0;
/// Distance from a card's edge to its stem when the card hangs by that edge.
const ANCHOR: f64 = 14.0;

#[derive(Debug, Clone, Copy)]
pub struct Scale {
    pub a0: f64,
    pub a1: f64,
    pub ppy: f64,
}

impl Scale {
    pub fn x(&self, t: f64) -> f64 {
        (t - self.a0) * self.ppy
    }
}

#[derive(Debug, Clone)]
pub struct Stem {
    pub id: String,
    pub x: f64,
    pub y1: f64,
    pub y2: f64,
}

#[derive(Debug, Clone)]
pub struct Deco {
    /// Graduations: (x, label).
    pub ticks: Vec<(f64, String)>,
    pub centuries: Vec<(f64, String)>,
    pub stems: Vec<Stem>,
    pub width: f64,
    pub band_bottom: f64,
}

fn ppy(scale: &str) -> f64 {
    match scale {
        "year" => 60.0,
        "decade" => 8.0,
        "millennium" => 0.12,
        _ => 0.9,
    }
}

/// (start, end) of a card; `end` is included at the precision written (v0.8: it was a bound
/// here), an event starts and ends on its date.
/// Two bands or bars that overlap by less than this, as drawn, touch and share a row (v0.8).
pub const TOUCH: f64 = 2.0;

pub fn span(n: &skemo_lang::Node) -> Option<(f64, f64)> {
    match (&n.date, &n.start) {
        (Some(d), _) => {
            let t = parse_frise(d, false).ok()?;
            Some((t, t))
        }
        (None, Some(s)) => {
            let a = parse_frise(s, false).ok()?;
            let b = match &n.end {
                Some(e) => parse_frise(e, true).ok()?,
                None => parse_frise(s, true).ok()?,
            };
            Some((a, b))
        }
        _ => None,
    }
}

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
    // the axis bounds `start` and `end` (`from`/`to` before v0.8, read as them by `lang`); `end`
    // is included like any end: `end="2018"` runs to the end of 2018
    let a0 = model
        .option("start")
        .and_then(|s| parse_frise(s, false).ok())
        .unwrap_or(if lo.is_finite() { lo } else { 0.0 });
    let a1 = model
        .option("end")
        .and_then(|s| parse_frise(s, true).ok())
        .unwrap_or(if hi.is_finite() { hi } else { a0 + 100.0 });
    let a1 = if a1 > a0 { a1 } else { a0 + 1.0 };
    let ppy = match model
        .option("width")
        .and_then(|w| w.trim().parse::<f64>().ok())
    {
        Some(w) if w > 0.0 => w / (a1 - a0),
        _ => ppy(model.option("scale").unwrap_or("century")),
    };
    Scale { a0, a1, ppy }
}

fn eras(model: &Model) -> Vec<&skemo_lang::Node> {
    model
        .nodes
        .iter()
        .filter(|n| n.kind == Kind::Card && n.date.is_none() && span(n).is_some())
        .collect()
}

/// Mode classes (`era`, `event`) and, for eras, an imposed width.
pub fn extras(model: &Model, sc: &Scale, outside: &[String]) -> Extras {
    let mut e = Extras::default();
    for n in model.nodes.iter().filter(|n| n.kind == Kind::Card) {
        let Some((a, b)) = span(n) else { continue };
        if let Some(raw) = &n.date {
            e.classes.insert(n.id.clone(), vec!["event".into()]);
            e.inline.insert(
                n.id.clone(),
                vec![("--date".into(), date_label(crate::label_lang(model), raw))],
            );
        } else {
            let mut c = vec!["era".to_string()];
            if outside.contains(&n.id) {
                c.push("outside".into());
            }
            e.classes.insert(n.id.clone(), c);
            let w = (sc.x(b) - sc.x(a)).max(8.0);
            e.inline
                .insert(n.id.clone(), vec![("--frise-w".into(), format!("{w}px"))]);
        }
    }
    e
}

/// Eras whose name does not fit in the band (class `outside`).
pub fn outside(model: &Model, scene: &Scene, sc: &Scale) -> Vec<String> {
    let mut out = Vec::new();
    for n in eras(model) {
        let Some(el) = scene.cards.get(&n.id) else {
            continue;
        };
        let (a, b) = span(n).unwrap_or((0.0, 0.0));
        let w = (sc.x(b) - sc.x(a)).max(8.0);
        let need = tw(scene.style(el.title), &n.title).ceil() + 20.0;
        if need > w {
            out.push(n.id.clone());
        }
    }
    out
}

fn roman(mut n: i64) -> String {
    let v = [
        (1000, "M"),
        (900, "CM"),
        (500, "D"),
        (400, "CD"),
        (100, "C"),
        (90, "XC"),
        (50, "L"),
        (40, "XL"),
        (10, "X"),
        (9, "IX"),
        (5, "V"),
        (4, "IV"),
        (1, "I"),
    ];
    let mut r = String::new();
    for (k, l) in v {
        while n >= k {
            r.push_str(l);
            n -= k;
        }
    }
    r
}

/// Date of an event as written on its card: `-44-03-15` → `15 Mar 44 BC`, `476` → `476`.
pub fn date_label(lang: Lang, raw: &str) -> String {
    let (neg, body) = raw
        .trim()
        .strip_prefix('-')
        .map_or((false, raw.trim()), |b| (true, b));
    let mut parts = body.split('-');
    let Some(y) = parts.next().and_then(|y| y.parse::<i64>().ok()) else {
        return raw.to_string();
    };
    let year = year_label(lang, if neg { -y } else { y });
    let months = if lang == Lang::Fr {
        &crate::timeline::MONTHS_FR
    } else {
        &crate::timeline::MONTHS_EN
    };
    let month = parts
        .next()
        .and_then(|m| m.parse::<usize>().ok())
        .filter(|m| (1..=12).contains(m))
        .map(|m| months[m - 1]);
    let day = parts.next().and_then(|d| d.parse::<u32>().ok());
    match (day, month) {
        (Some(d), Some(m)) => format!("{d} {m} {year}"),
        (None, Some(m)) => format!("{m} {year}"),
        _ => year,
    }
}

fn year_label(lang: Lang, y: i64) -> String {
    if y < 0 {
        if lang == Lang::Fr {
            format!("{} av. J.-C.", -y)
        } else {
            format!("{} BC", -y)
        }
    } else {
        y.to_string()
    }
}

/// Placement of the frieze.
pub fn layout(
    model: &Model,
    scene: &Scene,
    sizes: &mut Sizes,
    (sc, outside): (&Scale, &[String]),
    (pad, head): (f64, f64),
) -> (BTreeMap<String, Box2>, Deco) {
    let w_axis = sc.x(sc.a1);
    let mut boxes = BTreeMap::new();
    let m = metrics(model, scene);
    let rows = place_eras(model, scene, sizes, (sc, m), outside, &mut boxes);
    // Without eras, no empty band row: events below the axis sit under the year labels.
    let band_bottom = if rows == 0 {
        m.label_h
    } else {
        m.label_h + (rows as f64) * (m.band_h + BAND_GAP) - BAND_GAP
    };
    let zones = zone_sides(model, sizes, sc, (pad, head));
    // the theme may let event cards hang by an edge of their stem to pack (`--event-anchor: edge`)
    let anchor = scene
        .style(scene.stage)
        .get("--event-anchor")
        .is_some_and(|v| v.trim() == "edge");
    let mut stems = place_events(
        model,
        sizes,
        (sc, &zones, anchor),
        (band_bottom, pad),
        &mut boxes,
    );
    zone_frames(model, &zones, (band_bottom, pad), &mut boxes);
    clear_zone_titles(model, scene, &zones, &stems, &mut boxes);
    let ticks = graduations(model, sc);
    clear_tick_labels(scene, &ticks, &mut stems);
    let centuries = century_labels(model, scene, sc, &stems);
    let deco = Deco {
        ticks,
        centuries,
        stems,
        width: w_axis,
        band_bottom,
    };
    (boxes, deco)
}

/// Heights that follow the text size: year labels, era bands.
#[derive(Clone, Copy)]
struct Metrics {
    label_h: f64,
    band_h: f64,
}

fn metrics(model: &Model, scene: &Scene) -> Metrics {
    let lh = |el: usize| crate::measure::line_height(scene.style(el));
    let label_h = scene
        .deco
        .get("fr-tick-label")
        .map_or(LABEL_H, |&el| (TICK_LABEL_TOP + lh(el) + 2.0).max(LABEL_H));
    let band_h = eras(model)
        .iter()
        .filter_map(|n| scene.cards.get(&n.id))
        .map(|c| lh(c.title) + 11.0)
        .fold(BAND_H, f64::max);
    Metrics { label_h, band_h }
}

/// Eras: the longest first; two bands that touch share a row (bands that overlap by less than 2 px,
/// as drawn, touch: Republic → -27, Empire -27 →, v0.8). Returns the number of rows.
fn place_eras(
    model: &Model,
    scene: &Scene,
    sizes: &mut Sizes,
    (sc, m): (&Scale, Metrics),
    outside: &[String],
    boxes: &mut BTreeMap<String, Box2>,
) -> usize {
    let mut list = eras(model);
    let dur = |n: &skemo_lang::Node| span(n).map_or(0.0, |(a, b)| b - a);
    list.sort_by(|p, q| {
        dur(q).total_cmp(&dur(p)).then(
            span(p)
                .map_or(0.0, |s| s.0)
                .total_cmp(&span(q).map_or(0.0, |s| s.0)),
        )
    });
    let mut rows: Vec<Vec<(f64, f64)>> = Vec::new();
    for n in list {
        let (s0, s1) = span(n).unwrap_or((0.0, 0.0));
        let x = sc.x(s0);
        let w = (sc.x(s1) - sc.x(s0)).max(8.0);
        let mut occupied = w;
        if outside.contains(&n.id) {
            let need = scene
                .cards
                .get(&n.id)
                .map_or(0.0, |el| tw(scene.style(el.title), &n.title).ceil() + 20.0);
            occupied = w + 6.0 + need;
        }
        let (a, b) = (x, x + occupied);
        let r = rows
            .iter()
            .position(|row| {
                row.iter()
                    .all(|&(p0, p1)| b <= p0 + TOUCH || a >= p1 - TOUCH)
            })
            .unwrap_or_else(|| {
                rows.push(Vec::new());
                rows.len() - 1
            });
        rows[r].push((a, b));
        boxes.insert(
            n.id.clone(),
            Box2 {
                x,
                y: m.label_h + r as f64 * (m.band_h + BAND_GAP),
                w,
                h: m.band_h,
            },
        );
        sizes.cards.insert(n.id.clone(), Size { w, h: m.band_h });
    }
    rows.len()
}

/// Level of events on one side of the axis: intervals taken and height of the level.
type Level = (Vec<(f64, f64)>, f64);

/// First level where `[x1, x2]` fits (at `GAPX` from its neighbours), or a new level.
fn first_free_level(levels: &[Level], x1: f64, x2: f64) -> usize {
    levels
        .iter()
        .position(|(items, _)| items.iter().all(|&(a, b)| x2 + GAPX <= a || x1 >= b + GAPX))
        .unwrap_or(levels.len())
}

/// Starting y of each level, moving away from the axis.
fn level_offsets(levels: &[Level]) -> Vec<f64> {
    let mut acc = 0.0;
    levels
        .iter()
        .map(|(_, h)| {
            let y = acc;
            acc += h + GAPY;
            y
        })
        .collect()
}

/// A top-level zone of the frieze (D-045): the span of its events across the axis (with its
/// padding) and the side they all go to.
struct FZone {
    id: String,
    x1: f64,
    x2: f64,
    up: bool,
    /// Room of its title and of its description lines (v0.8)
    head: f64,
}

fn event_extent(n: &skemo_lang::Node, sizes: &Sizes, sc: &Scale) -> (f64, f64, Size) {
    let s = sizes
        .cards
        .get(&n.id)
        .copied()
        .unwrap_or(Size { w: 150.0, h: 42.0 });
    let cx = sc.x(span(n).map_or(0.0, |s| s.0));
    (cx - s.w / 2.0, cx + s.w / 2.0, s)
}

fn is_event(n: &skemo_lang::Node) -> bool {
    n.kind == Kind::Card && n.date.is_some() && span(n).is_some()
}

/// Zones and their side: above the axis, unless a zone already above overlaps its dates.
fn zone_sides(model: &Model, sizes: &Sizes, sc: &Scale, (pad, head): (f64, f64)) -> Vec<FZone> {
    let mut out: Vec<FZone> = Vec::new();
    for z in model.children(None).filter(|n| n.kind == Kind::Zone) {
        let (mut x1, mut x2) = (f64::INFINITY, f64::NEG_INFINITY);
        for n in model.children(Some(&z.id)).filter(|n| is_event(n)) {
            let (a, b, _) = event_extent(n, sizes, sc);
            (x1, x2) = (x1.min(a - pad), x2.max(b + pad));
        }
        if !x1.is_finite() {
            continue;
        }
        let clash = |up: bool| {
            out.iter()
                .any(|o| o.up == up && x1 < o.x2 + GAPX && o.x1 < x2 + GAPX)
        };
        let up = !clash(true) || clash(false);
        out.push(FZone {
            id: z.id.clone(),
            x1,
            x2,
            up,
            head: head + sizes.zone_heads.get(&z.id).copied().unwrap_or(0.0),
        });
    }
    out
}

/// Side imposed on an event by the zones: its zone's side; outside any zone, the side opposite
/// a zone whose dates it falls in (if only one side has such zones).
fn forced_side(n: &skemo_lang::Node, zones: &[FZone], (x1, x2): (f64, f64)) -> Option<bool> {
    if let Some(z) = zones
        .iter()
        .find(|z| n.parent.as_deref() == Some(z.id.as_str()))
    {
        return Some(z.up);
    }
    let hit = |up: bool| {
        zones
            .iter()
            .any(|z| z.up == up && x1 < z.x2 + GAPX && z.x1 < x2 + GAPX)
    };
    match (hit(true), hit(false)) {
        (true, false) => Some(false),
        (false, true) => Some(true),
        _ => None,
    }
}

/// Frames of the zones: around their cards and stems, from the axis (above) or the bands (below),
/// with the title on top.
fn zone_frames(
    model: &Model,
    zones: &[FZone],
    (band_bottom, pad): (f64, f64),
    boxes: &mut BTreeMap<String, Box2>,
) {
    for z in zones {
        let kids: Vec<Box2> = model
            .children(Some(&z.id))
            .filter(|n| is_event(n))
            .filter_map(|n| boxes.get(&n.id).copied())
            .collect();
        let x1 = kids.iter().map(|b| b.x).fold(f64::INFINITY, f64::min) - pad;
        let x2 = kids
            .iter()
            .map(Box2::right)
            .fold(f64::NEG_INFINITY, f64::max)
            + pad;
        let top = kids.iter().map(|b| b.y).fold(f64::INFINITY, f64::min) - z.head - pad * 0.6;
        let bottom = kids
            .iter()
            .map(Box2::bottom)
            .fold(f64::NEG_INFINITY, f64::max)
            + pad;
        let (y1, y2) = if z.up {
            (top, -4.0)
        } else {
            (top.max(band_bottom + 4.0), bottom)
        };
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

/// Events: on the side that asks for the lowest level, alternating on a tie, unless a zone imposes
/// a side (`forced_side`). The cards of a zone also keep its margin; below the axis, they leave
/// room for its title. Returns the stems.
fn place_events(
    model: &Model,
    sizes: &Sizes,
    (sc, zones, anchor): (&Scale, &[FZone], bool),
    (band_bottom, pad): (f64, f64),
    boxes: &mut BTreeMap<String, Box2>,
) -> Vec<Stem> {
    let mut events: Vec<&skemo_lang::Node> = model.nodes.iter().filter(|n| is_event(n)).collect();
    events.sort_by(|p, q| {
        span(p)
            .map_or(0.0, |s| s.0)
            .total_cmp(&span(q).map_or(0.0, |s| s.0))
    });
    let (mut up, mut down): (Vec<Level>, Vec<Level>) = (Vec::new(), Vec::new());
    let mut place = Vec::new();
    let mut last_up = false;
    for n in events {
        let (cx1, cx2, s) = event_extent(n, sizes, sc);
        let cx = (cx1 + cx2) / 2.0;
        let zone = zones
            .iter()
            .find(|z| n.parent.as_deref() == Some(z.id.as_str()));
        let zoned = zone.is_some();
        let margin = if zoned { pad } else { 0.0 };
        // where the card may sit on its stem: centred, or (`anchor`) hung by an edge
        let offsets: &[f64] = if anchor {
            &[0.0, s.w / 2.0 - ANCHOR, ANCHOR - s.w / 2.0]
        } else {
            &[0.0]
        };
        let best = |levels: &[Level]| {
            offsets
                .iter()
                .map(|&o| {
                    let x1 = cx1 + o;
                    (first_free_level(levels, x1 - margin, x1 + s.w + margin), x1)
                })
                .min_by(|a, b| a.0.cmp(&b.0))
                .unwrap_or((0, cx1))
        };
        let ((ku, xu), (kd, xd)) = (best(&up), best(&down));
        let is_up = forced_side(n, zones, (cx1, cx2)).unwrap_or(match ku.cmp(&kd) {
            std::cmp::Ordering::Less => true,
            std::cmp::Ordering::Greater => false,
            std::cmp::Ordering::Equal => !last_up,
        });
        let x1 = if is_up { xu } else { xd };
        let (r1, r2) = (x1 - margin, x1 + s.w + margin);
        let k = if is_up { ku } else { kd };
        let lv = if is_up { &mut up } else { &mut down };
        if lv.len() <= k {
            lv.push((Vec::new(), 0.0));
        }
        lv[k].0.push((r1, r2));
        lv[k].1 = lv[k].1.max(s.h);
        let drop = match zone {
            Some(z) if !is_up => z.head,
            _ => 0.0,
        };
        place.push((n.id.clone(), is_up, k, x1, s, cx, drop));
        last_up = is_up;
    }
    let (ou, od) = (level_offsets(&up), level_offsets(&down));
    let mut stems = Vec::new();
    for (id, is_up, k, x, s, cx, drop) in place {
        if is_up {
            let lh = up[k].1;
            let y = -STEM - ou[k] - lh;
            boxes.insert(
                id.clone(),
                Box2 {
                    x,
                    y: y + (lh - s.h),
                    w: s.w,
                    h: s.h,
                },
            );
            stems.push(Stem {
                id,
                x: cx,
                y1: y + lh,
                y2: 0.0,
            });
        } else {
            let y = band_bottom + STEM + od[k] + drop;
            boxes.insert(
                id.clone(),
                Box2 {
                    x,
                    y,
                    w: s.w,
                    h: s.h,
                },
            );
            // The stem starts on the axis, like above it; bands are drawn over it.
            stems.push(Stem {
                id,
                x: cx,
                y1: 0.0,
                y2: y,
            });
        }
    }
    stems
}

/// Gap kept between a stem and the end of a zone's title.
const TITLE_GAP: f64 = 8.0;

/// Below the axis a zone's title sits between the axis and its events: a stem of one of its events
/// that would run through the title moves the zone's left edge (and the title with it) far enough
/// left for the title to end before the stem, unless that would cover another zone.
fn clear_zone_titles(
    model: &Model,
    scene: &Scene,
    zones: &[FZone],
    stems: &[Stem],
    boxes: &mut BTreeMap<String, Box2>,
) {
    for z in zones.iter().filter(|z| !z.up) {
        let (Some(el), Some(f)) = (scene.zones.get(&z.id), boxes.get(&z.id).copied()) else {
            continue;
        };
        let c = scene.style(el.title);
        let left = c.length("left").unwrap_or(16.0);
        // the title and the description lines under it
        let width = el
            .subs
            .iter()
            .map(|(sub, text)| tw(scene.style(*sub), text))
            .fold(
                tw(
                    c,
                    &model
                        .node(&z.id)
                        .map(|n| n.title.clone())
                        .unwrap_or_default(),
                ),
                f64::max,
            );
        let (t1, t2) = (f.x + left, f.x + left + width);
        let crossing = stems
            .iter()
            .filter(|st| {
                model.node(&st.id).and_then(|n| n.parent.as_deref()) == Some(z.id.as_str())
            })
            .map(|st| st.x)
            .filter(|&x| x > t1 - TITLE_GAP && x < t2 + TITLE_GAP)
            .fold(f64::INFINITY, f64::min);
        if !crossing.is_finite() {
            continue;
        }
        let x = crossing - TITLE_GAP - width - left;
        let wide = Box2 {
            x,
            w: f.right() - x,
            ..f
        };
        let covers = zones.iter().filter(|o| o.id != z.id).any(|o| {
            boxes.get(&o.id).is_some_and(|b| {
                b.x < wide.right()
                    && wide.x < b.right()
                    && b.y < wide.bottom()
                    && wide.y < b.bottom()
            })
        });
        if !covers {
            boxes.insert(z.id.clone(), wide);
        }
    }
}

/// A stem below the axis that would cross a year label starts under that label instead.
fn clear_tick_labels(scene: &Scene, ticks: &[(f64, String)], stems: &mut [Stem]) {
    let Some(&el) = scene.deco.get("fr-tick-label") else {
        return;
    };
    let c = scene.style(el);
    let bottom = TICK_LABEL_TOP + crate::measure::line_height(c) + 1.0;
    for st in stems.iter_mut().filter(|s| s.y1 >= 0.0) {
        let hit = ticks
            .iter()
            .any(|(x, label)| (st.x - x).abs() < tw(c, label) / 2.0 + 2.0);
        if hit {
            st.y1 = bottom;
        }
    }
}

/// Graduations: at least 90 px between two labels.
fn graduations(model: &Model, sc: &Scale) -> Vec<(f64, String)> {
    let unit = match model.option("scale").unwrap_or("century") {
        "year" => 1.0,
        "decade" => 10.0,
        "millennium" => 1000.0,
        _ => 100.0,
    };
    let stride = [1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0]
        .iter()
        .map(|k| k * unit)
        .find(|st| st * sc.ppy >= 90.0)
        .unwrap_or(unit * 100.0);
    let mut ticks = Vec::new();
    let h0 = to_historical(sc.a0.ceil() as i64);
    let h1 = to_historical(sc.a1.floor() as i64);
    let st = stride as i64;
    let mut y = (h0 as f64 / stride).ceil() as i64 * st;
    while y <= h1 {
        let yy = if y == 0 { 1 } else { y };
        ticks.push((
            sc.x(to_astronomical(yy) as f64),
            year_label(crate::label_lang(model), yy),
        ));
        y += st;
    }
    ticks
}

/// Label of a century (`c`: astronomical century number, 0 = year 1 to 100).
fn century_label(lang: Lang, c: i64) -> String {
    let n = if c >= 0 { c + 1 } else { -c };
    if lang == Lang::Fr {
        format!(
            "{}{} s.{}",
            roman(n),
            if n == 1 { "ᵉʳ" } else { "ᵉ" },
            if c < 0 { " av." } else { "" }
        )
    } else {
        let suffix = match (n % 100, n % 10) {
            (11..=13, _) => "th",
            (_, 1) => "st",
            (_, 2) => "nd",
            (_, 3) => "rd",
            _ => "th",
        };
        format!("{n}{suffix} c.{}", if c < 0 { " BC" } else { "" })
    }
}

/// Centuries ("century" scale, wide enough): label centred on the century.
fn century_labels(model: &Model, scene: &Scene, sc: &Scale, stems: &[Stem]) -> Vec<(f64, String)> {
    let w_axis = sc.x(sc.a1);
    let mut centuries = Vec::new();
    if model.option("scale").unwrap_or("century") != "century" || 100.0 * sc.ppy < 70.0 {
        return centuries;
    }
    // a theme may keep a single row of labels (`.fr-century { display: none }`)
    if scene
        .deco
        .get("fr-century")
        .is_some_and(|&el| scene.style(el).display() == "none")
    {
        return centuries;
    }
    let mut prev_right: Option<f64> = None;
    let hist = |a: f64| if a <= 0.0 { a - 1.0 } else { a };
    let c0 = (hist(sc.a0) / 100.0).floor() as i64;
    let c1 = (hist(sc.a1) / 100.0).ceil() as i64;
    for c in c0..=c1 {
        let (start, end) = if c >= 0 {
            (100 * c + 1, 100 * (c + 1))
        } else {
            (100 * c, 100 * c + 99)
        };
        let xa = sc.x(to_astronomical(start) as f64);
        let xb = sc.x((to_astronomical(end) + 1) as f64);
        if xb < 0.0 || xa > w_axis {
            continue;
        }
        let label = century_label(crate::label_lang(model), c);
        let mut cx = (xa.max(0.0) + xb.min(w_axis)) / 2.0;
        if let Some(&el) = scene.deco.get("fr-century") {
            let half = tw(scene.style(el), &label) / 2.0 + 4.0;
            cx = avoid_stems(cx, half, stems, prev_right, w_axis);
            prev_right = Some(cx + tw(scene.style(el), &label) / 2.0);
        }
        centuries.push((cx, label));
    }
    centuries
}

/// F-FR-8: keep the label (half-width `half`) from being crossed by the stem of an event above.
/// The label may run past its century, without leaving the axis or touching the previous one.
fn avoid_stems(cx: f64, half: f64, stems: &[Stem], prev_right: Option<f64>, w_axis: f64) -> f64 {
    let up_stems: Vec<f64> = stems.iter().filter(|s| s.y1 < 0.0).map(|s| s.x).collect();
    let hits = |c: f64| up_stems.iter().any(|&x| x > c - half && x < c + half);
    if !hits(cx) {
        return cx;
    }
    let left_limit = 0.0_f64.max(prev_right.unwrap_or(f64::NEG_INFINITY));
    let mut best: Option<f64> = None;
    for &x in &up_stems {
        for cand in [x + half + 0.5, x - half - 0.5] {
            if cand - half >= left_limit
                && cand + half <= w_axis
                && !hits(cand)
                && best.is_none_or(|b| (cand - cx).abs() < (b - cx).abs())
            {
                best = Some(cand);
            }
        }
    }
    best.unwrap_or(cx)
}

/// Box of the axis, for the framing only.
pub fn axis_box(d: &Deco) -> Box2 {
    Box2 {
        x: -10.0,
        y: 0.0,
        w: d.width + 20.0,
        h: d.band_bottom,
    }
}
