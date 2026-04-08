//! DAP 1.51 message type model.
//!
//! All types are `serde`-compatible for JSON serialization over the
//! Content-Length–framed wire protocol.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

// ── Base protocol ─────────────────────────────────────────────────────────────

/// Top-level DAP message discriminant.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum DapMessage {
    /// A client-to-server request.
    Request(ProtocolRequest),
    /// A server-to-client response.
    Response(ProtocolResponse),
    /// A server-to-client event.
    Event(ProtocolEvent),
}

/// A DAP request sent by the client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolRequest {
    /// Sequence number (monotonically increasing per sender).
    pub seq: i64,
    /// Command name (e.g. `"initialize"`, `"launch"`).
    pub command: String,
    /// Command-specific arguments.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<serde_json::Value>,
}

/// A DAP response from the server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolResponse {
    /// Sequence number of this response.
    pub seq: i64,
    /// Sequence number of the request this is a response to.
    pub request_seq: i64,
    /// Whether the request succeeded.
    pub success: bool,
    /// Command that this is a response to.
    pub command: String,
    /// Error message when `success` is false.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// Response body (command-specific).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<serde_json::Value>,
}

/// A DAP event sent by the server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolEvent {
    /// Sequence number.
    pub seq: i64,
    /// Event name (e.g. `"initialized"`, `"stopped"`).
    pub event: String,
    /// Event body (event-specific).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<serde_json::Value>,
}

// ── Capabilities ──────────────────────────────────────────────────────────────

/// Capabilities advertised by the debug adapter to the client.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Capabilities {
    /// Adapter supports conditional breakpoints.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supports_conditional_breakpoints: Option<bool>,
    /// Adapter supports the `configurationDone` request.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supports_configuration_done_request: Option<bool>,
    /// Adapter supports `setVariable`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supports_set_variable: Option<bool>,
    /// Adapter supports `evaluate` for hover expressions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supports_evaluate_for_hovers: Option<bool>,
    /// Exception breakpoint filters the adapter supports.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exception_breakpoint_filters: Option<Vec<ExceptionBreakpointsFilter>>,
    /// Adapter supports function breakpoints.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supports_function_breakpoints: Option<bool>,
    /// Adapter supports `terminate`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supports_terminate_request: Option<bool>,
}

/// A filter for exception breakpoints.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExceptionBreakpointsFilter {
    /// Filter identifier.
    pub filter: String,
    /// Human-readable label.
    pub label: String,
    /// Default enabled state.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<bool>,
}

// ── Source / Breakpoint ───────────────────────────────────────────────────────

/// A source file reference.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Source {
    /// Human-readable name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// File system path to the source file.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// Opaque source reference for adapter-managed sources.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_reference: Option<i64>,
}

/// A breakpoint descriptor.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Breakpoint {
    /// Unique breakpoint identifier.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,
    /// Whether the breakpoint was verified.
    pub verified: bool,
    /// Human-readable message when not verified.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// Source the breakpoint is set in.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<Source>,
    /// Actual line the breakpoint was set on.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<i64>,
}

/// A breakpoint location within a source.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BreakpointLocation {
    /// 1-based line number.
    pub line: i64,
    /// Optional 1-based column.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<i64>,
    /// Optional end line.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_line: Option<i64>,
    /// Optional end column.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_column: Option<i64>,
}

// ── Stack frame / Scope / Variable / Thread ───────────────────────────────────

/// A stack frame in a suspended thread.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StackFrame {
    /// Unique frame identifier.
    pub id: i64,
    /// Human-readable frame name (keyword/test name).
    pub name: String,
    /// Source file for this frame.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<Source>,
    /// 1-based current line.
    pub line: i64,
    /// 1-based current column.
    pub column: i64,
}

/// A variable scope within a stack frame.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Scope {
    /// Human-readable scope name (e.g. `"Local"`, `"Global"`).
    pub name: String,
    /// Reference used to retrieve variables in this scope.
    pub variables_reference: i64,
    /// Whether this scope is expensive to retrieve.
    pub expensive: bool,
}

/// A variable within a scope or another variable container.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Variable {
    /// Variable name.
    pub name: String,
    /// String representation of the value.
    pub value: String,
    /// Type string (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#type: Option<String>,
    /// Reference for nested variables (0 = no children).
    pub variables_reference: i64,
}

/// A running thread.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Thread {
    /// Unique thread identifier.
    pub id: i64,
    /// Human-readable thread name.
    pub name: String,
}

// ── Request argument types ────────────────────────────────────────────────────

/// Arguments for the `initialize` request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeArguments {
    /// Client identifier.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_id: Option<String>,
    /// Client display name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_name: Option<String>,
    /// Adapter identifier.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub adapter_id: Option<String>,
    /// Whether the client supports `runInTerminal`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supports_run_in_terminal_request: Option<bool>,
}

/// Arguments for the `launch` request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchArguments {
    /// Whether to start in a no-debug mode.
    #[serde(default)]
    pub no_debug: bool,
    /// Path to the robot file or directory.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    /// Additional command-line arguments for `robot`.
    #[serde(default)]
    pub args: Vec<String>,
    /// Working directory.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    /// Python interpreter path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub python: Option<String>,
    /// Extra environment variables.
    #[serde(default)]
    pub env: HashMap<String, String>,
}

/// Arguments for the `attach` request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachArguments {
    /// Host to attach to.
    #[serde(default = "default_host")]
    pub host: String,
    /// Port to attach to.
    pub port: u16,
}

fn default_host() -> String {
    "127.0.0.1".to_owned()
}

/// A source breakpoint specification (line + optional condition).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceBreakpoint {
    /// 1-based line number.
    pub line: i64,
    /// Optional column offset.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<i64>,
    /// Optional breakpoint condition (expression).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub condition: Option<String>,
    /// Optional hit-count condition.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hit_condition: Option<String>,
    /// Optional log message (tracepoint).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub log_message: Option<String>,
}

/// Arguments for the `setBreakpoints` request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetBreakpointsArguments {
    /// The source in which breakpoints are set.
    pub source: Source,
    /// The breakpoints to set.
    #[serde(default)]
    pub breakpoints: Vec<SourceBreakpoint>,
}

/// Arguments for the `setFunctionBreakpoints` request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetFunctionBreakpointsArguments {
    /// Function breakpoints to set.
    pub breakpoints: Vec<FunctionBreakpoint>,
}

/// A function breakpoint specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FunctionBreakpoint {
    /// Name of the function/keyword.
    pub name: String,
    /// Optional condition.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub condition: Option<String>,
}

/// Arguments for the `setExceptionBreakpoints` request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetExceptionBreakpointsArguments {
    /// Active filter IDs.
    pub filters: Vec<String>,
}

/// Arguments for the `stackTrace` request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StackTraceArguments {
    /// Thread to retrieve the stack from.
    pub thread_id: i64,
    /// Optional starting frame.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_frame: Option<i64>,
    /// Optional maximum number of frames.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub levels: Option<i64>,
}

/// Arguments for the `scopes` request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScopesArguments {
    /// Frame ID to retrieve scopes for.
    pub frame_id: i64,
}

/// Arguments for the `variables` request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VariablesArguments {
    /// Variables reference obtained from a [`Scope`] or parent [`Variable`].
    pub variables_reference: i64,
}

/// Arguments for the `continue` request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinueArguments {
    /// Thread to continue (or all threads if the adapter ignores it).
    pub thread_id: i64,
}

/// Arguments for the `next` (step over) request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NextArguments {
    /// Thread to step.
    pub thread_id: i64,
}

/// Arguments for the `stepIn` request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StepInArguments {
    /// Thread to step into.
    pub thread_id: i64,
}

/// Arguments for the `stepOut` request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StepOutArguments {
    /// Thread to step out of.
    pub thread_id: i64,
}

/// Arguments for the `pause` request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PauseArguments {
    /// Thread to pause.
    pub thread_id: i64,
}

/// Arguments for the `disconnect` request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DisconnectArguments {
    /// Whether the debug adapter should terminate the debuggee.
    #[serde(default)]
    pub terminate_debuggee: bool,
    /// Whether the debug adapter should restart the debuggee.
    #[serde(default)]
    pub restart: bool,
}

/// Arguments for the `evaluate` request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvaluateArguments {
    /// Expression to evaluate.
    pub expression: String,
    /// Stack frame context for the evaluation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frame_id: Option<i64>,
    /// Evaluation context (`"watch"`, `"repl"`, `"hover"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
}

/// Arguments for the `setVariable` request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetVariableArguments {
    /// Variables reference of the scope that contains the variable.
    pub variables_reference: i64,
    /// Variable name.
    pub name: String,
    /// New value.
    pub value: String,
}

/// Arguments for the `source` request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceArguments {
    /// Source to retrieve.
    pub source: Source,
    /// Source reference (alternative to `source.source_reference`).
    pub source_reference: i64,
}

// ── Response body types ───────────────────────────────────────────────────────

/// Body of the `initialize` response.
pub type InitializeResponseBody = Capabilities;

/// Body of an empty success response (launch, attach, configurationDone, etc.).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EmptyBody {}

/// Body of the `setBreakpoints` response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetBreakpointsResponseBody {
    /// Actual breakpoints set (parallel to the request's `breakpoints` array).
    pub breakpoints: Vec<Breakpoint>,
}

/// Body of the `stackTrace` response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StackTraceResponseBody {
    /// Stack frames for the requested thread.
    pub stack_frames: Vec<StackFrame>,
    /// Total number of frames available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_frames: Option<i64>,
}

/// Body of the `scopes` response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScopesResponseBody {
    /// Scopes for the requested frame.
    pub scopes: Vec<Scope>,
}

/// Body of the `variables` response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariablesResponseBody {
    /// Variables in the requested scope or container.
    pub variables: Vec<Variable>,
}

/// Body of the `continue` response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinueResponseBody {
    /// Whether all threads continued (true) or just the requested one.
    pub all_threads_continued: bool,
}

/// Body of the `threads` response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadsResponseBody {
    /// All currently running threads.
    pub threads: Vec<Thread>,
}

/// Body of the `evaluate` response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvaluateResponseBody {
    /// String representation of the result.
    pub result: String,
    /// Optional type of the result.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#type: Option<String>,
    /// Variables reference if the result has children.
    pub variables_reference: i64,
}

/// Body of the `source` response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceResponseBody {
    /// Source content.
    pub content: String,
    /// Optional MIME type.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

/// Body of the `setVariable` response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetVariableResponseBody {
    /// New value as a string.
    pub value: String,
    /// Optional type.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#type: Option<String>,
    /// Variables reference if the new value has children.
    pub variables_reference: i64,
}

// ── Event body types ──────────────────────────────────────────────────────────

/// Body of the `stopped` event.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoppedEventBody {
    /// Reason for stopping (`"step"`, `"breakpoint"`, `"exception"`, etc.).
    pub reason: String,
    /// Human-readable description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Thread that stopped.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<i64>,
    /// Whether all threads stopped.
    #[serde(default)]
    pub all_threads_stopped: bool,
}

/// Body of the `continued` event.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinuedEventBody {
    /// Thread that continued.
    pub thread_id: i64,
    /// Whether all threads continued.
    #[serde(default)]
    pub all_threads_continued: bool,
}

/// Body of the `exited` event.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExitedEventBody {
    /// Exit code of the debuggee process.
    pub exit_code: i32,
}

/// Body of the `terminated` event.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TerminatedEventBody {
    /// Whether the adapter is requesting a restart.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub restart: Option<serde_json::Value>,
}

/// Body of the `thread` event.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadEventBody {
    /// The thread that started or exited.
    pub thread_id: i64,
    /// `"started"` or `"exited"`.
    pub reason: String,
}

/// Body of the `output` event.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutputEventBody {
    /// Output text.
    pub output: String,
    /// Output category (`"console"`, `"stdout"`, `"stderr"`, `"telemetry"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    /// Source location of the output.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<Source>,
    /// Line of the output in the source.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<i64>,
}

/// Body of the `breakpoint` event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BreakpointEventBody {
    /// `"changed"`, `"new"`, or `"removed"`.
    pub reason: String,
    /// Updated breakpoint.
    pub breakpoint: Breakpoint,
}

/// Body of the `loadedSource` event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadedSourceEventBody {
    /// `"new"`, `"changed"`, or `"removed"`.
    pub reason: String,
    /// The source.
    pub source: Source,
}

/// Body of the `module` event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleEventBody {
    /// `"new"`, `"changed"`, or `"removed"`.
    pub reason: String,
    /// The module.
    pub module: Module,
}

/// A module (loaded library or resource file).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Module {
    /// Unique module identifier.
    pub id: serde_json::Value,
    /// Human-readable module name.
    pub name: String,
    /// Optional file system path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

/// Checksum of a source file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checksum {
    /// Checksum algorithm (`"MD5"`, `"SHA1"`, `"SHA256"`, `"timestamp"`).
    pub algorithm: String,
    /// Checksum value.
    pub checksum: String,
}

/// A column descriptor for multi-column output.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnDescriptor {
    /// Column attribute name.
    pub attribute_name: String,
    /// Column label.
    pub label: String,
    /// Column format.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    /// Column type.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#type: Option<String>,
    /// Column width hint.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<i64>,
}
