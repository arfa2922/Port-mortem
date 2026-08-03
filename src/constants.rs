//! Constants.
//!
//! Ported from `internal/constants.js`. These bounds are load-bearing:
//! `MAX_LENGTH` is what keeps a hostile input from being parsed at all,
//! and the original relies on it to make its regexes safe. We do not use
//! regexes, but the limits still define observable behaviour — a version
//! string of 257 characters must be rejected by both implementations, so
//! the constant has to match exactly.

/// The version of the SemVer spec implemented, not of this crate.
pub const SEMVER_SPEC_VERSION: &str = "2.0.0";

/// Longest version string accepted. Anything longer is invalid.
pub const MAX_LENGTH: usize = 256;

/// Largest integer a version component may hold.
///
/// The original inherits JavaScript's `Number.MAX_SAFE_INTEGER`, which is
/// 2^53 - 1. That is not an accident of the host language — it is part of
/// the contract, because a version whose component exceeds it is rejected.
/// Using `u64::MAX` here would silently accept versions the original
/// rejects, so the JavaScript bound is reproduced deliberately.
pub const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

/// Longest component `coerce` will pull out of arbitrary text.
pub const MAX_SAFE_COMPONENT_LENGTH: usize = 16;

/// Longest build identifier: `MAX_LENGTH` minus the six characters of the
/// shortest version that can carry one, `0.0.0+`.
pub const MAX_SAFE_BUILD_LENGTH: usize = MAX_LENGTH - 6;

/// Release kinds accepted by `inc`.
pub const RELEASE_TYPES: &[&str] = &[
    "major",
    "premajor",
    "minor",
    "preminor",
    "patch",
    "prepatch",
    "prerelease",
];

/// Parsing options.
///
/// The original accepts `true`, `false`, or an options object and
/// normalizes them in `internal/parse-options.js`; a bare boolean means
/// `loose`. A struct is clearer in Rust, and `From<bool>` preserves the
/// shorthand so ported fixture cases read the same.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Options {
    /// Accept the loosened grammar (leading zeros coerce, `v`/`=`
    /// prefixes, etc.) instead of the strict SemVer grammar.
    pub loose: bool,
    /// Let a prerelease version satisfy a range even when no comparator
    /// in that range names a matching prerelease.
    pub include_prerelease: bool,
}

impl Options {
    /// Strict parsing, no prereleases admitted beyond the usual rule.
    /// Equivalent to `Options::default()`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Loose parsing, matching the original's `{loose: true}` shorthand.
    pub fn loose() -> Self {
        Options {
            loose: true,
            include_prerelease: false,
        }
    }

    /// Set [`loose`](Self::loose) and return `self`, for chaining.
    pub fn with_loose(mut self, loose: bool) -> Self {
        self.loose = loose;
        self
    }

    /// Set [`include_prerelease`](Self::include_prerelease) and return
    /// `self`, for chaining.
    pub fn with_include_prerelease(mut self, include: bool) -> Self {
        self.include_prerelease = include;
        self
    }
}

impl From<bool> for Options {
    /// `true` means loose, matching the original's shorthand.
    fn from(loose: bool) -> Self {
        Options {
            loose,
            include_prerelease: false,
        }
    }
}
