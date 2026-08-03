#!/usr/bin/env bash
# Fetch the original and pin its test suite.
#
# Run once before the hackathon, and again at kickoff to write the hash
# that judges verify against at submission.

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
REPO=https://github.com/npm/node-semver
VENDOR="$ROOT/vendor/node-semver"

echo "==> Cloning $REPO"
rm -rf "$VENDOR"
mkdir -p "$ROOT/vendor"
git clone --depth 1 -q "$REPO" "$VENDOR"

echo "==> Pinning the original's sources and test suite"
rm -rf "$ROOT/tests/original"
mkdir -p "$ROOT/tests/original"
cp -r "$VENDOR/test/." "$ROOT/tests/original/"
for d in internal functions classes ranges; do
  [ -d "$VENDOR/$d" ] && cp -r "$VENDOR/$d" "$ROOT/tests/original/js-$d"
done
cp "$VENDOR/index.js" "$ROOT/tests/original/" 2>/dev/null || true

cd "$ROOT"
find tests/original -type f -name '*.js' -exec sha256sum {} \; | sort > kickoff.hash

echo "    $(wc -l < kickoff.hash) files pinned"
echo "    $(ls tests/original/fixtures/*.js 2>/dev/null | wc -l) fixture files"

echo "==> Exporting fixtures to JSON"
node scripts/export_fixtures.js | tail -3

cat <<'MSG'

Commit kickoff.hash in your first commit. Judges compare it against the
suite at submission to confirm no test file was edited.

Next:
  cargo test --test fixtures -- --nocapture
  cargo run --release --bin fuzz-harness -- --cases 50000
MSG
