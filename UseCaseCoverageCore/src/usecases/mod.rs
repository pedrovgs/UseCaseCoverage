use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use aho_corasick::AhoCorasick;
use rayon::prelude::*;

use crate::domain::{
    ArtifactCoverageIndex, ArtifactTestLocation, FeatureDocument, UccLintIssue, UccLintResult,
};
use crate::ports::{CoreError, TestFileRepository, UccFileRepository, UccParser};

fn deduplicate_roots(roots: &[PathBuf]) -> Vec<PathBuf> {
    let mut unique_roots = HashSet::new();
    for root in roots {
        if let Ok(canonical) = root.canonicalize() {
            unique_roots.insert(canonical);
        } else {
            unique_roots.insert(root.clone());
        }
    }

    let mut sorted_roots: Vec<PathBuf> = unique_roots.into_iter().collect();
    sorted_roots.sort_by_key(|p| p.as_os_str().len());

    let mut final_roots = Vec::new();
    for root in sorted_roots {
        if !final_roots.iter().any(|r| root.starts_with(r)) {
            final_roots.push(root);
        }
    }
    final_roots
}

/// Use case that discovers and parses all `.ucc` files from a root folder recursively.
pub struct CollectFeaturesUseCase<R, P> {
    repository: R,
    parser: P,
}

impl<R, P> CollectFeaturesUseCase<R, P>
where
    R: Sync + UccFileRepository,
    P: Sync + UccParser,
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
    pub fn execute(&self, roots: &[PathBuf]) -> Result<Vec<FeatureDocument>, CoreError> {
        let roots = deduplicate_roots(roots);
        let mut all_paths = HashSet::new();
        for root in &roots {
            let paths = self.repository.find_ucc_files(root)?;
            for path in paths {
                all_paths.insert(path);
            }
        }
        let mut paths: Vec<PathBuf> = all_paths.into_iter().collect();
        paths.sort();

        let total = paths.len();
        println!("📂 Found {total} .ucc files to parse...");

        let count = std::sync::atomic::AtomicUsize::new(0);
        paths
            .par_iter()
            .map(|path| {
                let current = count.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                if current % 50 == 0 || current == total {
                    println!("⏳ Parsed {current}/{total} .ucc files...");
                }
                let content = self.repository.read_file(path)?;
                self.parser.parse(path, &content)
            })
            .collect()
    }
}

/// Use case that searches automated tests referencing artifact ids.
pub struct FindArtifactCoverageUseCase<R> {
    repository: R,
}

/// Use case that validates the format of all discovered `.ucc` files.
pub struct LintUccFormatsUseCase<R, P> {
    repository: R,
    parser: P,
}

impl<R, P> LintUccFormatsUseCase<R, P>
where
    R: Sync + UccFileRepository,
    P: Sync + UccParser,
{
    #[must_use]
    pub const fn new(repository: R, parser: P) -> Self {
        Self { repository, parser }
    }

    /// Lints every `.ucc` file under `root` and returns a per-file analysis result.
    ///
    /// # Errors
    ///
    /// Returns an error when file discovery or file reading fails.
    pub fn execute(&self, roots: &[PathBuf]) -> Result<Vec<UccLintResult>, CoreError> {
        let roots = deduplicate_roots(roots);
        let mut all_paths = HashSet::new();
        for root in &roots {
            let paths = self.repository.find_ucc_files(root)?;
            for path in paths {
                all_paths.insert(path);
            }
        }
        let mut paths: Vec<PathBuf> = all_paths.into_iter().collect();
        paths.sort();

        let total = paths.len();
        println!("📂 Found {total} .ucc files to lint...");

        let count = std::sync::atomic::AtomicUsize::new(0);
        paths
            .par_iter()
            .map(|path| {
                let current = count.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                if current % 50 == 0 || current == total {
                    println!("⏳ Linted {current}/{total} .ucc files...");
                }
                let content = self.repository.read_file(path)?;
                match self.parser.parse(path, &content) {
                    Ok(_) => {
                        Ok(UccLintResult { file_path: path.clone(), is_valid: true, issue: None })
                    }
                    Err(CoreError::Parse { reason, .. }) => {
                        let (line, column) = extract_line_and_column(&reason);
                        Ok(UccLintResult {
                            file_path: path.clone(),
                            is_valid: false,
                            issue: Some(UccLintIssue {
                                message: reason.clone(),
                                line,
                                column,
                                suggestion: infer_suggestion(&reason),
                            }),
                        })
                    }
                    Err(other) => Err(other),
                }
            })
            .collect()
    }
}

impl<R> FindArtifactCoverageUseCase<R>
where
    R: Sync + TestFileRepository,
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
        roots: &[PathBuf],
        features: &[FeatureDocument],
    ) -> Result<ArtifactCoverageIndex, CoreError> {
        let roots = deduplicate_roots(roots);
        let artifact_matcher = ArtifactIdMatcher::from_features(features);

        let mut all_test_files = HashSet::new();
        for root in &roots {
            let paths = self.repository.find_test_files(root)?;
            for path in paths {
                all_test_files.insert(path);
            }
        }

        let mut test_files: Vec<PathBuf> = all_test_files.into_iter().collect();
        test_files.sort();

        let total = test_files.len();
        println!("📂 Found {total} test files to scan for coverage...");

        let count = std::sync::atomic::AtomicUsize::new(0);
        let matches_by_file: Vec<Vec<(String, ArtifactTestLocation)>> = test_files
            .par_iter()
            .map(|file_path| {
                let current = count.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                if current % 100 == 0 || current == total {
                    println!("⏳ Scanned {current}/{total} files...");
                }
                let lines = self.repository.read_lines(file_path)?;
                let mut matches = Vec::new();
                let mut previous_was_test_attribute = false;

                for (line_idx, line) in lines.iter().enumerate() {
                    let has_test_context =
                        looks_like_test_declaration(line) || previous_was_test_attribute;
                    previous_was_test_attribute = line.to_ascii_lowercase().contains("#[test]")
                        || line.to_ascii_lowercase().contains("@test");

                    if !has_test_context {
                        continue;
                    }

                    for artifact_id in artifact_matcher.find_artifact_ids(line) {
                        matches.push((
                            artifact_id,
                            ArtifactTestLocation {
                                file_path: file_path.clone(),
                                line: line_idx + 1,
                            },
                        ));
                    }
                }

                Ok(matches)
            })
            .collect::<Result<Vec<_>, CoreError>>()?;

        println!("✅ Coverage scan completed.");

        let mut by_artifact_id: HashMap<String, Vec<ArtifactTestLocation>> = HashMap::new();
        for (artifact_id, location) in matches_by_file.into_iter().flatten() {
            by_artifact_id.entry(artifact_id).or_default().push(location);
        }

        for locations in by_artifact_id.values_mut() {
            locations.sort_by(|left, right| {
                left.file_path.cmp(&right.file_path).then_with(|| left.line.cmp(&right.line))
            });
        }

        Ok(ArtifactCoverageIndex { by_artifact_id })
    }
}

struct ArtifactIdMatcher {
    matcher: Option<AhoCorasick>,
    artifact_ids_by_pattern_index: Vec<String>,
}

impl ArtifactIdMatcher {
    fn from_features(features: &[FeatureDocument]) -> Self {
        let artifact_ids: HashSet<&str> = features
            .iter()
            .flat_map(|feature| feature.artifacts.iter().map(|artifact| artifact.id.as_str()))
            .collect();

        let mut patterns = Vec::new();
        let mut artifact_ids_by_pattern_index = Vec::new();

        for artifact_id in artifact_ids {
            for pattern in artifact_id_patterns(artifact_id) {
                patterns.push(pattern);
                artifact_ids_by_pattern_index.push(artifact_id.to_string());
            }
        }

        let matcher = if patterns.is_empty() {
            None
        } else {
            Some(
                AhoCorasick::builder()
                    .ascii_case_insensitive(true)
                    .build(patterns)
                    .expect("artifact id patterns are valid"),
            )
        };

        Self { matcher, artifact_ids_by_pattern_index }
    }

    fn find_artifact_ids(&self, line: &str) -> Vec<String> {
        let Some(matcher) = &self.matcher else {
            return Vec::new();
        };

        let mut artifact_ids = HashSet::new();
        for matched in matcher.find_iter(line) {
            if let Some(artifact_id) =
                self.artifact_ids_by_pattern_index.get(matched.pattern().as_usize())
            {
                artifact_ids.insert(artifact_id.clone());
            }
        }

        artifact_ids.into_iter().collect()
    }
}

fn artifact_id_patterns(artifact_id: &str) -> Vec<String> {
    let normalized = normalize_artifact_id_for_test_name(artifact_id);
    if normalized == artifact_id {
        vec![artifact_id.to_string()]
    } else {
        vec![artifact_id.to_string(), normalized]
    }
}

fn normalize_artifact_id_for_test_name(artifact_id: &str) -> String {
    artifact_id
        .chars()
        .map(|character| if character.is_ascii_alphanumeric() { character } else { '_' })
        .collect()
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

fn extract_line_and_column(reason: &str) -> (Option<usize>, Option<usize>) {
    let tokens: Vec<&str> = reason
        .split(|character: char| character.is_whitespace() || [',', ':'].contains(&character))
        .collect();

    let mut line = None;
    let mut column = None;

    for (index, token) in tokens.iter().enumerate() {
        if *token == "line" {
            line = tokens.get(index + 1).and_then(|value| value.parse::<usize>().ok());
        }
        if *token == "column" {
            column = tokens.get(index + 1).and_then(|value| value.parse::<usize>().ok());
        }
    }

    (line, column)
}

fn infer_suggestion(reason: &str) -> Option<String> {
    let lower = reason.to_ascii_lowercase();

    if lower.contains("cannot start any token") {
        return Some("Check YAML syntax near the reported location: fix invalid characters, indentation, or unclosed quotes/brackets.".to_string());
    }
    if lower.contains("did not find expected key") {
        return Some("Ensure mapping keys end with ':' and indentation is consistent (2 spaces recommended).".to_string());
    }
    if lower.contains("invalid type") {
        return Some(
            "Verify field types and structure match the .ucc schema (for example, list vs string)."
                .to_string(),
        );
    }

    None
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, HashMap};
    use std::io;
    use std::path::{Path, PathBuf};

    use proptest::prelude::*;

    use super::{
        deduplicate_roots, CollectFeaturesUseCase, FindArtifactCoverageUseCase,
        LintUccFormatsUseCase,
    };
    use crate::domain::{FeatureDocument, Priority};
    use crate::infrastructure::YamlUccParser;
    use crate::ports::{CoreError, TestFileRepository, UccFileRepository};

    #[test]
    fn deduplicate_roots_removes_exact_duplicates_and_redundant_subpaths() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().to_path_buf();
        let sub = root.join("sub");
        std::fs::create_dir(&sub).unwrap();

        let roots = vec![root.clone(), root.clone(), sub];
        let result = deduplicate_roots(&roots);

        // Should only contain the root, as sub is redundant and root is duplicated
        assert_eq!(result.len(), 1);
        assert!(result.contains(&root.canonicalize().unwrap()));
    }

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

        let result = use_case.execute(&[root.to_path_buf()]);

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
        let features = collect.execute(&[root.to_path_buf()]).expect("features should parse");

        let finder = FindArtifactCoverageUseCase::new(collect.repository);
        let index =
            finder.execute(&[root.to_path_buf()], &features).expect("coverage search should work");

        let matches = index.for_artifact("ucc-feat-1");
        assert_eq!(matches.len(), 5);
        let mut lines: Vec<usize> = matches.iter().map(|location| location.line).collect();
        lines.sort_unstable();
        assert_eq!(lines, vec![1, 1, 2, 2, 2]);
    }

    #[test]
    fn lint_ucc_formats_reports_valid_and_invalid_files_with_hints() {
        let root = Path::new("/lint");
        let repository = InMemoryUccRepository::default()
            .with_file(root.join("valid.ucc"), sample_document("feat-lint", Priority::High))
            .with_file(root.join("broken.ucc"), "schema_version: [".to_string());

        let use_case = LintUccFormatsUseCase::new(repository, YamlUccParser);
        let results = use_case.execute(&[root.to_path_buf()]).expect("lint should run");

        assert_eq!(results.len(), 2);
        let valid = results
            .iter()
            .find(|result| result.file_path.ends_with("valid.ucc"))
            .expect("valid result should be present");
        assert!(valid.is_valid);
        assert!(valid.issue.is_none());

        let invalid = results
            .iter()
            .find(|result| result.file_path.ends_with("broken.ucc"))
            .expect("invalid result should be present");
        assert!(!invalid.is_valid);
        let issue = invalid.issue.as_ref().expect("invalid file should include issue");
        assert!(!issue.message.trim().is_empty());
        assert!(issue.line.is_some() || issue.column.is_some());
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
            let mut features = collect.execute(&[root.to_path_buf()]).expect("features should parse");
            features[0].artifacts[0].id = artifact_id.clone();

            let finder = FindArtifactCoverageUseCase::new(collect.repository);
            let index = finder.execute(&[root.to_path_buf()], &features).expect("coverage should be searchable");

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
            let result: Vec<FeatureDocument> = use_case.execute(&[root.to_path_buf()]).expect("documents should parse");

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
