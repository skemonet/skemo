#!/bin/sh
# Publishes the GitHub release of a tag with the binaries of dist/ (the `release` job of
# .gitlab-ci.yml; image with curl and jq). The release is created if it does not exist yet, then
# each binary is uploaded; the SHA-256 of each is in the release's description and in this log.
#   GITHUB_TOKEN  a token allowed to write the repository's contents (tag pipelines only)
#   GITHUB_REPO   owner/name (default skemonet/skemo)
#   CI_COMMIT_TAG the tag (vX.Y.Z; a hyphen, as in -rc.1, gives a pre-release)
set -eu
repo="${GITHUB_REPO:-skemonet/skemo}"
tag="${CI_COMMIT_TAG:?a tag pipeline only}"
: "${GITHUB_TOKEN:?missing: add GITHUB_TOKEN to the tag variables of the CI}"
api="https://api.github.com/repos/$repo"

# curl with the token read from stdin (never on the command line nor in the log); on an HTTP error,
# GitHub's message goes to stderr and the call fails
gh() {
  out=$(mktemp)
  code=$(printf 'header = "Authorization: Bearer %s"\nheader = "Accept: application/vnd.github+json"\n' "$GITHUB_TOKEN" \
    | curl -sS -K - -o "$out" -w '%{http_code}' "$@") || { rm -f "$out"; return 1; }
  case "$code" in
    2??) cat "$out"; rm -f "$out" ;;
    *) echo "GitHub API: HTTP $code: $(jq -c '{message, errors}' "$out" 2> /dev/null || head -c 300 "$out")" >&2
       rm -f "$out"; return 1 ;;
  esac
}

cd dist
echo "binaries:"; ls -l skemo-*; sha256sum skemo-* | tee ../SHA256SUMS

pre=false; case "$tag" in *-*) pre=true ;; esac
notes=$(cat <<EOF
One binary per platform: download it, make it executable, run it.

| Your machine | File |
|---|---|
| Linux, Intel or AMD | \`skemo-linux-x86_64\` |
| Linux, ARM (Raspberry Pi 4/5, Graviton…) | \`skemo-linux-arm64\` |
| Mac with Apple silicon (M1 and later) | \`skemo-macos-arm64\` |
| Windows, Intel or AMD | \`skemo-windows-x86_64.exe\` |

Linux and macOS in one line: \`curl -fsSL https://raw.githubusercontent.com/$repo/main/cli/install.sh | sh\`

Check it against its line below (\`sha256sum -c\`, or \`shasum -a 256 -c\` on macOS).
The licences of what it embeds (fonts, icons, Rust crates): \`skemo --licences\`.

\`\`\`
$(cat ../SHA256SUMS)
\`\`\`
EOF
)

gh "$api/git/ref/tags/$tag" > /dev/null || { echo "tag $tag not readable on GitHub: push it there first (or check the token)" >&2; exit 1; }
if id=$(gh "$api/releases/tags/$tag" 2> /dev/null | jq -er .id); then
  echo "release $tag exists (id $id)"
else
  body=$(jq -n --arg t "$tag" --arg c "${CI_COMMIT_SHA:-}" --arg n "$notes" --argjson p "$pre" \
    '{tag_name: $t, name: ("skemo " + $t), body: $n, prerelease: $p} + (if $c == "" then {} else {target_commitish: $c} end)')
  # stdin carries the token (gh), so the body goes through a file
  printf '%s' "$body" > ../release.json
  id=$(gh -X POST "$api/releases" --data-binary @../release.json | jq -er .id)
  echo "release $tag created (id $id)"
fi

for f in skemo-*; do
  gh -X POST -H "Content-Type: application/octet-stream" --data-binary "@$f" \
    "https://uploads.github.com/repos/$repo/releases/$id/assets?name=$f" | jq -r '"uploaded " + .name + " (" + (.size|tostring) + " bytes)"'
done
