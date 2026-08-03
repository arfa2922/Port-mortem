//! Functions built on top of parsing and comparison.
//!
//! Ported from the `functions/` directory of the original. These are
//! thin by design — the interesting behaviour lives in `semver.rs` and
//! `range.rs` — but several have edge cases worth naming.

use crate::constants::Options;
use crate::error::Result;
use crate::semver::SemVer;
use std::cmp::Ordering;

/// Pull a version out of arbitrary text.
///
/// Ported from `functions/coerce.js`. The original scans with a regex
/// that requires the match to be delimited by a non-digit on the left,
/// so `1.2.3` inside `v1.2.3-rc` coerces cleanly but the `2.3` inside
/// `1.2.3.4` does not become the answer.
///
/// Missing components become zero: `"42"` coerces to `42.0.0`.
pub fn coerce(text: &str, options: impl Into<Options>) -> Option<SemVer> {
    let options = options.into();
    let bytes = text.as_bytes();

    // Scan left to right for the first digit run that starts a
    // coercible version. A digit run only qualifies if what precedes it
    // is not itself a digit or a dot, matching the original's
    // `(^|[^\d])` guard.
    let mut i = 0;
    while i < bytes.len() {
        if !bytes[i].is_ascii_digit() {
            i += 1;
            continue;
        }
        if i > 0 && (bytes[i - 1].is_ascii_digit() || bytes[i - 1] == b'.') {
            i += 1;
            continue;
        }

        if let Some(v) = coerce_at(text, i, options) {
            return Some(v);
        }
        i += 1;
    }
    None
}

/// Attempt to read a coercible version starting exactly at `start`.
fn coerce_at(text: &str, start: usize, options: Options) -> Option<SemVer> {
    let bytes = text.as_bytes();
    let mut pos = start;

    // Up to three dot-separated numeric components, each capped at the
    // length the original allows.
    let mut components: Vec<u64> = Vec::with_capacity(3);
    loop {
        let run_start = pos;
        while pos < bytes.len() && bytes[pos].is_ascii_digit() {
            pos += 1;
        }
        if pos == run_start {
            break;
        }
        let raw = &text[run_start..pos];
        if raw.len() > crate::constants::MAX_SAFE_COMPONENT_LENGTH {
            return None;
        }
        let n: u64 = raw.parse().ok()?;
        if n > crate::constants::MAX_SAFE_INTEGER {
            return None;
        }
        components.push(n);

        if components.len() == 3 {
            break;
        }
        if pos < bytes.len() && bytes[pos] == b'.' {
            // A trailing dot with no digits after it ends the version.
            if pos + 1 >= bytes.len() || !bytes[pos + 1].is_ascii_digit() {
                break;
            }
            pos += 1;
        } else {
            break;
        }
    }

    if components.is_empty() {
        return None;
    }

    let major = components[0];
    let minor = components.get(1).copied().unwrap_or(0);
    let patch = components.get(2).copied().unwrap_or(0);

    let mut out = format!("{major}.{minor}.{patch}");

    // With includePrerelease, a prerelease and build tag immediately
    // following the numbers are carried through.
    if options.include_prerelease && pos < bytes.len() {
        let rest = &text[pos..];
        if let Some(tail) = coerce_tail(rest) {
            out.push_str(tail);
        }
    }

    SemVer::parse(&out, options).ok()
}

/// The prerelease and build portion of a coercible match, if the text
/// at this point begins one.
fn coerce_tail(rest: &str) -> Option<&str> {
    let bytes = rest.as_bytes();
    if bytes.is_empty() || (bytes[0] != b'-' && bytes[0] != b'+') {
        return None;
    }
    // Stop at the first character that cannot appear in a tag.
    let mut end = 1;
    while end < bytes.len() {
        let b = bytes[end];
        if b.is_ascii_alphanumeric() || b == b'-' || b == b'.' || b == b'+' {
            end += 1;
        } else {
            break;
        }
    }
    Some(&rest[..end])
}

/// The release kind separating two versions, or `None` when equal.
///
/// Ported from `functions/diff.js`. The prerelease-to-release case has
/// its own rules, which is most of the function: going from `1.0.0-1` to
/// `1.0.0` is a *major* difference, because the prerelease belongs to a
/// version that had not been released yet.
pub fn diff(a: &str, b: &str) -> Option<String> {
    let v1 = SemVer::parse(a, true).ok()?;
    let v2 = SemVer::parse(b, true).ok()?;

    let comparison = v1.compare(&v2);
    if comparison == Ordering::Equal {
        return None;
    }

    let v1_higher = comparison == Ordering::Greater;
    let high = if v1_higher { &v1 } else { &v2 };
    let low = if v1_higher { &v2 } else { &v1 };

    let high_has_pre = high.is_prerelease();
    let low_has_pre = low.is_prerelease();

    if low_has_pre && !high_has_pre {
        // A prerelease of x.0.0 becoming any release is a major step.
        if low.patch == 0 && low.minor == 0 {
            return Some("major".to_string());
        }
        // When the numeric parts match, the step is whichever component
        // the prerelease was attached to.
        if low.compare_main(high) == Ordering::Equal {
            if low.minor != 0 && low.patch == 0 {
                return Some("minor".to_string());
            }
            return Some("patch".to_string());
        }
    }

    let prefix = if high_has_pre { "pre" } else { "" };

    if v1.major != v2.major {
        return Some(format!("{prefix}major"));
    }
    if v1.minor != v2.minor {
        return Some(format!("{prefix}minor"));
    }
    if v1.patch != v2.patch {
        return Some(format!("{prefix}patch"));
    }

    // Only the prerelease differs.
    Some("prerelease".to_string())
}

/// Compare with an operator given as a string.
///
/// Ported from `functions/cmp.js`. `""`, `"="`, and `"=="` all mean
/// equality; `"!=="` and `"!="` mean inequality. The identity operators
/// `"==="` and `"!=="` compare the raw versions in the original, which
/// for parsed input is the same as `==` and `!=` here.
pub fn cmp(a: &str, op: &str, b: &str, options: impl Into<Options> + Copy) -> Result<bool> {
    let ord = crate::compare(a, b, options)?;
    Ok(match op {
        "" | "=" | "==" | "===" => ord == 0,
        "!=" | "!==" => ord != 0,
        ">" => ord > 0,
        ">=" => ord >= 0,
        "<" => ord < 0,
        "<=" => ord <= 0,
        _ => {
            return Err(crate::error::Error::InvalidIncrement {
                kind: format!("invalid operator: {op}"),
            })
        }
    })
}

/// Sort ascending by precedence, then by build metadata.
///
/// Ported from `functions/sort.js`, which sorts with `compareBuild` —
/// so two versions that are equal by precedence still order
/// deterministically by their build tags.
pub fn sort(versions: &mut [String], options: impl Into<Options> + Copy) {
    versions.sort_by(|a, b| build_aware_compare(a, b, options));
}

/// Sort descending.
pub fn rsort(versions: &mut [String], options: impl Into<Options> + Copy) {
    versions.sort_by(|a, b| build_aware_compare(b, a, options));
}

fn build_aware_compare(a: &str, b: &str, options: impl Into<Options> + Copy) -> Ordering {
    match (SemVer::parse(a, options), SemVer::parse(b, options)) {
        (Ok(x), Ok(y)) => x.compare_build(&y),
        // Unparseable versions sort last, and stably among themselves.
        (Ok(_), Err(_)) => Ordering::Less,
        (Err(_), Ok(_)) => Ordering::Greater,
        (Err(_), Err(_)) => a.cmp(b),
    }
}

/// The major component, or `None` if the version does not parse.
pub fn major(version: &str, options: impl Into<Options>) -> Option<u64> {
    SemVer::parse(version, options).ok().map(|v| v.major)
}

/// The minor component.
pub fn minor(version: &str, options: impl Into<Options>) -> Option<u64> {
    SemVer::parse(version, options).ok().map(|v| v.minor)
}

/// The patch component.
pub fn patch(version: &str, options: impl Into<Options>) -> Option<u64> {
    SemVer::parse(version, options).ok().map(|v| v.patch)
}

/// The prerelease identifiers, as strings.
///
/// Returns `None` for an unparseable version and an empty vector for a
/// version with no prerelease — the original returns `null` and `[]`
/// respectively, and the distinction matters to callers.
pub fn prerelease(version: &str, options: impl Into<Options>) -> Option<Vec<String>> {
    SemVer::parse(version, options)
        .ok()
        .map(|v| v.prerelease.iter().map(|i| i.to_string()).collect())
}

/// Strip whitespace and a leading `=` or `v`, then validate.
///
/// Ported from `functions/clean.js`.
pub fn clean(version: &str, options: impl Into<Options> + Copy) -> Option<String> {
    let trimmed = version.trim();
    let stripped = trimmed.trim_start_matches(['=', 'v']).trim();
    crate::valid(stripped, options)
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::*;

    #[test]
    fn coerce_pulls_a_version_out_of_text() {
        let o = Options::new();
        assert_eq!(coerce("v1.2.3", o).map(|v| v.version()), Some("1.2.3".into()));
        assert_eq!(coerce("42", o).map(|v| v.version()), Some("42.0.0".into()));
        assert_eq!(coerce("1.2", o).map(|v| v.version()), Some("1.2.0".into()));
        assert_eq!(coerce("not a version", o), None);
    }

    #[test]
    fn diff_reports_the_release_kind() {
        assert_eq!(diff("1.2.3", "1.2.4").as_deref(), Some("patch"));
        assert_eq!(diff("1.2.3", "1.3.0").as_deref(), Some("minor"));
        assert_eq!(diff("1.2.3", "2.0.0").as_deref(), Some("major"));
        assert_eq!(diff("1.2.3", "1.2.3"), None);
    }

    #[test]
    fn diff_treats_a_prerelease_of_a_major_as_major() {
        // The prerelease belongs to a version that was never released,
        // so reaching the release is a major step.
        assert_eq!(diff("1.0.0-1", "1.0.0").as_deref(), Some("major"));
    }

    #[test]
    fn cmp_accepts_operators_as_strings() {
        let o = Options::new();
        assert_eq!(cmp("1.2.3", ">", "1.2.2", o), Ok(true));
        assert_eq!(cmp("1.2.3", "=", "1.2.3", o), Ok(true));
        assert_eq!(cmp("1.2.3", "!=", "1.2.3", o), Ok(false));
    }

    /// Regression for the upstream bug in `simplifyRange`.
    ///
    /// The original returns "" when no version satisfies the range, and
    /// "" parses as `*`. A range matching nothing must not simplify to
    /// one matching everything.
    #[test]
    fn simplify_range_of_nothing_is_not_everything() {
        let o = Options::new();
        let versions = ["1.0.0", "2.0.0"];

        let simplified = simplify_range(&versions, "^5.0.0", o).unwrap();

        // Whatever we return, it must still match nothing.
        for v in versions {
            assert!(
                !crate::satisfies(v, &simplified, o),
                "{v} matches the simplified form of a range it did not match"
            );
        }
        assert_eq!(simplified, "<0.0.0-0");
    }

    #[test]
    fn sort_orders_by_precedence() {
        let mut v: Vec<String> = ["1.2.3", "1.0.0", "2.0.0", "1.2.3-alpha"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        sort(&mut v, Options::new());
        assert_eq!(v, vec!["1.0.0", "1.2.3-alpha", "1.2.3", "2.0.0"]);
    }

    #[test]
    fn rsort_orders_by_reverse_precedence() {
        let mut v: Vec<String> = ["1.2.3", "1.0.0", "2.0.0", "1.2.3-alpha"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        rsort(&mut v, Options::new());
        assert_eq!(v, vec!["2.0.0", "1.2.3", "1.2.3-alpha", "1.0.0"]);
    }

    #[test]
    fn major_minor_patch_extract_components() {
        let o = Options::new();
        assert_eq!(major("1.2.3", o), Some(1));
        assert_eq!(minor("1.2.3", o), Some(2));
        assert_eq!(patch("1.2.3", o), Some(3));
        assert_eq!(major("not-a-version", o), None);
    }

    #[test]
    fn prerelease_extracts_identifiers_or_empty_vec_or_none() {
        let o = Options::new();
        assert_eq!(
            prerelease("1.2.3-alpha.1", o),
            Some(vec!["alpha".to_string(), "1".to_string()])
        );
        // A valid version with no prerelease: empty vec, not None.
        assert_eq!(prerelease("1.2.3", o), Some(vec![]));
        // An unparseable version: None.
        assert_eq!(prerelease("not-a-version", o), None);
    }

    #[test]
    fn clean_strips_prefix_and_whitespace() {
        let o = Options::new();
        assert_eq!(clean("  =v1.2.3 ", o).as_deref(), Some("1.2.3"));
        assert_eq!(clean("v1.2.3", o).as_deref(), Some("1.2.3"));
        assert_eq!(clean("not a version", o), None);
    }

    #[test]
    fn truncate_zeroes_lower_precision_components() {
        let o = Options::new();
        assert_eq!(
            truncate("1.2.3-beta.1", "major", o).as_deref(),
            Some("1.0.0")
        );
        assert_eq!(
            truncate("1.2.3-beta.1", "minor", o).as_deref(),
            Some("1.2.0")
        );
        assert_eq!(
            truncate("1.2.3-beta.1", "patch", o).as_deref(),
            Some("1.2.3")
        );
        assert_eq!(
            truncate("1.2.3-beta.1", "prerelease", o).as_deref(),
            Some("1.2.3-beta.1")
        );
        assert_eq!(truncate("1.2.3", "not-a-release-type", o), None);
    }

    #[test]
    fn compare_loose_is_lenient_about_input() {
        assert_eq!(compare_loose("1.2.3", "1.2.4"), Ok(-1));
        assert_eq!(compare_loose("1.2.3", "1.2.3"), Ok(0));
    }

    #[test]
    fn compare_build_breaks_ties_on_build_metadata() {
        let o = Options::new();
        assert_eq!(compare_build("1.2.3+build.1", "1.2.3+build.2", o), Ok(-1));
        assert_eq!(compare_build("1.2.3", "1.2.3", o), Ok(0));
    }
}

/// Drop the components below a given precision.
///
/// Ported from `functions/truncate.js`. A `pre*` truncation is a no-op —
/// the version is already at prerelease precision — while `major`,
/// `minor`, and `patch` zero out everything below themselves and drop
/// the prerelease.
pub fn truncate(
    version: &str,
    truncation: &str,
    options: impl Into<Options> + Copy,
) -> Option<String> {
    if !crate::constants::RELEASE_TYPES.contains(&truncation) {
        return None;
    }

    let mut v = SemVer::parse(version, options).ok()?;

    // A prerelease truncation keeps the version as it stands.
    if truncation.starts_with("pre") {
        return Some(v.version());
    }

    v.prerelease.clear();
    match truncation {
        "major" => {
            v.minor = 0;
            v.patch = 0;
        }
        "minor" => {
            v.patch = 0;
        }
        // "patch" drops only the prerelease, which is already done.
        _ => {}
    }

    Some(v.version())
}

/// Compare in loose mode. Mirrors `compareLoose()`.
pub fn compare_loose(a: &str, b: &str) -> Result<i8> {
    crate::compare(a, b, true)
}

/// Compare including build metadata.
///
/// Mirrors `compareBuild()`. Precedence ignores build metadata, so this
/// is for sorting rather than for deciding which version is newer.
pub fn compare_build(a: &str, b: &str, options: impl Into<Options> + Copy) -> Result<i8> {
    let x = SemVer::parse(a, options)?;
    let y = SemVer::parse(b, options)?;
    Ok(match x.compare_build(&y) {
        Ordering::Less => -1,
        Ordering::Equal => 0,
        Ordering::Greater => 1,
    })
}

/// Rewrite a range as the shortest equivalent over a known version set.
///
/// Ported from `ranges/simplify.js`. Given every version that exists,
/// the range can be expressed as the contiguous runs it admits. The
/// original returns whichever of the two is shorter, so a range that is
/// already terse is left alone.
pub fn simplify_range(
    versions: &[&str],
    range: &str,
    options: impl Into<Options> + Copy,
) -> Option<String> {
    // Sort by precedence, dropping anything unparseable.
    let mut sorted: Vec<&str> = versions
        .iter()
        .filter(|v| SemVer::parse(v, options).is_ok())
        .copied()
        .collect();
    sorted.sort_by(|a, b| {
        let x = SemVer::parse(a, options).unwrap();
        let y = SemVer::parse(b, options).unwrap();
        x.compare(&y)
    });

    // Walk the sorted versions, collecting contiguous runs that satisfy.
    let mut runs: Vec<(String, Option<String>)> = Vec::new();
    let mut first: Option<&str> = None;
    let mut prev: Option<&str> = None;

    for v in &sorted {
        if crate::satisfies(v, range, options) {
            prev = Some(v);
            if first.is_none() {
                first = Some(v);
            }
        } else if let Some(p) = prev {
            runs.push((first.unwrap().to_string(), Some(p.to_string())));
            prev = None;
            first = None;
        }
    }
    if let Some(f) = first {
        runs.push((f.to_string(), None));
    }

    let lowest = sorted.first().copied().unwrap_or("");

    let parts: Vec<String> = runs
        .iter()
        .map(|(min, max)| match max {
            Some(mx) if mx == min => min.clone(),
            // Open-ended from the lowest version there is: everything.
            None if min == lowest => "*".to_string(),
            None => format!(">={min}"),
            Some(mx) if min == lowest => format!("<={mx}"),
            Some(mx) => format!("{min} - {mx}"),
        })
        .collect();

    let simplified = parts.join(" || ");

    // UPSTREAM BUG, deliberately not reproduced.
    //
    // When nothing satisfies the range, `parts` is empty and the join
    // yields "". In semver an empty range means `*` — everything — so
    // the original turns a range that matched nothing into one that
    // matches everything, and the length check below always prefers it
    // because 0 < any length.
    //
    //     semver.simplifyRange(['1.0.0'], '^5.0.0')  //  ''
    //     semver.satisfies('1.0.0', '')              //  true
    //
    // We return the canonical empty range instead. See UPSTREAM_BUG.md.
    if simplified.is_empty() {
        return Some("<0.0.0-0".to_string());
    }

    // Only worth returning if it is actually shorter.
    if simplified.len() < range.len() {
        Some(simplified)
    } else {
        Some(range.to_string())
    }
}