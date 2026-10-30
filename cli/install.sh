#!/bin/sh
# Installs the `skemo` binary from a GitHub release (Linux and macOS).
#   curl -fsSL https://raw.githubusercontent.com/skemonet/skemo/main/cli/install.sh | sh
#   SKEMO_VERSION=v0.1.0-rc.3 SKEMO_BIN=~/bin sh install.sh
# Variables: SKEMO_VERSION (default: latest release, pre-releases included), SKEMO_BIN (default
# ~/.local/bin), GITHUB_TOKEN (optional: a higher API rate limit).
# The binary is checked against the SHA-256 the release's description gives before it is installed.
set -eu

repo="skemonet/skemo"
bin_dir="${SKEMO_BIN:-$HOME/.local/bin}"
api="https://api.github.com/repos/$repo"

die() { echo "install.sh: $*" >&2; exit 1; }
need() { command -v "$1" >/dev/null 2>&1 || die "$1 is required"; }
need curl; need uname

case "$(uname -s)-$(uname -m)" in
  Linux-x86_64) asset=skemo-linux-x86_64 target=x86_64-unknown-linux-musl ;;
  Linux-aarch64 | Linux-arm64) asset=skemo-linux-arm64 target=aarch64-unknown-linux-musl ;;
  Darwin-arm64) asset=skemo-macos-arm64 target=aarch64-apple-darwin ;;
  *) die "no binary for $(uname -s) $(uname -m) (Windows: download skemo-windows-x86_64.exe from the releases page)" ;;
esac

auth=""
[ -n "${GITHUB_TOKEN:-}" ] && auth="Authorization: Bearer $GITHUB_TOKEN"
get() { # get URL [accept]
  if [ -n "$auth" ]; then curl -fsSL -H "$auth" -H "Accept: ${2:-application/vnd.github+json}" "$1"
  else curl -fsSL -H "Accept: ${2:-application/vnd.github+json}" "$1"; fi
}

tag="${SKEMO_VERSION:-}"
if [ -z "$tag" ]; then
  tag="$(get "$api/releases?per_page=1" | sed -n 's/.*"tag_name": *"\([^"]*\)".*/\1/p' | head -n 1)"
  [ -n "$tag" ] || die "no release found"
fi
echo "installing skemo $tag ($asset) into $bin_dir"

release="$(get "$api/releases/tags/$tag")"
asset_url() { # asset file name -> API url of the asset
  printf '%s' "$release" | tr ',' '\n' | grep -B8 "\"name\": *\"$1\"" | sed -n 's/.*"url": *"\(https:[^"]*\/assets\/[0-9]*\)".*/\1/p' | tail -n 1
}

tmp="$(mktemp -d)"; trap 'rm -rf "$tmp"' EXIT
# releases up to v0.1.0-rc.3 named their binaries skemo-<version>-<target>
name="$asset"; url="$(asset_url "$name")"
[ -n "$url" ] || { name="skemo-$tag-$target"; url="$(asset_url "$name")"; }
[ -n "$url" ] || die "asset $asset not found in release $tag"
get "$url" application/octet-stream > "$tmp/skemo"

# the release's description holds one line per binary: "<sha256>  <name>"
expected="$(printf '%s' "$release" | grep -o "[0-9a-f]\{64\}  $name" | head -n 1 | cut -d' ' -f1)"
[ -n "$expected" ] || die "no SHA-256 for $name in the description of release $tag"
if command -v sha256sum >/dev/null 2>&1; then actual="$(sha256sum "$tmp/skemo" | cut -d' ' -f1)"
else actual="$(shasum -a 256 "$tmp/skemo" | cut -d' ' -f1)"; fi
[ "$expected" = "$actual" ] || die "SHA-256 mismatch (expected $expected, got $actual)"

mkdir -p "$bin_dir"
install -m 755 "$tmp/skemo" "$bin_dir/skemo"
echo "installed: $bin_dir/skemo"
