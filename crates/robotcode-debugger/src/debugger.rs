//! Core debugger state machine implementing the [`DapHandler`] trait.

use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::{anyhow, Result};
use tracing::{debug, info, warn};

use crate::dap_types::{
    Breakpoint, Capabilities, ContinueResponseBody, DapMessage, EmptyBody, EvaluateArguments,
    EvaluateResponseBody, ExceptionBreakpointsFilter, LaunchArguments, PauseArguments,
    ScopesArguments, ScopesResponseBody, SetBreakpointsArguments, SetBreakpointsResponseBody,
    SetVariableArguments, SetVariableResponseBody, Source, StackFrame, StackTraceArguments,
    StackTraceResponseBody, Thread, ThreadsResponseBody, VariablesArguments, VariablesResponseBody,
};
use crate::launcher::LaunchConfig;
use crate::protocol::DapHandler;

// ── Debugger state ────────────────────────────────────────────────────────────

/// Running state of the RF subprocess.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DebuggerState {
    /// Not yet initialized.
    NotInitialized,
    /// Initialized, waiting for `launch` or `attach`.
    Initialized,
    /// RF process running.
    Running,
    /// RF process paused at a breakpoint or step.
    Stopped,
    /// RF process terminated.
    Terminated,
}

/// Core Robot Framework debugger state machine.
///
/// Implements [`DapHandler`] to translate DAP protocol messages into
/// actions on the RF subprocess.
pub struct RfDebugger {
    /// Current state.
    state: DebuggerState,
    /// Line breakpoints per source path.
    breakpoints: HashMap<String, Vec<Breakpoint>>,
    /// Active threads (one for RF's single-threaded execution model).
    threads: Vec<Thread>,
    /// RF subprocess handle (set after `launch`).
    process: Option<tokio::process::Child>,
}

impl RfDebugger {
    /// Create a new, uninitialised debugger.
    pub fn new() -> Self {
        Self {
            state: DebuggerState::NotInitialized,
            breakpoints: HashMap::new(),
            threads: Vec::new(),
            process: None,
        }
    }
}

impl Default for RfDebugger {
    fn default() -> Self {
        Self::new()
    }
}

// ── DapHandler implementation ─────────────────────────────────────────────────

impl DapHandler for RfDebugger {
    fn handle(&mut self, message: &DapMessage) -> Result<Option<serde_json::Value>> {
        let DapMessage::Request(req) = message else {
            return Ok(None);
        };

        debug!(command = %req.command, "Handling DAP request");

        match req.command.as_str() {
            "initialize" => self.handle_initialize(),
            "launch" => self.handle_launch(req.arguments.as_ref()),
            "attach" => self.handle_attach(),
            "configurationDone" => self.handle_configuration_done(),
            "setBreakpoints" => self.handle_set_breakpoints(req.arguments.as_ref()),
            "setFunctionBreakpoints" => self.handle_set_function_breakpoints(),
            "setExceptionBreakpoints" => self.handle_set_exception_breakpoints(),
            "threads" => self.handle_threads(),
            "stackTrace" => self.handle_stack_trace(req.arguments.as_ref()),
            "scopes" => self.handle_scopes(req.arguments.as_ref()),
            "variables" => self.handle_variables(req.arguments.as_ref()),
            "continue" => self.handle_continue(),
            "next" => self.handle_next(),
            "stepIn" => self.handle_step_in(),
            "stepOut" => self.handle_step_out(),
            "pause" => self.handle_pause(req.arguments.as_ref()),
            "disconnect" => self.handle_disconnect(req.arguments.as_ref()),
            "evaluate" => self.handle_evaluate(req.arguments.as_ref()),
            "setVariable" => self.handle_set_variable(req.arguments.as_ref()),
            "source" => self.handle_source(),
            unknown => {
                warn!(command = %unknown, "Unknown DAP command");
                Err(anyhow!("Unknown command: {unknown}"))
            }
        }
    }
}

// ── Individual request handlers ───────────────────────────────────────────────

impl RfDebugger {
    /// `initialize` — advertise adapter capabilities.
    fn handle_initialize(&mut self) -> Result<Option<serde_json::Value>> {
        info!("DAP initialize");
        self.state = DebuggerState::Initialized;

        let caps = Capabilities {
            supports_conditional_breakpoints: Some(true),
            supports_configuration_done_request: Some(true),
            supports_set_variable: Some(true),
            supports_evaluate_for_hovers: Some(true),
            supports_function_breakpoints: Some(false),
            supports_terminate_request: Some(true),
            exception_breakpoint_filters: Some(vec![ExceptionBreakpointsFilter {
                filter: "raised".to_owned(),
                label: "Raised Exceptions".to_owned(),
                default: Some(false),
            }]),
        };

        Ok(Some(serde_json::to_value(caps)?))
    }

    /// `launch` — start `python -m robot` with debug listener.
    fn handle_launch(
        &mut self,
        arguments: Option<&serde_json::Value>,
    ) -> Result<Option<serde_json::Value>> {
        let launch_args: LaunchArguments = if let Some(args) = arguments {
            serde_json::from_value(args.clone())?
        } else {
            return Err(anyhow!("launch request missing arguments"));
        };

        info!(target = ?launch_args.target, "DAP launch");

        let program = launch_args
            .target
            .as_deref()
            .map(PathBuf::from)
            .ok_or_else(|| anyhow!("launch: 'target' field is required"))?;

        let cfg = LaunchConfig {
            program,
            args: launch_args.args,
            cwd: launch_args.cwd.map(PathBuf::from),
            python: launch_args.python.map(PathBuf::from),
            env: launch_args.env,
        };

        let mut cmd = tokio::process::Command::new(cfg.python_executable());
        cmd.args(["-m", "robot"])
            .args(&cfg.args)
            .arg(cfg.program.as_os_str())
            .current_dir(cfg.resolved_cwd())
            .envs(&cfg.env);

        // Spawn the process (non-blocking — actual exec happens when the event
        // loop processes this future).  The process is stored for later lifecycle
        // management.
        let child = cmd.spawn()?;
        self.process = Some(child);
        self.state = DebuggerState::Running;

        // Seed a single main thread to represent RF's execution model.
        self.threads = vec![Thread {
            id: 1,
            name: "Robot Framework".to_owned(),
        }];

        Ok(Some(serde_json::to_value(EmptyBody {})?))
    }

    /// `attach` — attach to a running RF debug session.
    fn handle_attach(&mut self) -> Result<Option<serde_json::Value>> {
        info!("DAP attach");
        self.state = DebuggerState::Running;
        self.threads = vec![Thread {
            id: 1,
            name: "Robot Framework".to_owned(),
        }];
        Ok(Some(serde_json::to_value(EmptyBody {})?))
    }

    /// `configurationDone` — signal RF to begin execution.
    fn handle_configuration_done(&mut self) -> Result<Option<serde_json::Value>> {
        debug!("DAP configurationDone");
        Ok(Some(serde_json::to_value(EmptyBody {})?))
    }

    /// `setBreakpoints` — store and validate breakpoints for a source file.
    fn handle_set_breakpoints(
        &mut self,
        arguments: Option<&serde_json::Value>,
    ) -> Result<Option<serde_json::Value>> {
        let args: SetBreakpointsArguments = if let Some(a) = arguments {
            serde_json::from_value(a.clone())?
        } else {
            return Err(anyhow!("setBreakpoints: missing arguments"));
        };

        let path = args
            .source
            .path
            .clone()
            .unwrap_or_else(|| "<unknown>".to_owned());

        debug!(path = %path, count = args.breakpoints.len(), "Setting breakpoints");

        let mut verified: Vec<Breakpoint> = Vec::new();
        let mut stored: Vec<Breakpoint> = Vec::new();

        for (i, sb) in args.breakpoints.iter().enumerate() {
            let bp = Breakpoint {
                id: Some(i as i64 + 1),
                verified: true,
                message: None,
                source: Some(Source {
                    path: Some(path.clone()),
                    name: args.source.name.clone(),
                    source_reference: None,
                }),
                line: Some(sb.line),
            };
            verified.push(bp.clone());
            stored.push(bp);
        }

        self.breakpoints.insert(path, stored);

        let body = SetBreakpointsResponseBody {
            breakpoints: verified,
        };
        Ok(Some(serde_json::to_value(body)?))
    }

    /// `setFunctionBreakpoints` — not supported (returns empty list).
    fn handle_set_function_breakpoints(&mut self) -> Result<Option<serde_json::Value>> {
        let body = SetBreakpointsResponseBody {
            breakpoints: vec![],
        };
        Ok(Some(serde_json::to_value(body)?))
    }

    /// `setExceptionBreakpoints` — accept but do not act (stub).
    fn handle_set_exception_breakpoints(&mut self) -> Result<Option<serde_json::Value>> {
        Ok(Some(serde_json::to_value(EmptyBody {})?))
    }

    /// `threads` — return the list of active threads.
    fn handle_threads(&mut self) -> Result<Option<serde_json::Value>> {
        let body = ThreadsResponseBody {
            threads: self.threads.clone(),
        };
        Ok(Some(serde_json::to_value(body)?))
    }

    /// `stackTrace` — return stack frames for a thread.
    ///
    /// Returns an empty list when the adapter is not in the `Stopped` state.
    fn handle_stack_trace(
        &mut self,
        arguments: Option<&serde_json::Value>,
    ) -> Result<Option<serde_json::Value>> {
        let args: StackTraceArguments = if let Some(a) = arguments {
            serde_json::from_value(a.clone())?
        } else {
            return Err(anyhow!("stackTrace: missing arguments"));
        };

        debug!(thread_id = args.thread_id, "stackTrace request");

        // Stub implementation: no frames when running/not stopped.
        let frames: Vec<StackFrame> = if self.state == DebuggerState::Stopped {
            vec![StackFrame {
                id: 0,
                name: "Robot Framework Execution".to_owned(),
                source: None,
                line: 0,
                column: 0,
            }]
        } else {
            vec![]
        };

        let body = StackTraceResponseBody {
            total_frames: Some(frames.len() as i64),
            stack_frames: frames,
        };
        Ok(Some(serde_json::to_value(body)?))
    }

    /// `scopes` — return scopes for a stack frame.
    fn handle_scopes(
        &mut self,
        arguments: Option<&serde_json::Value>,
    ) -> Result<Option<serde_json::Value>> {
        let args: ScopesArguments = if let Some(a) = arguments {
            serde_json::from_value(a.clone())?
        } else {
            return Err(anyhow!("scopes: missing arguments"));
        };

        debug!(frame_id = args.frame_id, "scopes request");

        // Stub: return two scopes (Local variables = ref 1000, Global = ref 1001).
        let body = ScopesResponseBody {
            scopes: vec![
                crate::dap_types::Scope {
                    name: "Local".to_owned(),
                    variables_reference: 1000,
                    expensive: false,
                },
                crate::dap_types::Scope {
                    name: "Global".to_owned(),
                    variables_reference: 1001,
                    expensive: false,
                },
            ],
        };
        Ok(Some(serde_json::to_value(body)?))
    }

    /// `variables` — return variables for a scope/container reference.
    fn handle_variables(
        &mut self,
        arguments: Option<&serde_json::Value>,
    ) -> Result<Option<serde_json::Value>> {
        let args: VariablesArguments = if let Some(a) = arguments {
            serde_json::from_value(a.clone())?
        } else {
            return Err(anyhow!("variables: missing arguments"));
        };

        debug!(
            variables_reference = args.variables_reference,
            "variables request"
        );

        // Stub: no variables.
        let body = VariablesResponseBody { variables: vec![] };
        Ok(Some(serde_json::to_value(body)?))
    }

    /// `continue` — resume execution.
    fn handle_continue(&mut self) -> Result<Option<serde_json::Value>> {
        debug!("DAP continue");
        if self.state == DebuggerState::Stopped {
            self.state = DebuggerState::Running;
        }
        let body = ContinueResponseBody {
            all_threads_continued: true,
        };
        Ok(Some(serde_json::to_value(body)?))
    }

    /// `next` — step over.
    fn handle_next(&mut self) -> Result<Option<serde_json::Value>> {
        debug!("DAP next (step over)");
        Ok(Some(serde_json::to_value(EmptyBody {})?))
    }

    /// `stepIn` — step into.
    fn handle_step_in(&mut self) -> Result<Option<serde_json::Value>> {
        debug!("DAP stepIn");
        Ok(Some(serde_json::to_value(EmptyBody {})?))
    }

    /// `stepOut` — step out.
    fn handle_step_out(&mut self) -> Result<Option<serde_json::Value>> {
        debug!("DAP stepOut");
        Ok(Some(serde_json::to_value(EmptyBody {})?))
    }

    /// `pause` — pause execution.
    fn handle_pause(
        &mut self,
        arguments: Option<&serde_json::Value>,
    ) -> Result<Option<serde_json::Value>> {
        let args: PauseArguments = if let Some(a) = arguments {
            serde_json::from_value(a.clone())?
        } else {
            return Err(anyhow!("pause: missing arguments"));
        };

        debug!(thread_id = args.thread_id, "DAP pause");
        self.state = DebuggerState::Stopped;
        Ok(Some(serde_json::to_value(EmptyBody {})?))
    }

    /// `disconnect` — terminate the RF subprocess.
    fn handle_disconnect(
        &mut self,
        arguments: Option<&serde_json::Value>,
    ) -> Result<Option<serde_json::Value>> {
        debug!("DAP disconnect");

        let terminate = arguments
            .and_then(|a| a.get("terminateDebugee").and_then(|v| v.as_bool()))
            .unwrap_or(true);

        if terminate {
            if let Some(child) = &mut self.process {
                let _ = child.start_kill();
            }
        }

        self.process = None;
        self.state = DebuggerState::Terminated;

        Ok(Some(serde_json::to_value(EmptyBody {})?))
    }

    /// `evaluate` — evaluate an expression in the RF context (stub).
    fn handle_evaluate(
        &mut self,
        arguments: Option<&serde_json::Value>,
    ) -> Result<Option<serde_json::Value>> {
        let args: EvaluateArguments = if let Some(a) = arguments {
            serde_json::from_value(a.clone())?
        } else {
            return Err(anyhow!("evaluate: missing arguments"));
        };

        debug!(expression = %args.expression, "DAP evaluate");

        let body = EvaluateResponseBody {
            result: format!("<evaluate: {}>", args.expression),
            r#type: None,
            variables_reference: 0,
        };
        Ok(Some(serde_json::to_value(body)?))
    }

    /// `setVariable` — change a variable value (stub).
    fn handle_set_variable(
        &mut self,
        arguments: Option<&serde_json::Value>,
    ) -> Result<Option<serde_json::Value>> {
        let args: SetVariableArguments = if let Some(a) = arguments {
            serde_json::from_value(a.clone())?
        } else {
            return Err(anyhow!("setVariable: missing arguments"));
        };

        debug!(name = %args.name, value = %args.value, "DAP setVariable");

        let body = SetVariableResponseBody {
            value: args.value,
            r#type: None,
            variables_reference: 0,
        };
        Ok(Some(serde_json::to_value(body)?))
    }

    /// `source` — retrieve source content for a source reference (stub).
    fn handle_source(&mut self) -> Result<Option<serde_json::Value>> {
        debug!("DAP source request");
        let body = crate::dap_types::SourceResponseBody {
            content: String::new(),
            mime_type: Some("text/x-robotframework".to_owned()),
        };
        Ok(Some(serde_json::to_value(body)?))
    }
}
