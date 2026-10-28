//! Animated outputs: one PNG per state, GIF, APNG, contact sheet; unknown states and still
//! diagrams.

use skemo_engine::animated::{self, GifOptions};
use skemo_engine::{Options, from_source};
use std::path::Path;

fn animated_diagram() -> skemo_engine::Diagram {
    let src = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/animation.pug"),
    )
    .unwrap();
    from_source(&src, &Options::default()).unwrap().0
}

fn still_diagram() -> skemo_engine::Diagram {
    from_source(
        "diagram\n  card#a A\n    to(b)\n  card#b B\n",
        &Options::default(),
    )
    .unwrap()
    .0
}

fn fast() -> GifOptions {
    GifOptions {
        fps: 10,
        hold: 0.4,
        transition: 0.2,
        draw_transition: 0.2,
        final_hold: 0.4,
        ..GifOptions::default()
    }
}

#[test]
fn one_png_per_state_with_an_index() {
    let d = animated_diagram();
    let (pngs, index) = animated::states_png(&d, 1.0).unwrap();
    assert!(pngs.len() >= 2, "{}", pngs.len());
    assert!(pngs.iter().all(|p| p.starts_with(b"\x89PNG")));
    assert_eq!(index["states"].as_array().unwrap().len(), pngs.len());
    // deterministic
    assert_eq!(animated::states_png(&d, 1.0).unwrap().0, pngs);
}

#[test]
fn a_named_state_and_an_unknown_one() {
    let d = animated_diagram();
    let first = d.anim.states.first().unwrap().raw.clone();
    let png = animated::state_png(&d, &first, 1.0).unwrap();
    assert!(png.starts_with(b"\x89PNG"));
    let err = animated::state_png(&d, "abc", 1.0).unwrap_err();
    assert!(err.contains("abc"), "{err}");
    // a numeric step that is not a written state is accepted (view at that value)
    assert!(animated::state_png(&d, "99", 1.0).is_ok());
}

#[test]
fn a_still_diagram_has_a_single_state() {
    let d = still_diagram();
    let (pngs, _) = animated::states_png(&d, 1.0).unwrap();
    assert_eq!(pngs.len(), 1);
    assert_eq!(animated::frames(&d, &fast()).len(), 1);
}

#[test]
fn gif_apng_and_board_are_valid_and_deterministic() {
    let d = animated_diagram();
    let o = fast();
    let frames = animated::frames(&d, &o);
    assert!(frames.len() > d.anim.states.len(), "transitions add frames");
    assert!(
        frames
            .iter()
            .all(|(svg, secs)| svg.starts_with("<svg") && *secs > 0.0)
    );
    let gif = animated::gif(&d, &o).unwrap();
    assert_eq!(&gif[..6], b"GIF89a");
    assert_eq!(animated::gif(&d, &o).unwrap(), gif);
    let apng = animated::apng(&d, &o).unwrap();
    assert!(apng.starts_with(b"\x89PNG") && apng.windows(4).any(|w| w == b"acTL"));
    let once = animated::gif(
        &d,
        &GifOptions {
            looping: false,
            ..fast()
        },
    )
    .unwrap();
    assert_ne!(once, gif, "no loop extension");
    let board = animated::board(&d, 1.0).unwrap();
    assert!(board.starts_with(b"\x89PNG"));
    assert!(animated::dim_opacity(&d) > 0.0 && animated::dim_opacity(&d) < 1.0);
}
