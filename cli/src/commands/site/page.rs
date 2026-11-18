//! The pages of `skemo site`: one per diagram, and the overview when the folder has no
//! `index.skemo`. Their text is in English whatever the system language, so that a site built on
//! two machines is the same.

use super::{Doc, Site, url_path};
use std::fmt::Write;

pub const CSS: &str = include_str!("site.css");
pub const JS: &str = include_str!("site.js");

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// `../` as many times as `page` is deep.
fn up(page: &str) -> String {
    "../".repeat(page.matches('/').count())
}

/// The link from the page `from` to the page `to` (both from the site's root): out of the folders
/// they do not share, then down.
fn href(from: &str, to: &str) -> String {
    let dirs: Vec<&str> = from.split('/').collect();
    let dirs = &dirs[..dirs.len() - 1];
    let parts: Vec<&str> = to.split('/').collect();
    let shared = dirs
        .iter()
        .zip(&parts[..parts.len() - 1])
        .take_while(|(a, b)| a == b)
        .count();
    format!(
        "{}{}",
        "../".repeat(dirs.len() - shared),
        url_path(&parts[shared..].join("/"))
    )
}

/// The thumbnail of a diagram's drawing `t`, from the site's root.
pub fn thumb(doc: &Doc, t: usize) -> String {
    let base = &doc.page[..doc.page.len() - ".html".len()];
    match doc.drawings.get(t).and_then(|d| d.theme) {
        Some(theme) => format!("{base}.thumb-{theme}.png"),
        None => format!("{base}.thumb.png"),
    }
}

const ICON_MENU: &str =
    r#"<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 6h16M4 12h16M4 18h16"/></svg>"#;
const ICON_OUT: &str = r#"<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M5 12h14"/></svg>"#;
const ICON_IN: &str =
    r#"<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M5 12h14M12 5v14"/></svg>"#;
const ICON_FIT: &str = r#"<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 9V4h5M15 4h5v5M20 15v5h-5M9 20H4v-5"/></svg>"#;
const ICON_THEME: &str = r#"<svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="12" cy="12" r="8"/><path d="M12 4a8 8 0 0 1 0 16z" class="fill"/></svg>"#;
const ICON_SOURCE: &str =
    r#"<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M9 7l-5 5 5 5M15 7l5 5-5 5"/></svg>"#;

/// `<head>` and the top bar, shared by every page.
fn head(site: &Site, page: &str, title: &str, fonts: Option<&str>) -> String {
    let up = up(page);
    let theme = if site.dark { "dark" } else { "light" };
    let theme_pick = if site.auto {
        // before the first paint: the reader's choice, otherwise the system's
        "<script>(function(){var t;try{t=localStorage.getItem('skemo-site-theme')}catch(e){}if(t!=='light'&&t!=='dark'){t=matchMedia('(prefers-color-scheme: dark)').matches?'dark':'light'}document.documentElement.dataset.theme=t})()</script>\n"
    } else {
        ""
    };
    // the panel's width the reader chose, before the first paint too
    let pick = format!(
        "{theme_pick}<script>(function(){{var w;try{{w=+localStorage.getItem('skemo-site-side')}}catch(e){{}}if(w>=180&&w<=640){{document.documentElement.style.setProperty('--sk-side',w+'px')}}}})()</script>\n"
    );
    let fonts = fonts.map_or_else(String::new, |f| {
        format!("<link rel=\"stylesheet\" href=\"{up}assets/{f}\">\n")
    });
    let full = if title == site.title {
        esc(title)
    } else {
        format!("{} · {}", esc(title), esc(&site.title))
    };
    format!(
        "<!doctype html>\n<html lang=\"en\" data-theme=\"{theme}\"{auto}>\n<head>\n<meta charset=\"utf-8\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n<meta name=\"generator\" content=\"skemo {version}\">\n<title>{full}</title>\n{pick}<link rel=\"stylesheet\" href=\"{up}assets/site.css\">\n{fonts}</head>\n",
        auto = if site.auto { " data-auto" } else { "" },
        version = env!("CARGO_PKG_VERSION"),
    )
}

/// The breadcrumb of diagram `k` (`None`: the overview).
fn crumbs(site: &Site, page: &str, k: Option<usize>) -> String {
    let mut s = String::from("<nav class=\"sk-crumbs\" aria-label=\"Breadcrumb\"><ol>");
    let home_is_here = k.is_none_or(|k| site.docs[k].rel == "index.skemo");
    if home_is_here {
        let _ = write!(
            s,
            "<li aria-current=\"page\"><span>{}</span></li>",
            esc(&site.title)
        );
    } else {
        let _ = write!(
            s,
            "<li><a href=\"{}\">{}</a></li>",
            href(page, "index.html"),
            esc(&site.title)
        );
    }
    if let Some(k) = k.filter(|_| !home_is_here) {
        for &p in &site.crumbs[k] {
            let d = &site.docs[p];
            if d.rel == "index.skemo" {
                continue;
            }
            let _ = write!(
                s,
                "<li><a href=\"{}\">{}</a></li>",
                href(page, &d.page),
                esc(&d.title)
            );
        }
        let _ = write!(
            s,
            "<li aria-current=\"page\"><span>{}</span></li>",
            esc(&site.docs[k].title)
        );
    }
    s.push_str("</ol></nav>");
    s
}

/// The list of every diagram, by folder; the current one marked.
fn tree(site: &Site, page: &str, current: Option<usize>) -> String {
    let mut s = String::from("<nav class=\"sk-tree\" aria-label=\"Diagrams\"><ul>");
    let mut open: Vec<&str> = Vec::new();
    for (k, d) in site.docs.iter().enumerate() {
        let folders: Vec<&str> = d.rel.split('/').collect();
        let folders = &folders[..folders.len() - 1];
        let same = open.iter().zip(folders).take_while(|(a, b)| a == b).count();
        for _ in same..open.len() {
            s.push_str("</ul></li>");
        }
        open.truncate(same);
        for f in &folders[same..] {
            let _ = write!(
                s,
                "<li class=\"sk-folder\"><span class=\"sk-folder-name\">{}</span><ul>",
                esc(f)
            );
            open.push(f);
        }
        let here = if Some(k) == current {
            " aria-current=\"page\""
        } else {
            ""
        };
        let _ = write!(
            s,
            "<li><a href=\"{}\"{here} title=\"{}\">{}</a></li>",
            href(page, &d.page),
            esc(&d.rel),
            esc(&d.title)
        );
    }
    for _ in 0..open.len() {
        s.push_str("</ul></li>");
    }
    s.push_str("</ul></nav>");
    s
}

/// A titled list of diagrams (what a view opens, what opens it).
fn related(site: &Site, page: &str, heading: &str, list: &[usize]) -> String {
    if list.is_empty() {
        return String::new();
    }
    let mut s = format!("<h2>{heading}</h2><ul>");
    for &k in list {
        let d = &site.docs[k];
        let _ = write!(
            s,
            "<li><a href=\"{}\">{}</a></li>",
            href(page, &d.page),
            esc(&d.title)
        );
    }
    s.push_str("</ul>");
    s
}

fn bar(site: &Site, page: &str, k: Option<usize>, tools: &str) -> String {
    let theme = if site.auto {
        format!(
            "<button class=\"sk-btn sk-theme\" type=\"button\" title=\"Light or dark\" aria-label=\"Light or dark\">{ICON_THEME}</button>"
        )
    } else {
        String::new()
    };
    format!(
        "<header class=\"sk-bar\"><button class=\"sk-btn sk-menu\" type=\"button\" aria-label=\"Diagrams\" aria-expanded=\"false\" aria-controls=\"sk-side\">{ICON_MENU}</button>{}{}<div class=\"sk-tools\">{tools}{theme}</div></header>\n",
        crumbs(site, page, k),
        search(page)
    )
}

/// The search box: the cards of every diagram (`assets/search.js`, loaded when first used), a
/// result opening its diagram's page centred on the card (`page.html#id`).
fn search(page: &str) -> String {
    format!(
        "<div class=\"sk-search\" role=\"search\"><input class=\"sk-q\" type=\"search\" placeholder=\"Search cards\" aria-label=\"Search the cards of every diagram\" autocomplete=\"off\" spellcheck=\"false\" role=\"combobox\" aria-autocomplete=\"list\" aria-expanded=\"false\" aria-controls=\"sk-hits\" data-root=\"{}\"><kbd class=\"sk-key\" aria-hidden=\"true\">/</kbd><div class=\"sk-hits\" id=\"sk-hits\" role=\"listbox\" aria-label=\"Cards found\" hidden></div></div>",
        up(page)
    )
}

fn side(site: &Site, page: &str, k: Option<usize>, extra: &str) -> String {
    format!(
        "<aside class=\"sk-side\" id=\"sk-side\"><input class=\"sk-filter\" type=\"search\" placeholder=\"Filter diagrams\" aria-label=\"Filter diagrams\">{}{extra}</aside><div class=\"sk-resize\" role=\"separator\" aria-orientation=\"vertical\" aria-controls=\"sk-side\" aria-label=\"Width of the panel (drag, arrow keys; double-click to reset)\" aria-valuemin=\"180\" aria-valuemax=\"640\" tabindex=\"0\"></div>\n",
        tree(site, page, k)
    )
}

/// The page of diagram `k`.
pub fn diagram(site: &Site, k: usize) -> String {
    let doc = &site.docs[k];
    let page = doc.page.as_str();
    let source = doc.rel.rsplit('/').next().unwrap_or(&doc.rel);
    let tools = format!(
        "<button class=\"sk-btn\" type=\"button\" data-zoom=\"out\" title=\"Zoom out (−)\" aria-label=\"Zoom out\">{ICON_OUT}</button><button class=\"sk-btn\" type=\"button\" data-zoom=\"fit\" title=\"Fit (0)\" aria-label=\"Fit to the window\">{ICON_FIT}</button><button class=\"sk-btn\" type=\"button\" data-zoom=\"in\" title=\"Zoom in (+)\" aria-label=\"Zoom in\">{ICON_IN}</button><a class=\"sk-btn\" href=\"{}\" title=\"Source ({})\" aria-label=\"Source\">{ICON_SOURCE}</a>",
        url_path(source),
        esc(source)
    );
    let rel = format!(
        "<section class=\"sk-rel\">{}{}</section>",
        related(site, page, "Opens", &doc.opens),
        related(site, page, "Opened from", &site.from[k])
    );
    let mut views = String::new();
    for d in &doc.drawings {
        let class = d.theme.map_or_else(String::new, |t| format!(" sk-{t}"));
        let _ = write!(
            views,
            "<div class=\"sk-view{class}\" style=\"background:{}\"><div class=\"sk-stage\">{}</div></div>",
            esc(&d.background),
            d.svg
        );
    }
    let mut s = head(site, page, &doc.title, Some(&doc.fonts));
    s.push_str("<body class=\"sk-diagram\">\n");
    s.push_str(&bar(site, page, Some(k), &tools));
    s.push_str("<div class=\"sk-body\">\n");
    s.push_str(&side(site, page, Some(k), &rel));
    let _ = writeln!(
        s,
        "<main class=\"sk-canvas\" tabindex=\"0\" aria-label=\"{}\">{views}</main>",
        esc(&doc.title)
    );
    let _ = write!(
        s,
        "</div>\n<script src=\"{}assets/site.js\"></script>\n</body>\n</html>\n",
        up(page)
    );
    s
}

/// `index.html` when the folder has no `index.skemo`: a thumbnail per diagram, the diagrams no
/// other opens first.
pub fn overview(site: &Site) -> String {
    let page = "index.html";
    let mut order: Vec<usize> = (0..site.docs.len()).collect();
    order.sort_by_key(|&k| (!site.from[k].is_empty(), k));
    let mut cards = String::new();
    for k in order {
        let d = &site.docs[k];
        let mut imgs = String::new();
        for (t, dr) in d.drawings.iter().enumerate() {
            let class = dr
                .theme
                .map_or_else(String::new, |t| format!(" class=\"sk-{t}\""));
            let _ = write!(
                imgs,
                "<img{class} src=\"{}\" alt=\"\" loading=\"lazy\">",
                url_path(&thumb(d, t))
            );
        }
        let backgrounds: String = d
            .drawings
            .iter()
            .map(|dr| {
                format!(
                    "--sk-thumb-{}:{};",
                    dr.theme.unwrap_or("light"),
                    dr.background
                )
            })
            .collect();
        let folder = d.rel.rsplit_once('/').map_or("", |(f, _)| f);
        let meta = match d.opens.len() {
            0 => String::new(),
            1 => "opens 1 view".to_string(),
            n => format!("opens {n} views"),
        };
        let _ = write!(
            cards,
            "<li><a class=\"sk-card\" href=\"{}\"><span class=\"sk-thumb\" style=\"{}\">{imgs}</span><span class=\"sk-card-title\">{}</span><span class=\"sk-card-meta\">{}{}{}</span></a></li>",
            url_path(&d.page),
            esc(&backgrounds),
            esc(&d.title),
            esc(folder),
            if folder.is_empty() || meta.is_empty() {
                ""
            } else {
                " · "
            },
            meta
        );
    }
    let mut s = head(site, page, &site.title, None);
    s.push_str("<body class=\"sk-home\">\n");
    s.push_str(&bar(site, page, None, ""));
    s.push_str("<div class=\"sk-body\">\n");
    s.push_str(&side(site, page, None, ""));
    let _ = write!(
        s,
        "<main class=\"sk-grid\"><h1>{}</h1><ul>{cards}</ul></main>\n</div>\n<script src=\"assets/site.js\"></script>\n</body>\n</html>\n",
        esc(&site.title)
    );
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_between_pages_climb_out_of_their_folder() {
        assert_eq!(up("a.html"), "");
        assert_eq!(up("x/y/a.html"), "../../");
        assert_eq!(href("x/a.html", "index.html"), "../index.html");
        assert_eq!(href("a.html", "x y/b.html"), "x%20y/b.html");
        assert_eq!(href("x/a.html", "x/b.html"), "b.html");
        assert_eq!(href("x/y/a.html", "x/z/b.html"), "../z/b.html");
        assert_eq!(esc("<a & \"b\">"), "&lt;a &amp; &quot;b&quot;&gt;");
    }
}
