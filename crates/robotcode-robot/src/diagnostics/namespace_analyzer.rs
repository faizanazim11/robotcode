//! Namespace analyzer — walks the Robot Framework AST and emits diagnostics.
//!
//! This is the heart of the diagnostics engine.  [`NamespaceAnalyzer`] takes
//! a parsed [`robotcode_rf_parser::parser::ast::File`] and a populated
//! [`Namespace`], walks every body item, and emits [`lsp_types::Diagnostic`]
//! structs for:
//!
//! - Undefined keyword calls
//! - Ambiguous (multiple) keyword matches
//! - Deprecated keyword usage
//! - Private keyword usage from outside the defining file
//! - Undefined variable references
//! - Duplicate keyword names in the Keywords section
//! - Import errors (recorded on the namespace)
//!
//! Mirrors the structure of `robotcode.robot.diagnostics.namespace_analyzer`.

use lsp_types::{Diagnostic, Position, Range};

use robotcode_rf_parser::parser::ast::{
    BodyItem, File, Keyword, Section, SettingItem, Task, TestCase, VariableItem,
};
use robotcode_rf_parser::variables::{contains_variable, search_variable};

use super::entities::{normalize_keyword_name, VariableDefinition, VariableScope};
use super::errors::{
    make_diagnostic, DEPRECATED_KEYWORD, KEYWORD_ALREADY_DEFINED, KEYWORD_NOT_FOUND,
    MULTIPLE_KEYWORDS, PRIVATE_KEYWORD, VARIABLE_NOT_FOUND,
};
use super::keyword_finder::KeywordMatch;
use super::namespace::Namespace;
use super::variable_scope::{
    builtin_variables, definition_from_set_variable, is_set_variable_keyword, parse_assignment,
    VariableScopeTracker,
};

// ---------------------------------------------------------------------------
// Analysis output
// ---------------------------------------------------------------------------

/// The result of analyzing one document.
#[derive(Debug, Default)]
pub struct AnalysisResult {
    /// All diagnostics emitted for this document.
    pub diagnostics: Vec<Diagnostic>,
}

// ---------------------------------------------------------------------------
// NamespaceAnalyzer
// ---------------------------------------------------------------------------

/// Analyzes a parsed RF file against a [`Namespace`] and collects diagnostics.
pub struct NamespaceAnalyzer<'a> {
    namespace: &'a Namespace,
    source_path: Option<std::path::PathBuf>,
    /// When `false`, keyword-existence checks (`KeywordNotFound`,
    /// `MultipleKeywords`) are suppressed.  Set to `false` when the namespace
    /// has no resolved imports (libraries or resources) so that incompletely
    /// resolved namespaces do not produce a flood of false-positive diagnostics.
    check_keywords: bool,
}

impl<'a> NamespaceAnalyzer<'a> {
    /// Create a new analyzer for `namespace`.
    ///
    /// Keyword-existence checks are automatically enabled only when the
    /// namespace has at least one resolved library or resource file, preventing
    /// false-positive `KeywordNotFound` diagnostics on files whose imports
    /// haven't been resolved yet.
    pub fn new(namespace: &'a Namespace) -> Self {
        let check_keywords = !namespace.libraries.is_empty() || !namespace.resources.is_empty();
        Self {
            namespace,
            source_path: namespace.source.clone(),
            check_keywords,
        }
    }

    /// Override whether keyword-existence checks are run.
    pub fn with_keyword_checks(mut self, enabled: bool) -> Self {
        self.check_keywords = enabled;
        self
    }

    /// Analyze `file` and return all diagnostics.
    pub fn analyze(&self, file: &File) -> AnalysisResult {
        let mut result = AnalysisResult::default();
        let mut scope = VariableScopeTracker::new();

        // Seed the scope with built-in variables.
        for var in builtin_variables() {
            scope.define_outer(var);
        }

        // Seed with suite-level variables from the namespace.
        for var in self.namespace.all_suite_variables() {
            scope.define_outer(var.clone());
        }

        // Check for duplicate keyword names in the Keywords section.
        self.check_duplicate_keywords(file, &mut result);

        for section in &file.sections {
            match section {
                Section::Settings(s) => self.analyze_settings_section(s, &mut result),
                Section::Variables(s) => self.analyze_variables_section(s, &mut result, &mut scope),
                Section::TestCases(s) => {
                    for tc in &s.body {
                        self.analyze_test_case(tc, &mut result, &mut scope);
                    }
                }
                Section::Tasks(s) => {
                    for task in &s.body {
                        self.analyze_task(task, &mut result, &mut scope);
                    }
                }
                Section::Keywords(s) => {
                    for kw in &s.body {
                        self.analyze_keyword_def(kw, &mut result, &mut scope);
                    }
                }
                Section::Comments(_) | Section::Invalid(_) => {}
            }
        }

        result
    }

    // -----------------------------------------------------------------------
    // Duplicate keyword detection
    // -----------------------------------------------------------------------

    fn check_duplicate_keywords(&self, file: &File, result: &mut AnalysisResult) {
        use std::collections::HashMap;
        let mut seen: HashMap<String, Vec<Range>> = HashMap::new();

        for section in &file.sections {
            if let Section::Keywords(s) = section {
                for kw in &s.body {
                    let normalized = normalize_keyword_name(&kw.name);
                    let range = pos_to_range(&kw.position);
                    seen.entry(normalized).or_default().push(range);
                }
            }
        }

        for (normalized, ranges) in &seen {
            if ranges.len() > 1 {
                for range in ranges {
                    result.diagnostics.push(make_diagnostic(
                        *range,
                        KEYWORD_ALREADY_DEFINED,
                        format!("Keyword '{normalized}' is already defined"),
                    ));
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // Settings section
    // -----------------------------------------------------------------------

    fn analyze_settings_section(
        &self,
        section: &robotcode_rf_parser::parser::ast::SettingsSection,
        result: &mut AnalysisResult,
    ) {
        for item in &section.body {
            match item {
                SettingItem::SuiteSetup(f)
                | SettingItem::SuiteTeardown(f)
                | SettingItem::TestSetup(f)
                | SettingItem::TestTeardown(f) => {
                    self.check_keyword_call(&f.name, &f.args, pos_to_range(&f.position), result);
                }
                SettingItem::TestTemplate(t) => {
                    self.check_keyword_call(&t.name, &[], pos_to_range(&t.position), result);
                }
                _ => {}
            }
        }
    }

    // -----------------------------------------------------------------------
    // Variables section
    // -----------------------------------------------------------------------

    fn analyze_variables_section(
        &self,
        section: &robotcode_rf_parser::parser::ast::VariablesSection,
        _result: &mut AnalysisResult,
        scope: &mut VariableScopeTracker,
    ) {
        for item in &section.body {
            if let VariableItem::Variable(v) = item {
                let range = pos_to_range(&v.position);
                if let Some(def) = VariableDefinition::from_name(
                    &v.name,
                    v.value.first().cloned(),
                    range,
                    self.source_path.clone(),
                    VariableScope::Suite,
                ) {
                    scope.define_outer(def);
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // Test case / Task
    // -----------------------------------------------------------------------

    fn analyze_test_case(
        &self,
        tc: &TestCase,
        result: &mut AnalysisResult,
        outer_scope: &mut VariableScopeTracker,
    ) {
        outer_scope.push_scope();
        for item in &tc.body {
            self.analyze_body_item(item, result, outer_scope);
        }
        outer_scope.pop_scope();
    }

    fn analyze_task(
        &self,
        task: &Task,
        result: &mut AnalysisResult,
        outer_scope: &mut VariableScopeTracker,
    ) {
        outer_scope.push_scope();
        for item in &task.body {
            self.analyze_body_item(item, result, outer_scope);
        }
        outer_scope.pop_scope();
    }

    // -----------------------------------------------------------------------
    // Keyword definition
    // -----------------------------------------------------------------------

    fn analyze_keyword_def(
        &self,
        kw: &Keyword,
        result: &mut AnalysisResult,
        outer_scope: &mut VariableScopeTracker,
    ) {
        outer_scope.push_scope();

        // Add keyword arguments to scope.
        for item in &kw.body {
            if let BodyItem::Arguments(args) = item {
                for arg in &args.args {
                    let range = pos_to_range(&args.position);
                    // Strip default value (arg format: `${name}=default`).
                    let name_part = if let Some(eq) = arg.find('=') {
                        &arg[..eq]
                    } else {
                        arg.as_str()
                    };
                    if let Some(def) = VariableDefinition::from_name(
                        name_part,
                        None,
                        range,
                        self.source_path.clone(),
                        VariableScope::Local,
                    ) {
                        outer_scope.define(def);
                    }
                }
                break; // only first [Arguments] block
            }
        }

        for item in &kw.body {
            self.analyze_body_item(item, result, outer_scope);
        }
        outer_scope.pop_scope();
    }

    // -----------------------------------------------------------------------
    // Body items
    // -----------------------------------------------------------------------

    fn analyze_body_item(
        &self,
        item: &BodyItem,
        result: &mut AnalysisResult,
        scope: &mut VariableScopeTracker,
    ) {
        match item {
            BodyItem::KeywordCall(kw_call) => {
                // Record any assignment targets as local variables.
                for assign in &kw_call.assigns {
                    let range = pos_to_range(&kw_call.position);
                    if let Some(def) = parse_assignment(assign, range, self.source_path.clone()) {
                        scope.define(def);
                    }
                }

                let kw_name = kw_call.name.trim();
                if kw_name.is_empty() {
                    return;
                }

                let range = pos_to_range(&kw_call.position);
                self.check_keyword_call(kw_name, &kw_call.args, range, result);

                // Handle `Set * Variable` calls — define the variable in the
                // appropriate scope based on the keyword's intent.
                let normalized = normalize_keyword_name(kw_name);
                if is_set_variable_keyword(&normalized) {
                    if let Some(def) = definition_from_set_variable(
                        &normalized,
                        &kw_call.args,
                        range,
                        self.source_path.clone(),
                    ) {
                        // Suite- and global-scope variables must be visible
                        // beyond the current local frame; local/test variables
                        // only live in the current frame.
                        match def.scope {
                            VariableScope::Global | VariableScope::Suite => {
                                scope.define_outer(def);
                            }
                            VariableScope::Test | VariableScope::Local => {
                                scope.define(def);
                            }
                        }
                    }
                }

                // Check variable references in arguments.
                for arg in &kw_call.args {
                    self.check_variable_refs(arg, range, result, scope);
                }
            }

            BodyItem::For(f) => {
                scope.push_scope();
                // FOR loop variables.
                for var in &f.variables {
                    let range = pos_to_range(&f.position);
                    if let Some(def) = VariableDefinition::from_name(
                        var,
                        None,
                        range,
                        self.source_path.clone(),
                        VariableScope::Local,
                    ) {
                        scope.define(def);
                    }
                }
                for body_item in &f.body {
                    self.analyze_body_item(body_item, result, scope);
                }
                scope.pop_scope();
            }

            BodyItem::While(w) => {
                scope.push_scope();
                if let Some(cond) = &w.condition {
                    let range = pos_to_range(&w.position);
                    self.check_variable_refs(cond, range, result, scope);
                }
                for body_item in &w.body {
                    self.analyze_body_item(body_item, result, scope);
                }
                scope.pop_scope();
            }

            BodyItem::If(if_block) => {
                for branch in &if_block.branches {
                    scope.push_scope();
                    for body_item in &branch.body {
                        self.analyze_body_item(body_item, result, scope);
                    }
                    scope.pop_scope();
                }
            }

            BodyItem::Try(try_block) => {
                for branch in &try_block.branches {
                    scope.push_scope();
                    // Add exception variable to scope if present.
                    if let Some(var) = &branch.var {
                        let range = pos_to_range(&branch.position);
                        if let Some(def) = VariableDefinition::from_name(
                            var,
                            None,
                            range,
                            self.source_path.clone(),
                            VariableScope::Local,
                        ) {
                            scope.define(def);
                        }
                    }
                    for body_item in &branch.body {
                        self.analyze_body_item(body_item, result, scope);
                    }
                    scope.pop_scope();
                }
            }

            // Inline settings (setup/teardown in keywords)
            BodyItem::Setup(f) | BodyItem::Teardown(f) => {
                let range = pos_to_range(&f.position);
                self.check_keyword_call(&f.name, &f.args, range, result);
            }

            BodyItem::Template(t) => {
                let range = pos_to_range(&t.position);
                self.check_keyword_call(&t.name, &[], range, result);
            }

            // No analysis needed for these.
            BodyItem::Documentation(_)
            | BodyItem::Arguments(_)
            | BodyItem::Tags(_)
            | BodyItem::Timeout(_)
            | BodyItem::ReturnSetting(_)
            | BodyItem::TemplateArguments(_)
            | BodyItem::Break(_)
            | BodyItem::Continue(_)
            | BodyItem::Return(_)
            | BodyItem::Comment(_)
            | BodyItem::EmptyLine(_)
            | BodyItem::Error(_) => {}
        }
    }

    // -----------------------------------------------------------------------
    // Keyword call checking
    // -----------------------------------------------------------------------

    fn check_keyword_call(
        &self,
        name: &str,
        _args: &[String],
        range: Range,
        result: &mut AnalysisResult,
    ) {
        // Skip NONE/empty keyword names.
        let trimmed = name.trim();
        if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("none") {
            return;
        }

        match self.namespace.find_keyword(trimmed) {
            KeywordMatch::Found(kw) => {
                // Deprecated keyword.
                if let Some(notice) = &kw.deprecated {
                    result.diagnostics.push(make_diagnostic(
                        range,
                        DEPRECATED_KEYWORD,
                        format!("Keyword '{name}' is deprecated: {notice}"),
                    ));
                }
                // Private keyword from another file.
                if kw.is_private {
                    if let (Some(kw_src), Some(self_src)) = (&kw.source, &self.source_path) {
                        if kw_src != self_src {
                            result.diagnostics.push(make_diagnostic(
                                range,
                                PRIVATE_KEYWORD,
                                format!("Keyword '{name}' is private"),
                            ));
                        }
                    }
                }
            }
            KeywordMatch::Ambiguous(matches) => {
                if self.check_keywords {
                    let sources: Vec<String> = matches
                        .iter()
                        .map(|kw| {
                            kw.library_name
                                .clone()
                                .or_else(|| kw.source.as_ref().map(|p| p.display().to_string()))
                                .unwrap_or_else(|| "<unknown>".to_owned())
                        })
                        .collect();
                    result.diagnostics.push(make_diagnostic(
                        range,
                        MULTIPLE_KEYWORDS,
                        format!(
                            "Multiple keywords with name '{name}' found. Match in: {}",
                            sources.join(", ")
                        ),
                    ));
                }
            }
            KeywordMatch::NotFound => {
                // Only emit if keyword checks are enabled (i.e. the namespace has
                // resolved imports) and the name doesn't look like a variable
                // substitution (e.g. `${kw_name}` — dynamic dispatch).
                if self.check_keywords && !contains_variable(trimmed) {
                    result.diagnostics.push(make_diagnostic(
                        range,
                        KEYWORD_NOT_FOUND,
                        format!("No keyword with name '{name}' found"),
                    ));
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // Variable reference checking
    // -----------------------------------------------------------------------

    /// Check all variable references within a value string.
    fn check_variable_refs(
        &self,
        value: &str,
        range: Range,
        result: &mut AnalysisResult,
        scope: &VariableScopeTracker,
    ) {
        let mut remaining = value;
        while let Some(m) = search_variable(remaining) {
            // Ignore environment variables (%{…}) — we can't statically verify
            // them.
            if m.identifier != '%' {
                let normalized = super::entities::normalize_variable_name(&m.base);
                if !scope.is_defined(&normalized) && !Namespace::is_builtin_variable(&normalized) {
                    result.diagnostics.push(make_diagnostic(
                        range,
                        VARIABLE_NOT_FOUND,
                        format!("Variable '{}{{{}}}' not found", m.identifier, m.base),
                    ));
                }
            }
            // Advance past the matched variable.
            if m.end >= remaining.len() {
                break;
            }
            remaining = &remaining[m.end..];
        }
    }
}

// ---------------------------------------------------------------------------
// AST position helpers
// ---------------------------------------------------------------------------

fn pos_to_range(pos: &robotcode_rf_parser::parser::ast::Position) -> Range {
    Range {
        start: Position {
            line: pos.line.saturating_sub(1),
            character: pos.column,
        },
        end: Position {
            line: pos.end_line.saturating_sub(1),
            character: pos.end_column,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostics::entities::{ArgKind, ArgSpec, KeywordDoc, LibraryEntry};
    use crate::diagnostics::namespace::Namespace;

    fn make_builtin_kw(name: &str) -> KeywordDoc {
        KeywordDoc {
            name: name.to_owned(),
            normalized_name: normalize_keyword_name(name),
            args: vec![ArgSpec {
                name: "message".to_owned(),
                kind: ArgKind::PositionalOrKeyword,
                default: None,
                types: vec![],
            }],
            doc: String::new(),
            deprecated: None,
            source: None,
            line_no: None,
            is_embedded: false,
            embedded_regex: None,
            library_name: Some("BuiltIn".to_owned()),
            is_private: false,
        }
    }

    fn parse_file(content: &str) -> robotcode_rf_parser::parser::ast::File {
        use robotcode_rf_parser::parser::parse;
        parse(content)
    }

    fn make_namespace_with_builtin() -> Namespace {
        let mut ns = Namespace::new(None);
        ns.libraries.push(LibraryEntry {
            name: "BuiltIn".to_owned(),
            alias: None,
            keywords: vec![
                make_builtin_kw("Log"),
                make_builtin_kw("Fail"),
                make_builtin_kw("Should Be Equal"),
                make_builtin_kw("Set Suite Variable"),
                make_builtin_kw("Set Global Variable"),
                make_builtin_kw("Set Test Variable"),
                make_builtin_kw("Set Variable"),
            ],
        });
        ns
    }

    #[test]
    fn test_no_diagnostics_for_known_keyword() {
        let content = "\
*** Test Cases ***
My Test
    Log    hello world
";
        let file = parse_file(content);
        let ns = make_namespace_with_builtin();
        let analyzer = NamespaceAnalyzer::new(&ns);
        let result = analyzer.analyze(&file);
        assert!(
            result.diagnostics.is_empty(),
            "Unexpected diagnostics: {:?}",
            result.diagnostics
        );
    }

    #[test]
    fn test_keyword_not_found_diagnostic() {
        let content = "\
*** Test Cases ***
My Test
    Nonexistent Keyword
";
        let file = parse_file(content);
        let ns = make_namespace_with_builtin();
        let analyzer = NamespaceAnalyzer::new(&ns);
        let result = analyzer.analyze(&file);
        assert!(
            result.diagnostics.iter().any(|d| d.code
                == Some(lsp_types::NumberOrString::String(
                    KEYWORD_NOT_FOUND.to_owned()
                ))),
            "Expected KeywordNotFound diagnostic, got: {:?}",
            result.diagnostics
        );
    }

    #[test]
    fn test_dynamic_keyword_no_diagnostic() {
        // Keywords whose name is a variable reference should not produce diagnostics.
        let content = "\
*** Test Cases ***
My Test
    ${kw_name}
    Run Keyword    ${kw_name}
";
        let file = parse_file(content);
        let mut ns = make_namespace_with_builtin();
        ns.libraries.push(LibraryEntry {
            name: "BuiltIn".to_owned(),
            alias: None,
            keywords: vec![make_builtin_kw("Run Keyword")],
        });
        let analyzer = NamespaceAnalyzer::new(&ns);
        let result = analyzer.analyze(&file);
        // The `${kw_name}` call itself may produce a keyword-not-found for
        // `${kw_name}` as a keyword name, which is fine — but there should be
        // no diagnostic for `Run Keyword`.
        let kw_not_found: Vec<_> = result
            .diagnostics
            .iter()
            .filter(|d| {
                d.code
                    == Some(lsp_types::NumberOrString::String(
                        KEYWORD_NOT_FOUND.to_owned(),
                    ))
                    && d.message.contains("Run Keyword")
            })
            .collect();
        assert!(kw_not_found.is_empty(), "Unexpected Run Keyword diagnostic");
    }

    #[test]
    fn test_duplicate_keyword_diagnostic() {
        let content = "\
*** Keywords ***
My Keyword
    Log    hello

My Keyword
    Fail    duplicate
";
        let file = parse_file(content);
        let ns = make_namespace_with_builtin();
        let analyzer = NamespaceAnalyzer::new(&ns);
        let result = analyzer.analyze(&file);
        assert!(
            result.diagnostics.iter().any(|d| d.code
                == Some(lsp_types::NumberOrString::String(
                    "KeywordAlreadyDefined".to_owned()
                ))),
            "Expected KeywordAlreadyDefined diagnostic, got: {:?}",
            result.diagnostics
        );
    }
}
