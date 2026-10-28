#!/usr/bin/env bash
# The skemo pipeline — the SAME one as the CI (the workflow only calls this script).
#
#   tools/ci.sh            everything: quality and security IN PARALLEL (logs in target/ci/)
#   tools/ci.sh quality    format, lints, tests, documentation, coverage, WebAssembly, smoke test
#   tools/ci.sh security   dependencies (advisories, licences, sources), unsafe, secrets, workflows
#   tools/ci.sh --list     lists the stages
set -euo pipefail
cd "$(dirname "$0")/.."
[ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
export CARGO_TERM_COLOR="${CARGO_TERM_COLOR:-auto}"
export RUSTDOCFLAGS="${RUSTDOCFLAGS:--D warnings}"

step() { printf '\n\033[1m▶ %s\033[0m\n' "$*"; }
need() { command -v "$1" > /dev/null || { echo "missing tool: $1 ($2)" >&2; exit 127; }; }

quality() {
  step "format";  cargo fmt --all -- --check
  step "lints (clippy, -D warnings)"; cargo clippy --workspace --release --all-targets --locked -- -D warnings
  step "tests";   cargo test --workspace --release --locked
  step "documentation (links and code blocks)"; cargo doc --workspace --release --no-deps --locked
  coverage
  step "WebAssembly: router and lang (wasm32-unknown-unknown)"
  cargo build --release --locked -p skemo-router -p skemo-lang --target wasm32-unknown-unknown
  step "WebAssembly: the same layout.json as native"; (cd engine && tools/wasm-parity.sh)
  smoke_test
}

# Line coverage (cargo-llvm-cov) of the whole workspace: must not fall below tools/coverage.min.
coverage() {
  if ! command -v cargo-llvm-cov > /dev/null; then
    echo "(cargo-llvm-cov missing: coverage skipped — cargo install cargo-llvm-cov --locked)"; return 0
  fi
  step "coverage (minimum $(cat tools/coverage.min) % of lines)"
  cargo llvm-cov --workspace --release --locked --summary-only \
    --fail-under-lines "$(cat tools/coverage.min)" | tail -n 3
}

# The release binary starts and renders a sample.
smoke_test() {
  step "smoke test: release binary"
  cargo build --release --locked -p skemo-cli
  local d; d=$(mktemp -d)
  target/release/skemo init "$d/x" > /dev/null 2>&1
  target/release/skemo render "$d/x/example.skemo" --css "$d/x/custom.css" -o "$d/x.svg" --deny-warnings
  target/release/skemo check "$d/x/example.skemo"
  rm -rf "$d"
}

# No `unsafe` line outside comments and string literals, and the prohibition must be declared.
check_no_unsafe() {
  step "unsafe forbidden"
  if grep -rnE '\bunsafe\b' --include='*.rs' . 2> /dev/null \
    | grep -vE '/target/' | grep -vE ':\s*//|forbid\(unsafe_code\)|unsafe_code = "forbid"' \
    | grep -vE '"[^"]*\bunsafe\b[^"]*"'; then
    echo "unsafe code is present (see above)" >&2; return 1
  fi
  grep -qE 'unsafe_code *= *"forbid"' Cargo.toml \
    || { echo "unsafe_code = \"forbid\" missing from the workspace lints" >&2; return 1; }
}

# Workflow hardening: explicit permissions, no persisted token, no pull_request_target, no
# event data interpolated into a shell script.
check_workflows() {
  step "GitHub workflows"
  local f bad=0
  for f in .github/workflows/*.yml; do
    [ -e "$f" ] || continue
    grep -qE '^permissions:' "$f" || { echo "$f: permissions: missing" >&2; bad=1; }
    grep -qE 'pull_request_target' "$f" && { echo "$f: pull_request_target forbidden" >&2; bad=1; }
    grep -E '^\s*run:' "$f" | grep -E 'github\.(event|head_ref)' && { echo "$f: event data in a run:" >&2; bad=1; }
    # every checkout must refuse to persist the token
    local n_checkout n_safe
    n_checkout=$(grep -c 'actions/checkout@' "$f" || true)
    n_safe=$(grep -c 'persist-credentials: false' "$f" || true)
    [ "$n_checkout" -le "$n_safe" ] || { echo "$f: checkout without persist-credentials: false" >&2; bad=1; }
  done
  if command -v actionlint > /dev/null; then actionlint .github/workflows/*.yml || bad=1
  else echo "(actionlint missing: syntax analysis of the workflows skipped)"; fi
  return $bad
}

security() {
  need cargo-deny "cargo install cargo-deny --locked"
  step "dependencies: advisories, licences, sources, duplicates (cargo-deny)"; cargo deny --locked check
  step "Cargo.lock up to date"; cargo metadata --locked --format-version 1 > /dev/null
  check_no_unsafe
  check_workflows
  step "secrets (tree and history)"; tools/scan-secrets.sh --history
}

run_parallel() {
  mkdir -p target/ci
  local pids=() names=(quality security) i rc=0
  for n in "${names[@]}"; do
    ( "$0" "$n" > "target/ci/$n.log" 2>&1 ) &
    pids+=($!)
  done
  for i in "${!pids[@]}"; do
    if wait "${pids[$i]}"; then printf '\033[32m✔ %s\033[0m\n' "${names[$i]}"
    else printf '\033[31m✘ %s\033[0m (log: target/ci/%s.log)\n' "${names[$i]}" "${names[$i]}"; tail -n 25 "target/ci/${names[$i]}.log"; rc=1; fi
  done
  return $rc
}

case "${1:-all}" in
  quality) quality ;;
  security) security ;;
  all) run_parallel ;;
  --list) echo "quality security all" ;;
  *) echo "usage: $0 [quality|security|all|--list]" >&2; exit 2 ;;
esac
