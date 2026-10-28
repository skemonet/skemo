# Contributing to skemo

Thank you for your interest. Issues are closed on this repository: propose fixes, examples that render badly
and documentation as pull requests. Please read the [code of conduct](CODE_OF_CONDUCT.md) first; security issues go through
[`SECURITY.md`](SECURITY.md), not public issues.

## Getting set up

```bash
rustup target add wasm32-unknown-unknown
cargo install cargo-deny cargo-llvm-cov --locked
cargo install wasm-bindgen-cli --version 0.2.129 --locked   # same version as engine-wasm
# Node.js 20 or later for the WebAssembly parity check; actionlint is optional
```

```bash
cargo build --release -p skemo-cli       # target/release/skemo
cargo test --workspace --release         # every crate's tests
tools/ci.sh                              # the full pipeline, as the CI runs it (quality + security in parallel)
tools/ci.sh quality                      # format, clippy, tests, docs, coverage, WebAssembly, smoke test
tools/ci.sh security                     # cargo-deny, unsafe, secrets, workflows
```

Run `tools/ci.sh` before opening a pull request: the CI runs exactly this script.

## How the code is organised

See the [root README](README.md#repository-layout). Each crate folder has a `README.md` (usage) and a `SPEC.md`
(requirements, each with an identifier). The dependency direction is strict: `router` and `lang` depend on no other
project crate, `engine` uses both, `cli` only calls `engine` and `lang`. A feature the CLI needs is written in
`engine` or `lang`, never in `cli`.

## Rules the code follows

- **The format is a contract.** `lang/FORMAT.md` describes the language. A change to the syntax starts with a
  change to `FORMAT.md`, discussed in a pull request, before the code. User-visible features also update the user
  documentation (`lang/site/pages/`) and get an example in `lang/site/examples/`, which the tests render.
- **Reference data is read-only.** The `fixtures/` folders hold reference outputs of an earlier JavaScript
  prototype of skemo (the "oracle"): sizes, positions and models the Rust code is checked against. They are never
  edited by hand.
- **Rendering changes are deliberate.** `engine/crates/engine/tests/golden.rs` fingerprints every output (SVG in
  three themes, HTML, PNG) of every example. A refactoring must not change it; after an intended change, run
  `UPDATE_GOLDEN=1 cargo test --release -p skemo-engine --test golden` and say in the pull request which outputs
  moved and why.
- **Diagnostics are snapshots.** `lang` checks its messages with `insta`; review a changed snapshot with
  `cargo insta review` (or by reading the `.snap.new` diff) before accepting it.
- **Messages in English and French.** Every message a person reads is written where it is produced, with
  `tr!("english", "français")` (`lang/src/i18n.rs`); the CLI's help texts have their French twin in
  `cli/src/help.rs`. If you do not write French, put the English text in both halves and say so in the pull
  request: a maintainer will translate it. Diagnostic codes (`unknown-target`…) never change.
- **No `unsafe`, no panics on input.** `unsafe_code = "forbid"` everywhere; the parser and the router return
  diagnostics on bad input, they do not panic (they are tested on random input).
- **Small functions.** Clippy runs with `-D warnings` and the workspace lints in `Cargo.toml` (functions under 100
  lines, bounded complexity, no `dbg!` or `todo!`). Split a function rather than adding `#[allow]`.
- **Coverage does not go down.** `tools/ci.sh quality` fails below the minimum line coverage set in
  `tools/coverage.min`.
- **Dependencies are vetted.** `deny.toml` lists the allowed licences and sources; a new dependency must pass
  `cargo deny check` and should be justified in the pull request. The engine and the CLI must stay buildable as
  WebAssembly and as a static musl binary: no C library.
- **Router provenance.** `router` is written from the published papers and from `hightower` (MIT/Apache). Never
  read, copy or translate the code of libavoid or its ports (LGPL). Record any new source of ideas in
  `router/PROVENANCE.md`.
- **Documentation is in English**, comments included.

## Identifiers you will meet in the code

| Form | Meaning |
|---|---|
| `F-…-N`, `E-…-N`, `R-N`, `C-NF-N` | requirements of a crate, defined in its `SPEC.md` (functional, engine, router, CLI non-functional) |
| `R0`, `E0`…`E8`, `L1`…`L3`, `C1`, `C2` | milestones of a crate (`SPEC.md`, "milestones") |
| `D-NNN` | a design decision; [`docs/decisions.md`](docs/decisions.md) summarises the ones the code refers to |
| `§N` | a section of `FORMAT.md` or of the `SPEC.md` at hand |
| `pitfall PN`, `limit LN`, `QN`, `ED-N` | notes from the prototype's study and from the design questions; the comment says what matters |

## Pull requests

- One topic per pull request, with a test that fails without the change.
- Describe what changes for a user; for a rendering change, attach a before/after image (`skemo render`).
- Keep commits readable; we squash when merging if needed.
- Add a line to [`CHANGELOG.md`](CHANGELOG.md) under *Unreleased* for anything a user would notice.

By contributing, you agree that your contribution is dual-licensed under MIT OR Apache-2.0, like the rest of the
project.
