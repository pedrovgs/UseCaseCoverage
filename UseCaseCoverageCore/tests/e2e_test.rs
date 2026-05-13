use std::path::Path;
use use_case_coverage_core::collect_features_from;
use use_case_coverage_core::domain::FeatureDocument;

fn parse_scenario(scenario: &str) -> Vec<FeatureDocument> {
    let manifest_dir =
        std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR should be set");
    let root = Path::new(&manifest_dir).parent().unwrap().join("e2e").join(scenario);
    let mut features = collect_features_from(std::slice::from_ref(&root))
        .unwrap_or_else(|_| panic!("Should successfully parse the '{scenario}' scenario"));

    // Sort features explicitly to ensure deterministic snapshot order
    features.sort_by(|a, b| a.feature.id.cmp(&b.feature.id));

    // Sort artifacts to ensure deterministic order inside snapshots too
    for doc in &mut features {
        doc.artifacts.sort_by(|a, b| a.id.cmp(&b.id));
    }

    // Clean up absolute path roots since it makes snapshots non-portable
    for doc in &mut features {
        doc.source_path =
            doc.source_path.strip_prefix(&root).unwrap_or(&doc.source_path).to_path_buf();
    }

    // Strip filesystem-dependent timestamps to keep snapshots portable
    for doc in &mut features {
        doc.feature.last_modified_at = None;
        for artifact in &mut doc.artifacts {
            artifact.last_modified_at = None;
        }
    }

    features
}

#[test]
fn test_simple_e2e_scenario() {
    let features = parse_scenario("simple");
    insta::assert_yaml_snapshot!(features);
}

#[test]
fn test_multiple_use_cases_scenario() {
    let features = parse_scenario("multiple_use_cases");
    insta::assert_yaml_snapshot!(features);
}

#[test]
fn test_multiple_use_cases_and_bugs_scenario() {
    let features = parse_scenario("multiple_use_cases_and_bugs");
    insta::assert_yaml_snapshot!(features);
}

#[test]
fn test_multiple_features_scenario() {
    let features = parse_scenario("multiple_features");
    insta::assert_yaml_snapshot!(features);
}

#[test]
fn test_nested_features_scenario() {
    let features = parse_scenario("nested_features");
    insta::assert_yaml_snapshot!(features);
}

#[test]
fn test_only_bugs_scenario() {
    let features = parse_scenario("only_bugs");
    insta::assert_yaml_snapshot!(features);
}

#[test]
fn test_multiple_nested_features_scenario() {
    let features = parse_scenario("multiple_nested_features");
    insta::assert_yaml_snapshot!(features);
}
