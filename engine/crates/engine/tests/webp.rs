//! WebP (D-034): still image and animation, read back with `image-webp`'s decoder and compared
//! pixel by pixel with the images the engine rasterised.

use image_webp::WebPDecoder;
use resvg::tiny_skia::Pixmap;
use skemo_engine::animated::{GifOptions, frames};
use skemo_engine::png::svg_to_png;
use skemo_engine::{Options, from_source, webp};
use std::io::Cursor;
use std::path::Path;

fn fixture(name: &str) -> String {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    std::fs::read_to_string(dir.join(name)).unwrap()
}

/// Non-premultiplied RGBA pixels of an SVG, as the engine writes them.
fn rgba(svg: &str, scale: f32) -> Vec<u8> {
    let pix = Pixmap::decode_png(&svg_to_png(svg, scale).unwrap()).unwrap();
    pix.pixels()
        .iter()
        .flat_map(|p| {
            let c = p.demultiply();
            [c.red(), c.green(), c.blue(), c.alpha()]
        })
        .collect()
}

#[test]
fn still_webp_roundtrips_and_keeps_transparency() {
    let src = "diagram(dir=\"down\")\n  card#a One\n    to(b)\n  card#b Two\n";
    for transparent in [false, true] {
        let (d, _) = from_source(
            src,
            &Options {
                transparent,
                ..Options::default()
            },
        )
        .unwrap();
        let bytes = webp::render(&d, 1.0).unwrap();
        assert_eq!(&bytes[..4], b"RIFF");
        let mut dec = WebPDecoder::new(Cursor::new(bytes)).unwrap();
        assert!(!dec.is_animated());
        let (w, h) = dec.dimensions();
        let mut buf = vec![0; dec.output_buffer_size().unwrap()];
        dec.read_image(&mut buf).unwrap();
        assert_eq!(buf.len(), (w * h * 4) as usize);
        assert_eq!(buf[3], if transparent { 0 } else { 255 }, "alpha du coin");
    }
}

#[test]
fn animated_webp_frames_equal_the_rasterised_frames() {
    let (d, _) = from_source(&fixture("animation.pug"), &Options::default()).unwrap();
    let o = GifOptions {
        fps: 4,
        hold: 0.5,
        transition: 0.5,
        draw_transition: 0.5,
        final_hold: 0.5,
        ..GifOptions::default()
    };
    let expected = frames(&d, &o);
    let bytes = webp::animated(&d, &o).unwrap();
    let mut dec = WebPDecoder::new(Cursor::new(bytes)).unwrap();
    assert!(dec.is_animated());
    assert_eq!(dec.num_frames() as usize, expected.len());
    let mut buf = vec![0; dec.output_buffer_size().unwrap()];
    for (k, (svg, _)) in expected.iter().enumerate() {
        dec.read_frame(&mut buf).unwrap();
        assert!(buf == rgba(svg, o.scale), "frame {k} differs");
    }
}

#[test]
fn webp_output_is_deterministic() {
    let (d, _) = from_source(&fixture("animation.pug"), &Options::default()).unwrap();
    let o = GifOptions {
        fps: 2,
        ..GifOptions::default()
    };
    assert_eq!(
        webp::animated(&d, &o).unwrap(),
        webp::animated(&d, &o).unwrap()
    );
}

#[test]
fn unknown_step_is_reported() {
    let (d, _) = from_source(&fixture("animation.pug"), &Options::default()).unwrap();
    assert!(webp::state(&d, "99x", 1.0).is_err());
    assert!(webp::state(&d, "1", 1.0).is_ok());
}
