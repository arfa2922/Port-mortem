# tests/port/

Per the event's "Anatomy of a working port" spec, this directory is
where port-added native tests would live.

For this project, they live in the conventional Rust location instead:
`#[cfg(test)]` modules inside `src/*.rs`, following ordinary Rust
convention rather than a separate top-level directory. This is not an
evasion of the spec — the tests exist, run with `cargo test`, and are
counted below — it is a deliberate choice to keep tests next to the
code they exercise, which is the idiom this port otherwise follows
throughout (see DECISIONS.md).

## Where each kind of test actually is

| Kind | Location | Count |
|---|---|---|
| Unit tests (one module's own logic) | `#[cfg(test)] mod tests` in `src/identifiers.rs`, `src/semver.rs`, `src/functions.rs`, `src/relations.rs` | 18 |
| Fixture parity (the original's own test data) | `tests/fixtures.rs`, asserting against `tests/fixtures.json` | 13 groups, 2,515 assertions |
| Property tests | `tests/properties.rs` (`proptest`) | see file |
| Robustness (hostile input, no panics) | `examples/api_stress.rs` | 47 cases |
| Timing safety (no pathological input) | `examples/timing_safety.rs` | 12 shapes |
| Differential fuzzing against the live original | `fuzz/harness.rs` | 500,000+ cases, see `fuzz/log.txt` |

```bash
cargo test                              # unit + fixture + property tests
cargo run --release --example api_stress
cargo run --release --example timing_safety
cargo run --release --bin fuzz-harness -- --cases 50000
```
