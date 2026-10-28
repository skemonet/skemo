//! `skemo watch`: live preview. Watches the source and the CSS files (by polling, every 300 ms),
//! renders again on each change and serves the HTML on `127.0.0.1` (never on the network).

use crate::args::{ThemeArg, WatchArgs};
use crate::common::{Fail, Inputs, prepare, with_source, write_out};
use crate::out::Out;
use skemo_engine::html::HtmlOptions;
use skemo_lang::tr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

const POLL: Duration = Duration::from_millis(300);

/// Current page and its version number (the browser reloads when it changes).
type Shared = Arc<Mutex<(u64, String)>>;

/// What is needed to rebuild the page; owned, to go into a thread.
#[derive(Clone)]
struct Builder {
    file: String,
    css: Vec<PathBuf>,
    base: Option<PathBuf>,
    output: Option<String>,
    theme: Option<ThemeArg>,
    out: Out,
}

pub fn run(out: &Out, a: &WatchArgs) -> Result<(), Fail> {
    if a.file == "-" {
        return Err(Fail::usage(tr!(
            "watch: a file is required",
            "watch : un fichier est nécessaire"
        )));
    }
    let server = tiny_http::Server::http(("127.0.0.1", a.port))
        .map_err(|e| Fail::internal(tr!("local server: {e}", "serveur local : {e}")))?;
    if !out.quiet {
        eprintln!(
            "{}",
            tr!(
                "preview: http://127.0.0.1:{}/  (Ctrl+C to stop)",
                "aperçu : http://127.0.0.1:{}/  (Ctrl+C pour arrêter)",
                a.port
            )
        );
    }
    let builder = Builder {
        file: a.file.clone(),
        css: a.css.clone(),
        base: a.base_theme.clone(),
        output: a.output.clone(),
        theme: a.theme,
        out: *out,
    };
    let state: Shared = Arc::new(Mutex::new((1, builder.build())));
    let mut watched = vec![PathBuf::from(&a.file)];
    watched.extend(a.css.iter().cloned());
    watched.extend(a.base_theme.clone());
    spawn_watcher(watched, builder, Arc::clone(&state));
    serve(&server, &state);
    Ok(())
}

impl Builder {
    /// HTML of the preview, or error page; also writes the output file asked for.
    fn build(&self) -> String {
        let inputs = Inputs::new(&self.file, &self.css, self.base.as_ref()).theme(self.theme);
        match prepare(&self.out, inputs) {
            Ok((d, _, bundle)) => {
                let title = Path::new(&self.file)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("skemo")
                    .to_string();
                let html = skemo_engine::html::render(
                    &d,
                    &HtmlOptions {
                        title,
                        embed_fonts: true,
                    },
                );
                if let Some(path) = &self.output {
                    let bytes = with_source(output_bytes(path, &d, &html), Some(&bundle));
                    let _ = write_out(path, &bytes);
                }
                html
            }
            Err(Fail(_, m)) => error_page(&self.error_text(m)),
        }
    }

    /// Text of the error page: diagnostics of the source, then the possible message.
    fn error_text(&self, message: Option<String>) -> String {
        let src = std::fs::read_to_string(&self.file).unwrap_or_default();
        let doc = skemo_lang::parse(&src);
        let mut msg: String = doc
            .errors()
            .map(|d| skemo_lang::diag::render(&src, &self.file, d))
            .collect();
        msg.extend(message);
        msg
    }
}

/// Content of the output file according to its extension (SVG, PNG, otherwise the HTML).
fn output_bytes(path: &str, d: &skemo_engine::Diagram, html: &str) -> Vec<u8> {
    match Path::new(path).extension().and_then(|e| e.to_str()) {
        Some("svg") => skemo_engine::svg::render(d).into_bytes(),
        Some("png") => skemo_engine::png::render(d, 2.0).unwrap_or_default(),
        _ => html.as_bytes().to_vec(),
    }
}

/// Error page of the preview (the message is escaped).
fn error_page(msg: &str) -> String {
    let esc = msg.replace('&', "&amp;").replace('<', "&lt;");
    let title = tr!("error", "erreur");
    format!(
        "<!doctype html><meta charset=\"utf-8\"><title>{title}</title><pre style=\"color:#b00;font:13px monospace;padding:16px\">{esc}</pre>"
    )
}

fn stamps(paths: &[PathBuf]) -> Vec<Option<SystemTime>> {
    paths
        .iter()
        .map(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok())
        .collect()
}

/// Watch thread: on each change of a watched file, rebuilds the page.
fn spawn_watcher(watched: Vec<PathBuf>, builder: Builder, state: Shared) {
    std::thread::spawn(move || {
        let mut last = stamps(&watched);
        loop {
            std::thread::sleep(POLL);
            let now = stamps(&watched);
            if now != last {
                last = now;
                let html = builder.build();
                if let Ok(mut st) = state.lock() {
                    *st = (st.0 + 1, html);
                }
            }
        }
    });
}

/// Auto-reload script, inserted before `</body>`: polls `/version`.
fn with_reload_script(html: &str, version: u64) -> String {
    let reload = format!(
        "<script>(()=>{{const v='{version}';setInterval(async()=>{{try{{const t=await (await fetch('/version')).text();if(t!==v)location.reload()}}catch(e){{}}}},400)}})()</script>"
    );
    match html.rfind("</body>") {
        Some(i) => format!("{}{reload}{}", &html[..i], &html[i..]),
        None => format!("{html}{reload}"),
    }
}

fn serve(server: &tiny_http::Server, state: &Shared) {
    for req in server.incoming_requests() {
        let (version, html) = state.lock().map(|s| s.clone()).unwrap_or_default();
        let resp = if req.url().starts_with("/version") {
            tiny_http::Response::from_string(version.to_string())
        } else {
            tiny_http::Response::from_string(with_reload_script(&html, version)).with_header(
                tiny_http::Header::from_bytes(
                    &b"Content-Type"[..],
                    &b"text/html; charset=utf-8"[..],
                )
                .expect("header"),
            )
        };
        let _ = req.respond(resp);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_page_escapes_markup() {
        let p = error_page("<script>alert(1)</script> & co");
        assert!(!p.contains("<script>"));
        assert!(p.contains("&lt;script>alert(1)&lt;/script> &amp; co"));
    }

    #[test]
    fn reload_script_goes_before_the_closing_body() {
        let page = with_reload_script("<html><body>x</body></html>", 7);
        let (head, tail) = page.split_once("</body>").unwrap();
        assert!(head.contains("const v='7'") && head.ends_with("</script>"));
        assert_eq!(tail, "</html>");
        assert!(with_reload_script("no body", 1).starts_with("no body<script>"));
    }

    use std::io::{Read, Write};

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("skemo-watch-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn builder(file: &Path, output: Option<String>) -> Builder {
        Builder {
            file: file.to_string_lossy().into_owned(),
            css: Vec::new(),
            base: None,
            output,
            theme: None,
            out: Out {
                color: false,
                quiet: true,
            },
        }
    }

    #[test]
    fn build_renders_a_page_or_an_error_page() {
        let dir = scratch("build");
        let f = dir.join("a.skemo");
        std::fs::write(&f, "diagram\n  card#a A\n").unwrap();
        let html = builder(&f, None).build();
        assert!(html.contains("<card") && html.contains("<title>a</title>"));
        std::fs::write(&f, "card#a(\n").unwrap();
        let err = builder(&f, None).build();
        assert!(err.contains("<pre") && !err.contains("<card"), "{err}");
        let missing = builder(&dir.join("absent.skemo"), None).build();
        assert!(missing.contains("<pre"), "{missing}");
    }

    #[test]
    fn build_writes_the_requested_output_by_extension() {
        let dir = scratch("output");
        let f = dir.join("a.skemo");
        std::fs::write(&f, "diagram\n  card#a A\n").unwrap();
        for (name, magic) in [
            ("o.svg", &b"<svg"[..]),
            ("o.png", &b"\x89PNG"[..]),
            ("o.html", &b"<!doctype"[..]),
        ] {
            let out = dir.join(name);
            builder(&f, Some(out.to_string_lossy().into_owned())).build();
            let bytes = std::fs::read(&out).unwrap();
            assert!(bytes.starts_with(magic), "{name}");
        }
    }

    #[test]
    fn a_change_bumps_the_version_and_the_server_answers() {
        let dir = scratch("serve");
        let f = dir.join("a.skemo");
        std::fs::write(&f, "diagram\n  card#a A\n").unwrap();
        let b = builder(&f, None);
        let state: Shared = Arc::new(Mutex::new((1, b.build())));
        spawn_watcher(vec![f.clone()], b, Arc::clone(&state));
        let server = Arc::new(tiny_http::Server::http("127.0.0.1:0").unwrap());
        let port = server.server_addr().to_ip().unwrap().port();
        let (srv, st) = (Arc::clone(&server), Arc::clone(&state));
        std::thread::spawn(move || serve(&srv, &st));
        let get = |path: &str| {
            let mut s = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
            write!(s, "GET {path} HTTP/1.0\r\n\r\n").unwrap();
            let mut body = String::new();
            s.read_to_string(&mut body).unwrap();
            body
        };
        assert!(get("/version").ends_with('1'));
        assert!(get("/").contains("fetch('/version')"));
        std::thread::sleep(Duration::from_millis(1100)); // mtime granularity
        std::fs::write(&f, "diagram\n  card#a A\n  card#b B\n").unwrap();
        let start = std::time::Instant::now();
        while state.lock().unwrap().0 == 1 {
            assert!(
                start.elapsed() < Duration::from_secs(5),
                "version never changed"
            );
            std::thread::sleep(Duration::from_millis(50));
        }
        assert!(get("/version").ends_with('2'));
        assert!(get("/").contains("data-id=\"b\""));
    }

    #[test]
    fn stamps_are_none_for_missing_files() {
        assert_eq!(stamps(&[PathBuf::from("/nonexistent/x")]), vec![None]);
    }
}
