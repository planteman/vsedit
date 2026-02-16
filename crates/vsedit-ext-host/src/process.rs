//! Extension host child-process management.

use std::io::{self, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use vsedit_ext_rpc::RpcMessage;

use crate::transport::{decode_message, encode_message};

// ---------------------------------------------------------------------------
// ExtensionRuntime
// ---------------------------------------------------------------------------

/// Runtime used to execute extension code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtensionRuntime {
    Node,
    Deno,
}

impl ExtensionRuntime {
    /// Default binary name for this runtime.
    pub fn binary_name(self) -> &'static str {
        match self {
            Self::Node => "node",
            Self::Deno => "deno",
        }
    }
}

// ---------------------------------------------------------------------------
// ExtensionHostConfig
// ---------------------------------------------------------------------------

/// Configuration for spawning an extension host process.
#[derive(Debug, Clone)]
pub struct ExtensionHostConfig {
    /// Which JS runtime to use.
    pub runtime: ExtensionRuntime,
    /// Paths to extension directories to load.
    pub extension_paths: Vec<PathBuf>,
    /// Log level forwarded to the extension host.
    pub log_level: String,
    /// Locale forwarded to the extension host.
    pub locale: String,
    /// Optional override for the runtime binary path.
    pub runtime_path: Option<PathBuf>,
    /// Optional path to the extension host bootstrap script.
    pub boot_script: Option<PathBuf>,
}

impl Default for ExtensionHostConfig {
    fn default() -> Self {
        Self {
            runtime: ExtensionRuntime::Node,
            extension_paths: Vec::new(),
            log_level: "info".into(),
            locale: "en".into(),
            runtime_path: None,
            boot_script: None,
        }
    }
}

impl ExtensionHostConfig {
    /// Resolve the boot script path, falling back to the bundled
    /// `runtime/extHostMain.js` next to the current executable.
    pub fn resolved_boot_script(&self) -> PathBuf {
        if let Some(ref p) = self.boot_script {
            return p.clone();
        }
        // Look next to the binary first, then relative to CWD
        if let Ok(exe) = std::env::current_exe() {
            let candidate = exe
                .parent()
                .unwrap_or(Path::new("."))
                .join("runtime")
                .join("extHostMain.js");
            if candidate.exists() {
                return candidate;
            }
        }
        PathBuf::from("runtime/extHostMain.js")
    }
}

// ---------------------------------------------------------------------------
// ExtensionHostProcess
// ---------------------------------------------------------------------------

/// Manages a single extension-host child process and its stdio transport.
pub struct ExtensionHostProcess {
    child: Child,
    reader: BufReader<std::process::ChildStdout>,
    stdin: Option<std::process::ChildStdin>,
}

impl ExtensionHostProcess {
    /// Spawn a new extension host process using the supplied configuration.
    ///
    /// The child process is started with `stdin` and `stdout` piped so we can
    /// communicate using the `Content-Length`-framed JSON-RPC protocol.
    pub fn spawn(config: &ExtensionHostConfig) -> io::Result<Self> {
        let binary = config
            .runtime_path
            .as_deref()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|| config.runtime.binary_name().to_string());

        let mut cmd = Command::new(&binary);
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // Always pass the boot script (resolved from config or default)
        let boot_script = config.resolved_boot_script();
        cmd.arg(&boot_script);

        cmd.env("VSEDIT_LOG_LEVEL", &config.log_level);
        cmd.env("VSEDIT_LOCALE", &config.locale);

        let ext_paths: Vec<String> = config
            .extension_paths
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect();
        cmd.env("VSEDIT_EXTENSION_PATHS", ext_paths.join(":"));

        tracing::debug!(binary = %binary, "spawning extension host process");

        let mut child = cmd.spawn()?;
        let stdout = child.stdout.take().expect("stdout piped");
        let stdin = child.stdin.take().expect("stdin piped");

        Ok(Self {
            child,
            reader: BufReader::new(stdout),
            stdin: Some(stdin),
        })
    }

    /// Spawn an arbitrary command as an extension host process.
    ///
    /// This is useful for testing: pass `"cat"` to get an echo-style process.
    pub fn spawn_raw(program: &str, args: &[&str]) -> io::Result<Self> {
        let mut cmd = Command::new(program);
        cmd.args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = cmd.spawn()?;
        let stdout = child.stdout.take().expect("stdout piped");
        let stdin = child.stdin.take().expect("stdin piped");

        Ok(Self {
            child,
            reader: BufReader::new(stdout),
            stdin: Some(stdin),
        })
    }

    /// Send an RPC message to the child process via stdin.
    pub fn send_message(&mut self, msg: &RpcMessage) -> io::Result<()> {
        use std::io::Write;
        let writer = self
            .stdin
            .as_mut()
            .ok_or_else(|| io::Error::new(io::ErrorKind::BrokenPipe, "stdin closed"))?;
        let encoded = encode_message(msg);
        writer.write_all(&encoded)?;
        writer.flush()
    }

    /// Receive an RPC message from the child process via stdout.
    ///
    /// Returns `Ok(None)` when the child's stdout is closed (EOF).
    pub fn recv_message(&mut self) -> io::Result<Option<RpcMessage>> {
        decode_message(&mut self.reader)
    }

    /// Check whether the child process is still running.
    pub fn is_alive(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }

    /// Forcibly kill the child process.
    pub fn kill(&mut self) {
        // Close stdin first so the child sees EOF.
        self.stdin.take();
        let _ = self.child.kill();
        let _ = self.child.wait();
    }

    /// Close the stdin pipe without killing the process.
    ///
    /// This signals EOF to the child, which is useful for programs like `cat`
    /// that read until EOF.
    pub fn close_stdin(&mut self) {
        self.stdin.take();
    }

    /// Return the child's PID.
    pub fn pid(&self) -> u32 {
        self.child.id()
    }
}

impl Drop for ExtensionHostProcess {
    fn drop(&mut self) {
        self.kill();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use vsedit_ext_rpc::RpcRequest;

    #[test]
    fn config_defaults() {
        let cfg = ExtensionHostConfig::default();
        assert_eq!(cfg.runtime, ExtensionRuntime::Node);
        assert!(cfg.extension_paths.is_empty());
        assert_eq!(cfg.log_level, "info");
        assert_eq!(cfg.locale, "en");
        assert!(cfg.runtime_path.is_none());
        assert!(cfg.boot_script.is_none());
    }

    #[test]
    fn runtime_binary_names() {
        assert_eq!(ExtensionRuntime::Node.binary_name(), "node");
        assert_eq!(ExtensionRuntime::Deno.binary_name(), "deno");
    }

    #[test]
    fn spawn_raw_echo_process() {
        // `cat` echoes stdin back to stdout — we can do a message roundtrip.
        let mut proc = ExtensionHostProcess::spawn_raw("cat", &[]).unwrap();
        assert!(proc.is_alive());

        let msg = RpcMessage::Request(RpcRequest {
            id: 1,
            proxy_id: "Test".into(),
            method: "ping".into(),
            args: vec![json!("hello")],
        });

        proc.send_message(&msg).unwrap();
        // Close stdin so cat sees EOF and flushes its output.
        proc.close_stdin();

        let received = decode_message(&mut proc.reader).unwrap().unwrap();
        assert_eq!(received, msg);
    }

    #[test]
    fn spawn_raw_process_kill() {
        let mut proc = ExtensionHostProcess::spawn_raw("cat", &[]).unwrap();
        assert!(proc.is_alive());
        proc.kill();
        assert!(!proc.is_alive());
    }

    #[test]
    fn spawn_raw_invalid_program() {
        let result = ExtensionHostProcess::spawn_raw("this-binary-does-not-exist-12345", &[]);
        assert!(result.is_err());
    }
}
