#!/bin/sh
# Publishes the GitHub release of a tag with the binaries of dist/ (the `release-*` jobs of
# .gitlab-ci.yml; image with curl and jq). It runs twice: once with the Linux x86_64 binary, so that
# it can be downloaded a few minutes after the tag, then with all five. The release is created if it
# does not exist yet, otherwise its description is updated; each binary not yet there is uploaded,
# then SHA256SUMS is replaced; the SHA-256 of each is also in the release's description and in this
# log. The addresses stay the same from one release to the next:
#   https://github.com/<repo>/releases/download/<tag>/skemo-<target>   (and …/SHA256SUMS)
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
echo "binaries:"; ls -l skemo-*; sha256sum skemo-* | tee SHA256SUMS

pre=false; case "$tag" in *-*) pre=true ;; esac
# the first run publishes one binary: the others follow
later=""
[ "$(ls skemo-* | wc -l)" -lt 5 ] && later=" The other platforms follow in a few minutes."
notes=$(cat <<EOF
One binary per platform: download it, make it executable, run it.$later

| Your machine | File |
|---|---|
| Linux, Intel or AMD | \`skemo-linux-x86_64\` |
| Linux, ARM (Raspberry Pi 4/5, Graviton…) | \`skemo-linux-arm64\` |
| Mac with Apple silicon (M1 and later) | \`skemo-macos-arm64\` |
| Mac with an Intel processor | \`skemo-macos-x86_64\` |
| Windows, Intel or AMD | \`skemo-windows-x86_64.exe\` |

Linux and macOS in one line: \`curl -fsSL https://skemo.net/install.sh | sh\`

Check it against its line below, or against \`SHA256SUMS\` (\`sha256sum -c --ignore-missing SHA256SUMS\`, or
\`shasum -a 256 -c --ignore-missing SHA256SUMS\` on macOS). Install page: https://skemo.net/install
The licences of what it embeds (fonts, icons, Rust crates): \`skemo --licences\`.

\`\`\`
$(cat SHA256SUMS)
\`\`\`
EOF
)

gh "$api/git/ref/tags/$tag" > /dev/null || { echo "tag $tag not readable on GitHub: push it there first (or check the token)" >&2; exit 1; }
body=$(jq -n --arg t "$tag" --arg c "${CI_COMMIT_SHA:-}" --arg n "$notes" --argjson p "$pre" \
  '{tag_name: $t, name: ("skemo " + $t), body: $n, prerelease: $p} + (if $c == "" then {} else {target_commitish: $c} end)')
# stdin carries the token (gh), so the body goes through a file
printf '%s' "$body" > ../release.json
if id=$(gh "$api/releases/tags/$tag" 2> /dev/null | jq -er .id); then
  gh -X PATCH "$api/releases/$id" --data-binary @../release.json > /dev/null
  echo "release $tag exists (id $id): description updated"
else
  id=$(gh -X POST "$api/releases" --data-binary @../release.json | jq -er .id)
  echo "release $tag created (id $id)"
fi

# the assets already there: a binary is uploaded once, SHA256SUMS is replaced
assets=$(gh "$api/releases/$id/assets?per_page=100")
for f in skemo-* SHA256SUMS; do
  old=$(printf '%s' "$assets" | jq -r --arg n "$f" '.[] | select(.name == $n) | .id')
  if [ -n "$old" ]; then
    [ "$f" = SHA256SUMS ] || { echo "already there: $f"; continue; }
    gh -X DELETE "$api/releases/assets/$old" > /dev/null
  fi
  gh -X POST -H "Content-Type: application/octet-stream" --data-binary "@$f" \
    "https://uploads.github.com/repos/$repo/releases/$id/assets?name=$f" | jq -r '"uploaded " + .name + " (" + (.size|tostring) + " bytes)"'
done
