//! Errors.
//!
//! The original signals failure by returning `null` from `parse` and by
//! throwing `TypeError` from the `SemVer` constructor. Neither carries a
//! reason. We return a typed error instead, which costs nothing and
//! makes the `null` cases distinguishable.

use thiserror::Error;

/// Every way a call into this crate can fail.
///
/// Each variant carries the text that failed to parse plus a short,
/// static reason — enough to explain what went wrong without needing a
/// separate error-code enum.
#[derive(Debug, Error, PartialEq, Eq, Clone)]
pub enum Error {
    /// A version string did not parse. Mirrors the original's `parse()`
    /// returning `null`, or its constructor throwing.
    #[error("invalid version {input:?}: {reason}")]
    InvalidVersion {
        /// The text that was given.
        input: String,
        /// A short, human-readable reason it was rejected.
        reason: &'static str,
    },

    /// The input exceeded `MAX_LENGTH` before any parsing was attempted.
    #[error("version string is {len} characters, maximum is {max}")]
    TooLong {
        /// The length of the input that was rejected.
        len: usize,
        /// The maximum length allowed.
        max: usize,
    },

    /// A single comparator (e.g. `>=1.2.3`) did not parse.
    #[error("invalid comparator {input:?}: {reason}")]
    InvalidComparator {
        /// The text that was given.
        input: String,
        /// A short, human-readable reason it was rejected.
        reason: &'static str,
    },

    /// A range expression (e.g. `^1.2.3 || ~2.0.0`) did not parse.
    #[error("invalid range {input:?}: {reason}")]
    InvalidRange {
        /// The text that was given.
        input: String,
        /// A short, human-readable reason it was rejected.
        reason: &'static str,
    },

    /// [`inc`](crate::inc) was called with a release kind it doesn't
    /// recognize, or an identifier it can't accept.
    #[error("invalid increment {kind:?}")]
    InvalidIncrement {
        /// The release kind or identifier text that was rejected.
        kind: String,
    },
}

/// This crate's `Result`, with [`enum@Error`] as the error type.
pub type Result<T> = std::result::Result<T, Error>;
