//! Keyword finder — looks up keyword definitions by name within a namespace.
//!
//! Mirrors the logic in `robotcode.robot.diagnostics.keyword_finder`.
//!
//! Robot Framework keyword matching rules:
//! - Names are normalized: lowercase, spaces and underscores removed.
//! - A call may include a library/resource qualifier separated by a `.`
//!   (e.g. `BuiltIn.Log`).
//! - Embedded-argument keywords match via regex.
//! - Ambiguous matches (more than one keyword with the same normalized name
//!   from different sources) produce a `MULTIPLE_KEYWORDS` diagnostic.

use super::entities::{normalize_keyword_name, KeywordDoc};

// ---------------------------------------------------------------------------
// Match result
// ---------------------------------------------------------------------------

/// The result of a keyword lookup.
#[derive(Debug)]
pub enum KeywordMatch<'a> {
    /// Exactly one keyword found.
    Found(&'a KeywordDoc),
    /// Multiple keywords with the same name found (ambiguous).
    Ambiguous(Vec<&'a KeywordDoc>),
    /// No keyword found with the given name.
    NotFound,
}

// ---------------------------------------------------------------------------
// KeywordFinder
// ---------------------------------------------------------------------------

/// Searches a flat list of keyword definitions for a given name.
///
/// An instance is created per-namespace, wrapping the merged keyword list
/// assembled by [`super::namespace::Namespace`].
pub struct KeywordFinder<'a> {
    keywords: &'a [KeywordDoc],
}

impl<'a> KeywordFinder<'a> {
    /// Create a finder over `keywords`.
    pub fn new(keywords: &'a [KeywordDoc]) -> Self {
        Self { keywords }
    }

    /// Find a keyword by `name`, returning a [`KeywordMatch`].
    ///
    /// Search order mirrors the Python implementation:
    /// 1. Qualified name (e.g. `BuiltIn.Log`) — exact library+keyword match.
    /// 2. Exact normalized name match across all keywords.
    /// 3. Embedded-argument regex match.
    pub fn find(&self, name: &str) -> KeywordMatch<'_> {
        // Strip BDD prefixes (Given / When / Then / And / But).
        let stripped = strip_bdd_prefix(name);

        // Check for a qualified name (contains `.`).
        if let Some(dot) = stripped.rfind('.') {
            let qualifier = normalize_keyword_name(&stripped[..dot]);
            let kw_name = normalize_keyword_name(&stripped[dot + 1..]);
            let mut matches: Vec<&KeywordDoc> = self
                .keywords
                .iter()
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

        // Exact normalized name match.
        let exact: Vec<&KeywordDoc> = self
            .keywords
            .iter()
            .filter(|kw| !kw.is_embedded && kw.normalized_name == normalized)
            .collect();

        match exact.len() {
            1 => return KeywordMatch::Found(exact[0]),
            n if n > 1 => return KeywordMatch::Ambiguous(exact),
            _ => {}
        }

        // Embedded-argument regex match.
        let embedded: Vec<&KeywordDoc> = self
            .keywords
            .iter()
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

    /// Return `true` if `name` matches any keyword in the list.
    pub fn exists(&self, name: &str) -> bool {
        !matches!(self.find(name), KeywordMatch::NotFound)
    }
}

// ---------------------------------------------------------------------------
// BDD prefix stripping
// ---------------------------------------------------------------------------

/// Strip a BDD prefix from a keyword name.
///
/// Robot Framework treats `Given`, `When`, `Then`, `And`, `But` (case-
/// insensitive) as transparent prefixes in keyword calls.
pub fn strip_bdd_prefix(name: &str) -> &str {
    const PREFIXES: &[&str] = &["given ", "when ", "then ", "and ", "but "];
    let lower = name.to_lowercase();
    for prefix in PREFIXES {
        if lower.starts_with(prefix) {
            return name[prefix.len()..].trim_start();
        }
    }
    name
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostics::entities::{build_embedded_regex, ArgKind, ArgSpec, KeywordDoc};

    fn make_kw(name: &str, library: Option<&str>) -> KeywordDoc {
        KeywordDoc {
            name: name.to_owned(),
            normalized_name: normalize_keyword_name(name),
            args: vec![],
            doc: String::new(),
            deprecated: None,
            source: None,
            line_no: None,
            is_embedded: name.contains("${"),
            embedded_regex: if name.contains("${") {
                build_embedded_regex(name)
            } else {
                None
            },
            library_name: library.map(str::to_owned),
            is_private: name.starts_with('_'),
        }
    }

    // Minimal ArgSpec for testing (kept for future use)
    #[allow(dead_code)]
    fn dummy_arg(name: &str) -> ArgSpec {
        ArgSpec {
            name: name.to_owned(),
            kind: ArgKind::PositionalOrKeyword,
            default: None,
            types: vec![],
        }
    }

    #[test]
    fn test_find_exact_match() {
        let kws = vec![
            make_kw("Log", Some("BuiltIn")),
            make_kw("Fail", Some("BuiltIn")),
        ];
        let finder = KeywordFinder::new(&kws);
        assert!(matches!(finder.find("Log"), KeywordMatch::Found(_)));
        assert!(matches!(finder.find("log"), KeywordMatch::Found(_)));
        assert!(matches!(finder.find("LOG"), KeywordMatch::Found(_)));
        assert!(matches!(finder.find("log_message"), KeywordMatch::NotFound));
    }

    #[test]
    fn test_find_normalized_spaces() {
        let kws = vec![make_kw("Should Be Equal", Some("BuiltIn"))];
        let finder = KeywordFinder::new(&kws);
        assert!(matches!(
            finder.find("Should Be Equal"),
            KeywordMatch::Found(_)
        ));
        assert!(matches!(
            finder.find("should_be_equal"),
            KeywordMatch::Found(_)
        ));
        assert!(matches!(
            finder.find("SHOULD BE EQUAL"),
            KeywordMatch::Found(_)
        ));
    }

    #[test]
    fn test_find_ambiguous() {
        let kws = vec![
            make_kw("Log", Some("BuiltIn")),
            make_kw("Log", Some("MyLib")),
        ];
        let finder = KeywordFinder::new(&kws);
        assert!(matches!(finder.find("Log"), KeywordMatch::Ambiguous(_)));
    }

    #[test]
    fn test_find_not_found() {
        let kws: Vec<KeywordDoc> = vec![];
        let finder = KeywordFinder::new(&kws);
        assert!(matches!(finder.find("Nonexistent"), KeywordMatch::NotFound));
    }

    #[test]
    fn test_find_qualified_name() {
        let kws = vec![
            make_kw("Log", Some("BuiltIn")),
            make_kw("Log", Some("MyLib")),
        ];
        let finder = KeywordFinder::new(&kws);
        if let KeywordMatch::Found(kw) = finder.find("BuiltIn.Log") {
            assert_eq!(kw.library_name.as_deref(), Some("BuiltIn"));
        } else {
            panic!("Expected Found");
        }
    }

    #[test]
    fn test_find_embedded_args() {
        let kws = vec![make_kw("The ${item} should be ${value}", None)];
        let finder = KeywordFinder::new(&kws);
        assert!(matches!(
            finder.find("The book should be great"),
            KeywordMatch::Found(_)
        ));
        assert!(matches!(
            finder.find("Nonexistent keyword"),
            KeywordMatch::NotFound
        ));
    }

    #[test]
    fn test_strip_bdd_prefix() {
        assert_eq!(strip_bdd_prefix("Given I log a message"), "I log a message");
        assert_eq!(
            strip_bdd_prefix("when something happens"),
            "something happens"
        );
        assert_eq!(strip_bdd_prefix("Log Message"), "Log Message");
    }

    #[test]
    fn test_exists() {
        let kws = vec![make_kw("Log", Some("BuiltIn"))];
        let finder = KeywordFinder::new(&kws);
        assert!(finder.exists("Log"));
        assert!(!finder.exists("Missing"));
    }
}
