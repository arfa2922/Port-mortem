//! Hostile input against the public API.
//!
//! A library returns errors; it does not abort the process. Every call
//! here is wrapped so a panic is caught and reported rather than taking
//! the harness with it.

fn main() {
    let mut checks = 0;
    let mut panicked: Vec<&str> = Vec::new();

    macro_rules! check {
        ($name:expr, $body:expr) => {
            checks += 1;
            if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| $body)).is_err() {
                panicked.push($name);
            }
        };
    }

    let o = semver_rs::Options::new();
    let loose = semver_rs::Options::loose();

    // ── Parsing ─────────────────────────────────────────────────────────
    check!("empty string", { let _ = semver_rs::parse("", o); });
    check!("only dots", { let _ = semver_rs::parse("...", o); });
    check!("only hyphens", { let _ = semver_rs::parse("---", o); });
    check!("nul byte", { let _ = semver_rs::parse("1.2.3\0", o); });
    check!("very long", { let _ = semver_rs::parse(&"9".repeat(100_000), o); });
    check!("max length exactly", { let _ = semver_rs::parse(&"1".repeat(256), o); });
    check!("one past max length", { let _ = semver_rs::parse(&"1".repeat(257), o); });
    check!("u64 overflow", { let _ = semver_rs::parse("99999999999999999999999.0.0", o); });
    check!("deep prerelease", {
        let v = format!("1.2.3-{}", "a.".repeat(10_000));
        let _ = semver_rs::parse(&v, o);
    });
    check!("unicode", { let _ = semver_rs::parse("1.2.3-日本語", o); });
    check!("emoji", { let _ = semver_rs::parse("1.2.3-🚀", o); });
    check!("rtl override", { let _ = semver_rs::parse("1.2.3-\u{202E}evil", o); });

    // ── Ranges ──────────────────────────────────────────────────────────
    check!("empty range", { let _ = semver_rs::valid_range("", o); });
    check!("only pipes", { let _ = semver_rs::valid_range("||||||", o); });
    check!("only hyphen", { let _ = semver_rs::valid_range(" - ", o); });
    check!("nested carets", { let _ = semver_rs::valid_range("^^^^1.2.3", o); });
    check!("many alternatives", {
        let r = (0..5_000).map(|i| format!("^{i}.0.0")).collect::<Vec<_>>().join(" || ");
        let _ = semver_rs::valid_range(&r, o);
    });
    check!("many comparators", {
        let r = (0..5_000).map(|i| format!(">={i}.0.0")).collect::<Vec<_>>().join(" ");
        let _ = semver_rs::valid_range(&r, o);
    });
    check!("range of whitespace", { let _ = semver_rs::valid_range("        ", o); });
    check!("operator with no version", { let _ = semver_rs::valid_range(">=", o); });
    check!("hyphen no right side", { let _ = semver_rs::valid_range("1.0.0 - ", o); });

    // ── satisfies ───────────────────────────────────────────────────────
    check!("satisfies garbage/garbage", { let _ = semver_rs::satisfies("x", "y", o); });
    check!("satisfies empty/empty", { let _ = semver_rs::satisfies("", "", o); });

    // ── inc ─────────────────────────────────────────────────────────────
    use semver_rs::IdentifierBase;
    check!("inc invalid release", {
        let _ = semver_rs::inc("1.2.3", "sideways", None, IdentifierBase::Zero, o);
    });
    check!("inc at u64 boundary", {
        let _ = semver_rs::inc("9007199254740991.0.0", "major", None, IdentifierBase::Zero, o);
    });
    check!("inc with empty identifier", {
        let _ = semver_rs::inc("1.2.3", "prerelease", Some(""), IdentifierBase::Zero, o);
    });
    check!("inc with dotted identifier", {
        let _ = semver_rs::inc("1.2.3", "prerelease", Some("a.b.c"), IdentifierBase::Zero, o);
    });
    check!("inc release on non-prerelease", {
        let _ = semver_rs::inc("1.2.3", "release", None, IdentifierBase::Zero, o);
    });

    // ── Relations ───────────────────────────────────────────────────────
    check!("intersects garbage", { let _ = semver_rs::intersects("x", "y", o); });
    check!("subset of itself", { let _ = semver_rs::subset("^1.0.0", "^1.0.0", o); });
    check!("min_version of empty set", { let _ = semver_rs::min_version("<0.0.0-0", o); });
    check!("min_version of garbage", { let _ = semver_rs::min_version("nope", o); });
    check!("gtr on garbage", { let _ = semver_rs::gtr("x", "y", o); });
    check!("to_comparators garbage", { let _ = semver_rs::to_comparators("x", o); });

    // ── Functions ───────────────────────────────────────────────────────
    check!("coerce empty", { let _ = semver_rs::coerce("", o); });
    check!("coerce huge", { let _ = semver_rs::coerce(&"1".repeat(100_000), o); });
    check!("coerce only separators", { let _ = semver_rs::coerce("....---", o); });
    check!("diff garbage", { let _ = semver_rs::diff("x", "y"); });
    check!("cmp bad operator", { let _ = semver_rs::cmp("1.0.0", "<>", "2.0.0", o); });
    check!("truncate bad kind", { let _ = semver_rs::truncate("1.2.3", "nope", o); });
    check!("sort with invalid entries", {
        let mut v: Vec<String> = ["1.0.0", "garbage", "", "2.0.0"]
            .iter().map(|s| s.to_string()).collect();
        semver_rs::sort(&mut v, o);
    });
    check!("simplify empty version list", {
        let _ = semver_rs::simplify_range(&[], "^1.0.0", o);
    });
    check!("max_satisfying empty list", {
        let _ = semver_rs::max_satisfying(&[], "^1.0.0", o);
    });

    // ── Loose mode on the same hostile input ────────────────────────────
    check!("loose: empty", { let _ = semver_rs::parse("", loose); });
    check!("loose: only v", { let _ = semver_rs::parse("v", loose); });
    check!("loose: only equals", { let _ = semver_rs::parse("=", loose); });
    check!("loose: v with no version", { let _ = semver_rs::valid_range("v", loose); });

    println!("{checks} hostile calls");
    if panicked.is_empty() {
        println!("no panics");
    } else {
        println!("PANICKED in {}:", panicked.len());
        for p in &panicked {
            println!("  {p}");
        }
        std::process::exit(1);
    }
}
