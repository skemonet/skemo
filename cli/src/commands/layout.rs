//! `skemo layout` : placement complet au format `layout.json`.

use crate::args::LayoutArgs;
use crate::common::{Fail, Inputs, prepare, write_out};
use crate::out::Out;

pub fn run(out: &Out, a: &LayoutArgs) -> Result<(), Fail> {
    let inputs = Inputs::new(&a.file, &a.css, a.base_theme.as_ref()).theme(a.theme);
    let (d, _, _) = prepare(out, inputs)?;
    let s = serde_json::to_string_pretty(&d.layout_json()).unwrap_or_default();
    write_out("-", format!("{s}\n").as_bytes())
}
