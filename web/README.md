# web/

Two supplementary pieces, neither required to evaluate the port itself
— the port is `src/`, verified by `tests/`, `fuzz/`, and `bench/`. This
folder exists for a live demo and a quick visual summary.

## `dashboard.html` — open directly, no build step

A static page pulling the same numbers already in `README.md` and
`bench/results.json` into one view: fixture pass rate, differential
results, both upstream bugs, benchmarks, robustness. No JavaScript
computes anything here — every number is copied from a committed
artifact in this repo, with the command to reproduce it. Open the file
directly in a browser; nothing to build.

## `demo.html` — interactive, needs a WASM build first

A live demo of the actual compiled Rust port running as WebAssembly in
the browser — parse a version, compare two, check a range, desugar a
range, increment a version. Every result comes from `semver_rs`, the
same library the CLI and test suite use; `web/wasm/src/lib.rs` is a
thin `wasm-bindgen` wrapper that adds no logic of its own.

### Building it

```bash
rustup update                # wasm-bindgen's macro crate needs rustc 1.77+
cargo install wasm-pack      # if not already installed
bash build-wasm.sh
```

Then open `web/demo.html` directly — no server needed, no bundler.

**Why this needs a separate build step from the rest of the repo:**
`src/` (the port itself) targets ordinary native Rust and has no WASM
dependency. `web/wasm/` is a separate crate specifically so the core
library's dependency list and `#![forbid(unsafe_code)]`-style
guarantees stay about the port, not about a browser demo layered on
top of it — nothing in `src/` knows `web/wasm/` exists. This does mean
building the demo needs its own toolchain step, documented here rather
than silently assumed.

### A note on this repo's own build environment

This port was developed and CI-tested with rustc 1.75. `wasm-bindgen`'s
current macro crate requires rustc 1.77+, so building `web/wasm/`
specifically needs a newer local toolchain (`rustup update` above) even
though the core port does not. If `web/wasm/` won't build for you, the
port itself is entirely unaffected — `cargo test` at the repo root
never touches this directory.
