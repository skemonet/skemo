//! Contrast (`lang`'s F-STYLE-15, E-STYLE-7): each text of the scene against its real background
//! (semi-transparent backgrounds composed over what lies under them).
//!
//! Thresholds: text on a surface ≥ 4.5:1; text on an accent fill (badges, bands) ≥ 3:1 in light
//! and dark, ≥ 4.5:1 in `print`. The WCAG ratio depends on luminance only: it is therefore the same
//! after conversion to greyscale, which covers the "black and white" test.

use crate::diagram::Diagram;
use crate::scene::Scene;
use serde::Serialize;
use skemo_lang::style::{Computed, Rgba, Theme, parse_color};
use skemo_lang::tr;
use skemo_lang::{Model, Node};

#[derive(Debug, Clone, Serialize)]
pub struct Issue {
    /// Element (`fw`, `fw/badge`, `lan/title`, `fw->core/label`…).
    pub element: String,
    /// Classes of the element (the faulty types of a custom CSS).
    pub classes: Vec<String>,
    pub what: &'static str,
    pub ratio: f64,
    pub required: f64,
    pub text: String,
    pub background: String,
}

fn bg(c: &Computed) -> Option<Rgba> {
    c.color_of("background-color")
        .or_else(|| c.get("background").and_then(|b| parse_color(b, c.color())))
}

/// Effective background: the element's colour composed over the background below.
fn over(top: Option<Rgba>, under: Rgba) -> Rgba {
    match top {
        Some(c) if c.a > 0.0 => c.over(under),
        _ => under,
    }
}

/// A text (the style element `style`) against its effective background.
struct Pair {
    element: String,
    classes: Vec<String>,
    what: &'static str,
    style: usize,
    back: Rgba,
    required: f64,
}

impl Pair {
    /// Icons are graphics, not text: their ink is `--icon-color`.
    fn is_icon(&self) -> bool {
        self.what == "icon"
    }

    fn ink(&self, sc: &Scene) -> Rgba {
        let c = sc.style(self.style);
        if self.is_icon() {
            c.var_color("--icon-color").unwrap_or_else(|| c.color())
        } else {
            c.color()
        }
    }
}

/// WCAG 1.4.11 threshold of graphical elements (icons).
const GRAPHIC_MIN: f64 = 3.0;

/// Checks every text/background pair of the diagram.
pub fn check(d: &Diagram) -> Vec<Issue> {
    let mut out = Vec::new();
    for p in pairs(&d.model, &d.scene, d.theme) {
        let t = p.ink(&d.scene).over(p.back);
        let r = t.contrast(p.back);
        if r + 1e-9 < p.required {
            out.push(Issue {
                element: p.element,
                classes: p.classes,
                what: p.what,
                ratio: (r * 100.0).round() / 100.0,
                required: p.required,
                text: t.to_css(),
                background: p.back.to_css(),
            });
        }
    }
    out
}

/// Enforced contrast: each text under its threshold is darkened or lightened (on the side it
/// already is relative to its background, else on the other) up to the threshold. Changes colours
/// only: neither measure nor placement.
pub fn enforce(model: &Model, scene: &mut Scene, theme: Theme) {
    for p in pairs(model, scene, theme) {
        let t = p.ink(scene).over(p.back);
        if t.contrast(p.back) + 1e-9 >= p.required {
            continue;
        }
        let lighter = t.luminance() > p.back.luminance();
        let at = |toward: f64, f: f64| {
            let m = |v: f64| (v + (toward - v) * f).round().clamp(0.0, 255.0);
            Rgba {
                r: m(t.r),
                g: m(t.g),
                b: m(t.b),
                a: 1.0,
            }
        };
        // smallest mix towards `toward` that reaches the threshold (binary search)
        let solve = |toward: f64| -> Option<Rgba> {
            if at(toward, 1.0).contrast(p.back) + 1e-9 < p.required {
                return None;
            }
            let (mut lo, mut hi) = (0.0, 1.0);
            for _ in 0..20 {
                let mid = (lo + hi) / 2.0;
                if at(toward, mid).contrast(p.back) + 1e-9 >= p.required {
                    hi = mid;
                } else {
                    lo = mid;
                }
            }
            Some(at(toward, hi))
        };
        let (first, second) = if lighter { (255.0, 0.0) } else { (0.0, 255.0) };
        if let Some(c) = solve(first).or_else(|| solve(second)) {
            if p.is_icon() {
                scene.styles[p.style]
                    .vars
                    .insert("--icon-color".to_string(), c.to_css());
            } else {
                scene.styles[p.style]
                    .props
                    .insert("color".to_string(), c.to_css());
            }
        }
    }
}

/// Seuil WCAG d'un texte courant.
const TEXT_MIN: f64 = 4.5;

/// Context of the collection: scene, page background and threshold of the accent fills.
struct Collector<'a> {
    sc: &'a Scene,
    page: Rgba,
    accent_min: f64,
    out: Vec<Pair>,
}

impl Collector<'_> {
    fn add(
        &mut self,
        element: String,
        classes: &[String],
        what: &'static str,
        style: usize,
        back: Rgba,
        required: f64,
    ) {
        self.out.push(Pair {
            element,
            classes: classes.to_vec(),
            what,
            style,
            back,
            required,
        });
    }

    /// Own background of an element composed over what lies under it.
    fn back_of(&self, el: usize, under: Rgba) -> Rgba {
        over(bg(self.sc.style(el)), under)
    }

    /// Background under a node: its parent zones composed over the page.
    fn under(&self, model: &Model, id: &str) -> Rgba {
        let mut under = self.page;
        for z in model.ancestors(id).iter().rev() {
            if let Some(ze) = self.sc.zones.get(&z.id) {
                under = self.back_of(ze.el, under);
            }
        }
        under
    }

    /// Icon and link mark of a node, against their backgrounds (`icon`, `mark`).
    fn decor(&mut self, n: &Node, d: &crate::scene::Decor, (icon, mark): (Rgba, Rgba)) {
        if let Some((el, _)) = &d.icon {
            let id = format!("{}/icon", n.id);
            self.add(id, &n.classes, "icon", *el, icon, GRAPHIC_MIN);
        }
        if let Some((el, _)) = &d.mark {
            let id = format!("{}/link-mark", n.id);
            self.add(id, &n.classes, "icon", *el, mark, GRAPHIC_MIN);
        }
    }

    fn zone(&mut self, n: &Node, z: &crate::scene::ZoneEl, under: Rgba) {
        if n.title.is_empty() {
            return;
        }
        let zb = self.back_of(z.el, under);
        self.decor(n, &z.decor, (zb, zb));
        self.add(
            format!("{}/title", n.id),
            &n.classes,
            "zone title",
            z.title,
            zb,
            TEXT_MIN,
        );
    }

    fn card(&mut self, n: &Node, c: &crate::scene::CardEl, under: Rgba) {
        let cb = self.back_of(c.el, under);
        if let Some(h) = &c.header {
            let hb = self.back_of(h.head, cb);
            // the icon is on the band; the mark too in the "band" look, otherwise on the card
            self.decor(n, &c.decor, (hb, if h.band { hb } else { cb }));
            if !h.type_text.is_empty() {
                let min = self.accent_min;
                self.add(
                    format!("{}/bandeau", n.id),
                    &n.classes,
                    "band text",
                    h.type_el,
                    hb,
                    min,
                );
            }
            if let Some((t, _)) = &h.tag {
                let tb = self.back_of(*t, hb);
                self.add(
                    format!("{}/tag", n.id),
                    &n.classes,
                    "chip",
                    *t,
                    tb,
                    TEXT_MIN,
                );
            }
        }
        if c.header.is_none() {
            self.decor(n, &c.decor, (cb, cb));
        }
        if let Some((b, _)) = &c.badge {
            let (bb, min) = (self.back_of(*b, cb), self.accent_min);
            self.add(format!("{}/badge", n.id), &n.classes, "badge", *b, bb, min);
        }
        if !n.title.is_empty() {
            self.add(n.id.clone(), &n.classes, "title", c.title, cb, TEXT_MIN);
        }
        for (k, li) in c.lis.iter().enumerate() {
            if !li.rest.trim().is_empty() {
                self.add(
                    format!("{}/li[{k}]", n.id),
                    &n.classes,
                    "line",
                    li.el,
                    cb,
                    TEXT_MIN,
                );
            }
            if let Some((m, _)) = li.mono {
                self.add(
                    format!("{}/li[{k}]/ip", n.id),
                    &n.classes,
                    "ligne (IP)",
                    m,
                    cb,
                    TEXT_MIN,
                );
            }
        }
    }
}

/// Every text/background pair of the diagram.
fn pairs(model: &Model, sc: &Scene, theme: Theme) -> Vec<Pair> {
    let stage = sc.style(sc.stage);
    let mut cx = Collector {
        sc,
        page: over(bg(stage).or_else(|| stage.var_color("--bg")), Rgba::WHITE),
        accent_min: if theme == Theme::Print { 4.5 } else { 3.0 },
        out: Vec::new(),
    };
    for n in &model.nodes {
        let under = cx.under(model, &n.id);
        if let Some(z) = sc.zones.get(&n.id) {
            cx.zone(n, z, under);
        } else if let Some(c) = sc.cards.get(&n.id) {
            cx.card(n, c, under);
        }
    }
    for e in &model.edges {
        if let Some(&l) = sc.labels.get(&e.id) {
            let lb = cx.back_of(l, cx.page);
            cx.add(
                format!("{}/label", e.id),
                &e.classes,
                "arrow label",
                l,
                lb,
                TEXT_MIN,
            );
        }
    }
    cx.out
}

/// Name of a kind of text (`Item::what`) in the current language.
pub fn describe(what: &str) -> String {
    match what {
        "zone title" => tr!("zone title", "titre de zone"),
        "band text" => tr!("band text", "texte du bandeau"),
        "chip" => tr!("chip", "pastille"),
        "badge" => tr!("badge", "badge"),
        "icon" => tr!("icon", "icône"),
        "title" => tr!("title", "titre"),
        "line" => tr!("line", "ligne"),
        "arrow label" => tr!("arrow label", "libellé de flèche"),
        other => other.to_string(),
    }
}
