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
    ("", "quiet", "N'afficher que les erreurs"),
    ("", "color", "Couleurs dans les messages"),
    (
        "",
        "lang",
        "Langue des messages et de l'aide (par défaut : celle du système ; SKEMO_LANG, LC_ALL, LC_MESSAGES, LANGUAGE, LANG)",
    ),
    (
        "render",
        "",
        "Rendu d'un schéma ; le format est déduit de l'extension de la sortie (PNG par défaut). C'est la commande par défaut : `skemo demo.skemo` écrit `demo.png`",
    ),
    ("check", "", "Vérifie des fichiers sans les rendre"),
    (
        "fmt",
        "",
        "Formatage canonique (indentation, ordre des attributs, marqueurs @)",
    ),
    (
        "init",
        "",
        "Crée un exemple `example.skemo` et un `custom.css` commenté (n'écrase rien)",
    ),
    ("init", "dir", "Dossier de destination (créé si besoin)"),
    (
        "watch",
        "",
        "Aperçu en direct : re-rendu à chaque sauvegarde, rechargement automatique du navigateur",
    ),
    (
        "layout",
        "",
        "Placement complet au format layout.json (sur la sortie standard)",
    ),
    (
        "extract",
        "",
        "Récupère la source et les feuilles de style embarquées dans une image rendue (PNG, SVG, WebP, GIF, APNG, HTML) pour les modifier",
    ),
    ("extract", "image", "Image ou page rendue"),
    (
        "extract",
        "output",
        "Dossier de destination (par défaut : celui de l'image)",
    ),
    ("extract", "force", "Écraser les fichiers existants"),
    (
        "extract",
        "stdout",
        "Écrire seulement la source, sur la sortie standard",
    ),
    (
        "fit",
        "",
        "Propose des réglages (dir, wrap) qui affichent le schéma plus grand dans une boîte cible (rapport ou taille)",
    ),
    ("fit", "file", "Fichier source"),
    (
        "fit",
        "target",
        "Boîte cible : un rapport largeur:hauteur (16:9, 4:3, 1.5) ou une taille en px (1280x720) ; par défaut : diagram(fit=…)",
    ),
    (
        "fit",
        "write",
        "Écrire la meilleure proposition dans le fichier",
    ),
    (
        "fit",
        "css",
        "CSS perso, appliqué après le thème (répétable)",
    ),
    ("fit", "theme", "Thème du rendu"),
    (
        "fit",
        "base_theme",
        "Thème de base remplaçant le thème par défaut",
    ),
    (
        "fit",
        "font_scale",
        "Multiplier toutes les tailles de texte du thème, comme pour render",
    ),
    (
        "icons",
        "",
        "Liste les icônes intégrées (Lucide), filtrées par une recherche sur les noms et les mots-clés",
    ),
    (
        "icons",
        "query",
        "Texte à chercher dans les noms et les mots-clés (toutes les icônes sans lui)",
    ),
    ("icons", "json", "Écrit un tableau JSON de `{name, tags}`"),
    ("theme", "", "Thème par défaut"),
    (
        "licences",
        "",
        "Licences et mentions de copyright de ce que le binaire embarque : polices, icônes, crates Rust",
    ),
    (
        "theme",
        "print",
        "Écrit le thème par défaut (point de départ d'un thème perso)",
    ),
    ("render", "file", "Fichier source (`-` = entrée standard)"),
    (
        "render",
        "output",
        "Fichier de sortie (.html, .svg, .png, .gif, .apng, .webp ; `-` = sortie standard, avec --format). Par défaut : à côté du source, même nom, en PNG (ou selon --format)",
    ),
    ("render", "format", "Format, si l'extension ne suffit pas"),
    (
        "render",
        "css",
        "CSS perso, appliqué après le thème (répétable)",
    ),
    (
        "render",
        "theme",
        "Thème du rendu (remplace diagram(theme=…))",
    ),
    ("render", "scale", "Échelle du PNG (et du GIF ou de l'APNG)"),
    ("render", "no_fonts", "HTML : ne pas embarquer les polices"),
    (
        "render",
        "font_scale",
        "Multiplier toutes les tailles de texte du thème (1.4 pour une slide projetée) ; règle `--font-scale`",
    ),
    (
        "render",
        "embed_fonts",
        "SVG : embarquer les polices utilisées (environ 0,8 Mo par graisse), pour les lecteurs sans Inter",
    ),
    (
        "render",
        "font",
        "Un fichier de police (TrueType ou OpenType) à utiliser en plus des polices embarquées, partout où le CSS nomme sa famille (répétable : un fichier par graisse ou par style)",
    ),
    (
        "render",
        "no_source",
        "Ne pas embarquer la source et les feuilles de style dans la sortie (elles le sont par défaut, pour rouvrir l'image : skemo extract)",
    ),
    (
        "render",
        "deny_warnings",
        "Échouer si un avertissement est émis",
    ),
    (
        "render",
        "base_theme",
        "Thème de base remplaçant le thème par défaut",
    ),
    (
        "render",
        "transparent",
        "Fond transparent (SVG, PNG, APNG et planche) : pas de rectangle de fond ; sans effet en HTML et en GIF",
    ),
    (
        "render",
        "contrast",
        "Contraste des textes : `high` assombrit (ou éclaircit) chaque texte sous le seuil WCAG (4,5:1 ; 3:1 sur un aplat d'accent), sans toucher au placement",
    ),
    (
        "render",
        "step",
        "Rendu de l'état à cette étape (valeur telle qu'écrite : 2.5)",
    ),
    (
        "render",
        "animated",
        "PNG : un fichier par état (nom.00.png…) et nom.states.json",
    ),
    (
        "render",
        "board",
        "PNG : planche des étapes (un état par rangée)",
    ),
    (
        "render",
        "hold",
        "GIF et APNG : maintien de chaque état, en secondes",
    ),
    (
        "render",
        "transition",
        "GIF et APNG : durée des transitions, en secondes",
    ),
    ("render", "fps", "GIF et APNG : images par seconde"),
    ("render", "no_loop", "GIF et APNG : ne pas boucler"),
    (
        "render",
        "final_hold",
        "GIF et APNG : maintien de la dernière image, en secondes",
    ),
    ("check", "files", "Fichiers à vérifier"),
    (
        "check",
        "deny_warnings",
        "Échouer si un avertissement est émis",
    ),
    (
        "check",
        "contrast",
        "Vérifie aussi le contraste des textes dans les 3 thèmes (light, dark, print)",
    ),
    ("check", "css", "CSS perso (pour --contrast)"),
    ("check", "base_theme", "Thème de base (pour --contrast)"),
    (
        "fmt",
        "check",
        "Ne rien écrire : code 1 si un fichier n'est pas formaté",
    ),
    (
        "watch",
        "output",
        "Écrit aussi le rendu dans ce fichier à chaque changement",
    ),
    ("watch", "port", "Port du serveur local"),
    ("layout", "json", "Sortie JSON (seul format disponible)"),
];

/// Applies the help texts of `lang` to `cmd` and its subcommands (English is the base).
pub fn localize(cmd: Command, lang: Lang) -> Command {
    if lang != Lang::Fr {
        return cmd;
    }
    walk(cmd, "")
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
            cmd = cmd.mut_arg(id, |a| a.help(t));
        }
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
            if matches!(id.as_str(), "help" | "version") || a.get_help().is_none() {
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
            fr.contains("Rendu d'un schéma") && fr.contains("N'afficher que les erreurs"),
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
                && render.contains("GIF et APNG : images par seconde"),
            "{render}"
        );
    }
}
