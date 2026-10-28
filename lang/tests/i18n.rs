//! The language is process-wide: these tests have their own process so that they cannot disturb
//! the others (which run in English), and take a lock so that they do not disturb each other.

use skemo_lang::i18n::{Lang, language, set_language};
use skemo_lang::tr;
use std::sync::Mutex;

static LANGUAGE: Mutex<()> = Mutex::new(());

#[test]
fn the_macro_follows_the_current_language() {
    let _lock = LANGUAGE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let n = 3;
    assert_eq!(language(), Lang::En, "English by default");
    assert_eq!(tr!("{n} items", "{n} éléments"), "3 items");
    set_language(Lang::Fr);
    assert_eq!(language(), Lang::Fr);
    assert_eq!(tr!("{n} items", "{n} éléments"), "3 éléments");
    assert_eq!(tr!("{} and {}", "{} et {}", 1, 2), "1 et 2");
    set_language(Lang::En);
    assert_eq!(tr!("{} and {}", "{} et {}", 1, 2), "1 and 2");
}

#[test]
fn diagnostics_follow_the_language() {
    let _lock = LANGUAGE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let src = "card#a\n  to(zz)\n";
    set_language(Lang::En);
    let en = skemo_lang::parse(src);
    set_language(Lang::Fr);
    let fr = skemo_lang::parse(src);
    set_language(Lang::En);
    let (e, f) = (&en.diagnostics[0], &fr.diagnostics[0]);
    assert_eq!(e.code, f.code, "codes never change");
    assert_ne!(e.message, f.message);
    assert!(f.message.contains("zz") && e.message.contains("zz"));
}
