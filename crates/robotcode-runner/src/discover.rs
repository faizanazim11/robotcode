//! `robotcode discover` — Rust-native test discovery using the RF parser.
//!
//! Walks directories for `.robot` and `.resource` files, parses them with the
//! Rust RF parser, and extracts test/task/keyword names with line numbers.

use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

use robotcode_rf_parser::parser::{ast, parse_with_source};

// ── Public types ──────────────────────────────────────────────────────────────

/// A discovered test or task item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredTest {
    /// Test/task name.
    pub name: String,
    /// 1-based line number where the test/task starts.
    pub line: u32,
    /// Tags applied to the test/task.
    pub tags: Vec<String>,
}

/// A discovered keyword.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredKeyword {
    /// Keyword name.
    pub name: String,
    /// 1-based line number where the keyword starts.
    pub line: u32,
}

/// A discovered suite (one `.robot` or `.resource` file).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredSuite {
    /// Suite name derived from the file name.
    pub name: String,
    /// Absolute path to the source file.
    pub source: PathBuf,
    /// Tests/tasks found in the file.
    pub tests: Vec<DiscoveredTest>,
    /// Nested suites (subdirectories).
    pub suites: Vec<DiscoveredSuite>,
    /// Keywords defined in the file.
    pub keywords: Vec<DiscoveredKeyword>,
}

/// Top-level discovery report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryReport {
    /// All discovered suites.
    pub suites: Vec<DiscoveredSuite>,
}

/// Arguments for `robotcode discover`.
#[derive(Debug, Clone)]
pub struct DiscoverArgs {
    /// Paths to search (files or directories).
    pub paths: Vec<PathBuf>,
    /// Whether to recurse into subdirectories.
    pub recursive: bool,
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Discover Robot Framework tests in the given paths.
pub async fn discover(args: DiscoverArgs) -> Result<DiscoveryReport> {
    let mut suites = Vec::new();

    for path in &args.paths {
        if path.is_file() {
            if is_robot_file(path) {
                if let Some(suite) = parse_file(path) {
                    suites.push(suite);
                }
            }
        } else if path.is_dir() {
            let dir_suites = walk_directory(path, args.recursive)?;
            suites.extend(dir_suites);
        } else {
            warn!(path = %path.display(), "Path does not exist or is not accessible");
        }
    }

    Ok(DiscoveryReport { suites })
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn is_robot_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()),
        Some("robot" | "resource")
    )
}

/// Walk a directory and collect suites, optionally recursing.
fn walk_directory(dir: &Path, recursive: bool) -> Result<Vec<DiscoveredSuite>> {
    let mut suites = Vec::new();

    let entries = std::fs::read_dir(dir)?;
    let mut file_paths: Vec<PathBuf> = Vec::new();
    let mut sub_dirs: Vec<PathBuf> = Vec::new();

    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_file() && is_robot_file(&p) {
            file_paths.push(p);
        } else if recursive && p.is_dir() {
            sub_dirs.push(p);
        }
    }

    file_paths.sort();
    sub_dirs.sort();

    for path in &file_paths {
        if let Some(suite) = parse_file(path) {
            suites.push(suite);
        }
    }

    for sub_dir in &sub_dirs {
        let child_suites = walk_directory(sub_dir, recursive)?;
        if !child_suites.is_empty() {
            // Represent the directory as a container suite with no direct tests.
            let dir_name = suite_name_from_path(sub_dir);
            suites.push(DiscoveredSuite {
                name: dir_name,
                source: sub_dir.clone(),
                tests: Vec::new(),
                suites: child_suites,
                keywords: Vec::new(),
            });
        }
    }

    Ok(suites)
}

/// Parse a single `.robot` / `.resource` file and return a [`DiscoveredSuite`].
fn parse_file(path: &Path) -> Option<DiscoveredSuite> {
    let source = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            warn!(path = %path.display(), error = %e, "Failed to read file");
            return None;
        }
    };

    let source_name = path.to_string_lossy();
    debug!(path = %path.display(), "Parsing RF file");

    let file: ast::File = parse_with_source(&source, Some(&source_name));

    let mut tests = Vec::new();
    let mut keywords = Vec::new();

    for section in &file.sections {
        match section {
            ast::Section::TestCases(tc_section) => {
                for tc in &tc_section.body {
                    tests.push(DiscoveredTest {
                        name: tc.name.clone(),
                        line: tc.position.line,
                        tags: extract_tags_from_body(&tc.body),
                    });
                }
            }
            ast::Section::Tasks(task_section) => {
                for task in &task_section.body {
                    tests.push(DiscoveredTest {
                        name: task.name.clone(),
                        line: task.position.line,
                        tags: extract_tags_from_body(&task.body),
                    });
                }
            }
            ast::Section::Keywords(kw_section) => {
                for kw in &kw_section.body {
                    keywords.push(DiscoveredKeyword {
                        name: kw.name.clone(),
                        line: kw.position.line,
                    });
                }
            }
            _ => {}
        }
    }

    let name = suite_name_from_path(path);

    Some(DiscoveredSuite {
        name,
        source: path.to_path_buf(),
        tests,
        suites: Vec::new(),
        keywords,
    })
}

/// Extract inline `[Tags]` from a test/task/keyword body.
fn extract_tags_from_body(body: &[ast::BodyItem]) -> Vec<String> {
    for item in body {
        if let ast::BodyItem::Tags(tags) = item {
            return tags.tags.clone();
        }
    }
    Vec::new()
}

/// Derive a suite name from a file or directory path (mimics RF naming convention).
fn suite_name_from_path(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Unknown")
        .replace('_', " ")
        .split_whitespace()
        .map(capitalize_first)
        .collect::<Vec<_>>()
        .join(" ")
}

fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suite_name_from_path_simple() {
        let p = PathBuf::from("/some/path/my_test_suite.robot");
        assert_eq!(suite_name_from_path(&p), "My Test Suite");
    }

    #[test]
    fn suite_name_from_path_directory() {
        let p = PathBuf::from("/some/path/acceptance_tests");
        assert_eq!(suite_name_from_path(&p), "Acceptance Tests");
    }

    #[test]
    fn is_robot_file_extensions() {
        assert!(is_robot_file(Path::new("foo.robot")));
        assert!(is_robot_file(Path::new("foo.resource")));
        assert!(!is_robot_file(Path::new("foo.py")));
        assert!(!is_robot_file(Path::new("foo.txt")));
    }
}
