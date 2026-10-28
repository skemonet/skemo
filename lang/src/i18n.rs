//! Language of the messages (diagnostics, errors, help texts).
//!
//! The libraries speak **English** by default, so their output never depends on the machine
//! running them (tests, WebAssembly, servers). A program that talks to a person — the command
//! line, the editor — chooses the language once at start-up with [`set_language`], usually from
//! [`detect`] (the user's operating-system language).
//!
//! Messages are written at the place where they are produced, in every language, with `tr!`:
//! `tr!("unknown tag « {name} »", "balise « {name} » inconnue")`. Both strings are checked at
//! compile time like `format!` strings (inline `{name}` captures work in both).

use std::sync::atomic::{AtomicU8, Ordering};

/// A message language.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Lang {
    #[default]
    En,
    Fr,
}

impl Lang {
    /// Reads a locale name (`fr`, `fr_FR.UTF-8`, `fr-CA`, `en_US:fr`…); `None` if it is empty,
    /// `C` or `POSIX`. Any language without translation maps to English.
    pub fn parse(locale: &str) -> Option<Lang> {
        let first = locale.split(':').next()?.trim();
        let name = first.split(['.', '@']).next()?.to_ascii_lowercase();
        if name.is_empty() || name == "c" || name == "posix" {
            return None;
        }
        let lang = name.split(['_', '-']).next()?;
        Some(if lang == "fr" { Lang::Fr } else { Lang::En })
    }

    /// Code of the language (`en`, `fr`).
    pub fn code(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::Fr => "fr",
        }
    }
}

static CURRENT: AtomicU8 = AtomicU8::new(0);

/// Sets the language of the messages produced from now on (process-wide).
pub fn set_language(lang: Lang) {
    CURRENT.store(u8::from(lang == Lang::Fr), Ordering::Relaxed);
}

/// Current language.
pub fn language() -> Lang {
    if CURRENT.load(Ordering::Relaxed) == 1 {
        Lang::Fr
    } else {
        Lang::En
    }
}

/// Is the current language French? (what `tr!` tests)
pub fn is_french() -> bool {
    language() == Lang::Fr
}

/// The user's language from the environment, in the order of the usual convention:
/// `SKEMO_LANG`, `LC_ALL`, `LC_MESSAGES`, `LANGUAGE`, `LANG`. First one that names a language
/// wins (`C`/`POSIX` are skipped); English when none does.
pub fn detect() -> Lang {
    detect_from(|k| std::env::var(k).ok())
}

/// [`detect`] with an explicit environment (tests).
pub fn detect_from(get: impl Fn(&str) -> Option<String>) -> Lang {
    ["SKEMO_LANG", "LC_ALL", "LC_MESSAGES", "LANGUAGE", "LANG"]
        .iter()
        .filter_map(|k| get(k))
        .find_map(|v| Lang::parse(&v))
        .unwrap_or_default()
}

/// A message in every language, formatted like `format!`:
/// `tr!("english {x}", "français {x}")`.
#[macro_export]
macro_rules! tr {
    ($en:literal, $fr:literal $(, $($args:tt)*)?) => {
        if $crate::i18n::is_french() {
            format!($fr $(, $($args)*)?)
        } else {
            format!($en $(, $($args)*)?)
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let m: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect();
        move |k| m.get(k).cloned()
    }

    #[test]
    fn locale_names_map_to_a_language() {
        assert_eq!(Lang::parse("fr"), Some(Lang::Fr));
        assert_eq!(Lang::parse("fr_FR.UTF-8"), Some(Lang::Fr));
        assert_eq!(Lang::parse("FR-ca"), Some(Lang::Fr));
        assert_eq!(Lang::parse("fr_BE@euro"), Some(Lang::Fr));
        assert_eq!(Lang::parse("fr:en"), Some(Lang::Fr));
        assert_eq!(Lang::parse("en_US.UTF-8"), Some(Lang::En));
        assert_eq!(
            Lang::parse("de_DE"),
            Some(Lang::En),
            "no translation: English"
        );
        assert_eq!(Lang::parse("C"), None);
        assert_eq!(Lang::parse("POSIX"), None);
        assert_eq!(Lang::parse("C.UTF-8"), None);
        assert_eq!(Lang::parse(""), None);
        assert_eq!(Lang::Fr.code(), "fr");
        assert_eq!(Lang::En.code(), "en");
    }

    #[test]
    fn environment_order_and_fallbacks() {
        assert_eq!(detect_from(env(&[])), Lang::En);
        assert_eq!(detect_from(env(&[("LANG", "fr_FR.UTF-8")])), Lang::Fr);
        // LC_ALL beats LANG; SKEMO_LANG beats everything
        assert_eq!(
            detect_from(env(&[("LANG", "fr_FR"), ("LC_ALL", "en_US")])),
            Lang::En
        );
        assert_eq!(
            detect_from(env(&[("LC_ALL", "en_US"), ("SKEMO_LANG", "fr")])),
            Lang::Fr
        );
        // C / POSIX are skipped in favour of the next variable
        assert_eq!(
            detect_from(env(&[("LC_ALL", "C"), ("LANG", "fr_FR.UTF-8")])),
            Lang::Fr
        );
        assert_eq!(
            detect_from(env(&[("LC_ALL", "C"), ("LANG", "C")])),
            Lang::En
        );
    }
}
