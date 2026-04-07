//! Async cache for resolved imports.
//!
//! [`ImportsManager`] holds the async cache of resolved library, resource, and
//! variables-file imports for a single workspace.  It wraps the library cache
//! from phase 4 and adds resource/variables file management.
//!
//! Mirrors the high-level structure of `robotcode.robot.diagnostics.imports_manager`.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use dashmap::DashMap;
use tokio::sync::Mutex;
use tracing::{debug, info};

use robotcode_python_bridge::{
    Bridge, LibraryDoc, VariableEntry, VariablesDoc, VariablesDocParams,
};

use super::entities::{
    KeywordDoc, LibraryEntry, ResourceEntry, VariableDefinition, VariableScope, VariablesEntry,
};
use super::library_doc::{LibraryCache, LibraryCacheKey};

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors produced by the imports manager.
#[derive(Debug, thiserror::Error)]
pub enum ImportsError {
    #[error("Bridge error: {0}")]
    Bridge(#[from] robotcode_python_bridge::BridgeError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Parse error for {path}: {message}")]
    Parse { path: String, message: String },
}

// ---------------------------------------------------------------------------
// Cached resource/variables data
// ---------------------------------------------------------------------------

/// Cached analysis of a resource file.
#[derive(Debug, Clone)]
pub struct ResourceData {
    pub path: PathBuf,
    pub keywords: Vec<KeywordDoc>,
    pub variables: Vec<VariableDefinition>,
}

/// Cached variables file data.
#[derive(Debug, Clone)]
pub struct VariablesData {
    pub path: PathBuf,
    pub variables: Vec<VariableDefinition>,
}

// ---------------------------------------------------------------------------
// Per-key slot type used for deduplicating concurrent fetches
// ---------------------------------------------------------------------------

type Slot<T> = Arc<Mutex<Option<Arc<T>>>>;

fn get_or_create_slot<K, V>(map: &DashMap<K, Slot<V>>, key: K) -> Slot<V>
where
    K: std::hash::Hash + Eq,
{
    map.entry(key)
        .or_insert_with(|| Arc::new(Mutex::new(None)))
        .clone()
}

// ---------------------------------------------------------------------------
// ImportsManager
// ---------------------------------------------------------------------------

/// Per-workspace import cache.
///
/// Wraps [`LibraryCache`] (from phase 4) and adds slots for resource files
/// and variables files.  Each cache is keyed by path so that multiple
/// `.robot` files importing the same resource share a single resolved copy.
pub struct ImportsManager {
    /// Library documentation cache (phase 4 component).
    pub library_cache: LibraryCache,
    /// Resolved resource file cache.
    resource_slots: DashMap<PathBuf, Slot<ResourceData>>,
    /// Resolved variables file cache.
    variables_slots: DashMap<PathBuf, Slot<VariablesData>>,
    /// The bridge (needed for variables_doc calls).
    bridge: Arc<dyn Bridge>,
}

impl ImportsManager {
    /// Create a new manager backed by `bridge`.
    pub fn new(bridge: Arc<dyn Bridge>) -> Self {
        let library_cache = LibraryCache::new(Arc::clone(&bridge));
        Self {
            library_cache,
            resource_slots: DashMap::new(),
            variables_slots: DashMap::new(),
            bridge,
        }
    }

    // -----------------------------------------------------------------------
    // Library
    // -----------------------------------------------------------------------

    /// Fetch or compute a [`LibraryEntry`] for the given key.
    pub async fn get_library(
        &self,
        key: &LibraryCacheKey,
        base_dir: Option<&str>,
    ) -> Result<LibraryEntry, ImportsError> {
        let doc = self.library_cache.get(key, base_dir).await?;
        Ok(library_entry_from_doc(&doc))
    }

    // -----------------------------------------------------------------------
    // Resource
    // -----------------------------------------------------------------------

    /// Get the cached resource data for `path`, computing it if necessary.
    ///
    /// Resource analysis is handled externally (by the document cache), so
    /// this method accepts pre-computed `data` and caches it.
    pub async fn cache_resource(&self, path: PathBuf, data: ResourceData) -> Arc<ResourceData> {
        let slot = get_or_create_slot(&self.resource_slots, path.clone());
        let mut guard = slot.lock().await;
        let arc = Arc::new(data);
        *guard = Some(Arc::clone(&arc));
        arc
    }

    /// Return the cached resource data for `path`, if available.
    pub async fn get_resource(&self, path: &Path) -> Option<Arc<ResourceData>> {
        let slot = self.resource_slots.get(path)?.clone();
        let guard = slot.lock().await;
        guard.as_ref().cloned()
    }

    // -----------------------------------------------------------------------
    // Variables
    // -----------------------------------------------------------------------

    /// Fetch or compute a [`VariablesData`] for the given path via the bridge.
    pub async fn get_variables(
        &self,
        path: &Path,
        args: &[String],
        base_dir: Option<&str>,
    ) -> Result<Arc<VariablesData>, ImportsError> {
        let slot = get_or_create_slot(&self.variables_slots, path.to_path_buf());
        let mut guard = slot.lock().await;

        if let Some(data) = guard.as_ref() {
            debug!(path = %path.display(), "Variables cache hit");
            return Ok(Arc::clone(data));
        }

        info!(path = %path.display(), "Variables cache miss — fetching via bridge");
        let params = VariablesDocParams {
            path: path.display().to_string(),
            args: args.to_vec(),
            base_dir: base_dir.map(str::to_owned),
        };
        let doc: VariablesDoc = self.bridge.variables_doc(params).await?;
        let data = Arc::new(variables_data_from_doc(path, &doc));
        *guard = Some(Arc::clone(&data));
        Ok(data)
    }

    // -----------------------------------------------------------------------
    // Invalidation
    // -----------------------------------------------------------------------

    /// Remove the cached library documentation entry.
    pub fn invalidate_library(&self, key: &LibraryCacheKey) {
        self.library_cache.invalidate(key);
    }

    /// Remove the cached resource data for `path`.
    pub fn invalidate_resource(&self, path: &Path) {
        self.resource_slots.remove(path);
    }

    /// Remove the cached variables data for `path`.
    pub fn invalidate_variables(&self, path: &Path) {
        self.variables_slots.remove(path);
    }

    /// Clear all caches.
    pub fn clear(&self) {
        self.library_cache.clear();
        self.resource_slots.clear();
        self.variables_slots.clear();
    }
}

// ---------------------------------------------------------------------------
// Conversion helpers
// ---------------------------------------------------------------------------

fn library_entry_from_doc(doc: &LibraryDoc) -> LibraryEntry {
    let keywords = doc
        .keywords
        .iter()
        .map(|kw| KeywordDoc::from_bridge(kw, &doc.name))
        .collect();
    LibraryEntry {
        name: doc.name.clone(),
        alias: None,
        keywords,
    }
}

fn variables_data_from_doc(path: &Path, doc: &VariablesDoc) -> VariablesData {
    let variables = doc
        .variables
        .iter()
        .filter_map(|v| variable_from_entry(v, path))
        .collect();
    VariablesData {
        path: path.to_path_buf(),
        variables,
    }
}

fn variable_from_entry(entry: &VariableEntry, source: &Path) -> Option<VariableDefinition> {
    let pos = lsp_types::Position {
        line: (entry.lineno as u32).saturating_sub(1),
        character: 0,
    };
    let range = lsp_types::Range {
        start: pos,
        end: pos,
    };
    VariableDefinition::from_name(
        &entry.name,
        Some(entry.value.clone()),
        range,
        Some(source.to_path_buf()),
        VariableScope::Suite,
    )
}

// ---------------------------------------------------------------------------
// ResourceEntry / VariablesEntry builders
// ---------------------------------------------------------------------------

impl ResourceEntry {
    /// Build from cached [`ResourceData`].
    pub fn from_data(data: &ResourceData) -> Self {
        Self {
            path: data.path.clone(),
            keywords: data.keywords.clone(),
            variables: data.variables.clone(),
        }
    }
}

impl VariablesEntry {
    /// Build from cached [`VariablesData`].
    pub fn from_data(data: &VariablesData) -> Self {
        Self {
            path: data.path.clone(),
            variables: data.variables.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use robotcode_python_bridge::MockBridge;
    use serde_json::json;

    fn make_manager() -> ImportsManager {
        let bridge = MockBridge::with_responses([(
            "library_doc",
            vec![json!({
                "name": "BuiltIn",
                "doc": "Built-in library",
                "version": "7.0.0",
                "scope": "GLOBAL",
                "named_args": true,
                "keywords": [
                    {
                        "name": "Log",
                        "args": [
                            {"name": "message", "kind": "POSITIONAL_OR_KEYWORD", "default": null, "types": []},
                            {"name": "level", "kind": "POSITIONAL_OR_KEYWORD", "default": "INFO", "types": []}
                        ],
                        "doc": "Logs the given message.",
                        "tags": [],
                        "source": null,
                        "lineno": null
                    }
                ],
                "inits": [],
                "typedocs": []
            })],
        )]);
        ImportsManager::new(Arc::new(bridge))
    }

    #[tokio::test]
    async fn test_get_library_returns_entry() {
        use std::path::PathBuf;
        let manager = make_manager();
        let key = LibraryCacheKey::new("BuiltIn", vec![], vec![], PathBuf::from("python3"));
        let entry = manager.get_library(&key, None).await.unwrap();
        assert_eq!(entry.name, "BuiltIn");
        assert_eq!(entry.keywords.len(), 1);
        assert_eq!(entry.keywords[0].name, "Log");
    }

    #[tokio::test]
    async fn test_cache_and_get_resource() {
        let manager = make_manager();
        let path = PathBuf::from("/tmp/res.resource");
        let data = ResourceData {
            path: path.clone(),
            keywords: vec![],
            variables: vec![],
        };
        let cached = manager.cache_resource(path.clone(), data).await;
        assert_eq!(cached.path, path);

        let retrieved = manager.get_resource(&path).await;
        assert!(retrieved.is_some());
    }

    #[tokio::test]
    async fn test_invalidate_resource() {
        let manager = make_manager();
        let path = PathBuf::from("/tmp/res.resource");
        let data = ResourceData {
            path: path.clone(),
            keywords: vec![],
            variables: vec![],
        };
        manager.cache_resource(path.clone(), data).await;
        manager.invalidate_resource(&path);
        assert!(manager.get_resource(&path).await.is_none());
    }
}
