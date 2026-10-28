//! Embedded fonts (E-MEAS-3) and text shaping (harfrust).
//!
//! No system font is read: the same files serve natively and in WebAssembly, which guarantees
//! identical measurements everywhere. Inter (OFL-1.1) and `JetBrains` Mono (OFL-1.1).
//!
//! Other fonts are handed over by the program (`Options::fonts`, `skemo render --font file.ttf`):
//! the engine reads no file. A registered font is used wherever a `font-family` names it, for the
//! measure and every output; a family named nowhere among the available fonts falls back on Inter
//! with a warning (`unavailable_families`).

use std::sync::{Mutex, OnceLock};

/// An embedded font.
pub struct FontFile {
    pub family: &'static str,
    pub weight: u16,
    pub italic: bool,
    pub data: &'static [u8],
    /// File name (for `@font-face` and the notices).
    pub file: &'static str,
}

macro_rules! font {
    ($family:expr, $w:expr, $it:expr, $file:expr) => {
        FontFile {
            family: $family,
            weight: $w,
            italic: $it,
            data: include_bytes!(concat!("../assets/fonts/", $file)),
            file: $file,
        }
    };
}

pub static FILES: [FontFile; 8] = [
    font!("Inter", 400, false, "Inter-Regular.otf"),
    font!("Inter", 400, true, "Inter-Italic.otf"),
    font!("Inter", 500, false, "Inter-Medium.otf"),
    font!("Inter", 500, true, "Inter-MediumItalic.otf"),
    font!("Inter", 600, false, "Inter-SemiBold.otf"),
    font!("Inter", 700, false, "Inter-Bold.otf"),
    font!("Inter", 800, false, "Inter-ExtraBold.otf"),
    font!("JetBrains Mono", 400, false, "JetBrainsMono-Regular.ttf"),
];

pub struct Font {
    pub file: &'static FontFile,
    pub face: harfrust::Font,
}

fn faces() -> &'static Vec<Font> {
    static FACES: OnceLock<Vec<Font>> = OnceLock::new();
    FACES.get_or_init(|| {
        FILES
            .iter()
            .filter_map(|f| harfrust::Font::new(f.data, 0).map(|face| Font { file: f, face }))
            .collect()
    })
}

/// Generic families and aliases: everything that is not an embedded font falls back on Inter (or
/// `JetBrains` Mono for `monospace`).
fn family_key(name: &str) -> Option<&'static str> {
    match name.to_ascii_lowercase().as_str() {
        "inter" | "system-ui" | "sans-serif" | "-apple-system" | "helvetica" | "arial"
        | "serif" => Some("Inter"),
        "jetbrains mono" | "monospace" | "dejavu sans mono" | "menlo" | "consolas" => {
            Some("JetBrains Mono")
        }
        _ => None,
    }
}

/// Range of CSS weights each file serves: `[min, max]`. It reproduces the choice observed in
/// Chromium on Linux (fontconfig) on the oracle: 650 → Bold, 750 → Bold. The exported HTML declares
/// the same ranges in its `@font-face`, so that the browser picks the same file.
pub fn weight_range(f: &FontFile) -> (u16, u16) {
    match (f.family, f.italic, f.weight) {
        ("Inter", _, 400) => (1, 449),
        ("Inter", false, 500) => (450, 549),
        ("Inter", false, 600) => (550, 649),
        ("Inter", false, 700) => (650, 750),
        ("Inter", false, 800) => (751, 1000),
        ("Inter", true, 500) => (450, 1000),
        ("Inter" | "JetBrains Mono", _, _) => (1, 1000),
        // a registered font: its own weight (the browser picks the nearest, as `select` does)
        _ => (f.weight, f.weight),
    }
}

/// Fonts registered by the program, besides the embedded ones (`register`).
fn user_fonts() -> &'static Mutex<Vec<&'static Font>> {
    static USER: OnceLock<Mutex<Vec<&'static Font>>> = OnceLock::new();
    USER.get_or_init(|| Mutex::new(Vec::new()))
}

/// The registered fonts (a copy of the list: they live as long as the program).
pub fn registered() -> Vec<&'static Font> {
    user_fonts().lock().map(|v| v.clone()).unwrap_or_default()
}

/// Registers a font file (TrueType or OpenType): its family, weight and style are read from the
/// file. A file registered twice is kept once. Returns the family name.
pub fn register(data: &[u8]) -> Result<String, String> {
    let (family, weight, italic) = describe(data).ok_or_else(|| {
        skemo_lang::tr!(
            "not a font file the engine can read (TrueType or OpenType)",
            "fichier de police illisible (TrueType ou OpenType attendu)"
        )
    })?;
    let Ok(mut list) = user_fonts().lock() else {
        return Ok(family);
    };
    if list.iter().any(|f| f.file.data == data) {
        return Ok(family);
    }
    // a font lives as long as the program: the measure and the renderers borrow it
    let data: &'static [u8] = Box::leak(data.to_vec().into_boxed_slice());
    let face = harfrust::Font::new(data, 0).ok_or_else(|| {
        skemo_lang::tr!(
            "font « {family} »: the file cannot be shaped",
            "police « {family} » : le fichier ne peut pas être mis en forme"
        )
    })?;
    let name: &'static str = Box::leak(family.clone().into_boxed_str());
    let file: &'static str = Box::leak(
        format!("{name}-{weight}{}", if italic { "-italic" } else { "" }).into_boxed_str(),
    );
    let ff: &'static FontFile = Box::leak(Box::new(FontFile {
        family: name,
        weight,
        italic,
        data,
        file,
    }));
    list.push(Box::leak(Box::new(Font { file: ff, face })));
    Ok(family)
}

/// The font of a registered family: the right style if there is one, the nearest weight.
fn select_registered(family: &str, weight: u16, italic: bool) -> Option<&'static Font> {
    let list = registered();
    let mut of: Vec<&'static Font> = list
        .into_iter()
        .filter(|f| f.file.family.eq_ignore_ascii_case(family))
        .collect();
    if of.is_empty() {
        return None;
    }
    if of.iter().any(|f| f.file.italic == italic) {
        of.retain(|f| f.file.italic == italic);
    }
    of.into_iter()
        .min_by_key(|f| (i32::from(f.file.weight) - i32::from(weight)).unsigned_abs())
}

/// `true` when `family` is drawn by some font: an embedded one, an alias of one, a generic
/// family, or a registered font.
pub fn available(family: &str) -> bool {
    family_key(family).is_some()
        || matches!(
            family.to_ascii_lowercase().as_str(),
            "ui-sans-serif" | "ui-monospace" | "ui-serif" | "cursive" | "fantasy" | "inherit"
        )
        || registered()
            .iter()
            .any(|f| f.file.family.eq_ignore_ascii_case(family))
}

/// Family, weight and style of a font file, read from its `name` and `OS/2` tables.
pub fn describe(data: &[u8]) -> Option<(String, u16, bool)> {
    let u16_at = |o: usize| Some(u16::from_be_bytes([*data.get(o)?, *data.get(o + 1)?]));
    let u32_at = |o: usize| {
        Some(u32::from_be_bytes([
            *data.get(o)?,
            *data.get(o + 1)?,
            *data.get(o + 2)?,
            *data.get(o + 3)?,
        ]))
    };
    let tag = u32_at(0)?;
    if !matches!(tag, 0x0001_0000 | 0x4F54_544F | 0x7472_7565) {
        return None;
    }
    let tables = usize::from(u16_at(4)?);
    let find = |name: &[u8; 4]| {
        (0..tables).find_map(|k| {
            let rec = 12 + 16 * k;
            (data.get(rec..rec + 4)? == name).then(|| u32_at(rec + 8).map(|o| o as usize))?
        })
    };
    let name = find(b"name")?;
    let (count, strings) = (
        usize::from(u16_at(name + 2)?),
        usize::from(u16_at(name + 4)?),
    );
    let mut family: Option<(u16, String)> = None;
    for k in 0..count {
        let r = name + 6 + 12 * k;
        let (platform, id) = (u16_at(r)?, u16_at(r + 6)?);
        let (len, off) = (usize::from(u16_at(r + 8)?), usize::from(u16_at(r + 10)?));
        // the typographic family (16) beats the legacy one (1)
        let rank = match id {
            16 => 2,
            1 => 1,
            _ => continue,
        };
        if family.as_ref().is_some_and(|(r, _)| *r >= rank) {
            continue;
        }
        let bytes = data.get(name + strings + off..name + strings + off + len)?;
        let text = if platform == 3 || platform == 0 {
            let units: Vec<u16> = bytes
                .as_chunks::<2>()
                .0
                .iter()
                .map(|c| u16::from_be_bytes(*c))
                .collect();
            String::from_utf16_lossy(&units)
        } else {
            bytes.iter().map(|&b| char::from(b)).collect()
        };
        if !text.trim().is_empty() {
            family = Some((rank, text.trim().to_string()));
        }
    }
    let (weight, italic) = match find(b"OS/2") {
        Some(o) => (
            u16_at(o + 4).unwrap_or(400),
            u16_at(o + 62).is_some_and(|f| f & 1 == 1),
        ),
        None => (400, false),
    };
    Some((family?.1, weight.clamp(1, 1000), italic))
}

/// Choice of a font: family (`font-family` list), then weight range, then style. A
/// registered font wins for its family name; the families are tried in the order written.
pub fn select(families: &[String], weight: u16, italic: bool) -> &'static Font {
    for f in families {
        if let Some(font) = select_registered(f, weight, italic) {
            return font;
        }
        if family_key(f).is_some() {
            break;
        }
    }
    let all = faces();
    let family = families
        .iter()
        .find_map(|f| family_key(f))
        .unwrap_or("Inter");
    let in_family = |it: bool| {
        all.iter()
            .filter(move |f| f.file.family == family && f.file.italic == it)
    };
    let by_weight = |it: bool| {
        in_family(it).find(|f| {
            let (lo, hi) = weight_range(f.file);
            (lo..=hi).contains(&weight)
        })
    };
    by_weight(italic)
        .or_else(|| in_family(italic).next())
        .or_else(|| by_weight(false))
        .or_else(|| all.first())
        .expect("embedded fonts")
}

/// Text style the measurement needs.
#[derive(Debug, Clone, PartialEq)]
pub struct TextStyle {
    pub families: Vec<String>,
    pub size: f64,
    pub weight: u16,
    pub italic: bool,
    pub letter_spacing: f64,
}

/// Rounded to 1/64 px (Chromium's layout unit).
pub fn layout_unit(v: f64) -> f64 {
    (v * 64.0).round() / 64.0
}

pub fn layout_unit_floor(v: f64) -> f64 {
    (v * 64.0 + 1e-6).floor() / 64.0
}

pub fn layout_unit_ceil(v: f64) -> f64 {
    (v * 64.0 - 1e-6).ceil() / 64.0
}

impl TextStyle {
    pub fn font(&self) -> &'static Font {
        select(&self.families, self.weight, self.italic)
    }

    /// Width of a string (a single line), letter spacing included.
    pub fn width(&self, text: &str) -> f64 {
        if text.is_empty() {
            return 0.0;
        }
        let font = self.font();
        let mut buf = harfrust::Buffer::new();
        buf.push_str(text);
        buf.guess_segment_properties();
        let shaper = harfrust::ShaperFont::new(&font.face);
        if harfrust::shape(&shaper, &mut buf, harfrust::ShapeOptions::new()).is_err() {
            return 0.0;
        }
        let upem = f64::from(font.face.units_per_em());
        let adv: i64 = buf
            .glyph_positions()
            .iter()
            .map(|p| i64::from(p.x_advance))
            .sum();
        let chars = text.chars().count() as f64;
        adv as f64 * self.size / upem + self.letter_spacing * chars
    }

    /// Ascendante et descendante (px, positives), comme Chromium (valeurs arrondies).
    pub fn metrics(&self) -> (f64, f64) {
        let font = self.font();
        let upem = f64::from(font.face.units_per_em());
        let line = font.face.metrics().hhea_line;
        let asc = line.map_or(0.0, |l| l.ascender.to_f64()) * self.size / upem;
        let desc = -line.map_or(0.0, |l| l.descender.to_f64()) * self.size / upem;
        (asc.round(), desc.round())
    }
}

/// Splits `text` into lines of width ≤ `max` (breaks at spaces, then after "-" and "/").
pub fn wrap(style: &TextStyle, text: &str, max: f64) -> Vec<String> {
    let mut lines = Vec::new();
    for para in text.split('\n') {
        let words = split_breaks(para);
        let mut cur = String::new();
        for w in words {
            let candidate = format!("{cur}{w}");
            if !cur.is_empty() && style.width(candidate.trim_end()) > max + 1e-6 {
                lines.push(cur.trim_end().to_string());
                cur = w.trim_start().to_string();
            } else {
                cur = candidate;
            }
        }
        lines.push(cur.trim_end().to_string());
    }
    lines
}

/// Unbreakable pieces, each with the space or the dash that follows it.
fn split_breaks(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let chars: Vec<char> = s.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        cur.push(c);
        let next = chars.get(i + 1).copied();
        let brk = match c {
            ' ' => next != Some(' '),
            '-' | '/' => {
                next.is_some_and(char::is_alphanumeric) && i > 0 && chars[i - 1].is_alphanumeric()
            }
            _ => false,
        };
        if brk {
            out.push(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn st(size: f64, weight: u16, fam: &str) -> TextStyle {
        TextStyle {
            families: vec![fam.into()],
            size,
            weight,
            italic: false,
            letter_spacing: 0.0,
        }
    }

    #[test]
    fn widths_like_chromium() {
        // values measured in Chromium during milestone E0
        let w = st(14.0, 650, "Inter").width("Pare-feu périmétrique");
        assert!((w - 151.328125).abs() < 0.02, "{w}");
        let w = st(14.0, 650, "Inter").width("Internet");
        assert!((w - 53.96875).abs() < 0.02, "{w}");
        let w = st(12.0, 400, "JetBrains Mono").width("203.0.113.10/29");
        assert!((w - 108.015625).abs() < 0.02, "{w}");
        let w = st(11.5, 400, "Inter").width("WAN");
        assert!((w - 27.28125).abs() < 0.02, "{w}");
    }

    #[test]
    fn wrapping_breaks_at_spaces_and_inner_dashes() {
        let s = st(14.0, 400, "Inter");
        assert_eq!(wrap(&s, "un deux", 1000.0), vec!["un deux"]);
        let w = s.width("un deux");
        assert_eq!(wrap(&s, "un deux trois", w + 1.0), vec!["un deux", "trois"]);
        // a dash between letters allows a break, a leading or doubled one does not
        assert_eq!(split_breaks("pare-feu"), vec!["pare-", "feu"]);
        assert_eq!(split_breaks("-a"), vec!["-a"]);
        assert_eq!(split_breaks("a  b"), vec!["a  ", "b"]);
        assert_eq!(split_breaks("a/b"), vec!["a/", "b"]);
        assert!(split_breaks("").is_empty());
        // newlines always break; an empty text gives one empty line; a long word is kept whole
        assert_eq!(wrap(&s, "a\nb", 1000.0), vec!["a", "b"]);
        assert_eq!(wrap(&s, "", 100.0), vec![""]);
        assert_eq!(
            wrap(&s, "supercalifragilistic", 5.0),
            vec!["supercalifragilistic"]
        );
    }

    #[test]
    fn layout_units_round_to_a_64th() {
        assert_eq!(layout_unit_floor(1.0 / 64.0 + 0.001), 1.0 / 64.0);
        assert_eq!(layout_unit_ceil(1.0 / 64.0 + 0.001), 2.0 / 64.0);
        assert_eq!(layout_unit_ceil(3.0), 3.0);
        assert_eq!(layout_unit_floor(3.0), 3.0);
    }

    #[test]
    fn weight_matching() {
        assert_eq!(select(&["Inter".into()], 650, false).file.weight, 700);
        assert_eq!(select(&["Inter".into()], 750, false).file.weight, 700);
        assert_eq!(select(&["Inter".into()], 800, false).file.weight, 800);
        assert_eq!(select(&["Inter".into()], 450, false).file.weight, 500);
        assert_eq!(
            select(&["x".into(), "monospace".into()], 400, false)
                .file
                .family,
            "JetBrains Mono"
        );
    }
}
