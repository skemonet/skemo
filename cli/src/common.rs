//! Reading the inputs, writing the outputs and preparing a diagram, shared by the commands.

use crate::args::ThemeArg;
use crate::out::Out;
use skemo_engine::source::{Bundle, NamedText, embed, extract, safe_name};
use skemo_lang::style::Theme;
use skemo_lang::tr;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

/// Failure: exit code (SPEC §2) and possible message.
#[derive(Debug)]
pub struct Fail(pub u8, pub Option<String>);

impl Fail {
    /// Mauvaise utilisation (code 2).
    pub fn usage(msg: impl Into<String>) -> Fail {
        Fail(2, Some(msg.into()))
    }

    /// Erreur interne (code 3).
    pub fn internal(msg: impl Into<String>) -> Fail {
        Fail(3, Some(msg.into()))
    }

    /// Errors of the source file, already shown (code 1).
    pub fn source() -> Fail {
        Fail(1, None)
    }
}

pub fn read_source(file: &str) -> Result<String, String> {
    if file == "-" {
        let mut s = String::new();
        std::io::stdin().read_to_string(&mut s).map_err(|e| {
            tr!(
                "reading standard input: {e}",
                "lecture de l'entrée standard : {e}"
            )
        })?;
        Ok(s)
    } else {
        std::fs::read_to_string(file)
            .map_err(|e| tr!("reading « {file} »: {e}", "lecture de « {file} » : {e}"))
    }
}

pub fn read_file(p: &Path) -> Result<String, String> {
    std::fs::read_to_string(p).map_err(|e| {
        tr!(
            "reading « {} »: {e}",
            "lecture de « {} » : {e}",
            p.display()
        )
    })
}

pub fn read_css(paths: &[PathBuf]) -> Result<Vec<String>, String> {
    paths.iter().map(|p| read_file(p)).collect()
}

/// Writes `data` to `path` (`-`: standard output), creating the missing folders.
pub fn write_out(path: &str, data: &[u8]) -> Result<(), Fail> {
    if path == "-" {
        return std::io::stdout()
            .write_all(data)
            .map_err(|e| Fail::internal(tr!("writing: {e}", "écriture : {e}")));
    }
    if let Some(dir) = Path::new(path)
        .parent()
        .filter(|d| !d.as_os_str().is_empty())
    {
        std::fs::create_dir_all(dir).map_err(|e| {
            Fail::internal(tr!(
                "creating « {} »: {e}",
                "création de « {} » : {e}",
                dir.display()
            ))
        })?;
    }
    std::fs::write(path, data).map_err(|e| {
        Fail::internal(tr!(
            "writing « {path} »: {e}",
            "écriture de « {path} » : {e}"
        ))
    })
}

/// What is needed to prepare a diagram.
#[derive(Clone, Copy)]
pub struct Inputs<'a> {
    pub file: &'a str,
    pub css: &'a [PathBuf],
    pub theme: Option<ThemeArg>,
    pub base_theme: Option<&'a PathBuf>,
    pub high_contrast: bool,
    pub transparent: bool,
    /// `--font-scale`: appended last, so it wins over the CSS files.
    pub font_scale: Option<f32>,
    /// `--font`: font files to use besides the embedded ones.
    pub fonts: &'a [PathBuf],
}

impl<'a> Inputs<'a> {
    pub fn new(file: &'a str, css: &'a [PathBuf], base_theme: Option<&'a PathBuf>) -> Self {
        Inputs {
            file,
            css,
            theme: None,
            base_theme,
            high_contrast: false,
            transparent: false,
            font_scale: None,
            fonts: &[],
        }
    }

    pub fn fonts(self, fonts: &'a [PathBuf]) -> Self {
        Inputs { fonts, ..self }
    }

    pub fn font_scale(self, font_scale: Option<f32>) -> Self {
        Inputs { font_scale, ..self }
    }

    pub fn theme(self, theme: Option<ThemeArg>) -> Self {
        Inputs { theme, ..self }
    }

    pub fn transparent(self, transparent: bool) -> Self {
        Inputs {
            transparent,
            ..self
        }
    }

    pub fn high_contrast(self, high_contrast: bool) -> Self {
        Inputs {
            high_contrast,
            ..self
        }
    }
}

/// Source and stylesheets of a diagram, read from the files, or from a rendered image that
/// carries them (D-042), plus the bundle to embed in what is written.
pub struct Loaded {
    pub src: String,
    pub options: skemo_engine::Options,
    pub bundle: Bundle,
}

fn read_bytes(file: &str) -> Result<Vec<u8>, String> {
    if file == "-" {
        let mut b = Vec::new();
        std::io::stdin().read_to_end(&mut b).map_err(|e| {
            tr!(
                "reading standard input: {e}",
                "lecture de l'entrée standard : {e}"
            )
        })?;
        Ok(b)
    } else {
        std::fs::read(file)
            .map_err(|e| tr!("reading « {file} »: {e}", "lecture de « {file} » : {e}"))
    }
}

fn named(p: &Path, content: String) -> NamedText {
    NamedText {
        name: p
            .file_name()
            .and_then(|n| n.to_str())
            .and_then(safe_name)
            .unwrap_or_else(|| "style.css".into()),
        content,
    }
}

fn theme_name(t: ThemeArg) -> &'static str {
    match t {
        ThemeArg::Light => "light",
        ThemeArg::Dark => "dark",
        ThemeArg::Auto => "auto",
        ThemeArg::Print => "print",
    }
}

/// Reads the inputs. An image that carries a bundle gives its source, and its stylesheets and
/// options unless the command line gives its own.
pub fn load(i: &Inputs) -> Result<Loaded, Fail> {
    let raw = read_bytes(i.file).map_err(Fail::usage)?;
    let mut b = if let Some(b) = extract(&raw) {
        b
    } else {
        let src = String::from_utf8(raw).map_err(|_| {
            Fail::usage(tr!(
                "« {} » is neither a text file nor an image that carries a skemo source",
                "« {} » n'est ni un fichier texte ni une image qui contient une source skemo",
                i.file
            ))
        })?;
        let mut b = Bundle::new(&src);
        b.name = Path::new(i.file)
            .file_name()
            .and_then(|n| n.to_str())
            .and_then(safe_name)
            .filter(|_| i.file != "-");
        b
    };
    if !i.css.is_empty() || b.css.is_empty() {
        b.css = i
            .css
            .iter()
            .map(|p| read_file(p).map(|c| named(p, c)))
            .collect::<Result<_, _>>()
            .map_err(Fail::usage)?;
    }
    if let Some(p) = i.base_theme {
        b.base_theme = Some(named(p, read_file(p).map_err(Fail::usage)?));
    }
    if let Some(t) = i.theme {
        b.theme = Some(theme_name(t).into());
    }
    if let Some(k) = i.font_scale {
        if !(k.is_finite() && k > 0.0 && k <= 4.0) {
            return Err(Fail::usage(tr!(
                "--font-scale: expected a number between 0 and 4, got {k}",
                "--font-scale : nombre entre 0 et 4 attendu, reçu {k}"
            )));
        }
        // shortest decimal form of the f32 (1.4, not 1.399999976)
        b.font_scale = k.to_string().parse().ok();
    }
    let mut css: Vec<String> = b.css.iter().map(|c| c.content.clone()).collect();
    if let Some(k) = b.font_scale {
        css.push(format!(".stage {{ --font-scale: {k}; }}"));
    }
    let images = load_images(i.file, &mut b, &css)?;
    let fonts = i
        .fonts
        .iter()
        .map(|p| {
            let data = std::fs::read(p).map_err(|e| {
                Fail::usage(tr!("font « {} »: {e}", "police « {} » : {e}", p.display()))
            })?;
            if skemo_engine::fonts::describe(&data).is_none() {
                return Err(Fail::usage(tr!(
                    "font « {} »: not a TrueType or OpenType file",
                    "police « {} » : ce n'est pas un fichier TrueType ou OpenType",
                    p.display()
                )));
            }
            Ok(data)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let options = skemo_engine::Options {
        css,
        theme: b.theme.as_deref().and_then(Theme::parse),
        base_theme: b.base_theme.as_ref().map(|t| t.content.clone()),
        high_contrast: i.high_contrast,
        transparent: i.transparent,
        images,
        fonts,
    };
    Ok(Loaded {
        src: b.source.clone(),
        options,
        bundle: b,
    })
}

/// Image files used as icons (D-046): read next to the source (relative paths inside its folder),
/// or taken from the bundle of an image given as input. They go into the bundle, so that the
/// rendered file reopens with them. A refused name or a missing file stops the command.
pub fn load_images(
    file: &str,
    b: &mut Bundle,
    css: &[String],
) -> Result<std::collections::BTreeMap<String, Vec<u8>>, Fail> {
    let model = skemo_lang::parse(&b.source).model;
    let names = skemo_engine::images::referenced(&model, css);
    let base = Path::new(file).parent().filter(|_| file != "-");
    let mut out = std::collections::BTreeMap::new();
    let mut kept = Vec::new();
    for name in names {
        skemo_engine::images::check_name(&name).map_err(Fail::usage)?;
        let data = if let Some(i) = b.images.iter().find(|i| i.name == name) {
            i.bytes().unwrap_or_default()
        } else {
            let path = base.map_or_else(|| PathBuf::from(&name), |d| d.join(&name));
            std::fs::read(&path).map_err(|e| {
                Fail::usage(tr!(
                    "image « {name} » ({}): {e}",
                    "image « {name} » ({}) : {e}",
                    path.display()
                ))
            })?
        };
        if data.len() > skemo_engine::images::MAX_BYTES {
            return Err(Fail::usage(tr!(
                "image « {name} »: larger than 1 MB",
                "image « {name} » : plus de 1 Mo"
            )));
        }
        kept.push(skemo_engine::source::NamedBytes::new(&name, &data));
        out.insert(name, data);
    }
    b.images = kept;
    Ok(out)
}

/// The rendered bytes with the bundle inside, unless `bundle` is `None` (`--no-source`).
pub fn with_source(bytes: Vec<u8>, bundle: Option<&Bundle>) -> Vec<u8> {
    match bundle {
        Some(b) => embed(bytes, b),
        None => bytes,
    }
}

/// Reads, parses and prepares the diagram; shows the diagnostics. Also returns the number of
/// warnings and the source bundle to embed (D-042).
pub fn prepare(out: &Out, i: Inputs) -> Result<(skemo_engine::Diagram, usize, Bundle), Fail> {
    let Loaded {
        src,
        options,
        bundle,
    } = load(&i)?;
    let name = if i.file == "-" { "<stdin>" } else { i.file };
    match skemo_engine::from_source(&src, &options) {
        Ok((d, warnings)) => {
            for w in &warnings {
                out.diag(&src, name, w);
            }
            for w in &d.warnings {
                out.warn(w);
            }
            let n = warnings.len() + d.warnings.len();
            Ok((d, n, bundle))
        }
        Err(skemo_engine::Error::Source(diags)) => {
            for e in &diags {
                out.diag(&src, name, e);
            }
            Err(Fail::source())
        }
        Err(skemo_engine::Error::Internal(m)) => Err(Fail::internal(m)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fail_constructors_use_the_documented_exit_codes() {
        assert_eq!(Fail::usage("x").0, 2);
        assert_eq!(Fail::internal("x").0, 3);
        let s = Fail::source();
        assert_eq!((s.0, s.1), (1, None));
    }

    #[test]
    fn write_out_creates_missing_directories() {
        let dir = std::env::temp_dir().join(format!("skemo-test-{}", std::process::id()));
        let file = dir.join("a/b/c.txt");
        write_out(file.to_str().unwrap(), b"ok").unwrap();
        assert_eq!(std::fs::read(&file).unwrap(), b"ok");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn missing_input_is_reported_with_its_name() {
        let e = read_source("/nonexistent/skemo.skemo").unwrap_err();
        assert!(e.contains("/nonexistent/skemo.skemo"));
    }
}
