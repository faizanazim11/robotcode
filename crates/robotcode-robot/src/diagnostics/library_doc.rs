//! Library documentation fetching and caching.
//!
//! [`LibraryCache`] wraps a [`Bridge`] implementation and caches
//! [`robotcode_python_bridge::LibraryDoc`] structs keyed by
//! [`LibraryCacheKey`].  The cache is concurrency-safe and suitable for use
//! from multiple async tasks.

use std::path::PathBuf;
use std::sync::Arc;

use dashmap::DashMap;
use tokio::sync::Mutex;
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

/// Per-key fetch slot: `None` means not yet fetched; `Some` is the cached doc.
type Slot = Arc<Mutex<Option<Arc<LibraryDoc>>>>;

/// Thread-safe cache for [`LibraryDoc`] objects.
///
/// Wraps a [`Bridge`] and caches results so that repeated introspection of the
/// same library incurs only a single Python bridge round-trip.  Concurrent
/// requests for the same key are deduplicated: only one bridge call will be
/// in flight at a time per key.
pub struct LibraryCache {
    bridge: Arc<dyn Bridge>,
    /// Maps each key to a per-key [`Mutex`].  The first caller that misses the
    /// cache acquires the slot mutex and populates it; subsequent concurrent
    /// callers for the same key wait on the same mutex and read the result.
    slots: DashMap<LibraryCacheKey, Slot>,
}

impl LibraryCache {
    /// Create a new cache backed by `bridge`.
    pub fn new(bridge: Arc<dyn Bridge>) -> Self {
        Self {
            bridge,
            slots: DashMap::new(),
        }
    }

    /// Fetch the library documentation for `key`, using the cache when
    /// possible.
    ///
    /// Concurrent callers with the same `key` are serialized per-key so that
    /// only one bridge round-trip is ever in flight for a given library.
    ///
    /// * `base_dir` — optional working directory used when resolving relative
    ///   library paths (e.g. the directory containing the `.robot` file that
    ///   has the `Library` import).
    pub async fn get(
        &self,
        key: &LibraryCacheKey,
        base_dir: Option<&str>,
    ) -> Result<Arc<LibraryDoc>, robotcode_python_bridge::BridgeError> {
        // Get or create the per-key slot.  `entry().or_insert_with()` holds
        // the DashMap shard lock only briefly; once we have the Arc we release
        // it before any async work.
        let slot: Slot = self
            .slots
            .entry(key.clone())
            .or_insert_with(|| Arc::new(Mutex::new(None)))
            .clone();

        // Lock the slot.  This ensures only one fetch per key at a time.
        let mut guard = slot.lock().await;

        if let Some(doc) = guard.as_ref() {
            debug!(library = %key.name, "Library cache hit");
            return Ok(Arc::clone(doc));
        }

        info!(library = %key.name, "Library cache miss — fetching via bridge");
        let params = key.to_params(base_dir);
        let doc = Arc::new(self.bridge.library_doc(params).await?);
        *guard = Some(Arc::clone(&doc));
        Ok(doc)
    }

    /// Remove a cached entry (e.g. after the library source file changed).
    pub fn invalidate(&self, key: &LibraryCacheKey) {
        self.slots.remove(key);
    }

    /// Remove all cached entries.
    pub fn clear(&self) {
        self.slots.clear();
    }

    /// Number of entries currently in the cache.
    #[must_use]
    pub fn len(&self) -> usize {
        self.slots.len()
    }

    /// Returns `true` if the cache is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }
}
