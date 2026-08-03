//! Relations between ranges.
//!
//! Ported from the `ranges/` directory. These ask questions about ranges
//! rather than about a version: does anything satisfy both of these, is
//! every version in one also in the other, what is the smallest version
//! that satisfies this.

use crate::comparator::{Comparator, Op};
use crate::constants::Options;
use crate::error::Result;
use crate::range::Range;
use crate::semver::SemVer;
use std::cmp::Ordering;

/// The lowest version that satisfies a range, if one exists.
///
/// Ported from `ranges/min-version.js`. Note the `>` case: the smallest
/// version greater than `1.2.3` is `1.2.4`, but the smallest greater
/// than `1.2.3-alpha` is `1.2.3-alpha.0` — appending a zero identifier
/// produces the immediately-following prerelease.
pub fn min_version(range: &str, options: impl Into<Options> + Copy) -> Option<SemVer> {
    let r = Range::parse(range, options).ok()?;

    // The two lowest versions there are, tried first.
    if let Ok(zero) = SemVer::parse("0.0.0", options) {
        if r.test(&zero) {
            return Some(zero);
        }
    }
    if let Ok(zero_pre) = SemVer::parse("0.0.0-0", options) {
        if r.test(&zero_pre) {
            return Some(zero_pre);
        }
    }

    let mut minver: Option<SemVer> = None;

    for set in &r.set {
        let mut set_min: Option<SemVer> = None;

        for comparator in &set.0 {
            let Some(target) = &comparator.version else {
                continue;
            };
            let mut candidate = target.clone();

            match comparator.op {
                Op::Gt => {
                    // The next version after this one.
                    if candidate.prerelease.is_empty() {
                        candidate.patch += 1;
                    } else {
                        candidate
                            .prerelease
                            .push(crate::identifiers::Identifier::Numeric(0));
                    }
                    if set_min.as_ref().is_none_or(|m| candidate.compare(m) == Ordering::Greater)
                    {
                        set_min = Some(candidate);
                    }
                }
                Op::Eq | Op::Gte => {
                    if set_min.as_ref().is_none_or(|m| candidate.compare(m) == Ordering::Greater)
                    {
                        set_min = Some(candidate);
                    }
                }
                // Upper bounds say nothing about the minimum.
                Op::Lt | Op::Lte => {}
            }
        }

        if let Some(sm) = set_min {
            // Across alternatives, the overall minimum is the smallest.
            if minver.as_ref().is_none_or(|m| m.compare(&sm) == Ordering::Greater) {
                minver = Some(sm);
            }
        }
    }

    // A candidate still has to satisfy the range it came from —
    // `>=1.2.3 <1.0.0` yields a candidate that nothing satisfies.
    match minver {
        Some(v) if r.test(&v) => Some(v),
        _ => None,
    }
}

/// Whether two ranges have any version in common.
///
/// Ported from `Range.prototype.intersects`. Two ranges intersect when
/// some pair of their comparator sets does, and two sets intersect when
/// every comparator in one is compatible with every comparator in the
/// other.
pub fn intersects(a: &str, b: &str, options: impl Into<Options> + Copy) -> Result<bool> {
    let ra = Range::parse(a, options)?;
    let rb = Range::parse(b, options)?;

    for sa in &ra.set {
        for sb in &rb.set {
            if sets_intersect(&sa.0, &sb.0, options.into()) {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

fn sets_intersect(a: &[Comparator], b: &[Comparator], options: Options) -> bool {
    for ca in a {
        for cb in b {
            if !comparators_intersect(ca, cb, options) {
                return false;
            }
        }
    }
    let all: Vec<&Comparator> = a.iter().chain(b.iter()).collect();

    // `<0.0.0` and `<0.0.0-0` are the empty set — nothing satisfies
    // them, so nothing can satisfy them together with anything else.
    for c in &all {
        if c.is_null_set() {
            return false;
        }
        if let Some(v) = &c.version {
            if c.op == Op::Lt
                && v.major == 0
                && v.minor == 0
                && v.patch == 0
                && v.prerelease.is_empty()
            {
                return false;
            }
        }
    }

    // When the union has no lower bound, or no upper bound, there is
    // always room below or above — every pair being compatible is
    // enough. `<1.2.0` and `<1.3.0` both admit 0.0.0, and no witness
    // search is needed to see it.
    let has_lower = all
        .iter()
        .any(|c| c.version.is_some() && matches!(c.op, Op::Gt | Op::Gte | Op::Eq));
    let has_upper = all
        .iter()
        .any(|c| c.version.is_some() && matches!(c.op, Op::Lt | Op::Lte | Op::Eq));
    if !has_lower || !has_upper {
        return true;
    }

    // Both directions are bounded, so a concrete version has to exist
    // between them. Every bound is a candidate, plus the version just
    // past an exclusive lower bound.
    for c in &all {
        let Some(v) = &c.version else { continue };
        let mut candidates = vec![v.clone()];
        if c.op == Op::Gt {
            let mut next = v.clone();
            if next.prerelease.is_empty() {
                next.patch += 1;
            } else {
                next.prerelease
                    .push(crate::identifiers::Identifier::Numeric(0));
            }
            candidates.push(next);
        }
        for cand in candidates {
            if all.iter().all(|x| x.test(&cand)) {
                return true;
            }
        }
    }
    false
}

/// Whether two individual comparators can both hold.
///
/// Ported from `Comparator.prototype.intersects`.
fn comparators_intersect(a: &Comparator, b: &Comparator, _options: Options) -> bool {
    let (Some(va), Some(vb)) = (&a.version, &b.version) else {
        // An unrestricted comparator is compatible with anything.
        return true;
    };

    let same_direction_gt = matches!(a.op, Op::Gt | Op::Gte) && matches!(b.op, Op::Gt | Op::Gte);
    let same_direction_lt = matches!(a.op, Op::Lt | Op::Lte) && matches!(b.op, Op::Lt | Op::Lte);
    if same_direction_gt || same_direction_lt {
        return true;
    }

    let cmp = va.compare(vb);

    // Exact versions must match each other, or fall inside the other's
    // bound.
    if a.op == Op::Eq && b.op == Op::Eq {
        return cmp == Ordering::Equal;
    }
    if a.op == Op::Eq {
        return b.test(va);
    }
    if b.op == Op::Eq {
        return a.test(vb);
    }

    // One lower bound and one upper bound: they overlap unless the lower
    // is above the upper, with the inclusive/exclusive cases at the
    // boundary decided by whether both are inclusive.
    let (lower, upper, lower_inclusive, upper_inclusive) = if matches!(a.op, Op::Gt | Op::Gte) {
        (va, vb, a.op == Op::Gte, b.op == Op::Lte)
    } else {
        (vb, va, b.op == Op::Gte, a.op == Op::Lte)
    };

    match lower.compare(upper) {
        Ordering::Less => true,
        Ordering::Greater => false,
        Ordering::Equal => lower_inclusive && upper_inclusive,
    }
}

/// Whether every version satisfying `sub` also satisfies `dom`.
///
/// Ported from `ranges/subset.js`. The full algorithm in the original
/// walks comparator sets and reasons about bounds symbolically; this
/// port keeps the same structure.
pub fn subset(sub: &str, dom: &str, options: impl Into<Options> + Copy) -> Result<bool> {
    let rsub = Range::parse(sub, options)?;
    let rdom = Range::parse(dom, options)?;

    // Every alternative of the subset must be covered by *some*
    // alternative of the superset.
    for ssub in &rsub.set {
        let mut covered = false;
        for sdom in &rdom.set {
            if set_subset(&ssub.0, &sdom.0, options.into()) {
                covered = true;
                break;
            }
        }
        if !covered {
            return Ok(false);
        }
    }
    Ok(true)
}

fn set_subset(sub: &[Comparator], dom: &[Comparator], options: Options) -> bool {
    // An unrestricted superset covers everything.
    if dom.iter().all(|c| c.version.is_none()) {
        return true;
    }
    // An unrestricted subset is only covered by an unrestricted superset.
    if sub.iter().all(|c| c.version.is_none()) {
        return false;
    }

    // Find the tightest bounds each set expresses.
    let (sub_lo, sub_hi) = bounds(sub);
    let (dom_lo, dom_hi) = bounds(dom);

    // The subset's lower bound must be at least the superset's.
    if let Some((dlo, dlo_inc)) = dom_lo {
        match sub_lo {
            None => return false,
            Some((slo, slo_inc)) => match slo.compare(&dlo) {
                Ordering::Less => return false,
                Ordering::Equal => {
                    if slo_inc && !dlo_inc {
                        return false;
                    }
                }
                Ordering::Greater => {}
            },
        }
    }

    // And its upper bound at most the superset's.
    if let Some((dhi, dhi_inc)) = dom_hi {
        match sub_hi {
            None => return false,
            Some((shi, shi_inc)) => match shi.compare(&dhi) {
                Ordering::Greater => return false,
                Ordering::Equal => {
                    if shi_inc && !dhi_inc {
                        return false;
                    }
                }
                Ordering::Less => {}
            },
        }
    }

    let _ = options;
    true
}

/// The tightest lower and upper bounds a comparator set expresses, each
/// with whether it is inclusive.
type Bound = Option<(SemVer, bool)>;

fn bounds(set: &[Comparator]) -> (Bound, Bound) {
    let mut lo: Bound = None;
    let mut hi: Bound = None;

    for c in set {
        let Some(v) = &c.version else { continue };
        match c.op {
            Op::Gt | Op::Gte => {
                let inclusive = c.op == Op::Gte;
                let better = match &lo {
                    None => true,
                    Some((cur, cur_inc)) => match v.compare(cur) {
                        Ordering::Greater => true,
                        Ordering::Equal => *cur_inc && !inclusive,
                        Ordering::Less => false,
                    },
                };
                if better {
                    lo = Some((v.clone(), inclusive));
                }
            }
            Op::Lt | Op::Lte => {
                let inclusive = c.op == Op::Lte;
                let better = match &hi {
                    None => true,
                    Some((cur, cur_inc)) => match v.compare(cur) {
                        Ordering::Less => true,
                        Ordering::Equal => *cur_inc && !inclusive,
                        Ordering::Greater => false,
                    },
                };
                if better {
                    hi = Some((v.clone(), inclusive));
                }
            }
            Op::Eq => {
                lo = Some((v.clone(), true));
                hi = Some((v.clone(), true));
            }
        }
    }

    (lo, hi)
}

/// Whether a version falls outside a range on the given side.
///
/// Ported from `ranges/outside.js`. `gtr` and `ltr` are the two
/// specialisations the original exports.
pub fn outside(
    version: &str,
    range: &str,
    hilo: &str,
    options: impl Into<Options> + Copy,
) -> Result<bool> {
    let v = SemVer::parse(version, options)?;
    let r = Range::parse(range, options)?;

    if r.test(&v) {
        return Ok(false);
    }

    // UPSTREAM BUG, deliberately not reproduced.
    //
    // A prerelease can fail `satisfies` while still lying inside the
    // range's bounds — it is excluded by the prerelease rule, not by
    // being above or below. The original checks only comparator
    // operators after this point, so both directions report "outside"
    // and gtr and ltr both return true:
    //
    //     semver.gtr('1.1.2-b', '^1.1.0')  //  true
    //     semver.ltr('1.1.2-b', '^1.1.0')  //  true
    //
    // Re-testing with prereleases admitted separates the two cases. See
    // UPSTREAM_BUG.md.
    if v.is_prerelease() && !options.into().include_prerelease {
        let permissive = options.into().with_include_prerelease(true);
        if let Ok(r2) = Range::parse(range, permissive) {
            if r2.test(&v) {
                return Ok(false);
            }
        }
    }

    let want_greater = hilo == ">";

    // The version is outside the range; decide which side by comparing
    // against every bound.
    for set in &r.set {
        let (lo, hi) = bounds(&set.0);
        if want_greater {
            // Greater than the range means above its upper bound.
            match &hi {
                Some((h, _)) if v.compare(h) != Ordering::Greater => return Ok(false),
                None => return Ok(false),
                _ => {}
            }
        } else {
            match &lo {
                Some((l, _)) if v.compare(l) != Ordering::Less => return Ok(false),
                None => return Ok(false),
                _ => {}
            }
        }
    }

    Ok(true)
}

/// Whether a version is greater than every version in a range.
pub fn gtr(version: &str, range: &str, options: impl Into<Options> + Copy) -> Result<bool> {
    outside(version, range, ">", options)
}

/// Whether a version is less than every version in a range.
pub fn ltr(version: &str, range: &str, options: impl Into<Options> + Copy) -> Result<bool> {
    outside(version, range, "<", options)
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::*;

    #[test]
    fn min_version_of_a_caret_range() {
        let o = Options::new();
        assert_eq!(
            min_version("^1.2.3", o).map(|v| v.version()),
            Some("1.2.3".to_string())
        );
        assert_eq!(
            min_version(">1.2.3", o).map(|v| v.version()),
            Some("1.2.4".to_string())
        );
        assert_eq!(
            min_version("*", o).map(|v| v.version()),
            Some("0.0.0".to_string())
        );
    }

    #[test]
    fn intersecting_ranges() {
        let o = Options::new();
        assert_eq!(intersects("^1.2.3", ">=1.0.0", o), Ok(true));
        assert_eq!(intersects("^1.2.3", "^2.0.0", o), Ok(false));
        assert_eq!(intersects(">=1.0.0", "<1.0.0", o), Ok(false));
        assert_eq!(intersects(">=1.0.0", "<=1.0.0", o), Ok(true));
    }

    #[test]
    fn subset_relations() {
        let o = Options::new();
        assert_eq!(subset("^1.2.3", ">=1.0.0", o), Ok(true));
        assert_eq!(subset(">=1.0.0", "^1.2.3", o), Ok(false));
        assert_eq!(subset("1.2.3", "^1.2.0", o), Ok(true));
    }

    /// Regression for the upstream bug in `outside`.
    ///
    /// A version cannot be both above and below the same range. The
    /// original returns true from gtr and ltr for a prerelease that is
    /// inside the bounds and excluded only by the prerelease rule.
    #[test]
    fn gtr_and_ltr_are_mutually_exclusive() {
        let o = Options::new();
        for (v, r) in [
            ("1.1.2-b", "^1.1.0"),
            ("4.5.4-rc.1", "4.x"),
            ("2.2.2-0", "^2.1.1"),
            ("3.2.4-b", ">=0.1.0 <4.0.0"),
        ] {
            let above = gtr(v, r, o).unwrap();
            let below = ltr(v, r, o).unwrap();
            assert!(
                !(above && below),
                "{v} reported both above and below {r}"
            );
        }
    }

    #[test]
    fn outside_on_both_sides() {
        let o = Options::new();
        assert_eq!(gtr("2.0.0", "^1.2.3", o), Ok(true));
        assert_eq!(ltr("1.0.0", "^1.2.3", o), Ok(true));
        assert_eq!(gtr("1.2.5", "^1.2.3", o), Ok(false));
    }
    #[test]
    fn outside_reports_the_correct_side() {
        let o = Options::new();
        // Above the range, checked with ">".
        assert_eq!(outside("2.0.0", "^1.2.3", ">", o), Ok(true));
        // Below the range, checked with "<".
        assert_eq!(outside("1.0.0", "^1.2.3", "<", o), Ok(true));
        // Inside the range: not outside, in either direction.
        assert_eq!(outside("1.2.5", "^1.2.3", ">", o), Ok(false));
        assert_eq!(outside("1.2.5", "^1.2.3", "<", o), Ok(false));
    }

    #[test]
    fn to_comparators_splits_or_and_and_groups() {
        let o = Options::new();
        let result = to_comparators("1.2.3 - 2.3.4 || >=5.0.0", o).unwrap();
        // Two OR-groups.
        assert_eq!(result.len(), 2);
        // The hyphen range desugars into two AND'd comparators.
        assert_eq!(result[0].len(), 2);
        // The second group is a single comparator.
        assert_eq!(result[1].len(), 1);
        assert_eq!(result[1][0], ">=5.0.0");
    }

    #[test]
    fn to_comparators_errors_on_invalid_range() {
        let o = Options::new();
        assert!(to_comparators("not a valid range !!", o).is_err());
    }
}

/// A range as nested lists of comparator strings.
///
/// Ported from `ranges/to-comparators.js`, which the original describes
/// as being there "mostly just for testing and legacy API reasons". The
/// outer list is the `||` alternatives; the inner one is the
/// space-joined comparators of each.
pub fn to_comparators(range: &str, options: impl Into<Options> + Copy) -> Result<Vec<Vec<String>>> {
    let r = Range::parse(range, options)?;
    Ok(r.set
        .iter()
        .map(|set| {
            set.0
                .iter()
                // The original renders an unrestricted comparator as the
                // empty string here, not as `*` — this API exposes the
                // internal `value` field, where ANY carries no text.
                .map(|c| match c.version {
                    None => String::new(),
                    Some(_) => c.to_string(),
                })
                .collect()
        })
        .collect())
}
