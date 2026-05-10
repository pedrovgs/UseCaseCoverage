use std::path::Path;

use crate::domain::FeatureDocument;
use crate::ports::{CoreError, UccFileRepository, UccParser};

/// Use case that discovers and parses all `.ucc` files from a root folder recursively.
pub struct CollectFeaturesUseCase<R, P> {
    repository: R,
    parser: P,
}

impl<R, P> CollectFeaturesUseCase<R, P>
where
    R: UccFileRepository,
    P: UccParser,
{
    #[must_use]
    pub const fn new(repository: R, parser: P) -> Self {
        Self { repository, parser }
    }

    /// Executes the use case from a root folder.
    ///
    /// # Errors
    ///
    /// Returns an error when file discovery, file reading, or YAML parsing fails.
    pub fn execute(&self, root: &Path) -> Result<Vec<FeatureDocument>, CoreError> {
        let mut paths = self.repository.find_ucc_files(root)?;
        paths.sort();

        paths
            .into_iter()
            .map(|path| {
                let content = self.repository.read_file(&path)?;
                self.parser.parse(&path, &content)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, HashMap};
    use std::io;
    use std::path::{Path, PathBuf};

    use proptest::prelude::*;

    use super::CollectFeaturesUseCase;
    use crate::domain::{FeatureDocument, Priority};
    use crate::infrastructure::YamlUccParser;
    use crate::ports::{CoreError, UccFileRepository};

    #[derive(Default)]
    struct InMemoryUccRepository {
        files: HashMap<PathBuf, String>,
    }

    impl InMemoryUccRepository {
        fn with_file(mut self, path: impl Into<PathBuf>, content: String) -> Self {
            self.files.insert(path.into(), content);
            self
        }
    }

    impl UccFileRepository for InMemoryUccRepository {
        fn find_ucc_files(&self, root: &Path) -> Result<Vec<PathBuf>, CoreError> {
            let mut paths: Vec<PathBuf> =
                self.files.keys().filter(|path| path.starts_with(root)).cloned().collect();
            paths.sort();
            Ok(paths)
        }

        fn read_file(&self, path: &Path) -> Result<String, CoreError> {
            self.files.get(path).cloned().ok_or_else(|| CoreError::Io {
                path: path.to_path_buf(),
                source: io::Error::new(io::ErrorKind::NotFound, "file not found in repository"),
            })
        }
    }

    #[test]
    fn execute_returns_error_when_any_document_is_invalid() {
        let root = Path::new("/project");
        let repository = InMemoryUccRepository::default()
            .with_file(root.join("feature_ok.ucc"), sample_document("feat-ok", Priority::High))
            .with_file(root.join("feature_broken.ucc"), "schema_version: [".to_string());
        let use_case = CollectFeaturesUseCase::new(repository, YamlUccParser);

        let result = use_case.execute(root);

        assert!(result.is_err());
        let message = result.expect_err("must fail").to_string();
        assert!(message.contains("feature_broken.ucc"));
    }

    proptest! {
        #[test]
        fn execute_parses_random_in_memory_documents(
            feature_suffixes in prop::collection::btree_set(0_u16..5000, 1..6),
            priorities in prop::collection::vec(
                prop_oneof![
                    Just(Priority::None),
                    Just(Priority::Low),
                    Just(Priority::Medium),
                    Just(Priority::High),
                    Just(Priority::Highest),
                ],
                1..6
            )
        ) {
            let root = Path::new("/workspace");
            let total = feature_suffixes.len().min(priorities.len());
            let mut expected_by_feature_id: BTreeMap<String, Priority> = BTreeMap::new();

            let repository = feature_suffixes
                .into_iter()
                .zip(priorities.into_iter())
                .take(total)
                .fold(InMemoryUccRepository::default(), |repo, (suffix, priority)| {
                    let feature_id = format!("feat-{suffix}");
                    let file_name = format!("{feature_id}.ucc");
                    let file_path = root.join("nested").join(file_name);
                    let content = sample_document(&feature_id, priority);
                    expected_by_feature_id.insert(feature_id, priority);
                    repo.with_file(file_path, content)
                });

            let use_case = CollectFeaturesUseCase::new(repository, YamlUccParser);
            let result: Vec<FeatureDocument> = use_case.execute(root).expect("documents should parse");

            prop_assert_eq!(result.len(), total);
            for document in &result {
                let expected_priority = expected_by_feature_id.get(&document.feature.id);
                prop_assert!(expected_priority.is_some());
                prop_assert_eq!(document.artifacts.len(), 2);
                prop_assert_eq!(document.artifacts[0].priority, *expected_priority.unwrap_or(&Priority::None));
                prop_assert!(document.artifacts[1].related.is_empty());
                prop_assert!(document.feature.updated_at.is_none());
                prop_assert!(!document.tags.is_empty());
                prop_assert!(!document.platforms.is_empty());
            }
        }
    }

    fn sample_document(feature_id: &str, priority: Priority) -> String {
        format!(
            r#"schema_version: "1.0"

feature:
  id: {feature_id}
  title: Feature {feature_id}
  created_at: "2026-05-10"
  description: >
    Feature generated for in-memory tests.

tags:
  - generated

platforms:
  - android

artifacts:
  - id: ucc-{feature_id}
    created_at: "2026-05-10"
    title: Generated use case
    priority: {priority}
    steps:
      - Step 1
    expected:
      - Expected 1

  - id: bug-{feature_id}
    type: regression
    created_at: "2026-05-10"
    title: Generated bug
    steps:
      - Repro
    expected:
      - Should pass
"#
        )
    }
}
