//! Diagnostics sub-modules.

pub mod document_cache;
pub mod entities;
pub mod errors;
pub mod import_resolver;
pub mod imports_manager;
pub mod keyword_finder;
pub mod library_doc;
pub mod namespace;
pub mod namespace_analyzer;
pub mod variable_scope;

pub use document_cache::DocumentCache;
pub use library_doc::{LibraryCache, LibraryCacheKey};
pub use namespace::Namespace;
pub use namespace_analyzer::NamespaceAnalyzer;
