# Demo video — script

**Target: 5 minutes.** Screen recording plus your voice.

Record with OBS (free, obsproject.com) or Windows Game Bar (`Win + G`).

Before recording: open PowerShell, `cd` into the project, clear the
screen, go full screen, bump the font size until it reads comfortably on
a laptop.

---

## Structure

| Time | Section | Why it's there |
|---|---|---|
| 0:00–0:25 | What this is | Context, fast |
| 0:25–1:10 | The original's own tests | Proves parity |
| 1:10–2:20 | The live oracle | **The method** |
| 2:20–3:20 | Two bugs in the original | **The best find** |
| 3:20–4:10 | A bug in mine, found by timing | Shows rigour |
| 4:10–4:40 | Zero unsafe, 9x faster | The Bun contrast |
| 4:40–5:00 | Close | |

The two bug sections are what people remember. Do not rush them.

---

## 0:00–0:25 — What this is

> This is node-semver — npm's version parsing library — ported from
> JavaScript to Rust. About 2,900 lines. The port isn't the interesting
> part. How I proved it behaves like the original is.

```powershell
Get-Content README.md -TotalCount 12
```

Let the numbers sit for a beat, then move.

---

## 0:25–1:10 — The original's own tests

> The original ships fixture files — plain arrays of inputs and expected
> outputs. I export them to JSON mechanically and assert against them
> directly. Nothing hand-picked, nothing retyped. If these pass, the port
> agrees with what the original's authors wrote down.

```powershell
cargo test --test fixtures -- --nocapture
```

> Twenty-five hundred assertions across thirteen fixture groups. Every
> fixture the original ships.

---

## 1:10–2:20 — The live oracle

This is what distinguishes the submission. Slow down here.

> But fixtures only cover the cases someone thought to write down. So I
> also run the original itself, as an oracle. The same generated input
> goes to both implementations and I compare the answers. A disagreement
> is a real behavioural difference — not a guess about what the original
> might do.

```powershell
cargo run --release --example fuzz_differential -- --cases 50000 --seed 42
```

> Fifty thousand cases against Node in about two seconds. Zero
> disagreements on version parsing.

> It found nine real divergences the fixtures never covered. Here's the
> one I like most.

```powershell
node -e "const s=require('./vendor/node-semver'); const p=s.parse('90071992547.0991.59.145515',{loose:true}); console.log('patch:', p.patch, '  prerelease:', JSON.stringify(p.prerelease))"
```

> Look at that. The digits of "five nine" get split — five becomes the
> patch, nine goes into the prerelease. That's the loose-mode regex
> backtracking to make an optional group match. It reads like an accident
> of the pattern. But it's observable, so the port reproduces it
> deliberately.

```powershell
.\target\release\semver-rs.exe --loose 90071992547.0991.59.145515
```

> Same answer. Reading the source would never have told me this.

---

## 2:20–3:20 — Two bugs in the original

The strongest thirty seconds in the video.

> Running the original doesn't only find where I'm wrong. It finds where
> the original is wrong.

```powershell
node -e "const s=require('./vendor/node-semver'); const vs=['1.0.0','2.0.0']; console.log('versions:  ', JSON.stringify(vs)); console.log('range:      ^5.0.0'); console.log('matches:   ', JSON.stringify(vs.filter(v=>s.satisfies(v,'^5.0.0'))));"
```

> Nothing matches. Now simplify that range.

```powershell
node -e "const s=require('./vendor/node-semver'); const vs=['1.0.0','2.0.0']; const simp=s.simplifyRange(vs,'^5.0.0'); console.log('simplified:', JSON.stringify(simp)); console.log('matches:   ', JSON.stringify(vs.filter(v=>s.satisfies(v,String(simp)))));"
```

> Empty string. And in semver an empty range means star — it matches
> everything.
>
> So a range that matched **nothing** simplifies into one that matches
> **everything**. No exception, no warning, and the return value is a
> perfectly valid range.
>
> The library's own tests only ever call simplifyRange with ranges that
> match at least one version, which is why nine hundred fixture cases
> never reach it. A property test found it in a few thousand.

> My port returns the canonical empty range instead, with a regression
> test.

> There's a second one. A version cannot be both above and below the same
> range — but:

```powershell
node -e "const s=require('./vendor/node-semver'); console.log('satisfies:', s.satisfies('1.1.2-b','^1.1.0')); console.log('gtr:      ', s.gtr('1.1.2-b','^1.1.0')); console.log('ltr:      ', s.ltr('1.1.2-b','^1.1.0'))"
```

> Both true. That version sits numerically inside the range and fails only
> the prerelease rule — but `outside()` checks comparator operators alone,
> so neither bound test fires and both directions answer yes.
>
> Root causes and suggested patches for both are in UPSTREAM_BUG.md.

---

## 3:20–4:10 — A bug in mine, found by timing

> The oracle checks correctness. It says nothing about speed. So there's
> a second harness that asserts no input takes disproportionately long.

```powershell
cargo run --release --example timing_safety
```

> That found something the correctness tests could not. A range with five
> thousand alternatives took **four and a half seconds** — the duplicate
> check was re-rendering every earlier alternative on each iteration.
> Quadratic.
>
> Made linear, the same range parses in twelve milliseconds. Three
> hundred and forty times faster.
>
> Every one of those twenty-five hundred fixtures passed before the fix
> and after it. Only timing found it. And it was a real denial-of-service
> vector for anything parsing user-supplied ranges.

---

## 4:10–4:40 — Zero unsafe, and speed

> One more thing. Bun's Zig-to-Rust rewrite merged with thirteen thousand
> unsafe blocks. This port has zero.

```powershell
Select-String -Path src\*.rs -Pattern "^\s*unsafe\s"
```

Nothing prints.

> No output. And it's about nine times faster than the original on
> sustained parsing — same corpus, same budget, both warmed up.

```powershell
Get-Content bench\results.md | Select-String "semver-rs|node-semver|ratio"
```

---

## 4:40–5:00 — Close

> To summarise. Twenty-five hundred assertions from the original's own
> fixtures, all passing. Every one of its forty-one exports ported. Half
> a million differential cases against the running original. Zero unsafe.
> One bug found in the original, one found in mine.
>
> The method is the point: pin the suite, export the fixtures, run the
> original as an oracle, and fix whatever it tells you. It works on any
> repo.
>
> Code's on GitHub. Thanks for watching.

---

## Recording notes

- **Rehearse once without recording.** The second take is always better.
- **Don't rush the two bug sections.** Everything else is table stakes.
- **Font big.** If they can't read the terminal, the demo does nothing.
- **Test your mic** before the real take.
- If you fumble a line, pause and say it again — cut it later or leave it.

## Upload

YouTube **unlisted** is simplest. Put the link in your submission and at
the top of the README:

```markdown
## Demo
https://youtu.be/YOUR_VIDEO_ID
```
