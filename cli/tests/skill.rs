//! The public agent skill (`skill/skemo/`) must only teach diagrams that render: every `pug` block of its
//! Markdown files is rendered with `--deny-warnings`, with the `css` block that follows it, if any.

use assert_cmd::Command;
use std::path::{Path, PathBuf};

/// The `(language, body)` fenced blocks of a Markdown file, in order.
fn blocks(md: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut open: Option<(String, String)> = None;
    for line in md.lines() {
        match (&mut open, line.strip_prefix("```")) {
            (None, Some(lang)) => open = Some((lang.trim().to_string(), String::new())),
            (Some(_), Some(_)) => out.push(open.take().unwrap()),
            (Some((_, body)), None) => {
                body.push_str(line);
                body.push('\n');
            }
            (None, None) => {}
        }
    }
    out
}

fn markdown_files(dir: &Path, found: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            markdown_files(&path, found);
        } else if path.extension().is_some_and(|e| e == "md") {
            found.push(path);
        }
    }
}

#[test]
fn skill_examples_render_without_warnings() {
    let mut files = Vec::new();
    markdown_files(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("skill"),
        &mut files,
    );
    assert!(!files.is_empty());
    let tmp = Path::new(env!("CARGO_TARGET_TMPDIR")).join("skill-tests");
    std::fs::create_dir_all(&tmp).unwrap();
    let mut rendered = 0;
    for file in &files {
        let all = blocks(&std::fs::read_to_string(file).unwrap());
        for (i, (lang, src)) in all.iter().enumerate() {
            if lang != "pug" {
                continue;
            }
            let mut cmd = Command::cargo_bin("skemo").unwrap();
            cmd.env("SKEMO_LANG", "en").args([
                "render",
                "-",
                "--format",
                "svg",
                "-o",
                "-",
                "--deny-warnings",
            ]);
            if let Some((next, css)) = all.get(i + 1).filter(|(l, _)| l == "css") {
                assert_eq!(next, "css");
                let path = tmp.join(format!("block{rendered}.css"));
                std::fs::write(&path, css).unwrap();
                cmd.arg("--css").arg(path);
            }
            let out = cmd.write_stdin(src.as_str()).output().unwrap();
            assert!(
                out.status.success() && out.stdout.starts_with(b"<svg"),
                "{} block {i} does not render:\n{src}\n{}",
                file.display(),
                String::from_utf8_lossy(&out.stderr)
            );
            rendered += 1;
        }
    }
    assert!(rendered >= 8, "only {rendered} examples found");
}
