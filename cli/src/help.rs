//! Help texts in French. The English ones are the doc comments of `args.rs`; when the language
//! is French, [`localize`] replaces them at run time (clap derives its help from the comments, at
//! compile time, so it cannot translate by itself).

use clap::Command;
use skemo_lang::i18n::Lang;

/// `(path, id, text)`: `path` is the command path (empty for the program, `render`…), `id` the
/// argument name, or empty for the command's own description.
const FR: &[(&str, &str, &str)] = &[
    (
        "",
        "",
        "Diagrammes texte (skemo) : rendu HTML, SVG, PNG, GIF, APNG et WebP",
    ),
    ("", "help", "Afficher l'aide"),
    ("", "version", "Afficher la version"),
    ("", "quiet", "N'afficher que les erreurs"),
    (
        "",
        "licences",
        "Afficher les licences de ce que le binaire embarque (polices, icônes, crates)",
    ),
    ("init", "", "Crée un schéma d'exemple (n'écrase rien)"),
    ("init", "file", "Schéma à créer"),
    (
        "init",
        "css",
        "Aussi une courte feuille de style commentée, incluse par le schéma (`skemo_style.css`)",
    ),
    (
        "init",
        "css_full",
        "Idem, avec tout le thème par défaut à modifier",
    ),
    (
        "lint",
        "",
        "Vérifie des fichiers (diagnostics), peut les réécrire en forme canonique",
    ),
    ("lint", "files", "Fichiers à vérifier"),
    (
        "lint",
        "fix",
        "Réécrit les fichiers en forme canonique ; avec --fit, applique aussi le meilleur réglage",
    ),
    (
        "lint",
        "dry",
        "Avec --fix : ne rien écrire, code 1 si un fichier changerait",
    ),
    ("lint", "deny_warnings", "Code 1 au moindre avertissement"),
    (
        "lint",
        "contrast",
        "Vérifie aussi le contraste des textes (thèmes light, dark, print)",
    ),
    (
        "lint",
        "fit",
        "Réglages (dir, wrap) qui agrandissent le texte dans un cadre : 16:9, 1280x720 (défaut : diagram(fit=…))",
    ),
    (
        "lint",
        "font_scale",
        "Facteur de taille du texte pour --fit, comme pour render",
    ),
    (
        "lint",
        "css",
        "Feuille de style en plus, après celles du schéma (répétable)",
    ),
    ("lint", "base_theme", "Thème remplaçant celui par défaut"),
    (
        "render",
        "",
        "Rendu d'un schéma, format déduit de l'extension de sortie ; la commande par défaut",
    ),
    ("render", "file", "Fichier source (`-` : entrée standard)"),
    (
        "render",
        "output",
        "Fichier de sortie, son extension choisit le format (défaut : nom de la source, .png)",
    ),
    (
        "render",
        "format",
        "Format, quand l'extension ne le dit pas",
    ),
    ("render", "scale", "Échelle en pixels du PNG, GIF et APNG"),
    ("render", "transparent", "Sans fond (SVG, PNG, APNG)"),
    (
        "render",
        "no_source",
        "Ne pas embarquer la source dans la sortie (voir skemo extract)",
    ),
    ("render", "no_fonts", "HTML : ne pas embarquer les polices"),
    (
        "render",
        "embed_fonts",
        "SVG : embarquer les polices (environ 0,8 Mo par graisse)",
    ),
    (
        "render",
        "font",
        "Fichier de police pour une famille nommée par le CSS (répétable)",
    ),
    (
        "render",
        "watch",
        "Aperçu en direct dans le navigateur ; n'écrit un fichier qu'avec -o",
    ),
    ("render", "port", "Port de l'aperçu en direct"),
    ("render", "deny_warnings", "Code 1 au moindre avertissement"),
    (
        "render",
        "css",
        "Feuille de style en plus, après celles du schéma (répétable)",
    ),
    ("render", "base_theme", "Thème remplaçant celui par défaut"),
    (
        "render",
        "theme",
        "Variante du thème, prioritaire sur diagram(theme=…)",
    ),
    (
        "render",
        "contrast",
        "high : fonce les textes sous le contraste WCAG, placement inchangé",
    ),
    (
        "render",
        "font_scale",
        "Facteur de taille du texte (1.4 pour une diapositive)",
    ),
    (
        "render",
        "step",
        "L'état à cette étape (telle qu'écrite : 2.5)",
    ),
    (
        "render",
        "animated",
        "PNG : un fichier par état, et nom.states.json",
    ),
    ("render", "board", "PNG : toutes les étapes sur une planche"),
    (
        "render",
        "motion",
        "SVG : animer les flèches en mouvement (.flow, .pulse) ; fixe sinon",
    ),
    ("render", "hold", "GIF, APNG : secondes par état"),
    (
        "render",
        "transition",
        "GIF, APNG : secondes par transition",
    ),
    ("render", "fps", "GIF, APNG : images par seconde"),
    ("render", "no_loop", "GIF, APNG : jouer une fois"),
    (
        "render",
        "final_hold",
        "GIF, APNG : secondes sur la dernière image",
    ),
    (
        "extract",
        "",
        "Récupère la source et les feuilles de style embarquées dans une image rendue",
    ),
    ("extract", "image", "Image ou page rendue"),
    (
        "extract",
        "output",
        "Dossier de destination (défaut : celui de l'image)",
    ),
    ("extract", "force", "Écraser les fichiers existants"),
    (
        "extract",
        "stdout",
        "Seulement la source, sur la sortie standard",
    ),
    (
        "layout",
        "",
        "Placement complet en JSON, sur la sortie standard",
    ),
    ("layout", "file", "Fichier source"),
    ("layout", "json", "Sortie JSON (le seul format)"),
    (
        "layout",
        "css",
        "Feuille de style en plus, après celles du schéma (répétable)",
    ),
    (
        "layout",
        "theme",
        "Variante du thème, prioritaire sur diagram(theme=…)",
    ),
    ("layout", "base_theme", "Thème remplaçant celui par défaut"),
    (
        "icons",
        "",
        "Les icônes intégrées, filtrées par une recherche (pour les éditeurs et la complétion ; absente de l'aide)",
    ),
    (
        "icons",
        "query",
        "Texte à chercher dans les noms et les mots-clés",
    ),
    ("icons", "json", "Un tableau JSON de `{name, tags}`"),
    (
        "completions",
        "",
        "Script de complétion du shell : `eval \"$(skemo completions bash)\"` dans ~/.bashrc (absente de l'aide)",
    ),
    (
        "completions",
        "shell",
        "bash, zsh, fish, elvish ou powershell",
    ),
];

/// Applies the help texts of `lang` to `cmd` and its subcommands (English is the base).
pub fn localize(cmd: Command, lang: Lang) -> Command {
    compact(if lang == Lang::Fr { walk(cmd, "") } else { cmd })
}

/// Section headings in French (`args::GENERIC`, `OUTPUT`…).
const HEADINGS: &[(&str, &str)] = &[
    (crate::args::GENERIC, "Options générales"),
    (crate::args::OUTPUT, "Sortie"),
    (crate::args::STYLE, "Style"),
    (crate::args::ANIMATION, "Animation"),
];

/// One line per option: the short help only (`--help` would otherwise print the long form, one
/// paragraph per option), for every command alike.
fn compact(cmd: Command) -> Command {
    let mut cmd = cmd.long_about(None).next_line_help(false);
    let ids: Vec<String> = cmd
        .get_arguments()
        .map(|a| a.get_id().to_string())
        .collect();
    for id in ids {
        cmd = cmd.mut_arg(id, |a| a.long_help(None));
    }
    let subs: Vec<String> = cmd
        .get_subcommands()
        .map(|c| c.get_name().to_string())
        .collect();
    for name in subs {
        cmd = cmd.mut_subcommand(name, compact);
    }
    cmd
}

fn walk(mut cmd: Command, path: &str) -> Command {
    let at = |id: &str| {
        FR.iter()
            .find(|(p, i, _)| *p == path && *i == id)
            .map(|e| e.2)
    };
    if let Some(t) = at("") {
        cmd = cmd.about(t);
    }
    let ids: Vec<String> = cmd
        .get_arguments()
        .map(|a| a.get_id().to_string())
        .collect();
    for id in ids {
        // global options are looked up on the program level wherever they appear
        let text = at(&id).or_else(|| {
            FR.iter()
                .find(|(p, i, _)| p.is_empty() && *i == id)
                .map(|e| e.2)
        });
        if let Some(t) = text {
            cmd = cmd.mut_arg(&id, |a| a.help(t));
        }
        cmd = cmd.mut_arg(&id, |a| {
            let fr = a
                .get_help_heading()
                .and_then(|h| HEADINGS.iter().find(|(en, _)| *en == h))
                .map(|(_, fr)| *fr);
            match fr {
                Some(h) => a.help_heading(h),
                None => a,
            }
        });
    }
    let subs: Vec<String> = cmd
        .get_subcommands()
        .map(|c| c.get_name().to_string())
        .collect();
    for name in subs {
        let sub_path = if path.is_empty() {
            name.clone()
        } else {
            format!("{path} {name}")
        };
        cmd = cmd.mut_subcommand(name, |c| walk(c, &sub_path));
    }
    cmd
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    /// Every argument and every command of the CLI has a French text, and no French text is
    /// orphaned (a renamed option would silently lose its translation).
    #[test]
    fn the_french_table_matches_the_cli() {
        let cmd = crate::args::Cli::command();
        let mut expected = vec![];
        collect(&cmd, "", &mut expected);
        for (p, i) in &expected {
            assert!(
                FR.iter().any(|(fp, fi, _)| fp == p && fi == i),
                "no French text for ({p:?}, {i:?})"
            );
        }
        for (p, i, _) in FR {
            assert!(
                expected.iter().any(|(ep, ei)| ep == p && ei == i),
                "orphan French text ({p:?}, {i:?})"
            );
        }
    }

    fn collect(cmd: &Command, path: &str, out: &mut Vec<(String, String)>) {
        if !path.is_empty() || cmd.get_about().is_some() {
            out.push((path.to_string(), String::new()));
        }
        for a in cmd.get_arguments() {
            let id = a.get_id().to_string();
            if a.get_help().is_none() {
                continue;
            }
            // global options belong to the program level
            if a.is_global_set() && !path.is_empty() {
                continue;
            }
            out.push((path.to_string(), id));
        }
        for c in cmd.get_subcommands().filter(|c| c.get_name() != "help") {
            let sub = if path.is_empty() {
                c.get_name().to_string()
            } else {
                format!("{path} {}", c.get_name())
            };
            collect(c, &sub, out);
        }
    }

    #[test]
    fn french_help_replaces_the_english_one() {
        let fr = localize(crate::args::Cli::command(), Lang::Fr)
            .render_long_help()
            .to_string();
        assert!(
            fr.contains("Rendu d'un schéma")
                && fr.contains("N'afficher que les erreurs")
                && fr.contains("Options générales"),
            "{fr}"
        );
        let en = localize(crate::args::Cli::command(), Lang::En)
            .render_long_help()
            .to_string();
        assert!(
            en.contains("Render a diagram") && en.contains("Only show errors"),
            "{en}"
        );
        let mut sub = localize(crate::args::Cli::command(), Lang::Fr);
        let render = sub
            .find_subcommand_mut("render")
            .unwrap()
            .render_long_help()
            .to_string();
        assert!(
            render.contains("Fichier de sortie")
                && render.contains("GIF, APNG : images par seconde"),
            "{render}"
        );
    }
}
