use std::path::Path;
use use_case_coverage_core::collect_features_from;
use use_case_coverage_core::domain::FeatureDocument;

fn parse_scenario(scenario: &str) -> Vec<FeatureDocument> {
    let manifest_dir =
        std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR should be set");
    let root = Path::new(&manifest_dir).parent().unwrap().join("e2e").join(scenario);
    collect_features_from(&root)
        .unwrap_or_else(|_| panic!("Should successfully parse the '{}' scenario", scenario))
}

#[test]
fn test_simple_e2e_scenario() {
    let features = parse_scenario("simple");

    assert_eq!(features.len(), 1, "Expected exactly one feature");
    let document = &features[0];

    assert_eq!(document.feature.id, "feature-simple");
    assert_eq!(document.feature.title, "A simple feature");
    assert_eq!(document.artifacts.len(), 1, "Expected exactly one artifact");
    assert_eq!(document.artifacts[0].id, "ucc-001");
}

#[test]
fn test_multiple_use_cases_scenario() {
    let features = parse_scenario("multiple_use_cases");

    assert_eq!(features.len(), 1);
    assert_eq!(features[0].feature.id, "feature-multiple-use-cases");
    assert_eq!(features[0].artifacts.len(), 2, "Expected exactly two use cases");
}

#[test]
fn test_multiple_use_cases_and_bugs_scenario() {
    let features = parse_scenario("multiple_use_cases_and_bugs");

    assert_eq!(features.len(), 1);
    assert_eq!(features[0].feature.id, "feature-multiple-use-cases-and-bugs");
    assert_eq!(features[0].artifacts.len(), 4, "Expected two use cases and two bugs");
}

#[test]
fn test_multiple_features_scenario() {
    let mut features = parse_scenario("multiple_features");

    assert_eq!(features.len(), 2, "Expected two distinct features");

    // Sort to avoid flakiness since file reading order is not guaranteed
    features.sort_by(|a, b| a.feature.id.cmp(&b.feature.id));

    assert_eq!(features[0].feature.id, "feature-1");
    assert_eq!(features[0].artifacts.len(), 3);

    assert_eq!(features[1].feature.id, "feature-2");
    assert_eq!(features[1].artifacts.len(), 4);
}

#[test]
fn test_nested_features_scenario() {
    let mut features = parse_scenario("nested_features");

    assert_eq!(features.len(), 2, "Expected two features parsed from nested directories");

    features.sort_by(|a, b| a.feature.id.cmp(&b.feature.id));

    assert_eq!(features[0].feature.id, "nested-feature-a");
    assert_eq!(features[0].artifacts.len(), 2);

    assert_eq!(features[1].feature.id, "nested-feature-b");
    assert_eq!(features[1].artifacts.len(), 2);
}

#[test]
fn test_only_bugs_scenario() {
    let features = parse_scenario("only_bugs");

    assert_eq!(features.len(), 1);
    assert_eq!(features[0].feature.id, "feature-only-bugs");
    assert_eq!(features[0].artifacts.len(), 2, "Expected two bug artifacts");

    // Since bugs typically have an explicit type (e.g. 'bug', 'regression')
    assert!(
        features[0].artifacts.iter().all(|a| a.artifact_type.is_some()),
        "All artifacts should explicitly declare a type (since they are bugs)"
    );
}
