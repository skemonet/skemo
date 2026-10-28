//! Messages to the user on standard error, with or without colours.

use skemo_lang::tr;
/// Output of the diagnostics and the messages.
#[derive(Clone, Copy)]
pub struct Out {
    pub color: bool,
    pub quiet: bool,
}

impl Out {
    /// A diagnostic from `lang` (warnings are left out in quiet mode).
    pub fn diag(&self, src: &str, file: &str, d: &skemo_lang::Diagnostic) {
        if self.quiet && !d.is_error() {
            return;
        }
        let mut s = skemo_lang::diag::render(src, file, d);
        if self.color {
            let (word, code) = if d.is_error() {
                (tr!("error", "erreur"), "1;31")
            } else {
                (tr!("warning", "avertissement"), "1;33")
            };
            s = s.replacen(&word, &format!("\x1b[{code}m{word}\x1b[0m"), 1);
        }
        eprint!("{s}");
    }

    pub fn warn(&self, msg: &str) {
        if !self.quiet {
            eprintln!(
                "{}{} {msg}",
                self.tag(&tr!("warning", "avertissement"), "1;33"),
                tr!(":", " :")
            );
        }
    }

    /// Information line (files written…), omitted in quiet mode.
    pub fn note(&self, msg: &str) {
        if !self.quiet {
            eprintln!("{msg}");
        }
    }

    pub fn error(&self, msg: &str) {
        eprintln!(
            "{}{} {msg}",
            self.tag(&tr!("error", "erreur"), "1;31"),
            tr!(":", " :")
        );
    }

    /// Tag word, coloured if asked.
    fn tag(&self, word: &str, code: &str) -> String {
        if self.color {
            format!("\x1b[{code}m{word}\x1b[0m")
        } else {
            word.to_string()
        }
    }

    /// Same output, without the warnings.
    pub fn silenced(self) -> Out {
        Out {
            quiet: true,
            ..self
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_are_colored_only_on_request() {
        let plain = Out {
            color: false,
            quiet: false,
        };
        let colored = Out {
            color: true,
            quiet: false,
        };
        assert_eq!(plain.tag("erreur", "1;31"), "erreur");
        assert_eq!(colored.tag("erreur", "1;31"), "\x1b[1;31merreur\x1b[0m");
    }

    #[test]
    fn silenced_keeps_color() {
        let o = Out {
            color: true,
            quiet: false,
        }
        .silenced();
        assert!(o.quiet && o.color);
    }
}
