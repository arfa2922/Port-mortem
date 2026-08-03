//! Timing safety: no input should take disproportionately long.
//!
//! The original carries a second set of regexes (`safeRe`) specifically
//! because the natural ones backtrack catastrophically. A scanner has no
//! backtracking, so this should hold by construction — but "should" is
//! not "does", and the claim is worth measuring rather than asserting.
//!
//! Budgets are calibrated at runtime rather than fixed, because a fixed
//! millisecond figure is really a proxy for "this algorithm is linear,
//! not quadratic" — and that proxy silently assumes a specific machine's
//! clock speed. Found on a real Windows machine benchmarking ~4.2x slower
//! (2.68M vs 11.4M parses/sec) than the one bench/results.json was
//! generated on: two already-linear-time cases ("5k range alternatives",
//! "satisfies against 5k alternatives") exceeded a fixed 50ms budget for
//! no reason but raw hardware speed, which would also risk failing on
//! GitHub Actions' shared runners. Calibrating against a measured
//! baseline op keeps the check honest about what it's actually testing:
//! algorithmic complexity, not one developer's laptop.

use std::time::{Duration, Instant};

fn timed<F: FnOnce()>(name: &str, budget: Duration, f: F) -> bool {
    let start = Instant::now();
    f();
    let took = start.elapsed();
    let ok = took < budget;
    println!(
        "  {:<44} {:>9.3}ms  {}",
        name,
        took.as_secs_f64() * 1000.0,
        if ok { "ok" } else { "OVER BUDGET" }
    );
    ok
}

fn main() {
    let o = semver_rs::Options::new();

    // Reference: p50 nanoseconds to parse "1.2.3", measured on the
    // machine bench/results.json's numbers came from (see that file's
    // `parsing_ns.simple_version.p50`). Used only as a speed reference
    // to scale budgets, not as a claim this machine matches it.
    const REFERENCE_PARSE_NS: f64 = 77.0;

    // Calibrate: time this machine's own baseline parse cost, same
    // input shape as the reference figure above, warmed up first.
    for _ in 0..1_000 {
        let _ = semver_rs::parse("1.2.3", o);
    }
    let calib_start = Instant::now();
    const CALIB_ITERS: u32 = 100_000;
    for _ in 0..CALIB_ITERS {
        let _ = semver_rs::parse("1.2.3", o);
    }
    let measured_ns = calib_start.elapsed().as_secs_f64() * 1_000_000_000.0 / CALIB_ITERS as f64;

    // Only ever loosen the budget, never tighten it below the baseline
    // 50ms figure -- a faster machine shouldn't make the bar stricter,
    // only a slower one should make it more lenient.
    //
    // SAFETY_MARGIN exists because the linear scale above is calibrated
    // off a single trivial "1.2.3" parse, which doesn't capture that
    // heavier cases (range alternatives, satisfies-against-many) scale
    // worse under thermal throttling or background CPU contention than
    // a tight parse loop does. Without this, noisy/throttled hardware
    // (laptops under load, shared CI runners) can trip a false
    // OVER BUDGET even though nothing algorithmic regressed -- observed
    // in practice: calibration ranged 82ns-123ns run-to-run on the same
    // machine. This still catches real O(n^2)-class regressions (which
    // blow past budget by 100x+, not 1.3x), it just stops punishing
    // ordinary hardware noise.
    const SAFETY_MARGIN: f64 = 1.5;
    let scale = (measured_ns / REFERENCE_PARSE_NS).max(1.0) * SAFETY_MARGIN;
    let budget = Duration::from_secs_f64(0.050 * scale);
    let mut all_ok = true;

    println!(
        "Baseline parse: {:.0}ns/op here vs {:.0}ns/op on the reference machine ({:.2}x, incl. {:.1}x safety margin)",
        measured_ns, REFERENCE_PARSE_NS, scale, SAFETY_MARGIN
    );
    println!(
        "Each input must complete well under {:.1}ms (50ms reference, scaled for this machine).",
        budget.as_secs_f64() * 1000.0
    );
    println!();

    // Inputs shaped to make a backtracking engine work hardest: long
    // runs of a character that could begin several productions.
    all_ok &= timed("10k digits", budget, || {
        let _ = semver_rs::parse(&"9".repeat(10_000), o);
    });

    all_ok &= timed("10k dots", budget, || {
        let _ = semver_rs::parse(&".".repeat(10_000), o);
    });

    all_ok &= timed("10k hyphens after a version", budget, || {
        let v = format!("1.2.3{}", "-".repeat(10_000));
        let _ = semver_rs::parse(&v, o);
    });

    all_ok &= timed("alternating digit-dot, 10k", budget, || {
        let v: String = "1.".repeat(5_000);
        let _ = semver_rs::parse(&v, o);
    });

    all_ok &= timed("10k prerelease identifiers", budget, || {
        let v = format!("1.2.3-{}", "a.".repeat(10_000));
        let _ = semver_rs::parse(&v, o);
    });

    all_ok &= timed("10k build identifiers", budget, || {
        let v = format!("1.2.3+{}", "a.".repeat(10_000));
        let _ = semver_rs::parse(&v, o);
    });

    all_ok &= timed("nested carets, 10k", budget, || {
        let r = format!("{}1.2.3", "^".repeat(10_000));
        let _ = semver_rs::valid_range(&r, o);
    });

    all_ok &= timed("5k range alternatives", budget, || {
        let r = (0..5_000)
            .map(|i| format!("^{i}.0.0"))
            .collect::<Vec<_>>()
            .join(" || ");
        let _ = semver_rs::valid_range(&r, o);
    });

    all_ok &= timed("5k comparators in one set", budget, || {
        let r = (0..5_000)
            .map(|i| format!(">={i}.0.0"))
            .collect::<Vec<_>>()
            .join(" ");
        let _ = semver_rs::valid_range(&r, o);
    });

    all_ok &= timed("satisfies against 5k alternatives", budget, || {
        let r = (0..5_000)
            .map(|i| format!("^{i}.0.0"))
            .collect::<Vec<_>>()
            .join(" || ");
        let _ = semver_rs::satisfies("1.2.3", &r, o);
    });

    all_ok &= timed("coerce over 100k characters", budget, || {
        let _ = semver_rs::coerce(&"a1".repeat(50_000), o);
    });

    // The length cap should reject before any real work happens.
    all_ok &= timed("1MB input (rejected by length cap)", budget, || {
        let _ = semver_rs::parse(&"1".repeat(1_000_000), o);
    });

    println!();
    if all_ok {
        println!("every input completed within budget");
    } else {
        println!("SOME INPUTS EXCEEDED BUDGET");
        std::process::exit(1);
    }
}
