# Benchmark methodology

How the numbers in `bench/results.json` and `bench/results.md` were
produced, and what they do and don't claim.

## What's measured

| Category | What it captures |
|---|---|
| Parsing latency | Single-call cost of `parse()` on four input shapes |
| Comparison latency | Cost of comparing two already-parsed versions, and of comparing from raw strings |
| Range latency | Desugaring a range and testing `satisfies` |
| Increment latency | Cost of `inc()` |
| Sustained throughput | Versions/sec over a fixed 500ms window, warmed up |
| Peak RSS | `/proc/self/status` `VmHWM` during the throughput run |
| Process startup | Wall-clock time to launch the process and complete one call, as a caller invoking either as a subprocess would experience it |

## How

**Latency (`p50`/`p99`/`mean`/`min`)**: `examples/bench.rs`. Each
measurement warms up for a tenth of its final iteration count before
sampling, because the first calls include page faults and branch
predictor warm-up that don't represent steady-state cost. Percentiles
are reported rather than only a mean, because parse latency is
right-skewed — a mean alone hides the tail, and the tail is what a
caller waiting on a request actually experiences.

**Throughput**: a fixed 500ms window after warm-up, counting how many
times a 1,000-version corpus can be parsed. The corpus
(`{i%50}.{i%17}.{i%7}-beta.{i%3}`) is filename-listing-shaped rather
than adversarial, to measure the common case rather than a worst case.

**Peak RSS**: read directly from `/proc/self/status`, not estimated.
Only available on Linux; not measured on other platforms in this pass.

**Startup**: `date +%s%N` bracketing a full process launch — five runs
each of `semver-rs 1.2.3` and `node -e "require('./vendor/node-semver')
.valid('1.2.3')"` — with the median reported. `/usr/bin/time` was not
available in the environment this was run in, so wall-clock bracketing
was used instead; the numbers include shell fork/exec overhead on both
sides equally, so the comparison between them is still fair even
though neither absolute number is a pure "time inside the runtime"
figure.

**Against the original**: `bench/compare.js` runs the same 1,000-version
corpus through `node-semver` directly, same 500ms window, same warm-up
discipline, so the comparison is apples-to-apples rather than a
synthetic number pulled from documentation.

## What this doesn't claim

- **Not every workload shape.** The throughput corpus is one shape (short,
  filename-like strings). A corpus of very long prerelease chains, or
  deliberately adversarial range expressions, would show a different
  ratio — see `fuzz/harness.rs` and `examples/timing_safety.rs` for how
  those shapes are exercised instead, on correctness and worst-case
  latency rather than throughput.
- **Not a claim that Node itself is slow.** `node-semver`'s JavaScript
  is already a fairly direct implementation; the gap here is mostly the
  cost difference between an interpreted, garbage-collected runtime and
  a compiled native binary for this specific workload, not a defect in
  the original.
- **Machine-specific.** Numbers vary by hardware. Reproduce with
  `bash scripts/bench.sh` on your own machine rather than trusting the
  numbers here to transfer.

## Reproduce

```bash
bash scripts/fetch_original.sh   # if vendor/node-semver isn't present
bash scripts/bench.sh
```

Writes `bench/results.md`. `bench/results.json` is currently maintained
by hand from the same run; a future pass could have `bench.sh` emit
JSON directly rather than requiring the two to be kept in sync manually
— noted here rather than left silently inconsistent.
