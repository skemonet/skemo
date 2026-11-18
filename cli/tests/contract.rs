//! The contract of the command line for the tools that run it (`cli/SPEC.md` §2, `engine/SVG.md`):
//! the acceptance tests of glitu's requirements SK-0 to SK-11.

use assert_cmd::Command;
use serde_json::Value;
use std::path::{Path, PathBuf};

fn bin() -> Command {
    let mut c = Command::cargo_bin("skemo").unwrap();
    c.env("SKEMO_LANG", "en").env_remove("NO_COLOR");
    c
}

fn scratch(name: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("contract")
        .join(name);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// `skemo render - -o - --format svg` with `args`, in `dir`: (exit code, stdout, stderr).
fn render_in(dir: &Path, src: &str, args: &[&str]) -> (i32, String, String) {
    let o = bin()
        .current_dir(dir)
        .args(["render", "-", "-o", "-", "--format", "svg"])
        .args(args)
        .write_stdin(src)
        .output()
        .unwrap();
    (
        o.status.code().unwrap_or(-1),
        String::from_utf8(o.stdout).unwrap(),
        String::from_utf8(o.stderr).unwrap(),
    )
}

/// Every line of the standard error, parsed as JSON (each one must be).
fn json_lines(stderr: &str) -> Vec<Value> {
    stderr
        .lines()
        .map(|l| serde_json::from_str(l).unwrap_or_else(|e| panic!("not JSON: {l}: {e}")))
        .collect()
}

const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR\0\0\0\x01\0\0\0\x01\x08\x06\0\0\0\x1f\x15\xc4\x89\0\0\0\rIDATx\x9cc\xf8\xcf\xc0\xf0\x1f\0\x05\0\x01\xff\x89\x99=\x1d\0\0\0\0IEND\xaeB`\x82";
const LOGO: &str = "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 4 4\"><rect width=\"4\" height=\"4\"/></svg>";

/// SK-1: standard input and output only, no file written (even in a read-only folder), no
/// question, the exit codes of the contract, no colour off a terminal.
#[test]
fn a_tool_gets_stdin_stdout_and_the_exit_codes() {
    let dir = scratch("stdio");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o555)).unwrap();
    }
    let (code, out, err) = render_in(&dir, "diagram\n  card#a A\n", &[]);
    assert_eq!(code, 0, "{err}");
    assert!(out.starts_with("<svg"));
    assert!(!err.contains('\x1b'));
    // refused: nothing on the standard output
    let (code, out, err) = render_in(&dir, "diagram\n  card#a A\n    to(db)\n", &[]);
    assert_eq!((code, out.as_str()), (1, ""));
    assert!(
        err.contains("unknown-target") && !err.contains('\x1b'),
        "{err}"
    );
    let warned = "diagram\n  card#a A @1-2\n";
    assert_eq!(render_in(&dir, warned, &[]).0, 0);
    let (code, out, _) = render_in(&dir, warned, &["--deny-warnings"]);
    assert_eq!((code, out.as_str()), (1, ""));
    assert_eq!(render_in(&dir, "x", &["--nope"]).0, 2);
    // a closed standard input (assert_cmd's default) is an empty source, never a wait
    let o = bin()
        .current_dir(&dir)
        .args(["render", "-", "-o", "-", "--format", "svg"])
        .output()
        .unwrap();
    assert!(matches!(o.status.code(), Some(0 | 1)));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    assert_eq!(
        std::fs::read_dir(&dir).unwrap().count(),
        0,
        "no file written"
    );
}

/// SK-2: one JSON object per line, nothing else; positions from 1, columns in characters.
#[test]
fn diagnostics_in_json_lines() {
    let dir = scratch("json");
    let json = ["--diagnostics", "json"];
    let (code, _, err) = render_in(&dir, "diagram\n  card#é A\n    to(db)\n", &json);
    assert_eq!(code, 1);
    assert_eq!(
        err,
        "{\"severity\":\"error\",\"code\":\"unknown-target\",\"message\":\"unknown target `db`\",\"file\":\"-\",\"line\":3,\"column\":8,\"end_line\":3,\"end_column\":10}\n"
    );
    let (code, _, err) = render_in(&dir, "diagram\n  card#a A @1-2\n", &json);
    assert_eq!(code, 0);
    assert_eq!(
        err,
        "{\"severity\":\"warning\",\"code\":\"no-step-after\",\"message\":\"`a`: @…-2: no step after 2\",\"file\":\"-\",\"line\":2,\"column\":3,\"end_line\":2,\"end_column\":16}\n"
    );
    // a stylesheet the diagram includes: missing, then not text
    let (code, _, err) = render_in(&dir, "diagram(css=\"look.css\")\n  card#a A\n", &json);
    assert_eq!(code, 1);
    let v = json_lines(&err);
    assert_eq!(v.len(), 1);
    assert_eq!(v[0]["code"], "file-missing");
    assert_eq!(v[0]["file"], "look.css");
    std::fs::write(dir.join("look.css"), b"\xff\xfe").unwrap();
    let (_, _, err) = render_in(&dir, "diagram(css=\"look.css\")\n  card#a A\n", &json);
    assert_eq!(json_lines(&err)[0]["code"], "file-not-text");
    // a missing image
    let (code, _, err) = render_in(&dir, "diagram\n  card#a(icon=\"img/x.png\") A\n", &json);
    assert_eq!(code, 1);
    assert_eq!(
        err,
        "{\"severity\":\"error\",\"code\":\"file-missing\",\"message\":\"image `img/x.png`: no such file\",\"file\":\"img/x.png\"}\n"
    );
    // a usage error, in French too
    let o = bin()
        .env("SKEMO_LANG", "fr")
        .args(["render", "-", "--nope", "--diagnostics", "json"])
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(2));
    let v = json_lines(&String::from_utf8(o.stderr).unwrap());
    assert_eq!((v.len(), &v[0]["code"]), (1, &Value::from("usage")));
}

/// SK-3: the version and the features, in JSON; plain `--version` unchanged.
#[test]
fn capabilities_in_json() {
    let o = bin()
        .args(["capabilities", "--format", "json"])
        .output()
        .unwrap();
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(v["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(v["svg_conventions"], 1);
    for f in [
        "diagnostics-json",
        "highlights",
        "root",
        "batch",
        "limits",
        "allow",
    ] {
        assert!(v["features"].as_array().unwrap().contains(&f.into()), "{f}");
    }
    let o = bin().arg("--version").output().unwrap();
    let first = String::from_utf8(o.stdout).unwrap();
    assert_eq!(
        first.lines().next(),
        Some(format!("skemo {}", env!("CARGO_PKG_VERSION")).as_str())
    );
}

/// SK-4: glitu's own example.
#[test]
fn highlights_are_written_beside_the_appearance() {
    let dir = scratch("highlights");
    let (code, out, err) = render_in(&dir, "diagram\n  card#a A @1:up @3:halo\n", &[]);
    assert_eq!(code, 0, "{err}");
    assert!(
        out.contains(r#"data-step="1" data-fx="up" data-highlight="halo:3""#),
        "{out}"
    );
}

/// SK-4.5: a click that changes nothing is a warning on the marker after the gap; `@2.5` is no gap.
#[test]
fn a_gap_in_the_steps_is_a_warning() {
    let dir = scratch("gap");
    let deny = ["--deny-warnings", "--diagnostics", "json"];
    let src = "diagram\n  card#a A @1\n    to(b) @2\n  card#b B @2\n  card#c C @9:halo\n";
    let (code, out, err) = render_in(&dir, src, &deny);
    assert_eq!((code, out.as_str()), (1, ""), "{err}");
    let d = json_lines(&err);
    assert_eq!(d[0]["code"], "step-gap");
    assert_eq!(d[0]["line"], 5);
    assert_eq!(
        d[0]["message"],
        "clicks 3 to 8 change nothing: the next step after 2 is 9 (`@9:halo`)"
    );
    let (_, _, err) = render_in(
        &dir,
        "diagram\n  card#a A @1\n  card#b B @2\n  card#x X @7\n",
        &deny,
    );
    assert_eq!(json_lines(&err)[0]["code"], "step-gap", "{err}");
    for ok in [
        "diagram\n  card#a A @1\n  card#b B @2:halo\n",
        "diagram\n  card#a A @1\n  card#b B @1.5\n  card#c C @2\n",
        "diagram\n  card#a A @1\n  card#b B @2.5\n",
    ] {
        let (code, _, err) = render_in(&dir, ok, &deny);
        assert_eq!((code, err.as_str()), (0, ""), "{ok}");
    }
}

/// `--allow CODE`: the warnings of that code are neither shown nor counted by `--deny-warnings`;
/// the other codes and the errors still are. glitu's case: a diagram whose steps share the
/// slide's clicks with the text (`@1`, then `@8` after six clicks of bullets).
#[test]
fn allowed_codes_are_left_out() {
    let dir = scratch("allow");
    let src = "diagram\n  card#api API @1\n    to(pod)\n  card#pod Pod @1 @8-9:halo\n";
    let deny = ["--deny-warnings", "--diagnostics", "json"];
    let (code, out, err) = render_in(&dir, src, &deny);
    assert_eq!((code, out.as_str()), (1, ""), "{err}");
    let mut allow = deny.to_vec();
    allow.extend(["--allow", "step-gap"]);
    let (code, out, err) = render_in(&dir, src, &allow);
    assert_eq!((code, err.as_str()), (0, ""));
    assert!(
        out.contains("data-states=\"[&quot;1&quot;,&quot;8&quot;,&quot;9&quot;]\""),
        "{out}"
    );
    assert!(out.contains("data-highlight=\"halo:8-9\""), "{out}");
    // another code still counts; a list and a repeated option both work
    let icon = "diagram\n  card#a(icon=\"nope\") A @3\n";
    let (code, _, err) = render_in(&dir, icon, &allow);
    assert_eq!(code, 1, "{err}");
    assert_eq!(json_lines(&err)[0]["code"], "unknown-icon", "{err}");
    for extra in [
        &["--allow", "step-gap,unknown-icon"][..],
        &["--allow", "unknown-icon", "--allow", "step-gap"],
    ] {
        let mut args = deny.to_vec();
        args.extend(extra);
        let (code, _, err) = render_in(&dir, icon, &args);
        assert_eq!((code, err.as_str()), (0, ""), "{extra:?}");
    }
    // errors are never left out
    let mut args = deny.to_vec();
    args.extend(["--allow", "unknown-tag,syntax,error"]);
    let (code, _, _) = render_in(&dir, "diagram\n  card#a A\n  card#a B\n", &args);
    assert_eq!(code, 1);
}

/// The engine's warnings carry a code of their own and, when about an element, its line.
#[test]
fn engine_warnings_have_codes_and_lines() {
    let dir = scratch("engine-codes");
    let json = ["--diagnostics", "json"];
    let src = "diagram(layout=\"nope\")\n  card#a(icon=\"nope\") A\n";
    let (code, _, err) = render_in(&dir, src, &json);
    assert_eq!(code, 0, "{err}");
    let d = json_lines(&err);
    let get = |c: &str| d.iter().find(|w| w["code"] == c).cloned();
    assert!(
        get("unknown-layout").is_some_and(|w| w.get("line").is_none()),
        "{err}"
    );
    assert!(get("unknown-icon").is_some_and(|w| w["line"] == 2), "{err}");
    let src = "diagram(layout=\"mindmap\")\n  card#r R\n    to(a)\n  card#a A\n  card#z Z\n";
    let (_, _, err) = render_in(&dir, src, &json);
    let d = json_lines(&err);
    assert_eq!(
        (d[0]["code"].as_str(), d[0]["line"].as_u64()),
        (Some("mindmap-unlinked"), Some(5))
    );
    assert!(!err.contains("\"engine\""), "{err}");
}

/// A stylesheet's typos and broken structure are warnings at their place in that file.
#[test]
fn stylesheet_diagnostics_point_into_the_file() {
    let dir = scratch("css-check");
    std::fs::write(
        dir.join("t.css"),
        ".pod {\n  colr: red;\n  --brand: 1;\n}\nfill: blue;\n",
    )
    .unwrap();
    let (code, _, err) = render_in(
        &dir,
        "diagram\n  card#a.pod A\n",
        &["--css", "t.css", "--diagnostics", "json", "--deny-warnings"],
    );
    assert_eq!(code, 1, "{err}");
    let d = json_lines(&err);
    let at = |k: usize| {
        (
            d[k]["code"].as_str(),
            d[k]["file"].as_str(),
            d[k]["line"].as_u64(),
        )
    };
    assert_eq!(at(0), (Some("css-property"), Some("t.css"), Some(2)));
    assert_eq!(at(1), (Some("css-syntax"), Some("t.css"), Some(5)));
    assert_eq!(d.len(), 3, "the author's --brand is free: {err}");
}

/// SK-1: an empty diagram is a warning, still rendered without `--deny-warnings`.
#[test]
fn an_empty_diagram_is_a_warning() {
    let dir = scratch("empty");
    for src in ["", "\n\n", "diagram\n"] {
        let (code, out, err) = render_in(&dir, src, &["--deny-warnings", "--diagnostics", "json"]);
        assert_eq!((code, out.as_str()), (1, ""), "{src:?}");
        assert_eq!(json_lines(&err)[0]["code"], "empty-diagram", "{src:?}");
        let (code, out, _) = render_in(&dir, src, &[]);
        assert_eq!(code, 0);
        assert!(out.starts_with("<svg"), "{src:?}");
    }
}

/// SK-8: two stylesheets with the same base name stay two files, in order, after `extract`.
#[test]
fn stylesheets_with_the_same_name_stay_apart() {
    let dir = scratch("css-names");
    for (d, css) in [
        ("theme", ".web { --c: #2F7FD8; }\n"),
        ("deck", ".pod { --c: #2563eb; }\n"),
    ] {
        std::fs::create_dir_all(dir.join(d)).unwrap();
        std::fs::write(dir.join(d).join("skemo.css"), css).unwrap();
    }
    let src = "diagram\n  card#a.pod A\n  card#b.web B\n";
    let css = [
        "--css",
        "theme/skemo.css",
        "--css",
        "deck/skemo.css",
        "--no-source",
    ];
    let (_, plain, _) = render_in(&dir, src, &css);
    let (code, _, err) = render_in(&dir, src, &css[..4]);
    assert_eq!(code, 0, "{err}");
    std::fs::write(dir.join("a.skemo"), src).unwrap();
    bin()
        .current_dir(&dir)
        .args(["render", "a.skemo", "-o", "a.svg"])
        .args(&css[..4])
        .assert()
        .success();
    bin()
        .current_dir(&dir)
        .args(["extract", "a.svg", "-o", "out"])
        .assert()
        .success();
    let read = |n: &str| std::fs::read_to_string(dir.join("out").join(n)).unwrap();
    assert!(read("skemo.css").contains(".web"));
    assert!(read("skemo-2.css").contains(".pod"));
    let again = bin()
        .current_dir(dir.join("out"))
        .args([
            "render",
            "a.skemo",
            "-o",
            "-",
            "--format",
            "svg",
            "--no-source",
        ])
        .args(["--css", "skemo.css", "--css", "skemo-2.css"])
        .output()
        .unwrap();
    assert_eq!(String::from_utf8(again.stdout).unwrap(), plain);
}

/// SK-6: the files a diagram names are read inside the root only.
#[test]
fn files_are_read_inside_the_root_only() {
    let top = scratch("root");
    let dir = top.join("talk");
    std::fs::create_dir_all(dir.join("img")).unwrap();
    std::fs::write(top.join("x.svg"), LOGO).unwrap();
    std::fs::write(dir.join("img/logo.svg"), LOGO).unwrap();
    std::fs::write(dir.join("img/big.png"), vec![0u8; 2 << 20]).unwrap();
    std::fs::write(dir.join("img/fake.svg"), PNG).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(top.join("x.svg"), dir.join("img/out.svg")).unwrap();
    let icon = |name: &str| format!("diagram\n  card#a(icon=\"{name}\") A\n");
    let json = ["--diagnostics", "json"];
    let refused = |name: &str, args: &[&str]| {
        let (code, out, err) = render_in(&dir, &icon(name), &[&json[..], args].concat());
        assert_eq!((code, out.as_str()), (1, ""), "{name}: {err}");
        json_lines(&err)[0]["code"].as_str().unwrap().to_string()
    };
    assert_eq!(refused("../x.svg", &[]), "file-outside-root");
    assert_eq!(refused("/etc/passwd.svg", &[]), "file-outside-root");
    #[cfg(unix)]
    assert_eq!(refused("img/out.svg", &[]), "file-outside-root");
    assert_eq!(refused("img/big.png", &[]), "file-too-large");
    assert_eq!(refused("img/fake.svg", &[]), "image-type");
    // inside the root: read; --root moves it
    let (code, out, _) = render_in(&dir, &icon("img/logo.svg"), &[]);
    assert!(code == 0 && out.contains("data:image/svg+xml;base64,"));
    let root = dir.join("img");
    let (code, _, _) = render_in(&top, &icon("logo.svg"), &["--root", root.to_str().unwrap()]);
    assert_eq!(code, 0);
    assert_eq!(refused("logo.svg", &[]), "file-missing");
}

/// The diagrams of the repository: examples of the site, fixtures, gallery.
fn corpus() -> Vec<PathBuf> {
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut out = Vec::new();
    for d in ["lang/site/examples", "engine/fixtures", "site/gallery"] {
        let Ok(entries) = std::fs::read_dir(base.join(d)) else {
            continue;
        };
        for e in entries.flatten() {
            let p = e.path();
            if matches!(
                p.extension().and_then(|x| x.to_str()),
                Some("skemo" | "pug")
            ) {
                out.push(p);
            }
        }
    }
    out.sort();
    assert!(out.len() > 40, "{} diagrams", out.len());
    out
}

fn render_file(p: &Path, args: &[&str]) -> Option<String> {
    let o = bin()
        .current_dir(p.parent().unwrap())
        .args([
            "render",
            p.to_str().unwrap(),
            "-o",
            "-",
            "--format",
            "svg",
            "-q",
        ])
        .args(args)
        .output()
        .unwrap();
    o.status
        .success()
        .then(|| String::from_utf8(o.stdout).unwrap())
}

/// Elements an SVG of skemo may hold (`engine/SVG.md` §3); `image` only with a `data:` URL, `a`
/// only for a `href=` written in the source, `style` only with `--motion` or `--embed-fonts`.
const ELEMENTS: &[&str] = &[
    "svg",
    "g",
    "path",
    "rect",
    "circle",
    "ellipse",
    "line",
    "polyline",
    "polygon",
    "text",
    "tspan",
    "defs",
    "marker",
    "linearGradient",
    "radialGradient",
    "stop",
    "clipPath",
    "mask",
    "pattern",
    "symbol",
    "use",
    "filter",
    "title",
    "desc",
    "metadata",
    "skemo-source",
    "image",
];

fn check_allowlist(name: &str, svg: &str, extra: &[&str]) {
    for tag in svg.split('<').skip(1) {
        let el: String = tag
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == ':')
            .collect();
        if el.is_empty() || tag.starts_with('/') {
            continue;
        }
        assert!(
            ELEMENTS.contains(&el.as_str()) || el.starts_with("fe") || extra.contains(&el.as_str()),
            "{name}: <{el}>"
        );
        let head = tag.split('>').next().unwrap_or("");
        assert!(!head.contains(" on"), "{name}: an on* attribute in <{el}>");
        assert!(!head.to_ascii_lowercase().contains("javascript:"), "{name}");
        if let Some(h) = head.split("href=\"").nth(1) {
            let ok = h.starts_with('#')
                || (el == "image"
                    && ["png", "jpeg", "webp", "gif", "svg+xml"]
                        .iter()
                        .any(|t| h.starts_with(&format!("data:image/{t};"))))
                || (el == "a" && extra.contains(&"a"));
            assert!(ok, "{name}: href in <{el}>: {}", &h[..h.len().min(40)]);
        }
    }
}

/// SK-5 and SK-7: every example renders twice, in two processes, to the same bytes, and only
/// with the elements of the conventions.
#[test]
fn the_examples_are_deterministic_and_within_the_allowlist() {
    let mut rendered = 0;
    for p in corpus() {
        let Some(a) = render_file(&p, &[]) else {
            continue; // an example of an error
        };
        let b = render_file(&p, &[]).unwrap();
        assert!(a == b, "{}: two renderings differ", p.display());
        let src = std::fs::read_to_string(&p).unwrap_or_default();
        let extra: &[&str] = if src.contains("href=") { &["a"] } else { &[] };
        check_allowlist(&p.display().to_string(), &a, extra);
        rendered += 1;
    }
    assert!(rendered > 40, "{rendered}");
    // --motion adds one <style> of keyframes, nothing else
    let dir = scratch("motion");
    let (_, out, _) = render_in(
        &dir,
        "diagram\n  card#a A\n    to(b).flow\n  card#b B\n",
        &["--motion"],
    );
    check_allowlist("motion", &out, &["style"]);
}

/// SK-0 and SK-8: the command line writes the bytes glitu's call to the library writes, with
/// the theme's stylesheet, a text scale and an image.
#[test]
fn the_command_line_writes_what_the_library_writes() {
    use skemo_engine::source::{Bundle, NamedBytes, NamedText, embed_text};
    let dir = scratch("same");
    let theme = ".web { --c: #2F7FD8; }\n.stage { --gap: 30; }\n";
    std::fs::write(dir.join("skemo.css"), theme).unwrap();
    std::fs::write(dir.join("logo.svg"), LOGO).unwrap();
    let sources = [
        "diagram\n  card#a.web A\n    to(b)\n  card#b B\n",
        "diagram(dir=\"right\")\n  zone#z Zone @1\n    card#a(icon=\"logo.svg\") A @2:up @3-4:halo\n      | line @5:chase\n      to(b) @6:magnify\n  card#b B\n",
        "diagram(layout=\"timeline\")\n  card#a(start=\"2024-01\" end=\"2024-06\") Phase @1\n",
    ];
    for (k, src) in sources.iter().enumerate() {
        let scale = 1.35_f64;
        let images: std::collections::BTreeMap<String, Vec<u8>> = if src.contains("logo.svg") {
            [("logo.svg".to_string(), LOGO.as_bytes().to_vec())].into()
        } else {
            Default::default()
        };
        let opts = skemo_engine::Options {
            css: vec![theme.into(), format!(".stage {{ --font-scale: {scale}; }}")],
            transparent: true,
            images: images.clone(),
            ..Default::default()
        };
        let (d, _) = skemo_engine::from_source(src, &opts).unwrap();
        let mut bundle = Bundle::new(src);
        bundle.css = vec![NamedText {
            name: "skemo.css".into(),
            content: theme.into(),
        }];
        bundle.font_scale = Some(scale);
        bundle.images = images.iter().map(|(n, d)| NamedBytes::new(n, d)).collect();
        let lib = embed_text(&skemo_engine::svg::render(&d), &bundle);
        let (code, cli, err) = render_in(
            &dir,
            src,
            &[
                "--css",
                "skemo.css",
                "--font-scale",
                "1.35",
                "--transparent",
            ],
        );
        assert_eq!(code, 0, "{err}");
        assert!(cli == lib, "source {k}: the CLI and the library differ");
    }
}

/// SK-10: the answers of `--batch` are the SVGs of `render`, in order; a refused diagram does
/// not end the process.
#[test]
fn batch_answers_each_request_in_order() {
    let dir = scratch("batch");
    std::fs::write(dir.join("t.css"), ".web { --c: #2F7FD8; }").unwrap();
    let a = "diagram\n  card#a.web A\n    to(b)\n  card#b B\n";
    let requests = [
        serde_json::json!({"id": "s1", "source": a, "css": ["t.css"], "font_scale": 1.35}),
        serde_json::json!({"id": 2, "source": "diagram\n  card#a A\n    to(x)\n"}),
        serde_json::json!({"id": "s3", "source": a, "transparent": true}),
    ];
    let input: String = requests
        .iter()
        .map(|r| format!("{r}\nnot json\n"))
        .collect();
    let o = bin()
        .current_dir(&dir)
        .args(["render", "--batch"])
        .write_stdin(input)
        .output()
        .unwrap();
    assert!(o.status.success());
    let answers: Vec<Value> = String::from_utf8(o.stdout)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(answers.len(), 6);
    let single = |args: &[&str]| render_in(&dir, a, args).1;
    assert_eq!(answers[0]["id"], "s1");
    assert_eq!(
        answers[0]["svg"],
        single(&["--css", "t.css", "--font-scale", "1.35"]).as_str()
    );
    assert_eq!(answers[0]["diagnostics"], serde_json::json!([]));
    assert_eq!(answers[1]["id"], Value::Null);
    assert_eq!(answers[1]["diagnostics"][0]["code"], "bad-request");
    assert_eq!(answers[2]["id"], 2);
    assert_eq!(answers[2]["svg"], Value::Null);
    assert_eq!(answers[2]["diagnostics"][0]["code"], "unknown-target");
    assert_eq!(answers[4]["svg"], single(&["--transparent"]).as_str());
}

/// SK-11: a diagram over the limits is refused at once, with its code.
#[test]
fn a_diagram_over_the_limits_is_refused() {
    let dir = scratch("limits");
    let mut src = String::from("diagram\n");
    for k in 0..1001 {
        src.push_str(&format!("  card#c{k} C\n"));
    }
    let (code, out, err) = render_in(&dir, &src, &["--diagnostics", "json"]);
    assert_eq!((code, out.as_str()), (1, ""));
    assert_eq!(json_lines(&err)[0]["code"], "too-large");
}

/// The first string literal of every `tr!(` in `src` (the English message).
fn english_literals(src: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = src;
    while let Some(i) = rest.find("tr!(") {
        rest = &rest[i + 4..];
        let body = rest.trim_start();
        let Some(lit) = body.strip_prefix('"') else {
            continue;
        };
        let mut end = 0;
        let mut escaped = false;
        for (j, c) in lit.char_indices() {
            match c {
                '\\' if !escaped => escaped = true,
                '"' if !escaped => {
                    end = j;
                    break;
                }
                _ => escaped = false,
            }
        }
        out.push(lit[..end].to_string());
    }
    out
}

fn rust_sources(dir: &Path, out: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        let name = e.file_name();
        if p.is_dir() && name != "target" && name != "tests" && name != "fixtures" {
            rust_sources(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

/// SK-1.5, SK-2: English messages quote names with backticks; « » is French only.
#[test]
fn english_messages_have_no_french_quotes() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut files = Vec::new();
    for c in ["router", "lang", "engine", "cli"] {
        rust_sources(&root.join(c), &mut files);
    }
    let mut seen = 0;
    for f in files {
        let src = std::fs::read_to_string(&f).unwrap();
        for m in english_literals(&src) {
            seen += 1;
            assert!(
                !m.contains('«') && !m.contains('»'),
                "{}: French quotes in an English message: {m}",
                f.display()
            );
        }
    }
    assert!(seen > 100, "the English catalogue was not found ({seen})");
    let (_, _, err) = render_in(
        &scratch("quotes"),
        "diagram\n  card#a A\n    to(zz)\n  card#a B @2:glow\n",
        &["--diagnostics", "json"],
    );
    assert!(err.contains("`zz`") && !err.contains('«'), "{err}");
}
