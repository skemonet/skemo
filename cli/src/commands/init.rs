//! `skemo init`: writes an example and a commented custom theme, never overwriting a file.

use crate::common::Fail;
use skemo_lang::tr;
use std::path::Path;

const FILES: [(&str, &str); 2] = [
    ("example.skemo", include_str!("../init/example.skemo")),
    ("custom.css", include_str!("../init/custom.css")),
];

pub fn run(dir: &Path) -> Result<(), Fail> {
    if let Some((n, _)) = FILES.iter().find(|(n, _)| dir.join(n).exists()) {
        return Err(Fail::usage(tr!(
            "« {} » already exists: nothing was written",
            "« {} » existe déjà : rien n'a été écrit",
            dir.join(n).display()
        )));
    }
    std::fs::create_dir_all(dir).map_err(|e| {
        Fail::usage(tr!(
            "creating « {} »: {e}",
            "création de « {} » : {e}",
            dir.display()
        ))
    })?;
    for (n, content) in FILES {
        let p = dir.join(n);
        std::fs::write(&p, content).map_err(|e| {
            Fail::usage(tr!(
                "writing « {} »: {e}",
                "écriture de « {} » : {e}",
                p.display()
            ))
        })?;
        eprintln!("{}", tr!("created: {}", "créé : {}", p.display()));
    }
    eprintln!(
        "{}",
        tr!(
            "try: skemo example.skemo --css custom.css",
            "essayer : skemo example.skemo --css custom.css"
        )
    );
    Ok(())
}
