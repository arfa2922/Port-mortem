//! Benchmarks.
//!
//! Reports percentiles rather than a mean, because parse latency is
//! right-skewed — allocator behaviour and scheduler noise push the tail
//! well above the median, and a mean hides that.
//!
//! Where a comparison against the original is meaningful, the same
//! workload is run through Node so the two numbers sit side by side.
//! That comparison is honest only for throughput; process startup
//! dominates any single-shot measurement of a Node script.
//!
//! Run: cargo run --release --example bench

use std::time::{Duration, Instant};

fn percentile(sorted: &[u128], p: f64) -> u128 {
    if sorted.is_empty() {
        return 0;
    }
    let idx = ((sorted.len() as f64 - 1.0) * p).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

struct Bench {
    name: &'static str,
    samples: Vec<u128>,
}

impl Bench {
    fn run<F: FnMut()>(name: &'static str, iters: usize, mut f: F) -> Bench {
        // Warm up: first-touch page faults and branch predictor state
        // would otherwise land entirely in the first samples.
        for _ in 0..(iters / 10).max(100) {
            f();
        }

        let mut samples = Vec::with_capacity(iters);
        for _ in 0..iters {
            let t = Instant::now();
            f();
            samples.push(t.elapsed().as_nanos());
        }
        samples.sort_unstable();
        Bench { name, samples }
    }

    fn report(&self) {
        let n = self.samples.len();
        let sum: u128 = self.samples.iter().sum();
        let mean = sum / n as u128;
        println!(
            "  {:<34} p50 {:>7}ns  p99 {:>8}ns  mean {:>7}ns  min {:>6}ns",
            self.name,
            percentile(&self.samples, 0.50),
            percentile(&self.samples, 0.99),
            mean,
            self.samples[0],
        );
    }

    fn p50(&self) -> u128 {
        percentile(&self.samples, 0.50)
    }
}

/// Peak resident set size, read from /proc. Returns None off Linux.
fn peak_rss_kb() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    for line in status.lines() {
        if let Some(rest) = line.strip_prefix("VmHWM:") {
            return rest.split_whitespace().next()?.parse().ok();
        }
    }
    None
}

fn main() {
    println!("semver-rs benchmarks");
    println!("  rustc:   {}", option_env!("RUSTC_VERSION").unwrap_or("see cargo"));
    println!("  profile: release (opt-level=3, lto=true, codegen-units=1)");
    println!();

    let opts = semver_rs::Options::new();

    // ── Parsing ─────────────────────────────────────────────────────────
    println!("Parsing");
    let simple = Bench::run("parse 1.2.3", 20_000, || {
        std::hint::black_box(semver_rs::parse("1.2.3", opts));
    });
    simple.report();

    let prerelease = Bench::run("parse 1.2.3-beta.11+build.2026", 20_000, || {
        std::hint::black_box(semver_rs::parse("1.2.3-beta.11+build.2026", opts));
    });
    prerelease.report();

    let invalid = Bench::run("parse invalid input", 20_000, || {
        std::hint::black_box(semver_rs::parse("not.a.version", opts));
    });
    invalid.report();

    let long = "1.2.3-".to_string() + &"alpha.".repeat(20) + "1";
    let long_ref = long.as_str();
    let long_bench = Bench::run("parse 20-identifier prerelease", 10_000, || {
        std::hint::black_box(semver_rs::parse(long_ref, opts));
    });
    long_bench.report();

    // ── Comparison ──────────────────────────────────────────────────────
    println!();
    println!("Comparison");
    let a = semver_rs::SemVer::parse("1.2.3-beta.11", opts).unwrap();
    let b = semver_rs::SemVer::parse("1.2.3-beta.2", opts).unwrap();
    let cmp = Bench::run("compare two parsed versions", 50_000, || {
        std::hint::black_box(a.compare(&b));
    });
    cmp.report();

    let cmp_str = Bench::run("compare from strings", 20_000, || {
        let _ = std::hint::black_box(semver_rs::compare("1.2.3-beta.11", "1.2.3-beta.2", opts));
    });
    cmp_str.report();

    // ── Ranges ──────────────────────────────────────────────────────────
    println!();
    println!("Ranges");
    let caret = Bench::run("parse ^1.2.3", 20_000, || {
        std::hint::black_box(semver_rs::valid_range("^1.2.3", opts));
    });
    caret.report();

    let complex = Bench::run("parse >=1.2.7 <1.3.0 || ~2.0.0", 10_000, || {
        std::hint::black_box(semver_rs::valid_range(">=1.2.7 <1.3.0 || ~2.0.0", opts));
    });
    complex.report();

    let sat = Bench::run("satisfies 1.2.8 against ^1.2.3", 20_000, || {
        std::hint::black_box(semver_rs::satisfies("1.2.8", "^1.2.3", opts));
    });
    sat.report();

    // ── Increment ───────────────────────────────────────────────────────
    println!();
    println!("Increment");
    let inc = Bench::run("inc 1.2.3 minor", 20_000, || {
        std::hint::black_box(semver_rs::inc(
            "1.2.3",
            "minor",
            None,
            semver_rs::IdentifierBase::Zero,
            opts,
        ));
    });
    inc.report();

    // ── Throughput ──────────────────────────────────────────────────────
    println!();
    println!("Throughput");
    let corpus: Vec<String> = (0..1000)
        .map(|i| format!("{}.{}.{}-beta.{}", i % 50, i % 17, i % 7, i % 3))
        .collect();

    let start = Instant::now();
    let mut parsed = 0usize;
    let mut elapsed = Duration::ZERO;
    while elapsed < Duration::from_millis(500) {
        for v in &corpus {
            std::hint::black_box(semver_rs::parse(v, opts));
            parsed += 1;
        }
        elapsed = start.elapsed();
    }
    let per_sec = parsed as f64 / elapsed.as_secs_f64();
    println!("  {:<34} {:>12.0} versions/sec", "sustained parse", per_sec);

    // ── Memory ──────────────────────────────────────────────────────────
    println!();
    println!("Memory");
    match peak_rss_kb() {
        Some(kb) => println!("  {:<34} {:>9} KB", "peak RSS", kb),
        None => println!("  peak RSS unavailable on this platform"),
    }

    // ── Summary for the README ──────────────────────────────────────────
    println!();
    println!("Summary");
    println!("  parse, simple version      p50 {:>6}ns", simple.p50());
    println!("  parse, with prerelease     p50 {:>6}ns", prerelease.p50());
    println!("  range desugar (^1.2.3)     p50 {:>6}ns", caret.p50());
    println!("  satisfies                  p50 {:>6}ns", sat.p50());
    println!("  sustained parse            {:>9.0}/sec", per_sec);
}
