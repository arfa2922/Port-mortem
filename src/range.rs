//! Ranges — a set of comparator sets joined by `||`.
//!
//! Ported from `classes/range.js`. The original rewrites the range text
//! through a chain of regex replacements (hyphen, then caret, tilde,
//! X-range, star) and re-parses the result. This port performs the same
//! rewrites on parsed components, which keeps the desugaring rules
//! visible instead of buried in capture-group indices.
//!
//! The rules themselves are reproduced exactly, including the parts that
//! look arbitrary — `^0.2.3` and `^1.2.3` desugar differently because
//! major zero is treated as unstable, and every upper bound ends in `-0`
//! so that prereleases of the excluded version are excluded too.

use crate::comparator::{Comparator, Op};
use crate::constants::Options;
use crate::error::{Error, Result};
use crate::semver::SemVer;

/// A partial version, as written in a range.
///
/// `1.2.x`, `1.2`, and `1.2.*` all parse to major 1, minor 2, patch
/// `None`. The original calls these X-ranges and represents the wildcard
/// with the literal strings `x`, `X`, or `*`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Partial {
    major: Option<u64>,
    minor: Option<u64>,
    patch: Option<u64>,
}

impl Partial {
    fn is_any(&self) -> bool {
        self.major.is_none()
    }
}

/// Parse a partial version plus optional prerelease and build.
///
/// Returns `None` when the text is not a partial version at all.
/// Parse a partial version, honouring loose mode.
///
/// Loose mode accepts two spellings the strict grammar rejects: a
/// prerelease with no separating hyphen (`1.2.3beta`) and a component
/// with a leading zero (`09090`).
fn parse_partial_with(s: &str, loose: bool) -> Option<(Partial, Option<String>, Option<String>)> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let s = if loose {
        s.trim_start_matches(['=', 'v']).trim()
    } else {
        s.strip_prefix('v').unwrap_or(s)
    };

    // Split off build metadata, then prerelease. The build tag never
    // affects a bound, so it is parsed and discarded.
    let (head, build) = match s.split_once('+') {
        Some((h, b)) => (h, Some(b.to_string())),
        None => (s, None),
    };

    // A prerelease needs a fully-written version to attach to. When the
    // text before the hyphen has no `.` at all — `09090-0` — there is no
    // reading: the original's X-range pattern has no prerelease group
    // for a bare number, so it matches nothing and the range is invalid.
    if let Some(i) = head.find('-') {
        if i > 0 && !head[..i].contains('.') {
            return None;
        }
    }

    // A leading `-` cannot start a prerelease; find one after the numbers.
    let (numbers, prerelease) = match head.find('-') {
        Some(i) if i > 0 => (&head[..i], Some(head[i + 1..].to_string())),
        _ => (head, None),
    };

    // Loose mode also allows the hyphen to be omitted entirely
    // (`1.2.3beta`). Strict mode must reject that spelling outright
    // rather than silently reading it as a prerelease.
    let (numbers, prerelease) = match (loose, &prerelease) {
        (true, None) => split_loose_prerelease(numbers),
        (false, None) => {
            // A letter directly after a digit is not valid strictly.
            if numbers
                .as_bytes()
                .windows(2)
                .any(|w| w[0].is_ascii_digit() && w[1].is_ascii_alphabetic())
            {
                return None;
            }
            (numbers, prerelease)
        }
        _ => (numbers, prerelease),
    };

    let mut parts = numbers.split('.');

    let component = |t: Option<&str>| -> Option<Option<u64>> {
        match t {
            None => Some(None),
            Some("x" | "X" | "*" | "") => Some(None),
            // A leading zero is not a legal component in strict mode:
            // `>=09090` is invalid there, but means 9090 when loose.
            Some(t) if !loose && t.len() > 1 && t.starts_with('0') => None,
            Some(t) => match t.parse::<u64>() {
                // Beyond the safe integer bound the original cannot hold
                // the value, so the whole range is invalid.
                Ok(n) if n <= crate::constants::MAX_SAFE_INTEGER => Some(Some(n)),
                _ => None,
            },
        }
    };

    let major = component(parts.next())?;
    let minor = component(parts.next())?;
    let patch = component(parts.next())?;

    // More than three components is not a partial version.
    if parts.next().is_some() {
        return None;
    }

    // A wildcard in an earlier position forbids a concrete later one:
    // `1.x.5` and `x.1` are invalid, while `1.x` and `x` are fine.
    if major.is_none() && (minor.is_some() || patch.is_some()) {
        return None;
    }
    if minor.is_none() && patch.is_some() {
        return None;
    }

    // A prerelease needs somewhere to attach. `1.x.x-alpha` is fine —
    // the wildcard spans it and it is discarded — but `09090-0`, where
    // no minor or patch was written at all, has no reading: the
    // original's X-range pattern has no prerelease group for that shape.
    let wrote_minor = numbers.matches('.').count() >= 1;
    if prerelease.is_some() && !wrote_minor {
        return None;
    }

    Some((
        Partial {
            major,
            minor,
            patch,
        },
        prerelease,
        build,
    ))
}

/// Render a lower bound, appending the prerelease when one was written.
/// A version written inside a range may carry a prerelease with no
/// separating hyphen in loose mode: `1.2.3beta`. Split that off.
fn split_loose_prerelease(s: &str) -> (&str, Option<String>) {
    let bytes = s.as_bytes();
    for (i, b) in bytes.iter().enumerate() {
        if b.is_ascii_alphabetic() && i > 0 {
            // Only when the preceding run is numeric — `x` and `beta`
            // starting a component are handled elsewhere.
            if bytes[i - 1].is_ascii_digit() {
                return (&s[..i], Some(s[i..].to_string()));
            }
        }
    }
    (s, None)
}

/// Desugar `^`.
///
/// Compatible-with: allows changes that do not modify the left-most
/// non-zero component. Major zero is special — `^0.2.3` allows patch
/// releases only, because a zero major means the API is not yet stable.
/// A zero-width marker appended to a caret/tilde desugar's lower bound
/// when the source had a leading-zero major (`^00`, `~00`). It survives
/// whitespace-splitting (it's not whitespace) but is stripped before the
/// text reaches `Comparator::parse`, so its only effect is to make the
/// bound distinguishable from an ordinary `>=0.0.0` at the one place
/// that decides whether to absorb it into ANY.
const KEEP_ZERO_MARKER: &str = "\u{200b}";

/// Whether the first (major) component in a partial-version string was
/// written with a leading zero, e.g. `"00"` in `^00` or `~00.1`.
///
/// This only matters for zero itself: `01` and `00` are both invalid in
/// strict mode, and in loose mode both coerce to a number -- but `^00`
/// and `^0` are observably different range outputs even though they
/// parse to the same major. Checking the raw text here, before any
/// coercion, is the only way to keep that distinction.
fn major_component_has_leading_zero(body: &str) -> bool {
    let digits: &str = body
        .split(['.', '-', '+'])
        .next()
        .unwrap_or("")
        .trim_start_matches(['v', 'V']);
    digits.len() > 1 && digits.starts_with('0') && digits.bytes().all(|b| b.is_ascii_digit())
}

fn replace_caret(text: &str, options: Options) -> Option<String> {
    let body = text.strip_prefix('^')?;
    // `^=1.2.3` and `^==1.2.3` are accepted, any number of `=`. Found by
    // differential fuzzing -- undocumented, but the original strips a
    // leading `=` after `^` before reading the version.
    let body = body.trim_start_matches('=');
    let (p, pre, _build) = parse_partial_with(body, options.loose)?;
    let z = if options.include_prerelease { "-0" } else { "" };

    let Some(major) = p.major else {
        return Some(String::new()); // `^x` means anything
    };

    // `^00` keeps its `>=0.0.0` lower bound explicit; `^0` doesn't --
    // even though both parse to major 0, only the leading-zero
    // spelling is kept. This mirrors the original: the leading zero
    // takes a different branch of its regex than a bare `0` does, and
    // that branch doesn't get simplified away downstream the way the
    // bare-zero one does. `KEEP_ZERO` marks the bound here, at the
    // only point that still knows the raw text, and the marker is
    // stripped in parse_comparator_set once the absorption check has
    // used it. Found by differential fuzzing.
    let major_had_leading_zero = major_component_has_leading_zero(body);
    let zero_marker = if major == 0 && major_had_leading_zero {
        KEEP_ZERO_MARKER
    } else {
        ""
    };

    Some(match (p.minor, p.patch) {
        (None, _) => format!(">={major}.0.0{z}{zero_marker} <{}.0.0-0", major + 1),
        (Some(minor), None) => {
            if major == 0 {
                format!(">={major}.{minor}.0{z}{zero_marker} <{major}.{}.0-0", minor + 1)
            } else {
                format!(">={major}.{minor}.0{z} <{}.0.0-0", major + 1)
            }
        }
        (Some(minor), Some(patch)) => {
            let lo = match pre.as_deref() {
                Some(p) => format!(">={major}.{minor}.{patch}-{p}"),
                None => format!(">={major}.{minor}.{patch}"),
            };
            if major == 0 {
                if minor == 0 {
                    format!("{lo} <{major}.{minor}.{}-0", patch + 1)
                } else {
                    format!("{lo} <{major}.{}.0-0", minor + 1)
                }
            } else {
                format!("{lo} <{}.0.0-0", major + 1)
            }
        }
    })
}

/// Desugar `~`.
///
/// Approximately-equivalent: allows patch-level changes when a minor is
/// given, minor-level changes when it is not.
fn replace_tilde(text: &str, options: Options) -> Option<String> {
    let body = text.strip_prefix('~')?;
    // `~>1.2.3` is accepted as a synonym for `~1.2.3`.
    let body = body.strip_prefix('>').unwrap_or(body);
    // Same `=`-stripping as caret: `~=1.2.3` is accepted.
    let body = body.trim_start_matches('=');
    let (p, pre, _build) = parse_partial_with(body, options.loose)?;
    let z = if options.include_prerelease { "-0" } else { "" };

    let Some(major) = p.major else {
        return Some(String::new());
    };

    // Same leading-zero asymmetry as caret: `~00` keeps `>=0.0.0`
    // explicit, `~0` doesn't. See KEEP_ZERO_MARKER.
    let zero_marker = if major == 0 && major_component_has_leading_zero(body) {
        KEEP_ZERO_MARKER
    } else {
        ""
    };

    Some(match (p.minor, p.patch) {
        (None, _) => format!(">={major}.0.0{z}{zero_marker} <{}.0.0-0", major + 1),
        (Some(minor), None) => {
            format!(">={major}.{minor}.0{z}{zero_marker} <{major}.{}.0-0", minor + 1)
        }
        (Some(minor), Some(patch)) => {
            // A fully-specified version is its own lower bound; the `-0`
            // marker applies only where a component was omitted.
            let lo = match pre.as_deref() {
                Some(p) => format!(">={major}.{minor}.{patch}-{p}"),
                None => format!(">={major}.{minor}.{patch}"),
            };
            format!("{lo} <{major}.{}.0-0", minor + 1)
        }
    })
}

/// Desugar an X-range: `1.2.x`, `1.x`, `*`, and the same with an
/// operator in front.
fn replace_xrange(text: &str, options: Options) -> Option<String> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }

    let (op, rest) = for_xrange_split(text);
    // If the remainder is not a partial version, this is not an X-range
    // and the caller must report the failure — returning `*` here would
    // silently turn an invalid comparator into "match anything".
    let (p, pre, _build) = parse_partial_with(rest, options.loose)?;

    // Nothing to do when every component is present.
    if p.major.is_some() && p.minor.is_some() && p.patch.is_some() {
        return None;
    }

    let z = if options.include_prerelease { "-0" } else { "" };
    let x_major = p.major.is_none();
    let x_minor = x_major || p.minor.is_none();

    if x_major {
        // `>x` and `<x` can never be satisfied; anything else is `*`.
        return Some(if matches!(op, Some(Op::Gt) | Some(Op::Lt)) {
            "<0.0.0-0".to_string()
        } else {
            "*".to_string()
        });
    }

    let major = p.major.unwrap();
    let minor = p.minor.unwrap_or(0);

    Some(match op {
        Some(Op::Gt) => {
            // `>1` means `>=2.0.0`; `>1.2` means `>=1.3.0`.
            if x_minor {
                format!(">={}.0.0{z}", major + 1)
            } else {
                format!(">={major}.{}.0{z}", minor + 1)
            }
        }
        Some(Op::Lte) => {
            // `<=1.2.x` is `<1.3.0`, since any 1.2.x should pass.
            if x_minor {
                format!("<{}.0.0-0", major + 1)
            } else {
                format!("<{major}.{}.0-0", minor + 1)
            }
        }
        Some(Op::Gte) => format!(">={major}.{minor}.0{z}"),
        Some(Op::Lt) => format!("<{major}.{minor}.0-0"),
        // No operator: a bare X-range becomes a two-sided bound. A
        // prerelease written alongside a wildcard is discarded — the
        // wildcard already spans it.
        None | Some(Op::Eq) => {
            let _ = &pre;
            // Same leading-zero asymmetry as caret/tilde: `00.x` keeps
            // `>=0.0.0` explicit, `0.x` doesn't. See KEEP_ZERO_MARKER.
            let zero_marker = if major == 0 && major_component_has_leading_zero(rest) {
                KEEP_ZERO_MARKER
            } else {
                ""
            };
            let lo = format!(">={major}.{minor}.0{z}{zero_marker}");
            if x_minor {
                format!("{lo} <{}.0.0-0", major + 1)
            } else {
                format!("{lo} <{major}.{}.0-0", minor + 1)
            }
        }
    })
}

fn for_xrange_split(s: &str) -> (Option<Op>, &str) {
    for (lit, op) in [
        ("<=", Op::Lte),
        (">=", Op::Gte),
        ("<", Op::Lt),
        (">", Op::Gt),
        ("=", Op::Eq),
    ] {
        if let Some(rest) = s.strip_prefix(lit) {
            return (Some(op), rest.trim());
        }
    }
    (None, s)
}

/// Desugar a hyphen range: `1.2.3 - 2.3.4`.
///
/// Both ends may be partial, and each end has its own rule for what a
/// missing component means — the lower bound rounds down, the upper
/// bound rounds up to the next release and excludes it.
fn replace_hyphen(text: &str, options: Options) -> Option<String> {
    // The pattern is anchored, so the whole set must be exactly
    // `<from> - <to>` and nothing else.
    let tokens: Vec<&str> = text.split_whitespace().collect();
    if tokens.len() != 3 || tokens[1] != "-" {
        return None;
    }

    // Split on ` - ` with surrounding whitespace, not on any hyphen —
    // prerelease tags contain hyphens too.
    let idx = find_hyphen_separator(text)?;
    let (from_text, to_text) = text.split_at(idx);
    let to_text = to_text.trim_start_matches(|c: char| c.is_whitespace() || c == '-');

    // The left side of a hyphen range never accepts a leading `=` --
    // '=1.2.3 - 9.1.17' is null in every mode tried. The right side is
    // narrower still than it first looked: `=` is accepted there only
    // when that version carries a prerelease tag.
    //
    //     '1.2.3 - =9.1.17'    -> null                    (no prerelease: rejected)
    //     '1.2.3 - =9.1.17-a'  -> '>=1.2.3 <=9.1.17-a'     (has one: accepted)
    //
    // This reads like the original's hyphen-range pattern embeds the
    // `=` only inside the branch of its regex that also expects a
    // prerelease group, not as a general prefix on either side. Found
    // by differential fuzzing; an earlier version of this fix stripped
    // `=` unconditionally from both sides, which was too permissive in
    // exactly the cases without a prerelease.
    let from_text = from_text.trim();
    let to_text = to_text.trim();
    let to_text = if to_text.starts_with('=') && to_text.contains('-') {
        &to_text[1..]
    } else if to_text.starts_with('=') {
        // '=' with no prerelease on the right: leave the '=' in place
        // so parse_partial_with rejects the whole thing, matching the
        // original returning null here.
        to_text
    } else {
        to_text
    };

    let (from, from_pre, _) = parse_partial_with(from_text, options.loose)?;
    let (to, to_pre, _) = parse_partial_with(to_text, options.loose)?;

    let z = if options.include_prerelease { "-0" } else { "" };

    let lo = if from.is_any() {
        String::new()
    } else {
        let major = from.major.unwrap();
        match (from.minor, from.patch) {
            (None, _) => format!(">={major}.0.0{z}"),
            (Some(minor), None) => format!(">={major}.{minor}.0{z}"),
            (Some(minor), Some(patch)) => match from_pre.as_deref() {
                Some(p) => format!(">={major}.{minor}.{patch}-{p}"),
                None => format!(">={major}.{minor}.{patch}{z}"),
            },
        }
    };

    let hi = if to.is_any() {
        String::new()
    } else {
        let major = to.major.unwrap();
        match (to.minor, to.patch) {
            (None, _) => format!("<{}.0.0-0", major + 1),
            (Some(minor), None) => format!("<{major}.{}.0-0", minor + 1),
            (Some(minor), Some(patch)) => match to_pre.as_deref() {
                Some(pr) => format!("<={major}.{minor}.{patch}-{pr}"),
                None => {
                    if options.include_prerelease {
                        format!("<{major}.{minor}.{}-0", patch + 1)
                    } else {
                        format!("<={major}.{minor}.{patch}")
                    }
                }
            },
        }
    };

    Some(format!("{lo} {hi}").trim().to_string())
}

/// Find the ` - ` that separates a hyphen range, skipping hyphens that
/// belong to prerelease tags.
///
/// The original's HYPHENRANGE pattern is anchored with `^` and `$`, so
/// it only fires when the hyphen range is the *entire* comparator set.
/// `1.0.0 - 2.0.0` is a range; `1.0.0 - 2.0.0 >=1.5.0` is not — there
/// the separator is simply dropped and three plain comparators remain.
/// Found by differential fuzzing.
fn find_hyphen_separator(text: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'-' {
            let before_is_space = i > 0 && bytes[i - 1].is_ascii_whitespace();
            let after_is_space = i + 1 < bytes.len() && bytes[i + 1].is_ascii_whitespace();
            if before_is_space && after_is_space {
                return Some(i);
            }
        }
        i += 1;
    }
    None
}

/// One `&&`-joined set of comparators.
#[derive(Debug, Clone, PartialEq)]
pub struct ComparatorSet(pub Vec<Comparator>);

impl ComparatorSet {
    fn test(&self, version: &SemVer, options: Options) -> bool {
        for c in &self.0 {
            if !c.test(version) {
                return false;
            }
        }

        // A prerelease only satisfies a range if some comparator in this
        // set names the same [major, minor, patch] with a prerelease of
        // its own. Otherwise `^1.2.3-pr.1` would admit
        // `1.2.4-alpha.notready`, which nobody wants.
        if version.is_prerelease() && !options.include_prerelease {
            for c in &self.0 {
                let Some(target) = &c.version else { continue };
                if target.is_prerelease()
                    && target.major == version.major
                    && target.minor == version.minor
                    && target.patch == version.patch
                {
                    return true;
                }
            }
            return false;
        }

        true
    }
}

impl std::fmt::Display for ComparatorSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let parts: Vec<String> = self.0.iter().map(|c| c.to_string()).collect();
        write!(f, "{}", parts.join(" "))
    }
}

/// A full range: comparator sets joined by `||`.
#[derive(Debug, Clone, PartialEq)]
pub struct Range {
    /// The `||`-separated alternatives; a version satisfies the range
    /// if it satisfies any one of them.
    pub set: Vec<ComparatorSet>,
    options: Options,
    raw: String,
}

impl Range {
    /// Parse a range expression, such as `^1.2.3` or `>=1.0.0 <2.0.0`.
    pub fn parse(input: &str, options: impl Into<Options>) -> Result<Range> {
        let options = options.into();
        let raw = input.trim().to_string();

        // An alternative that is entirely unreadable is dropped from
        // the union, same as a token within a mixed alternative:
        //
        //   'foo || 1.2.3'  ->  '1.2.3'   (alternative dropped)
        //   'foo'           ->  null      (nothing left)
        //
        // But an alternative whose only problem is an oversized numeric
        // component is fatal to the whole range, not just dropped --
        // that token parses fine and names a version that cannot
        // exist, which is a different kind of failure from "cannot be
        // read at all". parse_comparator_set tags this case with a
        // distinct error reason so the two can be told apart here.
        // Found by differential fuzzing; this rule was tried once
        // before without the oversized/unreadable distinction and made
        // things much worse -- the distinction is what makes it work.
        let mut set: Vec<ComparatorSet> = Vec::new();
        let mut any_alternative_dropped = false;
        let mut pending_null_set: Option<ComparatorSet> = None;
        for alternative in raw.split("||") {
            let comparators = match Self::parse_comparator_set(alternative, options) {
                Ok(c) => c,
                Err(e) => {
                    let is_oversized = matches!(
                        &e,
                        Error::InvalidRange { reason, .. }
                            if *reason == "component exceeds the maximum safe integer"
                    );
                    if is_oversized || !options.loose {
                        return Err(e);
                    }
                    any_alternative_dropped = true;
                    continue;
                }
            };
            // `*` absorbs everything, but only after every alternative
            // has been validated — an invalid one later in the range
            // still invalidates the whole thing.
            let is_any = comparators.0.len() == 1 && comparators.0[0].is_any();
            if is_any {
                set.clear();
                set.push(comparators);
                // Keep validating the rest; `?` above will propagate any
                // failure. Nothing more is pushed once `*` is present.
                continue;
            }
            if set.len() == 1 && set[0].0.len() == 1 && set[0].0[0].is_any() {
                continue;
            }
            // An alternative that can never match contributes nothing to
            // a union when something else is already present, so it is
            // dropped in that case. But if it's the *only* alternative
            // that parsed at all -- everything else having been dropped
            // separately for being unreadable -- it has to survive and
            // become the answer, exactly as if it had been written
            // alone. Otherwise '>=V1.2.3 || >1 >*' would wrongly end up
            // with nothing at all instead of the null-set range that
            // '>1 >*' alone produces. Tracked by deferring the drop
            // until we know whether anything else will end up in `set`.
            let is_null = comparators.0.len() == 1 && comparators.0[0].is_null_set();
            if is_null {
                if set.is_empty() {
                    pending_null_set = Some(comparators);
                }
                continue;
            }
            // Something concrete showed up after all -- any pending
            // null-set alternative is superseded and can be forgotten.
            pending_null_set = None;
            // Duplicate alternatives are NOT collapsed -- the original
            // renders each one independently and never compares them:
            // '1 || 1' stays '>=1.0.0 <2.0.0-0||>=1.0.0 <2.0.0-0', not
            // deduplicated. An earlier version of this port did
            // deduplicate, which both diverged from the original and
            // motivated a HashSet that turned out to solve a problem
            // the original doesn't have: it's already fast without any
            // deduplication, because it never has to compare
            // alternatives against each other in the first place.
            set.push(comparators);
        }

        if set.is_empty() {
            if let Some(null_set) = pending_null_set {
                set.push(null_set);
            } else if any_alternative_dropped {
                // Every alternative was unreadable, not merely
                // unsatisfiable -- there is nothing left to mean
                // anything. 'foo' alone is null, unlike '<0.0.0-0'
                // alone which is a valid empty-set range.
                return Err(Error::InvalidRange {
                    input: input.to_string(),
                    reason: "no usable alternative",
                });
            } else {
                // Every alternative was unsatisfiable, so the range
                // matches nothing. That is a valid range, not a parse
                // failure.
                let null = Comparator::parse("<0.0.0-0", options)?;
                set.push(ComparatorSet(vec![null]));
            }
        }

        Ok(Range { set, options, raw })
    }

    fn parse_comparator_set(text: &str, options: Options) -> Result<ComparatorSet> {
        let text = text.trim();

        // Hyphen ranges are rewritten first, before the text is split on
        // whitespace — the separator is whitespace-delimited itself.
        let text = replace_hyphen(text, options).unwrap_or_else(|| text.to_string());

        // `> 1.2.3` → `>1.2.3`, `~ 1.2` → `~1.2`, so the split below does
        // not separate an operator from its version.
        let text = glue_operators(&text);

        let mut comparators = Vec::new();
        // Tracks whether any token failed to yield a comparator. A token
        // that desugars to the empty string (`~x`, `^x`) succeeded and
        // means "any version"; a token that could not be read at all
        // (`foo`) makes the whole alternative unusable.
        let mut saw_unusable = false;
        for token in text.split_whitespace() {
            // Try each sugar in turn; if none applies the token is
            // already a plain comparator and is passed through so that
            // Comparator::parse can accept or reject it.
            let desugared = replace_caret(token, options)
                .or_else(|| replace_tilde(token, options))
                .or_else(|| replace_xrange(token, options))
                .unwrap_or_else(|| token.to_string());

            // A sugar function returning "" is a successful desugar to
            // ANY (`^x`, `~x`, a bare `*`), not an absence of output.
            // `"".split_whitespace()` yields zero iterations, so without
            // this the survivor of a mixed set (e.g. the second token in
            // '>V1.2.3 ^*') would silently contribute nothing --
            // `comparators` would end up empty even though this token
            // parsed successfully, and the alternative would be wrongly
            // treated as if every token in it had failed. Found by
            // differential fuzzing.
            if desugared.is_empty() {
                comparators.push(Comparator::any(options));
                continue;
            }

            for part in desugared.split_whitespace() {
                if part.is_empty() {
                    continue;
                }
                // `>=0.0.0` admits everything, same as `*` -- except
                // when it's the lower bound half of a caret/tilde
                // desugar whose source had a leading-zero major
                // (`^00`, `~00`), where the original keeps it explicit
                // rather than treating it as an omitted component. That
                // case carries KEEP_ZERO_MARKER from replace_caret /
                // replace_tilde, so it's excluded from absorption here
                // and the marker is stripped just before the comparator
                // is actually parsed. Found by differential fuzzing.
                let keep_explicit = part.contains(KEEP_ZERO_MARKER);
                if part == ">=0.0.0" && !options.include_prerelease && !keep_explicit {
                    comparators.push(Comparator::any(options));
                    continue;
                }
                if part.trim_end_matches(KEEP_ZERO_MARKER) == ">=0.0.0-0"
                    && options.include_prerelease
                    && !keep_explicit
                {
                    comparators.push(Comparator::any(options));
                    continue;
                }
                // Build metadata never affects a bound and can be
                // arbitrarily long, so strip it before the comparator is
                // parsed — otherwise a long tag pushes the comparator
                // past MAX_LENGTH and it is wrongly rejected.
                let part = part.trim_end_matches(KEEP_ZERO_MARKER);
                let part = strip_build(part);
                match Comparator::parse(&part, options) {
                    Ok(c) => comparators.push(c),
                    Err(e) => {
                        // Loose mode drops any token it cannot read and
                        // keeps the rest of the set:
                        //
                        //   '>V1.2.3 >=1.0.0'   ->  '>=1.0.0'
                        //   '>foo >=1.0.0'      ->  '>=1.0.0'
                        //   '>1.2.3.4 >=1.0.0'  ->  '>=1.0.0'
                        //
                        // The one exception is a numeric component larger
                        // than the original can hold. That token is not
                        // unreadable — it parses fine and names a version
                        // that cannot exist — so it invalidates the whole
                        // range regardless of what else is present:
                        //
                        //   '>=1.0.0 9007199254740992.x'  ->  null
                        //
                        // Both rules were derived by asking the original.
                        // Neither is stated anywhere in its source.
                        if has_oversized_component(&part) {
                            return Err(Error::InvalidRange {
                                input: text.to_string(),
                                reason: "component exceeds the maximum safe integer",
                            });
                        }
                        if !options.loose {
                            return Err(e);
                        }
                        saw_unusable = true;
                    }
                }
            }
        }

        // An alternative whose every token was discarded is not an
        // unrestricted range — it is an unusable one. `1.0.0 || foo`
        // must not admit everything just because `foo` was dropped.
        if comparators.is_empty() {
            if saw_unusable {
                return Err(Error::InvalidRange {
                    input: text.to_string(),
                    reason: "no usable comparator in this alternative",
                });
            }
            comparators.push(Comparator::any(options));
        }

        // An unsatisfiable comparator collapses the whole set.
        if let Some(null) = comparators.iter().find(|c| c.is_null_set()) {
            return Ok(ComparatorSet(vec![null.clone()]));
        }

        // Drop `*` once anything more specific is present, and remove
        // duplicates while preserving order.
        let mut seen = std::collections::HashSet::new();
        let mut out: Vec<Comparator> = Vec::new();
        let has_specific = comparators.iter().any(|c| !c.is_any());
        for c in comparators {
            if c.is_any() && has_specific {
                continue;
            }
            let key = c.to_string();
            if !seen.insert(key) {
                continue;
            }
            out.push(c);
        }

        Ok(ComparatorSet(out))
    }

    /// Whether a version satisfies this range.
    pub fn test(&self, version: &SemVer) -> bool {
        self.set.iter().any(|s| s.test(version, self.options))
    }

    /// Whether a version string satisfies this range.
    pub fn test_str(&self, version: &str) -> bool {
        match SemVer::parse(version, self.options) {
            Ok(v) => self.test(&v),
            Err(_) => false,
        }
    }

    /// The trimmed input text this range was parsed from.
    pub fn raw(&self) -> &str {
        &self.raw
    }
}

/// `> 1.2.3` → `>1.2.3`, `~ 1.2` → `~1.2`, `^ 1` → `^1`.
///
/// The original does this with three separate regex passes
/// (COMPARATORTRIM, TILDETRIM, CARETTRIM).
fn glue_operators(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let bytes = text.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        let c = bytes[i];
        out.push(c as char);

        let is_op_end = match c {
            b'~' | b'^' => true,
            b'>' | b'<' => !matches!(bytes.get(i + 1), Some(b'=')),
            b'=' => matches!(
                bytes.get(i.wrapping_sub(1)),
                Some(b'>') | Some(b'<') | Some(b'=')
            ),
            _ => false,
        };

        if is_op_end {
            // Skip whitespace between the operator and its version.
            let mut j = i + 1;
            while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                j += 1;
            }
            // Only glue when a version actually follows.
            if j < bytes.len() && (bytes[j].is_ascii_digit() || matches!(bytes[j], b'x' | b'X' | b'*' | b'v')) {
                i = j;
                continue;
            }
        }
        i += 1;
    }

    out
}

impl std::fmt::Display for Range {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let parts: Vec<String> = self.set.iter().map(|s| s.to_string()).collect();
        write!(f, "{}", parts.join("||"))
    }
}

/// Remove build metadata from a comparator token.
///
/// `>=1.2.3+sha512.aaaa...` becomes `>=1.2.3`. The tag has no effect on
/// any bound and is unbounded in length, so leaving it in place would
/// push the token past the version-length limit and get it rejected.
fn strip_build(token: &str) -> String {
    match token.find('+') {
        Some(i) => token[..i].to_string(),
        None => token.to_string(),
    }
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod trace {
    #[test]
    fn xrange_declines_bare_number_with_prerelease() {
        let o = crate::constants::Options::loose();
        // `09090-0` is not a partial version, so replace_xrange must
        // decline rather than reporting a wildcard.
        assert_eq!(super::replace_xrange(">=09090-0", o), None);
    }
}

/// Whether a comparator names a numeric component larger than the
/// original can represent.
///
/// This is the one parse failure that is fatal to a whole range rather
/// than droppable. The token is well-formed and simply names a version
/// that cannot exist, so ignoring it would change what the range means
/// rather than tidying it away.
fn has_oversized_component(part: &str) -> bool {
    let body = part.trim_start_matches(['<', '>', '=', '~', '^']).trim();

    // An uppercase `V` is not a valid prefix in any mode, so such a
    // token never reaches the numeric check at all — it is unreadable
    // and therefore droppable, however large its components look.
    if body.starts_with('V') {
        return false;
    }

    // Only the numeric head can overflow; a prerelease or build tag is
    // never read as a number here.
    let head = body
        .split(['-', '+'])
        .next()
        .unwrap_or("")
        .trim_start_matches('v');

    for component in head.split('.') {
        if component.is_empty() || !component.bytes().all(|b| b.is_ascii_digit()) {
            continue;
        }
        match component.parse::<u64>() {
            Ok(n) if n > crate::constants::MAX_SAFE_INTEGER => return true,
            // More digits than u64 can hold at all.
            Err(_) => return true,
            _ => {}
        }
    }
    false
}
