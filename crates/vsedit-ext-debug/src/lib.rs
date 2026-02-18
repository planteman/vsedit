//! Ext API: Debug.
//!
//! RPC bridge between the extension host and the main thread for the
//! debug adapter protocol.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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

// ── Breakpoint Hit Count Evaluation ──

/// Specifies when a breakpoint should trigger based on its hit count.
#[derive(Debug, Clone, PartialEq)]
pub enum HitCondition {
    /// Break when hit count equals the value.
    Equal(u64),
    /// Break when hit count is greater than or equal to the value.
    GreaterOrEqual(u64),
    /// Break every N-th hit (modulo).
    Multiple(u64),
}

impl HitCondition {
    /// Parse a hit condition string such as `"= 5"`, `">= 10"`, or `"% 3"`.
    pub fn parse(s: &str) -> Result<Self, String> {
        let s = s.trim();
        if let Some(rest) = s.strip_prefix(">=") {
            let n: u64 = rest.trim().parse().map_err(|_| format!("invalid number in hit condition: '{}'", rest.trim()))?;
            Ok(HitCondition::GreaterOrEqual(n))
        } else if let Some(rest) = s.strip_prefix('%') {
            let n: u64 = rest.trim().parse().map_err(|_| format!("invalid number in hit condition: '{}'", rest.trim()))?;
            if n == 0 {
                return Err("modulo hit condition must be non-zero".to_string());
            }
            Ok(HitCondition::Multiple(n))
        } else if let Some(rest) = s.strip_prefix('=') {
            let n: u64 = rest.trim().parse().map_err(|_| format!("invalid number in hit condition: '{}'", rest.trim()))?;
            Ok(HitCondition::Equal(n))
        } else {
            // Try parsing as a bare number (treated as equal).
            let n: u64 = s.parse().map_err(|_| format!("unrecognised hit condition: '{s}'"))?;
            Ok(HitCondition::Equal(n))
        }
    }

    /// Returns `true` if the breakpoint should fire at the given hit count.
    pub fn should_break(&self, hit_count: u64) -> bool {
        match self {
            HitCondition::Equal(n) => hit_count == *n,
            HitCondition::GreaterOrEqual(n) => hit_count >= *n,
            HitCondition::Multiple(n) => hit_count > 0 && hit_count % *n == 0,
        }
    }
}

// ── Call Stack Frame Formatting ──

/// Represents a single frame in the debug call stack.
#[derive(Debug, Clone, PartialEq)]
pub struct StackFrame {
    pub id: u64,
    pub name: String,
    pub source_path: Option<String>,
    pub line: u32,
    pub column: u32,
    pub module_name: Option<String>,
}

impl StackFrame {
    pub fn new(id: u64, name: impl Into<String>, line: u32, column: u32) -> Self {
        Self {
            id,
            name: name.into(),
            source_path: None,
            line,
            column,
            module_name: None,
        }
    }

    pub fn with_source(mut self, path: impl Into<String>) -> Self {
        self.source_path = Some(path.into());
        self
    }

    pub fn with_module(mut self, module: impl Into<String>) -> Self {
        self.module_name = Some(module.into());
        self
    }

    /// Format the frame as a human-readable one-line summary.
    pub fn format_summary(&self) -> String {
        let location = match &self.source_path {
            Some(p) => format!("{}:{}:{}", p, self.line, self.column),
            None => format!("<unknown>:{}:{}", self.line, self.column),
        };
        match &self.module_name {
            Some(m) => format!("#{} {} [{}] at {}", self.id, self.name, m, location),
            None => format!("#{} {} at {}", self.id, self.name, location),
        }
    }
}

impl fmt::Display for StackFrame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.format_summary())
    }
}

/// Format an entire call stack into a multi-line string.
pub fn format_call_stack(frames: &[StackFrame]) -> String {
    frames
        .iter()
        .map(|f| f.format_summary())
        .collect::<Vec<_>>()
        .join("\n")
}

// ── Debug Console Command Parsing ──

/// Commands that can be entered in the debug console REPL.
#[derive(Debug, Clone, PartialEq)]
pub enum DebugConsoleCommand {
    /// Evaluate an expression and print the result.
    Evaluate(String),
    /// Set a variable to a new value.
    SetVariable { name: String, value: String },
    /// Step into the next statement.
    StepIn,
    /// Step over the next statement.
    StepOver,
    /// Step out of the current function.
    StepOut,
    /// Continue execution.
    Continue,
    /// Show the call stack.
    Backtrace,
    /// Unknown / unparseable command.
    Unknown(String),
}

impl DebugConsoleCommand {
    /// Parse a raw console input line into a command.
    pub fn parse(input: &str) -> Self {
        let input = input.trim();
        if input.is_empty() {
            return DebugConsoleCommand::Unknown(String::new());
        }

        // Check for built-in commands (case-insensitive prefix).
        let lower = input.to_ascii_lowercase();
        if lower == "stepin" || lower == "si" {
            return DebugConsoleCommand::StepIn;
        }
        if lower == "stepover" || lower == "so" || lower == "next" {
            return DebugConsoleCommand::StepOver;
        }
        if lower == "stepout" {
            return DebugConsoleCommand::StepOut;
        }
        if lower == "continue" || lower == "c" {
            return DebugConsoleCommand::Continue;
        }
        if lower == "bt" || lower == "backtrace" {
            return DebugConsoleCommand::Backtrace;
        }

        // `set <name> = <value>`
        if let Some(rest) = lower.strip_prefix("set ") {
            let original_rest = &input[4..];
            if let Some(eq_pos) = original_rest.find('=') {
                let name = original_rest[..eq_pos].trim().to_string();
                let value = original_rest[eq_pos + 1..].trim().to_string();
                if !name.is_empty() && !value.is_empty() {
                    return DebugConsoleCommand::SetVariable { name, value };
                }
            }
            // Malformed set – treat the whole thing as unknown.
            return DebugConsoleCommand::Unknown(input.to_string());
        }

        // Everything else is an expression evaluation.
        DebugConsoleCommand::Evaluate(input.to_string())
    }
}

// ── Debug Output Filtering ──

/// Categories for debug output messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DebugOutputCategory {
    Console,
    Stdout,
    Stderr,
    Telemetry,
}

/// A single debug output entry.
#[derive(Debug, Clone, PartialEq)]
pub struct DebugOutputEntry {
    pub category: DebugOutputCategory,
    pub text: String,
    pub source: Option<String>,
}

/// Collects debug output and provides filtering by category.
pub struct DebugOutputLog {
    entries: Vec<DebugOutputEntry>,
    max_entries: usize,
}

impl DebugOutputLog {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            max_entries,
        }
    }

    pub fn append(&mut self, category: DebugOutputCategory, text: impl Into<String>) {
        if self.entries.len() >= self.max_entries {
            self.entries.remove(0);
        }
        self.entries.push(DebugOutputEntry {
            category,
            text: text.into(),
            source: None,
        });
    }

    pub fn append_with_source(
        &mut self,
        category: DebugOutputCategory,
        text: impl Into<String>,
        source: impl Into<String>,
    ) {
        if self.entries.len() >= self.max_entries {
            self.entries.remove(0);
        }
        self.entries.push(DebugOutputEntry {
            category,
            text: text.into(),
            source: Some(source.into()),
        });
    }

    /// Return all entries matching the given category.
    pub fn filter_by_category(&self, category: DebugOutputCategory) -> Vec<&DebugOutputEntry> {
        self.entries.iter().filter(|e| e.category == category).collect()
    }

    /// Return all entries whose text contains the given substring.
    pub fn search(&self, needle: &str) -> Vec<&DebugOutputEntry> {
        self.entries.iter().filter(|e| e.text.contains(needle)).collect()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

// ── Breakpoint Conditions ──

/// Operator for hit-count based breakpoint conditions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum HitCountOp {
    Equal,
    GreaterThan,
    GreaterEqual,
    /// Break every N-th hit.
    Multiple,
}

/// A hit-count condition attached to a breakpoint.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HitCountCondition {
    pub operator: HitCountOp,
    pub value: u32,
}

/// Conditional breakpoint descriptor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebugBreakpointCondition {
    pub expression: String,
    pub hit_count_condition: Option<HitCountCondition>,
    pub log_message: Option<String>,
}

impl DebugBreakpointCondition {
    pub fn new(expr: &str) -> Self {
        Self {
            expression: expr.to_string(),
            hit_count_condition: None,
            log_message: None,
        }
    }

    pub fn with_hit_count(mut self, op: HitCountOp, value: u32) -> Self {
        self.hit_count_condition = Some(HitCountCondition { operator: op, value });
        self
    }

    pub fn with_log_message(mut self, msg: &str) -> Self {
        self.log_message = Some(msg.to_string());
        self
    }

    /// Returns `true` when the debugger should pause, given the current
    /// cumulative `hit_count` for this breakpoint location.
    pub fn should_break(&self, hit_count: u32) -> bool {
        match &self.hit_count_condition {
            None => true,
            Some(cond) => match cond.operator {
                HitCountOp::Equal => hit_count == cond.value,
                HitCountOp::GreaterThan => hit_count > cond.value,
                HitCountOp::GreaterEqual => hit_count >= cond.value,
                HitCountOp::Multiple => cond.value > 0 && hit_count % cond.value == 0,
            },
        }
    }

    pub fn evaluate_expression(&self) -> &str {
        &self.expression
    }
}

impl fmt::Display for DebugBreakpointCondition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Condition: {}", self.expression)?;
        if let Some(ref hc) = self.hit_count_condition {
            let op_str = match hc.operator {
                HitCountOp::Equal => "==",
                HitCountOp::GreaterThan => ">",
                HitCountOp::GreaterEqual => ">=",
                HitCountOp::Multiple => "%",
            };
            write!(f, " (hit count {} {})", op_str, hc.value)?;
        }
        if let Some(ref msg) = self.log_message {
            write!(f, " [log: {}]", msg)?;
        }
        Ok(())
    }
}

// ── Watch Expressions ──

/// A single entry in the watch panel, supporting a tree of children.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchEntry {
    pub expression: String,
    pub value: Option<String>,
    pub children: Vec<WatchEntry>,
    pub expandable: bool,
}

/// Watch tree: a hierarchical collection of watch entries with child expansion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebugWatchTree {
    pub expressions: Vec<WatchEntry>,
}

impl DebugWatchTree {
    pub fn new() -> Self {
        Self { expressions: Vec::new() }
    }

    pub fn add_expression(&mut self, expr: &str) {
        self.expressions.push(WatchEntry {
            expression: expr.to_string(),
            value: None,
            children: Vec::new(),
            expandable: false,
        });
    }

    pub fn update_value(&mut self, expr: &str, value: &str) -> bool {
        for entry in &mut self.expressions {
            if entry.expression == expr {
                entry.value = Some(value.to_string());
                return true;
            }
        }
        false
    }

    pub fn remove(&mut self, expr: &str) -> bool {
        let before = self.expressions.len();
        self.expressions.retain(|e| e.expression != expr);
        self.expressions.len() != before
    }

    pub fn get(&self, expr: &str) -> Option<&WatchEntry> {
        self.expressions.iter().find(|e| e.expression == expr)
    }

    pub fn all_expressions(&self) -> Vec<&str> {
        self.expressions.iter().map(|e| e.expression.as_str()).collect()
    }

    /// Render a human-readable, indented tree view of all watch entries.
    pub fn render_tree(&self) -> String {
        let mut buf = String::new();
        for entry in &self.expressions {
            Self::render_entry(&mut buf, entry, 0);
        }
        buf
    }

    fn render_entry(buf: &mut String, entry: &WatchEntry, depth: usize) {
        let indent = "  ".repeat(depth);
        match &entry.value {
            Some(v) => buf.push_str(&format!("{}{} = {}\n", indent, entry.expression, v)),
            None => buf.push_str(&format!("{}{}\n", indent, entry.expression)),
        }
        for child in &entry.children {
            Self::render_entry(buf, child, depth + 1);
        }
    }
}

// ── Hover Evaluator ──

/// Caches hover evaluation results so repeated hovers over the same
/// expression avoid redundant DAP requests.
#[derive(Debug, Clone)]
pub struct DebugHoverEvaluator {
    pub cache: HashMap<String, String>,
    pub max_cache_size: usize,
}

impl DebugHoverEvaluator {
    pub fn new(max_cache: usize) -> Self {
        Self {
            cache: HashMap::new(),
            max_cache_size: max_cache,
        }
    }

    /// Store an evaluated result. If the cache is at capacity the insertion
    /// is still performed but the oldest arbitrary entry is evicted first.
    pub fn evaluate(&mut self, expression: &str, value: &str) {
        if self.cache.len() >= self.max_cache_size && !self.cache.contains_key(expression) {
            if let Some(key) = self.cache.keys().next().cloned() {
                self.cache.remove(&key);
            }
        }
        self.cache.insert(expression.to_string(), value.to_string());
    }

    pub fn get_cached(&self, expression: &str) -> Option<&str> {
        self.cache.get(expression).map(|s| s.as_str())
    }

    pub fn invalidate_all(&mut self) {
        self.cache.clear();
    }

    pub fn cache_size(&self) -> usize {
        self.cache.len()
    }

    pub fn format_hover(expression: &str, value: &str) -> String {
        format!("{} = {}", expression, value)
    }
}

// ── Call Stack ──

/// Debug call-stack navigator wrapping a sequence of existing `StackFrame`s.
#[derive(Debug, Clone)]
pub struct DebugCallStack {
    pub frames: Vec<StackFrame>,
}

impl DebugCallStack {
    pub fn new() -> Self {
        Self { frames: Vec::new() }
    }

    pub fn push_frame(&mut self, frame: StackFrame) {
        self.frames.push(frame);
    }

    pub fn pop_frame(&mut self) -> Option<StackFrame> {
        self.frames.pop()
    }

    /// The most recently pushed frame (top of stack).
    pub fn current_frame(&self) -> Option<&StackFrame> {
        self.frames.last()
    }

    pub fn depth(&self) -> usize {
        self.frames.len()
    }

    pub fn frame_at(&self, index: usize) -> Option<&StackFrame> {
        self.frames.get(index)
    }

    /// Navigate toward the caller (lower index).
    pub fn navigate_up(&self, from: usize) -> Option<usize> {
        if from > 0 { Some(from - 1) } else { None }
    }

    /// Navigate toward the callee (higher index).
    pub fn navigate_down(&self, from: usize) -> Option<usize> {
        if from + 1 < self.frames.len() { Some(from + 1) } else { None }
    }

    /// Render the call stack for display, most-recent frame first.
    pub fn render(&self) -> String {
        let mut buf = String::new();
        for (i, frame) in self.frames.iter().enumerate().rev() {
            buf.push_str(&format!("#{} {}\n", i, frame));
        }
        buf
    }
}

// ---------------------------------------------------------------------------
// DebugVariableFormatter - debug variable formatter
// ---------------------------------------------------------------------------

/// Severity level for debug variable formatter issues.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DebugVariableFormatterSeverity {
    Low,
    Medium,
    High,
    Critical,
}

impl fmt::Display for DebugVariableFormatterSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Medium => write!(f, "medium"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// Entry tracked by [DebugVariableFormatter].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebugVariableFormatterEntry {
    pub id: String,
    pub label: String,
    pub severity: DebugVariableFormatterSeverity,
    pub detail: Option<String>,
    pub var_count: usize,
    enabled: bool,
}

impl DebugVariableFormatterEntry {
    pub fn new(id: &str, label: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            severity: DebugVariableFormatterSeverity::Low,
            detail: None,
            var_count: 0,
            enabled: true,
        }
    }

    pub fn with_severity(mut self, severity: DebugVariableFormatterSeverity) -> Self {
        self.severity = severity;
        self
    }

    pub fn with_detail(mut self, detail: &str) -> Self {
        self.detail = Some(detail.to_string());
        self
    }

    pub fn with_var_count(mut self, val: usize) -> Self {
        self.var_count = val;
        self
    }

    pub fn is_expandable(&self) -> bool {
        self.enabled && self.severity >= DebugVariableFormatterSeverity::Medium
    }

    pub fn disable(&mut self) {
        self.enabled = false;
    }

    pub fn enable(&mut self) {
        self.enabled = true;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn format_line(&self) -> String {
        let det = self.detail.as_deref().unwrap_or("-");
        format!("[{}] {} ({}): {}", self.severity, self.id, self.var_count, det)
    }
}

impl fmt::Display for DebugVariableFormatterEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} [{}]", self.label, self.severity)
    }
}

/// Manages a collection of [DebugVariableFormatterEntry] items.
#[derive(Debug, Clone)]
pub struct DebugVariableFormatter {
    entries: Vec<DebugVariableFormatterEntry>,
    name: String,
    capacity: usize,
}

impl DebugVariableFormatter {
    pub fn new(name: &str) -> Self {
        Self { entries: Vec::new(), name: name.to_string(), capacity: 1000 }
    }

    pub fn with_capacity(mut self, cap: usize) -> Self {
        self.capacity = cap;
        self
    }

    pub fn add(&mut self, entry: DebugVariableFormatterEntry) -> bool {
        if self.entries.len() >= self.capacity {
            return false;
        }
        self.entries.push(entry);
        true
    }

    pub fn remove(&mut self, id: &str) -> Option<DebugVariableFormatterEntry> {
        if let Some(pos) = self.entries.iter().position(|e| e.id == id) {
            Some(self.entries.remove(pos))
        } else {
            None
        }
    }

    pub fn get(&self, id: &str) -> Option<&DebugVariableFormatterEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn var_count(&self) -> usize { self.entries.len() }

    pub fn is_expandable(&self) -> bool {
        self.entries.iter().any(|e| e.is_expandable())
    }

    pub fn entries_by_severity(&self, severity: DebugVariableFormatterSeverity) -> Vec<&DebugVariableFormatterEntry> {
        self.entries.iter().filter(|e| e.severity == severity).collect()
    }

    pub fn high_severity_count(&self) -> usize {
        self.entries.iter().filter(|e| e.severity >= DebugVariableFormatterSeverity::High).count()
    }

    pub fn sorted_by_severity(&self) -> Vec<&DebugVariableFormatterEntry> {
        let mut sorted: Vec<_> = self.entries.iter().collect();
        sorted.sort_by(|a, b| b.severity.cmp(&a.severity));
        sorted
    }

    pub fn generate_summary(&self) -> String {
        format!(
            "{} | Total: {} | High+: {}",
            self.name, self.entries.len(), self.high_severity_count()
        )
    }

    pub fn clear(&mut self) { self.entries.clear(); }

    pub fn is_empty(&self) -> bool { self.entries.is_empty() }

    pub fn enabled_entries(&self) -> Vec<&DebugVariableFormatterEntry> {
        self.entries.iter().filter(|e| e.is_enabled()).collect()
    }

    pub fn disable_all(&mut self) {
        for e in &mut self.entries { e.disable(); }
    }

    pub fn enable_all(&mut self) {
        for e in &mut self.entries { e.enable(); }
    }
}

// ---------------------------------------------------------------------------
// DebugMemoryViewer - debug memory viewer
// ---------------------------------------------------------------------------

/// Configuration for [DebugMemoryViewer].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebugMemoryViewerConfig {
    pub max_items: usize,
    pub label: String,
    pub auto_refresh: bool,
    pub memory_size: usize,
}

impl DebugMemoryViewerConfig {
    pub fn new(label: &str) -> Self {
        Self { max_items: 100, label: label.to_string(), auto_refresh: true, memory_size: 0 }
    }

    pub fn with_max_items(mut self, max: usize) -> Self { self.max_items = max; self }

    pub fn with_auto_refresh(mut self, auto: bool) -> Self { self.auto_refresh = auto; self }

    pub fn with_memory_size(mut self, val: usize) -> Self { self.memory_size = val; self }
}

impl Default for DebugMemoryViewerConfig {
    fn default() -> Self { Self::new("default") }
}

/// Item tracked by [DebugMemoryViewer].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebugMemoryViewerItem {
    pub key: String,
    pub value: String,
    pub priority: u32,
    pub tags: Vec<String>,
}

impl DebugMemoryViewerItem {
    pub fn new(key: &str, value: &str) -> Self {
        Self { key: key.to_string(), value: value.to_string(), priority: 0, tags: Vec::new() }
    }

    pub fn with_priority(mut self, p: u32) -> Self { self.priority = p; self }

    pub fn with_tag(mut self, tag: &str) -> Self {
        self.tags.push(tag.to_string());
        self
    }

    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }

    pub fn has_children(&self) -> bool {
        self.priority > 0 && !self.tags.is_empty()
    }
}

impl fmt::Display for DebugMemoryViewerItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}={}", self.key, self.value)
    }
}

/// Manages [DebugMemoryViewerItem] entries with configuration.
#[derive(Debug, Clone)]
pub struct DebugMemoryViewer {
    config: DebugMemoryViewerConfig,
    items: Vec<DebugMemoryViewerItem>,
}

impl DebugMemoryViewer {
    pub fn new(config: DebugMemoryViewerConfig) -> Self {
        Self { config, items: Vec::new() }
    }

    pub fn add(&mut self, item: DebugMemoryViewerItem) -> bool {
        if self.items.len() >= self.config.max_items {
            return false;
        }
        self.items.push(item);
        true
    }

    pub fn remove(&mut self, key: &str) -> Option<DebugMemoryViewerItem> {
        if let Some(pos) = self.items.iter().position(|i| i.key == key) {
            Some(self.items.remove(pos))
        } else {
            None
        }
    }

    pub fn get(&self, key: &str) -> Option<&DebugMemoryViewerItem> {
        self.items.iter().find(|i| i.key == key)
    }

    pub fn memory_size(&self) -> usize { self.items.len() }

    pub fn has_children(&self) -> bool {
        self.items.iter().any(|i| i.has_children())
    }

    pub fn items_with_tag(&self, tag: &str) -> Vec<&DebugMemoryViewerItem> {
        self.items.iter().filter(|i| i.has_tag(tag)).collect()
    }

    pub fn sorted_by_priority(&self) -> Vec<&DebugMemoryViewerItem> {
        let mut sorted: Vec<_> = self.items.iter().collect();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
        sorted
    }

    pub fn clear(&mut self) { self.items.clear(); }

    pub fn is_empty(&self) -> bool { self.items.is_empty() }

    pub fn total_priority(&self) -> u64 {
        self.items.iter().map(|i| i.priority as u64).sum()
    }

    pub fn config(&self) -> &DebugMemoryViewerConfig {
        &self.config
    }

    pub fn generate_report(&self) -> String {
        format!(
            "{} | Items: {} | Auto-refresh: {}",
            self.config.label, self.items.len(), self.config.auto_refresh
        )
    }
}



// ─── Dbg Ring Buffer ──────────────────────────────────────

/// A fixed-capacity ring buffer for debug events.
#[derive(Debug, Clone)]
pub struct DbgRingBuffer<T> {
    buf: Vec<Option<T>>,
    head: usize,
    len: usize,
}

impl<T: Clone> DbgRingBuffer<T> {
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "capacity must be > 0");
        Self { buf: vec![None; capacity], head: 0, len: 0 }
    }

    pub fn push(&mut self, item: T) {
        let cap = self.buf.len();
        let idx = (self.head + self.len) % cap;
        self.buf[idx] = Some(item);
        if self.len == cap { self.head = (self.head + 1) % cap; }
        else { self.len += 1; }
    }

    pub fn len(&self) -> usize { self.len }
    pub fn is_empty(&self) -> bool { self.len == 0 }
    pub fn is_full(&self) -> bool { self.len == self.buf.len() }
    pub fn capacity(&self) -> usize { self.buf.len() }

    pub fn get(&self, index: usize) -> Option<&T> {
        if index >= self.len { return None; }
        self.buf[(self.head + index) % self.buf.len()].as_ref()
    }

    pub fn iter(&self) -> Vec<&T> {
        let cap = self.buf.len();
        (0..self.len).filter_map(|i| self.buf[(self.head + i) % cap].as_ref()).collect()
    }

    pub fn clear(&mut self) {
        for slot in &mut self.buf { *slot = None; }
        self.head = 0;
        self.len = 0;
    }

    pub fn to_vec(&self) -> Vec<T> { self.iter().into_iter().cloned().collect() }

    pub fn newest(&self) -> Option<&T> {
        if self.len == 0 { return None; }
        self.buf[(self.head + self.len - 1) % self.buf.len()].as_ref()
    }

    pub fn oldest(&self) -> Option<&T> {
        if self.len == 0 { return None; }
        self.buf[self.head].as_ref()
    }
}

impl<T: Clone + fmt::Display> fmt::Display for DbgRingBuffer<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DbgRingBuffer(len={}, cap={})", self.len, self.capacity())
    }
}

// ─── Dbg LRU Cache ───────────────────────────────────────

/// A simple LRU cache for debug breakpoints.
#[derive(Debug)]
pub struct DbgLruCache<V> {
    entries: Vec<(String, V)>,
    capacity: usize,
    hits: u64,
    misses: u64,
}

impl<V: Clone> DbgLruCache<V> {
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0);
        Self { entries: Vec::with_capacity(capacity), capacity, hits: 0, misses: 0 }
    }

    pub fn insert(&mut self, key: impl Into<String>, value: V) -> Option<(String, V)> {
        let key = key.into();
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == &key) {
            self.entries.remove(pos);
            self.entries.insert(0, (key, value));
            return None;
        }
        let evicted = if self.entries.len() >= self.capacity {
            Some(self.entries.pop().unwrap())
        } else { None };
        self.entries.insert(0, (key, value));
        evicted
    }

    pub fn get(&mut self, key: &str) -> Option<&V> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            self.hits += 1;
            let entry = self.entries.remove(pos);
            self.entries.insert(0, entry);
            Some(&self.entries[0].1)
        } else {
            self.misses += 1;
            None
        }
    }

    pub fn peek(&self, key: &str) -> Option<&V> {
        self.entries.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    pub fn remove(&mut self, key: &str) -> Option<V> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            Some(self.entries.remove(pos).1)
        } else { None }
    }

    pub fn len(&self) -> usize { self.entries.len() }
    pub fn is_empty(&self) -> bool { self.entries.is_empty() }

    pub fn hit_ratio(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 { 0.0 } else { self.hits as f64 / total as f64 }
    }

    pub fn hits(&self) -> u64 { self.hits }
    pub fn misses(&self) -> u64 { self.misses }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.hits = 0;
        self.misses = 0;
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    pub fn contains(&self, key: &str) -> bool {
        self.entries.iter().any(|(k, _)| k == key)
    }
}

impl<V: Clone + fmt::Display> fmt::Display for DbgLruCache<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DbgLruCache(size={}, cap={}, hits={}, misses={})",
            self.len(), self.capacity, self.hits, self.misses)
    }
}


// ---------------------------------------------------------------------------
// Debug adapter protocol bridge — extended utilities (yd)
// ---------------------------------------------------------------------------

/// Metric accumulator for ext_dbg operations.
#[derive(Debug, Clone)]
pub struct YdMetrics {
    samples: Vec<f64>,
    label: String,
}

impl YdMetrics {
    pub fn new(label: &str) -> Self {
        Self { samples: Vec::new(), label: label.to_string() }
    }

    pub fn record(&mut self, value: f64) {
        self.samples.push(value);
    }

    pub fn mean(&self) -> f64 {
        if self.samples.is_empty() { return 0.0; }
        self.samples.iter().sum::<f64>() / self.samples.len() as f64
    }

    pub fn max_val(&self) -> f64 {
        self.samples.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
    }

    pub fn min_val(&self) -> f64 {
        self.samples.iter().cloned().fold(f64::INFINITY, f64::min)
    }

    pub fn count(&self) -> usize {
        self.samples.len()
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn reset(&mut self) {
        self.samples.clear();
    }

    pub fn variance(&self) -> f64 {
        if self.samples.len() < 2 { return 0.0; }
        let m = self.mean();
        let sq: f64 = self.samples.iter().map(|v| (v - m).powi(2)).sum();
        sq / (self.samples.len() as f64 - 1.0)
    }

    pub fn std_dev(&self) -> f64 {
        self.variance().sqrt()
    }

    pub fn percentile(&self, p: f64) -> f64 {
        if self.samples.is_empty() { return 0.0; }
        let mut sorted = self.samples.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let idx = ((p / 100.0) * (sorted.len() as f64 - 1.0)).round() as usize;
        sorted[idx.min(sorted.len() - 1)]
    }

    pub fn sum(&self) -> f64 {
        self.samples.iter().sum()
    }

    pub fn merge(&mut self, other: &Self) {
        self.samples.extend_from_slice(&other.samples);
    }
}

/// Sliding-window rate counter for ext_dbg.
#[derive(Debug, Clone)]
pub struct YdRateWindow {
    timestamps: Vec<u64>,
    window_ms: u64,
}

impl YdRateWindow {
    pub fn new(window_ms: u64) -> Self {
        Self { timestamps: Vec::new(), window_ms }
    }

    pub fn tick(&mut self, now_ms: u64) {
        self.timestamps.push(now_ms);
        self.prune(now_ms);
    }

    fn prune(&mut self, now_ms: u64) {
        let cutoff = now_ms.saturating_sub(self.window_ms);
        self.timestamps.retain(|&t| t >= cutoff);
    }

    pub fn rate(&mut self, now_ms: u64) -> usize {
        self.prune(now_ms);
        self.timestamps.len()
    }

    pub fn clear(&mut self) {
        self.timestamps.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.timestamps.is_empty()
    }

    pub fn window_ms(&self) -> u64 {
        self.window_ms
    }
}

/// A small LRU-style cache for ext_dbg lookups.
#[derive(Debug, Clone)]
pub struct YdLruCache {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl YdLruCache {
    pub fn new(capacity: usize) -> Self {
        Self { entries: Vec::new(), capacity }
    }

    pub fn get(&mut self, key: &str) -> Option<String> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            let entry = self.entries.remove(pos);
            let val = entry.1.clone();
            self.entries.push(entry);
            Some(val)
        } else {
            None
        }
    }

    pub fn put(&mut self, key: String, value: String) {
        self.entries.retain(|(k, _)| k != &key);
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value));
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.entries.iter().any(|(k, _)| k == key)
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    pub fn remove(&mut self, key: &str) -> Option<String> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            Some(self.entries.remove(pos).1)
        } else {
            None
        }
    }
}


// ---------------------------------------------------------------------------
// xa_ extended helpers for ext_debug
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaExtDebugRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaExtDebugRingBuf {
    /// Create a new ring buffer with the given capacity.
    pub fn new(cap: usize) -> Self {
        assert!(cap > 0, "capacity must be > 0");
        Self {
            buf: vec![0.0; cap],
            cap,
            head: 0,
            len: 0,
        }
    }

    /// Push a value into the ring buffer.
    pub fn push(&mut self, v: f64) {
        let idx = (self.head + self.len) % self.cap;
        self.buf[idx] = v;
        if self.len == self.cap {
            self.head = (self.head + 1) % self.cap;
        } else {
            self.len += 1;
        }
    }

    /// Return the number of items currently stored.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Return the arithmetic mean, or `None` if empty.
    pub fn mean(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        let sum: f64 = (0..self.len)
            .map(|i| self.buf[(self.head + i) % self.cap])
            .sum();
        Some(sum / self.len as f64)
    }

    /// Return the minimum value, or `None` if empty.
    pub fn min_val(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        Some(
            (0..self.len)
                .map(|i| self.buf[(self.head + i) % self.cap])
                .fold(f64::INFINITY, f64::min),
        )
    }

    /// Return the maximum value, or `None` if empty.
    pub fn max_val(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        Some(
            (0..self.len)
                .map(|i| self.buf[(self.head + i) % self.cap])
                .fold(f64::NEG_INFINITY, f64::max),
        )
    }

    /// Drain all elements as a `Vec` in insertion order.
    pub fn drain_to_vec(&mut self) -> Vec<f64> {
        let v: Vec<f64> = (0..self.len)
            .map(|i| self.buf[(self.head + i) % self.cap])
            .collect();
        self.head = 0;
        self.len = 0;
        v
    }

    /// Iterate over elements in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = f64> + '_ {
        (0..self.len).map(move |i| self.buf[(self.head + i) % self.cap])
    }
}

/// Simple string-keyed counter map used by `xa_` utilities.
pub struct XaExtDebugCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaExtDebugCounter {
    /// Create an empty counter.
    pub fn new() -> Self {
        Self {
            counts: std::collections::HashMap::new(),
        }
    }

    /// Increment key by one.
    pub fn inc(&mut self, key: &str) {
        *self.counts.entry(key.to_owned()).or_insert(0) += 1;
    }

    /// Increment key by an arbitrary delta.
    pub fn inc_by(&mut self, key: &str, delta: u64) {
        *self.counts.entry(key.to_owned()).or_insert(0) += delta;
    }

    /// Get the current count (0 if absent).
    pub fn get(&self, key: &str) -> u64 {
        self.counts.get(key).copied().unwrap_or(0)
    }

    /// Return the total across all keys.
    pub fn total(&self) -> u64 {
        self.counts.values().sum()
    }

    /// Return the number of distinct keys.
    pub fn num_keys(&self) -> usize {
        self.counts.len()
    }

    /// Reset all counts to zero (keeps keys).
    pub fn reset(&mut self) {
        for v in self.counts.values_mut() {
            *v = 0;
        }
    }

    /// Remove all keys.
    pub fn clear(&mut self) {
        self.counts.clear();
    }
}

impl Default for XaExtDebugCounter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 54
// ---------------------------------------------------------------------------

/// Generic object pool `Xc54Pool<T>`.
pub struct Xc54Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc54Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc54PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc54Pool<T> {
    /// Create a pool with the given maximum capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            items: Vec::with_capacity(capacity),
            capacity,
            acquired: 0,
        }
    }

    /// Try to acquire an item from the pool.
    pub fn acquire(&mut self) -> Option<T> {
        if let Some(item) = self.items.pop() {
            self.acquired += 1;
            Some(item)
        } else {
            None
        }
    }

    /// Release an item back into the pool.
    pub fn release(&mut self, item: T) {
        if self.items.len() < self.capacity {
            self.items.push(item);
            if self.acquired > 0 {
                self.acquired -= 1;
            }
        }
    }

    /// Number of items currently stored in the pool.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Maximum capacity of the pool.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Number of items available for acquisition.
    pub fn available(&self) -> usize {
        self.items.len()
    }

    /// Drain all items from the pool.
    pub fn drain(&mut self) -> Vec<T> {
        self.acquired = 0;
        self.items.drain(..).collect()
    }

    /// Whether the pool is at capacity.
    pub fn is_full(&self) -> bool {
        self.items.len() >= self.capacity
    }

    /// Whether the pool is empty.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Return a statistics snapshot.
    pub fn stats(&self) -> Xc54PoolStats {
        Xc54PoolStats {
            capacity: self.capacity,
            len: self.items.len(),
            acquired: self.acquired,
            available: self.items.len(),
        }
    }

    /// Remove all items and reset counters.
    pub fn clear(&mut self) {
        self.items.clear();
        self.acquired = 0;
    }

    /// Shrink internal storage to fit current length.
    pub fn shrink_to_fit(&mut self) {
        self.items.shrink_to_fit();
    }

    /// Extend pool with an iterator of items (up to remaining capacity).
    pub fn extend_from<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        for item in iter {
            if self.items.len() >= self.capacity {
                break;
            }
            self.items.push(item);
        }
    }

    /// Retain only items matching a predicate.
    pub fn retain<F: FnMut(&T) -> bool>(&mut self, f: F) {
        self.items.retain(f);
    }
}

impl<T> Default for Xc54Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc54Scheduler`.
pub struct Xc54Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc54Scheduler {
    /// Create a scheduler with the given targets.
    pub fn new(targets: Vec<String>) -> Self {
        Self {
            targets,
            index: 0,
            dispatched: 0,
        }
    }

    /// Get the next target in round-robin order.
    pub fn next(&mut self) -> Option<&str> {
        if self.targets.is_empty() {
            return None;
        }
        let target = &self.targets[self.index % self.targets.len()];
        self.index += 1;
        self.dispatched += 1;
        Some(target)
    }

    /// Number of targets.
    pub fn len(&self) -> usize {
        self.targets.len()
    }

    /// Whether there are no targets.
    pub fn is_empty(&self) -> bool {
        self.targets.is_empty()
    }

    /// Total number of dispatches so far.
    pub fn dispatched(&self) -> usize {
        self.dispatched
    }

    /// Current index position.
    pub fn position(&self) -> usize {
        if self.targets.is_empty() {
            0
        } else {
            self.index % self.targets.len()
        }
    }

    /// Reset the scheduler to the beginning.
    pub fn reset(&mut self) {
        self.index = 0;
        self.dispatched = 0;
    }

    /// Add a target.
    pub fn add_target(&mut self, target: String) {
        self.targets.push(target);
    }

    /// Remove a target by name (first occurrence).
    pub fn remove_target(&mut self, name: &str) -> bool {
        if let Some(pos) = self.targets.iter().position(|t| t == name) {
            self.targets.remove(pos);
            if !self.targets.is_empty() {
                self.index %= self.targets.len();
            } else {
                self.index = 0;
            }
            true
        } else {
            false
        }
    }

    /// Get all targets.
    pub fn targets(&self) -> &[String] {
        &self.targets
    }
}

impl Default for Xc54Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_54 hash for the given byte slice.
pub fn xc_54_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_54 convention.
pub fn xc_54_reverse(s: &str) -> String {
    s.chars().rev().collect()
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

    // ── HitCondition tests ──

    #[test]
    fn hit_condition_parse_equal() {
        let hc = HitCondition::parse("= 5").unwrap();
        assert_eq!(hc, HitCondition::Equal(5));
        assert!(!hc.should_break(4));
        assert!(hc.should_break(5));
        assert!(!hc.should_break(6));
    }

    #[test]
    fn hit_condition_parse_bare_number() {
        let hc = HitCondition::parse("10").unwrap();
        assert_eq!(hc, HitCondition::Equal(10));
    }

    #[test]
    fn hit_condition_parse_greater_or_equal() {
        let hc = HitCondition::parse(">= 3").unwrap();
        assert_eq!(hc, HitCondition::GreaterOrEqual(3));
        assert!(!hc.should_break(2));
        assert!(hc.should_break(3));
        assert!(hc.should_break(100));
    }

    #[test]
    fn hit_condition_parse_multiple() {
        let hc = HitCondition::parse("% 4").unwrap();
        assert_eq!(hc, HitCondition::Multiple(4));
        assert!(!hc.should_break(0));
        assert!(!hc.should_break(1));
        assert!(hc.should_break(4));
        assert!(hc.should_break(8));
        assert!(!hc.should_break(5));
    }

    #[test]
    fn hit_condition_parse_modulo_zero_rejected() {
        assert!(HitCondition::parse("% 0").is_err());
    }

    #[test]
    fn hit_condition_parse_invalid() {
        assert!(HitCondition::parse("abc").is_err());
        assert!(HitCondition::parse(">= abc").is_err());
    }

    // ── StackFrame / call stack tests ──

    #[test]
    fn stack_frame_format_summary_minimal() {
        let frame = StackFrame::new(0, "main", 1, 0);
        assert_eq!(frame.format_summary(), "#0 main at <unknown>:1:0");
    }

    #[test]
    fn stack_frame_format_summary_full() {
        let frame = StackFrame::new(1, "foo", 42, 5)
            .with_source("src/main.rs")
            .with_module("myapp");
        assert_eq!(
            frame.format_summary(),
            "#1 foo [myapp] at src/main.rs:42:5"
        );
    }

    #[test]
    fn format_call_stack_multiple_frames() {
        let frames = vec![
            StackFrame::new(0, "main", 10, 0).with_source("main.rs"),
            StackFrame::new(1, "run", 20, 4).with_source("lib.rs"),
        ];
        let output = format_call_stack(&frames);
        let lines: Vec<&str> = output.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("main"));
        assert!(lines[1].contains("run"));
    }

    // ── DebugConsoleCommand tests ──

    #[test]
    fn console_command_parse_stepin() {
        assert_eq!(DebugConsoleCommand::parse("si"), DebugConsoleCommand::StepIn);
        assert_eq!(DebugConsoleCommand::parse("stepin"), DebugConsoleCommand::StepIn);
    }

    #[test]
    fn console_command_parse_continue() {
        assert_eq!(DebugConsoleCommand::parse("c"), DebugConsoleCommand::Continue);
        assert_eq!(DebugConsoleCommand::parse("continue"), DebugConsoleCommand::Continue);
    }

    #[test]
    fn console_command_parse_backtrace() {
        assert_eq!(DebugConsoleCommand::parse("bt"), DebugConsoleCommand::Backtrace);
    }

    #[test]
    fn console_command_parse_set_variable() {
        match DebugConsoleCommand::parse("set x = 42") {
            DebugConsoleCommand::SetVariable { name, value } => {
                assert_eq!(name, "x");
                assert_eq!(value, "42");
            }
            other => panic!("expected SetVariable, got {:?}", other),
        }
    }

    #[test]
    fn console_command_parse_expression() {
        match DebugConsoleCommand::parse("x + y * 2") {
            DebugConsoleCommand::Evaluate(expr) => assert_eq!(expr, "x + y * 2"),
            other => panic!("expected Evaluate, got {:?}", other),
        }
    }

    #[test]
    fn console_command_empty_is_unknown() {
        assert_eq!(
            DebugConsoleCommand::parse(""),
            DebugConsoleCommand::Unknown(String::new())
        );
    }

    // ── DebugOutputLog tests ──

    #[test]
    fn output_log_filter_by_category() {
        let mut log = DebugOutputLog::new(100);
        log.append(DebugOutputCategory::Stdout, "hello");
        log.append(DebugOutputCategory::Stderr, "error!");
        log.append(DebugOutputCategory::Stdout, "world");
        assert_eq!(log.filter_by_category(DebugOutputCategory::Stdout).len(), 2);
        assert_eq!(log.filter_by_category(DebugOutputCategory::Stderr).len(), 1);
        assert_eq!(log.filter_by_category(DebugOutputCategory::Telemetry).len(), 0);
    }

    #[test]
    fn output_log_search() {
        let mut log = DebugOutputLog::new(100);
        log.append(DebugOutputCategory::Console, "Loading module foo");
        log.append(DebugOutputCategory::Console, "Ready");
        log.append(DebugOutputCategory::Stdout, "foo output");
        let results = log.search("foo");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn output_log_max_entries_evicts_oldest() {
        let mut log = DebugOutputLog::new(3);
        log.append(DebugOutputCategory::Stdout, "a");
        log.append(DebugOutputCategory::Stdout, "b");
        log.append(DebugOutputCategory::Stdout, "c");
        log.append(DebugOutputCategory::Stdout, "d");
        assert_eq!(log.len(), 3);
        // "a" should have been evicted
        assert!(log.search("a").is_empty());
        assert_eq!(log.search("d").len(), 1);
    }

    #[test]
    fn output_log_clear() {
        let mut log = DebugOutputLog::new(100);
        log.append(DebugOutputCategory::Stdout, "x");
        log.clear();
        assert!(log.is_empty());
    }

    #[test]
    fn output_log_with_source() {
        let mut log = DebugOutputLog::new(100);
        log.append_with_source(DebugOutputCategory::Console, "msg", "adapter.js");
        let entries = log.filter_by_category(DebugOutputCategory::Console);
        assert_eq!(entries[0].source.as_deref(), Some("adapter.js"));
    }

    // ── Breakpoint condition tests ──

    #[test]
    fn breakpoint_condition_no_hit_count() {
        let cond = DebugBreakpointCondition::new("x > 5");
        assert!(cond.should_break(1));
        assert!(cond.should_break(100));
        assert_eq!(cond.evaluate_expression(), "x > 5");
    }

    #[test]
    fn breakpoint_condition_equal_hit() {
        let cond = DebugBreakpointCondition::new("true")
            .with_hit_count(HitCountOp::Equal, 3);
        assert!(!cond.should_break(1));
        assert!(!cond.should_break(2));
        assert!(cond.should_break(3));
        assert!(!cond.should_break(4));
    }

    #[test]
    fn breakpoint_condition_greater_than() {
        let cond = DebugBreakpointCondition::new("true")
            .with_hit_count(HitCountOp::GreaterThan, 2);
        assert!(!cond.should_break(1));
        assert!(!cond.should_break(2));
        assert!(cond.should_break(3));
    }

    #[test]
    fn breakpoint_condition_multiple() {
        let cond = DebugBreakpointCondition::new("true")
            .with_hit_count(HitCountOp::Multiple, 5);
        assert!(cond.should_break(5));
        assert!(cond.should_break(10));
        assert!(!cond.should_break(7));
    }

    #[test]
    fn breakpoint_condition_display() {
        let cond = DebugBreakpointCondition::new("i == 0")
            .with_hit_count(HitCountOp::GreaterEqual, 10)
            .with_log_message("loop iteration");
        let text = format!("{}", cond);
        assert!(text.contains("i == 0"));
        assert!(text.contains(">= 10"));
        assert!(text.contains("loop iteration"));
    }

    // ── Watch expression tests ──

    #[test]
    fn watch_add_update_remove() {
        let mut w = DebugWatchTree::new();
        w.add_expression("my_var");
        assert_eq!(w.all_expressions(), vec!["my_var"]);

        assert!(w.update_value("my_var", "42"));
        assert_eq!(w.get("my_var").unwrap().value.as_deref(), Some("42"));

        assert!(w.remove("my_var"));
        assert!(w.get("my_var").is_none());
    }

    #[test]
    fn watch_render_tree() {
        let mut w = DebugWatchTree::new();
        w.add_expression("obj");
        w.update_value("obj", "{...}");
        w.expressions[0].expandable = true;
        w.expressions[0].children.push(WatchEntry {
            expression: "obj.x".into(),
            value: Some("1".into()),
            children: vec![],
            expandable: false,
        });
        let tree = w.render_tree();
        assert!(tree.contains("obj = {...}"));
        assert!(tree.contains("  obj.x = 1"));
    }

    // ── Hover evaluator tests ──

    #[test]
    fn hover_cache_and_invalidate() {
        let mut h = DebugHoverEvaluator::new(3);
        h.evaluate("x", "10");
        h.evaluate("y", "20");
        assert_eq!(h.get_cached("x"), Some("10"));
        assert_eq!(h.cache_size(), 2);

        h.invalidate_all();
        assert_eq!(h.cache_size(), 0);
        assert_eq!(h.get_cached("x"), None);
    }

    #[test]
    fn hover_cache_eviction() {
        let mut h = DebugHoverEvaluator::new(2);
        h.evaluate("a", "1");
        h.evaluate("b", "2");
        h.evaluate("c", "3");
        assert_eq!(h.cache_size(), 2);
        assert_eq!(h.get_cached("c"), Some("3"));
    }

    #[test]
    fn hover_format() {
        assert_eq!(DebugHoverEvaluator::format_hover("counter", "7"), "counter = 7");
    }

    // ── Call stack tests ──

    #[test]
    fn call_stack_push_pop() {
        let mut cs = DebugCallStack::new();
        cs.push_frame(StackFrame::new(1, "main", 10, 1).with_source("main.rs"));
        cs.push_frame(StackFrame::new(2, "foo", 20, 5));
        assert_eq!(cs.depth(), 2);
        assert_eq!(cs.current_frame().unwrap().name, "foo");

        let popped = cs.pop_frame().unwrap();
        assert_eq!(popped.name, "foo");
        assert_eq!(cs.depth(), 1);
    }

    #[test]
    fn call_stack_navigation() {
        let mut cs = DebugCallStack::new();
        cs.push_frame(StackFrame::new(0, "a", 1, 1));
        cs.push_frame(StackFrame::new(1, "b", 2, 1));
        cs.push_frame(StackFrame::new(2, "c", 3, 1));

        assert_eq!(cs.navigate_up(2), Some(1));
        assert_eq!(cs.navigate_up(0), None);
        assert_eq!(cs.navigate_down(1), Some(2));
        assert_eq!(cs.navigate_down(2), None);
    }

    #[test]
    fn call_stack_render_and_display() {
        let mut cs = DebugCallStack::new();
        cs.push_frame(StackFrame::new(0, "main", 1, 1).with_source("main.rs"));
        cs.push_frame(StackFrame::new(1, "helper", 5, 3));
        let rendered = cs.render();
        assert!(rendered.contains("#1 helper"));
        assert!(rendered.contains("#0 main"));
        assert!(rendered.contains("main.rs"));

        let frame = cs.frame_at(0).unwrap();
        let display = format!("{}", frame);
        assert!(display.contains("main.rs"));
    }

#[test]
    fn debugvariableformatter_severity_ordering() {
        assert!(DebugVariableFormatterSeverity::Critical > DebugVariableFormatterSeverity::High);
        assert!(DebugVariableFormatterSeverity::High > DebugVariableFormatterSeverity::Medium);
        assert!(DebugVariableFormatterSeverity::Medium > DebugVariableFormatterSeverity::Low);
    }

    #[test]
    fn debugvariableformatter_severity_display() {
        assert_eq!(DebugVariableFormatterSeverity::Low.to_string(), "low");
        assert_eq!(DebugVariableFormatterSeverity::Critical.to_string(), "critical");
    }

    #[test]
    fn debugvariableformatter_entry_creation() {
        let e = DebugVariableFormatterEntry::new("e1", "Entry 1");
        assert_eq!(e.id, "e1");
        assert_eq!(e.severity, DebugVariableFormatterSeverity::Low);
        assert!(e.is_enabled());
    }

    #[test]
    fn debugvariableformatter_entry_builder() {
        let e = DebugVariableFormatterEntry::new("e2", "Entry 2")
            .with_severity(DebugVariableFormatterSeverity::High)
            .with_detail("some detail")
            .with_var_count(42);
        assert_eq!(e.severity, DebugVariableFormatterSeverity::High);
        assert_eq!(e.detail.as_deref(), Some("some detail"));
        assert_eq!(e.var_count, 42);
    }

    #[test]
    fn debugvariableformatter_entry_enable_disable() {
        let mut e = DebugVariableFormatterEntry::new("e3", "Entry 3");
        assert!(e.is_enabled());
        e.disable();
        assert!(!e.is_enabled());
        e.enable();
        assert!(e.is_enabled());
    }

    #[test]
    fn debugvariableformatter_add_and_count() {
        let mut mgr = DebugVariableFormatter::new("test");
        mgr.add(DebugVariableFormatterEntry::new("a", "A"));
        mgr.add(DebugVariableFormatterEntry::new("b", "B").with_severity(DebugVariableFormatterSeverity::High));
        assert_eq!(mgr.var_count(), 2);
        assert_eq!(mgr.high_severity_count(), 1);
    }

    #[test]
    fn debugvariableformatter_remove() {
        let mut mgr = DebugVariableFormatter::new("test");
        mgr.add(DebugVariableFormatterEntry::new("a", "A"));
        let removed = mgr.remove("a");
        assert!(removed.is_some());
        assert!(mgr.is_empty());
    }

    #[test]
    fn debugvariableformatter_capacity() {
        let mut mgr = DebugVariableFormatter::new("test").with_capacity(1);
        assert!(mgr.add(DebugVariableFormatterEntry::new("a", "A")));
        assert!(!mgr.add(DebugVariableFormatterEntry::new("b", "B")));
    }

    #[test]
    fn debugvariableformatter_sorted_by_severity() {
        let mut mgr = DebugVariableFormatter::new("test");
        mgr.add(DebugVariableFormatterEntry::new("lo", "Low"));
        mgr.add(DebugVariableFormatterEntry::new("hi", "High").with_severity(DebugVariableFormatterSeverity::Critical));
        let sorted = mgr.sorted_by_severity();
        assert_eq!(sorted[0].severity, DebugVariableFormatterSeverity::Critical);
    }

    #[test]
    fn debugvariableformatter_summary() {
        let mgr = DebugVariableFormatter::new("test-scope");
        let s = mgr.generate_summary();
        assert!(s.contains("test-scope"));
        assert!(s.contains("Total: 0"));
    }

    #[test]
    fn debugmemoryviewer_config_defaults() {
        let cfg = DebugMemoryViewerConfig::default();
        assert_eq!(cfg.max_items, 100);
        assert!(cfg.auto_refresh);
    }

    #[test]
    fn debugmemoryviewer_item_creation() {
        let item = DebugMemoryViewerItem::new("k1", "v1").with_priority(5).with_tag("tag1");
        assert_eq!(item.key, "k1");
        assert_eq!(item.priority, 5);
        assert!(item.has_tag("tag1"));
        assert!(!item.has_tag("tag2"));
    }

    #[test]
    fn debugmemoryviewer_add_and_get() {
        let mut mgr = DebugMemoryViewer::new(DebugMemoryViewerConfig::new("test"));
        mgr.add(DebugMemoryViewerItem::new("k1", "v1"));
        assert_eq!(mgr.memory_size(), 1);
        assert_eq!(mgr.get("k1").unwrap().value, "v1");
    }

    #[test]
    fn debugmemoryviewer_remove_item() {
        let mut mgr = DebugMemoryViewer::new(DebugMemoryViewerConfig::new("test"));
        mgr.add(DebugMemoryViewerItem::new("k1", "v1"));
        let removed = mgr.remove("k1");
        assert!(removed.is_some());
        assert!(mgr.is_empty());
    }

    #[test]
    fn debugmemoryviewer_sorted_by_priority() {
        let mut mgr = DebugMemoryViewer::new(DebugMemoryViewerConfig::new("test"));
        mgr.add(DebugMemoryViewerItem::new("lo", "low").with_priority(1));
        mgr.add(DebugMemoryViewerItem::new("hi", "high").with_priority(10));
        let sorted = mgr.sorted_by_priority();
        assert_eq!(sorted[0].key, "hi");
    }

    #[test]
    fn debugmemoryviewer_items_with_tag() {
        let mut mgr = DebugMemoryViewer::new(DebugMemoryViewerConfig::new("test"));
        mgr.add(DebugMemoryViewerItem::new("a", "1").with_tag("x"));
        mgr.add(DebugMemoryViewerItem::new("b", "2").with_tag("y"));
        assert_eq!(mgr.items_with_tag("x").len(), 1);
    }

    #[test]
    fn debugmemoryviewer_report() {
        let mgr = DebugMemoryViewer::new(DebugMemoryViewerConfig::new("my-label").with_auto_refresh(false));
        let r = mgr.generate_report();
        assert!(r.contains("my-label"));
        assert!(r.contains("false"));
    }

    #[test]
    fn dbg_ringbuf_push_get() {
        let mut rb = DbgRingBuffer::new(3);
        rb.push(10); rb.push(20); rb.push(30);
        assert_eq!(rb.get(0), Some(&10));
        assert_eq!(rb.get(2), Some(&30));
        assert_eq!(rb.len(), 3);
    }

    #[test]
    fn dbg_ringbuf_overflow() {
        let mut rb = DbgRingBuffer::<i32>::new(2);
        rb.push(1); rb.push(2); rb.push(3);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(&2));
        assert_eq!(rb.get(1), Some(&3));
    }

    #[test]
    fn dbg_ringbuf_clear() {
        let mut rb = DbgRingBuffer::new(5);
        rb.push("a".to_string()); rb.push("b".to_string());
        rb.clear();
        assert!(rb.is_empty());
    }

    #[test]
    fn dbg_ringbuf_newest_oldest() {
        let mut rb = DbgRingBuffer::new(4);
        rb.push(100); rb.push(200); rb.push(300);
        assert_eq!(rb.oldest(), Some(&100));
        assert_eq!(rb.newest(), Some(&300));
    }

    #[test]
    fn dbg_ringbuf_to_vec() {
        let mut rb = DbgRingBuffer::new(3);
        rb.push(1); rb.push(2);
        assert_eq!(rb.to_vec(), vec![1, 2]);
    }

    #[test]
    fn dbg_ringbuf_is_full() {
        let mut rb = DbgRingBuffer::new(2);
        assert!(!rb.is_full());
        rb.push(1); rb.push(2);
        assert!(rb.is_full());
    }

    #[test]
    fn dbg_lru_insert_get() {
        let mut c = DbgLruCache::new(3);
        c.insert("a", 1); c.insert("b", 2); c.insert("c", 3);
        assert_eq!(c.get("a"), Some(&1));
        assert_eq!(c.get("b"), Some(&2));
        assert_eq!(c.len(), 3);
    }

    #[test]
    fn dbg_lru_eviction() {
        let mut c = DbgLruCache::new(2);
        c.insert("a", 1); c.insert("b", 2);
        let ev = c.insert("c", 3);
        assert!(ev.is_some());
        assert_eq!(ev.unwrap().0, "a");
        assert!(!c.contains("a"));
    }

    #[test]
    fn dbg_lru_hit_ratio() {
        let mut c = DbgLruCache::new(5);
        c.insert("x", 10);
        c.get("x"); c.get("y");
        assert!(c.hit_ratio() > 0.4 && c.hit_ratio() < 0.6);
    }

    #[test]
    fn dbg_lru_clear() {
        let mut c = DbgLruCache::new(3);
        c.insert("a", 1); c.insert("b", 2);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.hits(), 0);
    }

    #[test]
    fn dbg_lru_remove() {
        let mut c = DbgLruCache::new(3);
        c.insert("a", 100);
        assert_eq!(c.remove("a"), Some(100));
        assert!(!c.contains("a"));
    }

    #[test]
    fn dbg_lru_peek() {
        let mut c = DbgLruCache::new(3);
        c.insert("x", 42);
        assert_eq!(c.peek("x"), Some(&42));
        assert_eq!(c.misses(), 0);
    }


    #[test]
    fn yd_metrics_empty() {
        let m = YdMetrics::new("ext_dbg");
        assert_eq!(m.count(), 0);
        assert!((m.mean() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn yd_metrics_record_and_mean() {
        let mut m = YdMetrics::new("ext_dbg");
        m.record(10.0);
        m.record(20.0);
        m.record(30.0);
        assert_eq!(m.count(), 3);
        assert!((m.mean() - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn yd_metrics_min_max() {
        let mut m = YdMetrics::new("test");
        m.record(5.0);
        m.record(15.0);
        m.record(10.0);
        assert!((m.min_val() - 5.0).abs() < f64::EPSILON);
        assert!((m.max_val() - 15.0).abs() < f64::EPSILON);
    }

    #[test]
    fn yd_metrics_variance_and_std() {
        let mut m = YdMetrics::new("v");
        m.record(2.0);
        m.record(4.0);
        m.record(4.0);
        m.record(4.0);
        m.record(5.0);
        m.record(5.0);
        m.record(7.0);
        m.record(9.0);
        assert!(m.variance() > 0.0);
        assert!(m.std_dev() > 0.0);
    }

    #[test]
    fn yd_metrics_percentile() {
        let mut m = YdMetrics::new("p");
        for i in 1..=100 {
            m.record(i as f64);
        }
        let p50 = m.percentile(50.0);
        assert!(p50 >= 49.0 && p50 <= 51.0);
    }

    #[test]
    fn yd_metrics_merge() {
        let mut a = YdMetrics::new("a");
        a.record(1.0);
        let mut b = YdMetrics::new("b");
        b.record(2.0);
        b.record(3.0);
        a.merge(&b);
        assert_eq!(a.count(), 3);
    }

    #[test]
    fn yd_metrics_reset() {
        let mut m = YdMetrics::new("r");
        m.record(42.0);
        m.reset();
        assert_eq!(m.count(), 0);
    }

    #[test]
    fn yd_rate_window_empty() {
        let rw = YdRateWindow::new(1000);
        assert!(rw.is_empty());
        assert_eq!(rw.window_ms(), 1000);
    }

    #[test]
    fn yd_rate_window_tick_and_rate() {
        let mut rw = YdRateWindow::new(1000);
        rw.tick(100);
        rw.tick(200);
        rw.tick(300);
        assert_eq!(rw.rate(500), 3);
        assert_eq!(rw.rate(1500), 0);
    }

    #[test]
    fn yd_lru_cache_basic() {
        let mut c = YdLruCache::new(2);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        assert_eq!(c.get("a"), Some("1".to_string()));
        c.put("c".into(), "3".into());
        assert_eq!(c.get("b"), None);
    }

    #[test]
    fn yd_lru_cache_contains_and_keys() {
        let mut c = YdLruCache::new(3);
        c.put("x".into(), "10".into());
        c.put("y".into(), "20".into());
        assert!(c.contains_key("x"));
        assert!(!c.contains_key("z"));
        assert_eq!(c.keys().len(), 2);
    }

    #[test]
    fn yd_lru_cache_remove() {
        let mut c = YdLruCache::new(3);
        c.put("k".into(), "v".into());
        assert_eq!(c.remove("k"), Some("v".to_string()));
        assert!(c.is_empty());
        assert_eq!(c.remove("k"), None);
    }

    #[test]
    fn yd_metrics_sum() {
        let mut m = YdMetrics::new("s");
        m.record(1.0);
        m.record(2.0);
        m.record(3.0);
        assert!((m.sum() - 6.0).abs() < f64::EPSILON);
    }

    #[test]
    fn yd_metrics_label() {
        let m = YdMetrics::new("my_label");
        assert_eq!(m.label(), "my_label");
    }

    #[test]
    fn yd_lru_cache_clear() {
        let mut c = YdLruCache::new(5);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // xa_ extended tests for ext_debug
    #[test]
    fn xa_ext_debug_ring_new() {
        let rb = super::XaExtDebugRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_ext_debug_ring_push_len() {
        let mut rb = super::XaExtDebugRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_ext_debug_ring_wrap() {
        let mut rb = super::XaExtDebugRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_ext_debug_ring_mean_empty() {
        let rb = super::XaExtDebugRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_ext_debug_ring_mean_values() {
        let mut rb = super::XaExtDebugRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_ext_debug_ring_min_max() {
        let mut rb = super::XaExtDebugRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_ext_debug_ring_iter() {
        let mut rb = super::XaExtDebugRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_ext_debug_counter_new() {
        let c = super::XaExtDebugCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_ext_debug_counter_inc() {
        let mut c = super::XaExtDebugCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_ext_debug_counter_inc_by() {
        let mut c = super::XaExtDebugCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_ext_debug_counter_reset() {
        let mut c = super::XaExtDebugCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_ext_debug_counter_clear() {
        let mut c = super::XaExtDebugCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_ext_debug_counter_default() {
        let c = super::XaExtDebugCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }


    // ---- xc_ pool / scheduler tests – block 54 ----

    #[test]
    fn xc_54_pool_new_empty() {
        let pool: super::Xc54Pool<i32> = super::Xc54Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_54_pool_release_acquire() {
        let mut pool = super::Xc54Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_54_pool_acquire_empty() {
        let mut pool: super::Xc54Pool<i32> = super::Xc54Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_54_pool_full() {
        let mut pool = super::Xc54Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_54_pool_drain() {
        let mut pool = super::Xc54Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_54_pool_stats() {
        let mut pool = super::Xc54Pool::new(8);
        pool.release(1);
        pool.release(2);
        let _ = pool.acquire();
        let s = pool.stats();
        assert_eq!(s.capacity, 8);
        assert_eq!(s.len, 1);
        assert_eq!(s.acquired, 1);
        assert_eq!(s.available, 1);
    }

    #[test]
    fn xc_54_pool_clear() {
        let mut pool = super::Xc54Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_54_pool_shrink() {
        let mut pool = super::Xc54Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_54_pool_default() {
        let pool: super::Xc54Pool<String> = super::Xc54Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_54_pool_extend() {
        let mut pool = super::Xc54Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_54_pool_retain() {
        let mut pool = super::Xc54Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_54_scheduler_round_robin() {
        let mut sched = super::Xc54Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_54_scheduler_empty() {
        let mut sched = super::Xc54Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_54_scheduler_reset() {
        let mut sched = super::Xc54Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_54_scheduler_add_remove() {
        let mut sched = super::Xc54Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_54_scheduler_targets() {
        let sched = super::Xc54Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_54_hash_empty() {
        assert_eq!(super::xc_54_hash(b""), 5381);
    }

    #[test]
    fn xc_54_hash_data() {
        let h = super::xc_54_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_54_hash(b"hello"), h);
    }

    #[test]
    fn xc_54_reverse_str() {
        assert_eq!(super::xc_54_reverse("abc"), "cba");
        assert_eq!(super::xc_54_reverse(""), "");
    }

}