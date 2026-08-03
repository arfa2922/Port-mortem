# Upstream bug: `simplifyRange` returns `""` when nothing matches

Found while differential-testing this port against node-semver.
Reported for Port Mortem 2026, Bug Catcher.

**Repository:** https://github.com/npm/node-semver
**File:** `ranges/simplify.js`
**Severity:** the failure mode inverts the meaning of the range

---

## Summary

When no version in the supplied list satisfies the range,
`simplifyRange` returns the empty string. In semver an empty range means
`*` — it matches everything. So a range that matched **nothing**
simplifies to one that matches **everything**.

---

## Reproduction

```js
const semver = require('semver')

const versions = ['1.0.0', '2.0.0']
const range = '^5.0.0'

versions.filter(v => semver.satisfies(v, range))
// []  — nothing matches, as expected

const simplified = semver.simplifyRange(versions, range)
// ''

versions.filter(v => semver.satisfies(v, simplified))
// ['1.0.0', '2.0.0']  — everything matches

semver.validRange('')
// '*'
```

---

## Why this is a bug

The README states the function returns

> a "simplified" range that matches the same items in the `versions` list
> as the range does

Here it matches strictly more: all of them instead of none. The
documented contract is violated in the one direction that matters, since
the result is permissive rather than restrictive.

---

## Root cause

`ranges/simplify.js`:

```js
const ranges = []
for (const [min, max] of set) {
  // ... never entered when `set` is empty
}
const simplified = ranges.join(' || ')          //  ''
const original = typeof range.raw === 'string' ? range.raw : String(range)
return simplified.length < original.length ? simplified : range
```

When nothing satisfies the range, `set` stays empty, so `ranges` is
empty and `join` yields `''`. The length comparison then prefers `''`
over any non-empty input, because `0 < original.length` always holds.

The empty string is never checked against what it means as a range.

---

## Suggested fix

Return the canonical empty range instead of the empty string:

```js
 const simplified = ranges.join(' || ')
+
+// An empty result means nothing matched. The empty string parses as
+// `*`, which matches everything — the opposite of what was computed.
+if (simplified === '') {
+  return '<0.0.0-0'
+}
+
 const original = typeof range.raw === 'string' ? range.raw : String(range)
 return simplified.length < original.length ? simplified : range
```

`<0.0.0-0` is the form the library already uses elsewhere for a range
that cannot be satisfied, so callers comparing against it will behave
consistently.

Returning `range` unchanged would also be defensible and is a smaller
change, though it gives up the simplification in a case where the
simplified form is genuinely shorter.

---

## Impact

Anything that simplifies a range and then tests versions against the
result will silently start accepting versions it previously rejected.
The likely shapes are dependency resolvers, registry queries, and CI
version gates — the same places where a range that matches nothing is a
meaningful signal.

The failure is quiet: no exception, no warning, and a return value that
is a valid range.

---

## How it was found

`examples/fuzz_differential.rs` in this port checks properties rather
than fixtures. One of them is the contract above: simplify a range, then
confirm the same versions match before and after. A seeded generator
found the first counterexample within a few thousand cases, and every
counterexample shared the same shape — a range that nothing satisfied.

The library's own test suite covers `simplifyRange` only with ranges
that match at least one version, which is why 982 fixture cases never
reach it.

---

## This port

`semver-rs` returns `<0.0.0-0` in this case, with a regression test in
`src/functions.rs`.

---
---

# Upstream bug 2: `gtr` and `ltr` both return `true`

**File:** `ranges/outside.js`
**Severity:** returns a logically impossible answer

---

## Summary

For a version excluded from a range by the prerelease rule rather than by
the range's bounds, `gtr` and `ltr` both return `true` — the version is
reported as simultaneously above and below the same range.

---

## Reproduction

```js
const semver = require('semver')

semver.satisfies('1.1.2-b', '^1.1.0')  // false
semver.gtr('1.1.2-b', '^1.1.0')        // true
semver.ltr('1.1.2-b', '^1.1.0')        // true
```

`1.1.2-b` sits numerically inside `^1.1.0`, which spans `1.1.0` up to
`2.0.0`. It fails to satisfy only because a prerelease needs a matching
prerelease in the range to be admitted.

More cases from the same search:

```
version        range                    gtr    ltr
4.5.4-rc.1     4.x                      true   true
2.0.0-b        >=0.1.1-b <2.0.0         true   true
3.2.4-b        >=0.1.0 <4.0.0           true   true
2.2.2-0        ^2.1.1                   true   true
```

---

## Why this is a bug

The README says:

> `gtr(version, range)`: Return `true` if version is greater than all the
> versions possible in the range.
>
> `ltr(version, range)`: Return `true` if version is less than all the
> versions possible in the range.

Both cannot hold for the same version and range. `1.1.2-b` is neither
above nor below `^1.1.0` — it is inside its bounds and excluded for a
different reason.

---

## Root cause

`outside()` establishes the range's bounds from comparator operators
alone:

```js
if (satisfies(version, range, options)) {
  return false
}

for (const comparators of range.set) {
  // ... find `high` and `low` by operator
  if (high.operator === comp || high.operator === ecomp) return false
  if ((!low.operator || low.operator === comp) && ltefn(version, low.semver)) return false
  // ...
}
return true
```

The early `satisfies` check catches versions inside the range. Everything
after it assumes that failing `satisfies` means falling outside the
bounds. For a prerelease that assumption does not hold: the version can
be within the bounds and still be excluded.

Called with `'>'` the bound checks do not fire, so it returns `true`.
Called with `'<'` the mirrored checks also do not fire, so it returns
`true` again.

---

## Suggested fix

After the `satisfies` check, distinguish exclusion-by-bounds from
exclusion-by-prerelease:

```js
 if (satisfies(version, range, options)) {
   return false
 }

+// A prerelease can fail `satisfies` while still lying within the
+// range's bounds. Re-test with prereleases admitted: if it satisfies
+// then, it is inside the range and neither above nor below it.
+if (version.prerelease.length && !options?.includePrerelease) {
+  const withPre = new Range(range.raw, { ...options, includePrerelease: true })
+  if (satisfies(version, withPre, { ...options, includePrerelease: true })) {
+    return false
+  }
+}
```

That makes `gtr` and `ltr` mutually exclusive again, and leaves the
behaviour for non-prerelease versions unchanged.

---

## Impact

Smaller than bug 1 — `gtr` and `ltr` are less used than `simplifyRange`
— but the failure mode is worse in kind: an impossible answer rather than
a wrong one. Code branching on `gtr(v, r) ? A : ltr(v, r) ? B : C` takes
branch `A` for a version that is in neither category.

---

## How it was found

By testing a documented contract rather than comparing outputs. The
property asserted was that `satisfies`, `gtr`, and `ltr` partition the
space: for any version and any satisfiable range, exactly one holds. A
seeded generator found counterexamples within a few thousand cases, all
sharing the same shape — a prerelease excluded by the prerelease rule.

Note that ranges nothing can satisfy, such as `>=4.2.2 <1.0.0`, also
report all three as `false`. That is defensible: "greater than every
version in the range" has no meaning when the range contains none.

---

## This port

`semver-rs` re-tests with prereleases admitted before deciding, so `gtr`
and `ltr` remain mutually exclusive. Regression test in
`src/relations.rs`.
