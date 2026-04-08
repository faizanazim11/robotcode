//! Diagnostic error codes for the RobotCode diagnostics engine.
//!
//! All code constants match the Python `Error` class in
//! `robotcode.robot.diagnostics.errors` exactly, so that diagnostic codes
//! are consistent between the Python and Rust implementations.

use lsp_types::DiagnosticSeverity;

/// The LSP `source` field value used in all diagnostics produced by RobotCode.
pub const SOURCE: &str = "robotcode";

// ---------------------------------------------------------------------------
// Error codes (mirror Python Error class constants)
// ---------------------------------------------------------------------------

pub const VARIABLE_NOT_FOUND: &str = "VariableNotFound";
pub const VARIABLE_NOT_REPLACED: &str = "VariableNotReplaced";
pub const ENVIRONMENT_VARIABLE_NOT_FOUND: &str = "EnvironmentVariableNotFound";
pub const ENVIRONMENT_VARIABLE_NOT_REPLACED: &str = "EnvironmentVariableNotReplaced";
pub const KEYWORD_NOT_FOUND: &str = "KeywordNotFound";
pub const LIBRARY_CONTAINS_NO_KEYWORDS: &str = "LibraryContainsNoKeywords";
pub const POSSIBLE_CIRCULAR_IMPORT: &str = "PossibleCircularImport";
pub const CIRCULAR_IMPORT: &str = "CircularImport";
pub const RESOURCE_EMPTY: &str = "ResourceEmpty";
pub const IMPORT_CONTAINS_ERRORS: &str = "ImportContainsErrors";
pub const RECURSIVE_IMPORT: &str = "RecursiveImport";
pub const RESOURCE_ALREADY_IMPORTED: &str = "ResourceAlreadyImported";
pub const VARIABLES_ALREADY_IMPORTED: &str = "VariablesAlreadyImported";
pub const LIBRARY_ALREADY_IMPORTED: &str = "LibraryAlreadyImported";
pub const LIBRARY_OVERRIDES_BUILTIN: &str = "LibraryOverridesBuiltIn";
pub const DEPRECATED_KEYWORD: &str = "DeprecatedKeyword";
pub const KEYWORD_CONTAINS_ERRORS: &str = "KeywordContainsErrors";
pub const RESERVED_KEYWORD: &str = "ReservedKeyword";
pub const PRIVATE_KEYWORD: &str = "PrivateKeyword";
pub const INCORRECT_USE: &str = "IncorrectUse";
pub const KEYWORD_NAME_EMPTY: &str = "KeywordNameEmpty";
pub const CODE_UNREACHABLE: &str = "CodeUnreachable";
pub const TESTCASE_NAME_EMPTY: &str = "TestCaseNameEmpty";
pub const KEYWORD_CONTAINS_NORMAL_AND_EMBEDDED_ARGUMENTS: &str =
    "KeywordContainsNormalAndEmbbededArguments";
pub const DEPRECATED_HYPHEN_TAG: &str = "DeprecatedHyphenTag";
pub const DEPRECATED_RETURN_SETTING: &str = "DeprecatedReturnSetting";
pub const DEPRECATED_FORCE_TAG: &str = "DeprecatedForceTag";
pub const IMPORT_REQUIRES_VALUE: &str = "ImportRequiresValue";
pub const KEYWORD_ERROR: &str = "KeywordError";
pub const MULTIPLE_KEYWORDS: &str = "MultipleKeywords";
pub const CONFLICTING_LIBRARY_KEYWORDS: &str = "ConflictingLibraryKeywords";
pub const INVALID_HEADER: &str = "InvalidHeader";
pub const DEPRECATED_HEADER: &str = "DeprecatedHeader";
pub const OVERRIDDEN_BY_COMMANDLINE: &str = "OverriddenByCommandLine";
pub const OVERRIDES_IMPORTED_VARIABLE: &str = "OverridesImportedVariable";
pub const VARIABLE_ALREADY_DEFINED: &str = "VariableAlreadyDefined";
pub const VARIABLE_OVERRIDDEN: &str = "VariableOverridden";
pub const MODEL_ERROR: &str = "ModelError";
pub const TOKEN_ERROR: &str = "TokenError";
pub const ASSIGN_MARK_ALLOWED_ONLY_ON_LAST_VAR: &str = "AssignmentMarkAllowedOnlyOnLastVariable";
pub const KEYWORD_ALREADY_DEFINED: &str = "KeywordAlreadyDefined";

// ---------------------------------------------------------------------------
// Severity mapping
// ---------------------------------------------------------------------------

/// Return the default [`DiagnosticSeverity`] for a given error code.
///
/// Matches the severity assignments in the Python implementation.
pub fn default_severity(code: &str) -> DiagnosticSeverity {
    match code {
        // Hints / informational
        DEPRECATED_KEYWORD
        | DEPRECATED_HYPHEN_TAG
        | DEPRECATED_RETURN_SETTING
        | DEPRECATED_FORCE_TAG
        | DEPRECATED_HEADER
        | OVERRIDDEN_BY_COMMANDLINE
        | CODE_UNREACHABLE => DiagnosticSeverity::HINT,

        // Warnings
        LIBRARY_CONTAINS_NO_KEYWORDS
        | POSSIBLE_CIRCULAR_IMPORT
        | RESOURCE_EMPTY
        | RESOURCE_ALREADY_IMPORTED
        | VARIABLES_ALREADY_IMPORTED
        | LIBRARY_ALREADY_IMPORTED
        | LIBRARY_OVERRIDES_BUILTIN
        | PRIVATE_KEYWORD
        | VARIABLE_OVERRIDDEN
        | OVERRIDES_IMPORTED_VARIABLE
        | VARIABLE_ALREADY_DEFINED => DiagnosticSeverity::WARNING,

        // Errors (everything else)
        _ => DiagnosticSeverity::ERROR,
    }
}

// ---------------------------------------------------------------------------
// Diagnostic builder helpers
// ---------------------------------------------------------------------------

/// Build an LSP [`lsp_types::Diagnostic`] for a given code and message.
pub fn make_diagnostic(
    range: lsp_types::Range,
    code: &str,
    message: impl Into<String>,
) -> lsp_types::Diagnostic {
    lsp_types::Diagnostic {
        range,
        severity: Some(default_severity(code)),
        code: Some(lsp_types::NumberOrString::String(code.to_owned())),
        source: Some(SOURCE.to_owned()),
        message: message.into(),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_codes_are_non_empty() {
        assert!(!VARIABLE_NOT_FOUND.is_empty());
        assert!(!KEYWORD_NOT_FOUND.is_empty());
        assert!(!MULTIPLE_KEYWORDS.is_empty());
    }

    #[test]
    fn test_severity_mapping() {
        assert_eq!(
            default_severity(DEPRECATED_KEYWORD),
            DiagnosticSeverity::HINT
        );
        assert_eq!(
            default_severity(LIBRARY_ALREADY_IMPORTED),
            DiagnosticSeverity::WARNING
        );
        assert_eq!(
            default_severity(KEYWORD_NOT_FOUND),
            DiagnosticSeverity::ERROR
        );
        assert_eq!(
            default_severity(VARIABLE_NOT_FOUND),
            DiagnosticSeverity::ERROR
        );
    }

    #[test]
    fn test_make_diagnostic() {
        let range = lsp_types::Range::default();
        let d = make_diagnostic(range, KEYWORD_NOT_FOUND, "No keyword 'Foo' found");
        assert_eq!(d.source.as_deref(), Some(SOURCE));
        assert_eq!(
            d.code,
            Some(lsp_types::NumberOrString::String(
                KEYWORD_NOT_FOUND.to_owned()
            ))
        );
        assert_eq!(d.severity, Some(DiagnosticSeverity::ERROR));
    }
}
