# AGENTS.md — notes for coding agents

This file is for AI coding agents working on skemo. Humans: see [`CONTRIBUTING.md`](CONTRIBUTING.md), which says
the same things at more length. (To *use* skemo from an agent, see the skill in `cli/skill/skemo/`.)

## Commands

```bash
cargo build --release -p skemo-cli                 # target/release/skemo
cargo test --workspace --release                   # all tests
cargo test --release -p skemo-engine --test golden # rendering fingerprints
tools/ci.sh                                        # the CI pipeline; must pass before a pull request
```

## Ground rules

1. **Read before touching.** Each crate has a `SPEC.md`; `lang/FORMAT.md` is the language contract. Do not change
   the syntax without changing `FORMAT.md` first, and only when the issue asks for it.
2. **Dependency direction**: `router` and `lang` depend on no project crate; `engine` uses both; `cli` is thin
   and holds no layout, routing or rendering logic.
3. **Never edit `fixtures/`** (reference outputs of the prototype) or `tests/golden.txt` by hand. If a rendering
   change is intended, regenerate the golden file with `UPDATE_GOLDEN=1` and report which outputs changed.
4. **Router licence**: never open, copy or translate libavoid, libavoid-js or libavoid-rust (LGPL). Work from the
   papers; record sources in `router/PROVENANCE.md`.
5. **Messages** are bilingual: `tr!("english", "français")`; CLI help also in `cli/src/help.rs`. Snapshot tests
   cover both languages. Diagnostic codes are stable.
6. **No `unsafe`, no panic on user input, no new C dependency** (the engine builds for `wasm32-unknown-unknown`
   and musl).
7. **Lints are strict** (`-D warnings`, functions under 100 lines): split code instead of adding `#[allow]`.
8. **Match the surrounding code**: naming, comment density, English comments, short doc comments that say why.
9. Do not lower `tools/coverage.min`; add tests with the code.
