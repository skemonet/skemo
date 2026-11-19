//! # skemo-engine — the skemo engine
//!
//! Pipeline: model (`lang`) → styled scene → measure → placement (ELK) → pinned positions →
//! routing (`router`) → labels → rendering (HTML, SVG, PNG) and steps.

#![forbid(unsafe_code)]

pub mod anim;
#[cfg(feature = "png")]
pub mod animated;
pub mod arrow;
pub(crate) mod breathe;
pub mod contrast;
pub(crate) mod corridors;
pub mod diagram;
pub mod dock;
pub mod fit;
pub mod fonts;
pub mod frise;
pub mod geometry;
pub(crate) mod grid;
pub mod html;
pub mod icons;
pub mod images;
pub mod labels;
pub mod layout;
pub mod lines;
pub mod measure;
pub mod mindmap;
pub mod motion;
#[cfg(feature = "png")]
pub mod png;
pub mod quality;
pub mod reorder;
pub mod route;
pub mod scene;
pub mod session;
pub(crate) mod slide;
pub mod source;
pub mod stack;
pub mod svg;
pub mod tidy;
pub mod timeline;
pub mod trace;
pub mod tunnel;
pub mod warning;
#[cfg(feature = "png")]
pub mod webp;

pub use diagram::{Diagram, Error, Frame, Options, build, from_source};
pub use warning::EngineWarning;

/// What the engine embeds that is not its own, with the licence and copyright notice that must
/// travel with it: (what, licence, full text). For `skemo --licences`.
pub fn notices() -> Vec<(&'static str, &'static str, &'static str)> {
    vec![
        (
            "Inter (fonts)",
            "OFL-1.1",
            include_str!("../assets/fonts/LICENSE-Inter.txt"),
        ),
        (
            "JetBrains Mono (font)",
            "OFL-1.1",
            include_str!("../assets/fonts/LICENSE-JetBrainsMono.txt"),
        ),
        (
            "Lucide (icons)",
            "ISC",
            include_str!("../assets/icons/LICENSE-lucide.txt"),
        ),
    ]
}

/// The README's examples are compiled and run with the tests (`cargo test --doc`).
#[cfg(doctest)]
#[doc = include_str!("../../../README.md")]
struct ReadmeDoctests;

/// Language of the labels produced in the drawing (months, quarters, centuries): the option
/// `diagram(lang="fr"|"en")` if it is written, otherwise the current language of the messages.
pub fn label_lang(model: &skemo_lang::Model) -> skemo_lang::i18n::Lang {
    model
        .option("lang")
        .and_then(skemo_lang::i18n::Lang::parse)
        .unwrap_or_else(skemo_lang::i18n::language)
}
