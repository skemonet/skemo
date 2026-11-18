//! `skemo capabilities`: what this binary supports, for the tools that run it (`cli/SPEC.md`
//! §2.4). glitu checks a minimum version and the features it needs.

use crate::args::Diagnostics;
use crate::common::{Fail, write_out};

/// Version of the SVG conventions (`engine/SVG.md`): raised on any change that is not an
/// addition.
pub const SVG_CONVENTIONS: u32 = 1;

/// Features, among those tools ask for (`cli/SPEC.md` §2).
pub const FEATURES: [&str; 6] = [
    "diagnostics-json",
    "highlights",
    "root",
    "batch",
    "limits",
    "allow",
];

pub fn json() -> serde_json::Value {
    serde_json::json!({
        "version": env!("CARGO_PKG_VERSION"),
        "svg_conventions": SVG_CONVENTIONS,
        "features": FEATURES,
    })
}

pub fn run(format: Diagnostics) -> Result<(), Fail> {
    let text = match format {
        Diagnostics::Json => format!("{}\n", json()),
        Diagnostics::Text => format!(
            "skemo {}\nsvg_conventions {SVG_CONVENTIONS}\nfeatures {}\n",
            env!("CARGO_PKG_VERSION"),
            FEATURES.join(" ")
        ),
    };
    write_out("-", text.as_bytes())
}
