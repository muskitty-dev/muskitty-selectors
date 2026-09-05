//! WPT `css/selectors/parsing` test suite harness.
//!
//! Drives the MusKitty selectors parser through the WPT selector parsing
//! fixtures (`tests/data/wpt/*.json`, extracted from upstream
//! `css/selectors/parsing/parse-*.html` + `invalid-pseudos.html`).
//!
//! Ported semantics (`css/support/parsing-testcommon.js`):
//! - `test_valid_selector(sel, ser)`      → strict parse must succeed.
//! - `test_valid_forgiving_selector(sel)` → strict parse must succeed
//!   (the selector itself is valid; only its forgivingly-parsed *contents*
//!   may fail to match — `CSS.supports()` semantics are not measurable
//!   here because this crate has no `:supports`-style query API).
//! - `test_invalid_selector(sel)`         → strict parse must fail.
//!
//! Serialization assertions recorded in the fixtures are deliberately NOT
//! asserted: muskitty-selectors does not implement selector serialization
//! (`selectorText`). Those cases contribute their validity assertion only.
//!
//! Run with:
//!   cargo test --test wpt_parsing -- --nocapture
//!
//! The harness is data-driven and never panics on an individual mismatch:
//! it collects all results, prints a report, and only asserts that the
//! fixture data was loaded.

use muskitty_selectors::parser::parse_a_selector;
use serde_json::Value;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq)]
enum Expectation {
    /// Strict parse must succeed (WPT `test_valid_selector` and
    /// `test_valid_forgiving_selector`).
    Valid,
    /// Strict parse must fail (WPT `test_invalid_selector`).
    Invalid,
}

struct Case {
    file: String,
    input: String,
    expected: Expectation,
    forgiving: bool,
}

fn load_file(path: &PathBuf) -> Vec<Case> {
    let text = fs::read_to_string(path).unwrap_or_else(|e| panic!("read {path:?}: {e}"));
    let root: Value = serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse {path:?}: {e}"));
    let source = root
        .get("source")
        .and_then(|v| v.as_str())
        .unwrap_or("?")
        .to_string();
    let cases = root
        .get("cases")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    cases
        .iter()
        .filter_map(|c| {
            let input = c.get("input")?.as_str()?.to_string();
            let forgiving = c.get("kind").and_then(|v| v.as_str()) == Some("forgiving");
            let kind = c.get("kind").and_then(|v| v.as_str())?;
            let expected = match kind {
                "valid" | "forgiving" => Expectation::Valid,
                "invalid" => Expectation::Invalid,
                _ => return None,
            };
            Some(Case {
                file: source.clone(),
                input,
                expected,
                forgiving,
            })
        })
        .collect()
}

struct CaseResult {
    passed: bool,
    detail: String,
}

fn run_case(c: &Case) -> CaseResult {
    let result = parse_a_selector(&c.input);
    let passed = match c.expected {
        Expectation::Valid => result.is_ok(),
        Expectation::Invalid => result.is_err(),
    };
    let detail = if passed {
        String::new()
    } else {
        match (&c.expected, result) {
            (Expectation::Valid, Err(e)) => format!("  expected valid, got error: {e:?}"),
            (Expectation::Invalid, Ok(_)) => "  expected invalid, but parsed".to_string(),
            _ => String::new(),
        }
    };
    CaseResult { passed, detail }
}

#[test]
fn wpt_selector_parsing_suite() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("data")
        .join("wpt");
    let mut entries: Vec<PathBuf> = fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("read wpt dir {dir:?}: {e}"))
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("json"))
        .collect();
    entries.sort();

    let mut total_pass = 0usize;
    let mut total_fail = 0usize;
    let mut per_file: Vec<(String, usize, usize)> = Vec::new();
    // Owned failure records: (file, input, kind-label, detail).
    let mut failures: Vec<(String, String, &'static str, String)> = Vec::new();

    for path in &entries {
        let cases = load_file(path);
        let name = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("?")
            .to_string();
        let mut file_pass = 0usize;
        let mut file_fail = 0usize;
        for case in &cases {
            let r = run_case(case);
            if r.passed {
                file_pass += 1;
            } else {
                file_fail += 1;
                let kind = if case.forgiving {
                    "forgiving-valid"
                } else {
                    match case.expected {
                        Expectation::Valid => "valid",
                        Expectation::Invalid => "invalid",
                    }
                };
                failures.push((case.file.clone(), case.input.clone(), kind, r.detail));
            }
        }
        total_pass += file_pass;
        total_fail += file_fail;
        per_file.push((name, file_pass, file_fail));
    }

    eprintln!("\n═══════════════════════════════════════════════════════════════");
    eprintln!(" WPT css/selectors/parsing — results (validity assertions)");
    eprintln!("═══════════════════════════════════════════════════════════════");
    eprintln!(
        " {:<48} {:>8} {:>8} {:>8}",
        "fixture", "pass", "fail", "total"
    );
    eprintln!(" ─────────────────────────────────────────────────────────────────");
    for (name, p, f) in &per_file {
        eprintln!(" {:<48} {:>8} {:>8} {:>8}", name, p, f, p + f);
    }
    eprintln!(" ─────────────────────────────────────────────────────────────────");
    let total = total_pass + total_fail;
    let pct = if total == 0 {
        0.0
    } else {
        100.0 * total_pass as f64 / total as f64
    };
    eprintln!(
        " {:<48} {:>8} {:>8} {:>8}   ({:.1}%)",
        "TOTAL", total_pass, total_fail, total, pct
    );
    eprintln!("\n── failures ──");
    for (file, input, kind, detail) in &failures {
        eprintln!("\n[{file}] {input:?} ({kind}){detail}");
    }
    eprintln!("═══════════════════════════════════════════════════════════════\n");

    assert!(
        total > 0,
        "no test cases were loaded — fixture data missing?"
    );
    eprintln!(
        "PASS RATE: {:.1}% ({}/{}) — informational; not asserting a hard threshold yet.",
        pct, total_pass, total
    );
}
