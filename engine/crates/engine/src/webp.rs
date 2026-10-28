//! WebP (F-RENDER-8, D-034): still image and animation, **lossless**, in pure Rust (`image-webp`),
//! hence without a C library: the same code natively, in WebAssembly and in the static musl binary.
//!
//! `image-webp` only encodes still images. The animation is a RIFF container (`VP8X`, `ANIM`, then
//! one `ANMF` per frame) whose every frame is the `VP8L` chunk of a still image: the container is
//! therefore assembled here. Each frame after the first only holds the rectangle that changed
//! (blending disabled: the rectangle exactly replaces the pixels below).
//! Deterministic: same inputs, same bytes.

use crate::animated::{GifOptions, dim_opacity, frames};
use crate::diagram::Diagram;
use crate::png::svg_to_png;
use crate::svg::{render_for_raster, render_view};
use image_webp::{ColorType, WebPEncoder};
use resvg::tiny_skia;
use skemo_lang::tr;

/// Non-premultiplied RGBA pixels of an SVG, transparency kept.
fn rgba(svg: &str, scale: f32) -> Result<(u32, u32, Vec<u8>), String> {
    let png = svg_to_png(svg, scale)?;
    let pix = tiny_skia::Pixmap::decode_png(&png).map_err(|e| format!("PNG : {e}"))?;
    let mut data = Vec::with_capacity(pix.data().len());
    for p in pix.pixels() {
        let c = p.demultiply();
        data.extend_from_slice(&[c.red(), c.green(), c.blue(), c.alpha()]);
    }
    Ok((pix.width(), pix.height(), data))
}

/// Lossless WebP file of an RGBA image.
fn encode(w: u32, h: u32, px: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    WebPEncoder::new(&mut out)
        .encode(px, w, h, ColorType::Rgba8)
        .map_err(|e| format!("WebP : {e}"))?;
    Ok(out)
}

/// Renders the diagram as a still WebP, at scale `scale`.
pub fn render(d: &Diagram, scale: f32) -> Result<Vec<u8>, String> {
    let (w, h, px) = rgba(&render_for_raster(d), scale)?;
    encode(w, h, &px)
}

/// Still WebP of state `step` (value as written, e.g. `"2.5"`).
pub fn state(d: &Diagram, step: &str, scale: f32) -> Result<Vec<u8>, String> {
    let s = d
        .anim
        .states
        .iter()
        .find(|s| s.raw == step)
        .map(|s| s.value)
        .or_else(|| step.parse().ok())
        .ok_or_else(|| tr!("unknown step « {step} »", "étape « {step} » inconnue"))?;
    let v = d.anim.view_at(&d.model, s, dim_opacity(d));
    let (w, h, px) = rgba(&render_view(d, &v, true), scale)?;
    encode(w, h, &px)
}

/// Image chunk (`VP8L`, header and padding byte included) of a simple WebP file.
fn image_chunk(file: &[u8]) -> Result<&[u8], String> {
    let bad = || {
        tr!(
            "WebP: unexpected encoder output",
            "WebP : fichier de l'encodeur inattendu"
        )
    };
    if file.len() < 20
        || &file[..4] != b"RIFF"
        || &file[8..12] != b"WEBP"
        || &file[12..16] != b"VP8L"
    {
        return Err(bad());
    }
    let size = u32::from_le_bytes([file[16], file[17], file[18], file[19]]) as usize;
    let end = 20 + size + (size & 1);
    file.get(12..end).ok_or_else(bad)
}

fn le24(v: u32) -> [u8; 3] {
    [v as u8, (v >> 8) as u8, (v >> 16) as u8]
}

fn chunk(out: &mut Vec<u8>, tag: &[u8; 4], payload: &[u8]) {
    out.extend_from_slice(tag);
    out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    out.extend_from_slice(payload);
    if payload.len() % 2 == 1 {
        out.push(0);
    }
}

/// Rectangle `(x0, y0, x1, y1)` of the RGBA pixels that differ; `x0` and `y0` are even (the WebP
/// format codes the position of a frame in units of 2 px); at least 1 × 1.
fn changed(a: &[u8], b: &[u8], w: usize, h: usize) -> (usize, usize, usize, usize) {
    let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0, 0);
    for y in 0..h {
        let row = y * w * 4..(y + 1) * w * 4;
        if a[row.clone()] == b[row] {
            continue;
        }
        y0 = y0.min(y);
        y1 = y + 1;
        for x in 0..w {
            let k = (y * w + x) * 4;
            if a[k..k + 4] != b[k..k + 4] {
                x0 = x0.min(x);
                x1 = x1.max(x + 1);
            }
        }
    }
    if x1 == 0 {
        (0, 0, 1, 1)
    } else {
        (x0 & !1, y0 & !1, x1, y1)
    }
}

/// Animated WebP: the same frames as the GIF and the APNG, in exact colours (lossless), durations
/// to the millisecond, transparency kept if the diagram has a transparent background.
pub fn animated(d: &Diagram, o: &GifOptions) -> Result<Vec<u8>, String> {
    let frames = frames(d, o);
    if frames.is_empty() {
        return Err("WebP : aucune image".into());
    }
    let mut anmf = Vec::new();
    let (mut cw, mut ch) = (0u32, 0u32);
    let mut prev: Option<Vec<u8>> = None;
    for (svg, dur) in &frames {
        let (w, h, px) = rgba(svg, o.scale)?;
        (cw, ch) = (w, h);
        let (x0, y0, x1, y1) = match &prev {
            None => (0, 0, w as usize, h as usize),
            Some(p) => changed(p, &px, w as usize, h as usize),
        };
        let mut sub = Vec::with_capacity((x1 - x0) * (y1 - y0) * 4);
        for y in y0..y1 {
            sub.extend_from_slice(&px[(y * w as usize + x0) * 4..(y * w as usize + x1) * 4]);
        }
        let file = encode((x1 - x0) as u32, (y1 - y0) as u32, &sub)?;
        let ms = (dur * 1000.0).round().clamp(1.0, 16_777_215.0) as u32;
        let mut payload = Vec::new();
        payload.extend_from_slice(&le24((x0 / 2) as u32));
        payload.extend_from_slice(&le24((y0 / 2) as u32));
        payload.extend_from_slice(&le24((x1 - x0 - 1) as u32));
        payload.extend_from_slice(&le24((y1 - y0 - 1) as u32));
        payload.extend_from_slice(&le24(ms));
        payload.push(0b10); // blending disabled, no disposal: the rectangle replaces the pixels
        payload.extend_from_slice(image_chunk(&file)?);
        chunk(&mut anmf, b"ANMF", &payload);
        prev = Some(px);
    }
    let mut body = b"WEBP".to_vec();
    let mut vp8x = vec![0b0001_0010, 0, 0, 0]; // animation + alpha
    vp8x.extend_from_slice(&le24(cw - 1));
    vp8x.extend_from_slice(&le24(ch - 1));
    chunk(&mut body, b"VP8X", &vp8x);
    let mut anim = vec![0, 0, 0, 0]; // fond : noir transparent
    anim.extend_from_slice(&(if o.looping { 0u16 } else { 1u16 }).to_le_bytes());
    chunk(&mut body, b"ANIM", &anim);
    body.extend_from_slice(&anmf);
    let mut out = b"RIFF".to_vec();
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend_from_slice(&body);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changed_rectangle_starts_on_even_coordinates() {
        let (w, h) = (6, 4);
        let a = vec![0u8; w * h * 4];
        let mut b = a.clone();
        b[(2 * w + 3) * 4] = 9; // pixel (3, 2)
        assert_eq!(changed(&a, &b, w, h), (2, 2, 4, 3));
        assert_eq!(changed(&a, &a, w, h), (0, 0, 1, 1));
    }

    #[test]
    fn le24_is_little_endian() {
        assert_eq!(le24(0x0003_0201), [1, 2, 3]);
    }

    #[test]
    fn chunks_are_padded_to_even_length() {
        let mut v = Vec::new();
        chunk(&mut v, b"TEST", &[1, 2, 3]);
        assert_eq!(v, [b'T', b'E', b'S', b'T', 3, 0, 0, 0, 1, 2, 3, 0]);
    }
}
