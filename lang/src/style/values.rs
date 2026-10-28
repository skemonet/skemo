//! Concrete CSS values: colours (`#hex`, `rgb[a]()`, names, `color-mix(in srgb, …)`) and lengths.

use super::css::split_top;

/// Couleur sRGB, composantes 0–255 (non arrondies), alpha 0–1.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rgba {
    pub r: f64,
    pub g: f64,
    pub b: f64,
    pub a: f64,
}

impl Rgba {
    pub const BLACK: Rgba = Rgba {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };
    pub const WHITE: Rgba = Rgba {
        r: 255.0,
        g: 255.0,
        b: 255.0,
        a: 1.0,
    };
    pub const TRANSPARENT: Rgba = Rgba {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 0.0,
    };

    /// Forme CSS, composantes arrondies (`#RRGGBB` ou `rgba(r, g, b, a)`).
    pub fn to_css(&self) -> String {
        let c = |v: f64| v.round().clamp(0.0, 255.0) as u8;
        if self.a >= 0.999 {
            format!("#{:02X}{:02X}{:02X}", c(self.r), c(self.g), c(self.b))
        } else {
            let a = (self.a * 1000.0).round() / 1000.0;
            format!("rgba({}, {}, {}, {})", c(self.r), c(self.g), c(self.b), a)
        }
    }

    /// Relative luminance (WCAG 2), on an opaque background.
    pub fn luminance(&self) -> f64 {
        let f = |v: f64| {
            let s = (v / 255.0).clamp(0.0, 1.0);
            if s <= 0.04045 {
                s / 12.92
            } else {
                ((s + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * f(self.r) + 0.7152 * f(self.g) + 0.0722 * f(self.b)
    }

    /// Lays `self` over an opaque background.
    pub fn over(&self, bg: Rgba) -> Rgba {
        let a = self.a.clamp(0.0, 1.0);
        Rgba {
            r: self.r * a + bg.r * (1.0 - a),
            g: self.g * a + bg.g * (1.0 - a),
            b: self.b * a + bg.b * (1.0 - a),
            a: 1.0,
        }
    }

    /// Rapport de contraste WCAG entre deux couleurs opaques.
    pub fn contrast(&self, other: Rgba) -> f64 {
        let (a, b) = (self.luminance(), other.luminance());
        let (hi, lo) = if a > b { (a, b) } else { (b, a) };
        (hi + 0.05) / (lo + 0.05)
    }
}

const NAMED: [(&str, u32); 22] = [
    ("black", 0x000000),
    ("white", 0xFFFFFF),
    ("red", 0xFF0000),
    ("green", 0x008000),
    ("blue", 0x0000FF),
    ("gray", 0x808080),
    ("grey", 0x808080),
    ("silver", 0xC0C0C0),
    ("orange", 0xFFA500),
    ("yellow", 0xFFFF00),
    ("purple", 0x800080),
    ("navy", 0x000080),
    ("teal", 0x008080),
    ("maroon", 0x800000),
    ("olive", 0x808000),
    ("lime", 0x00FF00),
    ("aqua", 0x00FFFF),
    ("fuchsia", 0xFF00FF),
    ("darkgray", 0xA9A9A9),
    ("lightgray", 0xD3D3D3),
    ("gold", 0xFFD700),
    ("crimson", 0xDC143C),
];

fn hex(s: &str) -> Option<Rgba> {
    let h = s.strip_prefix('#')?;
    let d = |c: &str| u8::from_str_radix(c, 16).ok().map(f64::from);
    let one = |c: char| d(&format!("{c}{c}"));
    let ch: Vec<char> = h.chars().collect();
    match ch.len() {
        3 | 4 => Some(Rgba {
            r: one(ch[0])?,
            g: one(ch[1])?,
            b: one(ch[2])?,
            a: if ch.len() == 4 {
                one(ch[3])? / 255.0
            } else {
                1.0
            },
        }),
        6 | 8 => Some(Rgba {
            r: d(&h[0..2])?,
            g: d(&h[2..4])?,
            b: d(&h[4..6])?,
            a: if ch.len() == 8 {
                d(&h[6..8])? / 255.0
            } else {
                1.0
            },
        }),
        _ => None,
    }
}

fn func<'a>(s: &'a str, name: &str) -> Option<&'a str> {
    let s = s.trim();
    let lower = s.get(..name.len() + 1)?.to_ascii_lowercase();
    if lower == format!("{name}(") && s.ends_with(')') {
        Some(&s[name.len() + 1..s.len() - 1])
    } else {
        None
    }
}

fn number_or_percent(s: &str, scale: f64) -> Option<f64> {
    let s = s.trim();
    match s.strip_suffix('%') {
        Some(p) => p.trim().parse::<f64>().ok().map(|v| v / 100.0 * scale),
        None => s.parse().ok(),
    }
}

/// Reads a colour. `current` = value of `currentColor`.
pub fn parse_color(s: &str, current: Rgba) -> Option<Rgba> {
    let s = s.trim();
    let lower = s.to_ascii_lowercase();
    if lower == "currentcolor" {
        return Some(current);
    }
    if lower == "transparent" {
        return Some(Rgba::TRANSPARENT);
    }
    if let Some(&(_, v)) = NAMED.iter().find(|(n, _)| *n == lower) {
        return Some(Rgba {
            r: f64::from((v >> 16) & 255),
            g: f64::from((v >> 8) & 255),
            b: f64::from(v & 255),
            a: 1.0,
        });
    }
    if s.starts_with('#') {
        return hex(s);
    }
    if let Some(args) = func(s, "rgba").or_else(|| func(s, "rgb")) {
        let (main, alpha) = match args.split_once('/') {
            Some((m, a)) => (m, Some(a)),
            None => (args, None),
        };
        let parts: Vec<&str> = if main.contains(',') {
            main.split(',').collect()
        } else {
            main.split_whitespace().collect()
        };
        if parts.len() < 3 {
            return None;
        }
        let a = match (alpha, parts.get(3).copied()) {
            (Some(a), _) | (None, Some(a)) => number_or_percent(a, 1.0)?,
            _ => 1.0,
        };
        return Some(Rgba {
            r: number_or_percent(parts[0], 255.0)?,
            g: number_or_percent(parts[1], 255.0)?,
            b: number_or_percent(parts[2], 255.0)?,
            a,
        });
    }
    if let Some(args) = func(s, "color-mix") {
        let parts = split_top(args, ',');
        if parts.len() != 3 || !parts[0].trim().eq_ignore_ascii_case("in srgb") {
            return None;
        }
        let item = |p: &str| -> Option<(Rgba, Option<f64>)> {
            let p = p.trim();
            // possible percentage as the last (or first) token of level 0
            let toks = split_top(p, ' ');
            let toks: Vec<&str> = toks.into_iter().filter(|t| !t.is_empty()).collect();
            let (col, pct) = match (toks.first(), toks.last()) {
                (Some(f), _) if f.ends_with('%') && toks.len() > 1 => {
                    (toks[1..].join(" "), Some(f))
                }
                (_, Some(l)) if l.ends_with('%') && toks.len() > 1 => {
                    (toks[..toks.len() - 1].join(" "), Some(l))
                }
                _ => (toks.join(" "), None),
            };
            let pct = match pct {
                Some(p) => Some(p.trim_end_matches('%').parse::<f64>().ok()? / 100.0),
                None => None,
            };
            Some((parse_color(&col, current)?, pct))
        };
        let (c1, p1) = item(parts[1])?;
        let (c2, p2) = item(parts[2])?;
        let (mut w1, mut w2) = match (p1, p2) {
            (Some(a), Some(b)) => (a, b),
            (Some(a), None) => (a, 1.0 - a),
            (None, Some(b)) => (1.0 - b, b),
            (None, None) => (0.5, 0.5),
        };
        let sum = w1 + w2;
        if sum <= 0.0 {
            return None;
        }
        let alpha_mult = if sum < 1.0 { sum } else { 1.0 };
        w1 /= sum;
        w2 /= sum;
        // interpolation in premultiplied alpha (CSS Color 5)
        let a = c1.a * w1 + c2.a * w2;
        let mix = |x: f64, y: f64| {
            if a > 0.0 {
                (x * c1.a * w1 + y * c2.a * w2) / a
            } else {
                0.0
            }
        };
        return Some(Rgba {
            r: mix(c1.r, c2.r),
            g: mix(c1.g, c2.g),
            b: mix(c1.b, c2.b),
            a: a * alpha_mult,
        });
    }
    None
}

/// Reads a length in px. `em` = reference font size; `percent_of` = base of the %.
pub fn parse_length(s: &str, em: f64, percent_of: Option<f64>) -> Option<f64> {
    let s = s.trim().to_ascii_lowercase();
    if s == "0" {
        return Some(0.0);
    }
    if let Some(inner) = s.strip_prefix("calc(").and_then(|r| r.strip_suffix(')')) {
        return calc(inner, em, percent_of);
    }
    let num = |t: &str| t.trim().parse::<f64>().ok();
    if let Some(v) = s.strip_suffix("px") {
        return num(v);
    }
    if let Some(v) = s.strip_suffix("rem") {
        return num(v).map(|v| v * 16.0);
    }
    if let Some(v) = s.strip_suffix("em") {
        return num(v).map(|v| v * em);
    }
    if let Some(v) = s.strip_suffix('%') {
        return Some(num(v)? / 100.0 * percent_of?);
    }
    if let Some(v) = s.strip_suffix("pt") {
        return num(v).map(|v| v * 4.0 / 3.0);
    }
    None
}

/// `calc()` on lengths: `+ - * /` and parentheses, a length times or divided by a number
/// (`calc(13px * 1.4)`, `calc(100% - 2 * 6px)`). Anything else gives `None`.
fn calc(expr: &str, em: f64, percent_of: Option<f64>) -> Option<f64> {
    let tokens = calc_tokens(expr)?;
    let mut pos = 0;
    let (v, is_len) = calc_sum(&tokens, &mut pos, em, percent_of)?;
    (pos == tokens.len() && (is_len || v == 0.0)).then_some(v)
}

fn calc_tokens(expr: &str) -> Option<Vec<String>> {
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    for c in expr.chars() {
        let unary = c == '-'
            && cur.is_empty()
            && !matches!(out.last().map(String::as_str), Some(t) if !"+-*/(".contains(t));
        if "+*/()".contains(c) || (c == '-' && !unary) || c.is_whitespace() {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
            if !c.is_whitespace() {
                out.push(c.to_string());
            }
        } else {
            cur.push(c);
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    (out.len() <= 64).then_some(out)
}

type CalcValue = (f64, bool);

fn calc_sum(t: &[String], pos: &mut usize, em: f64, pct: Option<f64>) -> Option<CalcValue> {
    let mut acc = calc_product(t, pos, em, pct)?;
    while let Some(op) = t.get(*pos).filter(|o| *o == "+" || *o == "-") {
        let neg = op == "-";
        *pos += 1;
        let r = calc_product(t, pos, em, pct)?;
        if r.1 != acc.1 && r.0 != 0.0 && acc.0 != 0.0 {
            return None;
        }
        acc = (if neg { acc.0 - r.0 } else { acc.0 + r.0 }, acc.1 || r.1);
    }
    Some(acc)
}

fn calc_product(t: &[String], pos: &mut usize, em: f64, pct: Option<f64>) -> Option<CalcValue> {
    let mut acc = calc_atom(t, pos, em, pct)?;
    while let Some(op) = t.get(*pos).filter(|o| *o == "*" || *o == "/") {
        let div = op == "/";
        *pos += 1;
        let r = calc_atom(t, pos, em, pct)?;
        acc = if div {
            if r.1 || r.0 == 0.0 {
                return None;
            }
            (acc.0 / r.0, acc.1)
        } else {
            if acc.1 && r.1 {
                return None;
            }
            (acc.0 * r.0, acc.1 || r.1)
        };
    }
    Some(acc)
}

fn calc_atom(t: &[String], pos: &mut usize, em: f64, pct: Option<f64>) -> Option<CalcValue> {
    let tok = t.get(*pos)?;
    *pos += 1;
    if tok == "(" {
        let v = calc_sum(t, pos, em, pct)?;
        (t.get(*pos)? == ")").then_some(())?;
        *pos += 1;
        return Some(v);
    }
    if let Ok(n) = tok.parse::<f64>() {
        return Some((n, false));
    }
    if tok.starts_with("calc(") {
        return None;
    }
    parse_length(tok, em, pct).map(|v| (v, true))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calc_lengths() {
        assert_eq!(parse_length("calc(13px * 1.5)", 16.0, None), Some(19.5));
        assert_eq!(parse_length("calc(2 * 6px + 1em)", 10.0, None), Some(22.0));
        assert_eq!(
            parse_length("calc(100% - 6px)", 16.0, Some(50.0)),
            Some(44.0)
        );
        assert_eq!(parse_length("calc((4px + 2px) / 2)", 16.0, None), Some(3.0));
        assert_eq!(parse_length("calc(-2px * 2)", 16.0, None), Some(-4.0));
        assert_eq!(parse_length("calc(2px * 3px)", 16.0, None), None);
        assert_eq!(parse_length("calc(1.5)", 16.0, None), None);
        assert_eq!(parse_length("calc(4px / 0)", 16.0, None), None);
        assert_eq!(parse_length("calc(4px +)", 16.0, None), None);
    }

    #[test]
    fn colors() {
        let c = parse_color("#C94A44", Rgba::BLACK).unwrap();
        assert_eq!(c.to_css(), "#C94A44");
        let m = parse_color("color-mix(in srgb, #C94A44 85%, #000)", Rgba::BLACK).unwrap();
        assert_eq!(m.to_css(), "#AB3F3A");
        let m = parse_color(
            "color-mix(in srgb, #FFFFFF 50%, rgba(0,0,0,0))",
            Rgba::BLACK,
        )
        .unwrap();
        assert!((m.a - 0.5).abs() < 1e-9 && m.r == 255.0);
        assert_eq!(
            parse_color("rgba(20,30,50,.06)", Rgba::BLACK).unwrap().a,
            0.06
        );
    }

    #[test]
    fn contrast() {
        assert!((Rgba::BLACK.contrast(Rgba::WHITE) - 21.0).abs() < 1e-9);
    }

    #[test]
    fn lengths() {
        assert_eq!(parse_length(".03em", 10.0, None), Some(0.3));
        assert_eq!(parse_length("1.5px", 10.0, None), Some(1.5));
    }
}
