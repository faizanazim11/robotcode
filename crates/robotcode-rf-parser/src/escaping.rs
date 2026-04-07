//! Robot Framework escape-sequence utilities.
//!
//! Port of `robot.utils.escaping.unescape` and `split_from_equals`.

/// Unescape Robot Framework escape sequences in `text`.
///
/// RF uses backslash escaping:
/// - `\n` → newline, `\t` → tab, `\r` → carriage return
/// - `\\` → `\`, `\#` → `#`, `\$` → `$`, `\@` → `@`, `\&` → `&`
/// - `\%` → `%`, `\=` → `=`, `\|` → `|`
/// - `\a` → bell, `\b` → backspace, `\f` → formfeed, `\v` → vertical tab
/// - `\0` → null
/// - `\xHH`, `\uHHHH`, `\UHHHHHHHH` → Unicode code points (same as Python)
pub fn unescape(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        if chars[i] != '\\' || i + 1 >= len {
            result.push(chars[i]);
            i += 1;
            continue;
        }

        // We have a backslash with at least one following character.
        match chars[i + 1] {
            'n' => { result.push('\n'); i += 2; }
            't' => { result.push('\t'); i += 2; }
            'r' => { result.push('\r'); i += 2; }
            '\\' => { result.push('\\'); i += 2; }
            '#' => { result.push('#'); i += 2; }
            '$' => { result.push('$'); i += 2; }
            '@' => { result.push('@'); i += 2; }
            '&' => { result.push('&'); i += 2; }
            '%' => { result.push('%'); i += 2; }
            '=' => { result.push('='); i += 2; }
            '|' => { result.push('|'); i += 2; }
            'a' => { result.push('\x07'); i += 2; } // bell
            'b' => { result.push('\x08'); i += 2; } // backspace
            'f' => { result.push('\x0C'); i += 2; } // formfeed
            'v' => { result.push('\x0B'); i += 2; } // vertical tab
            '0' => { result.push('\0'); i += 2; }
            'x' => {
                // \xHH — exactly two hex digits.
                if i + 3 < len {
                    let h: String = chars[i + 2..=i + 3].iter().collect();
                    if let Ok(n) = u32::from_str_radix(&h, 16) {
                        if let Some(c) = char::from_u32(n) {
                            result.push(c);
                            i += 4;
                            continue;
                        }
                    }
                }
                result.push('\\');
                i += 1;
            }
            'u' => {
                // \uHHHH — exactly four hex digits.
                if i + 5 < len {
                    let h: String = chars[i + 2..=i + 5].iter().collect();
                    if let Ok(n) = u32::from_str_radix(&h, 16) {
                        if let Some(c) = char::from_u32(n) {
                            result.push(c);
                            i += 6;
                            continue;
                        }
                    }
                }
                result.push('\\');
                i += 1;
            }
            'U' => {
                // \UHHHHHHHH — exactly eight hex digits.
                if i + 9 < len {
                    let h: String = chars[i + 2..=i + 9].iter().collect();
                    if let Ok(n) = u32::from_str_radix(&h, 16) {
                        if let Some(c) = char::from_u32(n) {
                            result.push(c);
                            i += 10;
                            continue;
                        }
                    }
                }
                result.push('\\');
                i += 1;
            }
            _ => {
                // Unknown escape — keep the backslash.
                result.push('\\');
                i += 1;
            }
        }
    }

    result
}

/// Split `text` on the first unescaped `=` sign, returning `(key, value)`.
///
/// Returns `None` if there is no unescaped `=`.  Variable syntax inside the
/// key (e.g. `${a}=value`) is handled: the `=` must be *outside* any `{…}`.
pub fn split_from_equals(text: &str) -> Option<(String, String)> {
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    let mut i = 0;
    let mut depth = 0usize; // brace depth (inside ${…})

    while i < len {
        match chars[i] {
            '\\' => { i += 2; } // skip escaped character
            '{' => { depth += 1; i += 1; }
            '}' => { depth = depth.saturating_sub(1); i += 1; }
            '=' if depth == 0 => {
                let key = chars[..i].iter().collect();
                let value = chars[i + 1..].iter().collect();
                return Some((key, value));
            }
            _ => { i += 1; }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unescape_basic() {
        assert_eq!(unescape(r"\n"), "\n");
        assert_eq!(unescape(r"\t"), "\t");
        assert_eq!(unescape(r"\\"), "\\");
        assert_eq!(unescape(r"\$"), "$");
        assert_eq!(unescape(r"\#"), "#");
    }

    #[test]
    fn test_unescape_no_escape() {
        assert_eq!(unescape("hello world"), "hello world");
    }

    #[test]
    fn test_split_from_equals() {
        assert_eq!(split_from_equals("key=value"), Some(("key".into(), "value".into())));
        assert_eq!(split_from_equals("no equals"), None);
        assert_eq!(split_from_equals("a=b=c"), Some(("a".into(), "b=c".into())));
    }

    #[test]
    fn test_split_from_equals_inside_variable() {
        // The `=` inside `${…}` should NOT be the split point.
        assert_eq!(split_from_equals("${a=b}=value"), Some(("${a=b}".into(), "value".into())));
    }
}
