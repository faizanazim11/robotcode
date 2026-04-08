//! Tests for the core debugger state machine.

use robotcode_debugger::dap_types::{DapMessage, ProtocolRequest};
use robotcode_debugger::debugger::RfDebugger;
use robotcode_debugger::protocol::DapHandler;

fn make_request(seq: i64, command: &str, arguments: Option<serde_json::Value>) -> DapMessage {
    DapMessage::Request(ProtocolRequest {
        seq,
        command: command.to_owned(),
        arguments,
    })
}

#[test]
fn initialize_returns_capabilities() {
    let mut dbg = RfDebugger::new();
    let msg = make_request(1, "initialize", None);
    let result = dbg.handle(&msg).unwrap();
    let body = result.unwrap();

    // Should report supportsConditionalBreakpoints = true.
    assert_eq!(body["supportsConditionalBreakpoints"], true);
    assert_eq!(body["supportsConfigurationDoneRequest"], true);
    assert_eq!(body["supportsSetVariable"], true);
    assert_eq!(body["supportsEvaluateForHovers"], true);
}

#[test]
fn initialize_response_has_exception_filters() {
    let mut dbg = RfDebugger::new();
    let msg = make_request(1, "initialize", None);
    let body = dbg.handle(&msg).unwrap().unwrap();

    let filters = body["exceptionBreakpointFilters"].as_array().unwrap();
    assert!(!filters.is_empty());
    assert_eq!(filters[0]["filter"], "raised");
}

#[test]
fn set_breakpoints_stores_and_returns_them() {
    let mut dbg = RfDebugger::new();

    // First initialize.
    let init = make_request(1, "initialize", None);
    dbg.handle(&init).unwrap();

    let args = serde_json::json!({
        "source": {"path": "/tests/suite.robot", "name": "Suite"},
        "breakpoints": [{"line": 10}, {"line": 20}, {"line": 30}]
    });
    let msg = make_request(2, "setBreakpoints", Some(args));
    let body = dbg.handle(&msg).unwrap().unwrap();

    let bps = body["breakpoints"].as_array().unwrap();
    assert_eq!(bps.len(), 3);
    assert_eq!(bps[0]["verified"], true);
    assert_eq!(bps[0]["line"], 10);
    assert_eq!(bps[1]["line"], 20);
    assert_eq!(bps[2]["line"], 30);
}

#[test]
fn threads_returns_empty_before_launch() {
    let mut dbg = RfDebugger::new();
    let msg = make_request(1, "threads", None);
    let body = dbg.handle(&msg).unwrap().unwrap();
    let threads = body["threads"].as_array().unwrap();
    assert!(threads.is_empty());
}

#[test]
fn configuration_done_succeeds() {
    let mut dbg = RfDebugger::new();
    let init = make_request(1, "initialize", None);
    dbg.handle(&init).unwrap();

    let msg = make_request(2, "configurationDone", None);
    let result = dbg.handle(&msg);
    assert!(result.is_ok());
}

#[test]
fn evaluate_returns_expression_result() {
    let mut dbg = RfDebugger::new();

    let args = serde_json::json!({"expression": "${MY_VAR}"});
    let msg = make_request(1, "evaluate", Some(args));
    let body = dbg.handle(&msg).unwrap().unwrap();

    let result = body["result"].as_str().unwrap();
    assert!(result.contains("${MY_VAR}"));
}

#[test]
fn unknown_command_returns_error() {
    let mut dbg = RfDebugger::new();
    let msg = make_request(1, "unknownCommand", None);
    assert!(dbg.handle(&msg).is_err());
}

#[test]
fn continue_response_all_threads_continued() {
    let mut dbg = RfDebugger::new();
    let msg = make_request(1, "continue", Some(serde_json::json!({"threadId": 1})));
    let body = dbg.handle(&msg).unwrap().unwrap();
    assert_eq!(body["allThreadsContinued"], true);
}

#[test]
fn stack_trace_empty_when_not_stopped() {
    let mut dbg = RfDebugger::new();
    let msg = make_request(1, "stackTrace", Some(serde_json::json!({"threadId": 1})));
    let body = dbg.handle(&msg).unwrap().unwrap();
    let frames = body["stackFrames"].as_array().unwrap();
    assert!(frames.is_empty());
}

#[test]
fn scopes_returns_local_and_global() {
    let mut dbg = RfDebugger::new();
    let msg = make_request(1, "scopes", Some(serde_json::json!({"frameId": 0})));
    let body = dbg.handle(&msg).unwrap().unwrap();
    let scopes = body["scopes"].as_array().unwrap();
    assert_eq!(scopes.len(), 2);
    assert_eq!(scopes[0]["name"], "Local");
    assert_eq!(scopes[1]["name"], "Global");
}
