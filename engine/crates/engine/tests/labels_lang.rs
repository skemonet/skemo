//! Labels of the drawing (months, quarters, years and centuries): `diagram(lang=…)` first, otherwise
//! the current language. A separate process: the language is global.

use skemo_engine::timeline::{ticks, ticks_in};
use skemo_engine::{Options, from_source, svg};
use skemo_lang::i18n::{Lang, set_language};

const FRIEZE: &str = "diagram(layout=\"frise\" scale=\"century\")\n  card#a(date=\"-0450\") Athènes\n  card#b(date=\"1789\") Révolution\n";
const TIMELINE: &str = "diagram(layout=\"timeline\" scale=\"quarter\")\n  card#a(start=\"2026-01-10\" end=\"2026-09-30\") Projet\n";

fn labels(src: &str) -> String {
    let (d, _) = from_source(src, &Options::default()).unwrap();
    svg::render(&d)
}

#[test]
fn month_and_quarter_labels() {
    let en = ticks_in(Lang::En, "quarter", 20_454.0, 20_700.0);
    let fr = ticks_in(Lang::Fr, "quarter", 20_454.0, 20_700.0);
    assert!(en.iter().all(|t| t.1.starts_with('Q')), "{en:?}");
    assert!(fr.iter().all(|t| t.1.starts_with('T')), "{fr:?}");
    assert!(
        ticks_in(Lang::En, "month", 20_454.0, 20_700.0)
            .iter()
            .any(|t| t.1 == "Mar" || t.1 == "Feb")
    );
    assert!(
        ticks_in(Lang::Fr, "month", 20_454.0, 20_700.0)
            .iter()
            .any(|t| t.1 == "févr." || t.1 == "mars")
    );
    assert!(
        ticks_in(Lang::En, "week", 20_454.0, 20_480.0)[0]
            .1
            .contains(char::is_numeric)
    );
    assert!(ticks_in(Lang::Fr, "day", 20_454.0, 20_457.0).len() >= 3);
}

#[test]
fn drawing_labels_follow_the_option_then_the_current_language() {
    set_language(Lang::En);
    assert!(labels(FRIEZE).contains("BC"), "English by default");
    assert!(
        labels(&FRIEZE.replace("diagram(", "diagram(lang=\"fr\" ")).contains("av."),
        "the option wins"
    );
    assert!(labels(TIMELINE).contains("Q2"));
    set_language(Lang::Fr);
    assert!(
        labels(FRIEZE).contains("av."),
        "French from the current language"
    );
    assert!(labels(&FRIEZE.replace("diagram(", "diagram(lang=\"en\" ")).contains("BC"));
    assert!(labels(TIMELINE).contains("T2"));
    assert!(!ticks("month", 20_454.0, 20_700.0).is_empty());
    set_language(Lang::En);
}

#[test]
fn english_century_ordinals() {
    set_language(Lang::En);
    // the default theme hides the century row (D-048): show it
    let labels = |src: &str| {
        let opts = Options {
            css: vec![".fr-century { display: block; }".into()],
            ..Options::default()
        };
        svg::render(&from_source(src, &opts).unwrap().0)
    };
    let svg = labels(
        "diagram(layout=\"frise\" scale=\"century\" from=\"-1\" to=\"2300\")\n  card#a(date=\"1\") A\n  card#b(date=\"2200\") B\n",
    );
    for ordinal in ["1st c.", "2nd c.", "3rd c.", "11th c.", "21st c."] {
        assert!(svg.contains(ordinal), "{ordinal}");
    }
}
