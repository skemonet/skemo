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

use args::{Cli, Cmd};
use clap::{CommandFactory, FromArgMatches};
use common::Fail;
use out::Out;
use skemo_lang::tr;
use std::io::IsTerminal;
use std::process::ExitCode;

/// Colours in the messages: on a terminal, unless `NO_COLOR` is set.
fn use_color() -> bool {
    std::io::stderr().is_terminal() && std::env::var_os("NO_COLOR").is_none()
}

fn run(cli: Cli) -> Result<(), Fail> {
    let out = Out {
        color: use_color(),
        quiet: cli.quiet,
    };
    let Some(cmd) = cli.cmd else {
        if cli.licences {
            return commands::licences::run();
        }
        // `skemo` alone: the help, as a usage error
        let _ = help::localize(Cli::command(), skemo_lang::i18n::language()).print_help();
        return Err(Fail(2, None));
    };
    if cli.licences {
        return Err(Fail::usage(tr!(
            "--licences is used alone, without a command",
            "--licences s'emploie seul, sans commande"
        )));
    }
    match cmd {
        Cmd::Render(a) if a.live.watch => commands::watch::run(&out, &a),
        Cmd::Render(mut a) => {
            a.resolve_output()?;
            commands::render::run(&out, &a)
        }
        Cmd::Lint(a) => commands::lint::run(&out, &a),
        Cmd::Layout(a) => commands::layout::run(&out, &a),
        Cmd::Extract(a) => commands::extract::run(&out, &a),
        Cmd::Init(a) => commands::init::run(&a),
        Cmd::Completions { shell } => {
            let mut cmd = help::localize(Cli::command(), skemo_lang::i18n::language());
            clap_complete::generate(shell, &mut cmd, "skemo", &mut std::io::stdout());
            Ok(())
        }
        Cmd::Icons { query, json } => commands::icons::run(query.as_deref(), json),
    }
}

fn main() -> ExitCode {
    let raw: Vec<String> = std::env::args().collect();
    // the language comes first: even the help and the usage errors follow it
    let lang = i18n::init();
    if let Some(msg) = args::former_command(&raw) {
        Out {
            color: use_color(),
            quiet: false,
        }
        .error(&msg);
        return ExitCode::from(2);
    }
    let matches =
        help::localize(Cli::command(), lang).get_matches_from(args::with_default_command(raw));
    let cli = Cli::from_arg_matches(&matches).unwrap_or_else(|e| e.exit());
    let out = Out {
        color: use_color(),
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
