//! Differential fuzzing against the original.
//!
//! This is the check the fixtures cannot make. A fixture list proves the
//! port agrees with the original on the cases somebody thought to write
//! down; it says nothing about the cases nobody did.
//!
//! Here the original is run as a live oracle. Both implementations get
//! the same generated input and their answers are compared. Any
//! disagreement is a genuine behavioural divergence, not a guess about
//! what the original might do.
//!
//! Requires Node and a checkout under `vendor/node-semver`
//! (`bash scripts/fetch_original.sh`).
//!
//! Run:
//!   cargo run --release --example fuzz_differential
//!   cargo run --release --example fuzz_differential -- --cases 20000 --seed 42

use std::io::Write;
use std::process::{Command, Stdio};

// ── Seeded RNG, so any failure replays exactly ──────────────────────────

struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Rng(if seed == 0 { 0x2545F4914F6CDD1D } else { seed })
    }
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545F4914F6CDD1D)
    }
    fn below(&mut self, n: usize) -> usize {
        if n == 0 { 0 } else { (self.next() % n as u64) as usize }
    }
    fn pick<'a>(&mut self, xs: &'a [&'a str]) -> &'a str {
        xs[self.below(xs.len())]
    }
    fn chance(&mut self, pct: u64) -> bool {
        self.next() % 100 < pct
    }
}

// ── Generators ──────────────────────────────────────────────────────────

const PRE_WORDS: &[&str] = &[
    "alpha", "beta", "rc", "pre", "dev", "canary", "next", "0", "1", "2",
    "10", "01", "a", "Z", "x-y", "-", "0a", "00",
];

const BUILD_WORDS: &[&str] = &[
    "build", "001", "sha", "deadbeef", "0", "x", "a-b", "20260731",
];

/// Numbers chosen to sit on the boundaries that matter: zero, the safe
/// integer limit, and either side of it.
fn number(rng: &mut Rng) -> String {
    match rng.below(10) {
        0 => "0".to_string(),
        1 => "1".to_string(),
        2 => format!("{}", rng.below(10)),
        3 => format!("{}", rng.below(1000)),
        4 => format!("{}", rng.next() % 1_000_000),
        5 => "9007199254740991".to_string(), // MAX_SAFE_INTEGER
        6 => "9007199254740992".to_string(), // one past — must be rejected
        7 => format!("0{}", rng.below(100)), // leading zero
        8 => format!("{}", u32::MAX),
        _ => format!("{}", rng.next() % 100),
    }
}

/// A well-formed version, sometimes with a prerelease or build tag.
fn valid_version(rng: &mut Rng) -> String {
    let mut v = format!("{}.{}.{}", number(rng), number(rng), number(rng));

    if rng.chance(40) {
        v.push('-');
        let n = 1 + rng.below(3);
        for i in 0..n {
            if i > 0 {
                v.push('.');
            }
            v.push_str(rng.pick(PRE_WORDS));
        }
    }

    if rng.chance(25) {
        v.push('+');
        let n = 1 + rng.below(2);
        for i in 0..n {
            if i > 0 {
                v.push('.');
            }
            v.push_str(rng.pick(BUILD_WORDS));
        }
    }

    // Prefixes and whitespace that only loose mode tolerates.
    match rng.below(12) {
        0 => format!("v{v}"),
        1 => format!("={v}"),
        2 => format!(" {v} "),
        3 => format!("V{v}"),
        _ => v,
    }
}

/// Malformed input, aimed at the edges of the grammar.
fn invalid_version(rng: &mut Rng) -> String {
    match rng.below(16) {
        0 => String::new(),
        1 => "1".to_string(),
        2 => "1.2".to_string(),
        3 => "1.2.3.4".to_string(),
        4 => "a.b.c".to_string(),
        5 => "1.2.3-".to_string(),
        6 => "1.2.3+".to_string(),
        7 => "1.2.3-+".to_string(),
        8 => "-1.2.3".to_string(),
        9 => "1.-2.3".to_string(),
        10 => "1.2.3-alpha..1".to_string(),
        11 => format!("{}.0.0", "9".repeat(30)),
        12 => "x".repeat(300),
        13 => format!("1.2.3-{}", "a".repeat(300)),
        14 => "1.2.3 4.5.6".to_string(),
        _ => {
            // Corrupt a valid version at one byte.
            let mut b = valid_version(rng).into_bytes();
            if !b.is_empty() {
                let i = rng.below(b.len());
                b[i] = *rng.pick(&["!", "@", " ", ".", "-", "+", "~"]).as_bytes().first().unwrap();
            }
            String::from_utf8_lossy(&b).into_owned()
        }
    }
}

fn version(rng: &mut Rng) -> String {
    if rng.chance(70) {
        valid_version(rng)
    } else {
        invalid_version(rng)
    }
}

/// A range expression, covering every sugar the original supports.
fn range(rng: &mut Rng) -> String {
    let one = |rng: &mut Rng| -> String {
        let v = valid_version(rng);
        // Partial versions are where the X-range rules live.
        let partial = match rng.below(6) {
            0 => {
                let m = number(rng);
                m.to_string()
            }
            1 => format!("{}.{}", number(rng), number(rng)),
            2 => format!("{}.{}.x", number(rng), number(rng)),
            3 => format!("{}.x", number(rng)),
            4 => "*".to_string(),
            _ => v.clone(),
        };

        match rng.below(10) {
            0 => format!("^{partial}"),
            1 => format!("~{partial}"),
            2 => format!(">={partial}"),
            3 => format!("<={partial}"),
            4 => format!(">{partial}"),
            5 => format!("<{partial}"),
            6 => format!("={partial}"),
            7 => format!("{} - {}", partial, valid_version(rng)),
            8 => partial.to_string(),
            _ => format!("~>{partial}"),
        }
    };

    let mut parts = Vec::new();
    for _ in 0..1 + rng.below(3) {
        // A set may hold several space-joined comparators.
        let n = 1 + rng.below(2);
        let set: Vec<String> = (0..n).map(|_| one(rng)).collect();
        parts.push(set.join(" "));
    }
    parts.join(" || ")
}

// ── The oracle ──────────────────────────────────────────────────────────

/// One query for the original: parse a version, or compare two.
#[derive(serde::Serialize)]
#[serde(tag = "op")]
enum Query {
    #[serde(rename = "valid")]
    Valid { v: String, loose: bool },
    #[serde(rename = "compare")]
    Compare { a: String, b: String, loose: bool },
    #[serde(rename = "validRange")]
    ValidRange { r: String, loose: bool, ip: bool },
    #[serde(rename = "satisfies")]
    Satisfies {
        v: String,
        r: String,
        loose: bool,
        ip: bool,
    },
}

/// What the original answered. `null` means it rejected the input.
#[derive(serde::Deserialize, Debug, PartialEq)]
#[serde(untagged)]
enum Answer {
    Null(Option<()>),
    Bool(bool),
    Str(String),
    Int(i64),
}

/// Run every query through the original in one Node process.
///
/// Batching matters: spawning Node per case would cap throughput at a
/// few hundred cases a second and make a meaningful run impossible.
fn ask_original(queries: &[Query]) -> Result<Vec<Answer>, String> {
    let script = r#"
const path = require('path')
const semver = require(path.resolve('vendor/node-semver'))

let input = ''
process.stdin.on('data', c => input += c)
process.stdin.on('end', () => {
  const queries = JSON.parse(input)
  const out = queries.map(q => {
    try {
      if (q.op === 'valid') {
        return semver.valid(q.v, { loose: q.loose })
      }
      if (q.op === 'compare') {
        const a = semver.parse(q.a, { loose: q.loose })
        const b = semver.parse(q.b, { loose: q.loose })
        if (!a || !b) return null
        return semver.compare(a, b, { loose: q.loose })
      }
      if (q.op === 'validRange') {
        return semver.validRange(q.r, { loose: q.loose, includePrerelease: q.ip })
      }
      if (q.op === 'satisfies') {
        return semver.satisfies(q.v, q.r, { loose: q.loose, includePrerelease: q.ip })
      }
      return null
    } catch (e) {
      return null
    }
  })
  process.stdout.write(JSON.stringify(out))
})
"#;

    let mut child = Command::new("node")
        .arg("-e")
        .arg(script)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("cannot start node: {e}"))?;

    let payload = serde_json::to_string(queries).map_err(|e| e.to_string())?;
    child
        .stdin
        .as_mut()
        .ok_or("no stdin")?
        .write_all(payload.as_bytes())
        .map_err(|e| e.to_string())?;
    drop(child.stdin.take());

    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(format!(
            "node failed: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    serde_json::from_slice(&out.stdout).map_err(|e| {
        format!(
            "cannot parse node output: {e}\ngot: {}",
            String::from_utf8_lossy(&out.stdout).chars().take(200).collect::<String>()
        )
    })
}

// ── Main ────────────────────────────────────────────────────────────────

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut cases = 10_000usize;
    let mut seed = 0u64;
    // When set, overrides `cases` entirely: run for a wall-clock
    // duration instead of a fixed count. This is what the event's
    // Differential Fuzz Survivor bonus asks for -- "60 continuous
    // seconds" is a time budget, not a number of cases picked to
    // finish quickly.
    let mut seconds: Option<u64> = None;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--cases" | "-n" => {
                cases = args.get(i + 1).and_then(|s| s.parse().ok()).unwrap_or(cases);
                i += 2;
            }
            "--seed" | "-s" => {
                seed = args.get(i + 1).and_then(|s| s.parse().ok()).unwrap_or(0);
                i += 2;
            }
            "--seconds" => {
                seconds = args.get(i + 1).and_then(|s| s.parse().ok());
                i += 2;
            }
            _ => i += 1,
        }
    }
    if seed == 0 {
        seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x9E3779B97F4A7C15);
    }

    if !std::path::Path::new("vendor/node-semver").is_dir() {
        eprintln!("vendor/node-semver not found — run: bash scripts/fetch_original.sh");
        std::process::exit(2);
    }

    println!("semver-rs differential fuzzer");
    println!("  oracle: vendor/node-semver via node");
    println!("  seed:   {seed}  (replay with --seed {seed})");
    match seconds {
        Some(s) => println!("  mode:   continuous, {s}s wall-clock budget"),
        None => println!("  cases:  {cases}"),
    }
    println!();

    let mut rng = Rng::new(seed);
    let mut divergences: Vec<String> = Vec::new();
    let mut divergence_count = 0usize;
    let mut checked = 0usize;
    let started = std::time::Instant::now();

    const BATCH: usize = 2000;
    let mut done = 0;
    let deadline = seconds.map(|s| started + std::time::Duration::from_secs(s));
    let mut last_report = std::time::Instant::now();

    loop {
        if let Some(dl) = deadline {
            if std::time::Instant::now() >= dl {
                break;
            }
        } else if done >= cases {
            break;
        }
        let n = match deadline {
            Some(_) => BATCH,
            None => BATCH.min(cases - done),
        };
        let mut queries = Vec::with_capacity(n);
        let mut locals = Vec::with_capacity(n);

        for _ in 0..n {
            let loose = rng.chance(50);
            let ip = rng.chance(25);

            // A quarter of the budget goes to ranges, which are where
            // the desugaring rules live.
            if rng.chance(25) {
                let r = range(&mut rng);
                let o = semver_rs::Options::new()
                    .with_loose(loose)
                    .with_include_prerelease(ip);
                let local = match semver_rs::valid_range(&r, o) {
                    Some(s) => Answer::Str(s),
                    None => Answer::Null(None),
                };
                queries.push(Query::ValidRange {
                    r: r.clone(),
                    loose,
                    ip,
                });
                locals.push((format!("validRange({r:?}, loose={loose}, ip={ip})"), local));
                continue;
            }

            if rng.chance(20) {
                let r = range(&mut rng);
                let v = version(&mut rng);
                let o = semver_rs::Options::new()
                    .with_loose(loose)
                    .with_include_prerelease(ip);
                let local = Answer::Bool(semver_rs::satisfies(&v, &r, o));
                queries.push(Query::Satisfies {
                    v: v.clone(),
                    r: r.clone(),
                    loose,
                    ip,
                });
                locals.push((
                    format!("satisfies({v:?}, {r:?}, loose={loose}, ip={ip})"),
                    local,
                ));
                continue;
            }

            if rng.chance(60) {
                let v = version(&mut rng);
                let local = semver_rs::valid(&v, loose);
                queries.push(Query::Valid {
                    v: v.clone(),
                    loose,
                });
                locals.push((format!("valid({v:?}, loose={loose})"), match local {
                    Some(s) => Answer::Str(s),
                    None => Answer::Null(None),
                }));
            } else {
                let a = version(&mut rng);
                let b = version(&mut rng);
                let local = match (semver_rs::parse(&a, loose), semver_rs::parse(&b, loose)) {
                    (Some(x), Some(y)) => Answer::Int(match x.compare(&y) {
                        std::cmp::Ordering::Less => -1,
                        std::cmp::Ordering::Equal => 0,
                        std::cmp::Ordering::Greater => 1,
                    }),
                    _ => Answer::Null(None),
                };
                queries.push(Query::Compare {
                    a: a.clone(),
                    b: b.clone(),
                    loose,
                });
                locals.push((format!("compare({a:?}, {b:?}, loose={loose})"), local));
            }
        }

        let answers = match ask_original(&queries) {
            Ok(a) => a,
            Err(e) => {
                eprintln!("oracle error: {e}");
                std::process::exit(1);
            }
        };

        if answers.len() != locals.len() {
            eprintln!(
                "oracle returned {} answers for {} queries",
                answers.len(),
                locals.len()
            );
            std::process::exit(1);
        }

        for ((desc, ours), theirs) in locals.into_iter().zip(answers) {
            checked += 1;
            if ours != theirs {
                divergence_count += 1;
                // Keep only the first few for display; the count is what
                // matters and an unbounded list would exhaust memory.
                if divergences.len() < 20 {
                    divergences.push(format!("{desc}\n    ours:     {ours:?}\n    original: {theirs:?}"));
                }
            }
        }

        done += n;
        let elapsed = started.elapsed();
        if last_report.elapsed().as_secs() >= 5 {
            match deadline {
                Some(_) => println!(
                    "  {:>6.1}s, {done:>8} cases, {divergence_count} divergences",
                    elapsed.as_secs_f64()
                ),
                None => {
                    if done % 10_000 == 0 {
                        println!("  {done:>7} cases, {divergence_count} divergences");
                    }
                }
            }
            last_report = std::time::Instant::now();
        }
    }

    let secs = started.elapsed().as_secs_f64();
    println!();
    println!("── Results ──────────────────────────────────────────────");
    println!("  cases:       {checked}");
    println!("  duration:    {secs:.1}s");
    println!("  throughput:  {:.0} cases/s", checked as f64 / secs);
    println!("  divergences: {divergence_count}");

    if divergence_count > 0 {
        println!();
        println!("── Divergences (first {}) ───────────────────────────────", divergences.len());
        for d in &divergences {
            println!("  {d}");
        }
        std::process::exit(1);
    }

    println!();
    println!("  The port and the original agreed on every case.");
}
