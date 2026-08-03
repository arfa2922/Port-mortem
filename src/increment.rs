//! Version incrementing.
//!
//! Ported from `SemVer.prototype.inc` in `classes/semver.js`.
//!
//! The original mutates in place and returns `this`; this port returns a
//! new version, because a mutating `inc` on a value type would be
//! surprising in Rust and the original's callers all treat the result as
//! a fresh version anyway.
//!
//! The rules here are less obvious than they look. `major` on `1.0.0-5`
//! gives `1.0.0`, not `2.0.0` — a prerelease of a version is already
//! "before" it, so releasing it is the increment. The same asymmetry
//! applies to `minor` and `patch`, each with its own condition.

use crate::error::{Error, Result};
use crate::identifiers::{compare_identifiers, Identifier};
use crate::semver::SemVer;

/// What kind of increment to apply.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Release {
    /// Bump the major version, resetting minor and patch to 0.
    Major,
    /// Bump the minor version, resetting patch to 0.
    Minor,
    /// Bump the patch version.
    Patch,
    /// Bump the major version and start a new prerelease.
    PreMajor,
    /// Bump the minor version and start a new prerelease.
    PreMinor,
    /// Bump the patch version and start a new prerelease.
    PrePatch,
    /// Bump the prerelease tag, or start one at patch level if there
    /// wasn't already one.
    PreRelease,
    /// Drop the prerelease tag, promoting a prerelease to its release.
    ReleaseOnly,
    /// Bump the prerelease tag alone. The original notes this
    /// "probably shouldn't be used publicly" — `1.0.0` becomes
    /// `1.0.0-0`, which moves backwards.
    Pre,
}

impl Release {
    /// Parse one of the release-kind strings `inc()` accepts, such as
    /// `"major"` or `"prepatch"`.
    pub fn parse(s: &str) -> Result<Release> {
        Ok(match s {
            "major" => Release::Major,
            "minor" => Release::Minor,
            "patch" => Release::Patch,
            "premajor" => Release::PreMajor,
            "preminor" => Release::PreMinor,
            "prepatch" => Release::PrePatch,
            "prerelease" => Release::PreRelease,
            "release" => Release::ReleaseOnly,
            "pre" => Release::Pre,
            other => {
                return Err(Error::InvalidIncrement {
                    kind: other.to_string(),
                })
            }
        })
    }

    fn is_pre(self) -> bool {
        matches!(
            self,
            Release::PreMajor
                | Release::PreMinor
                | Release::PrePatch
                | Release::PreRelease
                | Release::Pre
        )
    }
}

/// Where a new numeric prerelease identifier starts.
///
/// The original takes `identifierBase` as `'0'`, `'1'`, or `false`.
/// `false` means "append no number at all", which is a different shape
/// from "start at zero", so it cannot be folded into an integer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IdentifierBase {
    /// New prerelease numeric identifiers start at 0. The default.
    #[default]
    Zero,
    /// New prerelease numeric identifiers start at 1.
    One,
    /// Append the identifier with no numeric suffix.
    None,
}

impl IdentifierBase {
    fn value(self) -> u64 {
        match self {
            IdentifierBase::One => 1,
            _ => 0,
        }
    }
}

/// Whether `prerelease` already begins with the identifiers in
/// `identifier`, dot-separated.
///
/// Ported from `isPrereleaseIdentifier`.
fn is_prerelease_identifier(prerelease: &[Identifier], identifier: &str) -> bool {
    let parts: Vec<&str> = identifier.split('.').collect();
    if parts.len() > prerelease.len() {
        return false;
    }
    for (i, part) in parts.iter().enumerate() {
        let want = Identifier::parse(part);
        if compare_identifiers(&prerelease[i], &want) != std::cmp::Ordering::Equal {
            return false;
        }
    }
    true
}

impl SemVer {
    /// Increment a version.
    ///
    /// `identifier` names the prerelease tag to use for the `pre*`
    /// releases — `inc("premajor", Some("beta"), _)` on `1.2.3` gives
    /// `2.0.0-beta.0`.
    pub fn inc(
        &self,
        release: Release,
        identifier: Option<&str>,
        base: IdentifierBase,
    ) -> Result<SemVer> {
        // A prerelease increment needs something to call the tag.
        if release.is_pre() {
            if identifier.is_none() && base == IdentifierBase::None {
                return Err(Error::InvalidIncrement {
                    kind: "identifier is empty".to_string(),
                });
            }
            if let Some(id) = identifier {
                // The identifier has to be a legal prerelease tag on its
                // own; the original checks this by matching `-{id}`
                // against the PRERELEASE pattern.
                if !is_valid_prerelease_tag(id, self.options().loose) {
                    return Err(Error::InvalidIncrement {
                        kind: format!("invalid identifier: {id}"),
                    });
                }
            }
        }

        let mut out = self.clone();

        match release {
            Release::PreMajor => {
                out.prerelease.clear();
                out.patch = 0;
                out.minor = 0;
                out.major += 1;
                out = out.inc(Release::Pre, identifier, base)?;
            }
            Release::PreMinor => {
                out.prerelease.clear();
                out.patch = 0;
                out.minor += 1;
                out = out.inc(Release::Pre, identifier, base)?;
            }
            Release::PrePatch => {
                // Any existing prerelease is irrelevant here — drop it
                // first so the patch bump is unconditional.
                out.prerelease.clear();
                out = out.inc(Release::Patch, identifier, base)?;
                out = out.inc(Release::Pre, identifier, base)?;
            }
            Release::PreRelease => {
                // On a plain version this behaves as prepatch.
                if out.prerelease.is_empty() {
                    out = out.inc(Release::Patch, identifier, base)?;
                }
                out = out.inc(Release::Pre, identifier, base)?;
            }
            Release::ReleaseOnly => {
                if out.prerelease.is_empty() {
                    return Err(Error::InvalidIncrement {
                        kind: format!("version {} is not a prerelease", out.version()),
                    });
                }
                out.prerelease.clear();
            }
            Release::Major => {
                // A prerelease of an x.0.0 is already before that
                // release, so releasing it *is* the major bump:
                // 1.0.0-5 → 1.0.0, but 1.1.0 → 2.0.0.
                if out.minor != 0 || out.patch != 0 || out.prerelease.is_empty() {
                    out.major += 1;
                }
                out.minor = 0;
                out.patch = 0;
                out.prerelease.clear();
            }
            Release::Minor => {
                if out.patch != 0 || out.prerelease.is_empty() {
                    out.minor += 1;
                }
                out.patch = 0;
                out.prerelease.clear();
            }
            Release::Patch => {
                if out.prerelease.is_empty() {
                    out.patch += 1;
                }
                out.prerelease.clear();
            }
            Release::Pre => {
                let base_value = base.value();

                if out.prerelease.is_empty() {
                    out.prerelease = vec![Identifier::Numeric(base_value)];
                } else {
                    // Bump the last numeric identifier, scanning from the
                    // right. If none exists, append one.
                    let mut bumped = false;
                    for id in out.prerelease.iter_mut().rev() {
                        if let Identifier::Numeric(n) = id {
                            *n += 1;
                            bumped = true;
                            break;
                        }
                    }
                    if !bumped {
                        let joined: Vec<String> =
                            out.prerelease.iter().map(|i| i.to_string()).collect();
                        if identifier == Some(joined.join(".").as_str())
                            && base == IdentifierBase::None
                        {
                            return Err(Error::InvalidIncrement {
                                kind: "identifier already exists".to_string(),
                            });
                        }
                        out.prerelease.push(Identifier::Numeric(base_value));
                    }
                }

                if let Some(id) = identifier {
                    // 1.2.0-beta.1 → 1.2.0-beta.2, but
                    // 1.2.0-beta.fooblz and 1.2.0-beta → 1.2.0-beta.0
                    let mut replacement: Vec<Identifier> =
                        id.split('.').map(Identifier::parse).collect();
                    if base != IdentifierBase::None {
                        replacement.push(Identifier::Numeric(base_value));
                    }

                    if is_prerelease_identifier(&out.prerelease, id) {
                        // The tag already matches; keep the bumped
                        // numeric suffix unless there isn't one.
                        let at = id.split('.').count();
                        let has_numeric_suffix = out
                            .prerelease
                            .get(at)
                            .map(|i| i.is_numeric())
                            .unwrap_or(false);
                        if !has_numeric_suffix {
                            out.prerelease = replacement;
                        }
                    } else {
                        out.prerelease = replacement;
                    }
                }
            }
        }

        Ok(out)
    }
}

/// Whether `tag` is a legal prerelease tag on its own.
///
/// The original tests `-{tag}` against the PRERELEASE pattern and
/// requires the captured group to equal the input, which rejects
/// anything that would need reinterpreting.
fn is_valid_prerelease_tag(tag: &str, loose: bool) -> bool {
    if tag.is_empty() {
        return false;
    }
    for part in tag.split('.') {
        if part.is_empty() {
            return false;
        }
        if !part
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        {
            return false;
        }
        // A numeric identifier may not carry a leading zero, except in
        // loose mode where it is coerced.
        if !loose
            && part.len() > 1
            && part.starts_with('0')
            && part.bytes().all(|b| b.is_ascii_digit())
        {
            return false;
        }
    }
    true
}
