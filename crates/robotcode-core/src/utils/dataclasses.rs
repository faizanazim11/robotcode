//! Common serde helpers and camelCase / snake_case conversion utilities.
//!
//! Port of the Python `robotcode.core.utils.dataclasses` module.

use serde::{Deserialize, Serialize};

/// Convert a `snake_case` string to `camelCase`.
///
/// ```
/// use robotcode_core::utils::dataclasses::to_camel_case;
/// assert_eq!(to_camel_case("hello_world"), "helloWorld");
/// assert_eq!(to_camel_case("my_long_field_name"), "myLongFieldName");
/// ```
pub fn to_camel_case(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut capitalize_next = false;

    for (i, ch) in s.char_indices() {
        if ch == '_' || ch == '-' || ch == '.' {
            capitalize_next = true;
        } else if capitalize_next {
            result.extend(ch.to_uppercase());
            capitalize_next = false;
        } else if i == 0 {
            result.extend(ch.to_lowercase());
        } else {
            result.push(ch);
        }
    }

    result
}

/// Convert a `camelCase` or `PascalCase` string to `snake_case`.
///
/// ```
/// use robotcode_core::utils::dataclasses::to_snake_case;
/// assert_eq!(to_snake_case("helloWorld"), "hello_world");
/// assert_eq!(to_snake_case("myLongFieldName"), "my_long_field_name");
/// ```
pub fn to_snake_case(s: &str) -> String {
    let mut result = String::with_capacity(s.len() + 4);

    for (i, ch) in s.char_indices() {
        if ch.is_uppercase() {
            if i > 0 {
                result.push('_');
            }
            result.extend(ch.to_lowercase());
        } else if ch == '-' || ch == '.' {
            result.push('_');
        } else {
            result.push(ch);
        }
    }

    result
}

/// A serde rename helper: serializes struct fields using `camelCase` names.
///
/// Use `#[serde(rename_all = "camelCase")]` on structs directly where possible;
/// this module provides the functions for programmatic use.
///
/// Serialize a value to a pretty-printed JSON string.
pub fn to_json_pretty<T: Serialize>(value: &T) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(value)
}

/// Deserialize a value from a JSON string.
pub fn from_json<'de, T: Deserialize<'de>>(s: &'de str) -> Result<T, serde_json::Error> {
    serde_json::from_str(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_camel_case() {
        assert_eq!(to_camel_case("hello_world"), "helloWorld");
        assert_eq!(to_camel_case("my_long_field_name"), "myLongFieldName");
        assert_eq!(to_camel_case("already"), "already");
        assert_eq!(to_camel_case(""), "");
    }

    #[test]
    fn test_to_snake_case() {
        assert_eq!(to_snake_case("helloWorld"), "hello_world");
        assert_eq!(to_snake_case("myLongFieldName"), "my_long_field_name");
        assert_eq!(to_snake_case("already"), "already");
        assert_eq!(to_snake_case(""), "");
    }

    #[test]
    fn test_roundtrip() {
        let original = "my_field_name";
        assert_eq!(to_snake_case(&to_camel_case(original)), original);
    }
}
