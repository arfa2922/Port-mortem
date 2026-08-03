//! Version parsing and comparison.
//!
//! Ported from `classes/semver.js`. The original drives everything from
//! regexes assembled in `internal/re.js`; this is a hand-written scanner
//! over the same grammar.
//!
//! That is the one significant structural divergence in the port, and it
//! is deliberate. The original carries a second set of regexes
//! (`safeRe`) built by rewriting every unbounded quantifier into a
//! bounded one, because the natural expression of this grammar is
//! vulnerable to catastrophic backtracking. A scanner has no
//! backtracking to catastrophise, so the whole `safeRe` apparatus has no
//! counterpart here — and the length limits it exists to enforce are
//! checked directly instead.

use crate::constants::{Options, MAX_LENGTH, MAX_SAFE_INTEGER};
use crate::error::{Error, Result};
use crate::identifiers::{compare_identifier_lists, Identifier};
use std::cmp::Ordering;

/// A parsed semantic version: `major.minor.patch[-prerelease][+build]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemVer {
    /// The `X` in `X.Y.Z`.
    pub major: u64,
    /// The `Y` in `X.Y.Z`.
    pub minor: u64,
    /// The `Z` in `X.Y.Z`.
    pub patch: u64,
    /// The dot-separated identifiers after `-`, if any. Empty means
    /// this is a release version, not a prerelease.
    pub prerelease: Vec<Identifier>,
    /// Build metadata. Carried through parsing and formatting but
    /// deliberately ignored by every comparison — the SemVer spec says
    /// build metadata does not affect precedence.
    pub build: Vec<String>,
    options: Options,
}

impl SemVer {
    /// Parse a version string.
    ///
    /// In loose mode the original tolerates a leading `v` or `=`,
    /// surrounding whitespace, and numeric components with leading zeros.
    pub fn parse(version: &str, options: impl Into<Options>) -> Result<SemVer> {
        let options = options.into();
        Self::parse_with(version, options)
    }

    fn parse_with(version: &str, options: Options) -> Result<SemVer> {
        // The original checks length before anything else, against the
        // untrimmed input.
        if version.len() > MAX_LENGTH {
            return Err(Error::TooLong {
                len: version.len(),
                max: MAX_LENGTH,
            });
        }

        let s = version.trim();

        // Loose mode allows `v1.2.3` and `=1.2.3`; strict mode allows a
        // bare leading `v` only.
        let s = if options.loose {
            s.trim_start_matches(['=', 'v']).trim_start()
        } else {
            s.strip_prefix('v').unwrap_or(s)
        };

        let mut scanner = Scanner::new(s, options);
        let parsed = scanner.version()?;
        scanner.expect_end()?;

        Ok(SemVer { options, ..parsed })
    }

    /// The canonical rendering: `major.minor.patch` plus a prerelease tag
    /// when present. Build metadata is not included, matching the
    /// original's `format()` and its `version` property.
    pub fn version(&self) -> String {
        let mut out = format!("{}.{}.{}", self.major, self.minor, self.patch);
        if !self.prerelease.is_empty() {
            out.push('-');
            for (i, id) in self.prerelease.iter().enumerate() {
                if i > 0 {
                    out.push('.');
                }
                out.push_str(&id.to_string());
            }
        }
        out
    }

    /// The full string including build metadata.
    pub fn to_string_with_build(&self) -> String {
        let mut out = self.version();
        if !self.build.is_empty() {
            out.push('+');
            out.push_str(&self.build.join("."));
        }
        out
    }

    /// The parsing options this version was created with.
    pub fn options(&self) -> Options {
        self.options
    }

    /// Whether this version has a prerelease tag.
    pub fn is_prerelease(&self) -> bool {
        !self.prerelease.is_empty()
    }

    /// Compare only the numeric components.
    pub fn compare_main(&self, other: &SemVer) -> Ordering {
        self.major
            .cmp(&other.major)
            .then_with(|| self.minor.cmp(&other.minor))
            .then_with(|| self.patch.cmp(&other.patch))
    }

    /// Compare only the prerelease tags.
    ///
    /// The asymmetry here is the part people get wrong: having no
    /// prerelease outranks having one, so `1.0.0` is greater than
    /// `1.0.0-alpha`.
    pub fn compare_pre(&self, other: &SemVer) -> Ordering {
        match (self.prerelease.is_empty(), other.prerelease.is_empty()) {
            (true, true) => Ordering::Equal,
            (false, true) => Ordering::Less,
            (true, false) => Ordering::Greater,
            (false, false) => compare_identifier_lists(&self.prerelease, &other.prerelease),
        }
    }

    /// Full precedence comparison. Build metadata is ignored.
    pub fn compare(&self, other: &SemVer) -> Ordering {
        self.compare_main(other).then_with(|| self.compare_pre(other))
    }

    /// Compare including build metadata, as the original's
    /// `compareBuild` does. Used for sorting, not for precedence.
    pub fn compare_build(&self, other: &SemVer) -> Ordering {
        self.compare(other).then_with(|| {
            let a: Vec<Identifier> = self.build.iter().map(|s| Identifier::parse(s)).collect();
            let b: Vec<Identifier> = other.build.iter().map(|s| Identifier::parse(s)).collect();
            compare_identifier_lists(&a, &b)
        })
    }
}

impl std::fmt::Display for SemVer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.version())
    }
}

impl PartialOrd for SemVer {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SemVer {
    fn cmp(&self, other: &Self) -> Ordering {
        self.compare(other)
    }
}

// ── Scanner ─────────────────────────────────────────────────────────────

struct Scanner<'a> {
    input: &'a str,
    pos: usize,
    options: Options,
}

impl<'a> Scanner<'a> {
    fn new(input: &'a str, options: Options) -> Self {
        Scanner {
            input,
            pos: 0,
            options,
        }
    }

    fn peek(&self) -> Option<u8> {
        self.input.as_bytes().get(self.pos).copied()
    }

    fn eat(&mut self, b: u8) -> bool {
        if self.peek() == Some(b) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn expect(&mut self, b: u8) -> Result<()> {
        if self.eat(b) {
            Ok(())
        } else {
            Err(Error::InvalidVersion {
                input: self.input.to_string(),
                reason: "missing separator",
            })
        }
    }

    fn expect_end(&self) -> Result<()> {
        if self.pos == self.input.len() {
            Ok(())
        } else {
            Err(Error::InvalidVersion {
                input: self.input.to_string(),
                reason: "trailing characters",
            })
        }
    }

    /// A numeric identifier: `0`, or a non-zero digit followed by more.
    ///
    /// Loose mode drops the leading-zero restriction, which is the whole
    /// difference between `NUMERICIDENTIFIER` and
    /// `NUMERICIDENTIFIERLOOSE` in the original.
    fn numeric_identifier(&mut self) -> Result<u64> {
        let start = self.pos;
        while matches!(self.peek(), Some(b) if b.is_ascii_digit()) {
            self.pos += 1;
        }
        let raw = &self.input[start..self.pos];

        if raw.is_empty() {
            return Err(Error::InvalidVersion {
                input: self.input.to_string(),
                reason: "expected a number",
            });
        }

        if !self.options.loose && raw.len() > 1 && raw.starts_with('0') {
            return Err(Error::InvalidVersion {
                input: self.input.to_string(),
                reason: "leading zero in a version component",
            });
        }

        let n: u64 = raw.parse().map_err(|_| Error::InvalidVersion {
            input: self.input.to_string(),
            reason: "version component out of range",
        })?;

        // The original inherits this bound from JavaScript's number type
        // and enforces it explicitly; a version above it is invalid, not
        // merely imprecise.
        if n > MAX_SAFE_INTEGER {
            return Err(Error::InvalidVersion {
                input: self.input.to_string(),
                reason: "version component exceeds the maximum safe integer",
            });
        }

        Ok(n)
    }

    /// One prerelease identifier: digits, letters, or hyphens.
    fn prerelease_identifier(&mut self) -> Result<Identifier> {
        let start = self.pos;
        while matches!(self.peek(), Some(b) if b.is_ascii_alphanumeric() || b == b'-') {
            self.pos += 1;
        }
        let raw = &self.input[start..self.pos];
        if raw.is_empty() {
            return Err(Error::InvalidVersion {
                input: self.input.to_string(),
                reason: "empty prerelease identifier",
            });
        }

        // A purely numeric prerelease identifier may not have a leading
        // zero under the strict grammar. Loose mode accepts it and
        // coerces it to a number, dropping the zero.
        if !self.options.loose
            && raw.len() > 1
            && raw.starts_with('0')
            && raw.bytes().all(|b| b.is_ascii_digit())
        {
            return Err(Error::InvalidVersion {
                input: self.input.to_string(),
                reason: "leading zero in a numeric prerelease identifier",
            });
        }

        Ok(Identifier::parse_with(raw, self.options.loose))
    }

    /// One build identifier. Leading zeros are allowed here — build
    /// metadata has no numeric interpretation.
    fn build_identifier(&mut self) -> Result<String> {
        let start = self.pos;
        while matches!(self.peek(), Some(b) if b.is_ascii_alphanumeric() || b == b'-') {
            self.pos += 1;
        }
        let raw = &self.input[start..self.pos];
        if raw.is_empty() {
            return Err(Error::InvalidVersion {
                input: self.input.to_string(),
                reason: "empty build identifier",
            });
        }
        Ok(raw.to_string())
    }

    /// Reproduce the loose grammar's backtracking split.
    ///
    /// Called when a `.` follows the patch component in loose mode. The
    /// regex would have surrendered trailing digits from the patch so the
    /// optional prerelease group could match; we do the same by hand,
    /// trying the longest patch first, which is what a greedy quantifier
    /// gives up last.
    fn backtrack_patch_for_prerelease(&mut self, patch_raw_end: PatchSpan) -> Option<SemVer> {
        let PatchSpan { start, end, major, minor } = patch_raw_end;
        let digits = &self.input[start..end];

        // Give back one digit at a time, longest patch first.
        for keep in (1..digits.len().max(2)).rev() {
            let patch_text = &digits[..keep];
            let rest_start = start + keep;

            // The patch must still be a legal numeric identifier.
            let Ok(patch) = patch_text.parse::<u64>() else { continue };
            if patch > MAX_SAFE_INTEGER {
                continue;
            }

            // Everything from here must parse as a prerelease followed by
            // an optional build tag, and then reach end of input.
            let mut probe = Scanner::new(self.input, self.options);
            probe.pos = rest_start;

            // The separator is optional in loose mode, so the remainder
            // may or may not start with `-`. When it does and nothing
            // identifier-like follows, that hyphen is itself the first
            // identifier — the same rule as a bare trailing `-`.
            let ate_hyphen = probe.eat(b'-');

            let mut prerelease = Vec::new();
            let mut ok = true;
            loop {
                let identifier_next =
                    matches!(probe.peek(), Some(b) if b.is_ascii_alphanumeric() || b == b'-');
                if !identifier_next {
                    if ate_hyphen && prerelease.is_empty() {
                        prerelease.push(Identifier::Alphanumeric("-".to_string()));
                    } else {
                        ok = false;
                        break;
                    }
                } else {
                    match probe.prerelease_identifier() {
                        Ok(id) => prerelease.push(id),
                        Err(_) => {
                            ok = false;
                            break;
                        }
                    }
                }
                if !probe.eat(b'.') {
                    break;
                }
            }
            if !ok || prerelease.is_empty() {
                continue;
            }

            let mut build = Vec::new();
            if probe.eat(b'+') {
                loop {
                    match probe.build_identifier() {
                        Ok(b) => build.push(b),
                        Err(_) => {
                            ok = false;
                            break;
                        }
                    }
                    if !probe.eat(b'.') {
                        break;
                    }
                }
            }
            if !ok || probe.pos != self.input.len() {
                continue;
            }

            self.pos = probe.pos;
            return Some(SemVer {
                major,
                minor,
                patch,
                prerelease,
                build,
                options: self.options,
            });
        }
        None
    }

    fn version(&mut self) -> Result<SemVer> {
        let major = self.numeric_identifier()?;
        self.expect(b'.')?;
        let minor = self.numeric_identifier()?;
        self.expect(b'.')?;
        // Scan the patch digits without validating them yet: in loose
        // mode an over-large run may still be legal once backtracking
        // hands some of its digits to the prerelease.
        let patch_start = self.pos;
        while matches!(self.peek(), Some(b) if b.is_ascii_digit()) {
            self.pos += 1;
        }
        let patch_raw = &self.input[patch_start..self.pos];
        if patch_raw.is_empty() {
            return Err(Error::InvalidVersion {
                input: self.input.to_string(),
                reason: "expected a patch number",
            });
        }
        if !self.options.loose && patch_raw.len() > 1 && patch_raw.starts_with('0') {
            return Err(Error::InvalidVersion {
                input: self.input.to_string(),
                reason: "leading zero in a version component",
            });
        }
        let patch = PatchSpan {
            start: patch_start,
            end: self.pos,
            major,
            minor,
        };
        let patch_fits = patch_raw
            .parse::<u64>()
            .ok()
            .filter(|n| *n <= MAX_SAFE_INTEGER);

        let mut prerelease = Vec::new();

        // Loose mode lets the prerelease hyphen be omitted: `1.2.3tag`
        // parses as `1.2.3-tag`. Detect a letter directly after the
        // patch digits and treat it as the start of a prerelease.
        if self.options.loose
            && !matches!(self.peek(), Some(b'-') | Some(b'+') | None)
            && matches!(self.peek(), Some(b) if b.is_ascii_alphabetic())
        {
            loop {
                prerelease.push(self.prerelease_identifier()?);
                if !self.eat(b'.') {
                    break;
                }
            }
        } else if self.eat(b'-') {
            // Loose mode treats a trailing `-` with nothing after it as
            // the identifier "-", so `1.2.3-` is a valid version whose
            // prerelease is ["-"]. Strict mode rejects it. Found by
            // differential fuzzing.
            // In loose mode a `-` with no identifier character after it
            // is itself the first identifier, whether the version ends
            // there (`1.2.3-` → ["-"]) or a dot follows
            // (`1.2.3-.x` → ["-", "x"]).
            let identifier_next =
                matches!(self.peek(), Some(b) if b.is_ascii_alphanumeric() || b == b'-');
            if self.options.loose && !identifier_next {
                prerelease.push(Identifier::Alphanumeric("-".to_string()));
                while self.eat(b'.') {
                    prerelease.push(self.prerelease_identifier()?);
                }
            } else {
                loop {
                    prerelease.push(self.prerelease_identifier()?);
                    if !self.eat(b'.') {
                        break;
                    }
                }
            }
        } else if self.options.loose
            && matches!(self.peek(), Some(b'.') | Some(b'-'))
        {
            // The loose grammar makes the prerelease separator optional:
            // `(?:-?(...))?`. A regex engine therefore backtracks the
            // patch digits and lets the remainder become a prerelease, so
            //
            //     90071992547.0991.59.145515  →  patch 5, prerelease 9.145515
            //
            // The digits of `59` are split between the two fields. This
            // has no counterpart in the strict grammar and reads like an
            // accident of the pattern rather than an intent, but it is
            // observable behaviour and the port has to reproduce it.
            //
            // A scanner has no backtracking to exploit, so the split is
            // performed explicitly: give the last digit of the patch back
            // and reparse from there.
            if let Some(split) = self.backtrack_patch_for_prerelease(patch) {
                return Ok(split);
            }
        }

        let mut build = Vec::new();
        if self.eat(b'+') {
            loop {
                build.push(self.build_identifier()?);
                if !self.eat(b'.') {
                    break;
                }
            }
        }

        let Some(patch_value) = patch_fits else {
            return Err(Error::InvalidVersion {
                input: self.input.to_string(),
                reason: "version component exceeds the maximum safe integer",
            });
        };

        Ok(SemVer {
            major,
            minor,
            patch: patch_value,
            prerelease,
            build,
            options: self.options,
        })
    }
}

/// Where the patch digits sat in the input, plus the components already
/// parsed. Carried so the loose backtracking path can reconstruct a
/// version after surrendering some of those digits.
#[derive(Clone, Copy)]
struct PatchSpan {
    start: usize,
    end: usize,
    major: u64,
    minor: u64,
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod semver_struct_tests {
    use super::*;

    #[test]
    fn parse_extracts_all_components() {
        let v = SemVer::parse("1.2.3-alpha.1+build.5", Options::new()).unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 3);
        assert!(v.is_prerelease());
        assert!(SemVer::parse("not-a-version", Options::new()).is_err());
    }

    #[test]
    fn version_and_to_string_with_build_differ_by_build_metadata() {
        let v = SemVer::parse("1.2.3+build.5", Options::new()).unwrap();
        assert_eq!(v.version(), "1.2.3");
        assert_eq!(v.to_string_with_build(), "1.2.3+build.5");
    }

    #[test]
    fn options_returns_the_options_used_to_parse() {
        let v = SemVer::parse("1.2.3", Options::new()).unwrap();
        // Just confirm it round-trips without panicking.
        let _ = v.options();
    }

    #[test]
    fn is_prerelease_is_false_for_release_versions() {
        let v = SemVer::parse("1.2.3", Options::new()).unwrap();
        assert!(!v.is_prerelease());
    }

    #[test]
    fn compare_main_ignores_prerelease_and_build() {
        let a = SemVer::parse("1.2.3-alpha", Options::new()).unwrap();
        let b = SemVer::parse("1.2.3+build.9", Options::new()).unwrap();
        assert_eq!(a.compare_main(&b), Ordering::Equal);
    }

    #[test]
    fn compare_pre_orders_prerelease_before_release() {
        let a = SemVer::parse("1.2.3-alpha", Options::new()).unwrap();
        let b = SemVer::parse("1.2.3", Options::new()).unwrap();
        assert_eq!(a.compare_pre(&b), Ordering::Less);
    }

    #[test]
    fn compare_ignores_build_metadata() {
        let a = SemVer::parse("1.2.3+build.1", Options::new()).unwrap();
        let b = SemVer::parse("1.2.3+build.2", Options::new()).unwrap();
        assert_eq!(a.compare(&b), Ordering::Equal);
    }

    #[test]
    fn compare_build_breaks_ties_using_build_metadata() {
        let a = SemVer::parse("1.2.3+build.1", Options::new()).unwrap();
        let b = SemVer::parse("1.2.3+build.2", Options::new()).unwrap();
        assert_eq!(a.compare_build(&b), Ordering::Less);
    }
}