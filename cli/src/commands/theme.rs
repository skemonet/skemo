//! `skemo theme --print`: the default theme, starting point of a custom theme.

use crate::common::{Fail, write_out};
use skemo_lang::tr;

pub fn run(print: bool) -> Result<(), Fail> {
    if !print {
        return Err(Fail::usage(tr!("add --print", "préciser --print")));
    }
    write_out("-", skemo_lang::style::DEFAULT_THEME.as_bytes())
}
