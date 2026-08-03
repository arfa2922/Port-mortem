//! Identifier comparison.
//!
//! Ported from `internal/identifiers.js`. This is small and very easy to
//! get subtly wrong, which is why it lives in its own module in both
//! implementations.
//!
//! The rule from the SemVer spec: numeric identifiers compare
//! numerically, non-numeric ones compare as ASCII strings, and a numeric
//! identifier always sorts before a non-numeric one.

/// A prerelease or build identifier.
///
/// The original stores these as JavaScript values that are either
/// `number` or `string`, and relies on `typeof` at comparison time. An
/// enum makes the same distinction explicit and total.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Identifier {
    /// A purely-numeric identifier, compared as an integer.
    Numeric(u64),
    /// Anything else, compared as ASCII text.
    Alphanumeric(String),
}

impl Identifier {
    /// Classify a raw identifier the way the original's parser does.
    ///
    /// The original converts to a number when the text matches
    /// `/^[0-9]+$/` and the value is below `MAX_SAFE_INTEGER`. Leading
    /// zeros matter: `01` stays a string in loose mode rather than
    /// becoming the number 1, because turning it into a number would lose
    /// the information that it was written that way.
    pub fn parse(raw: &str) -> Identifier {
        Identifier::parse_with(raw, false)
    }

    /// Classify an identifier, honouring loose mode.
    ///
    /// The distinction is real and observable. Under the strict grammar
    /// `01` is not a legal numeric identifier at all, so the scanner
    /// rejects the version. Loose mode accepts it and the original
    /// coerces it to the number 1 — which means the canonical form loses
    /// the leading zero:
    ///
    /// ```text
    /// semver.parse('0.7.0-beta.01', {loose: true}).prerelease  // ['beta', 1]
    /// ```
    ///
    /// Found by differential fuzzing against the original; the fixture
    /// suite has no case covering it.
    pub fn parse_with(raw: &str, loose: bool) -> Identifier {
        let all_digits = !raw.is_empty() && raw.bytes().all(|b| b.is_ascii_digit());
        if all_digits {
            if let Ok(n) = raw.parse::<u64>() {
                if n <= crate::constants::MAX_SAFE_INTEGER {
                    return Identifier::Numeric(n);
                }
            }
            // Too large for the original to hold as a number, so it stays
            // a string in both implementations.
            return Identifier::Alphanumeric(raw.to_string());
        }
        let _ = loose;
        Identifier::Alphanumeric(raw.to_string())
    }

    /// Whether this identifier is the numeric variant.
    pub fn is_numeric(&self) -> bool {
        matches!(self, Identifier::Numeric(_))
    }
}

impl std::fmt::Display for Identifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Identifier::Numeric(n) => write!(f, "{n}"),
            Identifier::Alphanumeric(s) => write!(f, "{s}"),
        }
    }
}

/// Compare two identifiers.
///
/// Ported from `compareIdentifiers`. The original works on raw JavaScript
/// values and re-derives numericness on every call:
///
/// ```js
/// const anum = numeric.test(a)
/// const bnum = numeric.test(b)
/// if (anum && bnum) { a = +a; b = +b }
/// return a === b ? 0 : (anum && !bnum) ? -1 : (bnum && !anum) ? 1 : a < b ? -1 : 1
/// ```
///
/// We classify once at parse time instead, which makes the four cases
/// below exhaustive rather than a chain of conditionals.
pub fn compare_identifiers(a: &Identifier, b: &Identifier) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    use Identifier::*;

    match (a, b) {
        (Numeric(x), Numeric(y)) => x.cmp(y),
        // A numeric identifier always has lower precedence than an
        // alphanumeric one.
        (Numeric(_), Alphanumeric(_)) => Ordering::Less,
        (Alphanumeric(_), Numeric(_)) => Ordering::Greater,
        // Alphanumeric identifiers compare as ASCII. The original uses
        // JavaScript's `<` on strings, which is UTF-16 code-unit order;
        // for the identifier charset [0-9A-Za-z-] that is byte order.
        (Alphanumeric(x), Alphanumeric(y)) => x.cmp(y),
    }
}

/// Reverse comparison, as exported by the original for sorting.
pub fn rcompare_identifiers(a: &Identifier, b: &Identifier) -> std::cmp::Ordering {
    compare_identifiers(b, a)
}

/// Compare two identifier lists (a whole prerelease tag).
///
/// Ported from the tail of `comparePre`. Precedence is decided by the
/// first differing identifier; if one list is a prefix of the other, the
/// shorter one has lower precedence.
pub fn compare_identifier_lists(a: &[Identifier], b: &[Identifier]) -> std::cmp::Ordering {
    use std::cmp::Ordering;

    let mut i = 0;
    loop {
        match (a.get(i), b.get(i)) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) => match compare_identifiers(x, y) {
                Ordering::Equal => i += 1,
                other => return other,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cmp::Ordering;

    fn id(s: &str) -> Identifier {
        Identifier::parse(s)
    }

    #[test]
    fn numeric_compares_numerically() {
        assert_eq!(compare_identifiers(&id("2"), &id("10")), Ordering::Less);
        assert_eq!(compare_identifiers(&id("10"), &id("2")), Ordering::Greater);
        assert_eq!(compare_identifiers(&id("5"), &id("5")), Ordering::Equal);
    }

    #[test]
    fn numeric_sorts_before_alphanumeric() {
        assert_eq!(compare_identifiers(&id("1"), &id("alpha")), Ordering::Less);
        assert_eq!(compare_identifiers(&id("alpha"), &id("1")), Ordering::Greater);
    }

    #[test]
    fn alphanumeric_compares_as_ascii() {
        assert_eq!(compare_identifiers(&id("alpha"), &id("beta")), Ordering::Less);
        assert_eq!(compare_identifiers(&id("rc"), &id("beta")), Ordering::Greater);
    }

    #[test]
    fn leading_zero_becomes_numeric() {
        // Only loose mode ever reaches here with a leading zero — the
        // strict scanner rejects the version first. The original coerces
        // it to a number, so the canonical form drops the zero.
        assert!(matches!(id("01"), Identifier::Numeric(1)));
        assert!(matches!(id("00"), Identifier::Numeric(0)));
        assert!(matches!(id("0"), Identifier::Numeric(0)));
        assert!(matches!(id("10"), Identifier::Numeric(10)));
        // A digit run with a non-digit is alphanumeric regardless.
        assert!(matches!(id("0a"), Identifier::Alphanumeric(_)));
    }

    #[test]
    fn oversized_numeric_stays_a_string() {
        // Beyond MAX_SAFE_INTEGER the original cannot represent the value
        // as a number, so it stays a string in both implementations.
        let big = "9007199254740992"; // MAX_SAFE_INTEGER + 1
        assert!(matches!(id(big), Identifier::Alphanumeric(_)));
    }

    #[test]
    fn shorter_prefix_has_lower_precedence() {
        let a = vec![id("alpha")];
        let b = vec![id("alpha"), id("1")];
        assert_eq!(compare_identifier_lists(&a, &b), Ordering::Less);
        assert_eq!(compare_identifier_lists(&b, &a), Ordering::Greater);
    }
}
