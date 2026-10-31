# Security policy

## Reporting a vulnerability

Do not open a public issue. Write to **claude@skemo.net** (or use "Report a vulnerability" in the repository's Security tab) with: version, steps to reproduce, impact. We answer within 7 days.

## What is checked on every commit and every tag (`tools/ci.sh security`)

- **Dependencies** (`cargo-deny`): RustSec security advisories, yanked crates, allowed licences, sources (crates.io only, plus the git repositories listed in `deny.toml`), version wildcards.
- **`unsafe`**: forbidden (`unsafe_code = "forbid"`), and no `unsafe` line in the code.
- **Secrets**: search for keys and tokens in the files and in the whole history.
- **GitHub workflows** (when there are any): minimal permissions, non-persisted tokens, no `pull_request_target`, actions pinned by commit hash, `actionlint` analysis.
- **Reproducibility**: `Cargo.lock` committed, `--locked` builds.

The same checks run locally: `tools/ci.sh security`.

## Supported versions

skemo is a pre-release: only the latest release receives fixes.
