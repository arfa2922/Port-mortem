//! Parity against the original's own fixtures.
//!
//! `tests/fixtures.json` is a mechanical export of the fixture files in
//! the pinned `tests/original/fixtures/`, produced by
//! `scripts/export_fixtures.js`. The fixtures are plain data — arrays of
//! inputs and expected results — so the export is lossless and this file
//! asserts against exactly the cases the original asserts against.
//!
//! Nothing here is hand-curated. If a case fails, either the port is
//! wrong or the original's behaviour has been misread; there is no third
//! option where the test was simply written to agree with the port.
//!
//! Run: cargo test --test fixtures -- --nocapture

use semver_rs::{compare, eq, gt, valid, Options};
use serde_json::Value as J;
use std::path::Path;
use std::sync::OnceLock;

fn fixtures() -> &'static J {
    static F: OnceLock<J> = OnceLock::new();
    F.get_or_init(|| {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures.json");
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| {
            panic!(
                "cannot read {}: {e}\nrun: node scripts/export_fixtures.js",
                path.display()
            )
        });
        serde_json::from_str(&text).expect("fixtures.json is not valid JSON")
    })
}

fn group(name: &str) -> &'static Vec<J> {
    fixtures()
        .get(name)
        .unwrap_or_else(|| panic!("fixture group {name:?} missing"))
        .as_array()
        .unwrap_or_else(|| panic!("fixture group {name:?} is not an array"))
}

/// The original accepts `true`, `false`, `{}`, or `{ loose, includePrerelease }`
/// wherever options are taken. Normalize all four spellings.
fn opts(v: Option<&J>) -> Options {
    match v {
        None | Some(J::Null) => Options::new(),
        Some(J::Bool(b)) => Options::from(*b),
        Some(J::Object(o)) => Options::new()
            .with_loose(o.get("loose").and_then(|x| x.as_bool()).unwrap_or(false))
            .with_include_prerelease(
                o.get("includePrerelease")
                    .and_then(|x| x.as_bool())
                    .unwrap_or(false),
            ),
        Some(other) => panic!("unexpected options value in fixture: {other}"),
    }
}

fn s(v: &J) -> &str {
    v.as_str().unwrap_or_else(|| panic!("expected a string, got {v}"))
}

/// Report every failure rather than stopping at the first, so one run
/// shows the whole picture.
struct Report {
    name: &'static str,
    total: usize,
    failures: Vec<String>,
}

impl Report {
    fn new(name: &'static str) -> Self {
        Report {
            name,
            total: 0,
            failures: Vec::new(),
        }
    }

    fn check(&mut self, ok: bool, detail: impl FnOnce() -> String) {
        self.total += 1;
        if !ok {
            self.failures.push(detail());
        }
    }

    fn finish(self) {
        println!(
            "{:<28} {:>4}/{:<4} passed",
            self.name,
            self.total - self.failures.len(),
            self.total
        );
        if !self.failures.is_empty() {
            for f in self.failures.iter().take(15) {
                println!("    {f}");
            }
            if self.failures.len() > 15 {
                println!("    ... and {} more", self.failures.len() - 15);
            }
            panic!(
                "{}: {} of {} cases failed",
                self.name,
                self.failures.len(),
                self.total
            );
        }
    }
}

// ── valid-versions ──────────────────────────────────────────────────────

/// `[version, major, minor, patch, prerelease[], build[]]`
///
/// The fixture carries the whole expected decomposition, not just a
/// canonical string, so every field is asserted rather than just the
/// rendering.
#[test]
fn valid_versions() {
    let mut r = Report::new("valid-versions");
    for case in group("valid-versions") {
        let c = case.as_array().unwrap();
        let input = s(&c[0]);

        // Loose, because the fixture includes spellings like `v1.2.3`
        // and `=1.2.3` that only loose mode accepts.
        let Some(v) = semver_rs::parse(input, true) else {
            r.check(false, || format!("parse({input:?}) → None, expected a version"));
            continue;
        };

        let want_major = c[1].as_u64().unwrap();
        let want_minor = c[2].as_u64().unwrap();
        let want_patch = c[3].as_u64().unwrap();

        r.check(v.major == want_major, || {
            format!("{input:?}: major {}, expected {want_major}", v.major)
        });
        r.check(v.minor == want_minor, || {
            format!("{input:?}: minor {}, expected {want_minor}", v.minor)
        });
        r.check(v.patch == want_patch, || {
            format!("{input:?}: patch {}, expected {want_patch}", v.patch)
        });

        // Prerelease identifiers: the fixture holds numbers for numeric
        // identifiers and strings for the rest, exactly the distinction
        // Identifier makes.
        let want_pre = c[4].as_array().unwrap();
        r.check(v.prerelease.len() == want_pre.len(), || {
            format!(
                "{input:?}: {} prerelease identifiers, expected {}",
                v.prerelease.len(),
                want_pre.len()
            )
        });
        for (i, want) in want_pre.iter().enumerate() {
            let Some(got) = v.prerelease.get(i) else { break };
            let matches = match want {
                J::Number(n) => {
                    matches!(got, semver_rs::Identifier::Numeric(g) if Some(*g) == n.as_u64())
                }
                J::String(sv) => {
                    matches!(got, semver_rs::Identifier::Alphanumeric(g) if g == sv)
                }
                other => panic!("unexpected prerelease element in fixture: {other}"),
            };
            r.check(matches, || {
                format!("{input:?}: prerelease[{i}] = {got:?}, expected {want}")
            });
        }

        let want_build = c[5].as_array().unwrap();
        r.check(v.build.len() == want_build.len(), || {
            format!(
                "{input:?}: {} build identifiers, expected {}",
                v.build.len(),
                want_build.len()
            )
        });
        for (i, want) in want_build.iter().enumerate() {
            let Some(got) = v.build.get(i) else { break };
            let want_s = match want {
                J::String(sv) => sv.clone(),
                J::Number(n) => n.to_string(),
                other => panic!("unexpected build element in fixture: {other}"),
            };
            r.check(*got == want_s, || {
                format!("{input:?}: build[{i}] = {got:?}, expected {want_s:?}")
            });
        }
    }
    r.finish();
}

// ── invalid-versions ────────────────────────────────────────────────────

/// `[version, reason, options?]` — the reason is documentation only; what
/// matters is that the version is rejected.
#[test]
fn invalid_versions() {
    let mut r = Report::new("invalid-versions");
    for case in group("invalid-versions") {
        let c = case.as_array().unwrap();
        // A few entries carry a non-string first element to exercise the
        // original's type coercion, which has no meaning in Rust.
        let Some(input) = c[0].as_str() else { continue };
        let o = opts(c.get(2));

        let got = valid(input, o);
        r.check(got.is_none(), || {
            format!("valid({input:?}) → {got:?}, expected rejection")
        });
    }
    r.finish();
}

// ── comparisons ─────────────────────────────────────────────────────────

/// `[v1, v2, options?]` where v1 is strictly greater than v2.
#[test]
fn comparisons() {
    let mut r = Report::new("comparisons");
    for case in group("comparisons") {
        let c = case.as_array().unwrap();
        let (v1, v2) = (s(&c[0]), s(&c[1]));
        let o = opts(c.get(2));

        match compare(v1, v2, o) {
            Ok(ord) => r.check(ord == 1, || {
                format!("compare({v1:?}, {v2:?}) → {ord}, expected 1")
            }),
            Err(e) => r.check(false, || format!("compare({v1:?}, {v2:?}) errored: {e}")),
        }

        // The reverse must hold too — the original tests both directions.
        match compare(v2, v1, o) {
            Ok(ord) => r.check(ord == -1, || {
                format!("compare({v2:?}, {v1:?}) → {ord}, expected -1")
            }),
            Err(e) => r.check(false, || format!("compare({v2:?}, {v1:?}) errored: {e}")),
        }

        match gt(v1, v2, o) {
            Ok(g) => r.check(g, || format!("gt({v1:?}, {v2:?}) → false, expected true")),
            Err(e) => r.check(false, || format!("gt({v1:?}, {v2:?}) errored: {e}")),
        }
    }
    r.finish();
}

// ── equality ────────────────────────────────────────────────────────────

/// `[v1, v2, options?]` where the two compare equal despite differing
/// spelling — this is where build metadata and loose parsing show up.
#[test]
fn equality() {
    let mut r = Report::new("equality");
    for case in group("equality") {
        let c = case.as_array().unwrap();
        let (v1, v2) = (s(&c[0]), s(&c[1]));
        let o = opts(c.get(2));

        match eq(v1, v2, o) {
            Ok(e) => r.check(e, || format!("eq({v1:?}, {v2:?}) → false, expected true")),
            Err(e) => r.check(false, || format!("eq({v1:?}, {v2:?}) errored: {e}")),
        }

        match compare(v1, v2, o) {
            Ok(ord) => r.check(ord == 0, || {
                format!("compare({v1:?}, {v2:?}) → {ord}, expected 0")
            }),
            Err(e) => r.check(false, || format!("compare({v1:?}, {v2:?}) errored: {e}")),
        }
    }
    r.finish();
}

// ── ordering is a total order ───────────────────────────────────────────

/// Beyond matching the fixtures case by case, the comparison has to be a
/// consistent total order. The original does not test this; a fixture
/// list cannot, because it only ever compares pairs it was given.
#[test]
fn ordering_is_consistent() {
    let mut versions: Vec<&str> = Vec::new();
    for case in group("comparisons") {
        let c = case.as_array().unwrap();
        if let (Some(a), Some(b)) = (c[0].as_str(), c[1].as_str()) {
            versions.push(a);
            versions.push(b);
        }
    }
    versions.sort_unstable();
    versions.dedup();

    let parsed: Vec<_> = versions
        .iter()
        .filter_map(|v| semver_rs::parse(v, true).map(|p| (*v, p)))
        .collect();

    let mut r = Report::new("ordering-consistency");

    // Antisymmetry: compare(a, b) == -compare(b, a)
    for (na, a) in &parsed {
        for (nb, b) in &parsed {
            let ab = a.compare(b);
            let ba = b.compare(a);
            r.check(ab == ba.reverse(), || {
                format!("antisymmetry broken: {na:?} vs {nb:?} → {ab:?} and {ba:?}")
            });
        }
    }

    // Reflexivity: every version equals itself.
    for (n, v) in &parsed {
        r.check(v.compare(v) == std::cmp::Ordering::Equal, || {
            format!("{n:?} does not compare equal to itself")
        });
    }

    r.finish();
}

// ── round trip ──────────────────────────────────────────────────────────

/// Every version the port accepts must re-parse from its own canonical
/// form, to the same value. The original has no such test; it is the
/// property that catches a formatter and parser drifting apart.
#[test]
fn canonical_form_round_trips() {
    let mut r = Report::new("round-trip");

    let mut inputs: Vec<&str> = Vec::new();
    for g in ["valid-versions", "comparisons", "equality"] {
        for case in group(g) {
            for item in case.as_array().unwrap().iter().take(2) {
                if let Some(v) = item.as_str() {
                    inputs.push(v);
                }
            }
        }
    }
    inputs.sort_unstable();
    inputs.dedup();

    for input in inputs {
        let Some(first) = semver_rs::parse(input, true) else {
            continue;
        };
        let text = first.version();

        match semver_rs::parse(&text, false) {
            None => r.check(false, || {
                format!("canonical form {text:?} of {input:?} does not re-parse")
            }),
            Some(second) => {
                r.check(first.compare(&second) == std::cmp::Ordering::Equal, || {
                    format!("{input:?} → {text:?} → different value")
                });
                r.check(second.version() == text, || {
                    format!("formatting is not idempotent for {input:?}: {text:?} then {:?}", second.version())
                });
            }
        }
    }

    r.finish();
}

// ── range-parse ─────────────────────────────────────────────────────────

/// `[range, expectedNormalizedForm, options?]`
///
/// This is where the desugaring rules are checked: `^1.2.3` must expand
/// to exactly the bounds the original produces, character for character.
#[test]
fn range_parse() {
    let mut r = Report::new("range-parse");
    for case in group("range-parse") {
        let c = case.as_array().unwrap();
        let input = s(&c[0]);
        let o = opts(c.get(2));

        // A null expectation means the range is invalid.
        if c[1].is_null() {
            let got = semver_rs::valid_range(input, o);
            r.check(got.is_none(), || {
                format!("validRange({input:?}) → {got:?}, expected rejection")
            });
            continue;
        }

        let expected = s(&c[1]);
        let got = semver_rs::valid_range(input, o);
        r.check(got.as_deref() == Some(expected), || {
            format!("validRange({input:?}) → {got:?}, expected {expected:?}")
        });
    }
    r.finish();
}

// ── range-include ───────────────────────────────────────────────────────

/// `[range, version, options?]` — the version satisfies the range.
#[test]
fn range_include() {
    let mut r = Report::new("range-include");
    for case in group("range-include") {
        let c = case.as_array().unwrap();
        // A few entries pass a non-string version to exercise the
        // original's type coercion, which has no meaning in Rust.
        let (Some(range), Some(version)) = (c[0].as_str(), c[1].as_str()) else {
            continue;
        };
        let o = opts(c.get(2));

        let got = semver_rs::satisfies(version, range, o);
        r.check(got, || {
            format!("satisfies({version:?}, {range:?}) → false, expected true")
        });
    }
    r.finish();
}

// ── range-exclude ───────────────────────────────────────────────────────

/// `[range, version, options?]` — the version does NOT satisfy the range.
#[test]
fn range_exclude() {
    let mut r = Report::new("range-exclude");
    for case in group("range-exclude") {
        let c = case.as_array().unwrap();
        // A few entries pass a non-string version to exercise the
        // original's type coercion, which has no meaning in Rust.
        let (Some(range), Some(version)) = (c[0].as_str(), c[1].as_str()) else {
            continue;
        };
        let o = opts(c.get(2));

        let got = semver_rs::satisfies(version, range, o);
        r.check(!got, || {
            format!("satisfies({version:?}, {range:?}) → true, expected false")
        });
    }
    r.finish();
}

// ── increments ──────────────────────────────────────────────────────────

/// `[version, release, expected, loose?, identifier?, identifierBase?]`
///
/// A `null` expectation means the increment is rejected — either the
/// version does not parse, or the release kind is not one of the seven
/// the original accepts.
#[test]
fn increments() {
    use semver_rs::IdentifierBase;

    let mut r = Report::new("increments");
    for case in group("increments") {
        let c = case.as_array().unwrap();
        let Some(version) = c[0].as_str() else { continue };
        let Some(release) = c[1].as_str() else { continue };

        let loose = c.get(3).and_then(|x| x.as_bool()).unwrap_or(false);
        // The original tests the identifier for truthiness, so an empty
        // string means "no identifier" rather than an empty one.
        let identifier = c
            .get(4)
            .and_then(|x| x.as_str())
            .filter(|s| !s.is_empty());

        // identifierBase is '0', '1', or false — three distinct shapes.
        let base = match c.get(5) {
            None | Some(J::Null) => IdentifierBase::Zero,
            Some(J::Bool(false)) => IdentifierBase::None,
            Some(J::String(s)) if s == "1" => IdentifierBase::One,
            Some(J::String(s)) if s == "0" => IdentifierBase::Zero,
            Some(J::Number(n)) if n.as_u64() == Some(1) => IdentifierBase::One,
            Some(_) => IdentifierBase::Zero,
        };

        let got = semver_rs::inc(version, release, identifier, base, loose);

        if c[2].is_null() {
            r.check(got.is_none(), || {
                format!("inc({version:?}, {release:?}) → {got:?}, expected rejection")
            });
            continue;
        }

        let expected = s(&c[2]);
        r.check(got.as_deref() == Some(expected), || {
            format!(
                "inc({version:?}, {release:?}, id={identifier:?}, base={base:?}) \
                 → {got:?}, expected {expected:?}"
            )
        });
    }
    r.finish();
}

// ── intersection ────────────────────────────────────────────────────────

/// `[rangeA, rangeB, expected, options?]`
///
/// Both fixture files have the same shape; the comparator one uses
/// single comparators, the range one uses full ranges. `intersects`
/// handles both, so they share an assertion.
fn intersection_group(name: &'static str, report: &mut Report) {
    for case in group(name) {
        let c = case.as_array().unwrap();
        let (Some(a), Some(b)) = (c[0].as_str(), c[1].as_str()) else {
            continue;
        };
        let Some(expected) = c[2].as_bool() else {
            continue;
        };
        let o = opts(c.get(3));

        match semver_rs::intersects(a, b, o) {
            Ok(got) => report.check(got == expected, || {
                format!("intersects({a:?}, {b:?}) → {got}, expected {expected}")
            }),
            Err(e) => report.check(false, || format!("intersects({a:?}, {b:?}) errored: {e}")),
        }
    }
}

#[test]
fn comparator_intersection() {
    let mut r = Report::new("comparator-intersection");
    intersection_group("comparator-intersection", &mut r);
    r.finish();
}

#[test]
fn range_intersection() {
    let mut r = Report::new("range-intersection");
    intersection_group("range-intersection", &mut r);
    r.finish();
}

// ── truncations ─────────────────────────────────────────────────────────

/// `[version, truncation, expected, options?]`
///
/// A `null` expectation means either the version does not parse or the
/// truncation is not one of the seven release types.
#[test]
fn truncations() {
    let mut r = Report::new("truncations");
    for case in group("truncations") {
        let c = case.as_array().unwrap();
        let Some(version) = c[0].as_str() else { continue };
        let Some(truncation) = c[1].as_str() else { continue };
        let o = opts(c.get(3));

        let got = semver_rs::truncate(version, truncation, o);

        if c[2].is_null() {
            r.check(got.is_none(), || {
                format!("truncate({version:?}, {truncation:?}) → {got:?}, expected rejection")
            });
            continue;
        }

        let expected = s(&c[2]);
        r.check(got.as_deref() == Some(expected), || {
            format!("truncate({version:?}, {truncation:?}) → {got:?}, expected {expected:?}")
        });
    }
    r.finish();
}
