//! Tests for DAP type serialization and deserialization.

use robotcode_debugger::dap_types::*;

#[test]
fn capabilities_serializes_optional_fields() {
    let caps = Capabilities {
        supports_conditional_breakpoints: Some(true),
        supports_configuration_done_request: Some(true),
        supports_set_variable: Some(false),
        supports_evaluate_for_hovers: Some(true),
        exception_breakpoint_filters: Some(vec![ExceptionBreakpointsFilter {
            filter: "raised".to_owned(),
            label: "Raised Exceptions".to_owned(),
            default: Some(false),
        }]),
        supports_function_breakpoints: None,
        supports_terminate_request: None,
    };

    let json = serde_json::to_value(&caps).unwrap();
    assert_eq!(json["supportsConditionalBreakpoints"], true);
    assert_eq!(json["supportsConfigurationDoneRequest"], true);
    assert_eq!(json["supportsSetVariable"], false);
    assert!(json.get("supportsFunctionBreakpoints").is_none());
}

#[test]
fn dap_message_request_roundtrip() {
    let json =
        r#"{"type":"request","seq":1,"command":"initialize","arguments":{"clientId":"vscode"}}"#;
    let msg: DapMessage = serde_json::from_str(json).unwrap();
    let DapMessage::Request(req) = msg else {
        panic!("Expected Request");
    };
    assert_eq!(req.command, "initialize");
    assert_eq!(req.seq, 1);
    assert!(req.arguments.is_some());
}

#[test]
fn dap_message_response_roundtrip() {
    let json =
        r#"{"type":"response","seq":2,"request_seq":1,"success":true,"command":"initialize"}"#;
    let msg: DapMessage = serde_json::from_str(json).unwrap();
    let DapMessage::Response(resp) = msg else {
        panic!("Expected Response");
    };
    assert_eq!(resp.command, "initialize");
    assert!(resp.success);
}

#[test]
fn dap_message_event_roundtrip() {
    let json = r#"{"type":"event","seq":3,"event":"initialized"}"#;
    let msg: DapMessage = serde_json::from_str(json).unwrap();
    let DapMessage::Event(ev) = msg else {
        panic!("Expected Event");
    };
    assert_eq!(ev.event, "initialized");
}

#[test]
fn stopped_event_body_serializes() {
    let body = StoppedEventBody {
        reason: "breakpoint".to_owned(),
        description: Some("Hit breakpoint".to_owned()),
        thread_id: Some(1),
        all_threads_stopped: true,
    };
    let json = serde_json::to_value(&body).unwrap();
    assert_eq!(json["reason"], "breakpoint");
    assert_eq!(json["allThreadsStopped"], true);
}

#[test]
fn source_breakpoint_optional_fields() {
    let sb = SourceBreakpoint {
        line: 42,
        column: None,
        condition: Some("${x} == 1".to_owned()),
        hit_condition: None,
        log_message: None,
    };
    let json = serde_json::to_value(&sb).unwrap();
    assert_eq!(json["line"], 42);
    assert_eq!(json["condition"], "${x} == 1");
    assert!(json.get("column").is_none());
}

#[test]
fn launch_arguments_defaults() {
    let json = r#"{"target": "suite.robot"}"#;
    let args: LaunchArguments = serde_json::from_str(json).unwrap();
    assert_eq!(args.target.as_deref(), Some("suite.robot"));
    assert!(!args.no_debug);
    assert!(args.args.is_empty());
    assert!(args.env.is_empty());
}

#[test]
fn thread_serializes() {
    let t = Thread {
        id: 1,
        name: "Robot Framework".to_owned(),
    };
    let json = serde_json::to_value(&t).unwrap();
    assert_eq!(json["id"], 1);
    assert_eq!(json["name"], "Robot Framework");
}
