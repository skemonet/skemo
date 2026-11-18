//! The tests written for ELK's drawing (its exact positions, its passes): their diagrams name
//! `layout="graph"`, since the slide engine became the default for a diagram that names none.
#![allow(dead_code)]

use skemo_engine::diagram::{Diagram, Error, Options};

/// `src` with `layout="graph"` on its `diagram` line, unless it names a layout already; an
/// implicit diagram gets a `diagram(layout="graph")` line, its elements under it.
pub fn graph(src: &str) -> String {
    let mut out = String::with_capacity(src.len() + 20);
    let mut done = false;
    for line in src.split_inclusive('\n') {
        let t = line.trim_start();
        if !done
            && (t.starts_with("diagram(")
                || t == "diagram"
                || t.starts_with("diagram\n")
                || t.starts_with("diagram "))
        {
            done = true;
            if t.contains("layout=") {
                out.push_str(line);
            } else if let Some(i) = line.find("diagram(") {
                out.push_str(&line[..i + 8]);
                out.push_str("layout=\"graph\" ");
                out.push_str(&line[i + 8..]);
            } else {
                let i = line.find("diagram").unwrap_or(0);
                out.push_str(&line[..i + 7]);
                out.push_str("(layout=\"graph\")");
                out.push_str(&line[i + 7..]);
            }
        } else {
            out.push_str(line);
        }
    }
    if done {
        return out;
    }
    // an implicit diagram (its elements at the top level): one written out around them
    let mut out = String::from("diagram(layout=\"graph\")\n");
    for line in src.split_inclusive('\n') {
        if !line.trim().is_empty() {
            out.push_str("  ");
        }
        out.push_str(line);
    }
    out
}

/// `skemo_engine::from_source`, drawn by ELK.
pub fn from_source(
    src: &str,
    opts: &Options,
) -> Result<(Diagram, Vec<skemo_lang::Diagnostic>), Error> {
    skemo_engine::diagram::from_source(&graph(src), opts)
}
