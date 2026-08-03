//! Comparators — one constraint, such as `>=1.2.3`.
//!
//! Ported from `classes/comparator.js`. The original keeps the operator
//! as a string and uses a sentinel `ANY` object for a bare `*`; an enum
//! plus `Option<SemVer>` covers both without the sentinel.

use crate::constants::Options;
use crate::error::{Error, Result};
use crate::semver::SemVer;
use std::cmp::Ordering;

/// A comparator's operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    /// `=`, or no operator written at all.
    Eq,
    /// `>`
    Gt,
    /// `>=`
    Gte,
    /// `<`
    Lt,
    /// `<=`
    Lte,
}

impl Op {
    /// The operator's canonical spelling, empty for [`Op::Eq`].
    pub fn as_str(self) -> &'static str {
        match self {
            Op::Eq => "",
            Op::Gt => ">",
            Op::Gte => ">=",
            Op::Lt => "<",
            Op::Lte => "<=",
        }
    }

    /// Split an operator off the front. Two-character forms first, so
    /// `<=` is never read as `<` followed by a stray `=`.
    fn split(s: &str, loose: bool) -> (Op, &str) {
        // `==` is only a valid spelling of `=` in loose mode; strict
        // mode rejects it (falls through and lets the "=?" text be
        // read as part of the version, which then fails to parse).
        // Found by differential fuzzing: `==557.5.0` is null in strict
        // mode but "557.5.0" in loose.
        let two_char_eq: &[(&str, Op)] = if loose { &[("==", Op::Eq)] } else { &[] };
        for (lit, op) in [("<=", Op::Lte), (">=", Op::Gte), ("<", Op::Lt), (">", Op::Gt)]
            .iter()
            .chain(two_char_eq)
            .chain(&[("=", Op::Eq)])
        {
            if let Some(rest) = s.strip_prefix(lit) {
                return (*op, rest);
            }
        }
        (Op::Eq, s)
    }
}

/// A single constraint, such as `>=1.2.3`.
#[derive(Debug, Clone, PartialEq)]
pub struct Comparator {
    /// The operator. Meaningless when [`version`](Self::version) is
    /// `None` — an ANY comparator matches regardless of `op`.
    pub op: Op,
    /// `None` is the original's `ANY` — matches every version.
    pub version: Option<SemVer>,
    options: Options,
}

impl Comparator {
    /// The comparator that matches every version (`*`).
    pub fn any(options: Options) -> Self {
        Comparator {
            op: Op::Gte,
            version: None,
            options,
        }
    }

    /// Parse a single comparator, such as `>=1.2.3` or `*`.
    pub fn parse(input: &str, options: impl Into<Options>) -> Result<Comparator> {
        let options = options.into();
        let s = input.trim();

        if s.is_empty() || s == "*" {
            return Ok(Comparator::any(options));
        }

        let (op, rest) = Op::split(s, options.loose);
        let rest = rest.trim();

        if rest.is_empty() || rest == "*" {
            return Ok(Comparator::any(options));
        }

        let version = SemVer::parse(rest, options).map_err(|_| Error::InvalidComparator {
            input: input.to_string(),
            reason: "the version part is not valid",
        })?;

        Ok(Comparator {
            op,
            version: Some(version),
            options,
        })
    }

    /// Whether this comparator matches every version (`*`).
    pub fn is_any(&self) -> bool {
        self.version.is_none()
    }

    /// `<0.0.0-0` — the canonical empty set the original emits when a
    /// range cannot be satisfied by anything.
    pub fn is_null_set(&self) -> bool {
        match (&self.version, self.op) {
            (Some(v), Op::Lt) => {
                v.major == 0
                    && v.minor == 0
                    && v.patch == 0
                    && v.prerelease.len() == 1
                    && v.prerelease[0].to_string() == "0"
            }
            _ => false,
        }
    }

    /// The parsing options this comparator was created with.
    pub fn options(&self) -> Options {
        self.options
    }

    /// Whether a version satisfies this comparator.
    ///
    /// Prerelease filtering is not done here — it depends on the whole
    /// comparator set, so `Range::test` handles it.
    pub fn test(&self, version: &SemVer) -> bool {
        let Some(target) = &self.version else {
            return true;
        };
        match self.op {
            Op::Eq => version.compare(target) == Ordering::Equal,
            Op::Gt => version.compare(target) == Ordering::Greater,
            Op::Gte => version.compare(target) != Ordering::Less,
            Op::Lt => version.compare(target) == Ordering::Less,
            Op::Lte => version.compare(target) != Ordering::Greater,
        }
    }
}

impl std::fmt::Display for Comparator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.version {
            None => write!(f, "*"),
            Some(v) => write!(f, "{}{}", self.op.as_str(), v.version()),
        }
    }
}
