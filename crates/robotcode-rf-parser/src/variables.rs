//! Robot Framework variable search utilities.
//!
//! Port of `robot.variables.search` (`is_variable`, `search_variable`,
//! `contains_variable`, `is_scalar_assign`).

/// The result of a variable search within a string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VariableMatch {
    /// The variable identifier character (`$`, `@`, `&`, `%`).
    pub identifier: char,
    /// The base name (contents inside `{…}`), e.g. `"name"` for `${name}`.
    pub base: String,
    /// Any item/index specifiers after the closing `}`, e.g. `["key"]`.
    pub items: Vec<String>,
    /// Byte offset of the opening `$`/`@`/`&`/`%` in the source string.
    pub start: usize,
    /// Byte offset of the character immediately after the closing `}` (plus items).
    pub end: usize,
    /// Text before the variable.
    pub before: String,
    /// Text after the variable.
    pub after: String,
}

/// Return `true` if `text` is *entirely* a single variable, e.g. `"${name}"`.
///
/// Supports all RF variable types: `${}`, `@{}`, `&{}`, `%{}`.
pub fn is_variable(text: &str) -> bool {
    match search_variable(text) {
        Some(m) => m.start == 0 && m.end == text.len() && m.before.is_empty() && m.after.is_empty(),
        None => false,
    }
}

/// Return `true` if `text` contains at least one variable anywhere.
pub fn contains_variable(text: &str) -> bool {
    search_variable(text).is_some()
}

/// Find the first variable in `text`, returning a [`VariableMatch`] or `None`.
pub fn search_variable(text: &str) -> Option<VariableMatch> {
    let bytes = text.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i + 1 < len {
        let ch = bytes[i] as char;
        if matches!(ch, '$' | '@' | '&' | '%') {
            // Skip escaped identifier: `\$`, `\@`, etc.
            if i > 0 && bytes[i - 1] == b'\\' {
                i += 1;
                continue;
            }
            // Expression variable: `${{…}}` or `@{{…}}`
            if i + 2 < len && bytes[i + 1] == b'{' && bytes[i + 2] == b'{' {
                if let Some(end) = find_double_brace_end(bytes, i + 2) {
                    let base = &text[i + 3..end - 1];
                    let var_end = end + 1; // skip second `}`
                    return Some(VariableMatch {
                        identifier: ch,
                        base: base.to_string(),
                        items: vec![],
                        start: i,
                        end: var_end,
                        before: text[..i].to_string(),
                        after: text[var_end..].to_string(),
                    });
                }
            }
            // Normal variable: `${…}`
            if bytes[i + 1] == b'{' {
                if let Some(close) = find_brace_end(bytes, i + 1) {
                    let base = &text[i + 2..close];
                    let mut var_end = close + 1;
                    // Collect `[item]` subscripts.
                    let mut items = Vec::new();
                    while var_end < len && bytes[var_end] == b'[' {
                        if let Some(bracket_end) = find_bracket_end(bytes, var_end) {
                            items.push(text[var_end + 1..bracket_end].to_string());
                            var_end = bracket_end + 1;
                        } else {
                            break;
                        }
                    }
                    return Some(VariableMatch {
                        identifier: ch,
                        base: base.to_string(),
                        items,
                        start: i,
                        end: var_end,
                        before: text[..i].to_string(),
                        after: text[var_end..].to_string(),
                    });
                }
            }
        }
        i += 1;
    }
    None
}

/// Return `true` if `text` is a scalar/list/dict *assignment* target.
///
/// Examples: `"${var}="`, `"${var} ="`, `"@{list}="`, `"&{dict}="`.
/// Note: the `=` must actually be present; `"${var}"` alone returns `false`.
pub fn is_scalar_assign(text: &str) -> bool {
    // Must end with `=` (possibly with whitespace before it).
    if !text.trim_end().ends_with('=') {
        return false;
    }
    let s = text.trim_end_matches('=').trim_end();
    is_variable(s) && matches!(s.chars().next(), Some('$' | '@' | '&'))
}

// ── Internal helpers ──────────────────────────────────────────────────────────

/// Find the matching `}` for a `{` at `open_pos` in `bytes`.
fn find_brace_end(bytes: &[u8], open_pos: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut i = open_pos;
    while i < bytes.len() {
        match bytes[i] {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// Find the end of a `{{…}}` double-brace expression.
fn find_double_brace_end(bytes: &[u8], open_pos: usize) -> Option<usize> {
    // `open_pos` points to the first `{` of `{{`.
    let mut depth = 0i32;
    let mut i = open_pos;
    while i + 1 < bytes.len() {
        if bytes[i] == b'{' && bytes[i + 1] == b'{' {
            depth += 1;
            i += 2;
        } else if bytes[i] == b'}' && bytes[i + 1] == b'}' {
            depth -= 1;
            if depth == 0 {
                return Some(i + 1); // points to second `}`
            }
            i += 2;
        } else {
            i += 1;
        }
    }
    None
}

/// Find the matching `]` for a `[` at `open_pos`.
fn find_bracket_end(bytes: &[u8], open_pos: usize) -> Option<usize> {
    let mut i = open_pos + 1;
    while i < bytes.len() {
        if bytes[i] == b']' {
            return Some(i);
        }
        i += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_variable_scalar() {
        assert!(is_variable("${name}"));
        assert!(is_variable("${NAME}"));
        assert!(is_variable("@{list}"));
        assert!(is_variable("&{dict}"));
        assert!(is_variable("%{ENV}"));
    }

    #[test]
    fn test_is_variable_false() {
        assert!(!is_variable("${name} extra"));
        assert!(!is_variable("prefix ${name}"));
        assert!(!is_variable("plain text"));
        assert!(!is_variable(""));
    }

    #[test]
    fn test_contains_variable() {
        assert!(contains_variable("Hello ${name}!"));
        assert!(contains_variable("${x}"));
        assert!(!contains_variable("plain text"));
    }

    #[test]
    fn test_search_variable() {
        let m = search_variable("Hello ${world}!").unwrap();
        assert_eq!(m.identifier, '$');
        assert_eq!(m.base, "world");
        assert_eq!(m.before, "Hello ");
        assert_eq!(m.after, "!");
    }

    #[test]
    fn test_is_scalar_assign() {
        assert!(is_scalar_assign("${var}="));
        assert!(is_scalar_assign("${var} ="));
        assert!(is_scalar_assign("@{list}="));
        assert!(!is_scalar_assign("${var}"));
        assert!(!is_scalar_assign("plain="));
    }

    #[test]
    fn test_variable_with_items() {
        let m = search_variable("${dict}[key]").unwrap();
        assert_eq!(m.base, "dict");
        assert_eq!(m.items, vec!["key"]);
    }
}
