//! Tests for the parser: every error code, the fixes, recovery, the nesting limit, speed and
//! robustness. The snapshot tests of trees and messages are in `tests/syntax.rs`.

use crate::{FileRole, NESTING_LIMIT, parse};

/// Inputs nested far beyond the limit, one per kind of nesting.
fn deep_inputs(depth: usize) -> Vec<(&'static str, String)> {
    vec![
        ("parentheses", format!("x = {}1{}", "(".repeat(depth), ")".repeat(depth))),
        ("lists", format!("x = {}1{}", "[".repeat(depth), "]".repeat(depth))),
        ("blocks", format!("x = {}1{}", "{".repeat(depth), "}".repeat(depth))),
        ("calls", format!("x = {}1{}", "f(".repeat(depth), ")".repeat(depth))),
        ("prefix minus", format!("x = {}1", "-".repeat(depth))),
        ("not", format!("x = {}y", "not ".repeat(depth))),
        ("try", format!("x = {}y", "try ".repeat(depth))),
        ("chain of additions", format!("x = 1{}", " + 1".repeat(depth))),
        ("chain of calls", format!("x = a{}", ".b()".repeat(depth))),
        ("coalesce", format!("x = a{}", " ?? a".repeat(depth))),
        ("else if", format!("x = if a {{ 1 }}{}", " else if a { 1 }".repeat(depth))),
        ("lambdas", format!("x = {}1", "a => ".repeat(depth))),
        ("ifs", format!("x = {}1{}", "if a { ".repeat(depth), " }".repeat(depth))),
        ("matches", format!("x = {}1{}", "match a { _ => ".repeat(depth), " }".repeat(depth))),
        ("strings", format!("x = {}{}", "\"{".repeat(depth), "}\"".repeat(depth))),
        ("types", format!("let x: {}Int{} = 1", "List<".repeat(depth), ">".repeat(depth))),
        ("function types", format!("let x: {}Int = 1", "() -> ".repeat(depth))),
        ("generic arguments", format!("x = f{}Int{}()", "<List".repeat(depth), ">".repeat(depth))),
        ("patterns", format!("match x {{ {}a{} => 1 }}", "(".repeat(depth), ")".repeat(depth))),
        (
            "payload patterns",
            format!("match x {{ {}a{} => 1 }}", "A(".repeat(depth), ")".repeat(depth)),
        ),
        ("binding patterns", format!("let {}a, b{} = 1", "(".repeat(depth), ", c)".repeat(depth))),
        ("ranges and comparisons", format!("x = {}1", "a < b and ".repeat(depth))),
    ]
}

#[test]
fn deep_nesting_is_reported_and_never_overflows_a_2_mib_stack() {
    // Run on a thread with exactly 2 MiB of stack, the smallest common default (a debug build
    // uses much more stack per call than a release build).
    let handle = std::thread::Builder::new()
        .stack_size(2 * 1024 * 1024)
        .spawn(|| {
            for (name, text) in deep_inputs(5_000) {
                let parse = parse(&text, FileRole::Entry);
                assert_eq!(parse.syntax().to_string(), text, "{name}: the tree is lossless");
                let codes: Vec<_> = parse.diagnostics().iter().filter_map(|d| d.code).collect();
                assert!(codes.contains(&"Q0106"), "{name}: {codes:?}");
                assert!(codes.len() <= 3, "{name}: one mistake gives one error: {codes:?}");
                drop(parse);
            }
            // Just below the limit, everything still parses.
            let depth = NESTING_LIMIT as usize / 8;
            for (name, text) in deep_inputs(depth) {
                let parse = parse(&text, FileRole::Entry);
                let codes: Vec<_> = parse.diagnostics().iter().filter_map(|d| d.code).collect();
                assert!(!codes.contains(&"Q0106"), "{name} at depth {depth}: {codes:?}");
            }
        })
        .expect("a thread");
    handle.join().expect("no stack overflow");
}
