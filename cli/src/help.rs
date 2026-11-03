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
        "Des schémas écrits en texte, dessinés en SVG, PNG, HTML, GIF, APNG ou WebP",
    ),
    ("", "help", "Afficher cette aide"),
    ("", "version", "Afficher la version"),
    ("", "quiet", "N'afficher que les erreurs"),
    (
        "",
        "licences",
        "Afficher les licences des polices, icônes et bibliothèques incluses",
    ),
    ("init", "", "Créer un schéma d'exemple pour démarrer"),
    ("init", "file", "Schéma à créer (défaut : diagram.skemo)"),
    (
        "init",
        "css",
        "Écrire aussi une courte feuille de style, utilisée par le schéma (nom par défaut : `skemo_style.css`)",
    ),
    (
        "init",
        "css_full",
        "Idem, avec tout le thème par défaut, à modifier",
    ),
    (
        "lint",
        "",
        "Vérifier des schémas, et les remettre en forme avec --fix",
    ),
    ("lint", "files", "Schémas à vérifier"),
    (
        "lint",
        "fix",
        "Les réécrire en forme canonique (avec --fit : appliquer aussi le meilleur réglage)",
    ),
    (
        "lint",
        "dry",
        "Avec --fix : ne rien modifier, code 1 si un fichier changerait",
    ),
    ("lint", "deny_warnings", "Code 1 au moindre avertissement"),
    (
        "lint",
        "contrast",
        "Vérifier aussi le contraste des textes (thèmes light, dark et print)",
    ),
    (
        "lint",
        "fit",
        "Proposer dir ou wrap pour agrandir le texte dans un cadre : 16:9, 1280x720 (défaut : diagram(fit=…))",
    ),
    (
        "lint",
        "font_scale",
        "Facteur de taille du texte pour --fit",
    ),
    (
        "lint",
        "css",
        "Feuille de style supplémentaire, après celles du schéma (répétable)",
    ),
    (
        "lint",
        "base_theme",
        "Feuille de style remplaçant le thème par défaut",
    ),
    (
        "render",
        "",
        "Dessiner un schéma (la commande par défaut : skemo demo.skemo)",
    ),
    ("render", "file", "Schéma (- lit l'entrée standard)"),
    (
        "render",
        "output",
        "Fichier de sortie ; son extension fixe le format (défaut : demo.png à côté de demo.skemo)",
    ),
    (
        "render",
        "format",
        "Format si l'extension ne le dit pas : html, svg, png, gif, apng, webp",
    ),
    (
        "render",
        "scale",
        "Densité de pixels du PNG, GIF et APNG (défaut : 2)",
    ),
    ("render", "transparent", "Sans fond (SVG, PNG, APNG)"),
    (
        "render",
        "no_source",
        "Ne pas embarquer la source (skemo extract ne pourra plus la relire)",
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
        "Fichier de police pour une famille nommée dans le CSS (répétable)",
    ),
    (
        "render",
        "watch",
        "Aperçu en direct dans le navigateur, redessiné à chaque enregistrement ; n'écrit de fichier qu'avec -o",
    ),
    (
        "render",
        "port",
        "Port de l'aperçu en direct (défaut : 8710)",
    ),
    ("render", "deny_warnings", "Code 1 au moindre avertissement"),
    (
        "render",
        "css",
        "Feuille de style supplémentaire, après celles du schéma (répétable)",
    ),
    (
        "render",
        "base_theme",
        "Feuille de style remplaçant le thème par défaut",
    ),
    (
        "render",
        "theme",
        "light, dark, print ou auto ; remplace diagram(theme=…)",
    ),
    (
        "render",
        "contrast",
        "high : foncer les textes sous le contraste WCAG, placement inchangé (défaut : standard)",
    ),
    (
        "render",
        "font_scale",
        "Facteur de taille du texte (1.4 pour une diapositive)",
    ),
    (
        "render",
        "step",
        "Dessiner l'état à cette étape, telle qu'écrite (2.5)",
    ),
    (
        "render",
        "animated",
        "PNG : un fichier par état, plus nom.states.json",
    ),
    ("render", "board", "PNG : toutes les étapes sur une planche"),
    (
        "render",
        "motion",
        "SVG : animer les flèches en mouvement (.flow, .pulse)",
    ),
    (
        "render",
        "hold",
        "GIF, APNG : secondes sur chaque état (défaut : 1.6)",
    ),
    (
        "render",
        "transition",
        "GIF, APNG : secondes par transition (défaut : 0.45)",
    ),
    (
        "render",
        "fps",
        "GIF, APNG : images par seconde (défaut : 20)",
    ),
    ("render", "no_loop", "GIF, APNG : jouer une seule fois"),
    (
        "render",
        "final_hold",
        "GIF, APNG : secondes sur la dernière image (défaut : 3)",
    ),
    ("extract", "", "Retrouver la source d'une image rendue"),
    (
        "extract",
        "image",
        "Image ou page rendue (PNG, SVG, WebP, GIF, APNG, HTML)",
    ),
    (
        "extract",
        "output",
        "Dossier où écrire (défaut : celui de l'image)",
    ),
    ("extract", "force", "Écraser les fichiers existants"),
    (
        "extract",
        "stdout",
        "N'écrire que la source, sur la sortie standard",
    ),
    ("layout", "", "Écrire les positions et les tracés en JSON"),
    ("layout", "file", "Schéma"),
    ("layout", "json", "Sortie JSON (le seul format)"),
    (
        "layout",
        "css",
        "Feuille de style supplémentaire, après celles du schéma (répétable)",
    ),
    (
        "layout",
        "theme",
        "light, dark, print ou auto ; remplace diagram(theme=…)",
    ),
    (
        "layout",
        "base_theme",
        "Feuille de style remplaçant le thème par défaut",
    ),
    (
        "icons",
        "",
        "Lister les icônes intégrées, filtrées par une recherche (pour les éditeurs et la complétion ; absente de l'aide)",
    ),
    (
        "icons",
        "query",
        "Mots à chercher dans les noms et les mots-clés",
    ),
    ("icons", "json", "Écrire un tableau JSON de `{name, tags}`"),
    (
        "completions",
        "",
        "Écrire un script de complétion du shell : `eval \"$(skemo completions bash)\"` dans ~/.bashrc (absente de l'aide)",
    ),
    (
        "completions",
        "shell",
        "bash, zsh, fish, elvish ou powershell",
    ),
];

/// Applies the help texts of `lang` to `cmd` and its subcommands (English is the base).
pub fn localize(cmd: Command, lang: Lang) -> Command {
    let cmd = if lang == Lang::Fr {
        walk(cmd, "").after_help(EXAMPLES_FR).override_usage(
            "skemo <FICHIER> [OPTIONS]   (= skemo render)\n              skemo <COMMANDE> [OPTIONS]\n              skemo --licences",
        )
    } else {
        cmd.after_help(EXAMPLES_EN)
    };
    compact(cmd, lang)
}

/// Examples under the main help.
const EXAMPLES_EN: &str = "Examples:
  skemo init demo.skemo --css    a sample diagram and its stylesheet
  skemo lint demo.skemo          check it
  skemo demo.skemo               draw demo.png
  skemo demo.skemo -o demo.svg   another format, from the extension
  skemo demo.skemo --watch       live preview in the browser

Documentation: https://skemo.net/docs/";

const EXAMPLES_FR: &str = "Exemples :
  skemo init demo.skemo --css    un schéma d'exemple et sa feuille de style
  skemo lint demo.skemo          le vérifier
  skemo demo.skemo               dessiner demo.png
  skemo demo.skemo -o demo.svg   un autre format, d'après l'extension
  skemo demo.skemo --watch       aperçu en direct dans le navigateur

Documentation : https://skemo.net/fr/docs/";

/// Section headings in French (`args::GENERIC`, `OUTPUT`…); the space before the colon clap
/// adds is French typography.
const HEADINGS: &[(&str, &str)] = &[
    (crate::args::GENERIC, "Options générales "),
    (crate::args::OUTPUT, "Sortie "),
    (crate::args::STYLE, "Style "),
    (crate::args::ANIMATION, "Animation "),
];

/// Names of the values in French (`--css <FICHIER>`).
const VALUES_FR: &[(&str, &str)] = &[
    ("FILE", "FICHIER"),
    ("FILES", "FICHIERS"),
    ("DIR", "DOSSIER"),
    ("SECONDS", "SECONDES"),
    ("FACTOR", "FACTEUR"),
    ("BOX", "CADRE"),
    ("THEME", "THÈME"),
    ("LEVEL", "NIVEAU"),
    ("STEP", "ÉTAPE"),
    ("QUERY", "RECHERCHE"),
];

/// Every help page alike: one line per option (`--help` would otherwise print the long form, one
/// paragraph per option), defaults and values written in the texts (clap prints them in English),
/// headings and value names in the language of the messages.
fn compact(cmd: Command, lang: Lang) -> Command {
    let fr = lang == Lang::Fr;
    let (usage, commands, options, arguments) = if fr {
        ("Utilisation :", "Commandes ", "Options ", "Arguments ")
    } else {
        ("Usage:", "Commands", "Options", "Arguments")
    };
    let mut cmd = cmd
        .long_about(None)
        .next_line_help(false)
        .subcommand_help_heading(commands)
        .help_template(format!(
            "{{about-with-newline}}\n{usage} {{usage}}\n\n{{all-args}}{{after-help}}"
        ));
    let ids: Vec<String> = cmd
        .get_arguments()
        .map(|a| a.get_id().to_string())
        .collect();
    for (order, id) in ids.into_iter().enumerate() {
        cmd = cmd.mut_arg(id, |a| {
            let name = a
                .get_value_names()
                .and_then(|n| n.first())
                .map(ToString::to_string)
                .or_else(|| {
                    a.is_positional()
                        .then(|| a.get_id().to_string().to_uppercase())
                });
            // every option under a heading of ours, so that it is written in the right language
            let heading = match a.get_help_heading() {
                Some(_) => None,
                None if a.is_positional() => Some(arguments),
                None => Some(options),
            };
            let a = a
                .long_help(None)
                .hide_default_value(true)
                .hide_possible_values(true);
            // under a heading of ours, positional arguments keep their order (clap would sort them)
            let a = if a.is_positional() {
                a.display_order(order)
            } else {
                a
            };
            let a = match heading {
                Some(h) => a.help_heading(h),
                None => a,
            };
            match name.and_then(|n| VALUES_FR.iter().find(|(en, _)| *en == n)) {
                Some((_, v)) if fr => a.value_name(*v),
                _ => a,
            }
        });
    }
    let subs: Vec<String> = cmd
        .get_subcommands()
        .map(|c| c.get_name().to_string())
        .collect();
    for name in subs {
        cmd = cmd.mut_subcommand(name, |c| compact(c, lang));
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
            fr.contains("Dessiner un schéma")
                && fr.contains("N'afficher que les erreurs")
                && fr.contains("Options générales :")
                && fr.contains("Commandes :")
                && fr.contains("Utilisation :")
                && fr.contains("Exemples :")
                && !fr.contains("[default"),
            "{fr}"
        );
        let en = localize(crate::args::Cli::command(), Lang::En)
            .render_long_help()
            .to_string();
        assert!(
            en.contains("Draw a diagram") && en.contains("Show errors only"),
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
                && render.contains("GIF, APNG : images par seconde")
                && render.contains("<SECONDES>")
                && !render.contains("[possible values"),
            "{render}"
        );
    }
}
