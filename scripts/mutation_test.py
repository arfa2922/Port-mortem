#!/usr/bin/env python3
"""
Mutation testing — manually curated, not an automated tool.

No tool like cargo-mutants was run to generate an exhaustive mutant set.
These four mutations were hand-picked as the smallest edits that would
flip observable comparison or parsing behavior without failing to
compile — the kind of bug a careless refactor could actually introduce.
Each is applied, the fixture suite is run, and the mutation is reverted.

This is a targeted correctness check on the test suite itself: it asks
"if this specific logic were wrong, would 2,515 assertions notice?" —
not a claim of exhaustive mutation coverage.

Usage:
    python3 scripts/mutation_test.py
"""
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SEMVER_RS = ROOT / "src" / "semver.rs"
IDENTIFIERS_RS = ROOT / "src" / "identifiers.rs"
RANGE_RS = ROOT / "src" / "range.rs"

# Each mutation: (file, description, old text, new text).
# old text must appear exactly once in the file.
MUTATIONS = [
    (
        SEMVER_RS,
        "reverse prerelease precedence (no-prerelease should outrank a prerelease)",
        "(false, true) => Ordering::Less,",
        "(false, true) => Ordering::Greater,",
    ),
    (
        IDENTIFIERS_RS,
        "swap which side wins when comparing numeric vs alphanumeric identifiers",
        "(Numeric(_), Alphanumeric(_)) => Ordering::Less,",
        "(Numeric(_), Alphanumeric(_)) => Ordering::Greater,",
    ),
    (
        SEMVER_RS,
        "reverse main version comparison (major.minor.patch)",
        ".then_with(|| self.minor.cmp(&other.minor))",
        ".then_with(|| other.minor.cmp(&self.minor))",
    ),
    (
        RANGE_RS,
        "invert the null-set check in a comparator set (an unsatisfiable range would look satisfiable)",
        "if let Some(null) = comparators.iter().find(|c| c.is_null_set()) {",
        "if let Some(null) = comparators.iter().find(|c| !c.is_null_set()) {",
    ),
]


def run(cmd, cwd=ROOT):
    return subprocess.run(cmd, cwd=cwd, capture_output=True, text=True, encoding="utf-8")


def run_suite():
    """Run the fixture suite. Returns (passed, output_tail)."""
    r = run(["cargo", "test", "--test", "fixtures"])
    passed = r.returncode == 0
    tail = "\n".join((r.stdout + r.stderr).splitlines()[-8:])
    return passed, tail


def main():
    print("Mutation testing — 4 manually curated mutations\n")

    # Baseline: everything must pass before mutating anything.
    baseline_ok, tail = run_suite()
    if not baseline_ok:
        print("FAIL: baseline fixture suite does not pass before mutation.")
        print(tail)
        sys.exit(1)
    print("baseline: fixture suite passes\n")

    caught = 0
    results = []

    for path, description, old, new in MUTATIONS:
        text = path.read_text(encoding="utf-8")
        count = text.count(old)
        if count != 1:
            print(f"SKIP: {description}")
            print(f"      expected exactly 1 occurrence of the target text in {path.name}, found {count}")
            results.append((description, "skipped (target text not found)"))
            continue

        mutated = text.replace(old, new, 1)
        path.write_text(mutated, encoding="utf-8")

        try:
            passed, tail = run_suite()
        finally:
            # Always revert, even if the test run itself errors.
            path.write_text(text, encoding="utf-8")

        if passed:
            print(f"NOT CAUGHT: {description}")
            print(f"    ({path.name}) — fixture suite still passed with this mutation")
            results.append((description, "NOT CAUGHT"))
        else:
            caught += 1
            print(f"caught: {description}")
            results.append((description, "caught"))

    # Confirm the revert actually took — the suite should pass again.
    reverted_ok, tail = run_suite()
    print()
    if not reverted_ok:
        print("FAIL: suite does not pass after reverting mutations. A revert did not apply cleanly.")
        print(tail)
        sys.exit(1)
    print("confirmed: all mutations reverted, suite passes again\n")

    print(f"{caught}/{len(MUTATIONS)} mutations caught by the fixture suite")
    for desc, status in results:
        marker = "PASS" if status == "caught" else "!!"
        print(f"  [{marker}] {desc}: {status}")

    if caught != len(MUTATIONS):
        print("\nNot all mutations were caught — see above.")
        sys.exit(1)


if __name__ == "__main__":
    main()
