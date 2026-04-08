//! Namespace — merged view of all imports visible within one `.robot` file.
//!
//! A [`Namespace`] collects all keywords and variables that are in scope for
//! a given file: those defined locally (in the file's Keywords / Variables
//! sections) plus those imported via `Library`, `Resource`, and `Variables`
//! statements.
//!
//! Mirrors the structure of `robotcode.robot.diagnostics.namespace`.

use std::path::PathBuf;

use super::entities::{
    normalize_keyword_name, KeywordDoc, LibraryEntry, LibraryImportRecord, ResourceEntry,
    ResourceImportRecord, VariableDefinition, VariablesEntry, VariablesImportRecord,
};
use super::keyword_finder::{strip_bdd_prefix, KeywordMatch};
use super::variable_scope::builtin_variables;

// ---------------------------------------------------------------------------
// Namespace
// ---------------------------------------------------------------------------

/// Merged keyword/variable scope for a single Robot Framework file.
#[derive(Debug, Default)]
pub struct Namespace {
    /// The source file path this namespace belongs to.
    pub source: Option<PathBuf>,
    /// Library import records from the Settings section.
    pub library_imports: Vec<LibraryImportRecord>,
    /// Resource import records from the Settings section.
    pub resource_imports: Vec<ResourceImportRecord>,
    /// Variables import records from the Settings section.
    pub variables_imports: Vec<VariablesImportRecord>,
    /// Keywords and library metadata from resolved Library imports.
    pub libraries: Vec<LibraryEntry>,
    /// Keywords and variables from resolved Resource imports.
    pub resources: Vec<ResourceEntry>,
    /// Variables from resolved Variables imports.
    pub variables_files: Vec<VariablesEntry>,
    /// Keywords defined in this file's Keywords section.
    pub local_keywords: Vec<KeywordDoc>,
    /// Variables defined in this file's Variables section.
    pub local_variables: Vec<VariableDefinition>,
}

impl Namespace {
    /// Create an empty namespace for the given source file.
    pub fn new(source: Option<PathBuf>) -> Self {
        Self {
            source,
            ..Default::default()
        }
    }

    /// Collect references to all keywords visible in this namespace.
    pub fn all_keywords(&self) -> Vec<&KeywordDoc> {
        let mut result: Vec<&KeywordDoc> = Vec::new();
        result.extend(self.local_keywords.iter());
        for res in &self.resources {
            result.extend(res.keywords.iter());
        }
        for lib in &self.libraries {
            result.extend(lib.keywords.iter());
        }
        result
    }

    /// Find a keyword by name, returning a [`KeywordMatch`] that borrows from `self`.
    pub fn find_keyword(&self, name: &str) -> KeywordMatch<'_> {
        let stripped = strip_bdd_prefix(name);

        // Qualified name check (e.g. `BuiltIn.Log`).
        if let Some(dot) = stripped.rfind('.') {
            let qualifier = normalize_keyword_name(&stripped[..dot]);
            let kw_name = normalize_keyword_name(&stripped[dot + 1..]);
            let mut matches: Vec<&KeywordDoc> = self
                .all_keywords()
                .into_iter()
                .filter(|kw| {
                    kw.normalized_name == kw_name
                        && kw
                            .library_name
                            .as_ref()
                            .map(|lib| normalize_keyword_name(lib) == qualifier)
                            .unwrap_or(false)
                })
                .collect();
            if matches.len() == 1 {
                return KeywordMatch::Found(matches.remove(0));
            }
            if matches.len() > 1 {
                return KeywordMatch::Ambiguous(matches);
            }
        }

        let normalized = normalize_keyword_name(stripped);

        // Exact normalized match.
        let exact: Vec<&KeywordDoc> = self
            .all_keywords()
            .into_iter()
            .filter(|kw| !kw.is_embedded && kw.normalized_name == normalized)
            .collect();
        match exact.len() {
            1 => return KeywordMatch::Found(exact[0]),
            n if n > 1 => return KeywordMatch::Ambiguous(exact),
            _ => {}
        }

        // Embedded argument regex match.
        let embedded: Vec<&KeywordDoc> = self
            .all_keywords()
            .into_iter()
            .filter(|kw| {
                kw.is_embedded
                    && kw
                        .embedded_regex
                        .as_ref()
                        .map(|re| re.is_match(stripped))
                        .unwrap_or(false)
            })
            .collect();
        match embedded.len() {
            1 => KeywordMatch::Found(embedded[0]),
            n if n > 1 => KeywordMatch::Ambiguous(embedded),
            _ => KeywordMatch::NotFound,
        }
    }

    /// Collect references to all suite-level variables visible in this namespace.
    pub fn all_suite_variables(&self) -> Vec<&VariableDefinition> {
        let mut result: Vec<&VariableDefinition> = Vec::new();
        result.extend(self.local_variables.iter());
        for vf in &self.variables_files {
            result.extend(vf.variables.iter());
        }
        for res in &self.resources {
            result.extend(res.variables.iter());
        }
        result
    }

    /// Find a suite-level variable by normalized name (does not check built-ins).
    pub fn find_suite_variable(&self, normalized_name: &str) -> Option<&VariableDefinition> {
        self.local_variables
            .iter()
            .chain(
                self.variables_files
                    .iter()
                    .flat_map(|vf| vf.variables.iter()),
            )
            .chain(self.resources.iter().flat_map(|r| r.variables.iter()))
            .find(|v| v.normalized_name == normalized_name)
    }

    /// Return `true` if `normalized_name` matches a built-in RF variable.
    pub fn is_builtin_variable(normalized_name: &str) -> bool {
        builtin_variables()
            .iter()
            .any(|v| v.normalized_name == normalized_name)
    }
}

#[cfg(test)]
mod tests {
    use lsp_types::{Position, Range};

    use super::*;
    use crate::diagnostics::entities::VariableScope;

    fn dummy_range() -> Range {
        Range {
            start: Position {
                line: 0,
                character: 0,
            },
            end: Position {
                line: 0,
                character: 0,
            },
        }
    }

    fn make_kw(name: &str, library: Option<&str>) -> KeywordDoc {
        KeywordDoc {
            name: name.to_owned(),
            normalized_name: normalize_keyword_name(name),
            args: vec![],
            doc: String::new(),
            deprecated: None,
            source: None,
            line_no: None,
            is_embedded: false,
            embedded_regex: None,
            library_name: library.map(str::to_owned),
            is_private: false,
        }
    }

    fn make_var(name: &str) -> VariableDefinition {
        VariableDefinition::from_name(name, None, dummy_range(), None, VariableScope::Suite)
            .unwrap()
    }

    #[test]
    fn test_find_keyword_local() {
        let mut ns = Namespace::new(None);
        ns.local_keywords.push(make_kw("My Keyword", None));
        assert!(matches!(
            ns.find_keyword("My Keyword"),
            KeywordMatch::Found(_)
        ));
    }

    #[test]
    fn test_find_keyword_from_library() {
        let mut ns = Namespace::new(None);
        ns.libraries.push(LibraryEntry {
            name: "BuiltIn".to_owned(),
            alias: None,
            keywords: vec![make_kw("Log", Some("BuiltIn"))],
        });
        assert!(matches!(ns.find_keyword("Log"), KeywordMatch::Found(_)));
    }

    #[test]
    fn test_find_keyword_not_found() {
        let ns = Namespace::new(None);
        assert!(matches!(ns.find_keyword("Missing"), KeywordMatch::NotFound));
    }

    #[test]
    fn test_find_keyword_ambiguous() {
        let mut ns = Namespace::new(None);
        ns.local_keywords.push(make_kw("Log", Some("BuiltIn")));
        ns.libraries.push(LibraryEntry {
            name: "MyLib".to_owned(),
            alias: None,
            keywords: vec![make_kw("Log", Some("MyLib"))],
        });
        assert!(matches!(ns.find_keyword("Log"), KeywordMatch::Ambiguous(_)));
    }

    #[test]
    fn test_find_keyword_qualified() {
        let mut ns = Namespace::new(None);
        ns.local_keywords.push(make_kw("Log", Some("BuiltIn")));
        ns.libraries.push(LibraryEntry {
            name: "MyLib".to_owned(),
            alias: None,
            keywords: vec![make_kw("Log", Some("MyLib"))],
        });
        if let KeywordMatch::Found(kw) = ns.find_keyword("BuiltIn.Log") {
            assert_eq!(kw.library_name.as_deref(), Some("BuiltIn"));
        } else {
            panic!("Expected Found");
        }
    }

    #[test]
    fn test_find_suite_variable() {
        let mut ns = Namespace::new(None);
        ns.local_variables.push(make_var("${MY_VAR}"));
        assert!(ns.find_suite_variable("my_var").is_some());
        assert!(ns.find_suite_variable("other").is_none());
    }

    #[test]
    fn test_is_builtin_variable() {
        assert!(Namespace::is_builtin_variable("empty"));
        assert!(Namespace::is_builtin_variable("true"));
        assert!(!Namespace::is_builtin_variable("nonexistent_builtin"));
    }

    #[test]
    fn test_all_keywords() {
        let mut ns = Namespace::new(None);
        ns.local_keywords.push(make_kw("Local KW", None));
        ns.libraries.push(LibraryEntry {
            name: "Lib".to_owned(),
            alias: None,
            keywords: vec![make_kw("Lib KW", Some("Lib"))],
        });
        let all = ns.all_keywords();
        assert_eq!(all.len(), 2);
    }
}
