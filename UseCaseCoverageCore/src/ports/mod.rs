use std::error::Error;
use std::fmt::{Display, Formatter};
use std::path::{Path, PathBuf};

use crate::domain::FeatureDocument;

#[derive(Debug)]
pub enum CoreError {
    Io { path: PathBuf, source: std::io::Error },
    Parse { path: PathBuf, reason: String },
}

impl Display for CoreError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io { path, source } => write!(f, "I/O error at '{}': {source}", path.display()),
            Self::Parse { path, reason } => {
                write!(f, "Parse error at '{}': {reason}", path.display())
            }
        }
    }
}

impl Error for CoreError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Parse { .. } => None,
        }
    }
}

pub trait UccFileRepository {
    /// Finds all `.ucc` files under `root` recursively.
    ///
    /// # Errors
    ///
    /// Returns an error when the file system cannot be read.
    fn find_ucc_files(&self, root: &Path) -> Result<Vec<PathBuf>, CoreError>;
    /// Reads a file and returns its raw contents.
    ///
    /// # Errors
    ///
    /// Returns an error when the file cannot be read.
    fn read_file(&self, path: &Path) -> Result<String, CoreError>;
}

pub trait UccParser {
    /// Parses a `.ucc` content string into a feature document.
    ///
    /// # Errors
    ///
    /// Returns an error when the input content is not a valid expected schema.
    fn parse(&self, source_path: &Path, content: &str) -> Result<FeatureDocument, CoreError>;
}

pub trait TestFileRepository {
    /// Finds source files that may contain automated tests.
    ///
    /// # Errors
    ///
    /// Returns an error when the file system cannot be read.
    fn find_test_files(&self, root: &Path) -> Result<Vec<PathBuf>, CoreError>;

    /// Reads a source file and returns contents by line.
    ///
    /// # Errors
    ///
    /// Returns an error when the file cannot be read.
    fn read_lines(&self, path: &Path) -> Result<Vec<String>, CoreError>;
}
