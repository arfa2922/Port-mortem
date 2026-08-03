//! WASM bindings for semver-rs, used only by `web/demo.html`.
//!
//! Separate crate rather than adding `wasm-bindgen` to the core library:
//! the port's dependency count and `#![forbid(unsafe_code)]`-style
//! guarantees stay about the *port*, not about a browser demo layered
//! on top of it. Nothing in `src/` knows this crate exists.
//!
//! Every function here is a thin pass-through to the real API in
//! `semver_rs` — no logic is reimplemented, so what the browser
//! exercises is the actual port, not a JS-facing approximation of it.

use semver_rs::Options;
use wasm_bindgen::prelude::*;

fn opts(loose: bool, include_prerelease: bool) -> Options {
    Options::new()
        .with_loose(loose)
        .with_include_prerelease(include_prerelease)
}

/// Parse a version, returning its canonical form or `null`.
#[wasm_bindgen(js_name = valid)]
pub fn valid(version: &str, loose: bool) -> Option<String> {
    semver_rs::valid(version, loose)
}

/// Compare two versions: -1, 0, or 1. Returns `null` if either is invalid.
#[wasm_bindgen(js_name = compare)]
pub fn compare(a: &str, b: &str, loose: bool) -> Option<i32> {
    semver_rs::compare(a, b, loose).ok().map(|n| n as i32)
}

/// Whether `version` satisfies `range`.
#[wasm_bindgen(js_name = satisfies)]
pub fn satisfies(version: &str, range: &str, loose: bool, include_prerelease: bool) -> bool {
    semver_rs::satisfies(version, range, opts(loose, include_prerelease))
}

/// A range's canonical desugared form, or `null` if invalid.
#[wasm_bindgen(js_name = validRange)]
pub fn valid_range(range: &str, loose: bool, include_prerelease: bool) -> Option<String> {
    semver_rs::valid_range(range, opts(loose, include_prerelease))
}

/// Increment a version. `release` is one of the seven release kinds
/// ("major", "preminor", etc). Returns `null` on any invalid input.
#[wasm_bindgen(js_name = inc)]
pub fn inc(version: &str, release: &str, loose: bool) -> Option<String> {
    semver_rs::inc(
        version,
        release,
        None,
        semver_rs::IdentifierBase::Zero,
        loose,
    )
}

/// Structured parse result for the demo's "inspect" panel.
#[derive(serde::Serialize)]
struct ParsedInfo {
    major: u64,
    minor: u64,
    patch: u64,
    prerelease: Vec<String>,
    build: Vec<String>,
    version: String,
}

/// Parse a version and return its components as a JS object, or
/// `undefined` if invalid.
#[wasm_bindgen(js_name = inspect)]
pub fn inspect(version: &str, loose: bool) -> JsValue {
    match semver_rs::parse(version, loose) {
        Some(v) => {
            let info = ParsedInfo {
                major: v.major,
                minor: v.minor,
                patch: v.patch,
                prerelease: v.prerelease.iter().map(|i| i.to_string()).collect(),
                build: v.build.clone(),
                version: v.version(),
            };
            serde_wasm_bindgen::to_value(&info).unwrap_or(JsValue::UNDEFINED)
        }
        None => JsValue::UNDEFINED,
    }
}

/// The crate version string, for the demo page's footer.
#[wasm_bindgen(js_name = portVersion)]
pub fn port_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}
