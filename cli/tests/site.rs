//! `skemo site` (D-097): a folder of diagrams turned into pages linked to one another.

use assert_cmd::Command;
use serde_json::Value;
use std::path::{Path, PathBuf};

fn bin() -> Command {
    let mut c = Command::cargo_bin("skemo").unwrap();
    c.env("SKEMO_LANG", "en").env("NO_COLOR", "1");
    c
}

/// A fresh folder `name` with the files `files` (path, content).
fn folder(name: &str, files: &[(&str, &str)]) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("site")
        .join(name);
    let _ = std::fs::remove_dir_all(&d);
    for (p, content) in files {
        let f = d.join("in").join(p);
        std::fs::create_dir_all(f.parent().unwrap()).unwrap();
        std::fs::write(f, content).unwrap();
    }
    d
}

/// `skemo site in -o out` in `d`, with `args`: (exit code, stderr).
fn site(d: &Path, args: &[&str]) -> (i32, String) {
    let o = bin()
        .current_dir(d)
        .args(["site", "in", "-o", "out"])
        .args(args)
        .output()
        .unwrap();
    (
        o.status.code().unwrap_or(-1),
        String::from_utf8(o.stderr).unwrap(),
    )
}

fn read(d: &Path, p: &str) -> String {
    std::fs::read_to_string(d.join("out").join(p)).unwrap_or_else(|e| panic!("{p}: {e}"))
}

const INDEX: &str = "diagram\n  title Context\n  card#user User\n    to(shop)\n  card#shop(href=\"shop/containers.skemo\") Shop\n";
const CONTAINERS: &str = "diagram\n  title Containers\n  card#web Web\n    to(api)\n  card#api(href=\"api.skemo#routes\") API\n  card#up(href=\"../index.skemo\") Context\n";
const API: &str =
    "diagram\n  card#routes Routes\n    to(db)\n  card#db(href=\"containers.skemo\") DB\n";

#[test]
fn links_to_diagrams_become_links_between_pages() {
    let d = folder(
        "links",
        &[
            ("index.skemo", INDEX),
            ("shop/containers.skemo", CONTAINERS),
            ("shop/api.skemo", API),
        ],
    );
    let (code, err) = site(&d, &["--title", "My system"]);
    assert_eq!(code, 0, "{err}");
    let index = read(&d, "index.html");
    assert!(
        index.contains("<a href=\"shop/containers.html\">"),
        "{index}"
    );
    assert!(index.contains("<title>Context · My system</title>"));
    let containers = read(&d, "shop/containers.html");
    assert!(containers.contains("<a href=\"api.html#routes\">"));
    assert!(containers.contains("<a href=\"../index.html\">"));
    // the breadcrumb: the site, then the current view
    assert!(containers.contains("<li><a href=\"../index.html\">My system</a></li><li aria-current=\"page\"><span>Containers</span></li>"));
    // a diagram without a title is named after its file; it is under the containers
    let api = read(&d, "shop/api.html");
    assert!(api.contains(
        "<a href=\"containers.html\">Containers</a></li><li aria-current=\"page\"><span>api</span>"
    ));
    assert!(api.contains("<h2>Opened from</h2><ul><li><a href=\"containers.html\">Containers</a>"));
    // the sources are beside the pages, the assets once
    assert_eq!(read(&d, "shop/api.skemo"), API);
    assert!(read(&d, "assets/site.js").contains("sk-canvas"));
    assert!(read(&d, "assets/fonts-0.css").contains("url(fonts/Inter-"));
    let fonts = read(&d, "assets/fonts-0.css");
    for f in fonts.split("url(fonts/").skip(1) {
        let name = f.split(')').next().unwrap();
        assert!(d.join("out/assets/fonts").join(name).is_file(), "{name}");
    }
    // no link of the drawings is left to a source; the "Source" button links to its own
    assert!(!containers.contains("<a href=\"api.skemo"));
    assert!(!containers.contains("<a href=\"../index.skemo"));
    assert!(containers.contains("href=\"containers.skemo\" title=\"Source"));
}

#[test]
fn a_link_to_a_diagram_draws_a_zoom_glyph() {
    let render = |src: &str| {
        let o = bin()
            .args(["render", "-", "-o", "-", "--format", "svg", "--no-source"])
            .write_stdin(src)
            .output()
            .unwrap();
        String::from_utf8(o.stdout).unwrap()
    };
    let zoom = render("diagram\n  card#a(href=\"b.skemo\") A\n");
    let external = render("diagram\n  card#a(href=\"https://x.org\") A\n");
    assert_ne!(zoom, external);
    // same size, same layout: only the glyph differs
    let viewbox = |s: &str| {
        let v = s.split("viewBox=\"").nth(1).unwrap();
        v.split('"').next().unwrap().to_string()
    };
    assert_eq!(viewbox(&zoom), viewbox(&external));
}

#[test]
fn without_an_index_the_home_page_shows_every_diagram() {
    let d = folder(
        "overview",
        &[
            ("b.skemo", API),
            ("a.skemo", "diagram\n  card#x(href=\"b.skemo\") X\n"),
        ],
    );
    let (code, err) = site(&d, &["--theme", "auto"]);
    assert_eq!(code, 0, "{err}");
    // b's link to containers.skemo is missing: a warning, the site is written
    assert!(err.contains("site-missing-diagram"), "{err}");
    let home = read(&d, "index.html");
    assert!(home.contains("class=\"sk-home\""));
    // a, which nothing opens, comes first; both themes have a thumbnail
    let a = home.find("href=\"a.html\"").unwrap();
    let b = home.find("href=\"b.html\"").unwrap();
    assert!(a < b);
    assert!(home.contains("a.thumb-dark.png"));
    assert!(d.join("out/a.thumb-light.png").is_file());
    // two drawings per page and the switch
    let page = read(&d, "a.html");
    assert!(page.contains("sk-view sk-light") && page.contains("sk-view sk-dark"));
    assert!(page.contains("class=\"sk-btn sk-theme\""));
    assert!(page.contains("data-auto"));
}

#[test]
fn a_search_finds_the_cards_of_every_diagram() {
    let d = folder(
        "search",
        &[
            ("index.skemo", INDEX),
            ("shop/containers.skemo", CONTAINERS),
            (
                "shop/api.skemo",
                "diagram\n  title Routes of the API\n  zone#core Core\n    card#routes(label=\"http\" tag=\"v2\") Routes\n      | `GET /orders` and **more**\n    card#n.note Handled by\n      | the router pod\n      to(routes).link\n  card#key.legend Key\n    |.note a note\n",
            ),
        ],
    );
    let (code, err) = site(&d, &[]);
    assert_eq!(code, 0, "{err}");
    let js = read(&d, "assets/search.js");
    let json = js
        .strip_prefix("window.skemoIndex=")
        .and_then(|s| s.trim_end().strip_suffix(';'))
        .unwrap();
    let ix: Value = serde_json::from_str(json).unwrap();
    // each diagram: title, page, the diagram above it in the breadcrumb
    let docs = ix["d"].as_array().unwrap();
    let api = docs.iter().position(|d| d[1] == "shop/api.html").unwrap();
    assert_eq!(docs[api][0], "Routes of the API");
    assert_eq!(docs[api][2], "Containers");
    // each card: its diagram, id, title, category, tag, zone, plain lines, the card a note is on
    let cards = ix["c"].as_array().unwrap();
    let routes = cards.iter().find(|c| c[1] == "routes").unwrap();
    assert_eq!(routes[0], api);
    assert_eq!(
        routes.as_array().unwrap()[2..],
        ["Routes", "http", "v2", "Core", "GET /orders and more", ""]
    );
    let note = cards.iter().find(|c| c[1] == "n").unwrap();
    assert_eq!(note[7], "Routes");
    // a legend names classes, not things
    assert!(cards.iter().all(|c| c[1] != "key"));
    // light and dark by default, with the switch; the panel's edge to resize it
    let page = read(&d, "shop/api.html");
    assert!(page.contains("data-auto") && page.contains("sk-view sk-dark"));
    assert!(page.contains("class=\"sk-resize\"") && page.contains("skemo-site-side"));
    // every page has the box, with the way back to the site's root
    assert!(read(&d, "shop/api.html").contains("class=\"sk-q\""));
    assert!(read(&d, "shop/api.html").contains("data-root=\"../\""));
    assert!(read(&d, "index.html").contains("data-root=\"\""));
}

#[test]
fn a_refused_diagram_refuses_the_site() {
    let d = folder(
        "refused",
        &[
            ("index.skemo", INDEX),
            ("bad.skemo", "diagram\n  card#a A\n    to(nowhere)\n"),
        ],
    );
    let (code, err) = site(&d, &[]);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("bad.skemo"), "{err}");
    assert!(!d.join("out").exists());
}

#[test]
fn deny_warnings_and_missing_files() {
    let d = folder(
        "deny",
        &[
            (
                "index.skemo",
                "diagram\n  card#a(href=\"docs/a.md\") A\n  card#b(href=\"gone.md\") B\n",
            ),
            ("docs/a.md", "# A\n"),
        ],
    );
    let (code, err) = site(&d, &["--diagnostics", "json"]);
    assert_eq!(code, 0, "{err}");
    // a linked file of the folder is copied; a missing one is a warning at its line
    assert_eq!(read(&d, "docs/a.md"), "# A\n");
    let w: Vec<Value> = err
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(w.len(), 1, "{err}");
    assert_eq!(w[0]["code"], "site-missing-file");
    assert_eq!(w[0]["line"], 3);
    let (code, _) = site(&d, &["--deny-warnings"]);
    assert_eq!(code, 1);
}

#[test]
fn misuse_is_a_usage_error() {
    let d = folder("usage", &[("notes.txt", "x")]);
    // no diagram in the folder
    assert_eq!(site(&d, &[]).0, 2);
    // a file instead of a folder
    let o = bin()
        .current_dir(&d)
        .args(["site", "in/notes.txt", "-o", "out"])
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(2));
    // the output is the folder of diagrams itself
    std::fs::write(d.join("in/a.skemo"), "diagram\n  card#a A\n").unwrap();
    let o = bin()
        .current_dir(&d)
        .args(["site", "in", "-o", "in"])
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(2));
    // an output inside the folder is not read as diagrams
    let o = bin()
        .current_dir(&d)
        .args(["site", "in", "-o", "in/_site"])
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(0));
    let o = bin()
        .current_dir(&d)
        .args(["site", "in", "-o", "in/_site"])
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(0));
    let err = String::from_utf8(o.stderr).unwrap();
    assert!(err.contains("1 diagram(s)"), "{err}");
}
