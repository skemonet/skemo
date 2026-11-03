//! Language of the command line: the system's (`SKEMO_LANG`, `LC_ALL`, `LC_MESSAGES`,
//! `LANGUAGE`, `LANG`), chosen before anything is printed.

use skemo_lang::i18n::{Lang, detect, set_language};

/// Chooses and installs the system language.
pub fn init() -> Lang {
    let lang = detect();
    set_language(lang);
    lang
}
