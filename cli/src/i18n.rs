//! Language of the command line: the system's (or `--lang`), chosen before anything is printed.

use skemo_lang::i18n::{Lang, detect, set_language};

/// Value of `--lang` / `--lang=` in the raw arguments, if any (read before clap parses, so that
/// even the help and the usage errors come out in the right language).
pub fn requested(args: &[String]) -> Option<String> {
    let mut it = args.iter().skip(1);
    while let Some(a) = it.next() {
        if a == "--" {
            break;
        }
        if let Some(v) = a.strip_prefix("--lang=") {
            return Some(v.to_string());
        }
        if a == "--lang" {
            return it.next().cloned();
        }
    }
    None
}

/// Chooses and installs the language: `--lang`, else the system language.
pub fn init(args: &[String]) -> Lang {
    let lang = requested(args)
        .and_then(|v| Lang::parse(&v))
        .unwrap_or_else(detect);
    set_language(lang);
    lang
}

#[cfg(test)]
mod tests {
    use super::*;

    fn argv(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| (*s).to_string()).collect()
    }

    #[test]
    fn the_option_is_found_in_both_forms_and_stops_at_double_dash() {
        assert_eq!(
            requested(&argv(&["skemo", "--lang", "fr", "x"])).as_deref(),
            Some("fr")
        );
        assert_eq!(
            requested(&argv(&["skemo", "render", "x", "--lang=en"])).as_deref(),
            Some("en")
        );
        assert_eq!(requested(&argv(&["skemo", "--lang"])), None);
        assert_eq!(requested(&argv(&["skemo", "x"])), None);
        assert_eq!(requested(&argv(&["skemo", "--", "--lang=fr"])), None);
    }
}
