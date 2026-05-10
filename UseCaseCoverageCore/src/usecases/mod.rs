use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::domain::{ArtifactCoverageIndex, ArtifactTestLocation, FeatureDocument};
use crate::ports::{CoreError, TestFileRepository, UccFileRepository, UccParser};

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

/// Use case that searches automated tests referencing artifact ids.
pub struct FindArtifactCoverageUseCase<R> {
    repository: R,
}

impl<R> FindArtifactCoverageUseCase<R>
where
    R: TestFileRepository,
{
    #[must_use]
    pub const fn new(repository: R) -> Self {
        Self { repository }
    }

    /// Executes artifact coverage lookup from source files under `root`.
    ///
    /// # Errors
    ///
    /// Returns an error when source discovery or file reads fail.
    pub fn execute(
        &self,
        root: &Path,
        features: &[FeatureDocument],
    ) -> Result<ArtifactCoverageIndex, CoreError> {
        let artifact_ids: HashSet<&str> = features
            .iter()
            .flat_map(|feature| feature.artifacts.iter().map(|artifact| artifact.id.as_str()))
            .collect();

        let mut test_files = self.repository.find_test_files(root)?;
        test_files.sort();

        let mut by_artifact_id: HashMap<String, Vec<ArtifactTestLocation>> = HashMap::new();

        for file_path in test_files {
            let lines = self.repository.read_lines(&file_path)?;
            let mut previous_was_test_attribute = false;

            for (line_idx, line) in lines.iter().enumerate() {
                let has_test_context =
                    looks_like_test_declaration(line) || previous_was_test_attribute;
                previous_was_test_attribute = line.to_ascii_lowercase().contains("#[test]")
                    || line.to_ascii_lowercase().contains("@test");

                if !has_test_context {
                    continue;
                }

                for artifact_id in &artifact_ids {
                    if line.contains(artifact_id) {
                        by_artifact_id.entry((*artifact_id).to_string()).or_default().push(
                            ArtifactTestLocation {
                                file_path: file_path.clone(),
                                line: line_idx + 1,
                            },
                        );
                    }
                }
            }
        }

        Ok(ArtifactCoverageIndex { by_artifact_id })
    }
}

fn looks_like_test_declaration(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();

    lower.contains("test(")
        || lower.contains("it(")
        || lower.contains("#[test]")
        || lower.contains("@test")
        || lower.contains("func test")
        || lower.contains("fun test")
        || lower.contains("fun `")
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, HashMap};
    use std::io;
    use std::path::{Path, PathBuf};

    use proptest::prelude::*;

    use super::{CollectFeaturesUseCase, FindArtifactCoverageUseCase};
    use crate::domain::{FeatureDocument, Priority};
    use crate::infrastructure::YamlUccParser;
    use crate::ports::{CoreError, TestFileRepository, UccFileRepository};

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
            let mut paths: Vec<PathBuf> = self
                .files
                .keys()
                .filter(|path| {
                    path.starts_with(root)
                        && path.extension().and_then(std::ffi::OsStr::to_str) == Some("ucc")
                })
                .cloned()
                .collect();
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

    impl TestFileRepository for InMemoryUccRepository {
        fn find_test_files(&self, root: &Path) -> Result<Vec<PathBuf>, CoreError> {
            let mut paths: Vec<PathBuf> = self
                .files
                .keys()
                .filter(|path| {
                    path.starts_with(root)
                        && matches!(
                            path.extension().and_then(std::ffi::OsStr::to_str),
                            Some("swift" | "ts" | "tsx" | "kt" | "kts" | "rs")
                        )
                })
                .cloned()
                .collect();
            paths.sort();
            Ok(paths)
        }

        fn read_lines(&self, path: &Path) -> Result<Vec<String>, CoreError> {
            self.files
                .get(path)
                .map(|content| content.lines().map(ToOwned::to_owned).collect())
                .ok_or_else(|| CoreError::Io {
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

    #[test]
    fn find_artifact_coverage_supports_multiple_languages_and_line_numbers() {
        let root = Path::new("/workspace");
        let repository = InMemoryUccRepository::default()
            .with_file(root.join("feature.ucc"), sample_document("feat-1", Priority::High))
            .with_file(
                root.join("ios/FeatureTests.swift"),
                "func test_ucc_feat_1_ucc_feat_1() {}\nfunc test_ucc-feat-1() {}\n".to_string(),
            )
            .with_file(
                root.join("web/feature.spec.ts"),
                "test('checks ucc-feat-1 flow', () => {});\n".to_string(),
            )
            .with_file(
                root.join("android/FeatureTest.kt"),
                "@Test\nfun `covers ucc-feat-1 scenario`() {}\n".to_string(),
            )
            .with_file(
                root.join("core/tests.rs"),
                "#[test]\nfn checks_ucc_feat_1() {}\n".to_string(),
            );

        let collect = CollectFeaturesUseCase::new(repository, YamlUccParser);
        let features = collect.execute(root).expect("features should parse");

        let finder = FindArtifactCoverageUseCase::new(collect.repository);
        let index = finder.execute(root, &features).expect("coverage search should work");

        let matches = index.for_artifact("ucc-feat-1");
        assert_eq!(matches.len(), 3);
        let mut lines: Vec<usize> = matches.iter().map(|location| location.line).collect();
        lines.sort_unstable();
        assert_eq!(lines, vec![1, 2, 2]);
    }

    proptest! {
        #[test]
        fn find_artifact_coverage_with_random_in_memory_test_files(
            suffix in 0_u16..5000,
            include_ts in any::<bool>(),
            include_swift in any::<bool>(),
            include_kotlin in any::<bool>(),
            include_rust in any::<bool>(),
        ) {
            let root = Path::new("/repo");
            let artifact_id = format!("ucc-{suffix}");
            let repository = InMemoryUccRepository::default()
                .with_file(root.join("feature.ucc"), sample_document("feat-random", Priority::Medium));

            let repository = if include_ts {
                repository.with_file(
                    root.join("web/feature.spec.ts"),
                    format!("test('covers {artifact_id}', () => {{}});\n"),
                )
            } else {
                repository
            };

            let repository = if include_swift {
                repository.with_file(
                    root.join("ios/FeatureTests.swift"),
                    format!("func test_{artifact_id}() {{}}\n"),
                )
            } else {
                repository
            };

            let repository = if include_kotlin {
                repository.with_file(
                    root.join("android/FeatureTest.kt"),
                    format!("@Test\nfun `covers {artifact_id}`() {{}}\n"),
                )
            } else {
                repository
            };

            let repository = if include_rust {
                repository.with_file(
                    root.join("core/tests.rs"),
                    format!("#[test]\nfn validates_{artifact_id}() {{}}\n"),
                )
            } else {
                repository
            };

            let collect = CollectFeaturesUseCase::new(repository, YamlUccParser);
            let mut features = collect.execute(root).expect("features should parse");
            features[0].artifacts[0].id = artifact_id.clone();

            let finder = FindArtifactCoverageUseCase::new(collect.repository);
            let index = finder.execute(root, &features).expect("coverage should be searchable");

            let expected = [include_ts, include_swift, include_kotlin, include_rust]
                .into_iter()
                .filter(|included| *included)
                .count();
            prop_assert_eq!(index.for_artifact(&artifact_id).len(), expected);
        }
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
