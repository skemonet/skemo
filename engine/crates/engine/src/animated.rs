//! Animated outputs (F-ANIM-5, 6, 8): one PNG per state (no effects, with the focus) and an
//! animated GIF (with effects). Deterministic: same input → same bytes.

use crate::diagram::Diagram;
use crate::png::svg_to_png;
use crate::svg::render_view;
use resvg::tiny_skia;
use serde_json::{Value, json};
use skemo_lang::tr;

/// Opacity of the elements dimmed during a focus (the theme's `--dim-opacity`, default 0.28).
pub fn dim_opacity(d: &Diagram) -> f64 {
    d.dim_opacity()
}

/// PNG of state `step` (value as written, e.g. `"2.5"`).
pub fn state_png(d: &Diagram, step: &str, scale: f32) -> Result<Vec<u8>, String> {
    let s = d
        .anim
        .states
        .iter()
        .find(|s| s.raw == step)
        .map(|s| s.value)
        .or_else(|| step.parse().ok())
        .ok_or_else(|| tr!("unknown step « {step} »", "étape « {step} » inconnue"))?;
    let v = d.anim.view_at(&d.model, s, dim_opacity(d));
    svg_to_png(&render_view(d, &v, true), scale)
}

/// One PNG per state, in order, and the index `states.json`.
pub fn states_png(d: &Diagram, scale: f32) -> Result<(Vec<Vec<u8>>, Value), String> {
    let dim = dim_opacity(d);
    let mut pngs = Vec::new();
    let mut index = Vec::new();
    let states: Vec<(String, f64)> = if d.anim.states.is_empty() {
        vec![("0".into(), 0.0)]
    } else {
        d.anim
            .states
            .iter()
            .map(|s| (s.raw.clone(), s.value))
            .collect()
    };
    for (k, (raw, s)) in states.iter().enumerate() {
        let v = d.anim.view_at(&d.model, *s, dim);
        pngs.push(svg_to_png(&render_view(d, &v, true), scale)?);
        let visible: Vec<&String> = d
            .anim
            .elements
            .keys()
            .filter(|e| d.anim.visible(e, *s))
            .collect();
        let focus: Vec<&String> = d
            .anim
            .elements
            .keys()
            .filter(|e| d.anim.focused(e, *s))
            .collect();
        index.push(json!({ "index": k, "step": raw, "visible": visible, "focus": focus }));
    }
    Ok((pngs, json!({ "states": index })))
}

/// GIF parameters (F-ANIM-6).
#[derive(Debug, Clone)]
pub struct GifOptions {
    /// Hold of each state (s).
    pub hold: f64,
    /// Duration of a transition (s); `draw_transition` if a link draws itself.
    pub transition: f64,
    pub draw_transition: f64,
    pub fps: u32,
    pub looping: bool,
    /// Hold of the last frame (s).
    pub final_hold: f64,
    pub scale: f32,
}

impl Default for GifOptions {
    fn default() -> Self {
        GifOptions {
            hold: 1.6,
            transition: 0.45,
            draw_transition: 0.6,
            fps: 20,
            looping: true,
            final_hold: 3.0,
            scale: 1.0,
        }
    }
}

fn rgba(svg: &str, scale: f32) -> Result<(u16, u16, Vec<u8>), String> {
    let png = svg_to_png(svg, scale)?;
    let pix = tiny_skia::Pixmap::decode_png(&png).map_err(|e| format!("PNG : {e}"))?;
    // non-premultiplied pixels for the GIF (opaque background: identical)
    let mut data = Vec::with_capacity(pix.data().len());
    for p in pix.pixels() {
        let c = p.demultiply();
        data.extend_from_slice(&[c.red(), c.green(), c.blue(), 255]);
    }
    Ok((pix.width() as u16, pix.height() as u16, data))
}

/// Frames of the animation: (SVG, duration in s), states chained with their effects (F-ANIM-6).
pub fn frames(d: &Diagram, o: &GifOptions) -> Vec<(String, f64)> {
    let dim = dim_opacity(d);
    let states: Vec<f64> = if d.anim.states.is_empty() {
        vec![0.0]
    } else {
        d.anim.states.iter().map(|s| s.value).collect()
    };
    let mut out = Vec::new();
    for (k, &s) in states.iter().enumerate() {
        let last = k + 1 == states.len();
        out.push((
            render_view(d, &d.anim.view_at(&d.model, s, dim), true),
            if last { o.final_hold } else { o.hold },
        ));
        if last {
            break;
        }
        let s1 = states[k + 1];
        let draws = d
            .anim
            .elements
            .iter()
            .any(|(e, r)| r.fx == "draw" && d.anim.visible(e, s) != d.anim.visible(e, s1));
        let dur = if draws {
            o.draw_transition
        } else {
            o.transition
        };
        let n = ((dur * f64::from(o.fps)).round() as usize).max(1);
        for i in 1..n {
            let t = i as f64 / n as f64;
            out.push((
                render_view(d, &d.anim.view_between(&d.model, s, s1, t, dim), true),
                1.0 / f64::from(o.fps),
            ));
        }
    }
    out
}

/// Animated GIF: states chained with their effects.
pub fn gif(d: &Diagram, o: &GifOptions) -> Result<Vec<u8>, String> {
    let frames = frames(d, o);
    let mut out = Vec::new();
    {
        let Some((first, _)) = frames.first() else {
            return Err("GIF : aucune image".into());
        };
        let (w, h, _) = rgba(first, o.scale)?;
        let mut enc = gif::Encoder::new(&mut out, w, h, &[]).map_err(|e| format!("GIF : {e}"))?;
        if o.looping {
            enc.set_repeat(gif::Repeat::Infinite)
                .map_err(|e| format!("GIF : {e}"))?;
        }
        for (svg, dur) in &frames {
            let (w, h, mut px) = rgba(svg, o.scale)?;
            let mut f = gif::Frame::from_rgba_speed(w, h, &mut px, 10);
            f.delay = ((dur * 100.0).round() as u16).max(2);
            enc.write_frame(&f).map_err(|e| format!("GIF : {e}"))?;
        }
    }
    Ok(out)
}

/// Rectangle (x0, y0, x1, y1) of the pixels that differ between two RGB frames; at least 1 × 1.
fn changed(a: &[u8], b: &[u8], w: usize, h: usize) -> (usize, usize, usize, usize) {
    let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0, 0);
    for y in 0..h {
        let row = y * w * 3..(y + 1) * w * 3;
        if a[row.clone()] == b[row.clone()] {
            continue;
        }
        y0 = y0.min(y);
        y1 = y + 1;
        for x in 0..w {
            let k = (y * w + x) * 3;
            if a[k..k + 3] != b[k..k + 3] {
                x0 = x0.min(x);
                x1 = x1.max(x + 1);
            }
        }
    }
    if x1 == 0 {
        (0, 0, 1, 1)
    } else {
        (x0, y0, x1, y1)
    }
}

/// Animated PNG (APNG, F-ANIM-10): the same frames as the GIF, in exact colours (no 256-colour
/// palette) and durations to the millisecond.
pub fn apng(d: &Diagram, o: &GifOptions) -> Result<Vec<u8>, String> {
    let frames = frames(d, o);
    let Some((first, _)) = frames.first() else {
        return Err("APNG : aucune image".into());
    };
    let err = |e: png::EncodingError| format!("APNG : {e}");
    let (w, h, _) = rgba(first, o.scale)?;
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, u32::from(w), u32::from(h));
        enc.set_color(png::ColorType::Rgb);
        enc.set_depth(png::BitDepth::Eight);
        enc.set_animated(frames.len() as u32, if o.looping { 0 } else { 1 })
            .map_err(err)?;
        let mut wr = enc.write_header().map_err(err)?;
        // each frame after the first: only the rectangle that changed
        let (w, h) = (usize::from(w), usize::from(h));
        let mut prev: Option<Vec<u8>> = None;
        for (svg, dur) in &frames {
            let (_, _, px) = rgba(svg, o.scale)?;
            let rgb: Vec<u8> = px
                .as_chunks::<4>()
                .0
                .iter()
                .flat_map(|c| [c[0], c[1], c[2]])
                .collect();
            let (x0, y0, x1, y1) = match &prev {
                None => (0, 0, w, h),
                Some(p) => changed(p, &rgb, w, h),
            };
            let ms = (dur * 1000.0).round().clamp(1.0, 65535.0) as u16;
            wr.set_frame_delay(ms, 1000).map_err(err)?;
            if prev.is_some() {
                // position reset first: the encoder checks the bounds on each call
                wr.set_frame_position(0, 0).map_err(err)?;
                wr.set_frame_dimension((x1 - x0) as u32, (y1 - y0) as u32)
                    .map_err(err)?;
                wr.set_frame_position(x0 as u32, y0 as u32).map_err(err)?;
            }
            let mut sub = Vec::with_capacity((x1 - x0) * (y1 - y0) * 3);
            for y in y0..y1 {
                sub.extend_from_slice(&rgb[(y * w + x0) * 3..(y * w + x1) * 3]);
            }
            wr.write_image_data(&sub).map_err(err)?;
            prev = Some(rgb);
        }
        wr.finish().map_err(err)?;
    }
    Ok(out)
}

/// Contact sheet of the steps: one state per row, top to bottom, 20 px apart (as the prototype's
/// `tools/steps.js`). No effects, with the focus.
pub fn board(d: &Diagram, scale: f32) -> Result<Vec<u8>, String> {
    let dim = dim_opacity(d);
    let states: Vec<f64> = if d.anim.states.is_empty() {
        vec![0.0]
    } else {
        d.anim.states.iter().map(|s| s.value).collect()
    };
    let mut shots = Vec::new();
    for s in &states {
        let png = svg_to_png(
            &render_view(d, &d.anim.view_at(&d.model, *s, dim), true),
            scale,
        )?;
        shots.push(tiny_skia::Pixmap::decode_png(&png).map_err(|e| format!("PNG : {e}"))?);
    }
    let gap = (20.0 * scale).round() as u32;
    let w = shots
        .iter()
        .map(resvg::tiny_skia::Pixmap::width)
        .max()
        .unwrap_or(1);
    let h = shots
        .iter()
        .map(resvg::tiny_skia::Pixmap::height)
        .sum::<u32>()
        + gap * (shots.len() as u32).saturating_sub(1);
    let mut out = tiny_skia::Pixmap::new(w, h.max(1)).ok_or("planche trop grande")?;
    let stage = d.scene.style(d.scene.stage);
    let bg = stage
        .var_color("--bg")
        .unwrap_or(skemo_lang::style::Rgba::WHITE);
    let c = |v: f64| (v.round().clamp(0.0, 255.0)) as u8;
    if !d.transparent {
        out.fill(tiny_skia::Color::from_rgba8(c(bg.r), c(bg.g), c(bg.b), 255));
    }
    let mut y = 0i32;
    for p in &shots {
        out.draw_pixmap(
            0,
            y,
            p.as_ref(),
            &tiny_skia::PixmapPaint::default(),
            tiny_skia::Transform::identity(),
            None,
        );
        y += p.height() as i32 + gap as i32;
    }
    out.encode_png().map_err(|e| format!("encodage PNG : {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The APNG put back together (changed rectangles laid on the previous frame) gives each rendered
    /// frame back exactly.
    #[test]
    fn apng_roundtrip() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/animation.pug");
        let src = std::fs::read_to_string(path).unwrap();
        let (d, _) = crate::diagram::from_source(&src, &Default::default()).unwrap();
        let o = GifOptions {
            fps: 4,
            ..GifOptions::default()
        };
        let bytes = apng(&d, &o).unwrap();
        let want: Vec<Vec<u8>> = frames(&d, &o)
            .iter()
            .map(|(svg, _)| {
                let (_, _, px) = rgba(svg, o.scale).unwrap();
                px.as_chunks::<4>()
                    .0
                    .iter()
                    .flat_map(|c| [c[0], c[1], c[2]])
                    .collect()
            })
            .collect();
        let mut r = png::Decoder::new(std::io::Cursor::new(bytes))
            .read_info()
            .unwrap();
        let w = r.info().width as usize;
        let mut canvas = vec![0u8; want[0].len()];
        let mut buf = vec![0u8; r.output_buffer_size().unwrap()];
        for (k, exp) in want.iter().enumerate() {
            let out = r.next_frame(&mut buf).unwrap();
            let fc = r.info().frame_control.unwrap();
            let (fx, fy) = (fc.x_offset as usize, fc.y_offset as usize);
            let fw = out.width as usize;
            for y in 0..out.height as usize {
                let dst = ((fy + y) * w + fx) * 3;
                canvas[dst..dst + fw * 3].copy_from_slice(&buf[y * fw * 3..(y + 1) * fw * 3]);
            }
            assert!(canvas == *exp, "frame {k} differs after recomposition");
        }
    }
}
