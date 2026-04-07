//! Robot Framework variable scope tracking.
//!
//! Implements RF's four-level variable scoping:
//! - **Global** — available everywhere; set by `Set Global Variable` or CLI
//! - **Suite** — available for the duration of a suite; set by `Set Suite Variable`
//!   or in the Variables section
//! - **Test** — available for the duration of a test case; set by `Set Test Variable`
//! - **Local** — available within a keyword body; regular assignment (`${x}=`)
//!
//! Mirrors the scoping logic in `robotcode.robot.diagnostics.variable_scope`.

use std::path::PathBuf;

use lsp_types::{Position, Range};

use super::entities::{
    normalize_variable_name, parse_variable_name, VariableDefinition, VariableScope,
};

// ---------------------------------------------------------------------------
// Variable scope tracker
// ---------------------------------------------------------------------------

/// Tracks variable definitions visible at the current point of analysis.
///
/// The tracker maintains a stack of scopes (pushed/popped for control-flow
/// blocks) plus a merged view of all outer scopes.
pub struct VariableScopeTracker {
    /// Variables from outer scopes (suite-level and above).
    outer: Vec<VariableDefinition>,
    /// Stack of local scope frames.  The last element is the innermost scope.
    stack: Vec<Vec<VariableDefinition>>,
}

impl VariableScopeTracker {
    /// Create an empty tracker.
    pub fn new() -> Self {
        Self {
            outer: Vec::new(),
            stack: vec![Vec::new()], // at least one local frame
        }
    }

    /// Define a variable in the *current* scope frame.
    ///
    /// Returns `true` if the variable was newly defined; `false` if it already
    /// existed in the current frame (a re-definition/shadowing).
    pub fn define(&mut self, def: VariableDefinition) -> bool {
        let frame = self.stack.last_mut().expect("scope stack is never empty");
        let already = frame
            .iter()
            .any(|v| v.normalized_name == def.normalized_name);
        frame.push(def);
        !already
    }

    /// Define a variable in the *outer* (suite or global) scope.
    pub fn define_outer(&mut self, def: VariableDefinition) {
        self.outer.push(def);
    }

    /// Look up a variable by normalized name, searching from innermost to outermost scope.
    pub fn lookup(&self, normalized_name: &str) -> Option<&VariableDefinition> {
        // Search from innermost stack frame outwards.
        for frame in self.stack.iter().rev() {
            if let Some(def) = frame
                .iter()
                .rev()
                .find(|v| v.normalized_name == normalized_name)
            {
                return Some(def);
            }
        }
        // Fall back to outer (suite/global) scope.
        self.outer
            .iter()
            .rev()
            .find(|v| v.normalized_name == normalized_name)
    }

    /// Return `true` if the variable is in scope.
    pub fn is_defined(&self, normalized_name: &str) -> bool {
        self.lookup(normalized_name).is_some()
    }

    /// Push a new local scope frame (e.g. entering a FOR body).
    pub fn push_scope(&mut self) {
        self.stack.push(Vec::new());
    }

    /// Pop the innermost local scope frame.
    pub fn pop_scope(&mut self) {
        if self.stack.len() > 1 {
            self.stack.pop();
        }
    }

    /// Collect all currently visible variable definitions (deduplicated, innermost wins).
    pub fn all_visible(&self) -> Vec<&VariableDefinition> {
        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();
        for frame in self.stack.iter().rev() {
            for def in frame.iter().rev() {
                if seen.insert(def.normalized_name.clone()) {
                    result.push(def);
                }
            }
        }
        for def in self.outer.iter().rev() {
            if seen.insert(def.normalized_name.clone()) {
                result.push(def);
            }
        }
        result
    }
}

impl Default for VariableScopeTracker {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Built-in variables
// ---------------------------------------------------------------------------

/// Return the set of built-in RF variables that are always in scope.
///
/// These match the variables defined in `robot.variables.scopes.GlobalVariables`.
pub fn builtin_variables() -> Vec<VariableDefinition> {
    let names = [
        "${EMPTY}",
        "${SPACE}",
        "${TRUE}",
        "${FALSE}",
        "${NULL}",
        "${NONE}",
        "${OUTPUT_DIR}",
        "${OUTPUT_FILE}",
        "${REPORT_FILE}",
        "${LOG_FILE}",
        "${DEBUG_FILE}",
        "${LOG_LEVEL}",
        "${SUITE_NAME}",
        "${SUITE_SOURCE}",
        "${SUITE_DOCUMENTATION}",
        "${SUITE_STATUS}",
        "${SUITE_MESSAGE}",
        "${KEYWORD_STATUS}",
        "${KEYWORD_MESSAGE}",
        "${TEST_NAME}",
        "${TEST_DOCUMENTATION}",
        "${TEST_STATUS}",
        "${TEST_MESSAGE}",
        "${PREV_TEST_NAME}",
        "${PREV_TEST_STATUS}",
        "${PREV_TEST_MESSAGE}",
        "${TEMPDIR}",
        "${EXECDIR}",
        "${/}",
        "${:}",
        "${\\n}",
        "@{EMPTY}",
        "&{EMPTY}",
    ];

    let dummy_range = Range {
        start: Position {
            line: 0,
            character: 0,
        },
        end: Position {
            line: 0,
            character: 0,
        },
    };

    names
        .iter()
        .filter_map(|name| {
            VariableDefinition::from_name(name, None, dummy_range, None, VariableScope::Global)
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Assignment target parsing
// ---------------------------------------------------------------------------

/// Parse an assignment target string and return a [`VariableDefinition`].
///
/// Handles both `${var}=` and `${var}` assignment styles.
pub fn parse_assignment(
    raw: &str,
    range: Range,
    source: Option<PathBuf>,
) -> Option<VariableDefinition> {
    // Strip trailing `=` and whitespace.
    let cleaned = raw.trim_end_matches('=').trim_end();
    // Ensure it is a valid variable token.
    let (_, _) = parse_variable_name(cleaned)?;
    VariableDefinition::from_name(cleaned, None, range, source, VariableScope::Local)
}

// ---------------------------------------------------------------------------
// Keyword names that mutate scope
// ---------------------------------------------------------------------------

/// Built-in keyword names (normalized) that perform variable assignment
/// to elevated scopes.
pub const SET_SUITE_VARIABLE: &str = "setsuitevariable";
pub const SET_GLOBAL_VARIABLE: &str = "setglobalvariable";
pub const SET_TEST_VARIABLE: &str = "settestvariable";
pub const SET_TASK_VARIABLE: &str = "settaskvariable";
pub const SET_LOCAL_VARIABLE: &str = "setlocalvariable";

/// Return `true` if a normalized keyword name is a `Set *Variable` call.
pub fn is_set_variable_keyword(normalized_name: &str) -> bool {
    matches!(
        normalized_name,
        n if n == SET_SUITE_VARIABLE
            || n == SET_GLOBAL_VARIABLE
            || n == SET_TEST_VARIABLE
            || n == SET_TASK_VARIABLE
            || n == SET_LOCAL_VARIABLE
    )
}

/// Determine the [`VariableScope`] that a `Set *Variable` keyword writes to.
pub fn set_variable_scope(normalized_name: &str) -> VariableScope {
    match normalized_name {
        n if n == SET_GLOBAL_VARIABLE => VariableScope::Global,
        n if n == SET_SUITE_VARIABLE => VariableScope::Suite,
        n if n == SET_TEST_VARIABLE || n == SET_TASK_VARIABLE => VariableScope::Test,
        _ => VariableScope::Local,
    }
}

/// Try to extract a variable name from the first argument of a `Set *Variable` call.
pub fn extract_set_variable_name(args: &[String]) -> Option<&str> {
    args.first().map(String::as_str)
}

/// Build a variable definition from a `Set *Variable` call.
pub fn definition_from_set_variable(
    keyword_normalized: &str,
    args: &[String],
    range: Range,
    source: Option<PathBuf>,
) -> Option<VariableDefinition> {
    let name = extract_set_variable_name(args)?;
    let scope = set_variable_scope(keyword_normalized);
    let normalized = parse_variable_name(name).map(|(_, base)| normalize_variable_name(base))?;
    Some(VariableDefinition {
        name: name.to_owned(),
        normalized_name: normalized,
        sigil: name.chars().next()?,
        value: args.get(1).cloned(),
        range,
        source,
        scope,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

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

    fn make_var(name: &str, scope: VariableScope) -> VariableDefinition {
        VariableDefinition::from_name(name, None, dummy_range(), None, scope).unwrap()
    }

    #[test]
    fn test_define_and_lookup() {
        let mut tracker = VariableScopeTracker::new();
        tracker.define(make_var("${FOO}", VariableScope::Local));
        assert!(tracker.is_defined("foo"));
        assert!(!tracker.is_defined("bar"));
    }

    #[test]
    fn test_outer_scope_fallback() {
        let mut tracker = VariableScopeTracker::new();
        tracker.define_outer(make_var("${SUITE_VAR}", VariableScope::Suite));
        assert!(tracker.is_defined("suite_var"));
    }

    #[test]
    fn test_push_pop_scope() {
        let mut tracker = VariableScopeTracker::new();
        tracker.define(make_var("${OUTER}", VariableScope::Local));
        tracker.push_scope();
        tracker.define(make_var("${INNER}", VariableScope::Local));
        assert!(tracker.is_defined("outer"));
        assert!(tracker.is_defined("inner"));
        tracker.pop_scope();
        assert!(tracker.is_defined("outer"));
        assert!(!tracker.is_defined("inner"));
    }

    #[test]
    fn test_builtin_variables() {
        let builtins = builtin_variables();
        assert!(!builtins.is_empty());
        // ${EMPTY} should always be present
        assert!(builtins.iter().any(|v| v.normalized_name == "empty"));
        // ${TRUE} should always be present
        assert!(builtins.iter().any(|v| v.normalized_name == "true"));
    }

    #[test]
    fn test_parse_assignment() {
        let r = dummy_range();
        let def = parse_assignment("${MY_VAR}=", r, None).unwrap();
        assert_eq!(def.normalized_name, "my_var");
        assert_eq!(def.sigil, '$');
    }

    #[test]
    fn test_is_set_variable_keyword() {
        assert!(is_set_variable_keyword(SET_SUITE_VARIABLE));
        assert!(is_set_variable_keyword(SET_GLOBAL_VARIABLE));
        assert!(!is_set_variable_keyword("log"));
    }

    #[test]
    fn test_definition_from_set_variable() {
        let r = dummy_range();
        let args = vec!["${MY_VAR}".to_owned(), "value".to_owned()];
        let def = definition_from_set_variable(SET_SUITE_VARIABLE, &args, r, None).unwrap();
        assert_eq!(def.scope, VariableScope::Suite);
        assert_eq!(def.normalized_name, "my_var");
    }
}
