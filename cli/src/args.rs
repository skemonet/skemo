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
    about = "Text diagrams (skemo): HTML, SVG, PNG, GIF, APNG and WebP rendering",
    override_usage = "skemo <FILE> [OPTIONS]   (= skemo render)\n       skemo <COMMAND> [OPTIONS]\n       skemo --licences",
    disable_help_subcommand = true,
    disable_help_flag = true,
    disable_version_flag = true
)]
pub struct Cli {
    /// Print the help
    #[arg(short, long, action = ArgAction::Help, global = true, help_heading = GENERIC)]
    pub help: Option<bool>,
    /// Print the version
    #[arg(short = 'V', long, action = ArgAction::Version, help_heading = GENERIC)]
    pub version: Option<bool>,
    /// Only show errors
    #[arg(long, short, global = true, help_heading = GENERIC)]
    pub quiet: bool,
    /// Print the licences of what the binary embeds (fonts, icons, crates)
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
    /// Create a sample diagram (overwrites nothing)
    Init(InitArgs),
    /// Check files (diagnostics), optionally rewrite them in canonical form
    Lint(LintArgs),
    /// Render a diagram, format from the output's extension; the default command
    Render(RenderArgs),
    /// Get back the source and stylesheets embedded in a rendered image
    Extract(ExtractArgs),
    /// Full layout as JSON, on the standard output
    Layout(LayoutArgs),
    /// The built-in icons, filtered by a query (for editors and completion; not in the help)
    #[command(hide = true)]
    Icons {
        /// Text to look for in the names and tags
        query: Option<String>,
        /// A JSON array of `{name, tags}`
        #[arg(long)]
        json: bool,
    },
    /// Shell completion script: `eval "$(skemo completions bash)"` in ~/.bashrc (not in the help)
    #[command(hide = true)]
    Completions {
        /// bash, zsh, fish, elvish or powershell
        shell: clap_complete::Shell,
    },
}

#[derive(Args)]
pub struct InitArgs {
    /// Diagram to create
    #[arg(default_value = "diagram.skemo")]
    pub file: PathBuf,
    /// Also a short commented stylesheet the diagram includes (`skemo_style.css`)
    #[arg(long, value_name = "FILE", num_args = 0..=1, default_missing_value = "skemo_style.css")]
    pub css: Option<String>,
    /// Same, with the whole default theme to edit
    #[arg(long, value_name = "FILE", num_args = 0..=1, default_missing_value = "skemo_style.css", conflicts_with = "css")]
    pub css_full: Option<String>,
}

/// Headings of the render options (`help::localize` translates them).
pub const OUTPUT: &str = "Output";
pub const STYLE: &str = "Style";
pub const ANIMATION: &str = "Animation";

#[derive(Args)]
pub struct RenderArgs {
    /// Source file (`-`: standard input)
    pub file: String,
    /// Output file, its extension picks the format (default: the source's name, .png)
    #[arg(short, long, value_name = "FILE", default_value = "", hide_default_value = true, help_heading = OUTPUT)]
    pub output: String,
    /// Format, when the extension does not say
    #[arg(long, value_enum, help_heading = OUTPUT)]
    pub format: Option<Format>,
    /// Pixel scale of PNG, GIF and APNG
    #[arg(long, help_heading = OUTPUT)]
    pub scale: Option<f32>,
    /// No background (SVG, PNG, APNG)
    #[arg(long, help_heading = OUTPUT)]
    pub transparent: bool,
    /// Do not embed the source in the output (see skemo extract)
    #[arg(long, help_heading = OUTPUT)]
    pub no_source: bool,
    #[command(flatten)]
    pub fonts: FontArgs,
    #[command(flatten)]
    pub live: LiveArgs,
    /// Exit 1 on a warning
    #[arg(long, help_heading = OUTPUT)]
    pub deny_warnings: bool,
    /// Extra stylesheet, after the diagram's own (repeatable)
    #[arg(long, value_name = "FILE", help_heading = STYLE)]
    pub css: Vec<PathBuf>,
    /// Theme replacing the default one
    #[arg(long, value_name = "FILE", help_heading = STYLE)]
    pub base_theme: Option<PathBuf>,
    /// Theme variant, over diagram(theme=…)
    #[arg(long, value_enum, help_heading = STYLE)]
    pub theme: Option<ThemeArg>,
    /// high: darken the texts below the WCAG contrast, layout unchanged
    #[arg(long, value_enum, default_value_t = ContrastArg::Standard, help_heading = STYLE)]
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
    /// Live preview in the browser; writes a file only with -o
    #[arg(long, help_heading = OUTPUT)]
    pub watch: bool,
    /// Port of the live preview
    #[arg(long, default_value_t = 8710, requires = "watch", help_heading = OUTPUT)]
    pub port: u16,
}

/// Canonical form (`--fix`, `--dry`).
#[derive(Args)]
pub struct FixArgs {
    /// Rewrite the files in canonical form; with --fit, apply the best setting too
    #[arg(long)]
    pub fix: bool,
    /// With --fix: write nothing, exit 1 if a file would change
    #[arg(long, requires = "fix")]
    pub dry: bool,
}

/// SVG options.
#[derive(Args)]
pub struct SvgArgs {
    /// SVG: play the moving arrows (.flow, .pulse); still otherwise
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
    /// The state at this step (as written: 2.5)
    #[arg(long, help_heading = ANIMATION)]
    pub step: Option<String>,
    /// PNG: one file per state, and name.states.json
    #[arg(long, help_heading = ANIMATION)]
    pub animated: bool,
    /// PNG: every step on one sheet
    #[arg(long, help_heading = ANIMATION)]
    pub board: bool,
}

/// Options of the animated outputs (GIF and APNG).
#[derive(Args)]
pub struct AnimArgs {
    /// GIF, APNG: seconds per state
    #[arg(long, default_value_t = 1.6, help_heading = ANIMATION)]
    pub hold: f64,
    /// GIF, APNG: seconds per transition
    #[arg(long, default_value_t = 0.45, help_heading = ANIMATION)]
    pub transition: f64,
    /// GIF, APNG: frames per second
    #[arg(long, default_value_t = 20, help_heading = ANIMATION)]
    pub fps: u32,
    /// GIF, APNG: play once
    #[arg(long, help_heading = ANIMATION)]
    pub no_loop: bool,
    /// GIF, APNG: seconds on the last frame
    #[arg(long, default_value_t = 3.0, help_heading = ANIMATION)]
    pub final_hold: f64,
}

#[derive(Args)]
pub struct LintArgs {
    /// Files to check
    #[arg(required = true)]
    pub files: Vec<String>,
    #[command(flatten)]
    pub canon: FixArgs,
    /// Exit 1 on a warning
    #[arg(long)]
    pub deny_warnings: bool,
    /// Also check the text contrast in the light, dark and print themes
    #[arg(long)]
    pub contrast: bool,
    /// Settings (dir, wrap) showing the text larger in a box: 16:9, 1280x720 (default: diagram(fit=…))
    #[arg(long, value_name = "BOX", num_args = 0..=1, default_missing_value = "")]
    pub fit: Option<String>,
    /// Text size factor for --fit, as for render
    #[arg(long, value_name = "FACTOR", requires = "fit")]
    pub font_scale: Option<f32>,
    /// Extra stylesheet, after the diagram's own (repeatable)
    #[arg(long, value_name = "FILE")]
    pub css: Vec<PathBuf>,
    /// Theme replacing the default one
    #[arg(long, value_name = "FILE")]
    pub base_theme: Option<PathBuf>,
}

#[derive(Args)]
pub struct LayoutArgs {
    /// Source file
    pub file: String,
    /// JSON output (the only format)
    #[arg(long)]
    pub json: bool,
    /// Extra stylesheet, after the diagram's own (repeatable)
    #[arg(long, value_name = "FILE")]
    pub css: Vec<PathBuf>,
    /// Theme variant, over diagram(theme=…)
    #[arg(long, value_enum)]
    pub theme: Option<ThemeArg>,
    /// Theme replacing the default one
    #[arg(long, value_name = "FILE")]
    pub base_theme: Option<PathBuf>,
}

#[derive(Args)]
pub struct ExtractArgs {
    /// Rendered image or page
    pub image: String,
    /// Destination folder (default: the image's)
    #[arg(short, long)]
    pub output: Option<PathBuf>,
    /// Overwrite existing files
    #[arg(long)]
    pub force: bool,
    /// Only the source, on the standard output
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
