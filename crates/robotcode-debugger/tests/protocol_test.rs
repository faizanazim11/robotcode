//! Tests for Content-Length framing read/write.

use robotcode_debugger::dap_types::{DapMessage, ProtocolEvent, ProtocolRequest};
use robotcode_debugger::protocol::DapProtocol;
use tokio::io::BufReader;

fn make_framed(json: &str) -> Vec<u8> {
    format!("Content-Length: {}\r\n\r\n{}", json.len(), json).into_bytes()
}

#[tokio::test]
async fn read_request_message() {
    let json = r#"{"type":"request","seq":5,"command":"threads"}"#;
    let raw = make_framed(json);
    let cursor = std::io::Cursor::new(raw);
    let mut reader = BufReader::new(cursor);

    let msg = DapProtocol::read_message(&mut reader).await.unwrap();
    let DapMessage::Request(req) = msg else {
        panic!("Expected Request");
    };
    assert_eq!(req.command, "threads");
    assert_eq!(req.seq, 5);
}

#[tokio::test]
async fn write_and_read_roundtrip() {
    let original = DapMessage::Event(ProtocolEvent {
        seq: 99,
        event: "terminated".to_owned(),
        body: None,
    });

    let mut buf: Vec<u8> = Vec::new();
    DapProtocol::write_message(&mut buf, &original)
        .await
        .unwrap();

    let cursor = std::io::Cursor::new(buf);
    let mut reader = BufReader::new(cursor);
    let parsed = DapProtocol::read_message(&mut reader).await.unwrap();

    let DapMessage::Event(ev) = parsed else {
        panic!("Expected Event");
    };
    assert_eq!(ev.event, "terminated");
    assert_eq!(ev.seq, 99);
}

#[tokio::test]
async fn multiple_messages_in_sequence() {
    let msgs = vec![
        r#"{"type":"request","seq":1,"command":"initialize"}"#,
        r#"{"type":"request","seq":2,"command":"launch"}"#,
        r#"{"type":"request","seq":3,"command":"disconnect"}"#,
    ];

    let mut raw: Vec<u8> = Vec::new();
    for json in &msgs {
        let framed = make_framed(json);
        raw.extend_from_slice(&framed);
    }

    let cursor = std::io::Cursor::new(raw);
    let mut reader = BufReader::new(cursor);

    for expected_cmd in ["initialize", "launch", "disconnect"] {
        let msg = DapProtocol::read_message(&mut reader).await.unwrap();
        let DapMessage::Request(req) = msg else {
            panic!("Expected Request for {expected_cmd}");
        };
        assert_eq!(req.command, expected_cmd);
    }
}

#[tokio::test]
async fn error_response_is_not_success() {
    let resp = DapProtocol::error_response(2, 1, "launch", "missing target");
    let DapMessage::Response(r) = resp else {
        panic!("Expected Response");
    };
    assert!(!r.success);
    assert_eq!(r.command, "launch");
    assert_eq!(r.message.as_deref(), Some("missing target"));
}

#[tokio::test]
async fn success_response_has_no_error_message() {
    let resp = DapProtocol::success_response(2, 1, "initialize", None);
    let DapMessage::Response(r) = resp else {
        panic!("Expected Response");
    };
    assert!(r.success);
    assert!(r.message.is_none());
    assert!(r.body.is_none());
}

#[tokio::test]
async fn framed_message_with_unicode() {
    // Verify Content-Length reflects byte count, not char count.
    let original = DapMessage::Event(ProtocolEvent {
        seq: 1,
        event: "output".to_owned(),
        body: Some(serde_json::json!({"output": "日本語テスト\n"})),
    });

    let mut buf: Vec<u8> = Vec::new();
    DapProtocol::write_message(&mut buf, &original)
        .await
        .unwrap();

    let cursor = std::io::Cursor::new(buf);
    let mut reader = BufReader::new(cursor);
    let parsed = DapProtocol::read_message(&mut reader).await.unwrap();

    let DapMessage::Event(ev) = parsed else {
        panic!("Expected Event");
    };
    assert_eq!(ev.event, "output");
}

#[tokio::test]
async fn request_with_arguments_preserved() {
    let req = ProtocolRequest {
        seq: 10,
        command: "setBreakpoints".to_owned(),
        arguments: Some(serde_json::json!({
            "source": {"path": "/foo/bar.robot"},
            "breakpoints": [{"line": 42}]
        })),
    };
    let original = DapMessage::Request(req);

    let mut buf: Vec<u8> = Vec::new();
    DapProtocol::write_message(&mut buf, &original)
        .await
        .unwrap();

    let cursor = std::io::Cursor::new(buf);
    let mut reader = BufReader::new(cursor);
    let parsed = DapProtocol::read_message(&mut reader).await.unwrap();

    let DapMessage::Request(r) = parsed else {
        panic!("Expected Request");
    };
    assert_eq!(r.command, "setBreakpoints");
    let args = r.arguments.unwrap();
    assert_eq!(args["breakpoints"][0]["line"], 42);
}
