//! End-to-end test against a fixture project.
//!
//! This is the test I explicitly flagged as non-negotiable: it's the only
//! one that exercises the path-matching logic across the complexity and
//! coverage passes. Unit tests for each layer can pass while the pipeline
//! is silently broken, because the two layers disagree about what a "path"
//! is until you wire them together.

#![expect(
    clippy::float_cmp,
    reason = "CC and coverage are deterministic from the fixture; exact equality is the right comparison"
)]

use cargo_crap::complexity;
use cargo_crap::coverage;
use cargo_crap::merge::{MissingCoveragePolicy, merge};
use std::path::PathBuf;

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sample_project")
}

#[test]
fn end_to_end_pipeline_produces_ranked_scores() {
    let root = fixture_root();

    // 1. Complexity pass over the fixture crate.
    let complexity =
        complexity::analyze_tree(&root.join("src"), &[] as &[&str]).expect("analyze_tree");
    let names: Vec<_> = complexity.iter().map(|f| f.name.as_str()).collect();
    assert!(names.contains(&"trivial"), "trivial fn not found");
    assert!(names.contains(&"moderate"), "moderate fn not found");
    assert!(names.contains(&"crappy"), "crappy fn not found");

    // 2. Parse the fixture LCOV file, which uses *relative* paths — this
    //    is exactly the mismatch case path_has_suffix must handle.
    let coverage = coverage::parse_lcov(&root.join("lcov.info")).expect("parse_lcov");
    assert!(
        !coverage.is_empty(),
        "parsed LCOV should have at least one file"
    );

    // 3. Merge with pessimistic policy (matches what a CI gate would do).
    let entries = merge(complexity, coverage, MissingCoveragePolicy::Pessimistic).entries;
    assert!(!entries.is_empty(), "merge produced no entries");

    // 4. Verify ordering: crappy must outrank moderate must outrank trivial.
    let by_name: std::collections::HashMap<_, _> =
        entries.iter().map(|e| (e.function.as_str(), e)).collect();
    let trivial = by_name.get("trivial").expect("trivial in results");
    let moderate = by_name.get("moderate").expect("moderate in results");
    let crappy = by_name.get("crappy").expect("crappy in results");

    assert!(
        crappy.crap > moderate.crap,
        "crappy ({}) should outrank moderate ({})",
        crappy.crap,
        moderate.crap
    );
    assert!(
        moderate.crap > trivial.crap,
        "moderate ({}) should outrank trivial ({})",
        moderate.crap,
        trivial.crap
    );

    // 5. Verify the top of the list is `crappy` — the first entry after
    //    sort must be the highest score.
    assert_eq!(
        entries[0].function, "crappy",
        "expected 'crappy' at top of ranked list, got {}",
        entries[0].function
    );

    // 6. trivial() should match the theoretical minimum.
    assert_eq!(
        trivial.cyclomatic, 1.0,
        "trivial should have CC=1, got {}",
        trivial.cyclomatic
    );

    // 7. Path matching must have succeeded — every function has a
    //    coverage number, not None. If the suffix match broke, we'd see
    //    None here and the assertion would catch it.
    for entry in &entries {
        assert!(
            entry.coverage.is_some(),
            "path matching failed for {} — coverage is None",
            entry.function
        );
    }
}

#[test]
fn json_output_round_trips() {
    // A user piping `cargo crap --format json` into another tool shouldn't
    // get invalid JSON, even with floats like CRAP scores that could
    // otherwise serialize as NaN.
    let root = fixture_root();
    let complexity =
        complexity::analyze_tree(&root.join("src"), &[] as &[&str]).expect("analyze_tree");
    let coverage = coverage::parse_lcov(&root.join("lcov.info")).expect("parse_lcov");
    let entries = merge(complexity, coverage, MissingCoveragePolicy::Pessimistic).entries;

    let mut buf = Vec::new();
    cargo_crap::report::render(
        &entries,
        30.0,
        cargo_crap::report::Format::Json,
        None,
        &mut buf,
    )
    .expect("render");

    // If this parses, the output is well-formed.
    let parsed: serde_json::Value =
        serde_json::from_slice(&buf).expect("JSON output must be parseable");
    assert!(parsed.is_object(), "JSON output must be an envelope object");
    assert!(
        parsed["entries"].is_array(),
        "envelope must contain an `entries` array"
    );
}

fn cfg_gated_fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/cfg_gated_project")
}

#[test]
fn cfg_gated_uncompiled_variant_excluded_from_output() {
    let root = cfg_gated_fixture_root();

    // 1. Complexity pass — syn sees BOTH cfg-gated variants of do_thing.
    let complexity =
        complexity::analyze_tree(&root.join("src"), &[] as &[&str]).expect("analyze_tree");
    let do_thing_count = complexity.iter().filter(|f| f.name == "do_thing").count();
    assert_eq!(
        do_thing_count, 2,
        "syn should parse both cfg variants of do_thing, found {do_thing_count}"
    );

    // Verify that both cfg variants are detected as cfg_gated.
    for fc in complexity.iter().filter(|f| f.name == "do_thing") {
        assert!(
            fc.cfg_gated,
            "do_thing at line {} should be cfg_gated",
            fc.start_line
        );
    }

    // always_here should NOT be cfg_gated.
    let always = complexity
        .iter()
        .find(|f| f.name == "always_here")
        .expect("always_here must exist");
    assert!(!always.cfg_gated, "always_here should not be cfg_gated");

    // 2. Parse the fixture LCOV — only has DA records for the first variant.
    let coverage = coverage::parse_lcov(&root.join("lcov.info")).expect("parse_lcov");

    // 3. Merge — the uncompiled variant should be excluded.
    let entries = merge(complexity, coverage, MissingCoveragePolicy::Pessimistic).entries;

    let do_thing_entries: Vec<_> = entries
        .iter()
        .filter(|e| e.function == "do_thing")
        .collect();
    assert_eq!(
        do_thing_entries.len(),
        1,
        "only the compiled cfg variant should survive merge, got {}",
        do_thing_entries.len()
    );

    // The surviving entry should be the first variant (whose lines have DA records).
    let surviving = &do_thing_entries[0];
    assert!(
        surviving.coverage.is_some(),
        "surviving variant must have coverage data"
    );

    // always_here should also be present.
    assert!(
        entries.iter().any(|e| e.function == "always_here"),
        "always_here must appear in output"
    );
}
