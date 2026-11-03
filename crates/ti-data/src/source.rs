//! Where an extracted build is read from. Directories today; the trait keeps the
//! door open for in-memory/bundled sources (e.g. a WASM build).

use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

pub trait DataSource: Send + Sync {
    /// Read a file by its path relative to the build root (`/`-separated).
    fn read(&self, rel: &str) -> Result<Vec<u8>>;
    /// List `/`-separated relative paths of files directly inside `dir`.
    fn list(&self, dir: &str) -> Result<Vec<String>>;
    /// Filesystem location of the build, if it lives on disk (used for asset paths).
    fn root(&self) -> Option<&Path> {
        None
    }
}

#[derive(Debug, Clone)]
pub struct DirSource {
    root: PathBuf,
}

impl DirSource {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }
}

impl DataSource for DirSource {
    fn read(&self, rel: &str) -> Result<Vec<u8>> {
        let path = self.root.join(rel);
        std::fs::read(&path).map_err(|source| Error::Io { path, source })
    }

    fn list(&self, dir: &str) -> Result<Vec<String>> {
        let path = self.root.join(dir);
        let entries = std::fs::read_dir(&path).map_err(|source| Error::Io { path: path.clone(), source })?;
        let mut out = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|source| Error::Io { path: path.clone(), source })?;
            if entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
                out.push(format!("{dir}/{}", entry.file_name().to_string_lossy()));
            }
        }
        out.sort();
        Ok(out)
    }

    fn root(&self) -> Option<&Path> {
        Some(&self.root)
    }
}

pub(crate) fn read_json<T: serde::de::DeserializeOwned>(src: &dyn DataSource, rel: &str) -> Result<T> {
    let bytes = src.read(rel)?;
    serde_json::from_slice(&bytes).map_err(|source| Error::Json { path: rel.into(), source })
}
