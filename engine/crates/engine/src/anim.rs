//! Resolution of the animation markers (F-ANIM-1, 2; FORMAT §3.8).
//!
//! Animatable elements: cards, zones, `li` lines (key `card/li[k]`), an animated chip (`card/tag`,
//! when `tag=` carries a marker) and links (`a->b`).

use serde::Serialize;
use skemo_lang::tr;
use skemo_lang::{Anim, Focus, Kind, Model, Step};
use std::collections::BTreeMap;

/// Effective step: value and text as written (copied into `data-step`).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct At {
    pub raw: String,
    pub value: f64,
}

impl At {
    fn zero() -> At {
        At {
            raw: "0".into(),
            value: 0.0,
        }
    }
    fn of(s: &Step) -> At {
        At {
            raw: s.raw.clone(),
            value: s.value,
        }
    }
}

/// Resolved animation of an element.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Resolved {
    pub appear: At,
    /// Last visible step (`None` = never hidden).
    pub until: Option<At>,
    pub fx: String,
    pub focus: Vec<(At, Option<At>)>,
    /// Group that holds it (`None` = root); for a link, `None`.
    pub container: Option<String>,
    pub is_link: bool,
    pub from: Option<String>,
    pub to: Option<String>,
}

impl Resolved {
    fn container_appear<'a>(&self, all: &'a BTreeMap<String, Resolved>) -> f64 {
        self.container
            .as_ref()
            .and_then(|c| all.get(c))
            .map_or(0.0, |c: &'a Resolved| c.appear.value)
    }
    fn container_until(&self, all: &BTreeMap<String, Resolved>) -> Option<f64> {
        self.container
            .as_ref()
            .and_then(|c| all.get(c))
            .and_then(|c| c.until.as_ref().map(|u| u.value))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Warning {
    pub element: String,
    pub code: &'static str,
    pub message: String,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Animation {
    pub elements: BTreeMap<String, Resolved>,
    /// States, in order (values as written).
    pub states: Vec<At>,
    pub warnings: Vec<Warning>,
}

fn max_at(a: &At, b: &At) -> At {
    if b.value > a.value {
        b.clone()
    } else {
        a.clone()
    }
}

fn min_until(a: &Option<At>, b: &Option<At>) -> Option<At> {
    match (a, b) {
        (Some(x), Some(y)) => Some(if y.value < x.value {
            y.clone()
        } else {
            x.clone()
        }),
        (Some(x), None) | (None, Some(x)) => Some(x.clone()),
        (None, None) => None,
    }
}

fn kind_word(model: &Model, id: &str) -> String {
    match model.node(id).map(|n| n.kind) {
        Some(Kind::Zone) => tr!("its zone", "sa zone"),
        _ => tr!("its card", "sa carte"),
    }
}

/// Appearance, end, effect and focus ranges after inheritance.
type Inherited = (At, Option<At>, String, Vec<(At, Option<At>)>);

/// Applies the inheritance rule to an element that has (or not) markers of its own.
#[allow(clippy::too_many_arguments)]
fn inherit(
    id: &str,
    own: Option<&Anim>,
    inh_appear: &At,
    inh_until: &Option<At>,
    parent_desc: &str,
    default_fx: &str,
    warnings: &mut Vec<Warning>,
) -> Inherited {
    let mut appear = inh_appear.clone();
    let mut until = inh_until.clone();
    let mut fx = default_fx.to_string();
    let mut focus = Vec::new();
    if let Some(a) = own {
        if let Some(s) = &a.start {
            if s.value < inh_appear.value {
                warnings.push(Warning {
                    element: id.into(),
                    code: "step-before-parent",
                    message: tr!(
                        "« {id} »: @{} is earlier than {parent_desc} (@{}); step kept: {}",
                        "« {id} » : @{} est plus tôt que {parent_desc} (@{}) ; étape retenue : {}",
                        s.raw,
                        inh_appear.raw,
                        inh_appear.raw
                    ),
                });
            } else {
                appear = At::of(s);
            }
        }
        if let Some(u) = &a.until {
            match inh_until {
                Some(iu) if u.value > iu.value => warnings.push(Warning {
                    element: id.into(),
                    code: "until-after-parent",
                    message: tr!("« {id} »: end @{} is later than {parent_desc} (@{}); end kept: {}", "« {id} » : fin @{} plus tardive que {parent_desc} (@{}) ; fin retenue : {}", u.raw, iu.raw, iu.raw),
                }),
                _ => until = Some(At::of(u)),
            }
        }
        if let Some(f) = &a.fx {
            fx = f.clone();
        }
        focus = a
            .focus
            .iter()
            .map(|Focus { from, to }| (At::of(from), to.as_ref().map(At::of)))
            .collect();
    }
    (appear, until, fx, focus)
}

/// Resolves every animation of the model.
pub fn resolve(model: &Model) -> Animation {
    let mut out = Animation::default();
    let mut warnings = Vec::new();
    resolve_nodes(model, &mut out, &mut warnings);
    resolve_edges(model, &mut out, &mut warnings);
    warn_focus_outside(&out, &mut warnings);
    let vals = step_values(&out);
    warn_never_visible(&out, &vals, &mut warnings);
    out.states = vals;
    warnings.sort_by(|a, b| a.element.cmp(&b.element).then(a.code.cmp(b.code)));
    warnings.dedup();
    out.warnings = warnings;
    out
}

/// Nodes and their lines, in writing order (parents before their children): each inherits the
/// visibility of its container.
fn resolve_nodes(model: &Model, out: &mut Animation, warnings: &mut Vec<Warning>) {
    for n in &model.nodes {
        let parent = n.parent.as_ref().and_then(|p| out.elements.get(p));
        let (pa, pu) = parent.map_or((At::zero(), None), |p| (p.appear.clone(), p.until.clone()));
        let desc = n
            .parent
            .as_ref()
            .map(|p| format!("{} « {p} »", kind_word(model, p)))
            .unwrap_or_default();
        let (appear, until, fx, focus) =
            inherit(&n.id, n.anim.as_ref(), &pa, &pu, &desc, "fade", warnings);
        out.elements.insert(
            n.id.clone(),
            Resolved {
                appear: appear.clone(),
                until: until.clone(),
                fx,
                focus,
                container: n.parent.clone(),
                is_link: false,
                from: None,
                to: None,
            },
        );
        if let Some(ta) = &n.tag_anim {
            let key = format!("{}/tag", n.id);
            let desc = tr!("its card « {} »", "sa carte « {} »", n.id);
            let (a, u, fx, focus) =
                inherit(&key, Some(ta), &appear, &until, &desc, "fade", warnings);
            out.elements.insert(
                key,
                Resolved {
                    appear: a,
                    until: u,
                    fx,
                    focus,
                    container: Some(n.id.clone()),
                    is_link: false,
                    from: None,
                    to: None,
                },
            );
        }
        for (k, li) in n.li.iter().enumerate() {
            let key = format!("{}/li[{k}]", n.id);
            let desc = tr!("its card « {} »", "sa carte « {} »", n.id);
            let (a, u, fx, focus) = inherit(
                &key,
                li.anim.as_ref(),
                &appear,
                &until,
                &desc,
                "fade",
                warnings,
            );
            out.elements.insert(
                key,
                Resolved {
                    appear: a,
                    until: u,
                    fx,
                    focus,
                    container: Some(n.id.clone()),
                    is_link: false,
                    from: None,
                    to: None,
                },
            );
        }
    }
}

/// Arrows: visible when both their ends are.
fn resolve_edges(model: &Model, out: &mut Animation, warnings: &mut Vec<Warning>) {
    for e in &model.edges {
        let (Some(f), Some(t)) = (out.elements.get(&e.from), out.elements.get(&e.to)) else {
            continue;
        };
        let ia = max_at(&f.appear, &t.appear);
        let iu = min_until(&f.until, &t.until);
        let (appear, until, fx, focus) = inherit(
            &e.id,
            e.anim.as_ref(),
            &ia,
            &iu,
            &tr!("its endpoints", "ses extrémités"),
            "draw",
            warnings,
        );
        out.elements.insert(
            e.id.clone(),
            Resolved {
                appear,
                until,
                fx,
                focus,
                container: None,
                is_link: true,
                from: Some(e.from.clone()),
                to: Some(e.to.clone()),
            },
        );
    }
}

/// Warns about an emphasis outside the element's visible period.
fn warn_focus_outside(out: &Animation, warnings: &mut Vec<Warning>) {
    for (id, r) in &out.elements {
        for (a, b) in &r.focus {
            let end = b.as_ref().map_or(a.value, |b| b.value);
            let visible_end = r.until.as_ref().map_or(f64::INFINITY, |u| u.value);
            if a.value < r.appear.value || end > visible_end {
                warnings.push(Warning {
                    element: id.clone(),
                    code: "focus-outside",
                    message: tr!(
                        "« {id} »: highlight @{} is outside the visibility period",
                        "« {id} » : mise en valeur @{} hors de la période de visibilité",
                        a.raw
                    ),
                });
            }
        }
    }
}

/// States: values written, sorted and deduplicated; 0 only if it holds something.
fn step_values(out: &Animation) -> Vec<At> {
    let mut vals: Vec<At> = Vec::new();
    for r in out.elements.values() {
        let mut push = |a: &At| {
            if a.value > 0.0 && !vals.iter().any(|v| v.value == a.value) {
                vals.push(a.clone());
            }
        };
        push(&r.appear);
        if let Some(u) = &r.until {
            push(u);
        }
        for (a, b) in &r.focus {
            push(a);
            if let Some(b) = b {
                push(b);
            }
        }
    }
    vals.sort_by(|a, b| a.value.total_cmp(&b.value));
    if out.elements.values().any(|r| r.appear.value == 0.0) {
        vals.insert(0, At::zero());
    }
    vals
}

/// Warns about a disappearance no step follows (never visible).
fn warn_never_visible(out: &Animation, vals: &[At], warnings: &mut Vec<Warning>) {
    for (id, r) in &out.elements {
        if let Some(u) = &r.until
            && !vals.iter().any(|v| v.value > u.value)
            && r.container_until(&out.elements)
                .is_none_or(|cu| cu > u.value)
        {
            warnings.push(Warning {
                element: id.clone(),
                code: "no-step-after",
                message: tr!(
                    "« {id} »: @…-{}: no step after {}",
                    "« {id} » : @…-{} : aucune étape après {}",
                    u.raw,
                    u.raw
                ),
            });
        }
    }
}

impl Animation {
    pub fn visible(&self, id: &str, state: f64) -> bool {
        self.elements.get(id).is_none_or(|r| {
            r.appear.value <= state && r.until.as_ref().is_none_or(|u| state <= u.value)
        })
    }

    pub fn focused(&self, id: &str, state: f64) -> bool {
        self.elements.get(id).is_some_and(|r| {
            self.visible(id, state)
                && r.focus.iter().any(|(a, b)| {
                    a.value <= state && state <= b.as_ref().map_or(a.value, |b| b.value)
                })
        })
    }

    /// Animation `data-*` attributes of an element (FORMAT §4.4), in writing order.
    pub fn attributes(&self, id: &str) -> Vec<(&'static str, String)> {
        let Some(r) = self.elements.get(id) else {
            return Vec::new();
        };
        let mut a = Vec::new();
        let animated = if r.is_link {
            r.appear.value > 0.0
        } else {
            r.appear.value > r.container_appear(&self.elements)
        };
        if animated {
            a.push(("data-step", r.appear.raw.clone()));
            a.push(("data-fx", r.fx.clone()));
        }
        if let Some(u) = &r.until {
            let cu = if r.is_link {
                None
            } else {
                r.container_until(&self.elements)
            };
            if cu.is_none_or(|c| u.value < c) {
                a.push(("data-until", u.raw.clone()));
            }
        }
        if !r.focus.is_empty() {
            let f: Vec<String> = r
                .focus
                .iter()
                .map(|(x, y)| match y {
                    Some(y) => format!("{}-{}", x.raw, y.raw),
                    None => x.raw.clone(),
                })
                .collect();
            a.push(("data-focus", f.join(" ")));
        }
        if r.is_link && !a.is_empty() {
            a.push(("data-from", r.from.clone().unwrap_or_default()));
            a.push(("data-to", r.to.clone().unwrap_or_default()));
        }
        a
    }

    /// Is the diagram animated?
    pub fn is_animated(&self) -> bool {
        self.states.len() > 1 || self.states.first().is_some_and(|s| s.value > 0.0)
    }
}

/// Presence of an element in a frame: opacity (focus dimming included), vertical offset, scale,
/// fraction drawn (links), strength of the focus outline.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Presence {
    pub opacity: f64,
    pub dy: f64,
    pub scale: f64,
    pub draw: f64,
    pub ring: f64,
}

impl Presence {
    pub const FULL: Presence = Presence {
        opacity: 1.0,
        dy: 0.0,
        scale: 1.0,
        draw: 1.0,
        ring: 0.0,
    };
    pub const HIDDEN: Presence = Presence {
        opacity: 0.0,
        dy: 0.0,
        scale: 1.0,
        draw: 0.0,
        ring: 0.0,
    };
}

/// What a frame shows: the presence of each animatable element.
#[derive(Debug, Clone, Default)]
pub struct View {
    pub map: BTreeMap<String, Presence>,
}

impl View {
    pub fn get(&self, key: &str) -> Presence {
        self.map.get(key).copied().unwrap_or(Presence::FULL)
    }
}

/// "ease" curve (approximated), for the transitions.
fn ease(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

impl Animation {
    fn parent_of<'a>(&self, model: &'a Model, key: &'a str) -> Option<&'a str> {
        if let Some((card, _)) = key.split_once("/li[") {
            return Some(card);
        }
        if let Some(card) = key.strip_suffix("/tag") {
            return Some(card);
        }
        model.node(key).and_then(|n| n.parent.as_deref())
    }

    /// Dimming (0 or 1) of each element at state `s`: the highest element that is neither in focus nor
    /// parent of an element in focus is dimmed (children inherit in the nested SVG).
    fn dims(&self, model: &Model, s: f64) -> BTreeMap<String, (f64, f64)> {
        let focused: Vec<&String> = self
            .elements
            .keys()
            .filter(|k| self.focused(k, s))
            .collect();
        let mut out = BTreeMap::new();
        let any = !focused.is_empty();
        for k in self.elements.keys() {
            if !any {
                out.insert(k.clone(), (0.0, 0.0));
                continue;
            }
            let is_f = focused.contains(&k);
            let has_f_desc = focused.iter().any(|f| {
                let mut cur = self.parent_of(model, f);
                while let Some(p) = cur {
                    if p == k {
                        return true;
                    }
                    cur = self.parent_of(model, p);
                }
                false
            });
            let anc_dimmed = {
                let mut cur = self.parent_of(model, k);
                let mut d = false;
                while let Some(p) = cur {
                    let pf = focused.iter().any(|f| f.as_str() == p);
                    let p_has = focused.iter().any(|f| {
                        let mut c = self.parent_of(model, f);
                        while let Some(q) = c {
                            if q == p {
                                return true;
                            }
                            c = self.parent_of(model, q);
                        }
                        false
                    });
                    if !pf && !p_has {
                        d = true;
                        break;
                    }
                    cur = self.parent_of(model, p);
                }
                d
            };
            let anc_focused = {
                let mut cur = self.parent_of(model, k);
                let mut f = false;
                while let Some(p) = cur {
                    if focused.iter().any(|x| x.as_str() == p) {
                        f = true;
                        break;
                    }
                    cur = self.parent_of(model, p);
                }
                f
            };
            let dim = if !is_f && !has_f_desc && !anc_focused && !anc_dimmed {
                1.0
            } else {
                0.0
            };
            let ring = if is_f && !self.elements[k].is_link {
                1.0
            } else {
                0.0
            };
            out.insert(k.clone(), (dim, ring));
        }
        out
    }

    /// Still frame at state `s` (no transition effect, with the focus).
    pub fn view_at(&self, model: &Model, s: f64, dim_opacity: f64) -> View {
        let dims = self.dims(model, s);
        let mut v = View::default();
        for k in self.elements.keys() {
            let p = if self.visible(k, s) {
                let (d, ring) = dims[k];
                Presence {
                    opacity: 1.0 - d * (1.0 - dim_opacity),
                    ring,
                    ..Presence::FULL
                }
            } else {
                Presence::HIDDEN
            };
            v.map.insert(k.clone(), p);
        }
        v
    }

    /// Middle frame of the transition from state `s0` to state `s1`, at time `t` (0 → 1).
    pub fn view_between(&self, model: &Model, s0: f64, s1: f64, t: f64, dim_opacity: f64) -> View {
        let e = ease(t);
        let (d0, d1) = (self.dims(model, s0), self.dims(model, s1));
        let mut v = View::default();
        for (k, r) in &self.elements {
            let (a, b) = (self.visible(k, s0), self.visible(k, s1));
            let (dim, ring) = {
                let (x0, r0) = d0[k];
                let (x1, r1) = d1[k];
                (x0 + (x1 - x0) * e, r0 + (r1 - r0) * e)
            };
            let base = Presence {
                opacity: 1.0 - dim * (1.0 - dim_opacity),
                ring,
                ..Presence::FULL
            };
            // progress of presence: 0 = absent, 1 = present
            let p = match (a, b) {
                (true, true) => 1.0,
                (false, false) => 0.0,
                (false, true) => e,
                (true, false) => 1.0 - e,
            };
            let mut pr = base;
            if p < 1.0 {
                match r.fx.as_str() {
                    "draw" => {
                        pr.draw = p;
                        pr.opacity *= if p > 0.0 { 1.0 } else { 0.0 };
                    }
                    "up" => {
                        pr.opacity *= p;
                        pr.dy = 12.0 * (1.0 - p);
                    }
                    "zoom" => {
                        pr.opacity *= p;
                        pr.scale = 0.92 + 0.08 * p;
                    }
                    _ => pr.opacity *= p,
                }
            }
            v.map.insert(k.clone(), pr);
        }
        v
    }
}
