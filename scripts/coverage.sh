#!/usr/bin/env bash
# Measure code coverage of the port with cargo-llvm-cov.
#
# This is not run in the development sandbox this port was built in --
# that sandbox pins an older rustc (1.75) for compatibility reasons
# documented in web/README.md, and cargo-llvm-cov's own dependency
# chain requires a newer one. Run this on a normal, up-to-date local
# machine instead:
#
#   rustup update
#   cargo install cargo-llvm-cov
#   bash scripts/coverage.sh
#
# Writes a summary to stdout and a full HTML report to
# target/llvm-cov/html/index.html.

set -euo pipefail
cd "$(dirname "$0")/.."

if ! cargo llvm-cov --version >/dev/null 2>&1; then
  echo "cargo-llvm-cov not found. Install it with:" >&2
  echo "  cargo install cargo-llvm-cov" >&2
  exit 1
fi

echo "==> Running cargo llvm-cov over the full test suite"
cargo llvm-cov --workspace --html
cargo llvm-cov report

echo
echo "Full HTML report: target/llvm-cov/html/index.html"
