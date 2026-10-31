#!/usr/bin/env bash
# Search for secrets (private keys, tokens, plain-text passwords) in the files tracked by git, and
# with --history in every commit. No dependency. Exceptions: `.secrets-allow` (one extended regular
# expression per line, applied to the line found).
set -euo pipefail
cd "$(dirname "$0")/.."

PATTERNS=(
  '-----BEGIN [A-Z ]*PRIVATE KEY-----'
  'AKIA[0-9A-Z]{16}'
  'gh[pousr]_[A-Za-z0-9]{36,}'
  'github_pat_[A-Za-z0-9_]{22,}'
  'xox[baprs]-[A-Za-z0-9-]{10,}'
  'AIza[0-9A-Za-z_-]{35}'
  'sk-[A-Za-z0-9]{32,}'
  '(api[_-]?key|secret|token|passw(or)?d)["'\'']? *[:=] *["'\''][^"'\'' $]{12,}["'\'']'
)
# references to GitHub secrets, obvious examples
ALLOW=('\$\{\{ *secrets\.' 'example|exemple|CHANGEME|xxxxx')
[ -f .secrets-allow ] && while IFS= read -r l; do [ -n "$l" ] && ALLOW+=("$l"); done < .secrets-allow

pattern=$(IFS='|'; echo "${PATTERNS[*]}")
allow=$(IFS='|'; echo "${ALLOW[*]}")

{
  # the tracked files, or every file when the folder is not a git checkout (a CI build folder)
  { git ls-files -z 2> /dev/null || find . -path ./target -prune -o -path ./.git -prune -o -path ./.cargo-home -prune -o -type f -print0; } \
    | grep -zvE '(^|/)(Cargo\.lock|.*\.(png|gif|apng|woff2?|ttf|otf|wasm))$' \
    | xargs -0 grep -InE -e "$pattern" 2> /dev/null || true
  if [ "${1:-}" = "--history" ]; then
    git log --all -p --no-color --unified=0 --format='' 2> /dev/null \
      | grep -E '^\+' | grep -E -e "$pattern" || true
  fi
} | grep -vE -e "$allow" > "${TMPDIR:-/tmp}/skemo-secrets.$$" || true

if [ -s "${TMPDIR:-/tmp}/skemo-secrets.$$" ]; then
  cut -c1-200 "${TMPDIR:-/tmp}/skemo-secrets.$$"; rm -f "${TMPDIR:-/tmp}/skemo-secrets.$$"
  echo "possible secret(s) found: fix them, or document them in .secrets-allow" >&2
  exit 1
fi
rm -f "${TMPDIR:-/tmp}/skemo-secrets.$$"
echo "no secret detected"
