#!/usr/bin/env bash
# Differential fuzz against the original, across several seeds.
#
#   bash scripts/run_differential.sh
#   CASES=100000 SEEDS="1 2 3" bash scripts/run_differential.sh

set -euo pipefail

CASES=${CASES:-50000}
SEEDS=${SEEDS:-"1 7 42 99 555 1337 2026 8888 31337 99999"}
LOG=fuzz/differential-multiseed.log
BIN=./target/release/fuzz-harness

if [ ! -d vendor/node-semver ]; then
  echo "vendor/node-semver missing — run: bash scripts/fetch_original.sh" >&2
  exit 2
fi

cargo build --release --bin fuzz-harness

{
  echo "semver-rs differential session"
  echo "date:   $(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "rustc:  $(rustc --version)"
  echo "node:   $(node --version)"
  echo "oracle: vendor/node-semver, run live"
  echo "cases:  $CASES per seed"
  echo "seeds:  $SEEDS"
  echo
  echo "Both implementations receive the same generated input; any"
  echo "disagreement is a real behavioural divergence, not a guess."
  echo "=================================================================="
} > "$LOG"

TOTAL=0
FAILED=0
for SEED in $SEEDS; do
  echo "==> seed $SEED"
  { echo; echo "=== seed $SEED ==="; } >> "$LOG"
  if "$BIN" --cases "$CASES" --seed "$SEED" >> "$LOG" 2>&1; then
    echo "    agreed on all $CASES cases"
  else
    echo "    DIVERGENCES — see $LOG"
    FAILED=1
  fi
  TOTAL=$((TOTAL + CASES))
done

{
  echo
  echo "=================================================================="
  echo "total cases: $TOTAL"
  echo "session end: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
} >> "$LOG"

echo
echo "$TOTAL cases; log in $LOG"
[ "$FAILED" -eq 0 ] || exit 1
echo "The port and the original agreed on every case."
