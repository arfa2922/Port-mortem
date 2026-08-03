//! Property tests.
//!
//! The fixture and differential tests check specific inputs — ones the
//! original's authors wrote down, or ones a generator happened to
//! produce. Properties check something different: statements that must
//! hold for *every* input, which a finite test list can never fully
//! cover.
//!
//! Each property here is a real algebraic law the port claims to
//! satisfy. If one of these fails, no specific fixture broke — the
//! shape of the comparison itself is wrong.

use proptest::prelude::*;
use semver_rs::Options;
use std::cmp::Ordering;

/// A generator biased toward the values that actually stress this
/// grammar: small numbers most of the time, occasional large ones,
/// occasional prerelease and build tags.
fn arb_version() -> impl Strategy<Value = String> {
    let major = 0u64..30;
    let minor = 0u64..30;
    let patch = 0u64..30;
    let pre = prop::option::of(prop::collection::vec(
        prop_oneof![
            "[a-z]{1,6}",
            "[0-9]{1,4}",
        ],
        1..3,
    ));
    let build = prop::option::of(prop::collection::vec("[a-zA-Z0-9]{1,6}", 1..2));

    (major, minor, patch, pre, build).prop_map(|(ma, mi, pa, pre, build)| {
        let mut s = format!("{ma}.{mi}.{pa}");
        if let Some(ids) = pre {
            s.push('-');
            s.push_str(&ids.join("."));
        }
        if let Some(ids) = build {
            s.push('+');
            s.push_str(&ids.join("."));
        }
        s
    })
}

proptest! {
    /// A version always compares equal to itself.
    #[test]
    fn reflexivity(v in arb_version()) {
        let o = Options::new();
        if let Some(parsed) = semver_rs::parse(&v, o) {
            prop_assert_eq!(parsed.compare(&parsed), Ordering::Equal);
        }
    }

    /// If a < b, then b > a. Ordering can't point both ways.
    #[test]
    fn antisymmetry(a in arb_version(), b in arb_version()) {
        let o = Options::new();
        if let (Some(pa), Some(pb)) = (semver_rs::parse(&a, o), semver_rs::parse(&b, o)) {
            prop_assert_eq!(pa.compare(&pb), pb.compare(&pa).reverse());
        }
    }

    /// If a <= b and b <= c, then a <= c. Without this, sorting isn't
    /// well-defined — a comparator that isn't transitive can put a
    /// sort algorithm into an infinite loop or a nonsensical order.
    #[test]
    fn transitivity(a in arb_version(), b in arb_version(), c in arb_version()) {
        let o = Options::new();
        if let (Some(pa), Some(pb), Some(pc)) =
            (semver_rs::parse(&a, o), semver_rs::parse(&b, o), semver_rs::parse(&c, o))
        {
            let ab = pa.compare(&pb);
            let bc = pb.compare(&pc);
            if ab != Ordering::Greater && bc != Ordering::Greater {
                prop_assert_ne!(pa.compare(&pc), Ordering::Greater);
            }
        }
    }

    /// Parsing is total: it returns Some or None, and never panics,
    /// for any string at all — not just well-formed version-shaped
    /// ones. This is the property api_stress.rs checks by example;
    /// here proptest searches for a counterexample instead of using
    /// a fixed list.
    #[test]
    fn parse_never_panics(s in ".{0,200}") {
        let o = Options::new();
        let _ = semver_rs::parse(&s, o);
        let _ = semver_rs::parse(&s, Options::loose());
    }

    /// Every version this port accepts must re-parse from its own
    /// canonical rendering, to the same value. A parser and formatter
    /// that drift apart is a common and quiet bug class.
    #[test]
    fn canonical_form_round_trips(v in arb_version()) {
        let o = Options::new();
        if let Some(parsed) = semver_rs::parse(&v, o) {
            let text = parsed.version();
            let reparsed = semver_rs::parse(&text, o)
                .expect("a version's own canonical form must re-parse");
            prop_assert_eq!(parsed.compare(&reparsed), Ordering::Equal);
        }
    }

    /// A version and its own precedence-equal twin (identical except
    /// for build metadata) must compare equal — the spec says build
    /// metadata never affects precedence.
    #[test]
    fn build_metadata_never_affects_precedence(
        v in arb_version(),
        build1 in "[a-zA-Z0-9]{1,8}",
        build2 in "[a-zA-Z0-9]{1,8}",
    ) {
        let o = Options::new();
        if let Some(base) = semver_rs::parse(&v, o) {
            let with_build1 = format!("{}+{}", base.version(), build1);
            let with_build2 = format!("{}+{}", base.version(), build2);
            if let (Some(a), Some(b)) =
                (semver_rs::parse(&with_build1, o), semver_rs::parse(&with_build2, o))
            {
                prop_assert_eq!(a.compare(&b), Ordering::Equal);
            }
        }
    }

    /// Ranges never panic on any string, well-formed or not.
    #[test]
    fn range_parsing_never_panics(s in ".{0,150}") {
        let o = Options::new();
        let _ = semver_rs::valid_range(&s, o);
        let _ = semver_rs::valid_range(&s, Options::loose());
    }

    /// satisfies never panics, for any version and any range text.
    #[test]
    fn satisfies_never_panics(v in ".{0,100}", r in ".{0,100}") {
        let o = Options::new();
        let _ = semver_rs::satisfies(&v, &r, o);
    }

    /// If a version satisfies a range, then a version equal to it by
    /// precedence (same fields, different build metadata) must too —
    /// build metadata never affects precedence, so it cannot affect
    /// which ranges admit a version either.
    #[test]
    fn satisfies_ignores_build_metadata(
        v in arb_version(),
        r in arb_version(),
        build in "[a-zA-Z0-9]{1,8}",
    ) {
        let o = Options::new();
        if let Some(parsed) = semver_rs::parse(&v, o) {
            let with_build = format!("{}+{}", parsed.version(), build);
            let range = format!("^{r}");
            if semver_rs::valid_range(&range, o).is_some() {
                let a = semver_rs::satisfies(&v, &range, o);
                let b = semver_rs::satisfies(&with_build, &range, o);
                prop_assert_eq!(a, b);
            }
        }
    }

    /// max_satisfying never returns a version that fails satisfies —
    /// whatever it picks has to actually be in the range it was asked
    /// to search.
    #[test]
    fn max_satisfying_result_always_satisfies(
        versions in prop::collection::vec(arb_version(), 1..8),
        r in arb_version(),
    ) {
        let o = Options::new();
        let range = format!("^{r}");
        if semver_rs::valid_range(&range, o).is_some() {
            let refs: Vec<&str> = versions.iter().map(|s| s.as_str()).collect();
            if let Some(best) = semver_rs::max_satisfying(&refs, &range, o) {
                prop_assert!(semver_rs::satisfies(best, &range, o));
            }
        }
    }
}
