//! Library documentation fetching and caching.
//!
//! [`LibraryCache`] wraps a [`Bridge`] implementation and caches
//! [`LibraryDoc`] structs keyed by [`LibraryCacheKey`].  The cache is
//! concurrency-safe and suitable for use from multiple async tasks.

use std::path::PathBuf;
use std::sync::Arc;

use dashmap::DashMap;
use tracing::{debug, info};

use robotcode_python_bridge::{Bridge, LibraryDoc, LibraryDocParams};

pub use robotcode_python_bridge::{ArgInfo, EmbeddedArgs, InitDoc, KeywordDoc};

// ---------------------------------------------------------------------------
// Cache key
// ---------------------------------------------------------------------------

/// Identifies a unique library introspection result.
///
/// Two keys are equal if and only if the same Python interpreter with the same
/// sys.path additions would produce the same `LibraryDoc`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LibraryCacheKey {
    /// Library name or absolute path.
    pub name: String,
    /// Constructor arguments.
    pub args: Vec<String>,
    /// Extra sys.path entries.
    pub python_path: Vec<PathBuf>,
    /// The Python interpreter binary.
    pub python_interpreter: PathBuf,
}

impl LibraryCacheKey {
    pub fn new(
        name: impl Into<String>,
        args: Vec<String>,
        python_path: Vec<PathBuf>,
        python_interpreter: impl Into<PathBuf>,
    ) -> Self {
        Self {
            name: name.into(),
            args,
            python_path,
            python_interpreter: python_interpreter.into(),
        }
    }

    /// Build the params struct to send to the Python bridge.
    fn to_params(&self, base_dir: Option<&str>) -> LibraryDocParams {
        LibraryDocParams {
            name: self.name.clone(),
            args: self.args.clone(),
            base_dir: base_dir.map(str::to_owned),
            python_path: self
                .python_path
                .iter()
                .map(|p| p.display().to_string())
                .collect(),
            variables: Default::default(),
        }
    }
}

// ---------------------------------------------------------------------------
// LibraryCache
// ---------------------------------------------------------------------------

/// Thread-safe cache for [`LibraryDoc`] objects.
///
/// Wraps a [`Bridge`] and caches results so that repeated introspection of the
/// same library incurs only a single Python bridge round-trip.
pub struct LibraryCache {
    bridge: Arc<dyn Bridge>,
    cache: DashMap<LibraryCacheKey, Arc<LibraryDoc>>,
}

impl LibraryCache {
    /// Create a new cache backed by `bridge`.
    pub fn new(bridge: Arc<dyn Bridge>) -> Self {
        Self {
            bridge,
            cache: DashMap::new(),
        }
    }

    /// Fetch the library documentation for `key`, using the cache when
    /// possible.
    ///
    /// * `base_dir` — optional working directory used when resolving relative
    ///   library paths (e.g. the directory containing the `.robot` file that
    ///   has the `Library` import).
    pub async fn get(
        &self,
        key: &LibraryCacheKey,
        base_dir: Option<&str>,
    ) -> Result<Arc<LibraryDoc>, robotcode_python_bridge::BridgeError> {
        if let Some(cached) = self.cache.get(key) {
            debug!(library = %key.name, "Library cache hit");
            return Ok(Arc::clone(cached.value()));
        }

        info!(library = %key.name, "Library cache miss — fetching via bridge");
        let params = key.to_params(base_dir);
        let doc = self.bridge.library_doc(params).await?;
        let doc = Arc::new(doc);
        self.cache.insert(key.clone(), Arc::clone(&doc));
        Ok(doc)
    }

    /// Remove a cached entry (e.g. after the library source file changed).
    pub fn invalidate(&self, key: &LibraryCacheKey) {
        self.cache.remove(key);
    }

    /// Remove all cached entries.
    pub fn clear(&self) {
        self.cache.clear();
    }

    /// Number of entries currently in the cache.
    #[must_use]
    pub fn len(&self) -> usize {
        self.cache.len()
    }

    /// Returns `true` if the cache is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }
}
