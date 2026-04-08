//! File-based analysis cache for `robotcode analyze`.

use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::Result;
use lsp_types::Diagnostic;
use serde::{Deserialize, Serialize};
use tracing::debug;

// ── Cache data types ──────────────────────────────────────────────────────────

/// A single cached entry for one analyzed file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    /// Absolute path of the analyzed file.
    pub path: String,
    /// Last-modified time (seconds since Unix epoch) at analysis time.
    pub mtime: u64,
    /// Diagnostics produced for this file.
    pub diagnostics: Vec<Diagnostic>,
}

/// Full cache file contents.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CacheData {
    /// Map from file path string to its cached entry.
    pub entries: HashMap<String, CacheEntry>,
}

// ── AnalysisCache ─────────────────────────────────────────────────────────────

/// Persistent file-based analysis cache.
///
/// The cache is stored as a JSON file at the given path.
pub struct AnalysisCache {
    path: PathBuf,
}

impl AnalysisCache {
    /// Create a new cache backed by the file at `path`.
    ///
    /// The file does not need to exist yet; it will be created on the first
    /// [`save`](Self::save).
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    /// Load the cache from disk.
    ///
    /// Returns an empty [`CacheData`] if the file does not exist or cannot
    /// be parsed.
    pub async fn load(&self) -> Result<CacheData> {
        debug!(path = %self.path.display(), "Loading analysis cache");
        let contents = match tokio::fs::read_to_string(&self.path).await {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(CacheData::default());
            }
            Err(e) => return Err(e.into()),
        };
        let data: CacheData = serde_json::from_str(&contents)?;
        Ok(data)
    }

    /// Save `data` to the cache file on disk.
    pub async fn save(&self, data: &CacheData) -> Result<()> {
        debug!(path = %self.path.display(), "Saving analysis cache");
        if let Some(parent) = self.path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let json = serde_json::to_string_pretty(data)?;
        tokio::fs::write(&self.path, json).await?;
        Ok(())
    }

    /// Delete the cache file.
    pub async fn clear(&self) -> Result<()> {
        debug!(path = %self.path.display(), "Clearing analysis cache");
        match tokio::fs::remove_file(&self.path).await {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
}
