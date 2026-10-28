//! `skemo extract`: the source and stylesheets embedded in a rendered image (D-042), written back
//! as files to edit. The image itself can also be given to any other command.

use crate::args::ExtractArgs;
use crate::common::{Fail, write_out};
use crate::out::Out;
use skemo_engine::source::{NamedText, extract, safe_name};
use skemo_lang::tr;
use std::path::{Path, PathBuf};

pub fn run(out: &Out, a: &ExtractArgs) -> Result<(), Fail> {
    let raw = std::fs::read(&a.image).map_err(|e| {
        Fail::usage(tr!(
            "reading « {} »: {e}",
            "lecture de « {} » : {e}",
            a.image
        ))
    })?;
    let b = extract(&raw).ok_or_else(|| {
        Fail(
            1,
            Some(tr!(
                "« {} » carries no skemo source (rendered with --no-source, or by another tool)",
                "« {} » ne contient pas de source skemo (rendu avec --no-source, ou par un autre outil)",
                a.image
            )),
        )
    })?;
    if a.stdout {
        return write_out("-", b.source.as_bytes());
    }
    let image = Path::new(&a.image);
    let dir = a
        .output
        .clone()
        .or_else(|| image.parent().map(Path::to_path_buf))
        .unwrap_or_default();
    // the image's name, so that a.png gives a.skemo (the bundle's name may be another file)
    let stem = image
        .file_stem()
        .and_then(|s| s.to_str())
        .map_or("diagram", |s| s.strip_suffix(".skemo").unwrap_or(s));
    let mut files: Vec<(PathBuf, Vec<u8>)> = vec![(
        dir.join(format!("{stem}.skemo")),
        b.source.clone().into_bytes(),
    )];
    let css = b.css.iter().chain(b.base_theme.iter());
    for NamedText { name, content } in css {
        if let Some(n) = safe_name(name) {
            files.push((dir.join(n), content.clone().into_bytes()));
        }
    }
    // image files, at the relative path the source names them by (D-046)
    for img in &b.images {
        if let (Ok(()), Some(data)) = (skemo_engine::images::check_name(&img.name), img.bytes()) {
            files.push((dir.join(&img.name), data));
        }
    }
    if !a.force
        && let Some((p, _)) = files.iter().find(|(p, _)| p.exists())
    {
        return Err(Fail::usage(tr!(
            "« {} » exists: give --force to overwrite it, or -o another folder",
            "« {} » existe : donner --force pour l'écraser, ou -o un autre dossier",
            p.display()
        )));
    }
    for (p, content) in &files {
        write_out(&p.to_string_lossy(), content)?;
        out.note(&p.display().to_string());
    }
    let mut hint = Vec::new();
    for c in &b.css {
        hint.push(format!("--css {}", c.name));
    }
    if let Some(t) = &b.theme {
        hint.push(format!("--theme {t}"));
    }
    if let Some(k) = b.font_scale {
        hint.push(format!("--font-scale {k}"));
    }
    if !hint.is_empty() {
        out.note(&tr!("rendered with: {}", "rendu avec : {}", hint.join(" ")));
    }
    Ok(())
}
