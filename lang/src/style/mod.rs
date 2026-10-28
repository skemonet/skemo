//! Styles (SPEC §4): CSS sheets, selectors, cascade, computed values, default theme.
//!
//! The engine builds a tree of elements ([`StyleTree`]) that reproduces the rendered structure
//! (`FORMAT.md` §4.2), then asks for each one's computed style ([`Styler::compute`]).

pub mod cascade;
pub mod css;
pub mod select;
pub mod values;

pub use cascade::{Computed, MediaContext, StyleTree, Styler, substitute, unquote};
pub use css::Stylesheet;
pub use values::{Rgba, parse_color, parse_length};

/// Embedded default theme (F-STYLE-1), equivalent to the prototype's theme.
pub const DEFAULT_THEME: &str = include_str!("../../theme/default.css");

/// Reference theme: the prototype's, frozen (parity oracle). Classic cards by default; to use with
/// `Options::base_theme` to get the oracle's outputs back.
pub const REFERENCE_THEME: &str = include_str!("../../default.reference.css");

/// Available themes (F-STYLE-14).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Theme {
    Light,
    Dark,
    Print,
    /// Follows the system (HTML only); rendered as `Light` in SVG and PNG.
    Auto,
}

impl Theme {
    pub fn parse(s: &str) -> Option<Theme> {
        match s {
            "light" => Some(Theme::Light),
            "dark" => Some(Theme::Dark),
            "print" => Some(Theme::Print),
            "auto" => Some(Theme::Auto),
            _ => None,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Theme::Light => "light",
            Theme::Dark => "dark",
            Theme::Print => "print",
            Theme::Auto => "auto",
        }
    }
    /// Class set on `.stage`.
    pub fn class(self) -> String {
        format!("theme-{}", self.name())
    }
}

/// Styler for the default theme followed by the custom CSS (in order).
pub fn styler(custom: &[&str]) -> Styler {
    let mut sheets = vec![Stylesheet::parse(DEFAULT_THEME)];
    sheets.extend(custom.iter().map(|c| Stylesheet::parse(c)));
    Styler::new(sheets, MediaContext::default())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reproduces the structure of a prototype card and checks the values the engine uses.
    #[test]
    fn card_t0() {
        let s = styler(&[]);
        let mut t = StyleTree::new();
        let stage = t.add(None, "div", Some("stage"), &["stage"]);
        let card = t.add(Some(stage), "card", Some("fw"), &["t0"]);
        let title = t.add(Some(card), "div", None, &["title"]);
        let before = t.add_pseudo(title, "before");
        let ul = t.add(Some(card), "ul", None, &[]);
        let li = t.add(Some(ul), "li", None, &[]);
        let mono = t.add(Some(li), "span", None, &["mono"]);
        let c = s.compute(&t);
        assert_eq!(c[stage].font_size(), 14.0);
        assert_eq!(c[card].padding(), [11.0, 15.0, 13.0, 15.0]);
        assert_eq!(c[card].border(), [3.0, 1.0, 1.0, 1.0]);
        assert_eq!(c[card].min_width(), 96.0);
        assert_eq!(c[card].max_width(), Some(280.0));
        assert_eq!(c[title].font_weight(), 650);
        assert!((c[title].line_height().unwrap() - 19.6).abs() < 1e-9);
        assert_eq!(c[before].string("content").as_deref(), Some("T0"));
        assert_eq!(c[before].font_size(), 10.5);
        assert!((c[before].letter_spacing() - 0.315).abs() < 1e-9);
        assert_eq!(c[before].padding(), [1.0, 7.0, 1.0, 7.0]);
        // accent fill: computed with the card's --c (pitfall P27)
        assert_eq!(
            c[before].color_of("background").map(|c| c.to_css()),
            Some("#AB3F3A".into())
        );
        assert_eq!(c[ul].margin(), [8.0, 0.0, 0.0, 0.0]);
        assert_eq!(c[ul].padding(), [0.0; 4]);
        assert_eq!(c[li].font_size(), 12.5);
        assert_eq!(c[li].margin()[0], 3.0);
        assert_eq!(c[mono].font_size(), 12.0);
        assert_eq!(c[mono].font_family()[0], "JetBrains Mono");
    }

    #[test]
    fn header_look_and_print() {
        let mut s = styler(&[".seg { --c: #4E9A3E; --label: \"Segment\"; --look: header; }"]);
        let mut t = StyleTree::new();
        let stage = t.add(None, "div", None, &["stage", "theme-print"]);
        let card = t.add(Some(stage), "card", None, &["seg"]);
        let c = s.compute(&t);
        assert_eq!(c[card].string("--look").as_deref(), Some("header"));
        assert_eq!(c[card].border()[1], 1.5);
        s = styler(&[]);
        let c2 = s.compute(&t);
        assert_eq!(c2[card].string("--label"), None);
    }
}
