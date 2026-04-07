//! Core entity types for the diagnostics engine.
//!
//! These types represent the resolved state of a Robot Framework file:
//! imported libraries, resource files, variables files, keyword definitions,
//! and variable definitions.  They are distinct from the bridge types in
//! `robotcode-python-bridge` because they carry additional analysis metadata
//! (source ranges, scope information, etc.).

use std::path::PathBuf;

use lsp_types::Range;

// ---------------------------------------------------------------------------
// Argument specification
// ---------------------------------------------------------------------------

/// Argument kind — mirrors `robot.running.arguments.argumentspec.ArgInfo.TYPES`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArgKind {
    PositionalOnly,
    PositionalOrKeyword,
    VarPositional,
    KeywordOnly,
    VarKeyword,
}

impl ArgKind {
    /// Parse the string representation produced by the Python bridge.
    pub fn parse(s: &str) -> Self {
        match s {
            "POSITIONAL_ONLY" => ArgKind::PositionalOnly,
            "VAR_POSITIONAL" => ArgKind::VarPositional,
            "KEYWORD_ONLY" => ArgKind::KeywordOnly,
            "VAR_KEYWORD" => ArgKind::VarKeyword,
            _ => ArgKind::PositionalOrKeyword,
        }
    }
}

/// A single argument in a keyword's argument list.
#[derive(Debug, Clone)]
pub struct ArgSpec {
    pub name: String,
    pub kind: ArgKind,
    pub default: Option<String>,
    pub types: Vec<String>,
}

// ---------------------------------------------------------------------------
// Keyword definitions
// ---------------------------------------------------------------------------

/// A keyword definition visible from within a namespace.
///
/// This type is used for keywords coming from both libraries (via the bridge)
/// and from resource files (parsed locally).
#[derive(Debug, Clone)]
pub struct KeywordDoc {
    /// The display name of the keyword.
    pub name: String,
    /// The normalized lookup name (lowercase, spaces/underscores collapsed).
    pub normalized_name: String,
    /// Full argument specification.
    pub args: Vec<ArgSpec>,
    /// Documentation string.
    pub doc: String,
    /// Deprecation notice extracted from the doc, if any.
    pub deprecated: Option<String>,
    /// Source file path.
    pub source: Option<PathBuf>,
    /// 1-based line number of the keyword definition.
    pub line_no: Option<u32>,
    /// `true` if the keyword name contains embedded argument patterns `${…}`.
    pub is_embedded: bool,
    /// Pre-compiled regex for embedded argument matching (when `is_embedded`).
    pub embedded_regex: Option<regex::Regex>,
    /// Library name the keyword belongs to (`None` for resource-file keywords).
    pub library_name: Option<String>,
    /// Whether the keyword is private (currently determined by the name starting with `_`).
    pub is_private: bool,
}

impl KeywordDoc {
    /// Create a `KeywordDoc` from bridge types.
    pub fn from_bridge(kw: &robotcode_python_bridge::KeywordDoc, library_name: &str) -> Self {
        let normalized_name = normalize_keyword_name(&kw.name);
        let is_embedded = kw.name.contains("${");
        let embedded_regex = if is_embedded {
            build_embedded_regex(&kw.name)
        } else {
            None
        };
        let deprecated = extract_deprecated(&kw.doc);
        let args = kw
            .args
            .iter()
            .map(|a| ArgSpec {
                name: a.name.clone(),
                kind: ArgKind::parse(&a.kind),
                default: a.default.clone(),
                types: a.types.clone(),
            })
            .collect();

        Self {
            name: kw.name.clone(),
            normalized_name,
            args,
            doc: kw.doc.clone(),
            deprecated,
            source: kw.source.as_ref().map(PathBuf::from),
            line_no: kw.lineno.map(|n| n as u32),
            is_embedded,
            embedded_regex,
            library_name: Some(library_name.to_owned()),
            is_private: kw.name.starts_with('_'),
        }
    }

    /// Create a `KeywordDoc` from a parsed resource file keyword.
    pub fn from_resource(
        name: &str,
        args: Vec<ArgSpec>,
        doc: String,
        source: Option<PathBuf>,
        line_no: Option<u32>,
    ) -> Self {
        let normalized_name = normalize_keyword_name(name);
        let is_embedded = name.contains("${");
        let embedded_regex = if is_embedded {
            build_embedded_regex(name)
        } else {
            None
        };
        let deprecated = extract_deprecated(&doc);
        Self {
            name: name.to_owned(),
            normalized_name,
            args,
            doc,
            deprecated,
            source,
            line_no,
            is_embedded,
            embedded_regex,
            library_name: None,
            is_private: name.starts_with('_'),
        }
    }

    /// Return the minimum number of required positional arguments.
    pub fn min_positional_args(&self) -> usize {
        self.args
            .iter()
            .filter(|a| {
                matches!(
                    a.kind,
                    ArgKind::PositionalOnly | ArgKind::PositionalOrKeyword
                ) && a.default.is_none()
            })
            .count()
    }

    /// Return the maximum positional capacity (`None` = unlimited via `*args`).
    pub fn max_positional_args(&self) -> Option<usize> {
        if self
            .args
            .iter()
            .any(|a| matches!(a.kind, ArgKind::VarPositional | ArgKind::VarKeyword))
        {
            return None;
        }
        Some(
            self.args
                .iter()
                .filter(|a| {
                    matches!(
                        a.kind,
                        ArgKind::PositionalOnly | ArgKind::PositionalOrKeyword
                    )
                })
                .count(),
        )
    }
}

// ---------------------------------------------------------------------------
// Variable definitions
// ---------------------------------------------------------------------------

/// Scope level at which a variable is defined.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum VariableScope {
    /// Built-in or command-line global variable.
    Global,
    /// Suite-level variable.
    Suite,
    /// Test/task-level variable.
    Test,
    /// Local (keyword body) variable.
    Local,
}

/// A variable definition visible from within a namespace.
#[derive(Debug, Clone)]
pub struct VariableDefinition {
    /// Raw variable token, e.g. `"${NAME}"`.
    pub name: String,
    /// Normalized base name (lowercase, no sigils/braces), e.g. `"name"`.
    pub normalized_name: String,
    /// Variable sigil (`$`, `@`, `&`, `%`).
    pub sigil: char,
    /// String representation of the default value, if statically known.
    pub value: Option<String>,
    /// Location where this variable is defined.
    pub range: Range,
    /// Source file path.
    pub source: Option<PathBuf>,
    /// Scope at which the variable is defined.
    pub scope: VariableScope,
}

impl VariableDefinition {
    /// Build from a raw variable name token (e.g. `"${MY_VAR}"`).
    pub fn from_name(
        name: &str,
        value: Option<String>,
        range: Range,
        source: Option<PathBuf>,
        scope: VariableScope,
    ) -> Option<Self> {
        let (sigil, base) = parse_variable_name(name)?;
        Some(Self {
            name: name.to_owned(),
            normalized_name: normalize_variable_name(base),
            sigil,
            value,
            range,
            source,
            scope,
        })
    }
}

// ---------------------------------------------------------------------------
// Import records
// ---------------------------------------------------------------------------

/// An `Library` import statement as found in a `.robot` file.
#[derive(Debug, Clone)]
pub struct LibraryImportRecord {
    pub name: String,
    pub args: Vec<String>,
    pub alias: Option<String>,
    /// Range of the import statement in the source file.
    pub range: Range,
}

/// A `Resource` import statement as found in a `.robot` file.
#[derive(Debug, Clone)]
pub struct ResourceImportRecord {
    pub path: String,
    pub range: Range,
}

/// A `Variables` import statement as found in a `.robot` file.
#[derive(Debug, Clone)]
pub struct VariablesImportRecord {
    pub path: String,
    pub args: Vec<String>,
    pub range: Range,
}

// ---------------------------------------------------------------------------
// Resolved entry types
// ---------------------------------------------------------------------------

/// A successfully resolved library import.
#[derive(Debug, Clone)]
pub struct LibraryEntry {
    pub name: String,
    pub alias: Option<String>,
    pub keywords: Vec<KeywordDoc>,
}

/// A successfully resolved resource file import.
#[derive(Debug, Clone)]
pub struct ResourceEntry {
    pub path: PathBuf,
    pub keywords: Vec<KeywordDoc>,
    pub variables: Vec<VariableDefinition>,
}

/// A successfully resolved variables file import.
#[derive(Debug, Clone)]
pub struct VariablesEntry {
    pub path: PathBuf,
    pub variables: Vec<VariableDefinition>,
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Normalize a keyword name for comparison.
///
/// Matches Robot Framework's `NormalizedString` behaviour:
/// - Convert to lowercase
/// - Remove all spaces and underscores
pub fn normalize_keyword_name(name: &str) -> String {
    name.chars()
        .filter(|&c| c != ' ' && c != '_')
        .map(|c| c.to_lowercase().next().unwrap_or(c))
        .collect()
}

/// Normalize a variable base name for comparison (lowercase only).
pub fn normalize_variable_name(name: &str) -> String {
    name.to_lowercase()
}

/// Parse a raw variable token like `"${MY_VAR}"` into `(sigil, base_name)`.
pub fn parse_variable_name(token: &str) -> Option<(char, &str)> {
    // Strip trailing `=` and whitespace (assignment-target syntax: `${VAR}=`).
    let token = token.trim_end_matches('=').trim_end();

    let mut chars = token.chars();
    let sigil = chars.next()?;
    if !matches!(sigil, '$' | '@' | '&' | '%') {
        return None;
    }
    let rest = &token[sigil.len_utf8()..];
    let inner = rest.strip_prefix('{')?.strip_suffix('}')?;
    Some((sigil, inner))
}

/// Attempt to build a regex from an embedded-argument keyword name.
pub fn build_embedded_regex(name: &str) -> Option<regex::Regex> {
    // Replace `${var}` patterns with a capture group that matches any text.
    let mut pattern = String::from("(?i)^");
    let mut remaining = name;
    while let Some(start) = remaining.find("${") {
        let prefix = &remaining[..start];
        pattern.push_str(&regex::escape(prefix));
        let end = remaining[start..].find('}')?;
        // The content of ${…} is the capture name — use a named group.
        let var_name = &remaining[start + 2..start + end];
        let safe_name: String = var_name
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        pattern.push_str(&format!("(?P<{safe_name}>.+?)"));
        remaining = &remaining[start + end + 1..];
    }
    pattern.push_str(&regex::escape(remaining));
    pattern.push('$');
    regex::Regex::new(&pattern).ok()
}

/// Extract the deprecation message from a keyword docstring.
///
/// Robot Framework marks deprecated keywords with `*DEPRECATED*` or
/// `*DEPRECATED: optional reason*` at the start of the docstring.
/// Returns the optional message suffix (e.g. `": reason"`) when the marker is
/// found, or an empty string when only `*DEPRECATED*` is present, so the
/// caller can append it cleanly to a base message.
/// Returns `None` when no deprecation marker is present.
fn extract_deprecated(doc: &str) -> Option<String> {
    // Find `*DEPRECATED` (case-insensitive) anchored at the start of the docstring.
    let trimmed = doc.trim_start();
    let upper = trimmed.to_uppercase();
    if !upper.starts_with("*DEPRECATED") {
        return None;
    }
    // Find the closing `*`.
    let after_star = &trimmed[1..]; // skip leading `*`
    let close = after_star.find('*')?;
    // Everything between `*DEPRECATED` and the closing `*` is the optional suffix.
    // E.g. `*DEPRECATED: use Foo instead*` → `: use Foo instead`
    //       `*DEPRECATED*`               → ``
    let inner = &after_star[..close]; // e.g. `DEPRECATED: use Foo instead`
    let message = inner["DEPRECATED".len()..].trim_end(); // strip the prefix word, keep `: reason`
    Some(message.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_keyword_name() {
        assert_eq!(normalize_keyword_name("Log Message"), "logmessage");
        assert_eq!(normalize_keyword_name("log_message"), "logmessage");
        assert_eq!(normalize_keyword_name("Log_Message"), "logmessage");
        assert_eq!(normalize_keyword_name("SHOULD BE EQUAL"), "shouldbeequal");
    }

    #[test]
    fn test_parse_variable_name() {
        assert_eq!(parse_variable_name("${MY_VAR}"), Some(('$', "MY_VAR")));
        assert_eq!(parse_variable_name("@{LIST}"), Some(('@', "LIST")));
        assert_eq!(parse_variable_name("&{DICT}"), Some(('&', "DICT")));
        assert_eq!(parse_variable_name("%{ENV}"), Some(('%', "ENV")));
        assert_eq!(parse_variable_name("${VAR}="), Some(('$', "VAR")));
        assert_eq!(parse_variable_name("not_a_var"), None);
    }

    #[test]
    fn test_build_embedded_regex() {
        let re = build_embedded_regex("The ${item} should be ${value}");
        assert!(re.is_some());
        let re = re.unwrap();
        assert!(re.is_match("The book should be great"));
        assert!(!re.is_match("The book is great"));
    }

    #[test]
    fn test_arg_kind_from_str() {
        assert_eq!(ArgKind::parse("POSITIONAL_ONLY"), ArgKind::PositionalOnly);
        assert_eq!(ArgKind::parse("VAR_POSITIONAL"), ArgKind::VarPositional);
        assert_eq!(ArgKind::parse("KEYWORD_ONLY"), ArgKind::KeywordOnly);
        assert_eq!(ArgKind::parse("VAR_KEYWORD"), ArgKind::VarKeyword);
        assert_eq!(
            ArgKind::parse("POSITIONAL_OR_KEYWORD"),
            ArgKind::PositionalOrKeyword
        );
    }
}
