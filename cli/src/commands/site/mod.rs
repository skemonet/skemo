//! `skemo site`: a folder of diagrams turned into a website (D-097). Each `.skemo` becomes a page
//! at the same place (`shop/api.skemo` → `shop/api.html`) that shows its drawing with pan and zoom;
//! a card, zone or arrow linked to another diagram (`href="api.skemo"`, `href="../net.skemo#fw"`)
//! opens that diagram's page. A breadcrumb (from `index.skemo`, or from the diagrams no other links
//! to), the list of every diagram, and what a view opens and is opened from let a reader go from the
//! big picture to the details of a part and back. A search box finds a card in every diagram and
//! opens its page centred on it.
//!
//! Every diagram is read and drawn before anything is written: one refused diagram refuses the
//! site, like `render`. A link to a diagram that is not in the folder is a warning. Other relative
//! links (`docs/fw.md`) are copied beside the pages when the file is in the folder.

mod page;

use crate::args::{SiteArgs, ThemeArg};
use crate::common::{Fail, Inputs, io, prepare, write_out};
use crate::out::Out;
use skemo_engine::Diagram;
use skemo_lang::model::diagram_link;
use skemo_lang::{Diagnostic, Span, tr};
use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::path::{Path, PathBuf};

/// Most diagrams a site is built from: a guard against a folder given by mistake (a home folder).
const MAX_DIAGRAMS: usize = 2000;

/// One drawing of a diagram: the theme it is drawn in (`None` when the site has one theme) and
/// what the page needs from it.
pub(crate) struct Drawing {
    pub theme: Option<&'static str>,
    pub svg: String,
    pub background: String,
    pub thumb: Option<Vec<u8>>,
}

/// A diagram of the site.
pub(crate) struct Doc {
    /// Path from the folder, with `/` (`shop/api.skemo`).
    pub rel: String,
    /// Its page, from the site's root (`shop/api.html`).
    pub page: String,
    pub title: String,
    pub drawings: Vec<Drawing>,
    /// Name of the stylesheet with its fonts, in `assets/`.
    pub fonts: String,
    /// Diagrams it opens, in writing order, once each.
    pub opens: Vec<usize>,
    /// Its cards, for the search.
    pub cards: Vec<Card>,
}

/// A card as the search sees it: what a reader may type, and where it stands.
pub(crate) struct Card {
    /// `data-id` of the card in the drawing (the page's fragment).
    pub id: String,
    pub title: String,
    /// `label=` (the category), or empty.
    pub category: String,
    /// `tag=`, or empty.
    pub tag: String,
    /// Title of the nearest drawn zone holding it, or empty.
    pub zone: String,
    /// Its lines, joined, cut at `LINES` characters.
    pub lines: String,
    /// For a note (`.note` with a `.link`), the title of the card it annotates.
    pub note_of: String,
}

/// Characters of a card's lines kept for the search: enough for a name or an example, not a page.
const LINES: usize = 160;

pub fn run(out: &Out, a: &SiteArgs) -> Result<(), Fail> {
    let (dir, output) = folders(a)?;
    let rels = scan(&dir, &output)?;
    if rels.is_empty() {
        return Err(Fail::usage(tr!(
            "no .skemo file in `{}`",
            "aucun fichier .skemo dans « {} »",
            a.dir.display()
        )));
    }
    check_pages(&rels)?;
    let index: HashMap<&str, usize> = rels
        .iter()
        .enumerate()
        .map(|(k, r)| (r.as_str(), k))
        .collect();
    // light and dark unless one theme is asked for: a page follows its reader's system, with a switch
    let themes: Vec<Option<ThemeArg>> = if matches!(a.theme, None | Some(ThemeArg::Auto)) {
        vec![Some(ThemeArg::Light), Some(ThemeArg::Dark)]
    } else {
        vec![a.theme]
    };
    let overview = !index.contains_key("index.skemo");
    let mut docs = Vec::new();
    let mut copies = BTreeSet::new();
    let mut refused = false;
    let mut warnings = 0;
    let mut fonts = BTreeMap::new();
    for rel in &rels {
        let file = a.dir.join(rel).to_string_lossy().into_owned();
        match build(out, a, &file, rel, &themes, &index, &dir, overview) {
            Ok(b) => {
                warnings += b.warnings;
                copies.extend(b.copies);
                let n = fonts.len();
                let name = fonts
                    .entry(b.fonts)
                    .or_insert_with(|| format!("fonts-{n}.css"))
                    .clone();
                docs.push(Doc {
                    rel: rel.clone(),
                    page: page_of(rel),
                    title: b.title,
                    drawings: b.drawings,
                    fonts: name,
                    opens: b.opens,
                    cards: b.cards,
                });
            }
            Err(Fail(1, msg)) => {
                if let Some(m) = msg {
                    out.error(&m);
                }
                refused = true;
            }
            Err(f) => return Err(f),
        }
    }
    if refused {
        return Err(Fail(
            1,
            Some(tr!(
                "the site is not written: fix the diagrams above",
                "le site n'est pas écrit : corriger les schémas ci-dessus"
            )),
        ));
    }
    if a.deny_warnings && warnings > 0 {
        return Err(Fail(
            1,
            Some(tr!(
                "{warnings} warning(s) with --deny-warnings",
                "{warnings} avertissement(s) avec --deny-warnings"
            )),
        ));
    }
    let site = Site {
        title: a.title.clone().unwrap_or_else(|| {
            dir.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("skemo")
                .to_string()
        }),
        auto: themes.len() > 1,
        dark: a.theme == Some(ThemeArg::Dark),
        overview,
        crumbs: crumbs(&docs, overview),
        from: from(&docs),
        docs,
    };
    write_site(a, &dir, &site, &fonts, &copies)?;
    let at = a.output.join("index.html");
    out.note(&tr!(
        "{} diagram(s) → {}",
        "{} schéma(s) → {}",
        site.docs.len(),
        at.display()
    ));
    Ok(())
}

/// The folder of diagrams and the output folder, both absolute; the output is not the folder.
fn folders(a: &SiteArgs) -> Result<(PathBuf, PathBuf), Fail> {
    let dir = std::fs::canonicalize(&a.dir)
        .map_err(|e| io(&e))
        .map_err(|e| {
            Fail::usage(tr!(
                "folder `{}`: {e}",
                "dossier « {} » : {e}",
                a.dir.display()
            ))
        })?;
    if !dir.is_dir() {
        return Err(Fail::usage(tr!(
            "`{}` is not a folder: skemo site takes a folder of diagrams",
            "« {} » n'est pas un dossier : skemo site prend un dossier de schémas",
            a.dir.display()
        )));
    }
    let output = absolute(&a.output);
    if output == dir {
        return Err(Fail::usage(tr!(
            "the site cannot be written into the folder of diagrams itself: give another folder with -o",
            "le site ne peut pas être écrit dans le dossier des schémas lui-même : donner un autre dossier avec -o"
        )));
    }
    Ok((dir, output))
}

/// The site as the pages see it.
pub(crate) struct Site {
    pub title: String,
    /// Two drawings per diagram, light and dark, and a switch (`--theme auto`).
    pub auto: bool,
    /// The page's own colours dark (`--theme dark`).
    pub dark: bool,
    /// No `index.skemo`: `index.html` is a page of thumbnails.
    pub overview: bool,
    pub docs: Vec<Doc>,
    /// For each diagram, the diagrams above it, from the top.
    pub crumbs: Vec<Vec<usize>>,
    /// For each diagram, the diagrams that open it.
    pub from: Vec<Vec<usize>>,
}

/// What one diagram gives once drawn.
struct Built {
    title: String,
    drawings: Vec<Drawing>,
    fonts: String,
    opens: Vec<usize>,
    /// Files of the folder its links point to, to copy (paths from the folder).
    copies: Vec<String>,
    warnings: usize,
    cards: Vec<Card>,
}

#[allow(clippy::too_many_arguments)]
fn build(
    out: &Out,
    a: &SiteArgs,
    file: &str,
    rel: &str,
    themes: &[Option<ThemeArg>],
    index: &HashMap<&str, usize>,
    dir: &Path,
    thumbs: bool,
) -> Result<Built, Fail> {
    let quiet = Out {
        quiet: true,
        ..Out::default()
    };
    let mut drawings = Vec::new();
    let mut built = None;
    for (k, theme) in themes.iter().enumerate() {
        // the diagnostics once: the second drawing only changes the colours
        let o = if k == 0 { out } else { &quiet };
        let inputs = Inputs::new(file, &a.css, a.base_theme.as_ref()).theme(*theme);
        let (mut d, warnings, bundle) = prepare(o, inputs)?;
        let links = links(o, &bundle.source, file, rel, &mut d, index, dir);
        if k == 0 {
            let title = d
                .model
                .title
                .as_ref()
                .map(|t| t.text.trim().to_string())
                .filter(|t| !t.is_empty())
                .unwrap_or_else(|| stem(rel));
            let fonts = skemo_engine::html::font_faces_with(&d, &mut |name, _, _| {
                format!("fonts/{}", url_path(name))
            });
            built = Some(Built {
                title,
                drawings: Vec::new(),
                fonts,
                opens: links.opens,
                copies: links.copies,
                warnings: warnings + links.warnings,
                cards: cards(&d.model),
            });
        }
        let thumb = if thumbs {
            #[allow(clippy::cast_possible_truncation)]
            let scale = (640.0 / d.frame.w.max(1.0)).min(1.0) as f32;
            Some(skemo_engine::png::render(&d, scale).map_err(Fail::internal)?)
        } else {
            None
        };
        let background = d
            .scene
            .style(d.scene.stage)
            .var_color("--bg")
            .map_or_else(|| "#F6F7F9".into(), |c| c.to_css());
        drawings.push(Drawing {
            theme: (themes.len() > 1).then_some(if k == 0 { "light" } else { "dark" }),
            svg: skemo_engine::svg::render_final(&d),
            background,
            thumb,
        });
    }
    let mut b = built.ok_or_else(|| Fail::internal("no theme"))?;
    b.drawings = drawings;
    Ok(b)
}

/// The cards of a diagram for the search, in writing order; a legend (`card.legend`) is left out:
/// its lines name classes, not things.
fn cards(model: &skemo_lang::Model) -> Vec<Card> {
    use skemo_lang::Kind;
    let has = |n: &skemo_lang::Node, c: &str| n.classes.iter().any(|x| x == c);
    let title = |id: &str| model.node(id).map(|n| n.title.trim().to_string());
    model
        .nodes
        .iter()
        .filter(|n| n.kind == Kind::Card && !has(n, "legend"))
        .map(|n| {
            // the nearest zone with a title (a stack or an untitled group draws none)
            let mut zone = String::new();
            let mut up = n.parent.as_deref();
            while let Some(p) = up.and_then(|p| model.node(p)) {
                if p.kind == Kind::Zone && !p.title.trim().is_empty() {
                    zone = p.title.trim().to_string();
                    break;
                }
                up = p.parent.as_deref();
            }
            let note_of = if has(n, "note") {
                model
                    .edges
                    .iter()
                    .filter(|e| e.classes.iter().any(|c| c == "link"))
                    .find_map(|e| {
                        if e.from == n.id {
                            title(&e.to)
                        } else if e.to == n.id {
                            title(&e.from)
                        } else {
                            None
                        }
                    })
                    .unwrap_or_default()
            } else {
                String::new()
            };
            // as drawn: without the emphasis delimiters (`code`, **bold**)
            let lines = n
                .items
                .iter()
                .map(|l| skemo_lang::inline::plain(l).trim().to_string())
                .filter(|l| !l.is_empty())
                .collect::<Vec<_>>()
                .join(" · ");
            Card {
                id: n.id.clone(),
                title: skemo_lang::inline::plain(&n.title).trim().to_string(),
                category: n.label.clone().unwrap_or_default(),
                tag: n.tag.clone().unwrap_or_default(),
                zone,
                lines: cut(&lines, LINES),
                note_of,
            }
        })
        .collect()
}

/// `s` cut after `max` characters, with an ellipsis.
fn cut(s: &str, max: usize) -> String {
    match s.char_indices().nth(max) {
        Some((i, _)) => format!("{}…", s[..i].trim_end()),
        None => s.to_string(),
    }
}

/// The search index (`assets/search.js`): a script, not JSON, so that pages opened from the disk
/// (`file://`, where `fetch` is refused) can load it. `d`: each diagram's title, page and the title
/// of the diagram above it in the breadcrumb (several views share a title: "Installation"); `c`:
/// each card as `[diagram, id, title, category, tag, zone, lines, note of]`.
fn search_index(site: &Site) -> String {
    let d: Vec<[&str; 3]> = site
        .docs
        .iter()
        .enumerate()
        .map(|(k, d)| {
            let above = site.crumbs[k]
                .iter()
                .rev()
                .map(|&p| &site.docs[p])
                .find(|p| p.rel != "index.skemo")
                .map_or("", |p| p.title.as_str());
            [d.title.as_str(), d.page.as_str(), above]
        })
        .collect();
    let c: Vec<serde_json::Value> = site
        .docs
        .iter()
        .enumerate()
        .flat_map(|(k, d)| {
            d.cards.iter().map(move |c| {
                serde_json::json!([
                    k, c.id, c.title, c.category, c.tag, c.zone, c.lines, c.note_of
                ])
            })
        })
        .collect();
    format!(
        "window.skemoIndex={};\n",
        serde_json::json!({ "d": d, "c": c })
    )
}

/// The links of a diagram, once rewritten for the site.
struct Links {
    opens: Vec<usize>,
    copies: Vec<String>,
    warnings: usize,
}

/// Rewrites the links of `d` to other diagrams into links to their pages, lists the diagrams it
/// opens and the files to copy, and warns about the targets that are missing.
fn links(
    out: &Out,
    src: &str,
    file: &str,
    rel: &str,
    d: &mut Diagram,
    index: &HashMap<&str, usize>,
    dir: &Path,
) -> Links {
    let map = skemo_lang::parse(src).map;
    let base = rel.rsplit_once('/').map_or("", |(b, _)| b);
    let mut l = Links {
        opens: Vec::new(),
        copies: Vec::new(),
        warnings: 0,
    };
    let warn = |l: &mut Links, span: Option<Span>, code: &'static str, msg: String| {
        let span = span.unwrap_or_default();
        out.diag(src, file, &Diagnostic::warning(code, span, msg));
        l.warnings += 1;
    };
    // the title's pills written as Markdown links have no span of their own: the diagram's
    let at_title = map.diagram.as_ref().map(|x| x.line);
    let pills = d
        .model
        .title
        .iter_mut()
        .flat_map(|t| t.pill_hrefs.iter_mut())
        .map(|h| (at_title, h));
    let hrefs = d
        .model
        .nodes
        .iter_mut()
        .map(|n| (map.nodes.get(&n.id).map(|x| x.line), &mut n.href))
        .chain(
            d.model
                .edges
                .iter_mut()
                .map(|e| (map.edges.get(&e.id).map(|x| x.line), &mut e.href)),
        )
        .chain(pills);
    for (span, href) in hrefs {
        let Some(h) = href.clone() else { continue };
        if let Some((path, fragment)) = diagram_link(&h) {
            let target = resolve(base, path);
            match target.as_deref().and_then(|t| index.get(t)) {
                Some(&k) => {
                    if !l.opens.contains(&k) {
                        l.opens.push(k);
                    }
                    let page = format!("{}.html", &path[..path.len() - ".skemo".len()]);
                    *href = Some(match fragment {
                        Some(f) => format!("{page}#{f}"),
                        None => page,
                    });
                }
                None => warn(
                    &mut l,
                    span,
                    "site-missing-diagram",
                    tr!(
                        "link to `{path}`: no such diagram in the folder",
                        "lien vers « {path} » : pas de tel schéma dans le dossier"
                    ),
                ),
            }
        } else if let Some(path) = local_file(&h) {
            let found = resolve(base, path).filter(|t| {
                std::fs::canonicalize(dir.join(t))
                    .is_ok_and(|real| real.starts_with(dir) && real.is_file())
            });
            match found {
                Some(t) => l.copies.push(t),
                None => warn(
                    &mut l,
                    span,
                    "site-missing-file",
                    tr!(
                        "link to `{path}`: no such file in the folder, not copied",
                        "lien vers « {path} » : pas de tel fichier dans le dossier, non copié"
                    ),
                ),
            }
        }
    }
    l
}

/// The path of a link to a file (`docs/fw.md`, `img/a.png?x`): neither a scheme nor `#id` alone.
fn local_file(h: &str) -> Option<&str> {
    let path = h.split(['#', '?']).next().unwrap_or_default();
    let scheme = h
        .find([':', '/', '?', '#'])
        .is_some_and(|i| h.as_bytes()[i] == b':');
    (!scheme && !path.is_empty() && !path.starts_with(['/', '\\'])).then_some(path)
}

/// `path`, written in a diagram of the folder `base` (from the root, `/`), from the root; `None`
/// when it leads out of it.
fn resolve(base: &str, path: &str) -> Option<String> {
    let mut parts: Vec<&str> = base.split('/').filter(|p| !p.is_empty()).collect();
    for p in path.split(['/', '\\']) {
        match p {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            p => parts.push(p),
        }
    }
    Some(parts.join("/"))
}

/// The page of a diagram: its path with `.html` for `.skemo`.
fn page_of(rel: &str) -> String {
    format!("{}.html", &rel[..rel.len() - ".skemo".len()])
}

fn stem(rel: &str) -> String {
    let name = rel.rsplit('/').next().unwrap_or(rel);
    name[..name.len() - ".skemo".len()].to_string()
}

/// A path as an URL: each part percent-encoded but `/`.
pub(crate) fn url_path(p: &str) -> String {
    let mut s = String::new();
    for b in p.bytes() {
        if b.is_ascii_alphanumeric() || b"/-_.~".contains(&b) {
            s.push(char::from(b));
        } else {
            s.push_str(&format!("%{b:02X}"));
        }
    }
    s
}

fn absolute(p: &Path) -> PathBuf {
    if let Ok(c) = std::fs::canonicalize(p) {
        return c;
    }
    let abs = std::env::current_dir().map_or_else(|_| p.to_path_buf(), |c| c.join(p));
    // the deepest existing parent, resolved, then the rest as written
    let mut rest = Vec::new();
    let mut cur = abs.as_path();
    while let Some(parent) = cur.parent() {
        rest.push(cur.file_name().unwrap_or_default().to_os_string());
        if let Ok(c) = std::fs::canonicalize(parent) {
            return rest.iter().rev().fold(c, |acc, r| acc.join(r));
        }
        cur = parent;
    }
    abs
}

/// The `.skemo` files of `dir`, its subfolders included, sorted; hidden folders, links and the
/// output folder are left out.
fn scan(dir: &Path, output: &Path) -> Result<Vec<String>, Fail> {
    let mut found = Vec::new();
    let mut todo = vec![(dir.to_path_buf(), String::new())];
    while let Some((d, prefix)) = todo.pop() {
        let entries = std::fs::read_dir(&d).map_err(|e| {
            Fail::usage(tr!(
                "reading `{}`: {}",
                "lecture de « {} » : {}",
                d.display(),
                io(&e)
            ))
        })?;
        for e in entries.flatten() {
            let Ok(kind) = e.file_type() else { continue };
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') || kind.is_symlink() {
                continue;
            }
            let rel = format!("{prefix}{name}");
            if kind.is_dir() {
                if e.path() != output {
                    todo.push((e.path(), format!("{rel}/")));
                }
            } else if kind.is_file()
                && name.len() > ".skemo".len()
                && name.to_ascii_lowercase().ends_with(".skemo")
            {
                found.push(rel);
                if found.len() > MAX_DIAGRAMS {
                    return Err(Fail::usage(tr!(
                        "more than {MAX_DIAGRAMS} diagrams in `{}`: is it the right folder?",
                        "plus de {MAX_DIAGRAMS} schémas dans « {} » : est-ce le bon dossier ?",
                        dir.display()
                    )));
                }
            }
        }
    }
    found.sort();
    Ok(found)
}

/// Two diagrams cannot share a page (`a.skemo` and `a.SKEMO`).
fn check_pages(rels: &[String]) -> Result<(), Fail> {
    let mut seen: HashMap<String, &str> = HashMap::new();
    for r in rels {
        let page = page_of(r);
        if let Some(other) = seen.insert(page.clone(), r) {
            return Err(Fail::usage(tr!(
                "`{other}` and `{r}` would both be the page `{page}`: rename one",
                "« {other} » et « {r} » seraient toutes deux la page « {page} » : en renommer une"
            )));
        }
    }
    Ok(())
}

/// The way down to each diagram: from `index.skemo` when there is one, otherwise from the diagrams
/// no other opens; the shortest, the first written link first. A diagram out of reach has none.
fn crumbs(docs: &[Doc], overview: bool) -> Vec<Vec<usize>> {
    let n = docs.len();
    let opened: BTreeSet<usize> = docs
        .iter()
        .enumerate()
        .flat_map(|(k, d)| d.opens.iter().copied().filter(move |&t| t != k))
        .collect();
    let starts: Vec<usize> = if overview {
        (0..n).filter(|k| !opened.contains(k)).collect()
    } else {
        docs.iter()
            .position(|d| d.rel == "index.skemo")
            .into_iter()
            .collect()
    };
    let mut parent: Vec<Option<usize>> = vec![None; n];
    let mut seen = vec![false; n];
    let mut queue = VecDeque::new();
    for &s in &starts {
        seen[s] = true;
        queue.push_back(s);
    }
    while let Some(k) = queue.pop_front() {
        for &t in &docs[k].opens {
            if !seen[t] {
                seen[t] = true;
                parent[t] = Some(k);
                queue.push_back(t);
            }
        }
    }
    (0..n)
        .map(|k| {
            let mut up = Vec::new();
            let mut cur = parent[k];
            while let Some(p) = cur {
                up.push(p);
                cur = parent[p];
            }
            up.reverse();
            up
        })
        .collect()
}

/// For each diagram, the diagrams that open it, in the folder's order.
fn from(docs: &[Doc]) -> Vec<Vec<usize>> {
    let mut from = vec![Vec::new(); docs.len()];
    for (k, d) in docs.iter().enumerate() {
        for &t in &d.opens {
            if t != k {
                from[t].push(k);
            }
        }
    }
    from
}

fn write_site(
    a: &SiteArgs,
    dir: &Path,
    site: &Site,
    fonts: &BTreeMap<String, String>,
    copies: &BTreeSet<String>,
) -> Result<(), Fail> {
    let at = |p: &str| a.output.join(p).to_string_lossy().into_owned();
    write_out(&at("assets/site.css"), page::CSS.as_bytes())?;
    write_out(&at("assets/site.js"), page::JS.as_bytes())?;
    write_out(&at("assets/search.js"), search_index(site).as_bytes())?;
    for (css, name) in fonts {
        write_out(&at(&format!("assets/{name}")), css.as_bytes())?;
    }
    write_fonts(&at("assets/fonts"), fonts)?;
    for (k, doc) in site.docs.iter().enumerate() {
        write_out(&at(&doc.page), page::diagram(site, k).as_bytes())?;
        let src = std::fs::read(dir.join(&doc.rel)).map_err(|e| Fail::internal(io(&e)))?;
        write_out(&at(&doc.rel), &src)?;
        if site.overview {
            for (t, d) in doc.drawings.iter().enumerate() {
                if let Some(png) = &d.thumb {
                    write_out(&at(&page::thumb(doc, t)), png)?;
                }
            }
        }
    }
    if site.overview {
        write_out(&at("index.html"), page::overview(site).as_bytes())?;
    }
    for c in copies {
        let data = std::fs::read(dir.join(c)).map_err(|e| Fail::internal(io(&e)))?;
        write_out(&at(c), &data)?;
    }
    Ok(())
}

/// The font files the stylesheets name (`url(fonts/…)`), each once.
fn write_fonts(at: &str, fonts: &BTreeMap<String, String>) -> Result<(), Fail> {
    let wanted: BTreeSet<&str> = fonts
        .keys()
        .flat_map(|css| css.split("url(fonts/").skip(1))
        .filter_map(|s| s.split(')').next())
        .collect();
    let mut done = BTreeSet::new();
    for (name, data) in skemo_engine::fonts::files() {
        let url = url_path(name);
        if wanted.contains(url.as_str()) && done.insert(url.clone()) {
            write_out(&format!("{at}/{url}"), data)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_resolve_inside_the_folder_only() {
        assert_eq!(resolve("", "a.skemo").as_deref(), Some("a.skemo"));
        assert_eq!(
            resolve("shop", "../net.skemo").as_deref(),
            Some("net.skemo")
        );
        assert_eq!(
            resolve("shop", "./api/db.skemo").as_deref(),
            Some("shop/api/db.skemo")
        );
        assert_eq!(resolve("", "../a.skemo"), None);
        assert_eq!(page_of("shop/api.skemo"), "shop/api.html");
        assert_eq!(stem("shop/api.skemo"), "api");
        assert_eq!(url_path("a b/é.html"), "a%20b/%C3%A9.html");
    }

    #[test]
    fn only_local_files_are_copied() {
        assert_eq!(local_file("docs/fw.md#x"), Some("docs/fw.md"));
        assert_eq!(local_file("https://x.org/a"), None);
        assert_eq!(local_file("mailto:a@b"), None);
        assert_eq!(local_file("#core"), None);
        assert_eq!(local_file("/etc/passwd"), None);
    }

    fn doc(rel: &str, opens: Vec<usize>) -> Doc {
        Doc {
            rel: rel.into(),
            page: page_of(rel),
            title: stem(rel),
            drawings: Vec::new(),
            fonts: String::new(),
            opens,
            cards: Vec::new(),
        }
    }

    #[test]
    fn crumbs_follow_the_shortest_way_down() {
        // index → a → b, index → b would be shorter: b is under index
        let docs = vec![
            doc("a.skemo", vec![2]),
            doc("index.skemo", vec![0, 2]),
            doc("b.skemo", vec![]),
            doc("lost.skemo", vec![]),
        ];
        let c = crumbs(&docs, false);
        assert_eq!(c[0], vec![1]);
        assert_eq!(c[2], vec![1]);
        assert!(c[1].is_empty() && c[3].is_empty());
        // without an index, from the diagrams nothing opens
        let docs = vec![
            doc("a.skemo", vec![1]),
            doc("b.skemo", vec![2]),
            doc("c.skemo", vec![]),
        ];
        assert_eq!(crumbs(&docs, true), vec![vec![], vec![0], vec![0, 1]]);
        assert_eq!(from(&docs), vec![vec![], vec![0], vec![1]]);
    }

    #[test]
    fn two_diagrams_cannot_share_a_page() {
        assert!(check_pages(&["a.skemo".into(), "b.skemo".into()]).is_ok());
        assert_eq!(
            check_pages(&["a.SKEMO".into(), "a.skemo".into()])
                .unwrap_err()
                .0,
            2
        );
    }
}
