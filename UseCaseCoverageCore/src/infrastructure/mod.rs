use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use chrono::{DateTime, Local};

use crate::domain::{Artifact, FeatureDocument, FeatureMetadata, Priority};
use crate::ports::{CoreError, TestFileRepository, UccFileRepository, UccParser};

pub struct LocalFileSystemRepository;
pub struct LocalTestFileRepository;

impl UccFileRepository for LocalFileSystemRepository {
    fn find_ucc_files(&self, root: &Path) -> Result<Vec<PathBuf>, CoreError> {
        collect_files_matching(root, is_ucc_file)
    }

    fn read_file(&self, path: &Path) -> Result<String, CoreError> {
        fs::read_to_string(path)
            .map_err(|source| CoreError::Io { path: path.to_path_buf(), source })
    }
}

impl TestFileRepository for LocalTestFileRepository {
    fn find_test_files(&self, root: &Path) -> Result<Vec<PathBuf>, CoreError> {
        collect_files_matching(root, is_supported_test_extension)
    }

    fn read_lines(&self, path: &Path) -> Result<Vec<String>, CoreError> {
        let content = fs::read_to_string(path)
            .map_err(|source| CoreError::Io { path: path.to_path_buf(), source })?;
        Ok(content.lines().map(ToOwned::to_owned).collect())
    }
}

pub struct YamlUccParser;

impl UccParser for YamlUccParser {
    fn parse(&self, source_path: &Path, content: &str) -> Result<FeatureDocument, CoreError> {
        let parsed: RawUccDocument = serde_yaml::from_str(content).map_err(|source| {
            CoreError::Parse { path: source_path.to_path_buf(), reason: source.to_string() }
        })?;

        let last_modified_at = fs::metadata(source_path)
            .and_then(|m| m.modified())
            .map(|t| {
                let dt: DateTime<Local> = t.into();
                dt.format("%Y-%m-%d %H:%M").to_string()
            })
            .ok();

        Ok(parsed.into_feature_document(source_path.to_path_buf(), last_modified_at.as_deref()))
    }
}

#[derive(Debug, Deserialize)]
struct RawUccDocument {
    schema_version: String,
    feature: RawFeatureMetadata,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    platforms: Vec<String>,
    #[serde(default)]
    related_features: Vec<String>,
    #[serde(default)]
    artifacts: Vec<RawArtifact>,
}

impl RawUccDocument {
    fn into_feature_document(
        self,
        source_path: PathBuf,
        last_modified_at: Option<&str>,
    ) -> FeatureDocument {
        FeatureDocument {
            source_path,
            schema_version: self.schema_version,
            feature: FeatureMetadata {
                id: self.feature.id,
                title: self.feature.title,
                created_at: self.feature.created_at,
                updated_at: self.feature.updated_at,
                last_modified_at: last_modified_at.map(str::to_owned),
                description: self.feature.description,
            },
            tags: self.tags,
            platforms: self.platforms,
            related_features: self.related_features,
            artifacts: self
                .artifacts
                .into_iter()
                .map(|artifact| Artifact {
                    id: artifact.id,
                    artifact_type: artifact.artifact_type,
                    created_at: artifact.created_at,
                    updated_at: artifact.updated_at,
                    last_modified_at: last_modified_at.map(str::to_owned),
                    title: artifact.title,
                    priority: artifact.priority,
                    related: artifact.related,
                    steps: artifact.steps,
                    expected: artifact.expected,
                    platforms: artifact.platforms,
                })
                .collect(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct RawFeatureMetadata {
    id: String,
    title: String,
    created_at: String,
    #[serde(default)]
    updated_at: Option<String>,
    description: String,
}

#[derive(Debug, Deserialize)]
struct RawArtifact {
    id: String,
    #[serde(rename = "type", default)]
    artifact_type: Option<String>,
    created_at: String,
    #[serde(default)]
    updated_at: Option<String>,
    title: String,
    #[serde(default)]
    priority: Priority,
    #[serde(default)]
    related: Vec<String>,
    #[serde(default)]
    platforms: Vec<String>,
    #[serde(default)]
    steps: Vec<String>,
    #[serde(default)]
    expected: Vec<String>,
}

fn is_supported_test_extension(path: &Path) -> bool {
    matches!(
        path.extension().and_then(std::ffi::OsStr::to_str),
        Some("swift" | "ts" | "tsx" | "kt" | "kts" | "rs")
    )
}

fn is_ucc_file(path: &Path) -> bool {
    path.extension().and_then(std::ffi::OsStr::to_str) == Some("ucc")
}

fn collect_files_matching(
    root: &Path,
    predicate: impl Fn(&Path) -> bool + Copy,
) -> Result<Vec<PathBuf>, CoreError> {
    let mut files = Vec::new();
    collect_files_matching_from_dir(root, predicate, &mut files)?;
    Ok(files)
}

fn collect_files_matching_from_dir(
    dir: &Path,
    predicate: impl Fn(&Path) -> bool + Copy,
    collector: &mut Vec<PathBuf>,
) -> Result<(), CoreError> {
    for entry in
        fs::read_dir(dir).map_err(|source| CoreError::Io { path: dir.to_path_buf(), source })?
    {
        let entry = entry.map_err(|source| CoreError::Io { path: dir.to_path_buf(), source })?;
        let path = entry.path();

        if path.is_dir() {
            collect_files_matching_from_dir(&path, predicate, collector)?;
        } else if predicate(&path) {
            collector.push(path);
        }
    }

    Ok(())
}
