#![forbid(unsafe_code)]

pub mod domain;
pub mod infrastructure;
pub mod ports;
pub mod usecases;

use std::path::Path;

use domain::{ArtifactCoverageIndex, FeatureDocument};
use infrastructure::{LocalFileSystemRepository, LocalTestFileRepository, YamlUccParser};
use ports::CoreError;
use usecases::{CollectFeaturesUseCase, FindArtifactCoverageUseCase};

/// Calculates simple use case coverage as a percentage in the `0.0..=100.0` range.
#[must_use]
pub fn coverage_percentage(covered: u32, total: u32) -> f64 {
    if total == 0 {
        return 0.0;
    }

    (f64::from(covered.min(total)) / f64::from(total)) * 100.0
}

/// Facade for the main domain use case used by adapters (CLI later).
///
/// # Errors
///
/// Returns an error when file discovery, file reading, or parsing fails.
pub fn collect_features_from(root: &Path) -> Result<Vec<FeatureDocument>, CoreError> {
    let use_case = CollectFeaturesUseCase::new(LocalFileSystemRepository, YamlUccParser);
    use_case.execute(root)
}

/// Facade for finding test coverage mapped by artifact id.
///
/// # Errors
///
/// Returns an error when source discovery or file reading fails.
pub fn find_artifact_coverage(
    root: &Path,
    features: &[FeatureDocument],
) -> Result<ArtifactCoverageIndex, CoreError> {
    let use_case = FindArtifactCoverageUseCase::new(LocalTestFileRepository);
    use_case.execute(root, features)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use proptest::prelude::*;
    use tempfile::tempdir;

    use super::{collect_features_from, coverage_percentage};
    use crate::domain::Priority;

    #[test]
    fn simple_coverage_test() {
        assert!((coverage_percentage(5, 10) - 50.0).abs() < f64::EPSILON);
    }

    proptest! {
        #[test]
        fn coverage_is_bounded(covered in 0_u32..10_000, total in 0_u32..10_000) {
            let value = coverage_percentage(covered, total);
            prop_assert!((0.0..=100.0).contains(&value));
        }
    }

    #[test]
    fn collect_features_from_parses_all_ucc_files_recursively() {
        let temp = tempdir().expect("temporary directory should be created");
        let root = temp.path();

        fs::create_dir_all(root.join("nested/deep")).expect("nested directories should be created");

        fs::write(root.join("nested/ignore.txt"), "not a ucc file")
            .expect("text file should be written");

        fs::write(root.join("feature_alpha.ucc"), sample_ucc("feat-alpha", "Alpha Feature"))
            .expect("first ucc file should be written");
        fs::write(
            root.join("nested/deep/feature_beta.ucc"),
            sample_ucc_with_bug("feat-beta", "Beta Feature"),
        )
        .expect("second ucc file should be written");

        let result = collect_features_from(root).expect("use case should parse all ucc files");

        assert_eq!(result.len(), 2);
        assert_eq!(result[0].feature.id, "feat-alpha");
        assert_eq!(result[1].feature.id, "feat-beta");

        let beta = &result[1];
        assert_eq!(beta.platforms, vec!["android", "ios", "web"]);
        assert_eq!(beta.tags, vec!["tag-one", "tag-two"]);
        assert_eq!(beta.artifacts.len(), 2);
        assert_eq!(beta.artifacts[0].priority, Priority::Medium);
        assert!(beta.artifacts[0].related.is_empty());

        let bug = &beta.artifacts[1];
        assert_eq!(bug.id, "bug-001");
        assert_eq!(bug.artifact_type.as_deref(), Some("regression"));
        assert_eq!(bug.priority, Priority::None);
    }

    #[test]
    fn collect_features_from_returns_parse_error_for_invalid_ucc_file() {
        let temp = tempdir().expect("temporary directory should be created");
        let root = temp.path();

        fs::write(root.join("broken.ucc"), "feature: [this is not valid")
            .expect("broken ucc file should be written");

        let result = collect_features_from(root);

        assert!(result.is_err());
        let error_message = result.expect_err("invalid yaml should fail").to_string();
        assert!(error_message.contains("Parse error"));
        assert!(error_message.contains("broken.ucc"));
    }

    #[test]
    fn collect_features_from_defaults_optional_and_collection_fields_when_missing() {
        let temp = tempdir().expect("temporary directory should be created");
        let root = temp.path();

        fs::write(
            root.join("minimal.ucc"),
            r#"schema_version: "1.0"

feature:
  id: feat-minimal
  title: Minimal Feature
  created_at: "2026-05-10"
  description: >
    Minimal content.
"#,
        )
        .expect("minimal ucc file should be written");

        let result = collect_features_from(root).expect("minimal ucc should parse");
        let feature = &result[0];

        assert_eq!(feature.feature.updated_at, None);
        assert!(feature.tags.is_empty());
        assert!(feature.platforms.is_empty());
        assert!(feature.related_features.is_empty());
        assert!(feature.artifacts.is_empty());
    }

    fn sample_ucc(feature_id: &str, feature_title: &str) -> String {
        format!(
            r#"schema_version: "1.0"

feature:
  id: {feature_id}
  title: {feature_title}
  created_at: "2026-05-10"
  updated_at: "2026-05-10"
  description: >
    Example feature description.

tags:
  - tag-one
  - tag-two

platforms:
  - android
  - ios
  - web

related_features: []

artifacts:
  - id: ucc-001
    created_at: "2026-05-10"
    title: Primary use case
    priority: high
    steps:
      - Step 1
      - Step 2
    expected:
      - Expected 1
"#
        )
    }

    fn sample_ucc_with_bug(feature_id: &str, feature_title: &str) -> String {
        format!(
            r#"schema_version: "1.0"

feature:
  id: {feature_id}
  title: {feature_title}
  created_at: "2026-05-10"
  updated_at: "2026-05-10"
  description: >
    Another example feature.

tags:
  - tag-one
  - tag-two

platforms:
  - android
  - ios
  - web

related_features: []

artifacts:
  - id: ucc-002
    created_at: "2026-05-10"
    title: Another use case
    priority: medium
    steps:
      - Step A
    expected:
      - Expected A

  - id: bug-001
    type: regression
    created_at: "2026-05-10"
    title: Example bug
    severity: high
    status: open
    related:
      - ucc-002
    steps:
      - Repro step
    expected:
      - Should work
    actual:
      - Fails right now
"#
        )
    }
}
