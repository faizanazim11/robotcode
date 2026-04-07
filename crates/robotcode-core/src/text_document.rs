//! Text document with incremental edit support.
//!
//! Port of the Python `robotcode.core.text_document` module.
//! Stores document content using a [`ropey::Rope`] for efficient edits.

use std::sync::{Arc, RwLock};

use ropey::Rope;

use crate::uri::Uri;

pub use lsp_types::{Position, Range, TextDocumentContentChangeEvent};

/// Error for invalid document range operations.
#[derive(Debug, thiserror::Error)]
pub enum TextDocumentError {
    #[error("Invalid range: start {0:?} is after end {1:?}")]
    InvalidRange(Position, Position),
    #[error("URI error: {0}")]
    Uri(#[from] crate::uri::UriError),
}

/// Converts an LSP UTF-16 position to a UTF-8 character offset in the rope.
///
/// LSP positions use UTF-16 code unit offsets for the character field.
/// The rope stores text as UTF-8 so we need to convert.
///
/// If `utf16_col` points into the middle of a surrogate pair (e.g. into an
/// emoji that is two UTF-16 code units), the offset is clamped to the start
/// of that code point — matching the Python `position_from_utf16` behaviour.
fn utf16_to_utf8_char_offset(rope: &Rope, line: u32, utf16_col: u32) -> usize {
    let line_idx = (line as usize).min(rope.len_lines().saturating_sub(1));
    let line_slice = rope.line(line_idx);

    let mut utf16_offset = 0u32;
    let mut char_offset = 0usize;

    for ch in line_slice.chars() {
        let utf16_len = ch.len_utf16() as u32;
        // If adding this character's UTF-16 units would exceed the target column,
        // stop here (clamp to the start of this code point if inside it).
        if utf16_offset + utf16_len > utf16_col {
            break;
        }
        utf16_offset += utf16_len;
        char_offset += 1;
    }

    rope.line_to_char(line_idx) + char_offset
}

/// A text document stored as a [`Rope`] with LSP-compatible incremental edit support.
#[derive(Debug)]
pub struct TextDocument {
    /// The normalized document URI.
    pub uri: Uri,
    /// The language identifier (e.g. `"robotframework"`).
    pub language_id: Option<String>,
    /// The current version number.
    version: RwLock<Option<i32>>,
    /// The document content.
    rope: RwLock<Rope>,
    /// Whether the document is currently open in an editor.
    pub opened_in_editor: bool,
}

impl TextDocument {
    /// Create a new text document.
    pub fn new(
        document_uri: impl AsRef<str>,
        text: impl AsRef<str>,
        language_id: Option<String>,
        version: Option<i32>,
    ) -> Result<Self, TextDocumentError> {
        let uri = Uri::parse(document_uri.as_ref())?.normalized();
        Ok(Self {
            uri,
            language_id,
            version: RwLock::new(version),
            rope: RwLock::new(Rope::from_str(text.as_ref())),
            opened_in_editor: false,
        })
    }

    /// Return the current version number.
    pub fn version(&self) -> Option<i32> {
        *self.version.read().unwrap()
    }

    /// Set the version number.
    pub fn set_version(&self, v: Option<i32>) {
        *self.version.write().unwrap() = v;
    }

    /// Return the full document text as a `String`.
    pub fn text(&self) -> String {
        self.rope.read().unwrap().to_string()
    }

    /// Return the number of lines.
    pub fn line_count(&self) -> usize {
        self.rope.read().unwrap().len_lines()
    }

    /// Apply a full-document content replacement.
    pub fn apply_full_change(&self, version: Option<i32>, new_text: &str) {
        if let Some(v) = version {
            *self.version.write().unwrap() = Some(v);
        }
        *self.rope.write().unwrap() = Rope::from_str(new_text);
    }

    /// Apply a single incremental [`TextDocumentContentChangeEvent`].
    ///
    /// If the event has no range it is treated as a full replacement.
    /// Returns an error if the range is invalid (start after end).
    pub fn apply_change(
        &self,
        version: Option<i32>,
        change: &TextDocumentContentChangeEvent,
    ) -> Result<(), TextDocumentError> {
        if let Some(v) = version {
            *self.version.write().unwrap() = Some(v);
        }

        match change.range {
            None => {
                // Full replacement
                *self.rope.write().unwrap() = Rope::from_str(&change.text);
            }
            Some(range) => {
                if range.start > range.end {
                    return Err(TextDocumentError::InvalidRange(range.start, range.end));
                }
                let mut rope = self.rope.write().unwrap();
                let start =
                    utf16_to_utf8_char_offset(&rope, range.start.line, range.start.character);
                let end = utf16_to_utf8_char_offset(&rope, range.end.line, range.end.character);
                rope.remove(start..end);
                rope.insert(start, &change.text);
            }
        }
        Ok(())
    }

    /// Apply a list of incremental changes in order.
    ///
    /// Propagates the first error encountered; the document version is only
    /// updated if all changes are applied successfully.
    pub fn apply_changes(
        &self,
        version: Option<i32>,
        changes: &[TextDocumentContentChangeEvent],
    ) -> Result<(), TextDocumentError> {
        for change in changes {
            self.apply_change(None, change)?;
        }
        if let Some(v) = version {
            *self.version.write().unwrap() = Some(v);
        }
        Ok(())
    }

    /// Convert an LSP UTF-16 [`Position`] to a UTF-8 character offset.
    pub fn offset_at(&self, position: Position) -> usize {
        let rope = self.rope.read().unwrap();
        utf16_to_utf8_char_offset(&rope, position.line, position.character)
    }

    /// Convert a UTF-8 character offset to an LSP UTF-16 [`Position`].
    pub fn position_at(&self, offset: usize) -> Position {
        let rope = self.rope.read().unwrap();
        let line = rope.char_to_line(offset.min(rope.len_chars()));
        let line_start = rope.line_to_char(line);
        let char_offset = offset - line_start;

        // Convert UTF-8 char offset to UTF-16 code unit offset
        let line_slice = rope.line(line);
        let utf16_col: u32 = line_slice
            .chars()
            .take(char_offset)
            .map(|c| c.len_utf16() as u32)
            .sum();

        Position {
            line: line as u32,
            character: utf16_col,
        }
    }

    /// Get the text in the given LSP range (using UTF-16 positions).
    pub fn get_text(&self, range: Range) -> Result<String, TextDocumentError> {
        if range.start > range.end {
            return Err(TextDocumentError::InvalidRange(range.start, range.end));
        }
        let rope = self.rope.read().unwrap();
        let start = utf16_to_utf8_char_offset(&rope, range.start.line, range.start.character);
        let end = utf16_to_utf8_char_offset(&rope, range.end.line, range.end.character);
        let end = end.min(rope.len_chars());
        let start = start.min(end);
        Ok(rope.slice(start..end).to_string())
    }
}

/// A thread-safe shared handle to a [`TextDocument`].
pub type SharedTextDocument = Arc<TextDocument>;

#[cfg(test)]
mod tests {
    use super::*;

    fn make_doc(text: &str) -> TextDocument {
        TextDocument::new(
            "file:///test.robot",
            text,
            Some("robotframework".to_string()),
            Some(1),
        )
        .unwrap()
    }

    #[test]
    fn test_full_change() {
        let doc = make_doc("hello world");
        doc.apply_full_change(Some(2), "goodbye world");
        assert_eq!(doc.text(), "goodbye world");
        assert_eq!(doc.version(), Some(2));
    }

    #[test]
    fn test_incremental_change_replaces_range() {
        let doc = make_doc("hello world\n");
        let change = TextDocumentContentChangeEvent {
            range: Some(Range {
                start: Position {
                    line: 0,
                    character: 0,
                },
                end: Position {
                    line: 0,
                    character: 5,
                },
            }),
            range_length: None,
            text: "goodbye".to_string(),
        };
        doc.apply_change(Some(2), &change).unwrap();
        assert_eq!(doc.text(), "goodbye world\n");
    }

    #[test]
    fn test_apply_change_invalid_range_returns_error() {
        let doc = make_doc("hello world\n");
        let change = TextDocumentContentChangeEvent {
            range: Some(Range {
                start: Position {
                    line: 0,
                    character: 5,
                },
                end: Position {
                    line: 0,
                    character: 0,
                },
            }),
            range_length: None,
            text: "oops".to_string(),
        };
        assert!(matches!(
            doc.apply_change(None, &change),
            Err(TextDocumentError::InvalidRange(_, _))
        ));
        // Document must be unchanged after the error.
        assert_eq!(doc.text(), "hello world\n");
    }

    #[test]
    fn test_position_at() {
        let doc = make_doc("hello\nworld\n");
        let pos = doc.position_at(6);
        assert_eq!(pos.line, 1);
        assert_eq!(pos.character, 0);
    }

    #[test]
    fn test_offset_at() {
        let doc = make_doc("hello\nworld\n");
        let offset = doc.offset_at(Position {
            line: 1,
            character: 0,
        });
        assert_eq!(offset, 6);
    }
}
