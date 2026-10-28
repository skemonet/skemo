//! Command-line arguments (clap). Help texts are written in English here; the French ones are
//! in `help.rs` and applied at run time (the language follows the system, see `i18n`).

use clap::{Args, Parser, Subcommand, ValueEnum};
use skemo_lang::style::Theme;
use skemo_lang::tr;
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "skemo",
    version,
    about = "Text diagrams (skemo): HTML, SVG, PNG, GIF, APNG and WebP rendering",
    disable_help_subcommand = true
)]
pub struct Cli {
    /// Only show errors
    #[arg(long, short, global = true)]
    pub quiet: bool,
    /// Colours in the messages
    #[arg(long, value_enum, default_value_t = Color::Auto, global = true)]
    pub color: Color,
    /// Language of the messages and of the help (default: the system language, `SKEMO_LANG`,
    /// `LC_ALL`, `LC_MESSAGES`, `LANGUAGE`, `LANG`)
    #[arg(long, value_name = "en|fr", global = true)]
    pub lang: Option<String>,
    #[command(subcommand)]
    pub cmd: Cmd,
}

#[derive(Clone, Copy, ValueEnum)]
pub enum Color {
    Auto,
    Always,
    Never,
}

#[derive(Clone, Copy, ValueEnum, PartialEq, Eq, Debug)]
pub enum ThemeArg {
    Light,
    Dark,
    Auto,
    Print,
}

impl From<ThemeArg> for Theme {
    fn from(t: ThemeArg) -> Theme {
        match t {
            ThemeArg::Light => Theme::Light,
            ThemeArg::Dark => Theme::Dark,
            ThemeArg::Auto => Theme::Auto,
            ThemeArg::Print => Theme::Print,
        }
    }
}

#[derive(Clone, Copy, ValueEnum, PartialEq, Eq, Debug)]
pub enum Format {
    Html,
    Svg,
    Png,
    Gif,
    /// Animated PNG
    Apng,
    /// WebP: still, or animated if the diagram has steps
    Webp,
}

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ContrastArg {
    Standard,
    High,
}

#[derive(Subcommand)]
pub enum Cmd {
    /// Render a diagram; the format follows the output extension (PNG by default).
    /// This is the default command: `skemo demo.skemo` writes `demo.png`
    Render(RenderArgs),
    /// Check files without rendering them
    Check(CheckArgs),
    /// Canonical formatting (indentation, attribute order, @ markers)
    Fmt(FmtArgs),
    /// Create a sample `example.skemo` and a commented `custom.css` (overwrites nothing)
    Init {
        /// Destination folder (created if needed)
        #[arg(default_value = ".")]
        dir: PathBuf,
    },
    /// Live preview: re-render on every save, the browser reloads by itself
    Watch(WatchArgs),
    /// Full layout as layout.json (on the standard output)
    Layout(LayoutArgs),
    /// Suggest settings (dir, wrap) that show the diagram larger in a target box (ratio or size)
    Fit(FitArgs),
    /// Get back the source and stylesheets embedded in a rendered image (PNG, SVG, WebP, GIF,
    /// APNG, HTML) to edit them
    Extract(ExtractArgs),
    /// List the built-in icons (Lucide), optionally filtered by a query on names and tags
    Icons {
        /// Text to look for in the names and the tags (all icons without it)
        query: Option<String>,
        /// Print a JSON array of `{name, tags}`
        #[arg(long)]
        json: bool,
    },
    /// Licences and copyright notices of what the binary embeds: fonts, icons, Rust crates
    Licences,
    /// Default theme
    Theme {
        /// Write the default theme (a starting point for a custom theme)
        #[arg(long)]
        print: bool,
    },
}

#[derive(Args)]
pub struct RenderArgs {
    /// Source file (`-` = standard input)
    pub file: String,
    /// Output file (.html, .svg, .png, .gif, .apng, .webp; `-` = standard output, with
    /// --format). Default: next to the source, same name, as PNG (or per --format)
    #[arg(short, long, default_value = "")]
    pub output: String,
    /// Format, when the extension is not enough
    #[arg(long, value_enum)]
    pub format: Option<Format>,
    /// Custom CSS, applied after the theme (repeatable)
    #[arg(long)]
    pub css: Vec<PathBuf>,
    /// Theme of the rendering (overrides diagram(theme=…))
    #[arg(long, value_enum)]
    pub theme: Option<ThemeArg>,
    /// Scale of the PNG (and of the GIF or APNG)
    #[arg(long)]
    pub scale: Option<f32>,
    #[command(flatten)]
    pub png: PngArgs,
    #[command(flatten)]
    pub anim: AnimArgs,
    #[command(flatten)]
    pub fonts: FontArgs,
    /// Multiply every text size of the theme (1.4 for a projected slide); sets `--font-scale`
    #[arg(long, value_name = "FACTOR")]
    pub font_scale: Option<f32>,
    /// Fail if a warning is emitted
    #[arg(long)]
    pub deny_warnings: bool,
    /// Do not embed the source and stylesheets in the output (they are by default, so that
    /// the image can be reopened: skemo extract)
    #[arg(long)]
    pub no_source: bool,
    /// Base theme replacing the default theme
    #[arg(long)]
    pub base_theme: Option<PathBuf>,
    /// Transparent background (SVG, PNG, APNG and board): no background rectangle; no effect
    /// for HTML and GIF
    #[arg(long)]
    pub transparent: bool,
    /// Text contrast: `high` darkens (or lightens) every text below the WCAG threshold
    /// (4.5:1; 3:1 on an accent fill), without touching the layout
    #[arg(long, value_enum, default_value_t = ContrastArg::Standard)]
    pub contrast: ContrastArg,
}

/// Fonts in the HTML and SVG outputs.
#[derive(Args)]
pub struct FontArgs {
    /// HTML: do not embed the fonts
    #[arg(long)]
    pub no_fonts: bool,
    /// SVG: embed the fonts used (about 0.8 MB per weight), for viewers without Inter
    #[arg(long)]
    pub embed_fonts: bool,
    /// A font file (TrueType or OpenType) to use besides the embedded ones, wherever the CSS
    /// names its family (repeatable: one file per weight or style)
    #[arg(long, value_name = "FILE")]
    pub font: Vec<PathBuf>,
}

/// PNG options (one given state, one file per state, board).
#[derive(Args)]
pub struct PngArgs {
    /// Render the state at this step (value as written: 2.5)
    #[arg(long)]
    pub step: Option<String>,
    /// PNG: one file per state (name.00.png…) and name.states.json
    #[arg(long)]
    pub animated: bool,
    /// PNG: board of the steps (one state per row)
    #[arg(long)]
    pub board: bool,
}

/// Options of the animated outputs (GIF and APNG).
#[derive(Args)]
pub struct AnimArgs {
    /// GIF and APNG: hold time of each state, in seconds
    #[arg(long, default_value_t = 1.6)]
    pub hold: f64,
    /// GIF and APNG: duration of the transitions, in seconds
    #[arg(long, default_value_t = 0.45)]
    pub transition: f64,
    /// GIF and APNG: frames per second
    #[arg(long, default_value_t = 20)]
    pub fps: u32,
    /// GIF and APNG: do not loop
    #[arg(long)]
    pub no_loop: bool,
    /// GIF and APNG: hold time of the last frame, in seconds
    #[arg(long, default_value_t = 3.0)]
    pub final_hold: f64,
}

#[derive(Args)]
pub struct CheckArgs {
    /// Files to check
    #[arg(required = true)]
    pub files: Vec<String>,
    /// Fail if a warning is emitted
    #[arg(long)]
    pub deny_warnings: bool,
    /// Also check the text contrast in the 3 themes (light, dark, print)
    #[arg(long)]
    pub contrast: bool,
    /// Custom CSS (for --contrast)
    #[arg(long)]
    pub css: Vec<PathBuf>,
    /// Base theme (for --contrast)
    #[arg(long)]
    pub base_theme: Option<PathBuf>,
}

#[derive(Args)]
pub struct FmtArgs {
    #[arg(required = true)]
    pub files: Vec<String>,
    /// Write nothing: exit code 1 if a file is not formatted
    #[arg(long)]
    pub check: bool,
}

#[derive(Args)]
pub struct WatchArgs {
    pub file: String,
    /// Also write the rendering to this file on every change
    #[arg(short, long)]
    pub output: Option<String>,
    #[arg(long)]
    pub css: Vec<PathBuf>,
    #[arg(long, value_enum)]
    pub theme: Option<ThemeArg>,
    #[arg(long)]
    pub base_theme: Option<PathBuf>,
    /// Port of the local server
    #[arg(long, default_value_t = 8710)]
    pub port: u16,
}

#[derive(Args)]
pub struct LayoutArgs {
    pub file: String,
    /// JSON output (the only format available)
    #[arg(long)]
    pub json: bool,
    #[arg(long)]
    pub css: Vec<PathBuf>,
    #[arg(long, value_enum)]
    pub theme: Option<ThemeArg>,
    #[arg(long)]
    pub base_theme: Option<PathBuf>,
}

#[derive(Args)]
pub struct ExtractArgs {
    /// Rendered image or page
    pub image: String,
    /// Destination folder (default: the image's folder)
    #[arg(short, long)]
    pub output: Option<PathBuf>,
    /// Overwrite existing files
    #[arg(long)]
    pub force: bool,
    /// Print the source only, on the standard output
    #[arg(long)]
    pub stdout: bool,
}

#[derive(Args)]
pub struct FitArgs {
    /// Source file
    pub file: String,
    /// Target box: a ratio width:height (16:9, 4:3, 1.5) or a size in px (1280x720); default:
    /// diagram(fit=…)
    #[arg(long)]
    pub target: Option<String>,
    /// Write the best suggestion into the file
    #[arg(long)]
    pub write: bool,
    /// Custom CSS, applied after the theme (repeatable)
    #[arg(long)]
    pub css: Vec<PathBuf>,
    /// Theme of the rendering
    #[arg(long, value_enum)]
    pub theme: Option<ThemeArg>,
    /// Base theme replacing the default theme
    #[arg(long)]
    pub base_theme: Option<PathBuf>,
    /// Multiply every text size of the theme, as for render
    #[arg(long, value_name = "FACTOR")]
    pub font_scale: Option<f32>,
}

impl Format {
    /// Extension of the files of this format.
    pub fn extension(self) -> &'static str {
        match self {
            Format::Html => "html",
            Format::Svg => "svg",
            Format::Png => "png",
            Format::Gif => "gif",
            Format::Apng => "apng",
            Format::Webp => "webp",
        }
    }
}

/// Subcommands, options specific to `Cli` and their aliases: anything else in first position is a
/// source file, hence the `render` command.
const KNOWN: &[&str] = &[
    "render",
    "check",
    "fmt",
    "init",
    "watch",
    "layout",
    "fit",
    "extract",
    "theme",
    "icons",
    "licences",
    "help",
    "-h",
    "--help",
    "-V",
    "--version",
];

/// `skemo demo.skemo [options]` = `skemo render demo.skemo [options]`.
/// The global options (`--quiet`, `--color`, `--lang`) may come before the file.
pub fn with_default_command(args: Vec<String>) -> Vec<String> {
    let mut out = args;
    let mut i = 1;
    while i < out.len() {
        match out[i].as_str() {
            "-q" | "--quiet" => i += 1,
            "--color" | "--lang" => i += 2,
            a if a.starts_with("--color=") || a.starts_with("--lang=") => i += 1,
            a if KNOWN.contains(&a) => return out,
            _ => {
                out.insert(i, "render".into());
                return out;
            }
        }
    }
    out
}

impl RenderArgs {
    /// Effective output: the one asked for, otherwise the source with the format's extension (PNG by
    /// default), in the same folder. Standard input requires `-o`.
    pub fn resolve_output(&mut self) -> Result<(), crate::common::Fail> {
        if !self.output.is_empty() {
            return Ok(());
        }
        if self.file == "-" {
            return Err(crate::common::Fail::usage(tr!(
                "standard input: give the output with -o (or -o - with --format)",
                "entrée standard : indiquer la sortie avec -o (ou -o - avec --format)"
            )));
        }
        let ext = self.format.unwrap_or(Format::Png).extension();
        self.output = PathBuf::from(&self.file)
            .with_extension(ext)
            .to_string_lossy()
            .into_owned();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn argv(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| (*s).to_string()).collect()
    }

    #[test]
    fn a_file_means_render_and_known_commands_are_kept() {
        let with = |a: &[&str]| with_default_command(argv(a));
        assert_eq!(
            with(&["skemo", "d.skemo"]),
            argv(&["skemo", "render", "d.skemo"])
        );
        assert_eq!(
            with(&["skemo", "check", "d.skemo"]),
            argv(&["skemo", "check", "d.skemo"])
        );
        assert_eq!(with(&["skemo", "--version"]), argv(&["skemo", "--version"]));
        assert_eq!(with(&["skemo", "-h"]), argv(&["skemo", "-h"]));
        assert_eq!(with(&["skemo", "-"]), argv(&["skemo", "render", "-"]));
        // global options may precede the file
        assert_eq!(
            with(&[
                "skemo", "--quiet", "--color", "never", "d.skemo", "-o", "x.png"
            ]),
            argv(&[
                "skemo", "--quiet", "--color", "never", "render", "d.skemo", "-o", "x.png"
            ])
        );
        assert_eq!(
            with(&["skemo", "--color=always", "d.skemo"]),
            argv(&["skemo", "--color=always", "render", "d.skemo"])
        );
        assert_eq!(with(&["skemo"]), argv(&["skemo"]));
        assert_eq!(with(&["skemo", "-q"]), argv(&["skemo", "-q"]));
    }

    fn render_args(file: &str, output: &str, format: Option<Format>) -> RenderArgs {
        let mut cli = argv(&["skemo", "render", file]);
        if !output.is_empty() {
            cli.extend(argv(&["-o", output]));
        }
        let Cli {
            cmd: Cmd::Render(mut a),
            ..
        } = <Cli as clap::Parser>::parse_from(cli)
        else {
            unreachable!()
        };
        a.format = format;
        a
    }

    #[test]
    fn default_output_follows_the_source_and_the_format() {
        let mut a = render_args("dir/demo.skemo", "", None);
        a.resolve_output().unwrap();
        assert_eq!(a.output, "dir/demo.png");
        let mut a = render_args("dir/demo.skemo", "", Some(Format::Webp));
        a.resolve_output().unwrap();
        assert_eq!(a.output, "dir/demo.webp");
        let mut a = render_args("demo", "", Some(Format::Apng));
        a.resolve_output().unwrap();
        assert_eq!(a.output, "demo.apng");
        let mut a = render_args("d.skemo", "kept.svg", None);
        a.resolve_output().unwrap();
        assert_eq!(a.output, "kept.svg");
        let mut a = render_args("-", "", None);
        assert_eq!(a.resolve_output().unwrap_err().0, 2);
    }

    #[test]
    fn every_format_has_an_extension() {
        for (f, e) in [
            (Format::Html, "html"),
            (Format::Svg, "svg"),
            (Format::Png, "png"),
            (Format::Gif, "gif"),
            (Format::Apng, "apng"),
            (Format::Webp, "webp"),
        ] {
            assert_eq!(f.extension(), e);
        }
        assert_eq!(Theme::from(ThemeArg::Dark), Theme::Dark);
        assert_eq!(Theme::from(ThemeArg::Auto), Theme::Auto);
        assert_eq!(Theme::from(ThemeArg::Print), Theme::Print);
        assert_eq!(Theme::from(ThemeArg::Light), Theme::Light);
    }
}
