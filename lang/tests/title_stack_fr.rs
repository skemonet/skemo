//! The warnings of `title_stack.rs` in French (language set once for this process).

use skemo_lang::i18n::{Lang, set_language};

const BAD: &str = "diagram
  title One
    card#x X
  title Two
  stack Named
    card#a A
      title Inside
";

#[test]
fn warnings_in_french() {
    set_language(Lang::Fr);
    let doc = skemo_lang::parse(BAD);
    let out: String = doc
        .diagnostics
        .iter()
        .map(|d| skemo_lang::diag::render(BAD, "bad.skemo", d))
        .collect();
    insta::assert_snapshot!("fr-title-stack-warnings", out);
}
