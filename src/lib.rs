//! A SemVer implementation in safe Rust.
//!
//! Port of [node-semver](https://github.com/npm/node-semver) for
//! Port Mortem 2026, Track H (JavaScript → Rust).
//!
//! # Safety
//! `#![forbid(unsafe_code)]` below is a compiler-enforced hard error,
//! not a lint — it cannot be locally silenced with `#[allow(...)]` the
//! way `#![deny(unsafe_code)]` could be. No FFI, no Node runtime.
//!
//! # Equivalence
//! The original's own fixtures are exported to JSON by
//! `scripts/export_fixtures.js` and asserted against directly, so the
//! port is checked against the same expectations the original is, rather
//! than against a re-typed approximation of them.
//!
//! # Example
//!
//! This is the same example shown in the README, and `cargo test` runs
//! it as a doc-test on every build — it is checked, not just displayed.
//!
//! ```
//! use semver_rs::{parse, satisfies, valid_range};
//!
//! let v = parse("1.2.3-beta.1", false).unwrap();
//! assert_eq!(v.major, 1);
//! assert!(v.is_prerelease());
//!
//! assert!(satisfies("1.2.5", "^1.2.3", false));
//! assert_eq!(valid_range("^1.2.3", false).as_deref(), Some(">=1.2.3 <2.0.0-0"));
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod comparator;
pub mod constants;
pub mod error;
pub mod functions;
pub mod identifiers;
pub mod increment;
pub mod range;
pub mod relations;
pub mod semver;

pub use comparator::{Comparator, Op};
pub use constants::Options;
pub use range::{ComparatorSet, Range};
pub use relations::{gtr, intersects, ltr, min_version, outside, subset, to_comparators};
pub use error::{Error, Result};
pub use functions::{
    clean, cmp, coerce, compare_build, compare_loose, diff, major, minor, patch, prerelease,
    rsort, simplify_range, sort, truncate,
};
pub use identifiers::{compare_identifiers, rcompare_identifiers, Identifier};
pub use increment::{IdentifierBase, Release};
pub use semver::SemVer;

/// Parse a version, returning `None` when it is invalid.
///
/// Mirrors the original's `parse()`, which returns `null` rather than
/// throwing. Use [`SemVer::parse`] when the reason matters.
pub fn parse(version: &str, options: impl Into<Options>) -> Option<SemVer> {
    SemVer::parse(version, options).ok()
}

/// Whether a version string is valid, returning its canonical form.
///
/// Mirrors `valid()`.
pub fn valid(version: &str, options: impl Into<Options>) -> Option<String> {
    parse(version, options).map(|v| v.version())
}

/// Compare two versions: `-1`, `0`, or `1`.
///
/// Mirrors `compare()`, which throws on invalid input in the original.
/// This returns `Err` instead of panicking, since Rust has no
/// exception model to mirror that behavior with.
///
/// ```
/// use semver_rs::compare;
///
/// assert_eq!(compare("1.2.3", "1.2.4", false), Ok(-1));
/// assert!(compare("not-a-version", "1.2.4", false).is_err());
/// ```
pub fn compare(a: &str, b: &str, options: impl Into<Options> + Copy) -> Result<i8> {
    let a = SemVer::parse(a, options)?;
    let b = SemVer::parse(b, options)?;
    Ok(match a.compare(&b) {
        std::cmp::Ordering::Less => -1,
        std::cmp::Ordering::Equal => 0,
        std::cmp::Ordering::Greater => 1,
    })
}

/// Reverse comparison. Mirrors `rcompare()`.
pub fn rcompare(a: &str, b: &str, options: impl Into<Options> + Copy) -> Result<i8> {
    compare(b, a, options)
}

/// `a > b`. Mirrors `gt()`.
pub fn gt(a: &str, b: &str, options: impl Into<Options> + Copy) -> Result<bool> {
    Ok(compare(a, b, options)? > 0)
}

/// `a < b`. Mirrors `lt()`.
pub fn lt(a: &str, b: &str, options: impl Into<Options> + Copy) -> Result<bool> {
    Ok(compare(a, b, options)? < 0)
}

/// `a == b` by precedence. Mirrors `eq()`.
pub fn eq(a: &str, b: &str, options: impl Into<Options> + Copy) -> Result<bool> {
    Ok(compare(a, b, options)? == 0)
}

/// `a != b`. Mirrors `neq()`.
pub fn neq(a: &str, b: &str, options: impl Into<Options> + Copy) -> Result<bool> {
    Ok(compare(a, b, options)? != 0)
}

/// `a >= b`. Mirrors `gte()`.
pub fn gte(a: &str, b: &str, options: impl Into<Options> + Copy) -> Result<bool> {
    Ok(compare(a, b, options)? >= 0)
}

/// `a <= b`. Mirrors `lte()`.
pub fn lte(a: &str, b: &str, options: impl Into<Options> + Copy) -> Result<bool> {
    Ok(compare(a, b, options)? <= 0)
}

/// Whether a version satisfies a range. Mirrors `satisfies()`.
///
/// ```
/// use semver_rs::satisfies;
///
/// assert!(satisfies("1.2.5", "^1.2.3", false));
/// assert!(!satisfies("2.0.0", "^1.2.3", false));
/// ```
pub fn satisfies(version: &str, range: &str, options: impl Into<Options> + Copy) -> bool {
    let Ok(r) = Range::parse(range, options) else {
        return false;
    };
    r.test_str(version)
}

/// A range's canonical form, or `None` if it does not parse.
/// Mirrors `validRange()`.
pub fn valid_range(range: &str, options: impl Into<Options> + Copy) -> Option<String> {
    Range::parse(range, options).ok().map(|r| r.to_string())
}

/// The highest version in `versions` that satisfies `range`.
/// Mirrors `maxSatisfying()`.
pub fn max_satisfying<'a>(
    versions: &[&'a str],
    range: &str,
    options: impl Into<Options> + Copy,
) -> Option<&'a str> {
    let r = Range::parse(range, options).ok()?;
    let mut best: Option<(&'a str, SemVer)> = None;
    for v in versions {
        let Ok(parsed) = SemVer::parse(v, options) else {
            continue;
        };
        if !r.test(&parsed) {
            continue;
        }
        match &best {
            Some((_, b)) if parsed.compare(b) != std::cmp::Ordering::Greater => {}
            _ => best = Some((v, parsed)),
        }
    }
    best.map(|(v, _)| v)
}

/// The lowest version in `versions` that satisfies `range`.
/// Mirrors `minSatisfying()`.
pub fn min_satisfying<'a>(
    versions: &[&'a str],
    range: &str,
    options: impl Into<Options> + Copy,
) -> Option<&'a str> {
    let r = Range::parse(range, options).ok()?;
    let mut best: Option<(&'a str, SemVer)> = None;
    for v in versions {
        let Ok(parsed) = SemVer::parse(v, options) else {
            continue;
        };
        if !r.test(&parsed) {
            continue;
        }
        match &best {
            Some((_, b)) if parsed.compare(b) != std::cmp::Ordering::Less => {}
            _ => best = Some((v, parsed)),
        }
    }
    best.map(|(v, _)| v)
}

/// Increment a version, returning the new version string.
///
/// Mirrors `inc()`, which returns `null` when the input version or the
/// release kind is invalid.
///
/// ```
/// use semver_rs::{inc, IdentifierBase};
///
/// assert_eq!(inc("1.2.3", "minor", None, IdentifierBase::Zero, false).as_deref(), Some("1.3.0"));
/// assert_eq!(
///     inc("1.2.3", "premajor", Some("beta"), IdentifierBase::Zero, false).as_deref(),
///     Some("2.0.0-beta.0")
/// );
/// ```
pub fn inc(
    version: &str,
    release: &str,
    identifier: Option<&str>,
    base: IdentifierBase,
    options: impl Into<Options> + Copy,
) -> Option<String> {
    let v = SemVer::parse(version, options).ok()?;
    let r = Release::parse(release).ok()?;
    v.inc(r, identifier, base).ok().map(|x| x.version())
}
#[cfg(test)]
mod public_api_tests {
    use super::*;

    #[test]
    fn valid_returns_canonical_form() {
        assert_eq!(valid("1.2.3", false), Some("1.2.3".to_string()));
        assert_eq!(valid("not-a-version", false), None);
    }

    #[test]
    fn rcompare_reverses_compare() {
        assert_eq!(rcompare("1.2.3", "1.2.4", false), Ok(1));
        assert_eq!(rcompare("1.2.4", "1.2.3", false), Ok(-1));
    }

    #[test]
    fn gt_lt_eq_neq_gte_lte_all_agree_with_compare() {
        assert_eq!(gt("2.0.0", "1.0.0", false), Ok(true));
        assert_eq!(gt("1.0.0", "2.0.0", false), Ok(false));
        assert_eq!(lt("1.0.0", "2.0.0", false), Ok(true));
        assert_eq!(lt("2.0.0", "1.0.0", false), Ok(false));
        assert_eq!(eq("1.0.0", "1.0.0", false), Ok(true));
        assert_eq!(eq("1.0.0", "2.0.0", false), Ok(false));
        assert_eq!(neq("1.0.0", "2.0.0", false), Ok(true));
        assert_eq!(neq("1.0.0", "1.0.0", false), Ok(false));
        assert_eq!(gte("1.0.0", "1.0.0", false), Ok(true));
        assert_eq!(gte("0.9.0", "1.0.0", false), Ok(false));
        assert_eq!(lte("1.0.0", "1.0.0", false), Ok(true));
        assert_eq!(lte("2.0.0", "1.0.0", false), Ok(false));
    }

    #[test]
    fn comparisons_error_on_invalid_input() {
        assert!(gt("garbage", "1.0.0", false).is_err());
        assert!(lt("1.0.0", "garbage", false).is_err());
    }

    #[test]
    fn valid_range_returns_canonical_form_or_none() {
        assert!(valid_range("^1.2.3", false).is_some());
        assert_eq!(valid_range("not a range !!", false), None);
    }

    #[test]
    fn max_satisfying_picks_the_highest_match() {
        let versions = ["1.0.0", "1.5.0", "2.0.0", "1.9.9"];
        assert_eq!(max_satisfying(&versions, "^1.0.0", false), Some("1.9.9"));
        assert_eq!(max_satisfying(&versions, ">=3.0.0", false), None);
    }

    #[test]
    fn min_satisfying_picks_the_lowest_match() {
        let versions = ["1.5.0", "1.0.0", "2.0.0", "1.9.9"];
        assert_eq!(min_satisfying(&versions, "^1.0.0", false), Some("1.0.0"));
        assert_eq!(min_satisfying(&versions, ">=3.0.0", false), None);
    }

    #[test]
    fn satisfies_matches_readme_examples() {
        assert!(satisfies("1.2.5", "^1.2.3", false));
        assert!(!satisfies("2.0.0", "^1.2.3", false));
        assert!(!satisfies("1.2.5", "not a range", false));
    }
}