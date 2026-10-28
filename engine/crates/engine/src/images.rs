//! Image files used as icons: `icon="img/logo.svg"` (D-046). The engine reads no file: the
//! caller (the CLI, an editor, a program using the library) loads the files named in the source
//! and hands their bytes over in `Options::images`, keyed by the value written in the source.
//! This module recognises the formats, checks the names and lists what a diagram refers to.
//!
//! Formats: SVG, PNG, JPEG and WebP, recognised from their content, at most `MAX_BYTES`. An image
//! keeps its own colours. It is drawn in the icon's square box, scaled to fit (aspect kept), and
//! only ever as an image: an SVG is never inlined into the output, so its scripts or links cannot
//! run, and the PNG renderer resolves nothing but `data:` URLs.

use skemo_lang::{Model, tr};

/// Largest image file accepted.
pub const MAX_BYTES: usize = 1 << 20;

/// File extensions that make an `icon=` value a file name rather than a built-in icon.
const EXTENSIONS: [&str; 5] = [".svg", ".png", ".jpg", ".jpeg", ".webp"];

/// A decoded image, ready to be drawn.
#[derive(Debug, Clone, PartialEq)]
pub struct Image {
    /// The value written in the source (`img/logo.svg`).
    pub name: String,
    pub mime: &'static str,
    pub data: Vec<u8>,
}

impl Image {
    /// `data:` URL of the image.
    pub fn data_url(&self) -> String {
        format!(
            "data:{};base64,{}",
            self.mime,
            crate::html::base64(&self.data)
        )
    }
}

/// Is this `icon=` value an image file (by its extension)?
pub fn is_file(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    EXTENSIONS.iter().any(|e| lower.ends_with(e))
}

/// Checks a file name written in a source: a relative path that stays inside the source's
/// folder (no `..`, no absolute path, no URL scheme). Returns the reason it is refused.
pub fn check_name(name: &str) -> Result<(), String> {
    let refused = |why: String| Err(tr!("image « {name} »: {why}", "image « {name} » : {why}"));
    if name.contains("://") || name.starts_with("data:") {
        return refused(tr!(
            "only local files are accepted (rendering stays offline)",
            "seuls les fichiers locaux sont acceptés (le rendu reste hors ligne)"
        ));
    }
    let abs = name.starts_with(['/', '\\']) || name.as_bytes().get(1) == Some(&b':');
    if abs || name.split(['/', '\\']).any(|p| p == "..") || name.contains('\0') {
        return refused(tr!(
            "a path relative to the source file, inside its folder, is expected",
            "un chemin relatif au fichier source, dans son dossier, est attendu"
        ));
    }
    Ok(())
}

/// Recognises the format of `data` and checks its size.
pub fn decode(name: &str, data: Vec<u8>) -> Result<Image, String> {
    if data.len() > MAX_BYTES {
        return Err(tr!(
            "image « {name} »: larger than 1 MB",
            "image « {name} » : plus de 1 Mo"
        ));
    }
    let mime = if data.starts_with(b"\x89PNG\r\n\x1a\n") {
        "image/png"
    } else if data.starts_with(&[0xFF, 0xD8, 0xFF]) {
        "image/jpeg"
    } else if data.len() > 12 && &data[..4] == b"RIFF" && &data[8..12] == b"WEBP" {
        "image/webp"
    } else if looks_like_svg(&data) {
        "image/svg+xml"
    } else {
        return Err(tr!(
            "image « {name} »: not an SVG, PNG, JPEG or WebP file",
            "image « {name} » : ni SVG, ni PNG, ni JPEG, ni WebP"
        ));
    };
    Ok(Image {
        name: name.to_string(),
        mime,
        data,
    })
}

/// Decodes standard base64 (padding optional); `None` on any other character.
pub fn unbase64(s: &str) -> Option<Vec<u8>> {
    let val = |c: u8| match c {
        b'A'..=b'Z' => Some(c - b'A'),
        b'a'..=b'z' => Some(c - b'a' + 26),
        b'0'..=b'9' => Some(c - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    };
    let digits: Vec<u8> = s
        .trim_end_matches('=')
        .bytes()
        .map(val)
        .collect::<Option<_>>()?;
    let mut out = Vec::with_capacity(digits.len() * 3 / 4);
    for c in digits.chunks(4) {
        let mut v = 0u32;
        for (k, d) in c.iter().enumerate() {
            v |= u32::from(*d) << (18 - 6 * k);
        }
        let n = c.len().saturating_sub(1);
        out.extend(v.to_be_bytes()[1..=n].iter().copied());
    }
    Some(out)
}

fn looks_like_svg(data: &[u8]) -> bool {
    let head = &data[..data.len().min(4096)];
    std::str::from_utf8(head).is_ok_and(|s| s.contains("<svg"))
}

/// Image files a diagram refers to: `icon=` values, and `--icon` values in the stylesheets,
/// in order of first appearance, without duplicates.
pub fn referenced(model: &Model, css: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut add = |v: &str| {
        if is_file(v) && !out.iter().any(|o| o == v) {
            out.push(v.to_string());
        }
    };
    for n in &model.nodes {
        if let Some(i) = &n.icon {
            add(i);
        }
    }
    for sheet in css {
        for part in sheet.split("--icon").skip(1) {
            let Some(rest) = part.trim_start().strip_prefix(':') else {
                continue;
            };
            let value = rest.split([';', '}']).next().unwrap_or("").trim();
            add(value.trim_matches(['"', '\'']));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_formats() {
        assert!(is_file("img/Logo.SVG") && is_file("a.jpeg") && !is_file("database"));
        assert!(check_name("img/docker.svg").is_ok());
        for bad in [
            "../x.svg",
            "/etc/x.png",
            "C:\\x.png",
            "https://x/y.svg",
            "a/../../b.png",
        ] {
            assert!(check_name(bad).is_err(), "{bad}");
        }
        let png = decode("a.png", b"\x89PNG\r\n\x1a\n....".to_vec()).unwrap();
        assert_eq!(png.mime, "image/png");
        let svg = decode("a.svg", b"<?xml version=\"1.0\"?><svg/>".to_vec()).unwrap();
        assert_eq!(svg.mime, "image/svg+xml");
        assert!(decode("a.png", b"GIF89a".to_vec()).is_err());
        assert!(decode("a.svg", vec![b' '; MAX_BYTES + 1]).is_err());
        for len in 0..8 {
            let data: Vec<u8> = (0..len).map(|i| (i * 37 + 5) as u8).collect();
            assert_eq!(unbase64(&crate::html::base64(&data)), Some(data));
        }
        assert_eq!(unbase64("a$b"), None);
    }

    #[test]
    fn references_in_source_and_css() {
        let doc = skemo_lang::parse(
            "card#a(icon=\"img/a.svg\") A\ncard#b(icon=\"database\") B\ncard#c(icon=\"img/a.svg\") C\n",
        );
        let css = vec![".k8s { --icon: \"img/k8s.png\"; } .db { --icon: database }".to_string()];
        assert_eq!(
            referenced(&doc.model, &css),
            vec!["img/a.svg", "img/k8s.png"]
        );
    }
}
