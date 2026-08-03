//! CLI for semver-rs.

use std::process;

fn usage() -> ! {
    eprintln!("semver-rs — SemVer parsing in safe Rust");
    eprintln!();
    eprintln!("usage:");
    eprintln!("  semver-rs [--loose] <version> [version ...]   parse and print canonical form");
    eprintln!("  semver-rs --compare <a> <b> [--loose]         print -1, 0, or 1");
    eprintln!("  semver-rs --version");
    eprintln!();
    eprintln!("exit codes: 0 all valid · 1 one or more invalid · 2 usage error");
    process::exit(2)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        usage();
    }

    if args[0] == "--version" {
        println!("semver-rs {}", env!("CARGO_PKG_VERSION"));
        println!("semver spec: {}", semver_rs::constants::SEMVER_SPEC_VERSION);
        println!("port of: npm/node-semver (JavaScript)");
        println!("unsafe blocks: 0");
        return;
    }

    // `--loose` may appear anywhere.
    let loose = args.iter().any(|a| a == "--loose");
    let rest: Vec<&String> = args.iter().filter(|a| *a != "--loose").collect();

    if rest.is_empty() {
        usage();
    }

    if rest[0] == "--compare" {
        if rest.len() != 3 {
            eprintln!("--compare needs exactly two versions");
            process::exit(2);
        }
        match semver_rs::compare(rest[1], rest[2], loose) {
            Ok(ord) => {
                println!("{ord}");
                process::exit(0);
            }
            Err(e) => {
                eprintln!("{e}");
                process::exit(1);
            }
        }
    }

    if rest[0].starts_with("--") {
        eprintln!("unknown option '{}'", rest[0]);
        usage();
    }

    let mut all_valid = true;
    for v in &rest {
        match semver_rs::valid(v, loose) {
            Some(canonical) => println!("{canonical}"),
            None => {
                eprintln!("invalid: {v}");
                all_valid = false;
            }
        }
    }
    process::exit(if all_valid { 0 } else { 1 });
}
