//! Source embedded in the rendered files (D-042): every format carries its bundle, gives it back
//! unchanged, and still decodes.

use skemo_engine::animated::{GifOptions, apng, gif};
use skemo_engine::source::{Bundle, NamedText, embed, extract};
use skemo_engine::{Options, from_source};
use std::io::Cursor;

const SRC: &str = "card#a(tag=\"v1\") Alpha  @1\n  to(b)\ncard#b Beta é  @2\n  li 10.0.0.1\n";

fn bundle() -> Bundle {
    let mut b = Bundle::new(SRC);
    b.name = Some("talk.skemo".into());
    b.css.push(NamedText {
        name: "custom.css".into(),
        content: ".t1 { --c: #C94A44; }\n".into(),
    });
    b.theme = Some("dark".into());
    b
}

fn quick() -> GifOptions {
    GifOptions {
        hold: 0.2,
        transition: 0.0,
        draw_transition: 0.0,
        fps: 5,
        ..GifOptions::default()
    }
}

#[test]
fn every_format_round_trips_and_still_decodes() {
    let (d, _) = from_source(SRC, &Options::default()).unwrap();
    let b = bundle();
    let outputs: Vec<(&str, Vec<u8>)> = vec![
        ("png", skemo_engine::png::render(&d, 1.0).unwrap()),
        ("apng", apng(&d, &quick()).unwrap()),
        ("gif", gif(&d, &quick()).unwrap()),
        ("webp", skemo_engine::webp::render(&d, 1.0).unwrap()),
        (
            "webp-anim",
            skemo_engine::webp::animated(&d, &quick()).unwrap(),
        ),
        ("svg", skemo_engine::svg::render(&d).into_bytes()),
        (
            "html",
            skemo_engine::html::render(&d, &skemo_engine::html::HtmlOptions::default())
                .into_bytes(),
        ),
    ];
    for (kind, plain) in outputs {
        assert_eq!(extract(&plain), None, "{kind}: nothing before embedding");
        let with = embed(plain.clone(), &b);
        assert!(with.len() > plain.len(), "{kind}: bundle added");
        assert_eq!(extract(&with).as_ref(), Some(&b), "{kind}: round trip");
        match kind {
            "png" | "apng" => {
                let dec = png::Decoder::new(Cursor::new(&with));
                let mut r = dec.read_info().unwrap();
                let mut buf = vec![0; r.output_buffer_size().unwrap()];
                r.next_frame(&mut buf).unwrap();
            }
            "gif" => {
                let mut dec = gif::DecodeOptions::new()
                    .read_info(Cursor::new(&with))
                    .unwrap();
                let mut n = 0;
                while dec.read_next_frame().unwrap().is_some() {
                    n += 1;
                }
                assert!(n >= 2);
            }
            "webp" | "webp-anim" => {
                let mut dec = image_webp::WebPDecoder::new(Cursor::new(&with)).unwrap();
                let mut buf = vec![0; dec.output_buffer_size().unwrap()];
                dec.read_image(&mut buf).unwrap();
            }
            _ => {}
        }
    }
}

#[test]
fn an_embedded_image_renders_like_its_source() {
    let (d, _) = from_source(SRC, &Options::default()).unwrap();
    let png = embed(
        skemo_engine::png::render(&d, 1.0).unwrap(),
        &Bundle::new(SRC),
    );
    let back = extract(&png).unwrap();
    let (d2, _) = from_source(&back.source, &Options::default()).unwrap();
    assert_eq!(d.boxes, d2.boxes);
    assert_eq!(d.routes, d2.routes);
}
