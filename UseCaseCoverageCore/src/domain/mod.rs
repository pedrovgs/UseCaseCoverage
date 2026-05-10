use std::collections::HashMap;
use std::path::PathBuf;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FeatureDocument {
    pub source_path: PathBuf,
    pub schema_version: String,
    pub feature: FeatureMetadata,
    pub tags: Vec<String>,
    pub platforms: Vec<String>,
    pub related_features: Vec<String>,
    pub artifacts: Vec<Artifact>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FeatureMetadata {
    pub id: String,
    pub title: String,
    pub created_at: String,
    pub updated_at: Option<String>,
    pub last_modified_at: Option<String>,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Artifact {
    pub id: String,
    pub artifact_type: Option<String>,
    pub created_at: String,
    pub updated_at: Option<String>,
    pub last_modified_at: Option<String>,
    pub title: String,
    pub priority: Priority,
    pub related: Vec<String>,
    pub platforms: Vec<String>,
    pub steps: Vec<String>,
    pub expected: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactTestLocation {
    pub file_path: PathBuf,
    pub line: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ArtifactCoverageIndex {
    pub by_artifact_id: HashMap<String, Vec<ArtifactTestLocation>>,
}

impl ArtifactCoverageIndex {
    #[must_use]
    pub fn for_artifact(&self, artifact_id: &str) -> &[ArtifactTestLocation] {
        self.by_artifact_id.get(artifact_id).map_or(&[], Vec::as_slice)
    }

    #[must_use]
    pub fn is_covered(&self, artifact_id: &str) -> bool {
        self.by_artifact_id.contains_key(artifact_id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UccLintIssue {
    pub message: String,
    pub line: Option<usize>,
    pub column: Option<usize>,
    pub suggestion: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UccLintResult {
    pub file_path: PathBuf,
    pub is_valid: bool,
    pub issue: Option<UccLintIssue>,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize, Default,
)]
#[serde(rename_all = "lowercase")]
pub enum Priority {
    #[default]
    None,
    Low,
    Medium,
    High,
    Highest,
}

impl Priority {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Highest => "highest",
        }
    }
}

impl std::fmt::Display for Priority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Priority {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "none" => Ok(Self::None),
            "low" => Ok(Self::Low),
            "medium" => Ok(Self::Medium),
            "high" => Ok(Self::High),
            "highest" => Ok(Self::Highest),
            _ => Err(format!("Invalid priority: '{value}'")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn priority_display_and_from_str() {
        let valid_cases = [
            ("none", Priority::None),
            ("low", Priority::Low),
            ("medium", Priority::Medium),
            ("high", Priority::High),
            ("highest", Priority::Highest),
        ];

        for (s, p) in valid_cases {
            assert_eq!(Priority::from_str(s).unwrap(), p);
            assert_eq!(p.to_string(), s);
        }

        assert_eq!(Priority::from_str("  HIGH  ").unwrap(), Priority::High);

        let err = Priority::from_str("critical").unwrap_err();
        assert_eq!(err, "Invalid priority: 'critical'");
    }
}
