//! The skemo binary: a thin layer over `skemo-engine` and `skemo-lang`.
//!
//! Exit codes (SPEC §2): 0 = OK, 1 = errors in the file, 2 = misuse, 3 = internal error.

#![forbid(unsafe_code)]

mod args;
mod commands;
mod common;
mod help;
mod i18n;
mod out;

use args::{Cli, Cmd, Color};
use clap::{CommandFactory, FromArgMatches};
use common::Fail;
use out::Out;
use std::io::IsTerminal;
use std::process::ExitCode;

/// Colours: asked for, or a terminal without `NO_COLOR` in automatic mode.
fn use_color(choice: Color) -> bool {
    match choice {
        Color::Always => true,
        Color::Never => false,
        Color::Auto => std::io::stderr().is_terminal() && std::env::var_os("NO_COLOR").is_none(),
    }
}

fn run(cli: Cli) -> Result<(), Fail> {
    let out = Out {
        color: use_color(cli.color),
        quiet: cli.quiet,
    };
    match cli.cmd {
        Cmd::Render(mut a) => {
            a.resolve_output()?;
            commands::render::run(&out, &a)
        }
        Cmd::Check(a) => commands::check::run(&out, &a),
        Cmd::Fmt(a) => commands::fmt::run(&out, &a),
        Cmd::Layout(a) => commands::layout::run(&out, &a),
        Cmd::Fit(a) => commands::fit::run(&out, &a),
        Cmd::Extract(a) => commands::extract::run(&out, &a),
        Cmd::Watch(a) => commands::watch::run(&out, &a),
        Cmd::Init { dir } => commands::init::run(&dir),
        Cmd::Icons { query, json } => commands::icons::run(query.as_deref(), json),
        Cmd::Theme { print } => commands::theme::run(print),
        Cmd::Licences => commands::licences::run(),
    }
}

fn main() -> ExitCode {
    let raw: Vec<String> = std::env::args().collect();
    // the language comes first: even the help and the usage errors follow it
    let lang = i18n::init(&raw);
    let matches =
        help::localize(Cli::command(), lang).get_matches_from(args::with_default_command(raw));
    let cli = Cli::from_arg_matches(&matches).unwrap_or_else(|e| e.exit());
    let out = Out {
        color: use_color(cli.color),
        quiet: false,
    };
    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(Fail(code, msg)) => {
            if let Some(m) = msg {
                out.error(&m);
            }
            ExitCode::from(code)
        }
    }
}
