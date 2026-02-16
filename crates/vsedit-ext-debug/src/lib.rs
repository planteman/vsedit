//! Ext API: Debug.
//!
//! RPC bridge between the extension host and the main thread for the
//! debug adapter protocol.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Proxy identifier for this extension API namespace.
pub const PROXY_ID: &str = "ext_debug";

// ── RPC Messages ──

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum DebugMessage {
    StartSession {
        configuration: DebugConfiguration,
    },
    StopSession {
        session_id: String,
    },
    SetBreakpoints {
        uri: String,
        breakpoints: Vec<Breakpoint>,
    },
    RemoveBreakpoints {
        uri: String,
        lines: Vec<u32>,
    },
    Continue {
        session_id: String,
        thread_id: u64,
    },
}

// ── Core Types ──

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DebugConfiguration {
    pub name: String,
    #[serde(rename = "type")]
    pub debug_type: String,
    pub request: String,
    pub program: Option<String>,
    pub args: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Breakpoint {
    pub id: Option<String>,
    pub line: u32,
    pub verified: bool,
    pub condition: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DebugSession {
    pub id: String,
    pub name: String,
    pub configuration: DebugConfiguration,
    pub is_active: bool,
}

// ── Bridge ──

pub struct DebugBridge {
    sessions: Vec<DebugSession>,
    breakpoints: Vec<(String, Vec<Breakpoint>)>,
    next_id: u64,
}

impl DebugBridge {
    pub fn new() -> Self {
        Self {
            sessions: Vec::new(),
            breakpoints: Vec::new(),
            next_id: 1,
        }
    }

    pub fn start_session(&mut self, config: DebugConfiguration) -> String {
        let id = format!("debug-{}", self.next_id);
        self.next_id += 1;
        self.sessions.push(DebugSession {
            id: id.clone(),
            name: config.name.clone(),
            configuration: config,
            is_active: true,
        });
        id
    }

    pub fn stop_session(&mut self, session_id: &str) -> bool {
        if let Some(s) = self.sessions.iter_mut().find(|s| s.id == session_id) {
            s.is_active = false;
            true
        } else {
            false
        }
    }

    pub fn active_sessions(&self) -> Vec<&DebugSession> {
        self.sessions.iter().filter(|s| s.is_active).collect()
    }

    pub fn set_breakpoints(&mut self, uri: &str, bps: Vec<Breakpoint>) {
        if let Some(entry) = self.breakpoints.iter_mut().find(|(u, _)| u == uri) {
            entry.1 = bps;
        } else {
            self.breakpoints.push((uri.to_string(), bps));
        }
    }

    pub fn handle_message(&mut self, msg: &DebugMessage) -> serde_json::Value {
        match msg {
            DebugMessage::StartSession { configuration } => {
                let id = self.start_session(configuration.clone());
                serde_json::json!({"sessionId": id})
            }
            DebugMessage::StopSession { session_id } => {
                let ok = self.stop_session(session_id);
                serde_json::json!({"stopped": ok})
            }
            DebugMessage::SetBreakpoints { uri, breakpoints } => {
                self.set_breakpoints(uri, breakpoints.clone());
                serde_json::json!({"set": breakpoints.len()})
            }
            DebugMessage::RemoveBreakpoints { uri, lines } => {
                if let Some(entry) = self.breakpoints.iter_mut().find(|(u, _)| u == uri) {
                    entry.1.retain(|bp| !lines.contains(&bp.line));
                }
                serde_json::json!({"removed": true})
            }
            DebugMessage::Continue { session_id, .. } => {
                let active = self.sessions.iter().any(|s| s.id == *session_id && s.is_active);
                serde_json::json!({"continued": active})
            }
        }
    }
}

impl Default for DebugBridge {
    fn default() -> Self {
        Self::new()
    }
}

// ── Error Types ──

/// Errors that can occur during debug operations.
#[derive(Debug, Clone, PartialEq)]
pub enum DebugError {
    /// The referenced session does not exist.
    SessionNotFound(String),
    /// The session exists but is no longer active.
    SessionInactive(String),
    /// A required configuration field is missing or invalid.
    InvalidConfiguration(String),
    /// A breakpoint could not be set at the requested location.
    BreakpointError { uri: String, line: u32, reason: String },
}

impl fmt::Display for DebugError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DebugError::SessionNotFound(id) => write!(f, "debug session not found: {id}"),
            DebugError::SessionInactive(id) => write!(f, "debug session is inactive: {id}"),
            DebugError::InvalidConfiguration(msg) => write!(f, "invalid configuration: {msg}"),
            DebugError::BreakpointError { uri, line, reason } => {
                write!(f, "breakpoint error at {uri}:{line}: {reason}")
            }
        }
    }
}

impl std::error::Error for DebugError {}

// ── Display impls ──

impl fmt::Display for DebugConfiguration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {} ({})", self.debug_type, self.name, self.request)
    }
}

impl fmt::Display for Breakpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let status = if self.verified { "verified" } else { "pending" };
        match &self.condition {
            Some(cond) => write!(f, "bp@{} [{}] when {}", self.line, status, cond),
            None => write!(f, "bp@{} [{}]", self.line, status),
        }
    }
}

impl fmt::Display for DebugSession {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let state = if self.is_active { "active" } else { "stopped" };
        write!(f, "session {} ({}) [{}]", self.id, self.name, state)
    }
}

// ── DebugConfiguration validation & helpers ──

impl DebugConfiguration {
    /// Validate that all required fields are present and non-empty.
    pub fn validate(&self) -> Result<(), DebugError> {
        if self.name.trim().is_empty() {
            return Err(DebugError::InvalidConfiguration("name must not be empty".into()));
        }
        if self.debug_type.trim().is_empty() {
            return Err(DebugError::InvalidConfiguration("type must not be empty".into()));
        }
        if self.request != "launch" && self.request != "attach" {
            return Err(DebugError::InvalidConfiguration(format!(
                "request must be 'launch' or 'attach', got '{}'",
                self.request
            )));
        }
        if self.request == "launch" && self.program.is_none() {
            return Err(DebugError::InvalidConfiguration(
                "program is required for launch requests".into(),
            ));
        }
        Ok(())
    }

    /// Returns `true` if this is a launch request.
    pub fn is_launch(&self) -> bool {
        self.request == "launch"
    }

    /// Returns `true` if this is an attach request.
    pub fn is_attach(&self) -> bool {
        self.request == "attach"
    }
}

// ── DebugConfigurationBuilder ──

/// Builder for constructing a [`DebugConfiguration`] incrementally.
#[derive(Debug, Clone, Default)]
pub struct DebugConfigurationBuilder {
    name: Option<String>,
    debug_type: Option<String>,
    request: Option<String>,
    program: Option<String>,
    args: Vec<String>,
}

impl DebugConfigurationBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn debug_type(mut self, debug_type: impl Into<String>) -> Self {
        self.debug_type = Some(debug_type.into());
        self
    }

    pub fn request(mut self, request: impl Into<String>) -> Self {
        self.request = Some(request.into());
        self
    }

    pub fn program(mut self, program: impl Into<String>) -> Self {
        self.program = Some(program.into());
        self
    }

    pub fn arg(mut self, arg: impl Into<String>) -> Self {
        self.args.push(arg.into());
        self
    }

    pub fn args(mut self, args: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.args.extend(args.into_iter().map(Into::into));
        self
    }

    /// Build and validate the configuration.
    pub fn build(self) -> Result<DebugConfiguration, DebugError> {
        let config = DebugConfiguration {
            name: self
                .name
                .ok_or_else(|| DebugError::InvalidConfiguration("name is required".into()))?,
            debug_type: self
                .debug_type
                .ok_or_else(|| DebugError::InvalidConfiguration("type is required".into()))?,
            request: self
                .request
                .ok_or_else(|| DebugError::InvalidConfiguration("request is required".into()))?,
            program: self.program,
            args: self.args,
        };
        config.validate()?;
        Ok(config)
    }
}

// ── Breakpoint helpers ──

impl Breakpoint {
    /// Create a simple unconditional breakpoint on the given line.
    pub fn at_line(line: u32) -> Self {
        Self {
            id: None,
            line,
            verified: false,
            condition: None,
        }
    }

    /// Create a conditional breakpoint.
    pub fn conditional(line: u32, condition: impl Into<String>) -> Self {
        Self {
            id: None,
            line,
            verified: false,
            condition: Some(condition.into()),
        }
    }

    /// Return a verified copy of this breakpoint with the given id.
    pub fn verify(self, id: impl Into<String>) -> Self {
        Self {
            id: Some(id.into()),
            verified: true,
            ..self
        }
    }
}

// ── Extended DebugBridge methods ──

impl DebugBridge {
    /// Look up a session by id, returning an error if not found.
    pub fn get_session(&self, session_id: &str) -> Result<&DebugSession, DebugError> {
        self.sessions
            .iter()
            .find(|s| s.id == session_id)
            .ok_or_else(|| DebugError::SessionNotFound(session_id.to_string()))
    }

    /// Require that a session exists *and* is active.
    pub fn require_active_session(&self, session_id: &str) -> Result<&DebugSession, DebugError> {
        let session = self.get_session(session_id)?;
        if !session.is_active {
            return Err(DebugError::SessionInactive(session_id.to_string()));
        }
        Ok(session)
    }

    /// Start a session after validating the configuration.
    pub fn start_session_checked(
        &mut self,
        config: DebugConfiguration,
    ) -> Result<String, DebugError> {
        config.validate()?;
        Ok(self.start_session(config))
    }

    /// Get all breakpoints for a given URI.
    pub fn breakpoints_for_uri(&self, uri: &str) -> &[Breakpoint] {
        self.breakpoints
            .iter()
            .find(|(u, _)| u == uri)
            .map(|(_, bps)| bps.as_slice())
            .unwrap_or(&[])
    }

    /// Remove all breakpoints for a URI, returning them.
    pub fn clear_breakpoints(&mut self, uri: &str) -> Vec<Breakpoint> {
        if let Some(idx) = self.breakpoints.iter().position(|(u, _)| u == uri) {
            self.breakpoints.remove(idx).1
        } else {
            Vec::new()
        }
    }

    /// Total number of breakpoints across all files.
    pub fn total_breakpoint_count(&self) -> usize {
        self.breakpoints.iter().map(|(_, bps)| bps.len()).sum()
    }

    /// Total number of sessions (active and stopped).
    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }
}

/// Initialize the debug extension API bridge.
pub fn register() {
    // Registration will connect RPC handlers when extension host starts
}

/// Accumulated statistics for ext-debug operations.
#[derive(Debug, Clone, PartialEq)]
pub struct ExtDebugStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl ExtDebugStats {
    /// Create a new empty statistics tracker.
    pub fn new() -> Self {
        Self {
            total_operations: 0,
            successful_operations: 0,
            failed_operations: 0,
            last_operation_ns: 0,
            max_operation_ns: 0,
            min_operation_ns: u64::MAX,
            total_time_ns: 0,
        }
    }

    /// Record a successful operation with its duration in nanoseconds.
    pub fn record_success(&mut self, duration_ns: u64) {
        self.total_operations += 1;
        self.successful_operations += 1;
        self.last_operation_ns = duration_ns;
        self.total_time_ns = self.total_time_ns.saturating_add(duration_ns);
        if duration_ns > self.max_operation_ns {
            self.max_operation_ns = duration_ns;
        }
        if duration_ns < self.min_operation_ns {
            self.min_operation_ns = duration_ns;
        }
    }

    /// Record a failed operation with its duration in nanoseconds.
    pub fn record_failure(&mut self, duration_ns: u64) {
        self.total_operations += 1;
        self.failed_operations += 1;
        self.last_operation_ns = duration_ns;
        self.total_time_ns = self.total_time_ns.saturating_add(duration_ns);
        if duration_ns > self.max_operation_ns {
            self.max_operation_ns = duration_ns;
        }
        if duration_ns < self.min_operation_ns {
            self.min_operation_ns = duration_ns;
        }
    }

    /// Return the average operation time in nanoseconds, or 0 if no operations recorded.
    pub fn average_time_ns(&self) -> u64 {
        if self.total_operations == 0 {
            return 0;
        }
        self.total_time_ns / self.total_operations
    }

    /// Return the success rate as a fraction in [0.0, 1.0].
    pub fn success_rate(&self) -> f64 {
        if self.total_operations == 0 {
            return 1.0;
        }
        self.successful_operations as f64 / self.total_operations as f64
    }

    /// Return the failure rate as a fraction in [0.0, 1.0].
    pub fn failure_rate(&self) -> f64 {
        1.0 - self.success_rate()
    }

    /// Return total number of recorded operations.
    pub fn total(&self) -> u64 {
        self.total_operations
    }

    /// Return the minimum operation time, or `None` if no operations recorded.
    pub fn min_time_ns(&self) -> Option<u64> {
        if self.total_operations == 0 {
            None
        } else {
            Some(self.min_operation_ns)
        }
    }

    /// Return the maximum operation time, or `None` if no operations recorded.
    pub fn max_time_ns(&self) -> Option<u64> {
        if self.total_operations == 0 {
            None
        } else {
            Some(self.max_operation_ns)
        }
    }

    /// Reset all counters to zero.
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Merge another stats instance into this one.
    pub fn merge(&mut self, other: &ExtDebugStats) {
        self.total_operations += other.total_operations;
        self.successful_operations += other.successful_operations;
        self.failed_operations += other.failed_operations;
        self.total_time_ns = self.total_time_ns.saturating_add(other.total_time_ns);
        if other.max_operation_ns > self.max_operation_ns {
            self.max_operation_ns = other.max_operation_ns;
        }
        if other.total_operations > 0 && other.min_operation_ns < self.min_operation_ns {
            self.min_operation_ns = other.min_operation_ns;
        }
    }
}

impl Default for ExtDebugStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ExtDebugStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ExtDebugStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for ext-debug.
#[derive(Debug, Clone)]
pub struct ExtDebugValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl ExtDebugValidator {
    /// Create a new validator with default settings.
    pub fn new() -> Self {
        Self {
            max_name_length: 256,
            allowed_chars: None,
            forbidden_prefixes: Vec::new(),
        }
    }

    /// Set the maximum allowed name length.
    pub fn max_length(mut self, max: usize) -> Self {
        self.max_name_length = max;
        self
    }

    /// Restrict names to only the given characters.
    pub fn allowed_chars(mut self, chars: &[char]) -> Self {
        self.allowed_chars = Some(chars.to_vec());
        self
    }

    /// Add a forbidden prefix.
    pub fn forbid_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.forbidden_prefixes.push(prefix.into());
        self
    }

    /// Validate a name, returning an error description on failure.
    pub fn validate_name(&self, name: &str) -> Result<(), String> {
        if name.is_empty() {
            return Err("name must not be empty".to_string());
        }
        if name.len() > self.max_name_length {
            return Err(format!(
                "name length {} exceeds maximum {}",
                name.len(),
                self.max_name_length
            ));
        }
        if let Some(ref allowed) = self.allowed_chars {
            for ch in name.chars() {
                if !allowed.contains(&ch) {
                    return Err(format!("character '{}' is not allowed", ch));
                }
            }
        }
        for prefix in &self.forbidden_prefixes {
            if name.starts_with(prefix.as_str()) {
                return Err(format!("name must not start with '{}'", prefix));
            }
        }
        Ok(())
    }

    /// Validate that a numeric value is within the given range.
    pub fn validate_range(&self, value: i64, min: i64, max: i64) -> Result<(), String> {
        if value < min || value > max {
            return Err(format!("value {} is outside range [{}..{}]", value, min, max));
        }
        Ok(())
    }

    /// Check whether a string contains only ASCII printable characters.
    pub fn is_ascii_printable(s: &str) -> bool {
        s.chars().all(|c| c.is_ascii_graphic() || c == ' ')
    }

    /// Sanitize a string by removing control characters.
    pub fn sanitize(s: &str) -> String {
        s.chars().filter(|c| !c.is_control()).collect()
    }

    /// Truncate a string to a maximum number of characters, appending an ellipsis if needed.
    pub fn truncate(s: &str, max_chars: usize) -> String {
        if s.chars().count() <= max_chars {
            return s.to_string();
        }
        let truncated: String = s.chars().take(max_chars.saturating_sub(1)).collect();
        format!("{}…", truncated)
    }
}

impl Default for ExtDebugValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ── Debug Variable Containers ──

/// Represents a variable scope container in the debug view.
#[derive(Debug, Clone, PartialEq)]
pub struct DebugVariableContainer {
    pub name: String,
    pub variables: Vec<DebugVariable>,
    pub scope: VariableScope,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DebugVariable {
    pub name: String,
    pub value: String,
    pub var_type: String,
    pub children: Vec<DebugVariable>,
    pub evaluate_name: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VariableScope {
    Local,
    Global,
    Closure,
    Arguments,
}

impl DebugVariableContainer {
    pub fn new(name: impl Into<String>, scope: VariableScope) -> Self {
        Self {
            name: name.into(),
            variables: Vec::new(),
            scope,
        }
    }

    pub fn add_variable(&mut self, var: DebugVariable) {
        self.variables.push(var);
    }

    pub fn find_variable(&self, name: &str) -> Option<&DebugVariable> {
        self.variables.iter().find(|v| v.name == name)
    }

    pub fn variable_count(&self) -> usize {
        self.variables.len()
    }

    /// Flattens the variable tree, returning references to all variables including children.
    pub fn flatten(&self) -> Vec<&DebugVariable> {
        let mut result = Vec::new();
        for var in &self.variables {
            Self::flatten_var(var, &mut result);
        }
        result
    }

    fn flatten_var<'a>(var: &'a DebugVariable, out: &mut Vec<&'a DebugVariable>) {
        out.push(var);
        for child in &var.children {
            Self::flatten_var(child, out);
        }
    }
}

impl DebugVariable {
    pub fn new(name: impl Into<String>, value: impl Into<String>, var_type: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
            var_type: var_type.into(),
            children: Vec::new(),
            evaluate_name: None,
        }
    }

    pub fn add_child(&mut self, child: DebugVariable) {
        self.children.push(child);
    }

    pub fn has_children(&self) -> bool {
        !self.children.is_empty()
    }

    pub fn child_count(&self) -> usize {
        self.children.len()
    }

    pub fn with_evaluate_name(mut self, name: impl Into<String>) -> Self {
        self.evaluate_name = Some(name.into());
        self
    }
}

// ── Debug Watch Panel ──

/// A watch expression tracked in the debug watch panel.
#[derive(Debug, Clone, PartialEq)]
pub struct DebugWatchExpression {
    pub id: u64,
    pub expression: String,
    pub result: Option<String>,
    pub error: Option<String>,
    pub has_children: bool,
}

/// Manages a list of watch expressions.
pub struct DebugWatchPanel {
    expressions: Vec<DebugWatchExpression>,
    next_id: u64,
}

impl DebugWatchPanel {
    pub fn new() -> Self {
        Self {
            expressions: Vec::new(),
            next_id: 1,
        }
    }

    pub fn add_expression(&mut self, expr: &str) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.expressions.push(DebugWatchExpression {
            id,
            expression: expr.to_string(),
            result: None,
            error: None,
            has_children: false,
        });
        id
    }

    pub fn remove_expression(&mut self, id: u64) -> bool {
        let len_before = self.expressions.len();
        self.expressions.retain(|e| e.id != id);
        self.expressions.len() != len_before
    }

    pub fn update_result(&mut self, id: u64, result: String) {
        if let Some(expr) = self.expressions.iter_mut().find(|e| e.id == id) {
            expr.result = Some(result);
            expr.error = None;
        }
    }

    pub fn update_error(&mut self, id: u64, error: String) {
        if let Some(expr) = self.expressions.iter_mut().find(|e| e.id == id) {
            expr.error = Some(error);
            expr.result = None;
        }
    }

    pub fn get_expression(&self, id: u64) -> Option<&DebugWatchExpression> {
        self.expressions.iter().find(|e| e.id == id)
    }

    pub fn expressions(&self) -> &[DebugWatchExpression] {
        &self.expressions
    }

    pub fn clear(&mut self) {
        self.expressions.clear();
    }

    pub fn expression_count(&self) -> usize {
        self.expressions.len()
    }
}

impl Default for DebugWatchPanel {
    fn default() -> Self {
        Self::new()
    }
}

// ── Debug Expression Evaluation ──

/// Result of evaluating a debug expression.
#[derive(Debug, Clone, PartialEq)]
pub struct EvalResult {
    pub value: String,
    pub var_type: String,
    pub has_children: bool,
}

/// Evaluate a simple expression against a set of variable containers.
/// Supports dot-notation for accessing children (e.g. "obj.field").
/// Returns the value if found, or an error string.
pub fn debug_evaluate(
    containers: &[DebugVariableContainer],
    expression: &str,
) -> Result<EvalResult, String> {
    let segments: Vec<&str> = expression.split('.').collect();
    if segments.is_empty() || segments[0].is_empty() {
        return Err("empty expression".to_string());
    }

    let root_name = segments[0];

    // Search all containers for the root variable.
    let mut current: Option<&DebugVariable> = None;
    for container in containers {
        if let Some(var) = container.find_variable(root_name) {
            current = Some(var);
            break;
        }
    }

    let mut var = current.ok_or_else(|| format!("variable '{}' not found", root_name))?;

    // Drill into children for subsequent segments.
    for &seg in &segments[1..] {
        var = var
            .children
            .iter()
            .find(|c| c.name == seg)
            .ok_or_else(|| format!("field '{}' not found on '{}'", seg, var.name))?;
    }

    Ok(EvalResult {
        value: var.value.clone(),
        var_type: var.var_type.clone(),
        has_children: var.has_children(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> DebugConfiguration {
        DebugConfiguration {
            name: "Launch".into(),
            debug_type: "lldb".into(),
            request: "launch".into(),
            program: Some("./target/debug/app".into()),
            args: vec!["--verbose".into()],
        }
    }

    #[test]
    fn proxy_id() {
        assert!(!PROXY_ID.is_empty());
    }

    #[test]
    fn message_roundtrip() {
        let msg = DebugMessage::StartSession {
            configuration: test_config(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let back: DebugMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, back);
    }

    #[test]
    fn breakpoint_serialization() {
        let bp = Breakpoint {
            id: Some("bp1".into()),
            line: 42,
            verified: true,
            condition: Some("x > 0".into()),
        };
        let json = serde_json::to_string(&bp).unwrap();
        let back: Breakpoint = serde_json::from_str(&json).unwrap();
        assert_eq!(bp, back);
    }

    #[test]
    fn bridge_start_and_stop() {
        let mut bridge = DebugBridge::new();
        let id = bridge.start_session(test_config());
        assert_eq!(bridge.active_sessions().len(), 1);
        bridge.stop_session(&id);
        assert_eq!(bridge.active_sessions().len(), 0);
    }

    #[test]
    fn bridge_set_breakpoints() {
        let mut bridge = DebugBridge::new();
        let bps = vec![Breakpoint {
            id: None,
            line: 10,
            verified: false,
            condition: None,
        }];
        bridge.set_breakpoints("file:///a.rs", bps);
        assert_eq!(bridge.breakpoints.len(), 1);
    }

    #[test]
    fn bridge_stop_nonexistent() {
        let mut bridge = DebugBridge::new();
        assert!(!bridge.stop_session("nope"));
    }

    // ── Additional tests ──

    #[test]
    fn builder_produces_valid_config() {
        let config = DebugConfigurationBuilder::new()
            .name("Test Launch")
            .debug_type("lldb")
            .request("launch")
            .program("./app")
            .arg("--flag")
            .args(["a", "b"])
            .build()
            .unwrap();

        assert_eq!(config.name, "Test Launch");
        assert_eq!(config.args, vec!["--flag", "a", "b"]);
        assert!(config.is_launch());
        assert!(!config.is_attach());
    }

    #[test]
    fn builder_rejects_missing_name() {
        let result = DebugConfigurationBuilder::new()
            .debug_type("lldb")
            .request("launch")
            .program("./app")
            .build();
        assert!(matches!(result, Err(DebugError::InvalidConfiguration(_))));
    }

    #[test]
    fn builder_rejects_invalid_request() {
        let result = DebugConfigurationBuilder::new()
            .name("Bad")
            .debug_type("lldb")
            .request("run")
            .program("./app")
            .build();
        assert!(matches!(result, Err(DebugError::InvalidConfiguration(_))));
    }

    #[test]
    fn builder_rejects_launch_without_program() {
        let result = DebugConfigurationBuilder::new()
            .name("No Program")
            .debug_type("lldb")
            .request("launch")
            .build();
        assert!(matches!(result, Err(DebugError::InvalidConfiguration(_))));
    }

    #[test]
    fn attach_config_accepts_no_program() {
        let config = DebugConfigurationBuilder::new()
            .name("Attach")
            .debug_type("lldb")
            .request("attach")
            .build()
            .unwrap();
        assert!(config.is_attach());
        assert!(config.program.is_none());
    }

    #[test]
    fn breakpoint_helpers() {
        let bp = Breakpoint::at_line(10);
        assert_eq!(bp.line, 10);
        assert!(!bp.verified);

        let cond = Breakpoint::conditional(20, "x > 5");
        assert_eq!(cond.condition.as_deref(), Some("x > 5"));

        let verified = bp.verify("bp-1");
        assert!(verified.verified);
        assert_eq!(verified.id.as_deref(), Some("bp-1"));
    }

    #[test]
    fn bridge_get_session_error() {
        let bridge = DebugBridge::new();
        assert_eq!(
            bridge.get_session("missing"),
            Err(DebugError::SessionNotFound("missing".into()))
        );
    }

    #[test]
    fn bridge_require_active_session() {
        let mut bridge = DebugBridge::new();
        let id = bridge.start_session(test_config());
        assert!(bridge.require_active_session(&id).is_ok());

        bridge.stop_session(&id);
        assert_eq!(
            bridge.require_active_session(&id),
            Err(DebugError::SessionInactive(id.clone()))
        );
    }

    #[test]
    fn bridge_breakpoint_queries() {
        let mut bridge = DebugBridge::new();
        let uri = "file:///main.rs";

        assert_eq!(bridge.breakpoints_for_uri(uri).len(), 0);
        assert_eq!(bridge.total_breakpoint_count(), 0);

        bridge.set_breakpoints(uri, vec![Breakpoint::at_line(1), Breakpoint::at_line(5)]);
        assert_eq!(bridge.breakpoints_for_uri(uri).len(), 2);
        assert_eq!(bridge.total_breakpoint_count(), 2);

        let cleared = bridge.clear_breakpoints(uri);
        assert_eq!(cleared.len(), 2);
        assert_eq!(bridge.total_breakpoint_count(), 0);
    }

    #[test]
    fn bridge_handle_continue_inactive() {
        let mut bridge = DebugBridge::new();
        let id = bridge.start_session(test_config());
        bridge.stop_session(&id);
        let resp = bridge.handle_message(&DebugMessage::Continue {
            session_id: id,
            thread_id: 1,
        });
        assert_eq!(resp["continued"], false);
    }

    #[test]
    fn display_impls() {
        let config = test_config();
        assert_eq!(format!("{config}"), "[lldb] Launch (launch)");

        let bp = Breakpoint::at_line(42);
        assert_eq!(format!("{bp}"), "bp@42 [pending]");

        let cond_bp = Breakpoint::conditional(10, "i > 0");
        assert_eq!(format!("{cond_bp}"), "bp@10 [pending] when i > 0");

        let mut bridge = DebugBridge::new();
        let id = bridge.start_session(test_config());
        let session = bridge.get_session(&id).unwrap();
        assert!(format!("{session}").contains("active"));
    }

    #[test]
    fn error_display() {
        let err = DebugError::SessionNotFound("abc".into());
        assert_eq!(format!("{err}"), "debug session not found: abc");

        let err = DebugError::BreakpointError {
            uri: "f.rs".into(),
            line: 5,
            reason: "read-only".into(),
        };
        assert!(format!("{err}").contains("f.rs:5"));
    }

    #[test]
    fn bridge_session_count() {
        let mut bridge = DebugBridge::new();
        assert_eq!(bridge.session_count(), 0);
        bridge.start_session(test_config());
        bridge.start_session(test_config());
        assert_eq!(bridge.session_count(), 2);
    }

    #[test]
    fn ext_debug_stats_new_defaults() {
        let stats = ExtDebugStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn ext_debug_stats_record_success() {
        let mut stats = ExtDebugStats::new();
        stats.record_success(100);
        stats.record_success(200);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.successful_operations, 2);
        assert_eq!(stats.failed_operations, 0);
        assert_eq!(stats.average_time_ns(), 150);
        assert_eq!(stats.min_time_ns(), Some(100));
        assert_eq!(stats.max_time_ns(), Some(200));
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn ext_debug_stats_record_failure() {
        let mut stats = ExtDebugStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn ext_debug_stats_reset() {
        let mut stats = ExtDebugStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn ext_debug_stats_merge() {
        let mut a = ExtDebugStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = ExtDebugStats::new();
        b.record_failure(50);
        b.record_success(400);
        a.merge(&b);
        assert_eq!(a.total(), 4);
        assert_eq!(a.successful_operations, 3);
        assert_eq!(a.failed_operations, 1);
        assert_eq!(a.min_time_ns(), Some(50));
        assert_eq!(a.max_time_ns(), Some(400));
    }

    #[test]
    fn ext_debug_stats_display() {
        let mut stats = ExtDebugStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn ext_debug_stats_default() {
        let stats = ExtDebugStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn ext_debug_validator_accepts_valid_name() {
        let v = ExtDebugValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn ext_debug_validator_rejects_empty() {
        let v = ExtDebugValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn ext_debug_validator_rejects_too_long() {
        let v = ExtDebugValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn ext_debug_validator_forbidden_prefix() {
        let v = ExtDebugValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn ext_debug_validator_allowed_chars() {
        let v = ExtDebugValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn ext_debug_validator_range() {
        let v = ExtDebugValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn ext_debug_sanitize_removes_control() {
        let result = ExtDebugValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn ext_debug_truncate_short_string() {
        assert_eq!(ExtDebugValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn ext_debug_truncate_long_string() {
        let result = ExtDebugValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn ext_debug_is_ascii_printable() {
        assert!(ExtDebugValidator::is_ascii_printable("Hello World 123"));
        assert!(!ExtDebugValidator::is_ascii_printable("Hello\x00World"));
    }

    // ── DebugVariableContainer tests ──

    #[test]
    fn variable_container_new() {
        let c = DebugVariableContainer::new("Locals", VariableScope::Local);
        assert_eq!(c.name, "Locals");
        assert_eq!(c.scope, VariableScope::Local);
        assert_eq!(c.variable_count(), 0);
    }

    #[test]
    fn variable_container_add_and_count() {
        let mut c = DebugVariableContainer::new("Globals", VariableScope::Global);
        c.add_variable(DebugVariable::new("x", "42", "i32"));
        c.add_variable(DebugVariable::new("y", "hello", "String"));
        assert_eq!(c.variable_count(), 2);
    }

    #[test]
    fn variable_container_find_variable() {
        let mut c = DebugVariableContainer::new("Locals", VariableScope::Local);
        c.add_variable(DebugVariable::new("x", "42", "i32"));
        c.add_variable(DebugVariable::new("y", "hello", "String"));
        assert_eq!(c.find_variable("x").unwrap().value, "42");
        assert!(c.find_variable("z").is_none());
    }

    #[test]
    fn variable_container_find_missing() {
        let c = DebugVariableContainer::new("Empty", VariableScope::Closure);
        assert!(c.find_variable("anything").is_none());
    }

    #[test]
    fn variable_container_flatten_no_children() {
        let mut c = DebugVariableContainer::new("Locals", VariableScope::Local);
        c.add_variable(DebugVariable::new("a", "1", "i32"));
        c.add_variable(DebugVariable::new("b", "2", "i32"));
        let flat = c.flatten();
        assert_eq!(flat.len(), 2);
    }

    #[test]
    fn variable_container_flatten_with_children() {
        let mut c = DebugVariableContainer::new("Locals", VariableScope::Local);
        let mut parent = DebugVariable::new("obj", "{...}", "Object");
        parent.add_child(DebugVariable::new("x", "1", "i32"));
        parent.add_child(DebugVariable::new("y", "2", "i32"));
        c.add_variable(parent);
        c.add_variable(DebugVariable::new("z", "3", "i32"));
        let flat = c.flatten();
        assert_eq!(flat.len(), 4); // obj, x, y, z
    }

    #[test]
    fn variable_container_flatten_nested_children() {
        let mut c = DebugVariableContainer::new("Locals", VariableScope::Local);
        let mut grandchild = DebugVariable::new("gc", "deep", "str");
        grandchild.add_child(DebugVariable::new("leaf", "end", "str"));
        let mut child = DebugVariable::new("child", "{}", "Object");
        child.add_child(grandchild);
        let mut root = DebugVariable::new("root", "{}", "Object");
        root.add_child(child);
        c.add_variable(root);
        let flat = c.flatten();
        assert_eq!(flat.len(), 4); // root, child, gc, leaf
    }

    #[test]
    fn variable_container_scope_variants() {
        assert_eq!(
            DebugVariableContainer::new("a", VariableScope::Arguments).scope,
            VariableScope::Arguments
        );
        assert_eq!(
            DebugVariableContainer::new("c", VariableScope::Closure).scope,
            VariableScope::Closure
        );
    }

    // ── DebugVariable tests ──

    #[test]
    fn debug_variable_new() {
        let v = DebugVariable::new("x", "42", "i32");
        assert_eq!(v.name, "x");
        assert_eq!(v.value, "42");
        assert_eq!(v.var_type, "i32");
        assert!(!v.has_children());
        assert_eq!(v.child_count(), 0);
        assert!(v.evaluate_name.is_none());
    }

    #[test]
    fn debug_variable_add_child() {
        let mut v = DebugVariable::new("obj", "{}", "Object");
        v.add_child(DebugVariable::new("field", "10", "i32"));
        assert!(v.has_children());
        assert_eq!(v.child_count(), 1);
    }

    #[test]
    fn debug_variable_with_evaluate_name() {
        let v = DebugVariable::new("x", "42", "i32").with_evaluate_name("self.x");
        assert_eq!(v.evaluate_name.as_deref(), Some("self.x"));
    }

    #[test]
    fn debug_variable_multiple_children() {
        let mut v = DebugVariable::new("arr", "[...]", "Vec");
        v.add_child(DebugVariable::new("0", "a", "char"));
        v.add_child(DebugVariable::new("1", "b", "char"));
        v.add_child(DebugVariable::new("2", "c", "char"));
        assert_eq!(v.child_count(), 3);
    }

    #[test]
    fn debug_variable_has_children_false() {
        let v = DebugVariable::new("simple", "val", "str");
        assert!(!v.has_children());
    }

    #[test]
    fn debug_variable_clone() {
        let mut v = DebugVariable::new("x", "1", "i32");
        v.add_child(DebugVariable::new("c", "2", "i32"));
        let cloned = v.clone();
        assert_eq!(v, cloned);
    }

    #[test]
    fn debug_variable_with_evaluate_name_chained() {
        let v = DebugVariable::new("field", "val", "str")
            .with_evaluate_name("obj.field");
        assert_eq!(v.name, "field");
        assert_eq!(v.evaluate_name.as_deref(), Some("obj.field"));
    }

    #[test]
    fn debug_variable_nested_children() {
        let mut parent = DebugVariable::new("p", "{}", "Object");
        let mut child = DebugVariable::new("c", "{}", "Object");
        child.add_child(DebugVariable::new("gc", "1", "i32"));
        parent.add_child(child);
        assert_eq!(parent.child_count(), 1);
        assert_eq!(parent.children[0].child_count(), 1);
    }

    // ── DebugWatchPanel tests ──

    #[test]
    fn watch_panel_new_is_empty() {
        let panel = DebugWatchPanel::new();
        assert_eq!(panel.expression_count(), 0);
        assert!(panel.expressions().is_empty());
    }

    #[test]
    fn watch_panel_add_expression() {
        let mut panel = DebugWatchPanel::new();
        let id = panel.add_expression("x + 1");
        assert_eq!(id, 1);
        assert_eq!(panel.expression_count(), 1);
        let expr = panel.get_expression(id).unwrap();
        assert_eq!(expr.expression, "x + 1");
        assert!(expr.result.is_none());
        assert!(expr.error.is_none());
    }

    #[test]
    fn watch_panel_unique_ids() {
        let mut panel = DebugWatchPanel::new();
        let id1 = panel.add_expression("a");
        let id2 = panel.add_expression("b");
        let id3 = panel.add_expression("c");
        assert_ne!(id1, id2);
        assert_ne!(id2, id3);
        assert_ne!(id1, id3);
    }

    #[test]
    fn watch_panel_remove_expression() {
        let mut panel = DebugWatchPanel::new();
        let id = panel.add_expression("x");
        assert!(panel.remove_expression(id));
        assert_eq!(panel.expression_count(), 0);
        assert!(!panel.remove_expression(id)); // already removed
    }

    #[test]
    fn watch_panel_update_result() {
        let mut panel = DebugWatchPanel::new();
        let id = panel.add_expression("x");
        panel.update_result(id, "42".into());
        let expr = panel.get_expression(id).unwrap();
        assert_eq!(expr.result.as_deref(), Some("42"));
        assert!(expr.error.is_none());
    }

    #[test]
    fn watch_panel_update_error() {
        let mut panel = DebugWatchPanel::new();
        let id = panel.add_expression("bad_expr");
        panel.update_error(id, "undefined variable".into());
        let expr = panel.get_expression(id).unwrap();
        assert_eq!(expr.error.as_deref(), Some("undefined variable"));
        assert!(expr.result.is_none());
    }

    #[test]
    fn watch_panel_error_clears_result() {
        let mut panel = DebugWatchPanel::new();
        let id = panel.add_expression("x");
        panel.update_result(id, "42".into());
        panel.update_error(id, "err".into());
        let expr = panel.get_expression(id).unwrap();
        assert!(expr.result.is_none());
        assert_eq!(expr.error.as_deref(), Some("err"));
    }

    #[test]
    fn watch_panel_result_clears_error() {
        let mut panel = DebugWatchPanel::new();
        let id = panel.add_expression("x");
        panel.update_error(id, "err".into());
        panel.update_result(id, "ok".into());
        let expr = panel.get_expression(id).unwrap();
        assert_eq!(expr.result.as_deref(), Some("ok"));
        assert!(expr.error.is_none());
    }

    #[test]
    fn watch_panel_clear() {
        let mut panel = DebugWatchPanel::new();
        panel.add_expression("a");
        panel.add_expression("b");
        panel.clear();
        assert_eq!(panel.expression_count(), 0);
    }

    #[test]
    fn watch_panel_get_missing() {
        let panel = DebugWatchPanel::new();
        assert!(panel.get_expression(999).is_none());
    }

    #[test]
    fn watch_panel_expressions_slice() {
        let mut panel = DebugWatchPanel::new();
        panel.add_expression("a");
        panel.add_expression("b");
        let exprs = panel.expressions();
        assert_eq!(exprs.len(), 2);
        assert_eq!(exprs[0].expression, "a");
        assert_eq!(exprs[1].expression, "b");
    }

    // ── debug_evaluate tests ──

    fn make_test_containers() -> Vec<DebugVariableContainer> {
        let mut locals = DebugVariableContainer::new("Locals", VariableScope::Local);
        locals.add_variable(DebugVariable::new("x", "42", "i32"));
        let mut obj = DebugVariable::new("obj", "{...}", "Object");
        obj.add_child(DebugVariable::new("name", "Alice", "String"));
        let mut nested = DebugVariable::new("inner", "{...}", "Object");
        nested.add_child(DebugVariable::new("val", "deep", "str"));
        obj.add_child(nested);
        locals.add_variable(obj);

        let mut globals = DebugVariableContainer::new("Globals", VariableScope::Global);
        globals.add_variable(DebugVariable::new("PI", "3.14", "f64"));

        vec![locals, globals]
    }

    #[test]
    fn eval_simple_variable() {
        let containers = make_test_containers();
        let result = debug_evaluate(&containers, "x").unwrap();
        assert_eq!(result.value, "42");
        assert_eq!(result.var_type, "i32");
        assert!(!result.has_children);
    }

    #[test]
    fn eval_dot_notation() {
        let containers = make_test_containers();
        let result = debug_evaluate(&containers, "obj.name").unwrap();
        assert_eq!(result.value, "Alice");
        assert_eq!(result.var_type, "String");
    }

    #[test]
    fn eval_nested_dot_notation() {
        let containers = make_test_containers();
        let result = debug_evaluate(&containers, "obj.inner.val").unwrap();
        assert_eq!(result.value, "deep");
        assert_eq!(result.var_type, "str");
        assert!(!result.has_children);
    }

    #[test]
    fn eval_parent_has_children() {
        let containers = make_test_containers();
        let result = debug_evaluate(&containers, "obj").unwrap();
        assert!(result.has_children);
    }

    #[test]
    fn eval_global_variable() {
        let containers = make_test_containers();
        let result = debug_evaluate(&containers, "PI").unwrap();
        assert_eq!(result.value, "3.14");
        assert_eq!(result.var_type, "f64");
    }

    #[test]
    fn eval_missing_variable() {
        let containers = make_test_containers();
        let result = debug_evaluate(&containers, "nonexistent");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    #[test]
    fn eval_missing_field() {
        let containers = make_test_containers();
        let result = debug_evaluate(&containers, "obj.missing");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    #[test]
    fn eval_empty_expression() {
        let containers = make_test_containers();
        let result = debug_evaluate(&containers, "");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("empty"));
    }

    #[test]
    fn eval_empty_containers() {
        let result = debug_evaluate(&[], "x");
        assert!(result.is_err());
    }

    #[test]
    fn eval_dot_on_leaf() {
        let containers = make_test_containers();
        let result = debug_evaluate(&containers, "x.field");
        assert!(result.is_err());
    }
}
