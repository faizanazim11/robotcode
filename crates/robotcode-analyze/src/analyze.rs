//! Batch static analysis using the RF parser to collect diagnostics.

use std::path::{Path, PathBuf};

use anyhow::Result;
use lsp_types::{Diagnostic, DiagnosticSeverity};
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

use robotcode_rf_parser::parser::{ast, parse_with_source};

// ── Output format ─────────────────────────────────────────────────────────────

/// Output format for `robotcode analyze`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    /// Human-readable text output.
    Text,
    /// Machine-readable JSON output.
    Json,
}

// ── Arguments ─────────────────────────────────────────────────────────────────

/// Arguments for `robotcode analyze`.
#[derive(Debug, Clone)]
pub struct AnalyzeArgs {
    /// Paths to analyze (files or directories).
    pub paths: Vec<PathBuf>,
    /// Optional Python interpreter (reserved for future Python-bridge diagnostics).
    pub python: Option<PathBuf>,
    /// Output format.
    pub output_format: OutputFormat,
    /// Exit with code 1 when any errors are found.
    pub fail_on_error: bool,
    /// Exit with code 2 when any warnings are found (and no errors).
    pub fail_on_warning: bool,
}

// ── Report types ──────────────────────────────────────────────────────────────

/// Per-file diagnostics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileDiagnostics {
    /// Absolute path to the analyzed file.
    pub path: PathBuf,
    /// Diagnostics found in this file.
    pub diagnostics: Vec<Diagnostic>,
}

/// Top-level analysis report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisReport {
    /// Number of files analyzed.
    pub files_analyzed: usize,
    /// Number of error-severity diagnostics.
    pub errors: usize,
    /// Number of warning-severity diagnostics.
    pub warnings: usize,
    /// All per-file diagnostics.
    pub diagnostics: Vec<FileDiagnostics>,
    /// Suggested exit code: 0 = ok, 1 = errors, 2 = warnings-only.
    pub exit_code: i32,
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Run batch static analysis on the given paths.
pub async fn analyze(args: AnalyzeArgs) -> Result<AnalysisReport> {
    let mut file_diagnostics: Vec<FileDiagnostics> = Vec::new();

    let files = collect_files(&args.paths);

    for path in &files {
        debug!(path = %path.display(), "Analyzing file");
        let diags = analyze_file(path);
        file_diagnostics.push(FileDiagnostics {
            path: path.clone(),
            diagnostics: diags,
        });
    }

    let errors: usize = file_diagnostics
        .iter()
        .flat_map(|fd| &fd.diagnostics)
        .filter(|d| d.severity == Some(DiagnosticSeverity::ERROR))
        .count();

    let warnings: usize = file_diagnostics
        .iter()
        .flat_map(|fd| &fd.diagnostics)
        .filter(|d| d.severity == Some(DiagnosticSeverity::WARNING))
        .count();

    let exit_code = if args.fail_on_error && errors > 0 {
        1
    } else if args.fail_on_warning && warnings > 0 {
        2
    } else {
        0
    };

    Ok(AnalysisReport {
        files_analyzed: files.len(),
        errors,
        warnings,
        diagnostics: file_diagnostics,
        exit_code,
    })
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn collect_files(paths: &[PathBuf]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for p in paths {
        if p.is_file() && is_robot_file(p) {
            out.push(p.clone());
        } else if p.is_dir() {
            collect_dir(p, &mut out);
        } else {
            warn!(path = %p.display(), "Path does not exist or is not a RF file");
        }
    }
    out
}

fn collect_dir(dir: &Path, out: &mut Vec<PathBuf>) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        let mut sorted: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
        sorted.sort();
        for p in sorted {
            if p.is_file() && is_robot_file(&p) {
                out.push(p);
            } else if p.is_dir() {
                collect_dir(&p, out);
            }
        }
    }
}

fn is_robot_file(p: &Path) -> bool {
    matches!(
        p.extension().and_then(|e| e.to_str()),
        Some("robot" | "resource")
    )
}

/// Analyze a single file and return LSP-style diagnostics.
///
/// Currently extracts parse-level errors from the AST. In future phases this
/// will delegate to the full `robotcode-robot` namespace analyzer.
fn analyze_file(path: &PathBuf) -> Vec<Diagnostic> {
    let source = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            warn!(path = %path.display(), error = %e, "Failed to read file");
            return Vec::new();
        }
    };

    let source_name = path.to_str().unwrap_or("");
    let file: ast::File = parse_with_source(&source, Some(source_name));

    collect_ast_diagnostics(&file)
}

/// Walk the AST and collect [`Diagnostic`]s from error nodes.
fn collect_ast_diagnostics(file: &ast::File) -> Vec<Diagnostic> {
    let mut diags = Vec::new();

    for section in &file.sections {
        if let ast::Section::Invalid(invalid) = section {
            for err in &invalid.body {
                diags.push(make_error_diagnostic(&err.message, err.position.line));
            }
        }
    }

    diags
}

fn make_error_diagnostic(message: &str, line: u32) -> Diagnostic {
    use lsp_types::{Position, Range};
    Diagnostic {
        range: Range {
            start: Position {
                line: line.saturating_sub(1),
                character: 0,
            },
            end: Position {
                line: line.saturating_sub(1),
                character: u32::MAX,
            },
        },
        severity: Some(DiagnosticSeverity::ERROR),
        message: message.to_owned(),
        ..Default::default()
    }
}
