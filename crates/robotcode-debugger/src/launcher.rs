//! Launch and attach configuration for the DAP server.

use std::collections::HashMap;
use std::path::PathBuf;

/// Configuration for launching a Robot Framework debug session.
#[derive(Debug, Clone)]
pub struct LaunchConfig {
    /// Path to the robot file or directory to execute.
    pub program: PathBuf,
    /// Additional arguments forwarded to `python -m robot`.
    pub args: Vec<String>,
    /// Working directory (defaults to the parent of `program`).
    pub cwd: Option<PathBuf>,
    /// Path to the Python interpreter (defaults to `python3` on PATH).
    pub python: Option<PathBuf>,
    /// Extra environment variables for the RF subprocess.
    pub env: HashMap<String, String>,
}

impl LaunchConfig {
    /// Resolve the Python interpreter path as a string.
    pub fn python_executable(&self) -> String {
        self.python
            .as_ref()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|| "python3".to_owned())
    }

    /// Resolve the working directory (falls back to `program`'s parent).
    pub fn resolved_cwd(&self) -> PathBuf {
        if let Some(cwd) = &self.cwd {
            return cwd.clone();
        }
        self.program
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."))
    }
}

/// Configuration for attaching to a running Robot Framework debug session.
#[derive(Debug, Clone)]
pub struct AttachConfig {
    /// Host where the RF debug listener is running.
    pub host: String,
    /// Port where the RF debug listener is running.
    pub port: u16,
}

impl Default for AttachConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_owned(),
            port: 6612,
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launch_config_python_executable_default() {
        let cfg = LaunchConfig {
            program: PathBuf::from("/tests/suite.robot"),
            args: vec![],
            cwd: None,
            python: None,
            env: HashMap::new(),
        };
        assert_eq!(cfg.python_executable(), "python3");
    }

    #[test]
    fn launch_config_python_executable_custom() {
        let cfg = LaunchConfig {
            program: PathBuf::from("/tests/suite.robot"),
            args: vec![],
            cwd: None,
            python: Some(PathBuf::from("/usr/bin/python3.11")),
            env: HashMap::new(),
        };
        assert_eq!(cfg.python_executable(), "/usr/bin/python3.11");
    }

    #[test]
    fn launch_config_resolved_cwd_explicit() {
        let cfg = LaunchConfig {
            program: PathBuf::from("/tests/suite.robot"),
            args: vec![],
            cwd: Some(PathBuf::from("/workspace")),
            python: None,
            env: HashMap::new(),
        };
        assert_eq!(cfg.resolved_cwd(), PathBuf::from("/workspace"));
    }

    #[test]
    fn launch_config_resolved_cwd_from_program() {
        let cfg = LaunchConfig {
            program: PathBuf::from("/tests/suite.robot"),
            args: vec![],
            cwd: None,
            python: None,
            env: HashMap::new(),
        };
        assert_eq!(cfg.resolved_cwd(), PathBuf::from("/tests"));
    }

    #[test]
    fn attach_config_default() {
        let cfg = AttachConfig::default();
        assert_eq!(cfg.host, "127.0.0.1");
        assert_eq!(cfg.port, 6612);
    }
}
