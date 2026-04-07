//! Shared data types used by both bridge implementations and callers.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// rf_version
// ---------------------------------------------------------------------------

/// Installed Robot Framework version information.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RfVersion {
    pub version: String,
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

// ---------------------------------------------------------------------------
// library_doc
// ---------------------------------------------------------------------------

/// Request parameters for `library_doc`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LibraryDocParams {
    /// Library name (e.g. `"BuiltIn"`) or filesystem path.
    pub name: String,
    /// Constructor arguments passed to the library.
    #[serde(default)]
    pub args: Vec<String>,
    /// Working directory for resolving relative library paths.
    #[serde(default)]
    pub base_dir: Option<String>,
    /// Extra entries to prepend to `sys.path`.
    #[serde(default)]
    pub python_path: Vec<String>,
    /// RF variables to set before introspection.
    #[serde(default)]
    pub variables: std::collections::HashMap<String, String>,
}

/// A keyword argument descriptor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArgInfo {
    pub name: String,
    pub kind: String,
    pub default: Option<String>,
    #[serde(default)]
    pub types: Vec<String>,
}

/// A single keyword's documentation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeywordDoc {
    pub name: String,
    #[serde(default)]
    pub args: Vec<ArgInfo>,
    #[serde(default)]
    pub doc: String,
    #[serde(default)]
    pub tags: Vec<String>,
    pub source: Option<String>,
    pub lineno: Option<i64>,
}

/// Library initializer documentation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitDoc {
    #[serde(default)]
    pub args: Vec<ArgInfo>,
    #[serde(default)]
    pub doc: String,
}

/// Full library documentation returned by `library_doc`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryDoc {
    pub name: String,
    #[serde(default)]
    pub doc: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub scope: String,
    #[serde(default = "default_true")]
    pub named_args: bool,
    #[serde(default)]
    pub keywords: Vec<KeywordDoc>,
    #[serde(default)]
    pub inits: Vec<InitDoc>,
    #[serde(default)]
    pub typedocs: Vec<serde_json::Value>,
}

fn default_true() -> bool {
    true
}

// ---------------------------------------------------------------------------
// variables_doc
// ---------------------------------------------------------------------------

/// Request parameters for `variables_doc`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct VariablesDocParams {
    pub path: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub base_dir: Option<String>,
}

/// A single variable entry from a variables file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariableEntry {
    pub name: String,
    pub value: String,
    pub source: String,
    pub lineno: i64,
}

/// Full variables file documentation returned by `variables_doc`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariablesDoc {
    #[serde(default)]
    pub variables: Vec<VariableEntry>,
}

// ---------------------------------------------------------------------------
// embedded_args
// ---------------------------------------------------------------------------

/// Parsed embedded argument pattern.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddedArgs {
    pub name: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub regex: String,
}

// ---------------------------------------------------------------------------
// discover
// ---------------------------------------------------------------------------

/// Request parameters for `discover`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DiscoverParams {
    #[serde(default)]
    pub paths: Vec<String>,
    #[serde(default)]
    pub include_tags: Vec<String>,
    #[serde(default)]
    pub exclude_tags: Vec<String>,
    #[serde(default)]
    pub python_path: Vec<String>,
}

/// A discovered test entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredTest {
    pub name: String,
    #[serde(default)]
    pub tags: Vec<String>,
    pub lineno: Option<i64>,
}

/// A discovered test suite (may be nested).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredSuite {
    pub name: String,
    pub source: Option<String>,
    #[serde(default)]
    pub tests: Vec<DiscoveredTest>,
    #[serde(default)]
    pub suites: Vec<DiscoveredSuite>,
}

/// Top-level discover result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoverResult {
    #[serde(default)]
    pub suites: Vec<DiscoveredSuite>,
}
