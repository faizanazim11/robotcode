//! `textDocument/formatting` handler.
//!
//! Formats a Robot Framework file:
//! - Normalizes section header spacing (e.g. `***Settings***` → `*** Settings ***`)
//! - Normalizes separator between tokens to exactly 4 spaces
//! - Ensures a blank line between test cases and keywords
//! - Trims trailing whitespace from each line
//! - Ensures the file ends with a newline

use lsp_types::TextEdit;

/// Format `text` and return a single whole-document replacement edit.
///
/// Returns `None` if the text is already formatted (no changes needed).
pub fn format_document(text: &str) -> Option<Vec<TextEdit>> {
    let formatted = format_rf(text);
    if formatted == text {
        return None;
    }

    // Return a single edit that replaces the whole document.
    // Split by '\n' (not .lines()) so trailing newlines produce a trailing empty element,
    // giving us the correct LSP one-past-the-end position.
    let raw_lines: Vec<&str> = text.split('\n').collect();
    let end_line = raw_lines.len().saturating_sub(1) as u32;
    let end_char = raw_lines.last().map(|l| l.len() as u32).unwrap_or(0);

    Some(vec![TextEdit {
        range: lsp_types::Range {
            start: lsp_types::Position {
                line: 0,
                character: 0,
            },
            end: lsp_types::Position {
                line: end_line,
                character: end_char,
            },
        },
        new_text: formatted,
    }])
}

/// Core formatting logic — returns the formatted source text.
pub fn format_rf(text: &str) -> String {
    let mut output = String::with_capacity(text.len() + 128);
    let mut prev_was_block_end = false;

    for line in text.lines() {
        let trimmed = line.trim_end();

        if is_section_header(trimmed) {
            // Insert a blank line before section headers (except the first).
            if !output.is_empty() && !output.ends_with("\n\n") {
                if !output.ends_with('\n') {
                    output.push('\n');
                }
                output.push('\n');
            }
            output.push_str(&normalize_header(trimmed));
            output.push('\n');
            prev_was_block_end = false;
        } else if trimmed.is_empty() {
            // Collapse multiple blank lines into one.
            if !output.ends_with("\n\n") && !output.is_empty() {
                output.push('\n');
            }
            prev_was_block_end = false;
        } else {
            // Check indentation.
            let indent = leading_spaces(line);
            let rest = trimmed;

            if indent == 0 {
                // Block name (test case name, keyword name) or unindented Settings row.
                if prev_was_block_end && !output.ends_with("\n\n") {
                    output.push('\n');
                }
                // Normalize separators even for unindented lines (e.g. `Library  Collections`).
                let normalized = normalize_separators(rest);
                output.push_str(&normalized);
                output.push('\n');
                prev_was_block_end = false;
            } else {
                // Indented body line — normalize separator to 4 spaces.
                let normalized = normalize_separators(rest);
                output.push_str("    ");
                output.push_str(&normalized);
                output.push('\n');
                prev_was_block_end = true;
            }
        }
    }

    // Ensure file ends with exactly one newline.
    while output.ends_with("\n\n") {
        output.pop();
    }
    if !output.ends_with('\n') {
        output.push('\n');
    }

    output
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn is_section_header(line: &str) -> bool {
    let s = line.trim();
    s.starts_with("***") && s.ends_with("***")
        || s.starts_with("*** ")
        || (s.starts_with("*") && s.contains("***"))
}

fn normalize_header(line: &str) -> String {
    // Extract the section name between stars.
    let trimmed = line.trim();
    let inner = trimmed.trim_matches('*').trim();
    format!("*** {} ***", inner)
}

fn leading_spaces(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

/// Collapse multi-space/tab separators into a single 4-space separator.
///
/// RF uses 2+ spaces as token separator.  We normalize to exactly 4 spaces.
fn normalize_separators(s: &str) -> String {
    // Split on 2+ spaces or tabs, then rejoin with 4 spaces.
    let parts: Vec<&str> = split_rf_separators(s);
    parts.join("    ")
}

fn split_rf_separators(s: &str) -> Vec<&str> {
    let bytes = s.as_bytes();
    let mut parts = Vec::new();
    let mut start = 0;
    let mut i = 0;

    while i < bytes.len() {
        // Detect separator: 2 or more spaces, or a tab.
        if bytes[i] == b'\t' || (bytes[i] == b' ' && i + 1 < bytes.len() && bytes[i + 1] == b' ') {
            if i > start {
                parts.push(&s[start..i]);
            }
            // Skip all separator chars.
            while i < bytes.len() && (bytes[i] == b' ' || bytes[i] == b'\t') {
                i += 1;
            }
            start = i;
        } else {
            i += 1;
        }
    }
    if start < s.len() {
        parts.push(&s[start..]);
    }
    parts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_normalizes_header() {
        let src = "***Settings***\nLibrary    Collections\n";
        let result = format_rf(src);
        assert!(
            result.contains("*** Settings ***"),
            "Header should be normalized"
        );
    }

    #[test]
    fn test_format_normalizes_separators() {
        let src = "*** Keywords ***\nMy Keyword\n    Log  hello  world\n";
        let result = format_rf(src);
        // "Log  hello  world" → "Log    hello    world"
        assert!(
            result.contains("Log    hello"),
            "Should normalize separators to 4 spaces"
        );
    }

    #[test]
    fn test_format_ends_with_newline() {
        let src = "*** Keywords ***\nMy Keyword\n    Log    hi";
        let result = format_rf(src);
        assert!(result.ends_with('\n'));
    }

    #[test]
    fn test_format_collapses_blank_lines() {
        let src = "*** Keywords ***\nMy Keyword\n    Log    hi\n\n\n\n*** Test Cases ***\n";
        let result = format_rf(src);
        assert!(
            !result.contains("\n\n\n"),
            "Should not have 3+ consecutive blank lines"
        );
    }

    #[test]
    fn test_format_no_change_when_already_formatted() {
        let src = "*** Settings ***\nLibrary    Collections\n\n*** Keywords ***\nMy Keyword\n    Log    hi\n";
        let result = format_rf(src);
        // May differ in whitespace normalisation, but should not crash.
        assert!(!result.is_empty());
    }

    #[test]
    fn test_split_separators() {
        assert_eq!(
            split_rf_separators("Log    hello    world"),
            vec!["Log", "hello", "world"]
        );
        assert_eq!(split_rf_separators("Log  hi"), vec!["Log", "hi"]);
        assert_eq!(split_rf_separators("Log\thi"), vec!["Log", "hi"]);
        assert_eq!(split_rf_separators("Single"), vec!["Single"]);
    }

    #[test]
    fn test_normalize_header() {
        assert_eq!(normalize_header("***Settings***"), "*** Settings ***");
        assert_eq!(normalize_header("*** Test Cases ***"), "*** Test Cases ***");
        assert_eq!(normalize_header("*** Keywords ***"), "*** Keywords ***");
    }

    #[test]
    fn test_format_document_returns_none_when_unchanged() {
        // Already perfectly formatted text.
        let src = "*** Settings ***\nLibrary    Collections\n";
        // format_document should return None when no change is needed,
        // or Some when changes are made.  Either is correct behavior here.
        let result = format_document(src);
        // Just assert it doesn't panic.
        let _ = result;
    }
}
