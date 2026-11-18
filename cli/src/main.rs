//! The skemo binary: a thin layer over `skemo-engine` and `skemo-lang`.
//!
//! Exit codes (SPEC §2): 0 = OK, 1 = the diagram is refused, 2 = misuse, 3 = internal error.

#![forbid(unsafe_code)]

mod args;
mod commands;
mod common;
mod help;
mod i18n;
mod limits;
mod out;
mod root;

use args::{Cli, Cmd, Diagnostics};
use clap::{CommandFactory, FromArgMatches};
use common::Fail;
use out::{Out, Place};
use skemo_lang::tr;
use std::io::IsTerminal;
use std::process::ExitCode;

/// Colours in the messages: on a terminal, unless `NO_COLOR` is set.
fn use_color() -> bool {
    std::io::stderr().is_terminal() && std::env::var_os("NO_COLOR").is_none()
}

fn run(cli: Cli, out: &Out) -> Result<(), Fail> {
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
        Cmd::Render(a) if a.tool.batch => commands::batch::run(out, &a),
        Cmd::Render(a) if a.live.watch => commands::watch::run(out, &a),
        Cmd::Render(mut a) => {
            a.resolve_output()?;
            commands::render::run(out, &a)
        }
        Cmd::Lint(a) => commands::lint::run(out, &a),
        Cmd::Layout(a) => commands::layout::run(out, &a),
        Cmd::Extract(a) => commands::extract::run(out, &a),
        Cmd::Site(a) => commands::site::run(out, &a),
        Cmd::Init(a) => commands::init::run(&a),
        Cmd::Capabilities { format } => commands::capabilities::run(format),
        Cmd::Completions { shell } => {
            let mut cmd = help::localize(Cli::command(), skemo_lang::i18n::language());
            clap_complete::generate(shell, &mut cmd, "skemo", &mut std::io::stdout());
            Ok(())
        }
        Cmd::Icons { query, json } => commands::icons::run(query.as_deref(), json),
    }
}

/// `--diagnostics json` among the raw arguments: clap's own errors must then be JSON too, before
/// the arguments are parsed.
fn wants_json(raw: &[String]) -> bool {
    raw.iter()
        .zip(raw.iter().skip(1))
        .any(|(a, b)| a == "--diagnostics" && b == "json")
        || raw.iter().any(|a| a == "--diagnostics=json")
}

/// A crash still ends with a JSON line (`internal`) when the messages are JSON.
fn report_panics_as_json() {
    std::panic::set_hook(Box::new(move |info| {
        let msg = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| (*s).to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_default();
        let at = info
            .location()
            .map(|l| format!(" ({}:{})", l.file(), l.line()))
            .unwrap_or_default();
        eprintln!(
            "{}",
            out::json_line(
                true,
                "internal",
                &format!("internal error, to report: {msg}{at}"),
                None,
                Place::file("-"),
            )
        );
    }));
}

/// The stable code of a failure without one of its own, from its exit code.
fn code_of(exit: u8) -> &'static str {
    match exit {
        1 => "refused",
        2 => "usage",
        _ => "internal",
    }
}

fn main() -> ExitCode {
    let raw: Vec<String> = std::env::args().collect();
    // the language comes first: even the help and the usage errors follow it
    let lang = i18n::init();
    let json = wants_json(&raw);
    if json {
        report_panics_as_json();
    }
    let early = Out {
        color: use_color() && !json,
        quiet: false,
        json,
    };
    if let Some(msg) = args::former_command(&raw) {
        early.error_at("usage", &msg, Place::file("-"));
        return ExitCode::from(2);
    }
    let parsed = help::localize(Cli::command(), lang)
        .try_get_matches_from(args::with_default_command(raw))
        .and_then(|m| Cli::from_arg_matches(&m));
    let cli = match parsed {
        Ok(c) => c,
        Err(e) if json && e.use_stderr() => {
            let text = e.kind().as_str().map_or_else(
                || e.to_string().trim().to_string(),
                |k| {
                    let full = e.to_string();
                    let first = full.lines().next().unwrap_or(k);
                    first.trim_start_matches("error: ").to_string()
                },
            );
            early.error_at("usage", &text, Place::file("-"));
            return ExitCode::from(2);
        }
        Err(e) => e.exit(),
    };
    out::set_allowed(cli.allow.clone());
    let out = Out {
        color: use_color() && cli.diagnostics == Diagnostics::Text,
        quiet: cli.quiet,
        json: cli.diagnostics == Diagnostics::Json,
    };
    match run(cli, &out) {
        Ok(()) => ExitCode::SUCCESS,
        Err(Fail(code, msg)) => {
            if let Some(m) = msg {
                out.error_at(code_of(code), &m, Place::file("-"));
            }
            ExitCode::from(code)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn argv(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| (*s).to_string()).collect()
    }

    #[test]
    fn json_is_seen_before_parsing() {
        assert!(wants_json(&argv(&["skemo", "-", "--diagnostics", "json"])));
        assert!(wants_json(&argv(&["skemo", "--diagnostics=json", "-"])));
        assert!(!wants_json(&argv(&["skemo", "--diagnostics", "text"])));
        assert_eq!(code_of(1), "refused");
        assert_eq!(code_of(2), "usage");
        assert_eq!(code_of(3), "internal");
    }
}
