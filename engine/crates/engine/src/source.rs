//! Source embedded in the rendered files (D-042), as draw.io does: the image carries its `.skemo`
//! text and stylesheets, so it can be reopened and edited later (`skemo extract`, or any command
//! given the image instead of the source).
//!
//! Where the bundle (JSON) goes:
//! - PNG and APNG: an `iTXt` chunk with the keyword `skemo`, right after `IHDR`;
//! - GIF: a comment extension starting with `skemo:`, before the trailer;
//! - WebP: a `SKMO` chunk (the file is turned into the extended format if needed);
//! - SVG: `<metadata><skemo-source>` (XML-escaped JSON) right after the opening tag;
//! - HTML: `<script type="application/json" id="skemo-source">` before `</body>`.
//!
//! Viewers ignore all of these. Nothing is rendered differently.

use serde::{Deserialize, Serialize};

/// A named text file (stylesheet).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NamedText {
    /// File name only, never a path.
    pub name: String,
    pub content: String,
}

/// An image file used as an icon (D-046), kept as base64 in the JSON.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NamedBytes {
    /// The value written in the source (`img/logo.svg`): a relative path, checked with
    /// `images::check_name` before any use as a file name.
    pub name: String,
    pub base64: String,
}

impl NamedBytes {
    pub fn new(name: &str, data: &[u8]) -> NamedBytes {
        NamedBytes {
            name: name.to_string(),
            base64: crate::html::base64(data),
        }
    }

    /// The file's bytes (`None` if the base64 is damaged).
    pub fn bytes(&self) -> Option<Vec<u8>> {
        crate::images::unbase64(&self.base64)
    }
}

/// What it takes to render the image again.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Bundle {
    /// Version of the bundle format.
    pub skemo: u32,
    /// Name of the source file (`diagram.skemo`), if known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub source: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub css: Vec<NamedText>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_theme: Option<NamedText>,
    /// Rendering options given on the command line (`theme`, `font-scale`…).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_scale: Option<f64>,
    /// Image files used as icons (D-046).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub images: Vec<NamedBytes>,
}

pub const VERSION: u32 = 1;
const KEYWORD: &[u8] = b"skemo";
const GIF_MARK: &[u8] = b"skemo:";
const WEBP_CHUNK: &[u8; 4] = b"SKMO";
const SVG_OPEN: &str = "<skemo-source xmlns=\"https://skemo.net/source\">";
const SVG_CLOSE: &str = "</skemo-source>";
const HTML_OPEN: &str = "<script type=\"application/json\" id=\"skemo-source\">";

impl Bundle {
    pub fn new(source: &str) -> Bundle {
        Bundle {
            skemo: VERSION,
            source: source.to_string(),
            ..Bundle::default()
        }
    }

    fn json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
}

/// File name without any directory part; `None` if nothing usable is left (`..`, empty).
pub fn safe_name(name: &str) -> Option<String> {
    let base = name.rsplit(['/', '\\']).next()?.trim();
    let ok = !base.is_empty()
        && base != "."
        && base != ".."
        && !base.starts_with('.')
        && !base.contains(['\0', ':']);
    ok.then(|| base.to_string())
}

/// The rendered file with the bundle inside; the format is recognised from its content.
/// An unknown format is returned unchanged.
pub fn embed(bytes: Vec<u8>, b: &Bundle) -> Vec<u8> {
    if bytes.starts_with(PNG_SIG) {
        embed_png(&bytes, b)
    } else if bytes.starts_with(b"GIF8") {
        embed_gif(&bytes, b).unwrap_or(bytes)
    } else if is_webp(&bytes) {
        embed_webp(&bytes, b).unwrap_or(bytes)
    } else {
        match String::from_utf8(bytes) {
            Ok(s) => embed_text(&s, b).into_bytes(),
            Err(e) => e.into_bytes(),
        }
    }
}

/// SVG or HTML text with the bundle inside (other text unchanged).
pub fn embed_text(text: &str, b: &Bundle) -> String {
    let head = text.trim_start();
    if head.starts_with("<svg") {
        let Some(end) = text.find('>') else {
            return text.to_string();
        };
        let meta = format!(
            "<metadata>{SVG_OPEN}{}{SVG_CLOSE}</metadata>",
            crate::svg::esc(&b.json())
        );
        format!("{}{meta}{}", &text[..=end], &text[end + 1..])
    } else if head
        .get(..9)
        .is_some_and(|h| h.eq_ignore_ascii_case("<!doctype"))
    {
        let Some(at) = text.rfind("</body>") else {
            return text.to_string();
        };
        // `<` escaped inside the JSON strings: the script can never be closed early
        let json = b.json().replace('<', "\\u003c");
        format!("{}{HTML_OPEN}{json}</script>\n{}", &text[..at], &text[at..])
    } else {
        text.to_string()
    }
}

/// The bundle carried by a rendered file, whatever its format.
pub fn extract(bytes: &[u8]) -> Option<Bundle> {
    let json = if bytes.starts_with(PNG_SIG) {
        extract_png(bytes)?
    } else if bytes.starts_with(b"GIF8") {
        extract_gif(bytes)?
    } else if is_webp(bytes) {
        extract_webp(bytes)?
    } else {
        extract_text(std::str::from_utf8(bytes).ok()?)?
    };
    let b: Bundle = serde_json::from_str(&json).ok()?;
    (b.skemo >= 1).then_some(b)
}

fn extract_text(text: &str) -> Option<String> {
    if let Some(start) = text.find(SVG_OPEN) {
        let rest = &text[start + SVG_OPEN.len()..];
        let raw = &rest[..rest.find(SVG_CLOSE)?];
        return Some(unescape_xml(raw));
    }
    let start = text.find(HTML_OPEN)?;
    let rest = &text[start + HTML_OPEN.len()..];
    Some(rest[..rest.find("</script>")?].to_string())
}

fn unescape_xml(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

// ---------- PNG ----------

const PNG_SIG: &[u8] = b"\x89PNG\r\n\x1a\n";

fn crc32(parts: &[&[u8]]) -> u32 {
    let mut c: u32 = 0xFFFF_FFFF;
    for p in parts {
        for &byte in *p {
            c ^= u32::from(byte);
            for _ in 0..8 {
                c = if c & 1 == 1 {
                    0xEDB8_8320 ^ (c >> 1)
                } else {
                    c >> 1
                };
            }
        }
    }
    !c
}

/// Chunks of a PNG: (type, data, start of the chunk, end of the chunk).
fn png_chunks(bytes: &[u8]) -> Vec<([u8; 4], &[u8], usize, usize)> {
    let mut out = Vec::new();
    let mut i = PNG_SIG.len();
    while i + 12 <= bytes.len() {
        let len = u32::from_be_bytes([bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3]]) as usize;
        let Some(end) = (i + 12).checked_add(len).filter(|e| *e <= bytes.len()) else {
            break;
        };
        let kind = [bytes[i + 4], bytes[i + 5], bytes[i + 6], bytes[i + 7]];
        out.push((kind, &bytes[i + 8..i + 8 + len], i, end));
        i = end;
    }
    out
}

fn embed_png(bytes: &[u8], b: &Bundle) -> Vec<u8> {
    let chunks = png_chunks(bytes);
    let Some(&(_, _, _, after_ihdr)) = chunks.iter().find(|c| &c.0 == b"IHDR") else {
        return bytes.to_vec();
    };
    // iTXt: keyword, 0, compression flag 0, method 0, language "", 0, translated "", 0, text
    let mut data = KEYWORD.to_vec();
    data.extend_from_slice(&[0, 0, 0, 0, 0]);
    data.extend_from_slice(b.json().as_bytes());
    let mut out = Vec::with_capacity(bytes.len() + data.len() + 12);
    out.extend_from_slice(&bytes[..after_ihdr]);
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(b"iTXt");
    out.extend_from_slice(&data);
    out.extend_from_slice(&crc32(&[b"iTXt", &data]).to_be_bytes());
    out.extend_from_slice(&bytes[after_ihdr..]);
    out
}

fn extract_png(bytes: &[u8]) -> Option<String> {
    for (kind, data, _, _) in png_chunks(bytes) {
        if &kind != b"iTXt" || !data.starts_with(KEYWORD) || data.get(KEYWORD.len()) != Some(&0) {
            continue;
        }
        let rest = &data[KEYWORD.len() + 1..];
        if rest.first() != Some(&0) {
            continue; // compressed: never written by skemo
        }
        // skip compression method, then language and translated keyword (both 0-terminated)
        let rest = rest.get(2..)?;
        let lang_end = rest.iter().position(|&c| c == 0)?;
        let rest = &rest[lang_end + 1..];
        let tr_end = rest.iter().position(|&c| c == 0)?;
        return String::from_utf8(rest[tr_end + 1..].to_vec()).ok();
    }
    None
}

// ---------- GIF ----------

/// Skips the data sub-blocks starting at `i`; returns the index after the terminator.
fn gif_skip_blocks(bytes: &[u8], mut i: usize) -> Option<usize> {
    loop {
        let n = *bytes.get(i)? as usize;
        i += 1;
        if n == 0 {
            return Some(i);
        }
        i += n;
    }
}

/// Positions of the blocks of a GIF: (introducer, start). The last one is the trailer.
fn gif_blocks(bytes: &[u8]) -> Option<Vec<(u8, usize)>> {
    let flags = *bytes.get(10)?;
    let mut i = 13;
    if flags & 0x80 != 0 {
        i += 3 << ((flags & 7) + 1);
    }
    let mut out = Vec::new();
    loop {
        let intro = *bytes.get(i)?;
        out.push((intro, i));
        match intro {
            0x3B => return Some(out),
            0x21 => i = gif_skip_blocks(bytes, i + 2)?,
            0x2C => {
                let f = *bytes.get(i + 9)?;
                i += 10;
                if f & 0x80 != 0 {
                    i += 3 << ((f & 7) + 1);
                }
                i = gif_skip_blocks(bytes, i + 1)?;
            }
            _ => return None,
        }
    }
}

fn embed_gif(bytes: &[u8], b: &Bundle) -> Option<Vec<u8>> {
    let blocks = gif_blocks(bytes)?;
    let &(_, trailer) = blocks.last()?;
    let mut text = GIF_MARK.to_vec();
    text.extend_from_slice(b.json().as_bytes());
    let mut out = bytes[..trailer].to_vec();
    out.extend_from_slice(&[0x21, 0xFE]);
    for piece in text.chunks(255) {
        out.push(piece.len() as u8);
        out.extend_from_slice(piece);
    }
    out.push(0);
    out.extend_from_slice(&bytes[trailer..]);
    Some(out)
}

fn extract_gif(bytes: &[u8]) -> Option<String> {
    for (intro, at) in gif_blocks(bytes)? {
        if intro != 0x21 || bytes.get(at + 1) != Some(&0xFE) {
            continue;
        }
        let mut text = Vec::new();
        let mut i = at + 2;
        loop {
            let n = *bytes.get(i)? as usize;
            if n == 0 {
                break;
            }
            text.extend_from_slice(bytes.get(i + 1..i + 1 + n)?);
            i += 1 + n;
        }
        if let Some(json) = text.strip_prefix(GIF_MARK) {
            return String::from_utf8(json.to_vec()).ok();
        }
    }
    None
}

// ---------- WebP ----------

fn is_webp(bytes: &[u8]) -> bool {
    bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP"
}

/// Chunks of a WebP: (fourcc, data, start, end with padding).
fn webp_chunks(bytes: &[u8]) -> Vec<([u8; 4], &[u8], usize, usize)> {
    let mut out = Vec::new();
    let mut i = 12;
    while i + 8 <= bytes.len() {
        let len =
            u32::from_le_bytes([bytes[i + 4], bytes[i + 5], bytes[i + 6], bytes[i + 7]]) as usize;
        let Some(data_end) = (i + 8).checked_add(len).filter(|e| *e <= bytes.len()) else {
            break;
        };
        let kind = [bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3]];
        let end = (data_end + (len & 1)).min(bytes.len());
        out.push((kind, &bytes[i + 8..data_end], i, end));
        i = end;
    }
    out
}

fn webp_chunk(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut c = kind.to_vec();
    c.extend_from_slice(&(data.len() as u32).to_le_bytes());
    c.extend_from_slice(data);
    if data.len() % 2 == 1 {
        c.push(0);
    }
    c
}

/// `VP8X` header for a simple lossless file: canvas size and alpha flag from the `VP8L` header.
fn vp8x_for(vp8l: &[u8]) -> Option<Vec<u8>> {
    if vp8l.first() != Some(&0x2F) || vp8l.len() < 5 {
        return None;
    }
    let bits = u32::from_le_bytes([vp8l[1], vp8l[2], vp8l[3], vp8l[4]]);
    let w = (bits & 0x3FFF) + 1;
    let h = ((bits >> 14) & 0x3FFF) + 1;
    let alpha = (bits >> 28) & 1 == 1;
    let mut d = vec![if alpha { 0x10 } else { 0 }, 0, 0, 0];
    d.extend_from_slice(&(w - 1).to_le_bytes()[..3]);
    d.extend_from_slice(&(h - 1).to_le_bytes()[..3]);
    Some(webp_chunk(b"VP8X", &d))
}

fn embed_webp(bytes: &[u8], b: &Bundle) -> Option<Vec<u8>> {
    let chunks = webp_chunks(bytes);
    let first = chunks.first()?;
    let mut body = Vec::new();
    match &first.0 {
        b"VP8X" => {}
        b"VP8L" => body.extend(vp8x_for(first.1)?),
        _ => return None, // lossy VP8: never written by skemo
    }
    body.extend_from_slice(&bytes[12..chunks.last()?.3]);
    body.extend(webp_chunk(WEBP_CHUNK, b.json().as_bytes()));
    let mut out = b"RIFF".to_vec();
    out.extend_from_slice(&((body.len() + 4) as u32).to_le_bytes());
    out.extend_from_slice(b"WEBP");
    out.extend(body);
    Some(out)
}

fn extract_webp(bytes: &[u8]) -> Option<String> {
    webp_chunks(bytes)
        .into_iter()
        .find(|c| &c.0 == WEBP_CHUNK)
        .and_then(|c| String::from_utf8(c.1.to_vec()).ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bundle() -> Bundle {
        let mut b = Bundle::new("card#a A <b> & \"c\" </script>\n  li x\n");
        b.css.push(NamedText {
            name: "custom.css".into(),
            content: ".t1 { --c: #C94A44; }".into(),
        });
        b.font_scale = Some(1.4);
        b
    }

    #[test]
    fn text_formats_round_trip() {
        let b = bundle();
        let svg = embed_text("<svg xmlns=\"x\"><g/></svg>\n", &b);
        assert!(svg.starts_with("<svg xmlns=\"x\"><metadata>"));
        assert_eq!(extract(svg.as_bytes()), Some(b.clone()));
        let html = embed_text("<!doctype html>\n<html><body>x</body></html>\n", &b);
        assert!(!html.contains("</script>\\n  li"), "no early close");
        assert_eq!(html.matches("</script>").count(), 1);
        assert_eq!(extract(html.as_bytes()), Some(b.clone()));
        assert_eq!(embed_text("plain", &b), "plain");
        assert_eq!(extract(b"plain"), None);
    }

    #[test]
    fn crc_matches_the_png_reference() {
        // CRC of the IEND chunk type, as in every PNG
        assert_eq!(crc32(&[b"IEND"]), 0xAE42_6082);
    }

    #[test]
    fn safe_names_drop_directories() {
        assert_eq!(safe_name("../../etc/passwd").as_deref(), Some("passwd"));
        assert_eq!(safe_name("a\\b\\c.css").as_deref(), Some("c.css"));
        assert_eq!(safe_name(".."), None);
        assert_eq!(safe_name(".hidden"), None);
        assert_eq!(safe_name(""), None);
    }

    #[test]
    fn garbage_never_panics() {
        for data in [
            &b"\x89PNG\r\n\x1a\n\0\0\0\x05IHDR"[..],
            b"GIF89a\x01\0\x01\0\xff",
            b"RIFF\x04\0\0\0WEBPVP8L",
            b"<svg",
            b"<!doctype html>",
        ] {
            let _ = extract(data);
            let _ = embed(data.to_vec(), &bundle());
        }
    }
}
