//! Command-line arguments (clap). Help texts are written in English here; the French ones are
//! in `help.rs` and applied at run time (the language follows the system, see `i18n`).

use clap::{ArgAction, Args, Parser, Subcommand, ValueEnum};
use skemo_lang::style::Theme;
use skemo_lang::tr;
use std::path::PathBuf;

/// Heading of the options every command takes (`help::localize` translates it).
pub const GENERIC: &str = "Generic options";

#[derive(Parser)]
#[command(
    name = "skemo",
    version,
    about = "Diagrams written as text, drawn as SVG, PNG, HTML, GIF, APNG or WebP",
    override_usage = "skemo <FILE> [OPTIONS]   (= skemo render)\n       skemo <COMMAND> [OPTIONS]\n       skemo --licences",
    disable_help_subcommand = true,
    disable_help_flag = true,
    disable_version_flag = true
)]
pub struct Cli {
    /// Show this help
    #[arg(short, long, action = ArgAction::Help, global = true, help_heading = GENERIC)]
    pub help: Option<bool>,
    /// Show the version
    #[arg(short = 'V', long, action = ArgAction::Version, help_heading = GENERIC)]
    pub version: Option<bool>,
    /// Show errors only
    #[arg(long, short, global = true, help_heading = GENERIC)]
    pub quiet: bool,
    /// Show the licences of the fonts, icons and libraries included
    #[arg(long, help_heading = GENERIC)]
    pub licences: bool,
    #[command(subcommand)]
    pub cmd: Option<Cmd>,
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
    Apng,
    Webp,
}

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ContrastArg {
    Standard,
    High,
}

/// The commands, in the order of a diagram's life: create, check, render, reopen, inspect.
#[derive(Subcommand)]
pub enum Cmd {
    /// Create a sample diagram to start from
    Init(InitArgs),
    /// Check diagrams, and tidy them with --fix
    Lint(LintArgs),
    /// Draw a diagram (the default command: skemo demo.skemo)
    Render(RenderArgs),
    /// Get the source back from a rendered image
    Extract(ExtractArgs),
    /// Print the positions and routes as JSON
    Layout(LayoutArgs),
    /// List the built-in icons, filtered by a query (for editors and completion; not in the help)
    #[command(hide = true)]
    Icons {
        /// Words to look for in the names and tags
        query: Option<String>,
        /// Print a JSON array of `{name, tags}`
        #[arg(long)]
        json: bool,
    },
    /// Print a shell completion script: `eval "$(skemo completions bash)"` in ~/.bashrc (not in the help)
    #[command(hide = true)]
    Completions {
        /// bash, zsh, fish, elvish or powershell
        shell: clap_complete::Shell,
    },
}

#[derive(Args)]
pub struct InitArgs {
    /// Diagram to create (default: diagram.skemo)
    #[arg(default_value = "diagram.skemo")]
    pub file: PathBuf,
    /// Also write a short stylesheet the diagram uses (default name: `skemo_style.css`)
    #[arg(long, value_name = "FILE", num_args = 0..=1, default_missing_value = "skemo_style.css")]
    pub css: Option<String>,
    /// Same, with the whole default theme, to edit
    #[arg(long, value_name = "FILE", num_args = 0..=1, default_missing_value = "skemo_style.css", conflicts_with = "css")]
    pub css_full: Option<String>,
}

/// Headings of the render options (`help::localize` translates them).
pub const OUTPUT: &str = "Output";
pub const STYLE: &str = "Style";
pub const ANIMATION: &str = "Animation";

#[derive(Args)]
pub struct RenderArgs {
    /// Diagram (- reads the standard input)
    pub file: String,
    /// Output file; its extension sets the format (default: demo.png beside demo.skemo)
    #[arg(short, long, value_name = "FILE", default_value = "", hide_default_value = true, help_heading = OUTPUT)]
    pub output: String,
    /// Format when the extension does not say: html, svg, png, gif, apng, webp
    #[arg(long, value_enum, value_name = "FORMAT", help_heading = OUTPUT)]
    pub format: Option<Format>,
    /// Pixel density of PNG, GIF and APNG (default: 2)
    #[arg(long, value_name = "N", help_heading = OUTPUT)]
    pub scale: Option<f32>,
    /// No background (SVG, PNG, APNG)
    #[arg(long, help_heading = OUTPUT)]
    pub transparent: bool,
    /// Do not embed the source (skemo extract then cannot read it back)
    #[arg(long, help_heading = OUTPUT)]
    pub no_source: bool,
    #[command(flatten)]
    pub fonts: FontArgs,
    #[command(flatten)]
    pub live: LiveArgs,
    /// Exit with 1 on any warning
    #[arg(long, help_heading = OUTPUT)]
    pub deny_warnings: bool,
    /// Extra stylesheet, applied after the diagram's own (repeatable)
    #[arg(long, value_name = "FILE", help_heading = STYLE)]
    pub css: Vec<PathBuf>,
    /// Stylesheet replacing the default theme
    #[arg(long, value_name = "FILE", help_heading = STYLE)]
    pub base_theme: Option<PathBuf>,
    /// light, dark, print or auto; overrides diagram(theme=…)
    #[arg(long, value_enum, value_name = "THEME", help_heading = STYLE)]
    pub theme: Option<ThemeArg>,
    /// high: darken the texts below the WCAG contrast, layout unchanged (default: standard)
    #[arg(long, value_enum, value_name = "LEVEL", default_value_t = ContrastArg::Standard, help_heading = STYLE)]
    pub contrast: ContrastArg,
    /// Text size factor (1.4 for a slide)
    #[arg(long, value_name = "FACTOR", help_heading = STYLE)]
    pub font_scale: Option<f32>,
    #[command(flatten)]
    pub png: PngArgs,
    #[command(flatten)]
    pub svg: SvgArgs,
    #[command(flatten)]
    pub anim: AnimArgs,
}

/// Live preview (`--watch`).
#[derive(Args)]
pub struct LiveArgs {
    /// Live preview in the browser, redrawn on every save; writes a file only with -o
    #[arg(long, help_heading = OUTPUT)]
    pub watch: bool,
    /// Port of the live preview (default: 8710)
    #[arg(long, default_value_t = 8710, requires = "watch", help_heading = OUTPUT)]
    pub port: u16,
}

/// Canonical form (`--fix`, `--dry`).
#[derive(Args)]
pub struct FixArgs {
    /// Rewrite them in canonical form (with --fit: apply the best setting too)
    #[arg(long)]
    pub fix: bool,
    /// With --fix: change nothing, exit with 1 if a file would change
    #[arg(long, requires = "fix")]
    pub dry: bool,
}

/// SVG options.
#[derive(Args)]
pub struct SvgArgs {
    /// SVG: animate the moving arrows (.flow, .pulse)
    #[arg(long, help_heading = ANIMATION)]
    pub motion: bool,
}

/// Fonts in the HTML and SVG outputs.
#[derive(Args)]
pub struct FontArgs {
    /// HTML: do not embed the fonts
    #[arg(long, help_heading = OUTPUT)]
    pub no_fonts: bool,
    /// SVG: embed the fonts (about 0.8 MB per weight)
    #[arg(long, help_heading = OUTPUT)]
    pub embed_fonts: bool,
    /// Font file for a family the CSS names (repeatable)
    #[arg(long, value_name = "FILE", help_heading = STYLE)]
    pub font: Vec<PathBuf>,
}

/// PNG options (one given state, one file per state, board).
#[derive(Args)]
pub struct PngArgs {
    /// Draw the state at this step, as written (2.5)
    #[arg(long, value_name = "STEP", help_heading = ANIMATION)]
    pub step: Option<String>,
    /// PNG: one file per state, plus name.states.json
    #[arg(long, help_heading = ANIMATION)]
    pub animated: bool,
    /// PNG: every step on one sheet
    #[arg(long, help_heading = ANIMATION)]
    pub board: bool,
}

/// Options of the animated outputs (GIF and APNG).
#[derive(Args)]
pub struct AnimArgs {
    /// GIF, APNG: seconds on each state (default: 1.6)
    #[arg(long, value_name = "SECONDS", default_value_t = 1.6, help_heading = ANIMATION)]
    pub hold: f64,
    /// GIF, APNG: seconds per transition (default: 0.45)
    #[arg(long, value_name = "SECONDS", default_value_t = 0.45, help_heading = ANIMATION)]
    pub transition: f64,
    /// GIF, APNG: frames per second (default: 20)
    #[arg(long, value_name = "N", default_value_t = 20, help_heading = ANIMATION)]
    pub fps: u32,
    /// GIF, APNG: play once
    #[arg(long, help_heading = ANIMATION)]
    pub no_loop: bool,
    /// GIF, APNG: seconds on the last frame (default: 3)
    #[arg(long, value_name = "SECONDS", default_value_t = 3.0, help_heading = ANIMATION)]
    pub final_hold: f64,
}

#[derive(Args)]
pub struct LintArgs {
    /// Diagrams to check
    #[arg(required = true)]
    pub files: Vec<String>,
    #[command(flatten)]
    pub canon: FixArgs,
    /// Exit with 1 on any warning
    #[arg(long)]
    pub deny_warnings: bool,
    /// Also check the contrast of the texts (light, dark and print themes)
    #[arg(long)]
    pub contrast: bool,
    /// Suggest dir or wrap to show the text larger in a box: 16:9, 1280x720 (default: diagram(fit=…))
    #[arg(long, value_name = "BOX", num_args = 0..=1, default_missing_value = "")]
    pub fit: Option<String>,
    /// Text size factor for --fit
    #[arg(long, value_name = "FACTOR", requires = "fit")]
    pub font_scale: Option<f32>,
    /// Extra stylesheet, applied after the diagram's own (repeatable)
    #[arg(long, value_name = "FILE")]
    pub css: Vec<PathBuf>,
    /// Stylesheet replacing the default theme
    #[arg(long, value_name = "FILE")]
    pub base_theme: Option<PathBuf>,
}

#[derive(Args)]
pub struct LayoutArgs {
    /// Diagram
    pub file: String,
    /// JSON output (the only format)
    #[arg(long)]
    pub json: bool,
    /// Extra stylesheet, applied after the diagram's own (repeatable)
    #[arg(long, value_name = "FILE")]
    pub css: Vec<PathBuf>,
    /// light, dark, print or auto; overrides diagram(theme=…)
    #[arg(long, value_enum, value_name = "THEME")]
    pub theme: Option<ThemeArg>,
    /// Stylesheet replacing the default theme
    #[arg(long, value_name = "FILE")]
    pub base_theme: Option<PathBuf>,
}

#[derive(Args)]
pub struct ExtractArgs {
    /// Rendered image or page (PNG, SVG, WebP, GIF, APNG, HTML)
    pub image: String,
    /// Folder to write to (default: the image's)
    #[arg(short, long, value_name = "DIR")]
    pub output: Option<PathBuf>,
    /// Overwrite existing files
    #[arg(long)]
    pub force: bool,
    /// Print only the source, on the standard output
    #[arg(long)]
    pub stdout: bool,
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
    "init",
    "lint",
    "render",
    "extract",
    "layout",
    "icons",
    "completions",
    // former commands (D-083, D-084): left to clap, which says they do not exist, rather than
    // read as a file
    "check",
    "fmt",
    "watch",
    "fit",
    "theme",
    "licences",
    "help",
    "-h",
    "--help",
    "-V",
    "--version",
    "--licences",
];

/// A command removed in 0.1.0-rc.8 (D-083, D-084), in first position: what replaces it. Clap would
/// only say, in English, that it does not exist.
pub fn former_command(args: &[String]) -> Option<String> {
    let first = args
        .iter()
        .skip(1)
        .find(|a| !matches!(a.as_str(), "-q" | "--quiet"))?;
    let now = match first.as_str() {
        "check" => "skemo lint",
        "fmt" => "skemo lint --fix",
        "watch" => "skemo <FILE> --watch",
        "fit" => "skemo lint --fit",
        "theme" => "skemo init --css-full",
        "licences" => "skemo --licences",
        _ => return None,
    };
    Some(tr!(
        "« skemo {first} » no longer exists: use « {now} » (skemo --help lists the commands)",
        "« skemo {first} » n'existe plus : utiliser « {now} » (skemo --help liste les commandes)"
    ))
}

/// `skemo demo.skemo [options]` = `skemo render demo.skemo [options]`.
/// The generic option `--quiet` may come before the file.
pub fn with_default_command(args: Vec<String>) -> Vec<String> {
    let mut out = args;
    let mut i = 1;
    while i < out.len() {
        match out[i].as_str() {
            "-q" | "--quiet" => i += 1,
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
            with(&["skemo", "lint", "d.skemo"]),
            argv(&["skemo", "lint", "d.skemo"])
        );
        assert_eq!(with(&["skemo", "--version"]), argv(&["skemo", "--version"]));
        assert_eq!(with(&["skemo", "-h"]), argv(&["skemo", "-h"]));
        assert_eq!(with(&["skemo", "-"]), argv(&["skemo", "render", "-"]));
        // the generic --quiet may precede the file
        assert_eq!(
            with(&["skemo", "--quiet", "d.skemo", "-o", "x.png"]),
            argv(&["skemo", "--quiet", "render", "d.skemo", "-o", "x.png"])
        );
        assert_eq!(
            with(&["skemo", "--licences"]),
            argv(&["skemo", "--licences"])
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
            cmd: Some(Cmd::Render(mut a)),
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
