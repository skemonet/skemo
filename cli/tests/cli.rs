//! End-to-end tests (SPEC §4) on the examples of `engine/fixtures` and the errors of `lang`.

use assert_cmd::Command;
use std::path::{Path, PathBuf};

/// The binary, in English whatever the language of the machine running the tests.
fn bin() -> Command {
    let mut c = Command::cargo_bin("skemo").unwrap();
    c.env("SKEMO_LANG", "en");
    c
}

/// The binary with an explicit environment (language tests).
fn bin_env(vars: &[(&str, &str)]) -> Command {
    let mut c = Command::cargo_bin("skemo").unwrap();
    for k in ["SKEMO_LANG", "LC_ALL", "LC_MESSAGES", "LANGUAGE", "LANG"] {
        c.env_remove(k);
    }
    for (k, v) in vars {
        c.env(k, v);
    }
    c
}

fn fx(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../engine/fixtures")
        .join(name)
}

fn tmp(name: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR")).join("cli-tests");
    std::fs::create_dir_all(&d).unwrap();
    d.join(name)
}

#[test]
fn render_formats() {
    for ext in ["png", "svg", "html"] {
        let out = tmp(&format!("reseau.{ext}"));
        bin()
            .args([
                "render",
                fx("reseau.pug").to_str().unwrap(),
                "-o",
                out.to_str().unwrap(),
            ])
            .assert()
            .success();
        let data = std::fs::read(&out).unwrap();
        assert!(data.len() > 1000, "{ext} vide");
    }
    let png = std::fs::read(tmp("reseau.png")).unwrap();
    assert_eq!(&png[1..4], b"PNG");
}

#[test]
fn render_is_deterministic() {
    let (a, b) = (tmp("det-a.png"), tmp("det-b.png"));
    for o in [&a, &b] {
        bin()
            .args([
                "render",
                fx("design.pug").to_str().unwrap(),
                "--css",
                fx("design.css").to_str().unwrap(),
                "-o",
                o.to_str().unwrap(),
            ])
            .assert()
            .success();
    }
    assert_eq!(std::fs::read(a).unwrap(), std::fs::read(b).unwrap());
}

#[test]
fn animated_png_and_states() {
    let out = tmp("anim.png");
    bin()
        .args([
            "render",
            fx("animation.pug").to_str().unwrap(),
            "-o",
            out.to_str().unwrap(),
            "--animated",
            "-q",
        ])
        .assert()
        .success();
    let idx: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(tmp("anim.states.json")).unwrap()).unwrap();
    let n = idx["states"].as_array().unwrap().len();
    assert_eq!(n, 8);
    assert!(tmp("anim.07.png").exists());
}

#[test]
fn check_errors_exit_1() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../lang/fixtures/erreurs");
    for f in [
        "cible-inconnue",
        "id-duplique",
        "to-hors-carte",
        "groupes-multiples",
        "indentation-melangee",
        "marqueurs",
        "nombres-mal-formes",
    ] {
        bin()
            .args(["check", dir.join(format!("{f}.skemo")).to_str().unwrap()])
            .assert()
            .code(1);
    }
    bin()
        .args(["check", fx("reseau.pug").to_str().unwrap()])
        .assert()
        .success();
}

#[test]
fn usage_errors_exit_2() {
    bin()
        .args(["render", fx("reseau.pug").to_str().unwrap(), "-o", "x.txt"])
        .assert()
        .code(2);
    bin()
        .args([
            "render",
            fx("reseau.pug").to_str().unwrap(),
            "-o",
            "x.svg",
            "--animated",
        ])
        .assert()
        .code(2);
    bin()
        .args(["render", "absent.pug", "-o", "x.svg"])
        .assert()
        .code(2);
}

#[test]
fn layout_json_and_stdin() {
    let src = std::fs::read_to_string(fx("reseau.pug")).unwrap();
    let out = bin()
        .args(["layout", "-", "--json"])
        .write_stdin(src)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert!(v["layout"]["sizes"]["fw"]["w"].as_f64().unwrap() > 150.0);
    assert!(v["layout"]["nodes"]["lan"].is_array());
}

#[test]
fn theme_print() {
    let out = bin()
        .args(["theme", "--print"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    assert!(String::from_utf8(out).unwrap().contains(".stage"));
}

#[test]
fn fmt_check_and_write() {
    let src = "card#a(step=2) A\n    to(b)\ncard#b B\n";
    let f = tmp("fmt.pug");
    std::fs::write(&f, src).unwrap();
    bin()
        .args(["fmt", "--check", f.to_str().unwrap()])
        .assert()
        .code(1);
    bin().args(["fmt", f.to_str().unwrap()]).assert().success();
    assert_eq!(
        std::fs::read_to_string(&f).unwrap(),
        "card#a A @2\n  to(b)\ncard#b B\n"
    );
    bin()
        .args(["fmt", "--check", f.to_str().unwrap()])
        .assert()
        .success();
}

#[test]
fn contrast_check() {
    // the default theme has known defects (DMZ zone title): code 1 and report
    let out = bin()
        .args(["check", "--contrast", fx("reseau.pug").to_str().unwrap()])
        .assert()
        .code(1)
        .get_output()
        .stderr
        .clone();
    assert!(String::from_utf8(out).unwrap().contains("dmz/title"));
    // a diagram without zones or coloured types passes
    let f = tmp("simple.pug");
    std::fs::write(&f, "card#a Un\n  to(b)\ncard#b Deux\n").unwrap();
    bin()
        .args(["check", "--contrast", f.to_str().unwrap()])
        .assert()
        .success();
}

#[test]
fn base_theme_replaces_default() {
    let theme = tmp("base.css");
    std::fs::write(&theme, ".stage { --bg: #102030; font: 14px/1.4 Inter; } card { display: block; min-width: 100px; padding: 4px; }").unwrap();
    let out = bin()
        .args([
            "layout",
            fx("reseau.pug").to_str().unwrap(),
            "--json",
            "--base-theme",
            theme.to_str().unwrap(),
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_ne!(v["layout"]["sizes"]["fw"]["w"], 220.0);
}

#[test]
fn steps_board() {
    let out = tmp("frise.steps.png");
    bin()
        .args([
            "render",
            fx("frise.pug").to_str().unwrap(),
            "--css",
            fx("frise.css").to_str().unwrap(),
            "-o",
            out.to_str().unwrap(),
            "--board",
            "-q",
        ])
        .assert()
        .success();
    let h = std::fs::read(&out).unwrap();
    let height = u32::from_be_bytes([h[20], h[21], h[22], h[23]]);
    assert!(
        height > 1500,
        "contact sheet of 5 states expected ({height} px)"
    );
}

/// F-ANIM-10: animated PNG (acTL), deterministic.
#[test]
fn render_apng() {
    let run = |name: &str| {
        let out = tmp(name);
        bin()
            .args([
                "render",
                fx("animation.pug").to_str().unwrap(),
                "-o",
                out.to_str().unwrap(),
                "--fps",
                "4",
            ])
            .assert()
            .success();
        std::fs::read(out).unwrap()
    };
    let a = run("anim-a.apng");
    assert!(a.starts_with(b"\x89PNG\r\n\x1a\n"));
    assert!(a.windows(4).any(|w| w == b"acTL"), "no acTL chunk");
    assert_eq!(a, run("anim-b.apng"), "non-deterministic APNG");
}

#[test]
fn render_high_contrast_changes_only_colors() {
    let render = |extra: &[&str], name: &str| {
        let out = tmp(name);
        let mut args = vec![
            "render",
            fx("design.pug").to_str().unwrap().to_owned().leak() as &str,
            "-o",
            out.to_str().unwrap().to_owned().leak(),
        ];
        args.extend_from_slice(extra);
        bin().args(args).assert().success();
        std::fs::read_to_string(out).unwrap()
    };
    let (std, high) = (
        render(&[], "c-std.svg"),
        render(&["--contrast", "high"], "c-high.svg"),
    );
    assert_ne!(std, high);
    assert_eq!(std.matches("<rect").count(), high.matches("<rect").count());
}

#[test]
fn init_creates_a_renderable_example_and_never_overwrites() {
    let dir = tmp("init-dir");
    let _ = std::fs::remove_dir_all(&dir);
    bin()
        .args(["init", dir.to_str().unwrap()])
        .assert()
        .success();
    let (src, css) = (dir.join("example.skemo"), dir.join("custom.css"));
    assert!(src.exists() && css.exists());
    let out = tmp("init.png");
    bin()
        .args([
            "render",
            src.to_str().unwrap(),
            "--css",
            css.to_str().unwrap(),
            "-o",
            out.to_str().unwrap(),
            "--deny-warnings",
        ])
        .assert()
        .success();
    bin()
        .args(["check", src.to_str().unwrap()])
        .assert()
        .success();
    // second call: refused, files intact
    std::fs::write(&css, "/* mine */").unwrap();
    bin().args(["init", dir.to_str().unwrap()]).assert().code(2);
    assert_eq!(std::fs::read_to_string(&css).unwrap(), "/* mine */");
}

#[test]
fn deny_warnings_turns_a_warning_into_exit_1() {
    let src =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../lang/fixtures/erreurs/avertissements.skemo");
    let out = tmp("warn.svg");
    let render = |extra: &[&str]| {
        let mut c = bin();
        c.args(["render", src.to_str().unwrap(), "-o", out.to_str().unwrap()]);
        c.args(extra);
        c.assert()
    };
    render(&[]).success();
    render(&["--deny-warnings"]).code(1);
    render(&["--quiet"]).success().stderr("");
}

/// `watch` serves the page on 127.0.0.1, with the reload script and a version that changes.
#[test]
fn watch_serves_the_page_and_its_version() {
    use std::io::{Read, Write};
    use std::net::TcpStream;
    use std::process::{Command as Std, Stdio};
    use std::time::{Duration, Instant};

    let dir = tmp("watch-dir");
    let _ = std::fs::remove_dir_all(&dir);
    bin()
        .args(["init", dir.to_str().unwrap()])
        .assert()
        .success();
    let port = 18_700 + u16::try_from(std::process::id() % 200).unwrap();
    let exe = assert_cmd::cargo::cargo_bin("skemo");
    let mut child = Std::new(exe)
        .args([
            "watch",
            dir.join("example.skemo").to_str().unwrap(),
            "--port",
            &port.to_string(),
        ])
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let get = |path: &str| -> Option<String> {
        let mut s = TcpStream::connect(("127.0.0.1", port)).ok()?;
        write!(s, "GET {path} HTTP/1.0\r\n\r\n").ok()?;
        let mut body = String::new();
        s.read_to_string(&mut body).ok()?;
        Some(body)
    };
    let start = Instant::now();
    let page = loop {
        if let Some(p) = get("/") {
            break p;
        }
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "the server does not answer"
        );
        std::thread::sleep(Duration::from_millis(50));
    };
    let version = get("/version").unwrap();
    child.kill().unwrap();
    child.wait().unwrap();
    assert!(
        page.contains("<card") && page.contains("fetch('/version')"),
        "page without reload"
    );
    assert!(page.starts_with("HTTP/1.") && page.contains("text/html"));
    assert!(version.ends_with('1'), "version initiale : {version}");
}

#[test]
fn transparent_png_has_an_alpha_corner_and_default_is_opaque() {
    // the PNG's corner pixel is tested in the engine's tests (tests/transparent.rs)
    let render = |extra: &[&str], name: &str| {
        let out = tmp(name);
        let mut args = vec![
            "render",
            fx("reseau.pug").to_str().unwrap().to_owned().leak() as &str,
            "-o",
            out.to_str().unwrap().to_owned().leak(),
        ];
        args.extend_from_slice(extra);
        bin().args(args).assert().success();
        std::fs::read_to_string(&out).unwrap_or_default()
    };
    let opaque = render(&[], "t-opaque.svg");
    let clear = render(&["--transparent"], "t-clear.svg");
    assert!(opaque.contains("class=\"stage\""));
    assert!(!clear.contains("class=\"stage\""));
}

#[test]
fn render_webp_still_animated_and_state() {
    let render = |src: &str, name: &str, extra: &[&str]| {
        let out = tmp(name);
        let mut args = vec![
            "render",
            fx(src).to_str().unwrap().to_owned().leak() as &str,
            "-o",
            out.to_str().unwrap().to_owned().leak(),
        ];
        args.extend_from_slice(extra);
        bin().args(args).assert().success();
        std::fs::read(out).unwrap()
    };
    let is_webp = |b: &[u8]| &b[..4] == b"RIFF" && &b[8..12] == b"WEBP";
    // without steps: still image (VP8L chunk; VP8X when the source is embedded, without ANIM);
    // with steps: animation (VP8X + ANIM chunks)
    let bare = render(
        "reseau.pug",
        "w-bare.webp",
        &["--scale", "1", "--no-source"],
    );
    assert!(is_webp(&bare) && &bare[12..16] == b"VP8L");
    let still = render("reseau.pug", "w-still.webp", &["--scale", "1"]);
    assert!(is_webp(&still) && &still[12..16] == b"VP8X");
    assert!(!still.windows(4).any(|w| w == b"ANIM"));
    let anim = render(
        "animation.pug",
        "w-anim.webp",
        &["--fps", "4", "--hold", "0.3"],
    );
    assert!(is_webp(&anim) && &anim[12..16] == b"VP8X");
    assert!(anim.windows(4).any(|w| w == b"ANIM") && anim.windows(4).any(|w| w == b"ANMF"));
    let state = render(
        "animation.pug",
        "w-state.webp",
        &["--step", "2", "--scale", "1"],
    );
    assert!(is_webp(&state) && &state[12..16] == b"VP8X");
    assert!(!state.windows(4).any(|w| w == b"ANIM"));
}

#[test]
fn bare_file_renders_a_png_next_to_the_source() {
    let dir = tmp("bare");
    std::fs::create_dir_all(&dir).unwrap();
    let src = dir.join("demo.skemo");
    std::fs::copy(fx("reseau.pug"), &src).unwrap();
    let _ = std::fs::remove_file(dir.join("demo.png"));
    bin().arg(&src).assert().success();
    let png = std::fs::read(dir.join("demo.png")).unwrap();
    assert_eq!(&png[1..4], b"PNG");
    // --format changes the default extension; the global options may come before the file
    bin()
        .args(["--quiet", src.to_str().unwrap(), "--format", "svg"])
        .assert()
        .success();
    assert!(dir.join("demo.svg").exists());
}

#[test]
fn bare_command_names_still_work_and_stdin_needs_an_output() {
    bin().arg("--version").assert().success();
    bin()
        .args(["check", fx("reseau.pug").to_str().unwrap()])
        .assert()
        .success();
    bin()
        .args(["-", "--format", "svg"])
        .write_stdin("diagram\n")
        .assert()
        .code(2);
}

#[test]
fn fmt_reads_stdin_and_reports_errors() {
    let out = bin()
        .args(["fmt", "-"])
        .write_stdin("card#a(step=2) A\n")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    assert_eq!(String::from_utf8(out).unwrap(), "card#a A @2\n");
    let bad = tmp("fmt-bad.skemo");
    std::fs::write(&bad, "card#a(\n").unwrap();
    bin().args(["fmt", bad.to_str().unwrap()]).assert().code(1);
    bin().args(["fmt", "/nonexistent/x.skemo"]).assert().code(2);
}

#[test]
fn theme_needs_print_and_check_reports_errors() {
    bin().arg("theme").assert().code(2);
    let out = bin()
        .args(["theme", "--print"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    assert!(String::from_utf8(out).unwrap().contains(".stage"));
    let bad = tmp("check-bad.skemo");
    std::fs::write(&bad, "card#a\n  to(zz)\n").unwrap();
    let err = bin()
        .args(["check", bad.to_str().unwrap()])
        .assert()
        .code(1)
        .get_output()
        .stderr
        .clone();
    assert!(String::from_utf8_lossy(&err).contains("zz"));
    // check keeps going over several files: an unreadable one is a failure (1), not a usage error
    bin()
        .args(["check", "/nonexistent/x.skemo"])
        .assert()
        .code(1);
}

fn stderr_of(mut c: Command, args: &[&str]) -> String {
    let out = c.args(args).assert().failure().get_output().stderr.clone();
    String::from_utf8_lossy(&out).into_owned()
}

/// The language follows the system (LANG…), `SKEMO_LANG` beats it, `--lang` beats everything.
#[test]
fn messages_follow_the_system_language() {
    let bad = tmp("lang-bad.skemo");
    std::fs::write(&bad, "card#a\n  to(zz)\n").unwrap();
    let f = bad.to_str().unwrap();
    let en = stderr_of(bin_env(&[]), &["check", f]);
    assert!(en.contains("error[unknown-target]: unknown target"), "{en}");
    let fr = stderr_of(bin_env(&[("LANG", "fr_FR.UTF-8")]), &["check", f]);
    assert!(
        fr.contains("erreur[unknown-target] : cible « zz » inconnue"),
        "{fr}"
    );
    // LC_ALL beats LANG; C/POSIX fall through to the next variable
    assert!(
        stderr_of(
            bin_env(&[("LANG", "fr_FR"), ("LC_ALL", "en_US.UTF-8")]),
            &["check", f]
        )
        .contains("unknown target")
    );
    assert!(
        stderr_of(
            bin_env(&[("LC_ALL", "C"), ("LANG", "fr_FR")]),
            &["check", f]
        )
        .contains("cible")
    );
    // SKEMO_LANG and --lang
    assert!(
        stderr_of(
            bin_env(&[("LANG", "fr_FR"), ("SKEMO_LANG", "en")]),
            &["check", f]
        )
        .contains("unknown target")
    );
    assert!(
        stderr_of(bin_env(&[("LANG", "fr_FR")]), &["--lang", "en", "check", f])
            .contains("unknown target")
    );
    assert!(stderr_of(bin_env(&[]), &["check", f, "--lang=fr"]).contains("cible"));
    // an unsupported language falls back to English
    assert!(
        stderr_of(bin_env(&[("LANG", "de_DE.UTF-8")]), &["check", f]).contains("unknown target")
    );
}

#[test]
fn errors_and_help_are_translated_too() {
    let fr = stderr_of(
        bin_env(&[("LANG", "fr_FR.UTF-8")]),
        &["check", "/nonexistent/x.skemo"],
    );
    assert!(
        fr.starts_with("erreur : lecture de « /nonexistent/x.skemo »"),
        "{fr}"
    );
    let en = stderr_of(bin_env(&[]), &["check", "/nonexistent/x.skemo"]);
    assert!(
        en.starts_with("error: reading « /nonexistent/x.skemo »"),
        "{en}"
    );
    let help = |vars: &[(&str, &str)]| {
        let out = bin_env(vars)
            .args(["render", "--help"])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        String::from_utf8_lossy(&out).into_owned()
    };
    assert!(help(&[("LANG", "fr_FR.UTF-8")]).contains("Fichier de sortie"));
    assert!(help(&[("LANG", "en_US.UTF-8")]).contains("Output file"));
    assert!(help(&[]).contains("Output file"));
    let top = bin_env(&[("LANG", "fr")])
        .arg("--help")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    assert!(String::from_utf8_lossy(&top).contains("Diagrammes texte"));
}

#[test]
fn drawing_labels_follow_the_language_unless_pinned() {
    let src = tmp("lang-timeline.skemo");
    std::fs::write(&src, "diagram(layout=\"timeline\" scale=\"quarter\")\n  card#a(start=\"2026-01-10\" end=\"2026-09-30\") P\n").unwrap();
    let render = |vars: &[(&str, &str)], extra: &str| {
        let s = std::fs::read_to_string(&src)
            .unwrap()
            .replace("diagram(", &format!("diagram({extra}"));
        let f = tmp("lang-timeline-run.skemo");
        std::fs::write(&f, s).unwrap();
        let out = bin_env(vars)
            .args(["render", f.to_str().unwrap(), "--format", "svg", "-o", "-"])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        String::from_utf8_lossy(&out).into_owned()
    };
    assert!(render(&[], "").contains("Q2"));
    assert!(render(&[("LANG", "fr_FR.UTF-8")], "").contains("T2"));
    assert!(
        render(&[("LANG", "fr_FR.UTF-8")], "lang=\"en\" ").contains("Q2"),
        "the diagram option pins the labels"
    );
}

#[test]
fn icons_lists_and_filters_the_builtin_set() {
    let all = bin().arg("icons").output().unwrap();
    let all = String::from_utf8(all.stdout).unwrap();
    assert!(all.lines().count() > 1500 && all.lines().any(|l| l == "database"));
    let one = bin().args(["icons", "databas"]).output().unwrap();
    let one = String::from_utf8(one.stdout).unwrap();
    assert!(one.lines().next() == Some("database"), "{one}");
    let json = bin()
        .args(["icons", "database", "--json"])
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&json.stdout).unwrap();
    assert_eq!(v[0]["name"], "database");
    assert!(v[0]["tags"].as_array().is_some_and(|t| !t.is_empty()));
}

#[test]
fn check_reports_unknown_icons_and_unsafe_links() {
    let f = tmp("liens.skemo");
    std::fs::write(&f, "card#a(icon=\"databse\") A\n").unwrap();
    let out = bin().arg("check").arg(&f).output().unwrap();
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success());
    assert!(
        err.contains("unknown-icon") || err.contains("unknown icon"),
        "{err}"
    );
    assert!(err.contains("database"), "suggestion: {err}");
    bin()
        .args(["check", "--deny-warnings"])
        .arg(&f)
        .assert()
        .failure();
    std::fs::write(&f, "card#a(href=\"javascript:alert(1)\") A\n").unwrap();
    let out = bin().arg("check").arg(&f).output().unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("unsafe-href"));
}

const WIDE: &str = "diagram(fit=\"16:9\")\ncard#lb Load balancer\n  to(p1)\nzone#ns Namespace\n  card#p1 pod-1\n  card#p2 pod-2\n  card#p3 pod-3\n  card#p4 pod-4\n  card#p5 pod-5\n  card#p6 pod-6\n  card#p7 pod-7\n";

#[test]
fn fit_lists_suggestions_and_writes_the_best() {
    let src = tmp("wide.skemo");
    std::fs::write(&src, WIDE).unwrap();
    let out = bin()
        .args(["fit", src.to_str().unwrap()])
        .assert()
        .success();
    let text = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(
        text.contains("target 16:9 (1.78)") && text.contains("1. ") && text.contains("text ×"),
        "{text}"
    );
    bin()
        .args(["fit", src.to_str().unwrap(), "--write"])
        .assert()
        .success();
    let written = std::fs::read_to_string(&src).unwrap();
    assert_ne!(written, WIDE);
    assert!(
        written.contains("wrap=") || written.contains("dir="),
        "{written}"
    );
    // the rendering suggests nothing more (it may still say the target is out of reach, D-045)
    let png = tmp("wide.png");
    let out = bin()
        .args(["render", src.to_str().unwrap(), "-o", png.to_str().unwrap()])
        .assert()
        .success();
    let err = String::from_utf8(out.get_output().stderr.clone()).unwrap();
    assert!(!err.contains("--write"), "{err}");
}

#[test]
fn fit_needs_a_target() {
    let src = tmp("plain.skemo");
    std::fs::write(&src, "card#a A\n").unwrap();
    bin().args(["fit", src.to_str().unwrap()]).assert().code(2);
    bin()
        .args(["fit", src.to_str().unwrap(), "--target", "wide"])
        .assert()
        .code(2);
}

#[test]
fn font_scale_and_embedded_fonts() {
    let src = tmp("scale.skemo");
    std::fs::write(&src, "card#a(tag=\"v1 @1\") A\n").unwrap();
    let svg = tmp("scale.svg");
    bin()
        .args(["render", src.to_str().unwrap(), "-o", svg.to_str().unwrap()])
        .args(["--font-scale", "1.5", "--embed-fonts"])
        .assert()
        .success();
    let s = std::fs::read_to_string(&svg).unwrap();
    assert!(s.contains("@font-face") && s.contains("data-states="));
    assert!(s.contains("font-size=\"21\""), "14 px × 1.5");
    bin()
        .args(["render", src.to_str().unwrap(), "-o", svg.to_str().unwrap()])
        .args(["--font-scale", "0"])
        .assert()
        .code(2);
}

#[test]
fn rendered_images_carry_their_source_and_reopen() {
    let src = tmp("embed.skemo");
    let css = tmp("embed.css");
    std::fs::write(&src, "card#a.t1 Alpha  @1\n  to(b)\ncard#b Beta  @2\n").unwrap();
    std::fs::write(&css, ".t1 { --c: #C94A44; --label: \"T1\"; }\n").unwrap();
    for ext in ["png", "svg", "html", "gif", "apng", "webp"] {
        let out = tmp(&format!("embed-out.{ext}"));
        bin()
            .args([
                "render",
                src.to_str().unwrap(),
                "--css",
                css.to_str().unwrap(),
            ])
            .args(["--font-scale", "1.4", "-o", out.to_str().unwrap()])
            .assert()
            .success();
        // any command takes the image instead of the source, with its stylesheets
        let again = tmp(&format!("embed-again.{ext}.svg"));
        bin()
            .args([
                "render",
                out.to_str().unwrap(),
                "-o",
                again.to_str().unwrap(),
            ])
            .assert()
            .success();
        let first = tmp("embed-first.svg");
        bin()
            .args([
                "render",
                src.to_str().unwrap(),
                "--css",
                css.to_str().unwrap(),
            ])
            .args(["--font-scale", "1.4", "-o", first.to_str().unwrap()])
            .assert()
            .success();
        assert_eq!(
            std::fs::read(&first).unwrap(),
            std::fs::read(&again).unwrap(),
            "{ext}: same drawing from the image"
        );
        // extract writes the source and the stylesheet back
        let dir = tmp(&format!("embed-back-{ext}"));
        bin()
            .args([
                "extract",
                out.to_str().unwrap(),
                "-o",
                dir.to_str().unwrap(),
                "--force",
            ])
            .assert()
            .success();
        let back = std::fs::read_to_string(dir.join("embed-out.skemo")).unwrap();
        assert!(back.starts_with("card#a.t1 Alpha"), "{ext}");
        assert!(
            std::fs::read_to_string(dir.join("embed.css"))
                .unwrap()
                .contains("C94A44")
        );
    }
}

#[test]
fn no_source_leaves_nothing_to_extract() {
    let src = tmp("bare.skemo");
    std::fs::write(&src, "card#a A\n").unwrap();
    let out = tmp("bare.png");
    bin()
        .args([
            "render",
            src.to_str().unwrap(),
            "--no-source",
            "-o",
            out.to_str().unwrap(),
        ])
        .assert()
        .success();
    bin()
        .args(["extract", out.to_str().unwrap()])
        .assert()
        .code(1);
}

/// `check` lays the diagram out and reports the engine's warnings (here `fit`), not only the
/// parser's (D-045).
#[test]
fn check_reports_layout_warnings() {
    let src = tmp("fit-check.skemo");
    std::fs::write(
        &src,
        "diagram(dir=\"down\" fit=\"2:1\")\ncard#a A\n  to(b)\ncard#b B\n  to(c)\ncard#c C\n",
    )
    .unwrap();
    let path = src.to_str().unwrap();
    let out = bin().args(["check", path]).assert().success();
    let err = String::from_utf8_lossy(&out.get_output().stderr).to_string();
    assert!(err.contains("fit=\"2:1\""), "{err}");
    bin()
        .args(["check", "--deny-warnings", path])
        .assert()
        .code(1);
}

/// Image icons (D-046): read next to the source, carried in the rendered file, written back by
/// `extract`; a path that leaves the source's folder is refused.
#[test]
fn image_icons_are_read_next_to_the_source_and_travel_with_the_image() {
    let dir = tmp("img-icons");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("img")).unwrap();
    std::fs::write(
        dir.join("img/logo.svg"),
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 24 24\"><circle cx=\"12\" cy=\"12\" r=\"9\"/></svg>",
    )
    .unwrap();
    let src = dir.join("d.skemo");
    std::fs::write(&src, "card#a(icon=\"img/logo.svg\") A\n").unwrap();
    let png = dir.join("d.png");
    bin()
        .args([
            "render",
            src.to_str().unwrap(),
            "-o",
            png.to_str().unwrap(),
            "--deny-warnings",
        ])
        .assert()
        .success();
    let out = dir.join("back");
    bin()
        .args([
            "extract",
            png.to_str().unwrap(),
            "-o",
            out.to_str().unwrap(),
        ])
        .assert()
        .success();
    assert!(out.join("img/logo.svg").exists());
    // the image alone renders again, with its icon, from anywhere
    let again = tmp("img-icons-again.svg");
    bin()
        .args([
            "render",
            png.to_str().unwrap(),
            "-o",
            again.to_str().unwrap(),
            "--deny-warnings",
        ])
        .assert()
        .success();
    assert!(
        std::fs::read_to_string(&again)
            .unwrap()
            .contains("<image class=\"icon\"")
    );
    std::fs::write(&src, "card#a(icon=\"../logo.svg\") A\n").unwrap();
    bin()
        .args(["render", src.to_str().unwrap(), "-o", png.to_str().unwrap()])
        .assert()
        .code(2);
}
