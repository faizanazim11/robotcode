//! Snapshot tests using `insta`.

use insta::assert_snapshot;
use robotcode_core::{
    text_document::TextDocument,
    uri::Uri,
    utils::dataclasses::{to_camel_case, to_snake_case},
};

// ── URI ──────────────────────────────────────────────────────────────────────

#[test]
fn snapshot_uri_display() {
    let uri = Uri::parse("file:///home/user/project/test.robot").unwrap();
    assert_snapshot!(uri.to_string());
}

#[test]
fn snapshot_uri_components() {
    let uri = Uri::parse("file:///home/user/project/test.robot?q=1#frag").unwrap();
    let out = format!(
        "scheme={}\npath={}\nquery={:?}\nfragment={:?}",
        uri.scheme(),
        uri.path(),
        uri.query(),
        uri.fragment()
    );
    assert_snapshot!(out);
}

// ── TextDocument ─────────────────────────────────────────────────────────────

#[test]
fn snapshot_text_document_initial() {
    let doc = TextDocument::new(
        "file:///test.robot",
        "*** Test Cases ***\nMy Test\n    Log    Hello\n",
        Some("robotframework".to_string()),
        Some(1),
    )
    .unwrap();
    assert_snapshot!(doc.text());
}

#[test]
fn snapshot_text_document_after_incremental_edit() {
    use lsp_types::{Position, Range, TextDocumentContentChangeEvent};

    let doc = TextDocument::new("file:///test.robot", "hello world\n", None, Some(1)).unwrap();
    let change = TextDocumentContentChangeEvent {
        range: Some(Range {
            start: Position {
                line: 0,
                character: 6,
            },
            end: Position {
                line: 0,
                character: 11,
            },
        }),
        range_length: None,
        text: "Rust".to_string(),
    };
    doc.apply_change(Some(2), &change).unwrap();
    assert_snapshot!(doc.text());
}

// ── dataclasses ───────────────────────────────────────────────────────────────

#[test]
fn snapshot_camel_case_conversions() {
    let cases = vec![
        ("hello_world", to_camel_case("hello_world")),
        ("my_long_field_name", to_camel_case("my_long_field_name")),
        ("", to_camel_case("")),
        ("single", to_camel_case("single")),
    ];
    let out: Vec<String> = cases
        .iter()
        .map(|(k, v)| format!("{} -> {}", k, v))
        .collect();
    assert_snapshot!(out.join("\n"));
}

#[test]
fn snapshot_snake_case_conversions() {
    let cases = vec![
        ("helloWorld", to_snake_case("helloWorld")),
        ("myLongFieldName", to_snake_case("myLongFieldName")),
        ("", to_snake_case("")),
        ("single", to_snake_case("single")),
    ];
    let out: Vec<String> = cases
        .iter()
        .map(|(k, v)| format!("{} -> {}", k, v))
        .collect();
    assert_snapshot!(out.join("\n"));
}
