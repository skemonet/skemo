//! # skemo-lang — langage de diagramme skemo
//!
//! A diagram is a Pug file limited to 7 tags (`diagram`, `title`, `card`, `cards`, `zone`, `stack`,
//! `to`) and lines `|` whose text follows Markdown (emphasis, tables). The contract of the format
//! is `FORMAT.md`.
//!
//! ```
//! let doc = skemo_lang::parse("card#a Départ\n  to(b) suite\ncard#b Arrivée\n");
//! assert!(!doc.has_errors());
//! assert_eq!(doc.model.edges[0].id, "a->b");
//! ```

#![forbid(unsafe_code)]

pub mod anim;
pub mod dates;
pub mod diag;
pub mod edit;
pub mod fmt;
pub mod i18n;
pub mod inline;
pub mod model;
pub mod parse;
pub mod structure;
pub mod style;
pub mod syntax;
pub mod table;

pub use diag::{Diagnostic, Severity, Span};
pub use model::{Align, Anim, Cell, Deck, Edge, Focus, Join, Kind, Li, Model, Node, Step, Title};
pub use parse::{Document, Located, SourceMap, parse};

/// The README's examples are compiled and run with the tests (`cargo test --doc`).
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;
