#!/usr/bin/env bash
# Entry point for the container. See Dockerfile for usage.
set -euo pipefail

case "${1:-test}" in
  test)
    echo "=== fixture parity against the original's own suite ==="
    cargo test --test fixtures -- --nocapture
    echo
    echo "=== unit and doc tests ==="
    cargo test
    ;;

  bench)
    bash scripts/bench.sh
    ;;

  differential)
    shift || true
    bash scripts/run_differential.sh "$@"
    ;;

  shell)
    exec /bin/bash
    ;;

  *)
    exec "$@"
    ;;
esac
