//! `skemo init`: writes a sample diagram and, on request, a stylesheet it includes
//! (`diagram(css=…)`): a short commented one (`--css`) or the whole default theme (`--css-full`).
//! Never overwrites a file.

use crate::args::InitArgs;
use crate::common::{Fail, check_css_name, io};
use skemo_lang::tr;
use std::path::Path;

const EXAMPLE: &str = include_str!("../init/example.skemo");
const STYLE: &str = include_str!("../init/style.css");

/// Head of the full theme written by `--css-full`.
const FULL_HEAD: &str = "/* The whole default theme, included by the diagram (diagram(css=…)): edit what you want.\n   It applies over the built-in theme, so a rule removed here falls back to it. */\n\n";

/// The diagram, and the stylesheet named relative to the diagram's folder.
pub fn run(a: &InitArgs) -> Result<(), Fail> {
    let file = a.file.as_path();
    let (css, style) = match (&a.css, &a.css_full) {
        (Some(c), _) => (Some(c.as_str()), STYLE.to_string()),
        (None, Some(c)) => (
            Some(c.as_str()),
            format!("{FULL_HEAD}{}", skemo_lang::style::DEFAULT_THEME),
        ),
        (None, None) => (None, String::new()),
    };
    if let Some(name) = css {
        check_css_name(name).map_err(Fail::usage)?;
    }
    let dir = file.parent().unwrap_or(Path::new(""));
    let name = |p: &Path| p.file_name().map(|n| n.to_string_lossy().into_owned());
    let shown = name(file).unwrap_or_default();
    let src = EXAMPLE
        .replace("{file}", &shown)
        .replace(
            "{png}",
            &name(&file.with_extension("png")).unwrap_or_default(),
        )
        .replace(
            "{css}",
            &css.map(|c| format!(" css=\"{c}\"")).unwrap_or_default(),
        );
    let mut files = vec![(file.to_path_buf(), src)];
    if let Some(c) = css {
        files.push((dir.join(c), style));
    }
    if let Some((p, _)) = files.iter().find(|(p, _)| p.exists()) {
        return Err(Fail::usage(tr!(
            "« {} » already exists: nothing was written",
            "« {} » existe déjà : rien n'a été écrit",
            p.display()
        )));
    }
    for (p, content) in &files {
        if let Some(d) = p.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(d)
                .map_err(|e| io(&e))
                .map_err(|e| {
                    Fail::usage(tr!(
                        "creating « {} »: {e}",
                        "création de « {} » : {e}",
                        d.display()
                    ))
                })?;
        }
        std::fs::write(p, content)
            .map_err(|e| io(&e))
            .map_err(|e| {
                Fail::usage(tr!(
                    "writing « {} »: {e}",
                    "écriture de « {} » : {e}",
                    p.display()
                ))
            })?;
    }
    let names: Vec<String> = files.iter().map(|(p, _)| p.display().to_string()).collect();
    let f = file.display();
    eprintln!(
        "{}",
        tr!(
            "Created {}.\nDraw it:      skemo {f}\nLive preview: skemo {f} --watch",
            "Créé : {}.\nLe dessiner :      skemo {f}\nAperçu en direct : skemo {f} --watch",
            names.join(", ")
        )
    );
    Ok(())
}
