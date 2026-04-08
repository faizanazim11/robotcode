//! Import path resolution for Robot Framework library, resource, and variables imports.
//!
//! Mirrors the logic in `robotcode.robot.diagnostics.import_resolver`.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use thiserror::Error;
use tracing::debug;

/// Errors produced by the import resolver.
#[derive(Debug, Error)]
pub enum ResolveError {
    #[error("Circular import detected: {0}")]
    CircularImport(String),

    #[error("Import path cannot be empty")]
    EmptyPath,

    #[error("Import not found: {0}")]
    NotFound(String),
}

/// Result of resolving a `Resource` or `Variables` import.
#[derive(Debug, Clone)]
pub struct ResolvedPath {
    /// Absolute path to the resolved file.
    pub path: PathBuf,
}

/// Result of resolving a `Library` import.
#[derive(Debug, Clone)]
pub struct ResolvedLibrary {
    /// The canonical library name or absolute path.
    pub name: String,
    /// Resolved filesystem path for file-based libraries (`.py`, `.robot`).
    pub path: Option<PathBuf>,
}

/// Resolver configuration.
///
/// Library module-name resolution (i.e. looking up a library by dotted Python
/// module name on `sys.path`) is delegated to the Python bridge and is not
/// performed here.  This config handles file-system-based resolution only.
#[derive(Debug, Clone, Default)]
pub struct ResolverConfig {
    /// Root directories of the workspace (used as additional search roots).
    pub root_dirs: Vec<PathBuf>,
}

/// Tracks in-progress imports to detect circular references.
#[derive(Debug, Default)]
pub struct ImportGuard {
    in_progress: HashSet<PathBuf>,
}

impl ImportGuard {
    /// Mark `path` as in-progress.  Returns `Err` if already in progress.
    pub fn enter(&mut self, path: &Path) -> Result<(), ResolveError> {
        let canonical = path.to_path_buf();
        if !self.in_progress.insert(canonical.clone()) {
            return Err(ResolveError::CircularImport(
                canonical.display().to_string(),
            ));
        }
        Ok(())
    }

    /// Remove `path` from the in-progress set when analysis of it completes.
    pub fn leave(&mut self, path: &Path) {
        self.in_progress.remove(path);
    }
}

/// Import resolver — converts import names/paths to absolute filesystem paths.
pub struct ImportResolver {
    config: ResolverConfig,
}

impl ImportResolver {
    pub fn new(config: ResolverConfig) -> Self {
        Self { config }
    }

    /// Resolve a `Resource` import path relative to `base_dir`.
    ///
    /// Search order:
    /// 1. Absolute path
    /// 2. Relative to `base_dir` (the directory of the importing file)
    /// 3. Relative to each workspace root directory
    pub fn resolve_resource(
        &self,
        path: &str,
        base_dir: Option<&Path>,
    ) -> Result<ResolvedPath, ResolveError> {
        if path.trim().is_empty() {
            return Err(ResolveError::EmptyPath);
        }

        let candidate = PathBuf::from(path);

        // 1. Absolute path
        if candidate.is_absolute() && candidate.exists() {
            debug!(path = %candidate.display(), "Resolved resource as absolute path");
            return Ok(ResolvedPath { path: candidate });
        }

        // 2. Relative to base_dir
        if let Some(base) = base_dir {
            let resolved = base.join(&candidate);
            if resolved.exists() {
                debug!(path = %resolved.display(), "Resolved resource relative to base_dir");
                return Ok(ResolvedPath { path: resolved });
            }
        }

        // 3. Relative to workspace root directories
        for root in &self.config.root_dirs {
            let resolved = root.join(&candidate);
            if resolved.exists() {
                debug!(path = %resolved.display(), "Resolved resource relative to root_dir");
                return Ok(ResolvedPath { path: resolved });
            }
        }

        Err(ResolveError::NotFound(path.to_owned()))
    }

    /// Resolve a `Variables` import path.
    ///
    /// Variables files may be `.py`, `.robot`, `.yaml`, or `.json`.
    /// Search order is the same as for resources.
    pub fn resolve_variables(
        &self,
        path: &str,
        base_dir: Option<&Path>,
    ) -> Result<ResolvedPath, ResolveError> {
        // Variables files use the same resolution algorithm as resources.
        self.resolve_resource(path, base_dir)
    }

    /// Resolve a `Library` import name.
    ///
    /// Returns a [`ResolvedLibrary`] with the canonical name.  For built-in
    /// libraries the path is `None`; for file-based libraries the path is the
    /// absolute `.py`/`.robot` path if found on disk.
    ///
    /// Note: True resolution (e.g. against the venv `site-packages`) is
    /// performed by the Python bridge via `library_doc`.  This method only
    /// handles the file-based case that can be detected without a Python
    /// interpreter.
    pub fn resolve_library(
        &self,
        name: &str,
        base_dir: Option<&Path>,
    ) -> Result<ResolvedLibrary, ResolveError> {
        if name.trim().is_empty() {
            return Err(ResolveError::EmptyPath);
        }

        // If the name contains a path separator or ends in `.py`/`.robot`, try
        // to resolve it as a file.
        let as_path = PathBuf::from(name);
        if as_path.extension().is_some()
            || name.contains('/')
            || name.contains(std::path::MAIN_SEPARATOR)
        {
            // Try to find the file
            let resolved = self.resolve_resource(name, base_dir).ok();
            return Ok(ResolvedLibrary {
                name: name.to_owned(),
                path: resolved.map(|r| r.path),
            });
        }

        // Otherwise it is a dotted module name — rely on the bridge for resolution.
        Ok(ResolvedLibrary {
            name: name.to_owned(),
            path: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;

    fn make_resolver(root: &Path) -> ImportResolver {
        ImportResolver::new(ResolverConfig {
            root_dirs: vec![root.to_path_buf()],
        })
    }

    #[test]
    fn test_resolve_resource_relative_to_base_dir() {
        let dir = TempDir::new().unwrap();
        let resource = dir.path().join("my_resource.resource");
        fs::write(&resource, "").unwrap();

        let resolver = make_resolver(dir.path());
        let result = resolver
            .resolve_resource("my_resource.resource", Some(dir.path()))
            .unwrap();
        assert_eq!(result.path, resource);
    }

    #[test]
    fn test_resolve_resource_relative_to_root() {
        let dir = TempDir::new().unwrap();
        let resource = dir.path().join("common.resource");
        fs::write(&resource, "").unwrap();

        let sub = dir.path().join("sub");
        fs::create_dir(&sub).unwrap();

        let resolver = make_resolver(dir.path());
        // base_dir is the sub-directory, but the file is in the root.
        let result = resolver
            .resolve_resource("common.resource", Some(&sub))
            .unwrap();
        assert_eq!(result.path, resource);
    }

    #[test]
    fn test_resolve_resource_not_found() {
        let dir = TempDir::new().unwrap();
        let resolver = make_resolver(dir.path());
        let err = resolver.resolve_resource("missing.resource", Some(dir.path()));
        assert!(matches!(err, Err(ResolveError::NotFound(_))));
    }

    #[test]
    fn test_resolve_resource_empty_path() {
        let dir = TempDir::new().unwrap();
        let resolver = make_resolver(dir.path());
        let err = resolver.resolve_resource("", None);
        assert!(matches!(err, Err(ResolveError::EmptyPath)));
    }

    #[test]
    fn test_import_guard_detects_circular() {
        let mut guard = ImportGuard::default();
        let path = PathBuf::from("/some/file.resource");
        guard.enter(&path).unwrap();
        let err = guard.enter(&path);
        assert!(matches!(err, Err(ResolveError::CircularImport(_))));
        guard.leave(&path);
        // After leave, entering again is OK.
        guard.enter(&path).unwrap();
    }

    #[test]
    fn test_resolve_library_module_name() {
        let dir = TempDir::new().unwrap();
        let resolver = make_resolver(dir.path());
        let resolved = resolver.resolve_library("BuiltIn", None).unwrap();
        assert_eq!(resolved.name, "BuiltIn");
        assert!(resolved.path.is_none());
    }
}
