//! `skemo lint --fit`: settings (`dir`, `wrap`) that show a diagram larger in a target box, arrows
//! kept readable (`skemo_engine::fit`). Prints them, best first; with `--fix` the best one goes
//! into the file.

use crate::args::LintArgs;
use crate::common::{Fail, Inputs, io, load};
use crate::out::Out;
use skemo_engine::fit;
use skemo_lang::tr;
use std::io::Write as _;

/// `target`: the box given to `--fit`, empty for the diagram's own `diagram(fit=…)`.
pub fn run(out: &Out, a: &LintArgs, file: &str, target: &str) -> Result<(), Fail> {
    let inputs = Inputs::new(file, &a.css, a.base_theme.as_ref()).font_scale(a.font_scale);
    let loaded = load(&inputs)?;
    let (src, opts) = (loaded.src, loaded.options);
    let raw = Some(target.to_string())
        .filter(|t| !t.is_empty())
        .or_else(|| {
            skemo_lang::parse(&src)
                .model
                .option("fit")
                .map(String::from)
        })
        .ok_or_else(|| {
            Fail::usage(tr!(
                "no target: give --fit 16:9 or write diagram(fit=\"16:9\")",
                "pas de cible : donner --fit 16:9 ou écrire diagram(fit=\"16:9\")"
            ))
        })?;
    let target = fit::parse_target(&raw).ok_or_else(|| {
        Fail::usage(tr!(
            "--fit « {raw} »: a ratio like 16:9 or a box like 1280x720 is expected",
            "--fit « {raw} » : un rapport comme 16:9 ou une boîte comme 1280x720 est attendu"
        ))
    })?;
    let shape = fit::shape_of(&src, &opts).ok_or_else(|| {
        Fail(
            1,
            Some(tr!(
                "the diagram does not build: run skemo lint",
                "le schéma ne se construit pas : lancer skemo lint"
            )),
        )
    })?;
    let size = shape.size;
    let now = target.scale(size);
    let bends = |n: usize| tr!(", {n} bends", ", {n} coudes");
    let px = |sz: (f64, f64)| {
        fit::text_px(target, sz).map_or(String::new(), |p| {
            tr!(", text {:.1} px", ", texte {:.1} px", p)
        })
    };
    let list = fit::suggestions(&src, &opts, target);
    let mut text = tr!(
        "now {:.0}×{:.0} px ({:.2} wide for 1 high){}{}, target {raw} ({:.2})\n",
        "actuellement {:.0}×{:.0} px ({:.2} de large pour 1 de haut){}{}, cible {raw} ({:.2})\n",
        size.0,
        size.1,
        size.0 / size.1,
        px(size),
        bends(shape.bends),
        target.ratio
    );
    for (i, s) in list.iter().take(5).enumerate() {
        let what: Vec<String> = s.settings.iter().map(fit::Setting::describe).collect();
        text.push_str(&tr!(
            "{}. {}  → {:.2} wide for 1 high, text ×{:.2}{}{}\n",
            "{}. {}  → {:.2} de large pour 1 de haut, texte ×{:.2}{}{}\n",
            i + 1,
            what.join(" + "),
            s.aspect(),
            s.scale / now,
            px(s.size),
            bends(s.bends)
        ));
    }
    if list.is_empty() {
        text.push_str(&tr!(
            "no setting of dir or wrap shows the text larger with arrows as readable\n",
            "aucun réglage de dir ou wrap n'affiche le texte plus grand avec des flèches aussi lisibles\n"
        ));
    }
    let _ = std::io::stdout().write_all(text.as_bytes());
    if a.canon.fix && !a.canon.dry {
        write_best(out, file, &src, list.first())?;
    }
    Ok(())
}

fn write_best(
    out: &Out,
    file: &str,
    src: &str,
    best: Option<&fit::Suggestion>,
) -> Result<(), Fail> {
    if file == "-" {
        return Err(Fail::usage(tr!(
            "--fit --fix needs a file, not the standard input",
            "--fit --fix demande un fichier, pas l'entrée standard"
        )));
    }
    let Some(best) = best else {
        out.warn(&tr!("nothing written", "rien d'écrit"));
        return Ok(());
    };
    let new = fit::apply(src, &best.settings).ok_or_else(|| {
        Fail::internal(tr!(
            "the suggestion could not be written",
            "la proposition n'a pas pu être écrite"
        ))
    })?;
    std::fs::write(file, new)
        .map_err(|e| io(&e))
        .map_err(|e| Fail::usage(format!("{file}: {e}")))
}
