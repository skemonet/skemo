//! `skemo fmt`: canonical formatting, or checking with `--check`.

use crate::args::FmtArgs;
use crate::common::{Fail, read_source, write_out};
use crate::out::Out;
use skemo_lang::tr;

pub fn run(out: &Out, a: &FmtArgs) -> Result<(), Fail> {
    let mut failed = false;
    for f in &a.files {
        let src = read_source(f).map_err(Fail::usage)?;
        match skemo_lang::fmt::format(&src) {
            Ok(formatted) if formatted == src => {}
            Ok(_) if a.check => {
                out.warn(&tr!("{f} is not formatted", "{f} n'est pas formaté"));
                failed = true;
            }
            // `-`: standard output, otherwise the file itself
            Ok(formatted) => write_out(f, formatted.as_bytes())?,
            Err(errs) => {
                for e in &errs {
                    out.diag(&src, f, e);
                }
                failed = true;
            }
        }
    }
    if failed { Err(Fail::source()) } else { Ok(()) }
}
