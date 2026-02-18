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


// --- xd_72 deepening: state machine + event bus ---

/// States for the Xd72 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd72State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd72State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Idle => write!(f, "Idle"),
            Self::Running => write!(f, "Running"),
            Self::Paused => write!(f, "Paused"),
            Self::Done => write!(f, "Done"),
        }
    }
}

/// Transition record for history tracking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xd72Transition {
    pub from: Xd72State,
    pub to: Xd72State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd72StateMachine {
    current: Xd72State,
    history: Vec<Xd72Transition>,
    step_counter: usize,
}

impl Xd72StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd72State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd72State {
        self.current
    }

    pub fn history(&self) -> &[Xd72Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd72State) -> Result<Xd72State, String> {
        let allowed = match (self.current, target) {
            (Xd72State::Idle, Xd72State::Running) => true,
            (Xd72State::Running, Xd72State::Paused) => true,
            (Xd72State::Running, Xd72State::Done) => true,
            (Xd72State::Paused, Xd72State::Running) => true,
            (Xd72State::Paused, Xd72State::Done) => true,
            (Xd72State::Done, Xd72State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_72: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd72Transition {
            from: self.current,
            to: target,
            step: self.step_counter,
        };
        self.step_counter += 1;
        self.current = target;
        self.history.push(t);
        Ok(self.current)
    }

    /// Serialize state machine to a simple string representation.
    pub fn serialize(&self) -> String {
        let hist: Vec<String> = self
            .history
            .iter()
            .map(|t| format!("{}->{}@{}", t.from, t.to, t.step))
            .collect();
        format!(
            "Xd72SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd72State> {
        let prefix = "Xd72SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd72State::Idle),
            "Running" => Some(Xd72State::Running),
            "Paused" => Some(Xd72State::Paused),
            "Done" => Some(Xd72State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd72State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd72 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd72Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd72Event {
    pub fn kind(&self) -> &str {
        match self {
            Self::Started(_) => "started",
            Self::Stopped(_) => "stopped",
            Self::Error(_) => "error",
            Self::Custom(k, _) => k.as_str(),
        }
    }

    pub fn payload(&self) -> &str {
        match self {
            Self::Started(p) | Self::Stopped(p) | Self::Error(p) => p.as_str(),
            Self::Custom(_, p) => p.as_str(),
        }
    }
}

type Xd72HandlerFn = Box<dyn Fn(&Xd72Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd72EventBus {
    handlers: Vec<(usize, Option<String>, Xd72HandlerFn)>,
    next_id: usize,
    published: Vec<Xd72Event>,
}

impl Xd72EventBus {
    pub fn new() -> Self {
        Self {
            handlers: Vec::new(),
            next_id: 0,
            published: Vec::new(),
        }
    }

    /// Subscribe to all events. Returns a subscription id.
    pub fn subscribe<F>(&mut self, handler: F) -> usize
    where
        F: Fn(&Xd72Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd72Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers
            .push((id, Some(kind_filter.to_string()), Box::new(handler)));
        id
    }

    /// Unsubscribe by subscription id.
    pub fn unsubscribe(&mut self, sub_id: usize) -> bool {
        let before = self.handlers.len();
        self.handlers.retain(|(id, _, _)| *id != sub_id);
        self.handlers.len() < before
    }

    /// Publish an event to all matching subscribers.
    pub fn publish(&mut self, event: Xd72Event) {
        for (_, filter, handler) in &self.handlers {
            let matched = match filter {
                None => true,
                Some(f) => event.kind() == f.as_str(),
            };
            if matched {
                handler(&event);
            }
        }
        self.published.push(event);
    }

    pub fn published_events(&self) -> &[Xd72Event] {
        &self.published
    }

    pub fn subscriber_count(&self) -> usize {
        self.handlers.len()
    }

    pub fn clear_history(&mut self) {
        self.published.clear();
    }
}


// ---------------------------------------------------------------------------
// xf_ data structures (Trie + BloomFilter) — unique instance #87
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf87Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf87TrieNode {
    children: std::collections::HashMap<char, Xf87TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf87Trie {
    root: Xf87TrieNode,
    count: usize,
}

impl Xf87Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf87TrieNode::default(), count: 0 }
    }

    /// Insert a word into the trie.
    pub fn xf_insert(&mut self, word: &str) {
        let mut node = &mut self.root;
        for ch in word.chars() {
            node = node.children.entry(ch).or_default();
        }
        if !node.is_end {
            node.is_end = true;
            self.count += 1;
        }
    }

    /// Return `true` if the exact word exists in the trie.
    pub fn xf_search(&self, word: &str) -> bool {
        let mut node = &self.root;
        for ch in word.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return false,
            }
        }
        node.is_end
    }

    /// Return `true` if any word in the trie starts with `prefix`.
    pub fn xf_starts_with(&self, prefix: &str) -> bool {
        let mut node = &self.root;
        for ch in prefix.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return false,
            }
        }
        true
    }

    /// Remove a word. Returns `true` if it was present.
    pub fn xf_remove(&mut self, word: &str) -> bool {
        if Self::xf_remove_recursive(&mut self.root, word, 0) {
            self.count -= 1;
            true
        } else {
            false
        }
    }

    fn xf_remove_recursive(node: &mut Xf87TrieNode, word: &str, depth: usize) -> bool {
        let chars: Vec<char> = word.chars().collect();
        if depth == chars.len() {
            if !node.is_end {
                return false;
            }
            node.is_end = false;
            return node.children.is_empty();
        }
        let ch = chars[depth];
        let should_delete = {
            if let Some(child) = node.children.get_mut(&ch) {
                Self::xf_remove_recursive(child, word, depth + 1)
            } else {
                return false;
            }
        };
        if should_delete {
            node.children.remove(&ch);
            return !node.is_end && node.children.is_empty();
        }
        false
    }

    /// Number of distinct words stored.
    pub fn xf_word_count(&self) -> usize {
        self.count
    }

    /// Return the longest prefix of `query` that exists as a word in the trie.
    pub fn xf_longest_prefix(&self, query: &str) -> Option<String> {
        let mut node = &self.root;
        let mut last_match: Option<usize> = None;
        for (i, ch) in query.chars().enumerate() {
            match node.children.get(&ch) {
                Some(n) => {
                    node = n;
                    if node.is_end {
                        last_match = Some(i + 1);
                    }
                }
                None => break,
            }
        }
        last_match.map(|end| query.chars().take(end).collect())
    }

    /// Collect every word in the trie.
    pub fn xf_all_words(&self) -> Vec<String> {
        let mut results = Vec::new();
        let mut buffer = String::new();
        Self::xf_collect(&self.root, &mut buffer, &mut results);
        results
    }

    fn xf_collect(node: &Xf87TrieNode, buf: &mut String, out: &mut Vec<String>) {
        if node.is_end {
            out.push(buf.clone());
        }
        let mut keys: Vec<char> = node.children.keys().copied().collect();
        keys.sort();
        for ch in keys {
            buf.push(ch);
            Self::xf_collect(&node.children[&ch], buf, out);
            buf.pop();
        }
    }

    /// Return all words that start with the given prefix.
    pub fn xf_autocomplete(&self, prefix: &str) -> Vec<String> {
        let mut node = &self.root;
        for ch in prefix.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return Vec::new(),
            }
        }
        let mut results = Vec::new();
        let mut buf = prefix.to_string();
        Self::xf_collect(node, &mut buf, &mut results);
        results
    }
}

// ---------------------------------------------------------------------------

/// Simple Bloom filter using two hash functions.
#[derive(Debug, Clone)]
pub struct Xf87BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf87BloomFilter {
    /// Create a Bloom filter with `size` bits and `num_hashes` hash functions.
    pub fn xf_new(size: usize, num_hashes: usize) -> Self {
        Self { bits: vec![false; size], num_hashes, len: size, item_count: 0 }
    }

    fn xf_hashes(&self, item: &str) -> Vec<usize> {
        let mut h1: u64 = 0;
        let mut h2: u64 = 0;
        for (i, b) in item.bytes().enumerate() {
            h1 = h1.wrapping_mul(31).wrapping_add(b as u64);
            h2 = h2.wrapping_mul(37).wrapping_add((b as u64).wrapping_add(i as u64));
        }
        (0..self.num_hashes)
            .map(|i| (h1.wrapping_add((i as u64).wrapping_mul(h2))) as usize % self.len)
            .collect()
    }

    /// Add an item to the filter.
    pub fn xf_add(&mut self, item: &str) {
        for idx in self.xf_hashes(item) {
            self.bits[idx] = true;
        }
        self.item_count += 1;
    }

    /// Check if an item might be in the filter.
    pub fn xf_might_contain(&self, item: &str) -> bool {
        self.xf_hashes(item).iter().all(|&idx| self.bits[idx])
    }

    /// Estimated false-positive rate.
    pub fn xf_false_positive_rate(&self) -> f64 {
        let set_bits = self.bits.iter().filter(|&&b| b).count() as f64;
        let ratio = set_bits / self.len as f64;
        ratio.powi(self.num_hashes as i32)
    }

    /// Clear all bits.
    pub fn xf_clear(&mut self) {
        for b in self.bits.iter_mut() {
            *b = false;
        }
        self.item_count = 0;
    }

    /// Bitwise OR union of two filters (must be same size).
    pub fn xf_union(&self, other: &Self) -> Option<Self> {
        if self.len != other.len || self.num_hashes != other.num_hashes {
            return None;
        }
        let bits = self.bits.iter().zip(&other.bits).map(|(&a, &b)| a || b).collect();
        Some(Self { bits, num_hashes: self.num_hashes, len: self.len, item_count: self.item_count + other.item_count })
    }

    /// Estimate intersection size using inclusion-exclusion on bit counts.
    pub fn xf_intersection_estimate(&self, other: &Self) -> f64 {
        if self.len != other.len {
            return 0.0;
        }
        let both = self.bits.iter().zip(&other.bits).filter(|(a, b)| **a && **b).count();
        both as f64
    }
}


/// A probabilistic sorted list using a skip-list structure (variant 53).
pub struct Xh53SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh53SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 95 as u64,
        }
    }

    fn xh_random_level(&mut self) -> usize {
        self.xh_seed ^= self.xh_seed << 13;
        self.xh_seed ^= self.xh_seed >> 7;
        self.xh_seed ^= self.xh_seed << 17;
        let mut lvl = 1;
        while lvl < self.xh_max_level && (self.xh_seed & 1) == 0 {
            lvl += 1;
            self.xh_seed ^= self.xh_seed.wrapping_mul(6364136223846793005);
        }
        lvl
    }

    /// Insert a value into the skip list.
    pub fn xh_insert(&mut self, value: i64) {
        let pos = self.xh_data.len();
        self.xh_data.push(value);
        let lvl = self.xh_random_level();
        for i in 0..lvl {
            self.xh_levels[i].push((value, pos));
            self.xh_levels[i].sort_by_key(|&(v, _)| v);
        }
        self.xh_len += 1;
    }

    /// Check whether the skip list contains the given value.
    pub fn xh_contains(&self, value: i64) -> bool {
        if self.xh_levels.is_empty() {
            return false;
        }
        self.xh_levels[0].binary_search_by_key(&value, |&(v, _)| v).is_ok()
    }

    /// Remove one occurrence of `value`. Returns `true` if found.
    pub fn xh_remove(&mut self, value: i64) -> bool {
        let mut found = false;
        for level in &mut self.xh_levels {
            if let Ok(idx) = level.binary_search_by_key(&value, |&(v, _)| v) {
                level.remove(idx);
                found = true;
            }
        }
        if found {
            self.xh_len -= 1;
        }
        found
    }

    /// Return the number of elements.
    pub fn xh_len(&self) -> usize {
        self.xh_len
    }

    /// Collect values in `[lo, hi]` inclusive.
    pub fn xh_range_query(&self, lo: i64, hi: i64) -> Vec<i64> {
        if self.xh_levels.is_empty() {
            return Vec::new();
        }
        self.xh_levels[0]
            .iter()
            .filter(|&&(v, _)| v >= lo && v <= hi)
            .map(|&(v, _)| v)
            .collect()
    }

    /// Greatest value <= `value`, if any.
    pub fn xh_floor(&self, value: i64) -> Option<i64> {
        if self.xh_levels.is_empty() {
            return None;
        }
        self.xh_levels[0]
            .iter()
            .rev()
            .find(|&&(v, _)| v <= value)
            .map(|&(v, _)| v)
    }

    /// Smallest value >= `value`, if any.
    pub fn xh_ceiling(&self, value: i64) -> Option<i64> {
        if self.xh_levels.is_empty() {
            return None;
        }
        self.xh_levels[0]
            .iter()
            .find(|&&(v, _)| v >= value)
            .map(|&(v, _)| v)
    }

    /// Number of elements strictly less than `value`.
    pub fn xh_rank(&self, value: i64) -> usize {
        if self.xh_levels.is_empty() {
            return 0;
        }
        self.xh_levels[0]
            .iter()
            .take_while(|&&(v, _)| v < value)
            .count()
    }
}

/// A compact bit set supporting boolean operations (variant 53).
pub struct Xh53BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh53BitSet {
    /// Create a bit set that can hold `nbits` bits.
    pub fn xh_new(nbits: usize) -> Self {
        let nwords = (nbits + 63) / 64;
        Self {
            xh_words: vec![0u64; nwords],
            xh_nbits: nbits,
        }
    }

    /// Set bit at `index`.
    pub fn xh_set(&mut self, index: usize) {
        if index < self.xh_nbits {
            self.xh_words[index / 64] |= 1u64 << (index % 64);
        }
    }

    /// Clear bit at `index`.
    pub fn xh_clear(&mut self, index: usize) {
        if index < self.xh_nbits {
            self.xh_words[index / 64] &= !(1u64 << (index % 64));
        }
    }

    /// Test whether bit at `index` is set.
    pub fn xh_test(&self, index: usize) -> bool {
        if index >= self.xh_nbits {
            return false;
        }
        (self.xh_words[index / 64] >> (index % 64)) & 1 == 1
    }

    /// Count the number of set bits.
    pub fn xh_count(&self) -> usize {
        self.xh_words.iter().map(|w| w.count_ones() as usize).sum()
    }

    /// Bitwise AND with another bit set, returning a new one.
    pub fn xh_and(&self, other: &Self) -> Self {
        let len = self.xh_words.len().min(other.xh_words.len());
        let mut result = Self::xh_new(self.xh_nbits.min(other.xh_nbits));
        for i in 0..len {
            result.xh_words[i] = self.xh_words[i] & other.xh_words[i];
        }
        result
    }

    /// Bitwise OR with another bit set, returning a new one.
    pub fn xh_or(&self, other: &Self) -> Self {
        let len = self.xh_words.len().max(other.xh_words.len());
        let mut result = Self::xh_new(self.xh_nbits.max(other.xh_nbits));
        for i in 0..len {
            let a = if i < self.xh_words.len() { self.xh_words[i] } else { 0 };
            let b = if i < other.xh_words.len() { other.xh_words[i] } else { 0 };
            result.xh_words[i] = a | b;
        }
        result
    }

    /// Bitwise XOR with another bit set, returning a new one.
    pub fn xh_xor(&self, other: &Self) -> Self {
        let len = self.xh_words.len().max(other.xh_words.len());
        let mut result = Self::xh_new(self.xh_nbits.max(other.xh_nbits));
        for i in 0..len {
            let a = if i < self.xh_words.len() { self.xh_words[i] } else { 0 };
            let b = if i < other.xh_words.len() { other.xh_words[i] } else { 0 };
            result.xh_words[i] = a ^ b;
        }
        result
    }

    /// Iterate over the indices of all set bits.
    pub fn xh_iter_ones(&self) -> Vec<usize> {
        let mut result = Vec::new();
        for (wi, &word) in self.xh_words.iter().enumerate() {
            let mut w = word;
            while w != 0 {
                let bit = w.trailing_zeros() as usize;
                result.push(wi * 64 + bit);
                w &= w - 1;
            }
        }
        result
    }

    /// Index of the first set bit, if any.
    pub fn xh_first_set(&self) -> Option<usize> {
        for (wi, &word) in self.xh_words.iter().enumerate() {
            if word != 0 {
                return Some(wi * 64 + word.trailing_zeros() as usize);
            }
        }
        None
    }

    /// Index of the last set bit, if any.
    pub fn xh_last_set(&self) -> Option<usize> {
        for (wi, &word) in self.xh_words.iter().enumerate().rev() {
            if word != 0 {
                return Some(wi * 64 + (63 - word.leading_zeros() as usize));
            }
        }
        None
    }
}


/// A double-ended queue backed by a ring buffer (variant 53).
pub struct Xi53Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi53Deque<T> {
    /// Create a new deque with the given capacity.
    pub fn xi_new(capacity: usize) -> Self {
        let cap = capacity.max(4);
        Self {
            xi_buf: (0..cap).map(|_| None).collect(),
            xi_head: 0,
            xi_tail: 0,
            xi_len: 0,
        }
    }

    /// Return the number of elements.
    pub fn xi_len(&self) -> usize {
        self.xi_len
    }

    /// Return the capacity.
    pub fn xi_capacity(&self) -> usize {
        self.xi_buf.len()
    }

    /// Return true if empty.
    pub fn xi_is_empty(&self) -> bool {
        self.xi_len == 0
    }

    fn xi_grow(&mut self) {
        let old_cap = self.xi_buf.len();
        let new_cap = old_cap * 2;
        let mut new_buf: Vec<Option<T>> = (0..new_cap).map(|_| None).collect();
        for i in 0..self.xi_len {
            let idx = (self.xi_head + i) % old_cap;
            new_buf[i] = self.xi_buf[idx].take();
        }
        self.xi_buf = new_buf;
        self.xi_head = 0;
        self.xi_tail = self.xi_len;
    }

    /// Push an element to the back.
    pub fn xi_push_back(&mut self, val: T) {
        if self.xi_len == self.xi_buf.len() {
            self.xi_grow();
        }
        self.xi_buf[self.xi_tail] = Some(val);
        self.xi_tail = (self.xi_tail + 1) % self.xi_buf.len();
        self.xi_len += 1;
    }

    /// Push an element to the front.
    pub fn xi_push_front(&mut self, val: T) {
        if self.xi_len == self.xi_buf.len() {
            self.xi_grow();
        }
        self.xi_head = if self.xi_head == 0 {
            self.xi_buf.len() - 1
        } else {
            self.xi_head - 1
        };
        self.xi_buf[self.xi_head] = Some(val);
        self.xi_len += 1;
    }

    /// Pop an element from the back.
    pub fn xi_pop_back(&mut self) -> Option<T> {
        if self.xi_len == 0 {
            return None;
        }
        self.xi_tail = if self.xi_tail == 0 {
            self.xi_buf.len() - 1
        } else {
            self.xi_tail - 1
        };
        self.xi_len -= 1;
        self.xi_buf[self.xi_tail].take()
    }

    /// Pop an element from the front.
    pub fn xi_pop_front(&mut self) -> Option<T> {
        if self.xi_len == 0 {
            return None;
        }
        let val = self.xi_buf[self.xi_head].take();
        self.xi_head = (self.xi_head + 1) % self.xi_buf.len();
        self.xi_len -= 1;
        val
    }

    /// Get element at index.
    pub fn xi_get(&self, index: usize) -> Option<&T> {
        if index >= self.xi_len {
            return None;
        }
        let real = (self.xi_head + index) % self.xi_buf.len();
        self.xi_buf[real].as_ref()
    }

    /// Rotate elements left by k positions.
    pub fn xi_rotate_left(&mut self, k: usize) {
        if self.xi_len <= 1 {
            return;
        }
        let k = k % self.xi_len;
        for _ in 0..k {
            if let Some(v) = self.xi_pop_front() {
                self.xi_push_back(v);
            }
        }
    }

    /// Rotate elements right by k positions.
    pub fn xi_rotate_right(&mut self, k: usize) {
        if self.xi_len <= 1 {
            return;
        }
        let k = k % self.xi_len;
        for _ in 0..k {
            if let Some(v) = self.xi_pop_back() {
                self.xi_push_front(v);
            }
        }
    }

    /// Collect elements into a vector.
    pub fn xi_iter(&self) -> Vec<T> {
        let mut out = Vec::with_capacity(self.xi_len);
        for i in 0..self.xi_len {
            let idx = (self.xi_head + i) % self.xi_buf.len();
            if let Some(ref v) = self.xi_buf[idx] {
                out.push(v.clone());
            }
        }
        out
    }

    /// Split at index, returning (left, right) vectors.
    pub fn xi_split_at(&self, mid: usize) -> (Vec<T>, Vec<T>) {
        let all = self.xi_iter();
        let mid = mid.min(all.len());
        let left = all[..mid].to_vec();
        let right = all[mid..].to_vec();
        (left, right)
    }
}

/// An interval represented as [low, high).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xi53Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi53Interval {
    /// Create a new interval.
    pub fn xi_new(low: i64, high: i64) -> Self {
        Self { xi_low: low, xi_high: high }
    }

    /// Check whether this interval overlaps with another.
    pub fn xi_overlaps(&self, other: &Self) -> bool {
        self.xi_low < other.xi_high && other.xi_low < self.xi_high
    }

    /// Check whether this interval contains a point.
    pub fn xi_contains_point(&self, p: i64) -> bool {
        p >= self.xi_low && p < self.xi_high
    }
}

/// A simple interval tree (variant 53).
pub struct Xi53IntervalTree {
    xi_intervals: Vec<Xi53Interval>,
}

impl Xi53IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi53Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi53Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi53Interval) -> Vec<&Xi53Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_overlaps(query)).collect()
    }

    /// Remove the first interval matching [low, high).
    pub fn xi_remove(&mut self, low: i64, high: i64) -> bool {
        if let Some(pos) = self.xi_intervals.iter().position(|iv| iv.xi_low == low && iv.xi_high == high) {
            self.xi_intervals.remove(pos);
            true
        } else {
            false
        }
    }

    /// Return all intervals.
    pub fn xi_all_intervals(&self) -> &[Xi53Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi53Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi53Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi53Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi53Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi53Interval> = Vec::new();
        for iv in &self.xi_intervals {
            if let Some(last) = merged.last_mut() {
                if iv.xi_low <= last.xi_high {
                    last.xi_high = last.xi_high.max(iv.xi_high);
                } else {
                    merged.push(iv.clone());
                }
            } else {
                merged.push(iv.clone());
            }
        }
        merged
    }
}


// --- xj_ Union-Find and B-Tree (crate index 53) ---

/// Disjoint set / union-find for crate 53.
pub struct Xj53UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj53UnionFind {
    /// Create an empty union-find.
    pub fn xj_new() -> Self {
        Self { parent: Vec::new(), rank: Vec::new(), size: Vec::new(), count: 0 }
    }

    /// Add a new singleton set and return its id.
    pub fn xj_make_set(&mut self) -> usize {
        let id = self.parent.len();
        self.parent.push(id);
        self.rank.push(0);
        self.size.push(1);
        self.count += 1;
        id
    }

    /// Find representative with path compression.
    pub fn xj_find(&mut self, mut x: usize) -> usize {
        while self.parent[x] != x {
            self.parent[x] = self.parent[self.parent[x]];
            x = self.parent[x];
        }
        x
    }

    /// Union two sets by rank. Returns true if they were separate.
    pub fn xj_union(&mut self, a: usize, b: usize) -> bool {
        let ra = self.xj_find(a);
        let rb = self.xj_find(b);
        if ra == rb { return false; }
        let (small, big) = if self.rank[ra] < self.rank[rb] { (ra, rb) } else { (rb, ra) };
        self.parent[small] = big;
        self.size[big] += self.size[small];
        if self.rank[big] == self.rank[small] { self.rank[big] += 1; }
        self.count -= 1;
        true
    }

    /// Check whether a and b are in the same component.
    pub fn xj_connected(&mut self, a: usize, b: usize) -> bool {
        self.xj_find(a) == self.xj_find(b)
    }

    /// Number of disjoint components.
    pub fn xj_component_count(&self) -> usize {
        self.count
    }

    /// Size of the component containing x.
    pub fn xj_component_size(&mut self, x: usize) -> usize {
        let r = self.xj_find(x);
        self.size[r]
    }

    /// Size of the largest component (0 if empty).
    pub fn xj_largest_component(&self) -> usize {
        self.size.iter().enumerate()
            .filter(|(i, _)| self.parent[*i] == *i)
            .map(|(_, s)| *s)
            .max()
            .unwrap_or(0)
    }
}

const XJ53_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 53.
pub struct Xj53BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj53BTreeNode<K, V>>>,
    len: usize,
}

struct Xj53BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj53BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj53BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ53_BTREE_ORDER - 1
    }

    fn xj_search(&self, key: &K) -> Option<&V> {
        let mut idx = 0;
        while idx < self.keys.len() && *key > self.keys[idx] { idx += 1; }
        if idx < self.keys.len() && self.keys[idx] == *key {
            return Some(&self.values[idx]);
        }
        if self.xj_is_leaf() { return None; }
        self.children[idx].xj_search(key)
    }

    fn xj_split_child(&mut self, i: usize) {
        let mid = XJ53_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj53BTreeNode::xj_new_leaf();
        new_node.keys = child.keys.split_off(mid + 1);
        new_node.values = child.values.split_off(mid + 1);
        if !child.xj_is_leaf() {
            new_node.children = child.children.split_off(mid + 1);
        }
        let up_key = child.keys.pop().unwrap();
        let up_val = child.values.pop().unwrap();
        self.keys.insert(i, up_key);
        self.values.insert(i, up_val);
        self.children.insert(i + 1, Box::new(new_node));
    }

    fn xj_insert_non_full(&mut self, key: K, value: V) -> Option<V> {
        let mut idx = self.keys.len();
        while idx > 0 && key < self.keys[idx - 1] { idx -= 1; }
        if idx < self.keys.len() && self.keys[idx] == key {
            let old = std::mem::replace(&mut self.values[idx], value);
            return Some(old);
        }
        if self.xj_is_leaf() {
            self.keys.insert(idx, key);
            self.values.insert(idx, value);
            return None;
        }
        if self.children[idx].xj_is_full() {
            self.xj_split_child(idx);
            if key > self.keys[idx] { idx += 1; }
            else if key == self.keys[idx] {
                let old = std::mem::replace(&mut self.values[idx], value);
                return Some(old);
            }
        }
        self.children[idx].xj_insert_non_full(key, value)
    }

    fn xj_collect_keys(&self, out: &mut Vec<K>) {
        for i in 0..self.keys.len() {
            if !self.xj_is_leaf() { self.children[i].xj_collect_keys(out); }
            out.push(self.keys[i].clone());
        }
        if !self.xj_is_leaf() { self.children[self.keys.len()].xj_collect_keys(out); }
    }

    fn xj_collect_values(&self, out: &mut Vec<V>) {
        for i in 0..self.keys.len() {
            if !self.xj_is_leaf() { self.children[i].xj_collect_values(out); }
            out.push(self.values[i].clone());
        }
        if !self.xj_is_leaf() { self.children[self.keys.len()].xj_collect_values(out); }
    }

    fn xj_collect_range(&self, lo: &K, hi: &K, out: &mut Vec<(K, V)>) {
        let mut i = 0;
        while i < self.keys.len() {
            if !self.xj_is_leaf() && self.keys[i] >= *lo {
                self.children[i].xj_collect_range(lo, hi, out);
            }
            if self.keys[i] >= *lo && self.keys[i] <= *hi {
                out.push((self.keys[i].clone(), self.values[i].clone()));
            }
            i += 1;
        }
        if !self.xj_is_leaf() && (i == 0 || self.keys[i - 1] <= *hi) {
            self.children[i].xj_collect_range(lo, hi, out);
        }
    }

    fn xj_min_key(&self) -> Option<&K> {
        if self.xj_is_leaf() { self.keys.first() }
        else { self.children[0].xj_min_key().or(self.keys.first()) }
    }

    fn xj_max_key(&self) -> Option<&K> {
        if self.xj_is_leaf() { self.keys.last() }
        else { self.children.last().unwrap().xj_max_key().or(self.keys.last()) }
    }

    fn xj_remove(&mut self, key: &K) -> Option<V> {
        let mut idx = 0;
        while idx < self.keys.len() && *key > self.keys[idx] { idx += 1; }
        if idx < self.keys.len() && self.keys[idx] == *key {
            if self.xj_is_leaf() {
                self.keys.remove(idx);
                return Some(self.values.remove(idx));
            }
            let pred_val = self.children[idx].xj_remove_max();
            let old_val = std::mem::replace(&mut self.values[idx], pred_val.1);
            self.keys[idx] = pred_val.0;
            return Some(old_val);
        }
        if self.xj_is_leaf() { return None; }
        self.children.get_mut(idx).and_then(|c| c.xj_remove(key))
    }

    fn xj_remove_max(&mut self) -> (K, V) {
        if self.xj_is_leaf() {
            let k = self.keys.pop().unwrap();
            let v = self.values.pop().unwrap();
            (k, v)
        } else {
            self.children.last_mut().unwrap().xj_remove_max()
        }
    }
}

impl<K: Ord + Clone, V: Clone> Xj53BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj53BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj53BTreeNode::xj_new_leaf();
            new_root.children.push(self.root.take().unwrap());
            new_root.xj_split_child(0);
            let old = new_root.xj_insert_non_full(key, value);
            self.root = Some(Box::new(new_root));
            if old.is_none() { self.len += 1; }
            old
        } else {
            let old = root.xj_insert_non_full(key, value);
            if old.is_none() { self.len += 1; }
            old
        }
    }

    /// Get a reference to the value for the given key.
    pub fn xj_get(&self, key: &K) -> Option<&V> {
        self.root.as_ref().and_then(|r| r.xj_search(key))
    }

    /// Remove a key and return its value.
    pub fn xj_remove(&mut self, key: &K) -> Option<V> {
        let result = self.root.as_mut().and_then(|r| r.xj_remove(key));
        if result.is_some() { self.len -= 1; }
        result
    }

    /// Check if a key is present.
    pub fn xj_contains_key(&self, key: &K) -> bool {
        self.xj_get(key).is_some()
    }

    /// Number of entries.
    pub fn xj_len(&self) -> usize {
        self.len
    }

    /// Collect all keys in sorted order.
    pub fn xj_keys(&self) -> Vec<K> {
        let mut out = Vec::new();
        if let Some(r) = &self.root { r.xj_collect_keys(&mut out); }
        out
    }

    /// Collect all values in key-sorted order.
    pub fn xj_values(&self) -> Vec<V> {
        let mut out = Vec::new();
        if let Some(r) = &self.root { r.xj_collect_values(&mut out); }
        out
    }

    /// Collect entries in [lo, hi] range.
    pub fn xj_range(&self, lo: &K, hi: &K) -> Vec<(K, V)> {
        let mut out = Vec::new();
        if let Some(r) = &self.root { r.xj_collect_range(lo, hi, &mut out); }
        out
    }

    /// Smallest key, if any.
    pub fn xj_min_key(&self) -> Option<&K> {
        self.root.as_ref().and_then(|r| r.xj_min_key())
    }

    /// Largest key, if any.
    pub fn xj_max_key(&self) -> Option<&K> {
        self.root.as_ref().and_then(|r| r.xj_max_key())
    }
}


// --- xk_53 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk53SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk53SegmentTree {
    /// Build a segment tree from the given slice.
    pub fn xk_build(data: &[i64]) -> Self {
        let n = data.len();
        let tree = vec![0i64; 4 * n.max(1)];
        let min_tree = vec![i64::MAX; 4 * n.max(1)];
        let max_tree = vec![i64::MIN; 4 * n.max(1)];
        let mut st = Self { xk_n: n, xk_tree: tree, xk_min_tree: min_tree, xk_max_tree: max_tree };
        if n > 0 {
            st.xk_build_rec(data, 1, 0, n - 1);
        }
        st
    }

    fn xk_build_rec(&mut self, data: &[i64], node: usize, start: usize, end: usize) {
        if start == end {
            self.xk_tree[node] = data[start];
            self.xk_min_tree[node] = data[start];
            self.xk_max_tree[node] = data[start];
        } else {
            let mid = (start + end) / 2;
            self.xk_build_rec(data, 2 * node, start, mid);
            self.xk_build_rec(data, 2 * node + 1, mid + 1, end);
            self.xk_tree[node] = self.xk_tree[2 * node] + self.xk_tree[2 * node + 1];
            self.xk_min_tree[node] = self.xk_min_tree[2 * node].min(self.xk_min_tree[2 * node + 1]);
            self.xk_max_tree[node] = self.xk_max_tree[2 * node].max(self.xk_max_tree[2 * node + 1]);
        }
    }

    /// Query the sum of elements in the range `[l, r]` (inclusive).
    pub fn xk_query(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return 0; }
        self.xk_query_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_query_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return 0; }
        if l <= start && end <= r { return self.xk_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_query_rec(2 * node, start, mid, l, r)
            + self.xk_query_rec(2 * node + 1, mid + 1, end, l, r)
    }

    /// Update the value at index `idx` to `val`.
    pub fn xk_update(&mut self, idx: usize, val: i64) {
        if idx >= self.xk_n { return; }
        self.xk_update_rec(1, 0, self.xk_n - 1, idx, val);
    }

    fn xk_update_rec(&mut self, node: usize, start: usize, end: usize, idx: usize, val: i64) {
        if start == end {
            self.xk_tree[node] = val;
            self.xk_min_tree[node] = val;
            self.xk_max_tree[node] = val;
        } else {
            let mid = (start + end) / 2;
            if idx <= mid {
                self.xk_update_rec(2 * node, start, mid, idx, val);
            } else {
                self.xk_update_rec(2 * node + 1, mid + 1, end, idx, val);
            }
            self.xk_tree[node] = self.xk_tree[2 * node] + self.xk_tree[2 * node + 1];
            self.xk_min_tree[node] = self.xk_min_tree[2 * node].min(self.xk_min_tree[2 * node + 1]);
            self.xk_max_tree[node] = self.xk_max_tree[2 * node].max(self.xk_max_tree[2 * node + 1]);
        }
    }

    /// Return the minimum value in the range `[l, r]` (inclusive).
    pub fn xk_range_min(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return i64::MAX; }
        self.xk_min_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_min_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return i64::MAX; }
        if l <= start && end <= r { return self.xk_min_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_min_rec(2 * node, start, mid, l, r)
            .min(self.xk_min_rec(2 * node + 1, mid + 1, end, l, r))
    }

    /// Return the maximum value in the range `[l, r]` (inclusive).
    pub fn xk_range_max(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return i64::MIN; }
        self.xk_max_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_max_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return i64::MIN; }
        if l <= start && end <= r { return self.xk_max_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_max_rec(2 * node, start, mid, l, r)
            .max(self.xk_max_rec(2 * node + 1, mid + 1, end, l, r))
    }

    /// Return the number of elements.
    pub fn xk_len(&self) -> usize {
        self.xk_n
    }
}

/// A set of non-overlapping intervals over `i64`.
pub struct Xk53DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk53DisjointIntervals {
    /// Create an empty interval set.
    pub fn xk_new() -> Self {
        Self { xk_intervals: Vec::new() }
    }

    /// Add interval `[lo, hi]` and merge any overlaps.
    pub fn xk_add_interval(&mut self, lo: i64, hi: i64) {
        if lo > hi { return; }
        let mut new_lo = lo;
        let mut new_hi = hi;
        let mut merged = Vec::new();
        for &(a, b) in &self.xk_intervals {
            if b < new_lo - 1 || a > new_hi + 1 {
                merged.push((a, b));
            } else {
                new_lo = new_lo.min(a);
                new_hi = new_hi.max(b);
            }
        }
        merged.push((new_lo, new_hi));
        merged.sort();
        self.xk_intervals = merged;
    }

    /// Remove interval `[lo, hi]` from the set.
    pub fn xk_remove_interval(&mut self, lo: i64, hi: i64) {
        if lo > hi { return; }
        let mut result = Vec::new();
        for &(a, b) in &self.xk_intervals {
            if b < lo || a > hi {
                result.push((a, b));
            } else {
                if a < lo { result.push((a, lo - 1)); }
                if b > hi { result.push((hi + 1, b)); }
            }
        }
        self.xk_intervals = result;
    }

    /// Check if a point is contained in any interval.
    pub fn xk_contains_point(&self, p: i64) -> bool {
        self.xk_intervals.iter().any(|&(a, b)| a <= p && p <= b)
    }

    /// Return the total length covered by all intervals.
    pub fn xk_covered_length(&self) -> i64 {
        self.xk_intervals.iter().map(|&(a, b)| b - a + 1).sum()
    }

    /// Return the gaps between intervals as a vec of `(start, end)`.
    pub fn xk_gaps(&self) -> Vec<(i64, i64)> {
        let mut gaps = Vec::new();
        for w in self.xk_intervals.windows(2) {
            gaps.push((w[0].1 + 1, w[1].0 - 1));
        }
        gaps
    }

    /// Merge adjacent intervals that are exactly contiguous.
    pub fn xk_merge_adjacent(&mut self) {
        if self.xk_intervals.len() < 2 { return; }
        let mut merged = vec![self.xk_intervals[0]];
        for &(a, b) in &self.xk_intervals[1..] {
            let last = merged.last_mut().unwrap();
            if a <= last.1 + 1 {
                last.1 = last.1.max(b);
            } else {
                merged.push((a, b));
            }
        }
        self.xk_intervals = merged;
    }

    /// Return the number of disjoint intervals.
    pub fn xk_interval_count(&self) -> usize {
        self.xk_intervals.len()
    }
}


/// Rope data structure for efficient large text manipulation (xl_53).
#[derive(Debug, Clone)]
pub struct Xl53Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl53Rope {
    /// Create a new empty rope.
    pub fn xl_new() -> Self {
        Self {
            xl_chunks: Vec::new(),
            xl_total_len: 0,
        }
    }

    /// Create a rope from a string.
    pub fn xl_from_str(s: &str) -> Self {
        let mut rope = Self::xl_new();
        if !s.is_empty() {
            let chunk_size = 64;
            let mut start = 0;
            while start < s.len() {
                let end = (start + chunk_size).min(s.len());
                let boundary = if end < s.len() {
                    let mut b = end;
                    while b > start && !s.is_char_boundary(b) {
                        b -= 1;
                    }
                    if b == start { end } else { b }
                } else {
                    end
                };
                rope.xl_chunks.push(s[start..boundary].to_string());
                rope.xl_total_len += boundary - start;
                start = boundary;
            }
        }
        rope
    }

    /// Insert text at a character offset.
    pub fn xl_insert_at(&mut self, pos: usize, text: &str) {
        if text.is_empty() {
            return;
        }
        let flat = self.xl_to_string();
        let byte_pos = flat.char_indices()
            .nth(pos)
            .map(|(i, _)| i)
            .unwrap_or(flat.len());
        let mut new_str = String::with_capacity(flat.len() + text.len());
        new_str.push_str(&flat[..byte_pos]);
        new_str.push_str(text);
        new_str.push_str(&flat[byte_pos..]);
        *self = Self::xl_from_str(&new_str);
    }

    /// Delete a range of characters [start, end).
    pub fn xl_delete_range(&mut self, start: usize, end: usize) {
        if start >= end {
            return;
        }
        let flat = self.xl_to_string();
        let indices: Vec<usize> = flat.char_indices().map(|(i, _)| i).collect();
        let byte_start = if start < indices.len() { indices[start] } else { flat.len() };
        let byte_end = if end < indices.len() { indices[end] } else { flat.len() };
        let mut new_str = String::with_capacity(flat.len() - (byte_end - byte_start));
        new_str.push_str(&flat[..byte_start]);
        new_str.push_str(&flat[byte_end..]);
        *self = Self::xl_from_str(&new_str);
    }

    /// Get the character at a given index.
    pub fn xl_char_at(&self, index: usize) -> Option<char> {
        self.xl_to_string().chars().nth(index)
    }

    /// Total length in bytes.
    pub fn xl_len(&self) -> usize {
        self.xl_total_len
    }

    /// Check if empty.
    pub fn xl_is_empty(&self) -> bool {
        self.xl_total_len == 0
    }

    /// Extract a substring by byte range.
    pub fn xl_slice(&self, start: usize, end: usize) -> String {
        let flat = self.xl_to_string();
        let clamped_end = end.min(flat.len());
        let clamped_start = start.min(clamped_end);
        flat[clamped_start..clamped_end].to_string()
    }

    /// Split the rope at a byte position into two ropes.
    pub fn xl_split(self, at: usize) -> (Self, Self) {
        let flat = self.xl_to_string();
        let split_at = at.min(flat.len());
        (Self::xl_from_str(&flat[..split_at]), Self::xl_from_str(&flat[split_at..]))
    }

    /// Concatenate another rope onto this one.
    pub fn xl_concat(&mut self, other: &Self) {
        for chunk in &other.xl_chunks {
            self.xl_total_len += chunk.len();
            self.xl_chunks.push(chunk.clone());
        }
    }

    /// Count lines (number of '\n' characters + 1).
    pub fn xl_line_count(&self) -> usize {
        let flat = self.xl_to_string();
        if flat.is_empty() {
            return 0;
        }
        flat.chars().filter(|&c| c == '\n').count() + 1
    }

    /// Get a specific line by zero-based index.
    pub fn xl_line_at(&self, index: usize) -> Option<String> {
        let flat = self.xl_to_string();
        flat.split('\n').nth(index).map(|s| s.to_string())
    }

    /// Flatten to a single String.
    pub fn xl_to_string(&self) -> String {
        let mut out = String::with_capacity(self.xl_total_len);
        for chunk in &self.xl_chunks {
            out.push_str(chunk);
        }
        out
    }

    /// Number of chunks in internal storage.
    pub fn xl_chunk_count(&self) -> usize {
        self.xl_chunks.len()
    }
}

/// Suffix array for efficient string searching (xl_53).
#[derive(Debug, Clone)]
pub struct Xl53SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl53SuffixArray {
    /// Build a suffix array from the given text.
    pub fn xl_build(text: &str) -> Self {
        let n = text.len();
        let mut sa: Vec<usize> = (0..n).collect();
        let bytes = text.as_bytes();
        sa.sort_by(|&a, &b| bytes[a..].cmp(&bytes[b..]));
        Self {
            xl_text: text.to_string(),
            xl_sa: sa,
        }
    }

    /// Search for a pattern; returns the first matching position or None.
    pub fn xl_search(&self, pattern: &str) -> Option<usize> {
        let pat = pattern.as_bytes();
        let text = self.xl_text.as_bytes();
        let mut lo: usize = 0;
        let mut hi: usize = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let suffix_start = self.xl_sa[mid];
            let suffix_end = (suffix_start + pat.len()).min(text.len());
            if &text[suffix_start..suffix_end] < pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        if lo < self.xl_sa.len() {
            let suffix_start = self.xl_sa[lo];
            let suffix_end = (suffix_start + pat.len()).min(text.len());
            if &text[suffix_start..suffix_end] == pat {
                return Some(self.xl_sa[lo]);
            }
        }
        None
    }

    /// Count occurrences of a pattern.
    pub fn xl_count_occurrences(&self, pattern: &str) -> usize {
        self.xl_all_positions(pattern).len()
    }

    /// Find the longest repeated substring.
    pub fn xl_longest_repeated(&self) -> String {
        if self.xl_sa.len() < 2 {
            return String::new();
        }
        let text = self.xl_text.as_bytes();
        let mut best_len = 0;
        let mut best_start = 0;
        for i in 1..self.xl_sa.len() {
            let a = self.xl_sa[i - 1];
            let b = self.xl_sa[i];
            let mut common = 0;
            while a + common < text.len() && b + common < text.len() && text[a + common] == text[b + common] {
                common += 1;
            }
            if common > best_len {
                best_len = common;
                best_start = a;
            }
        }
        self.xl_text[best_start..best_start + best_len].to_string()
    }

    /// Return all positions where the pattern occurs.
    pub fn xl_all_positions(&self, pattern: &str) -> Vec<usize> {
        let pat = pattern.as_bytes();
        let text = self.xl_text.as_bytes();
        let mut results = Vec::new();
        if pat.is_empty() || text.is_empty() {
            return results;
        }
        // Find lower bound
        let mut lo: usize = 0;
        let mut hi: usize = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let s = self.xl_sa[mid];
            let e = (s + pat.len()).min(text.len());
            if &text[s..e] < pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        let start = lo;
        // Find upper bound
        hi = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let s = self.xl_sa[mid];
            let e = (s + pat.len()).min(text.len());
            if &text[s..e] <= pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        for idx in start..lo {
            results.push(self.xl_sa[idx]);
        }
        results.sort();
        results
    }

    /// Length of the underlying text.
    pub fn xl_len(&self) -> usize {
        self.xl_text.len()
    }

    /// Whether the text is empty.
    pub fn xl_is_empty(&self) -> bool {
        self.xl_text.is_empty()
    }
}


/// Sparse matrix storing non-zero entries in coordinate format.
pub struct Xm53MatrixSparse {
    rows: usize,
    cols: usize,
    entries: Vec<(usize, usize, f64)>,
}

impl Xm53MatrixSparse {
    /// Create a new sparse matrix with the given dimensions.
    pub fn xm_new(rows: usize, cols: usize) -> Self {
        Self { rows, cols, entries: Vec::new() }
    }

    /// Set the value at `(row, col)`. Overwrites if already present.
    pub fn xm_set(&mut self, row: usize, col: usize, value: f64) {
        if row >= self.rows || col >= self.cols {
            return;
        }
        if let Some(pos) = self.entries.iter().position(|e| e.0 == row && e.1 == col) {
            if value == 0.0 {
                self.entries.remove(pos);
            } else {
                self.entries[pos].2 = value;
            }
        } else if value != 0.0 {
            self.entries.push((row, col, value));
        }
    }

    /// Get the value at `(row, col)`, returning 0 for absent entries.
    pub fn xm_get(&self, row: usize, col: usize) -> f64 {
        self.entries.iter()
            .find(|e| e.0 == row && e.1 == col)
            .map_or(0.0, |e| e.2)
    }

    /// Return all non-zero entries in the given row as `(col, value)` pairs.
    pub fn xm_row(&self, row: usize) -> Vec<(usize, f64)> {
        self.entries.iter()
            .filter(|e| e.0 == row)
            .map(|e| (e.1, e.2))
            .collect()
    }

    /// Return all non-zero entries in the given column as `(row, value)` pairs.
    pub fn xm_col(&self, col: usize) -> Vec<(usize, f64)> {
        self.entries.iter()
            .filter(|e| e.1 == col)
            .map(|e| (e.0, e.2))
            .collect()
    }

    /// Return a new sparse matrix that is the transpose of this one.
    pub fn xm_transpose(&self) -> Self {
        let mut t = Self::xm_new(self.cols, self.rows);
        for &(r, c, v) in &self.entries {
            t.entries.push((c, r, v));
        }
        t
    }

    /// Multiply this matrix by a dense vector, returning the result vector.
    pub fn xm_multiply_vec(&self, vec: &[f64]) -> Vec<f64> {
        let mut result = vec![0.0; self.rows];
        for &(r, c, v) in &self.entries {
            if c < vec.len() {
                result[r] += v * vec[c];
            }
        }
        result
    }

    /// Return the number of stored non-zero entries.
    pub fn xm_nnz(&self) -> usize {
        self.entries.len()
    }

    /// Return the density (nnz / total_elements).
    pub fn xm_density(&self) -> f64 {
        let total = self.rows * self.cols;
        if total == 0 { return 0.0; }
        self.entries.len() as f64 / total as f64
    }

    /// Remove all entries, keeping dimensions.
    pub fn xm_clear(&mut self) {
        self.entries.clear();
    }

    /// Return the matrix dimensions as `(rows, cols)`.
    pub fn xm_dims(&self) -> (usize, usize) {
        (self.rows, self.cols)
    }
}

/// Simple tokenizer for splitting text into tokens.
pub struct Xm53Tokenizer {
    text: String,
}

impl Xm53Tokenizer {
    /// Create a new tokenizer from the given text.
    pub fn xm_new(text: &str) -> Self {
        Self { text: text.to_string() }
    }

    /// Tokenize the text by splitting on whitespace and filtering empties.
    pub fn xm_tokenize(&self) -> Vec<String> {
        self.text.split_whitespace().map(String::from).collect()
    }

    /// Split by whitespace, preserving the raw split results.
    pub fn xm_split_by_whitespace(&self) -> Vec<String> {
        self.text.split(' ')
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect()
    }

    /// Split the text using a custom single-character delimiter.
    pub fn xm_split_by_delimiter(&self, delim: char) -> Vec<String> {
        self.text.split(delim)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect()
    }

    /// Return the number of whitespace-delimited tokens.
    pub fn xm_token_count(&self) -> usize {
        self.xm_tokenize().len()
    }

    /// Return the set of unique tokens.
    pub fn xm_unique_tokens(&self) -> Vec<String> {
        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();
        for tok in self.xm_tokenize() {
            if seen.insert(tok.clone()) {
                result.push(tok);
            }
        }
        result
    }

    /// Build a frequency map of each token.
    pub fn xm_frequency_map(&self) -> std::collections::HashMap<String, usize> {
        let mut map = std::collections::HashMap::new();
        for tok in self.xm_tokenize() {
            *map.entry(tok).or_insert(0) += 1;
        }
        map
    }

    /// Return the underlying text.
    pub fn xm_text(&self) -> &str {
        &self.text
    }

    /// Return whether the text is empty.
    pub fn xm_is_empty(&self) -> bool {
        self.text.is_empty()
    }
}


/// Fenwick (Binary Indexed) tree for prefix‑sum queries — crate 53.
pub struct Xn53Fenwick {
    xn_tree: Vec<i64>,
    xn_n: usize,
}

impl Xn53Fenwick {
    /// Create a new Fenwick tree of size `n` initialised to zero.
    pub fn xn_new(n: usize) -> Self {
        Self { xn_tree: vec![0i64; n + 1], xn_n: n }
    }

    /// Point‑update: add `delta` to index `i` (0‑based).
    pub fn xn_update(&mut self, mut i: usize, delta: i64) {
        i += 1;
        while i <= self.xn_n {
            self.xn_tree[i] += delta;
            i += i & i.wrapping_neg();
        }
    }

    /// Prefix sum of elements `[0, i]` (0‑based, inclusive).
    pub fn xn_prefix_sum(&self, mut i: usize) -> i64 {
        i += 1;
        let mut s = 0i64;
        while i > 0 {
            s += self.xn_tree[i];
            i -= i & i.wrapping_neg();
        }
        s
    }

    /// Range sum of elements `[l, r]` (inclusive, 0‑based).
    pub fn xn_range_sum(&self, l: usize, r: usize) -> i64 {
        if l == 0 {
            self.xn_prefix_sum(r)
        } else {
            self.xn_prefix_sum(r) - self.xn_prefix_sum(l - 1)
        }
    }

    /// Point query — value at index `i`.
    pub fn xn_point_query(&self, i: usize) -> i64 {
        self.xn_range_sum(i, i)
    }

    /// Number of elements the tree can hold.
    pub fn xn_len(&self) -> usize {
        self.xn_n
    }

    /// Find the smallest index whose prefix sum is at least `target`.
    /// Returns `None` when no such index exists.
    pub fn xn_find_kth(&self, mut target: i64) -> Option<usize> {
        let mut pos: usize = 0;
        let mut bit_mask = 1usize;
        while bit_mask <= self.xn_n {
            bit_mask <<= 1;
        }
        bit_mask >>= 1;
        while bit_mask > 0 {
            let next = pos + bit_mask;
            if next <= self.xn_n && self.xn_tree[next] < target {
                target -= self.xn_tree[next];
                pos = next;
            }
            bit_mask >>= 1;
        }
        let result = pos; // 0‑based
        if result < self.xn_n {
            Some(result)
        } else {
            None
        }
    }
}

// ----- AVL tree map — crate 53 -----

#[derive(Debug, Clone)]
struct Xn53AvlNode<K, V> {
    key: K,
    value: V,
    left: Option<Box<Xn53AvlNode<K, V>>>,
    right: Option<Box<Xn53AvlNode<K, V>>>,
    height: i32,
}

/// Self‑balancing AVL tree map — crate 53.
#[derive(Debug, Clone)]
pub struct Xn53AVL<K, V> {
    root: Option<Box<Xn53AvlNode<K, V>>>,
    xn_len: usize,
}

impl<K: Ord + Clone, V: Clone> Default for Xn53AVL<K, V> {
    fn default() -> Self {
        Self::xn_new()
    }
}

impl<K: Ord + Clone, V: Clone> Xn53AVL<K, V> {
    pub fn xn_new() -> Self {
        Self { root: None, xn_len: 0 }
    }

    fn xn_node_height(node: &Option<Box<Xn53AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| n.height)
    }

    fn xn_balance(node: &Option<Box<Xn53AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| Self::xn_node_height(&n.left) - Self::xn_node_height(&n.right))
    }

    fn xn_update_height(node: &mut Box<Xn53AvlNode<K, V>>) {
        node.height = 1 + std::cmp::max(Self::xn_node_height(&node.left), Self::xn_node_height(&node.right));
    }

    fn xn_rotate_right(mut y: Box<Xn53AvlNode<K, V>>) -> Box<Xn53AvlNode<K, V>> {
        let mut x = y.left.take().expect("xn rotate right");
        y.left = x.right.take();
        Self::xn_update_height(&mut y);
        x.right = Some(y);
        Self::xn_update_height(&mut x);
        x
    }

    fn xn_rotate_left(mut x: Box<Xn53AvlNode<K, V>>) -> Box<Xn53AvlNode<K, V>> {
        let mut y = x.right.take().expect("xn rotate left");
        x.right = y.left.take();
        Self::xn_update_height(&mut x);
        y.left = Some(x);
        Self::xn_update_height(&mut y);
        y
    }

    fn xn_rebalance(mut node: Box<Xn53AvlNode<K, V>>) -> Box<Xn53AvlNode<K, V>> {
        Self::xn_update_height(&mut node);
        let bal = Self::xn_balance(&Some(node.clone()));
        if bal > 1 {
            if Self::xn_balance(&node.left) < 0 {
                node.left = Some(Self::xn_rotate_left(node.left.take().unwrap()));
            }
            return Self::xn_rotate_right(node);
        }
        if bal < -1 {
            if Self::xn_balance(&node.right) > 0 {
                node.right = Some(Self::xn_rotate_right(node.right.take().unwrap()));
            }
            return Self::xn_rotate_left(node);
        }
        node
    }

    fn xn_insert_node(node: Option<Box<Xn53AvlNode<K, V>>>, key: K, value: V, inserted: &mut bool) -> Box<Xn53AvlNode<K, V>> {
        let Some(mut n) = node else {
            *inserted = true;
            return Box::new(Xn53AvlNode { key, value, left: None, right: None, height: 1 });
        };
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => n.left = Some(Self::xn_insert_node(n.left.take(), key, value, inserted)),
            std::cmp::Ordering::Greater => n.right = Some(Self::xn_insert_node(n.right.take(), key, value, inserted)),
            std::cmp::Ordering::Equal => { n.value = value; }
        }
        Self::xn_rebalance(n)
    }

    /// Insert or update a key‑value pair.
    pub fn xn_insert(&mut self, key: K, value: V) {
        let mut inserted = false;
        let root = Self::xn_insert_node(self.root.take(), key, value, &mut inserted);
        self.root = Some(root);
        if inserted { self.xn_len += 1; }
    }

    fn xn_get_node<'a>(node: &'a Option<Box<Xn53AvlNode<K, V>>>, key: &K) -> Option<&'a V> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => Self::xn_get_node(&n.left, key),
            std::cmp::Ordering::Greater => Self::xn_get_node(&n.right, key),
            std::cmp::Ordering::Equal => Some(&n.value),
        }
    }

    /// Look up a value by key.
    pub fn xn_get(&self, key: &K) -> Option<&V> {
        Self::xn_get_node(&self.root, key)
    }

    /// Check whether the map contains `key`.
    pub fn xn_contains(&self, key: &K) -> bool {
        self.xn_get(key).is_some()
    }

    fn xn_min_node(node: &Box<Xn53AvlNode<K, V>>) -> &Xn53AvlNode<K, V> {
        node.left.as_ref().map_or(node.as_ref(), |l| Self::xn_min_node(l))
    }

    fn xn_remove_min(mut node: Box<Xn53AvlNode<K, V>>) -> (Box<Xn53AvlNode<K, V>>, Option<Box<Xn53AvlNode<K, V>>>) {
        if node.left.is_none() {
            let right = node.right.take();
            return (node, right);
        }
        let (min, new_left) = Self::xn_remove_min(node.left.take().unwrap());
        node.left = new_left;
        (min, Some(Self::xn_rebalance(node)))
    }

    fn xn_remove_node(node: Option<Box<Xn53AvlNode<K, V>>>, key: &K, removed: &mut bool) -> Option<Box<Xn53AvlNode<K, V>>> {
        let Some(mut n) = node else { return None };
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => { n.left = Self::xn_remove_node(n.left.take(), key, removed); Some(Self::xn_rebalance(n)) }
            std::cmp::Ordering::Greater => { n.right = Self::xn_remove_node(n.right.take(), key, removed); Some(Self::xn_rebalance(n)) }
            std::cmp::Ordering::Equal => {
                *removed = true;
                match (n.left.take(), n.right.take()) {
                    (None, None) => None,
                    (Some(l), None) => Some(Self::xn_rebalance(l)),
                    (None, Some(r)) => Some(Self::xn_rebalance(r)),
                    (Some(l), Some(r)) => {
                        let (mut successor, new_right) = Self::xn_remove_min(r);
                        successor.left = Some(l);
                        successor.right = new_right;
                        Some(Self::xn_rebalance(successor))
                    }
                }
            }
        }
    }

    /// Remove a key from the map. Returns `true` when the key was present.
    pub fn xn_remove(&mut self, key: &K) -> bool {
        let mut removed = false;
        self.root = Self::xn_remove_node(self.root.take(), key, &mut removed);
        if removed { self.xn_len -= 1; }
        removed
    }

    /// Number of entries.
    pub fn xn_len(&self) -> usize {
        self.xn_len
    }

    fn xn_collect_in_order(node: &Option<Box<Xn53AvlNode<K, V>>>, out: &mut Vec<(K, V)>) {
        if let Some(n) = node {
            Self::xn_collect_in_order(&n.left, out);
            out.push((n.key.clone(), n.value.clone()));
            Self::xn_collect_in_order(&n.right, out);
        }
    }

    /// Return all key‑value pairs in sorted order.
    pub fn xn_in_order(&self) -> Vec<(K, V)> {
        let mut v = Vec::new();
        Self::xn_collect_in_order(&self.root, &mut v);
        v
    }

    /// Height of the tree (0 for empty).
    pub fn xn_height(&self) -> i32 {
        Self::xn_node_height(&self.root)
    }

    fn xn_min_key(node: &Option<Box<Xn53AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.left.is_some() { Self::xn_min_key(&n.left) } else { Some(&n.key) }
    }

    /// Smallest key in the map.
    pub fn xn_min(&self) -> Option<&K> {
        Self::xn_min_key(&self.root)
    }

    fn xn_max_key(node: &Option<Box<Xn53AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.right.is_some() { Self::xn_max_key(&n.right) } else { Some(&n.key) }
    }

    /// Largest key in the map.
    pub fn xn_max(&self) -> Option<&K> {
        Self::xn_max_key(&self.root)
    }

    fn xn_floor_key<'a>(node: &'a Option<Box<Xn53AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Equal => Some(&n.key),
            std::cmp::Ordering::Less => Self::xn_floor_key(&n.left, key),
            std::cmp::Ordering::Greater => Self::xn_floor_key(&n.right, key).or(Some(&n.key)),
        }
    }

    /// Greatest key less than or equal to `key`.
    pub fn xn_floor(&self, key: &K) -> Option<&K> {
        Self::xn_floor_key(&self.root, key)
    }

    fn xn_ceiling_key<'a>(node: &'a Option<Box<Xn53AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Equal => Some(&n.key),
            std::cmp::Ordering::Greater => Self::xn_ceiling_key(&n.right, key),
            std::cmp::Ordering::Less => Self::xn_ceiling_key(&n.left, key).or(Some(&n.key)),
        }
    }

    /// Smallest key greater than or equal to `key`.
    pub fn xn_ceiling(&self, key: &K) -> Option<&K> {
        Self::xn_ceiling_key(&self.root, key)
    }
}


// ---------------------------------------------------------------------------
// Xo53RedBlack<K,V> — red-black tree map
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Xo53Color {
    Red,
    Black,
}

#[derive(Debug, Clone)]
struct Xo53RBNode<K, V> {
    key: K,
    value: V,
    color: Xo53Color,
    left: Option<Box<Xo53RBNode<K, V>>>,
    right: Option<Box<Xo53RBNode<K, V>>>,
}

/// A red-black tree map for crate 53.
#[derive(Debug, Clone)]
pub struct Xo53RedBlack<K, V> {
    root: Option<Box<Xo53RBNode<K, V>>>,
    len: usize,
}

impl<K: Ord + Clone, V: Clone> Xo53RedBlack<K, V> {
    pub fn xo_new() -> Self {
        Self { root: None, len: 0 }
    }

    pub fn xo_len(&self) -> usize {
        self.len
    }

    pub fn xo_is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn xo_insert(&mut self, key: K, value: V) {
        self.root = Some(Self::xo_ins(self.root.take(), key, value, &mut self.len));
        if let Some(ref mut r) = self.root {
            r.color = Xo53Color::Black;
        }
    }

    fn xo_ins(node: Option<Box<Xo53RBNode<K, V>>>, key: K, value: V, len: &mut usize) -> Box<Xo53RBNode<K, V>> {
        match node {
            None => {
                *len += 1;
                Box::new(Xo53RBNode {
                    key, value, color: Xo53Color::Red, left: None, right: None,
                })
            }
            Some(mut n) => {
                use std::cmp::Ordering;
                match key.cmp(&n.key) {
                    Ordering::Less => n.left = Some(Self::xo_ins(n.left.take(), key, value, len)),
                    Ordering::Greater => n.right = Some(Self::xo_ins(n.right.take(), key, value, len)),
                    Ordering::Equal => { n.value = value; return n; }
                }
                Self::xo_balance(n)
            }
        }
    }

    fn xo_is_red(node: &Option<Box<Xo53RBNode<K, V>>>) -> bool {
        matches!(node, Some(n) if n.color == Xo53Color::Red)
    }

    fn xo_balance(mut h: Box<Xo53RBNode<K, V>>) -> Box<Xo53RBNode<K, V>> {
        if Self::xo_is_red(&h.right) && !Self::xo_is_red(&h.left) {
            h = Self::xo_rotate_left(h);
        }
        if Self::xo_is_red(&h.left) {
            let left_left_red = h.left.as_ref().and_then(|l| l.left.as_ref()).map_or(false, |ll| ll.color == Xo53Color::Red);
            if left_left_red {
                h = Self::xo_rotate_right(h);
            }
        }
        if Self::xo_is_red(&h.left) && Self::xo_is_red(&h.right) {
            Self::xo_flip_colors(&mut h);
        }
        h
    }

    fn xo_rotate_left(mut h: Box<Xo53RBNode<K, V>>) -> Box<Xo53RBNode<K, V>> {
        let mut x = h.right.take().unwrap();
        h.right = x.left.take();
        x.color = h.color;
        h.color = Xo53Color::Red;
        x.left = Some(h);
        x
    }

    fn xo_rotate_right(mut h: Box<Xo53RBNode<K, V>>) -> Box<Xo53RBNode<K, V>> {
        let mut x = h.left.take().unwrap();
        h.left = x.right.take();
        x.color = h.color;
        h.color = Xo53Color::Red;
        x.right = Some(h);
        x
    }

    fn xo_flip_colors(h: &mut Box<Xo53RBNode<K, V>>) {
        h.color = Xo53Color::Red;
        if let Some(l) = &mut h.left { l.color = Xo53Color::Black; }
        if let Some(r) = &mut h.right { r.color = Xo53Color::Black; }
    }

    pub fn xo_get(&self, key: &K) -> Option<&V> {
        let mut cur = &self.root;
        while let Some(node) = cur {
            use std::cmp::Ordering;
            match key.cmp(&node.key) {
                Ordering::Less => cur = &node.left,
                Ordering::Greater => cur = &node.right,
                Ordering::Equal => return Some(&node.value),
            }
        }
        None
    }

    pub fn xo_contains(&self, key: &K) -> bool {
        self.xo_get(key).is_some()
    }

    pub fn xo_min(&self) -> Option<&K> {
        let mut cur = &self.root;
        let mut result = None;
        while let Some(node) = cur {
            result = Some(&node.key);
            cur = &node.left;
        }
        result
    }

    pub fn xo_max(&self) -> Option<&K> {
        let mut cur = &self.root;
        let mut result = None;
        while let Some(node) = cur {
            result = Some(&node.key);
            cur = &node.right;
        }
        result
    }

    pub fn xo_remove(&mut self, key: &K) -> Option<V> {
        let mut found = None;
        self.root = Self::xo_remove_rec(self.root.take(), key, &mut found);
        if let Some(ref mut r) = self.root {
            r.color = Xo53Color::Black;
        }
        if found.is_some() { self.len -= 1; }
        found
    }

    fn xo_remove_rec(node: Option<Box<Xo53RBNode<K, V>>>, key: &K, found: &mut Option<V>) -> Option<Box<Xo53RBNode<K, V>>> {
        match node {
            None => None,
            Some(mut n) => {
                use std::cmp::Ordering;
                match key.cmp(&n.key) {
                    Ordering::Less => { n.left = Self::xo_remove_rec(n.left.take(), key, found); Some(n) }
                    Ordering::Greater => { n.right = Self::xo_remove_rec(n.right.take(), key, found); Some(n) }
                    Ordering::Equal => {
                        *found = Some(n.value.clone());
                        match (n.left.take(), n.right.take()) {
                            (None, None) => None,
                            (Some(l), None) => Some(l),
                            (None, Some(r)) => Some(r),
                            (Some(l), Some(r)) => {
                                let (min_key, min_val, new_right) = Self::xo_remove_min_node(*r);
                                n.key = min_key; n.value = min_val;
                                n.left = Some(l); n.right = new_right;
                                Some(n)
                            }
                        }
                    }
                }
            }
        }
    }

    fn xo_remove_min_node(mut node: Xo53RBNode<K, V>) -> (K, V, Option<Box<Xo53RBNode<K, V>>>) {
        if node.left.is_none() {
            return (node.key, node.value, node.right);
        }
        let (k, v, new_left) = Self::xo_remove_min_node(*node.left.take().unwrap());
        node.left = new_left;
        (k, v, Some(Box::new(node)))
    }

    pub fn xo_black_height(&self) -> usize {
        fn bh<K, V>(node: &Option<Box<Xo53RBNode<K, V>>>) -> usize {
            match node {
                None => 1,
                Some(n) => {
                    let add = if n.color == Xo53Color::Black { 1 } else { 0 };
                    add + bh(&n.left)
                }
            }
        }
        bh(&self.root)
    }

    pub fn xo_in_order(&self) -> Vec<(K, V)> {
        let mut result = Vec::new();
        fn collect<K: Clone, V: Clone>(node: &Option<Box<Xo53RBNode<K, V>>>, out: &mut Vec<(K, V)>) {
            if let Some(n) = node {
                collect(&n.left, out);
                out.push((n.key.clone(), n.value.clone()));
                collect(&n.right, out);
            }
        }
        collect(&self.root, &mut result);
        result
    }
}

// ---------------------------------------------------------------------------
// Xo53ConsistentHash — consistent hash ring
// ---------------------------------------------------------------------------

/// A consistent hash ring for crate 53.
#[derive(Debug, Clone)]
pub struct Xo53ConsistentHash {
    ring: std::collections::BTreeMap<u64, String>,
    nodes: std::collections::HashMap<String, usize>,
    virtual_count: usize,
}

impl Xo53ConsistentHash {
    pub fn xo_new(virtual_count: usize) -> Self {
        Self {
            ring: std::collections::BTreeMap::new(),
            nodes: std::collections::HashMap::new(),
            virtual_count,
        }
    }

    fn xo_hash(data: &str) -> u64 {
        let mut h: u64 = 5381;
        for b in data.bytes() {
            h = h.wrapping_mul(33).wrapping_add(b as u64);
        }
        h
    }

    pub fn xo_add_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo53#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.insert(hash, node.to_string());
        }
        *self.nodes.entry(node.to_string()).or_insert(0) += 1;
    }

    pub fn xo_remove_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo53#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.remove(&hash);
        }
        self.nodes.remove(node);
    }

    pub fn xo_get_node(&self, key: &str) -> Option<&str> {
        if self.ring.is_empty() {
            return None;
        }
        let hash = Self::xo_hash(key);
        let entry = self.ring.range(hash..).next().or_else(|| self.ring.iter().next());
        entry.map(|(_, v)| v.as_str())
    }

    pub fn xo_node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn xo_rebalance_factor(&self) -> f64 {
        if self.nodes.is_empty() {
            return 0.0;
        }
        let total = self.ring.len() as f64;
        let expected = total / self.nodes.len() as f64;
        let mut max_dev: f64 = 0.0;
        let counts: std::collections::HashMap<&str, usize> = self.ring.values().fold(
            std::collections::HashMap::new(),
            |mut acc, v| { *acc.entry(v.as_str()).or_insert(0) += 1; acc }
        );
        for &c in counts.values() {
            let dev = ((c as f64) - expected).abs();
            if dev > max_dev { max_dev = dev; }
        }
        if expected > 0.0 { max_dev / expected } else { 0.0 }
    }

    pub fn xo_virtual_nodes(&self) -> usize {
        self.ring.len()
    }

    pub fn xo_key_distribution(&self, keys: &[&str]) -> std::collections::HashMap<String, usize> {
        let mut dist: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for k in keys {
            if let Some(node) = self.xo_get_node(k) {
                *dist.entry(node.to_string()).or_insert(0) += 1;
            }
        }
        dist
    }
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


    // --- xd_72 deepening tests ---

    #[test]
    fn xd_72_sm_initial_state() {
        let sm = Xd72StateMachine::new();
        assert_eq!(sm.current_state(), Xd72State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_72_sm_valid_idle_to_running() {
        let mut sm = Xd72StateMachine::new();
        assert!(sm.transition(Xd72State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd72State::Running);
    }

    #[test]
    fn xd_72_sm_valid_running_to_paused() {
        let mut sm = Xd72StateMachine::new();
        sm.transition(Xd72State::Running).unwrap();
        assert!(sm.transition(Xd72State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd72State::Paused);
    }

    #[test]
    fn xd_72_sm_valid_running_to_done() {
        let mut sm = Xd72StateMachine::new();
        sm.transition(Xd72State::Running).unwrap();
        assert!(sm.transition(Xd72State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd72State::Done);
    }

    #[test]
    fn xd_72_sm_valid_paused_to_running() {
        let mut sm = Xd72StateMachine::new();
        sm.transition(Xd72State::Running).unwrap();
        sm.transition(Xd72State::Paused).unwrap();
        assert!(sm.transition(Xd72State::Running).is_ok());
    }

    #[test]
    fn xd_72_sm_valid_done_to_idle() {
        let mut sm = Xd72StateMachine::new();
        sm.transition(Xd72State::Running).unwrap();
        sm.transition(Xd72State::Done).unwrap();
        assert!(sm.transition(Xd72State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd72State::Idle);
    }

    #[test]
    fn xd_72_sm_invalid_idle_to_done() {
        let mut sm = Xd72StateMachine::new();
        assert!(sm.transition(Xd72State::Done).is_err());
    }

    #[test]
    fn xd_72_sm_invalid_idle_to_paused() {
        let mut sm = Xd72StateMachine::new();
        assert!(sm.transition(Xd72State::Paused).is_err());
    }

    #[test]
    fn xd_72_sm_history_tracking() {
        let mut sm = Xd72StateMachine::new();
        sm.transition(Xd72State::Running).unwrap();
        sm.transition(Xd72State::Paused).unwrap();
        sm.transition(Xd72State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd72State::Idle);
        assert_eq!(sm.history()[0].to, Xd72State::Running);
        assert_eq!(sm.history()[1].from, Xd72State::Running);
        assert_eq!(sm.history()[2].to, Xd72State::Done);
    }

    #[test]
    fn xd_72_sm_serialize_deserialize() {
        let mut sm = Xd72StateMachine::new();
        sm.transition(Xd72State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd72StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd72State::Running));
    }

    #[test]
    fn xd_72_sm_deserialize_invalid() {
        assert_eq!(Xd72StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_72_sm_reset() {
        let mut sm = Xd72StateMachine::new();
        sm.transition(Xd72State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd72State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_72_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd72EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd72Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_72_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd72EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd72Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd72Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_72_bus_unsubscribe() {
        let mut bus = Xd72EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_72_event_kind_and_payload() {
        let e = Xd72Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd72Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_72_bus_clear_history() {
        let mut bus = Xd72EventBus::new();
        bus.publish(Xd72Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_72_sm_step_counter_increments() {
        let mut sm = Xd72StateMachine::new();
        sm.transition(Xd72State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd72State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xf_ trie + bloom tests for instance #87 --

    #[test]
    fn xf87_trie_insert_search() {
        let mut t = Xf87Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf87_trie_starts_with() {
        let mut t = Xf87Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf87_trie_remove() {
        let mut t = Xf87Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf87_trie_word_count() {
        let mut t = Xf87Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf87_trie_longest_prefix() {
        let mut t = Xf87Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf87_trie_all_words() {
        let mut t = Xf87Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf87_trie_autocomplete() {
        let mut t = Xf87Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf87_trie_empty_search() {
        let t = Xf87Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf87_bloom_add_contains() {
        let mut bf = Xf87BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf87_bloom_probably_absent() {
        let bf = Xf87BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf87_bloom_false_positive_rate() {
        let mut bf = Xf87BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf87_bloom_clear() {
        let mut bf = Xf87BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf87_bloom_union() {
        let mut a = Xf87BloomFilter::xf_new(512, 2);
        let mut b = Xf87BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf87_bloom_intersection_estimate() {
        let mut a = Xf87BloomFilter::xf_new(512, 2);
        let mut b = Xf87BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf87_bloom_union_size_mismatch() {
        let a = Xf87BloomFilter::xf_new(256, 2);
        let b = Xf87BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }


    #[test]
    fn xh53_skip_insert_contains() {
        let mut sl = super::Xh53SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh53_skip_remove() {
        let mut sl = super::Xh53SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh53_skip_len() {
        let mut sl = super::Xh53SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh53_skip_range_query() {
        let mut sl = super::Xh53SkipList::xh_new(4);
        for v in [3, 7, 1, 9, 5] {
            sl.xh_insert(v);
        }
        let r = sl.xh_range_query(3, 7);
        assert!(r.contains(&3));
        assert!(r.contains(&5));
        assert!(r.contains(&7));
        assert!(!r.contains(&1));
        assert!(!r.contains(&9));
    }

    #[test]
    fn xh53_skip_floor_ceiling() {
        let mut sl = super::Xh53SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh53_skip_rank() {
        let mut sl = super::Xh53SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh53_skip_empty() {
        let sl = super::Xh53SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh53_skip_duplicates() {
        let mut sl = super::Xh53SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh53_bitset_set_test() {
        let mut bs = super::Xh53BitSet::xh_new(256);
        bs.xh_set(0);
        bs.xh_set(63);
        bs.xh_set(64);
        bs.xh_set(255);
        assert!(bs.xh_test(0));
        assert!(bs.xh_test(63));
        assert!(bs.xh_test(64));
        assert!(bs.xh_test(255));
        assert!(!bs.xh_test(1));
    }

    #[test]
    fn xh53_bitset_clear_count() {
        let mut bs = super::Xh53BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh53_bitset_and_or_xor() {
        let mut a = super::Xh53BitSet::xh_new(128);
        let mut b = super::Xh53BitSet::xh_new(128);
        a.xh_set(1);
        a.xh_set(2);
        b.xh_set(2);
        b.xh_set(3);
        let and_r = a.xh_and(&b);
        assert!(and_r.xh_test(2));
        assert!(!and_r.xh_test(1));
        let or_r = a.xh_or(&b);
        assert!(or_r.xh_test(1));
        assert!(or_r.xh_test(2));
        assert!(or_r.xh_test(3));
        let xor_r = a.xh_xor(&b);
        assert!(xor_r.xh_test(1));
        assert!(!xor_r.xh_test(2));
        assert!(xor_r.xh_test(3));
    }

    #[test]
    fn xh53_bitset_iter_ones() {
        let mut bs = super::Xh53BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh53_bitset_first_last() {
        let mut bs = super::Xh53BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh53_bitset_empty() {
        let bs = super::Xh53BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi53_deque_push_pop_back() {
        let mut dq = super::Xi53Deque::xi_new(4);
        dq.xi_push_back(10);
        dq.xi_push_back(20);
        dq.xi_push_back(30);
        assert_eq!(dq.xi_len(), 3);
        assert_eq!(dq.xi_pop_back(), Some(30));
        assert_eq!(dq.xi_pop_back(), Some(20));
        assert_eq!(dq.xi_pop_back(), Some(10));
        assert_eq!(dq.xi_pop_back(), None);
    }

    #[test]
    fn xi53_deque_push_pop_front() {
        let mut dq = super::Xi53Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi53_deque_mixed_ops() {
        let mut dq = super::Xi53Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi53_deque_get_and_split() {
        let mut dq = super::Xi53Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_get(0), Some(&0));
        assert_eq!(dq.xi_get(4), Some(&4));
        assert_eq!(dq.xi_get(5), None);
        let (left, right) = dq.xi_split_at(3);
        assert_eq!(left, vec![0, 1, 2]);
        assert_eq!(right, vec![3, 4]);
    }

    #[test]
    fn xi53_deque_rotate_left() {
        let mut dq = super::Xi53Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi53_deque_rotate_right() {
        let mut dq = super::Xi53Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi53_deque_grow() {
        let mut dq = super::Xi53Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi53_deque_empty() {
        let dq = super::Xi53Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi53_interval_tree_insert_query() {
        let mut tree = super::Xi53IntervalTree::xi_new();
        tree.xi_insert(super::Xi53Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi53Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi53Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi53_interval_tree_overlap() {
        let mut tree = super::Xi53IntervalTree::xi_new();
        tree.xi_insert(super::Xi53Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi53Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi53Interval::xi_new(12, 20));
        let q = super::Xi53Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi53_interval_tree_remove() {
        let mut tree = super::Xi53IntervalTree::xi_new();
        tree.xi_insert(super::Xi53Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi53Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi53_interval_tree_gaps() {
        let mut tree = super::Xi53IntervalTree::xi_new();
        tree.xi_insert(super::Xi53Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi53Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi53Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi53Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi53Interval::xi_new(8, 10));
    }

    #[test]
    fn xi53_interval_tree_merge() {
        let mut tree = super::Xi53IntervalTree::xi_new();
        tree.xi_insert(super::Xi53Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi53Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi53Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi53Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi53Interval::xi_new(10, 15));
    }

    #[test]
    fn xi53_interval_tree_all() {
        let mut tree = super::Xi53IntervalTree::xi_new();
        tree.xi_insert(super::Xi53Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi53Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi53_interval_tree_empty() {
        let tree = super::Xi53IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi53_interval_tree_contains_point() {
        let iv = super::Xi53Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 53) ---

    #[test]
    fn xj_53_uf_make_and_find() {
        let mut uf = super::Xj53UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_53_uf_union_connected() {
        let mut uf = super::Xj53UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_53_uf_component_count() {
        let mut uf = super::Xj53UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let c = uf.xj_make_set();
        assert_eq!(uf.xj_component_count(), 3);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_count(), 2);
        uf.xj_union(b, c);
        assert_eq!(uf.xj_component_count(), 1);
    }

    #[test]
    fn xj_53_uf_component_size() {
        let mut uf = super::Xj53UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_53_uf_largest_component() {
        let mut uf = super::Xj53UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_53_uf_many_elements() {
        let mut uf = super::Xj53UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_53_uf_separate_components() {
        let mut uf = super::Xj53UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let c = uf.xj_make_set();
        let d = uf.xj_make_set();
        uf.xj_union(a, b);
        uf.xj_union(c, d);
        assert!(uf.xj_connected(a, b));
        assert!(uf.xj_connected(c, d));
        assert!(!uf.xj_connected(a, c));
    }

    #[test]
    fn xj_53_uf_path_compression() {
        let mut uf = super::Xj53UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_53_bt_insert_get() {
        let mut bt = super::Xj53BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_53_bt_contains_len() {
        let mut bt = super::Xj53BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_53_bt_replace() {
        let mut bt = super::Xj53BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_53_bt_remove() {
        let mut bt = super::Xj53BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_53_bt_keys_values() {
        let mut bt = super::Xj53BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_53_bt_range() {
        let mut bt = super::Xj53BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_53_bt_min_max() {
        let mut bt = super::Xj53BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_53_bt_many_inserts() {
        let mut bt = super::Xj53BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_53 segment tree tests ---

    #[test]
    fn xk_53_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk53SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_53_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk53SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_53_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk53SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_53_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk53SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_53_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk53SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_53_st_single_element() {
        let data = vec![42];
        let st = super::Xk53SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_53_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk53SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_53_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk53SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_53 disjoint intervals tests ---

    #[test]
    fn xk_53_di_add_and_count() {
        let mut di = super::Xk53DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_53_di_merge_overlap() {
        let mut di = super::Xk53DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_53_di_contains() {
        let mut di = super::Xk53DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_53_di_remove() {
        let mut di = super::Xk53DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_53_di_covered_length() {
        let mut di = super::Xk53DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_53_di_gaps() {
        let mut di = super::Xk53DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_53_di_merge_adjacent() {
        let mut di = super::Xk53DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_53_di_empty() {
        let di = super::Xk53DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_53_rope_new_empty() {
        let rope = super::Xl53Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_53_rope_from_str() {
        let rope = super::Xl53Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_53_rope_insert_at() {
        let mut rope = super::Xl53Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_53_rope_delete_range() {
        let mut rope = super::Xl53Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_53_rope_char_at() {
        let rope = super::Xl53Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_53_rope_split_concat() {
        let rope = super::Xl53Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_53_rope_line_count() {
        let rope = super::Xl53Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_53_rope_line_at() {
        let rope = super::Xl53Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_53_sa_build_and_search() {
        let sa = super::Xl53SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_53_sa_count() {
        let sa = super::Xl53SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_53_sa_longest_repeated() {
        let sa = super::Xl53SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_53_sa_all_positions() {
        let sa = super::Xl53SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_53_sa_len() {
        let sa = super::Xl53SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_53_sa_empty() {
        let sa = super::Xl53SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_53_rope_slice() {
        let rope = super::Xl53Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_53_sa_search_start() {
        let sa = super::Xl53SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }

    #[test]
    fn xm_53_sparse_set_get() {
        let mut m = super::Xm53MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 1, 5.0);
        assert!((m.xm_get(0, 1) - 5.0).abs() < f64::EPSILON);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_53_sparse_row_col() {
        let mut m = super::Xm53MatrixSparse::xm_new(4, 4);
        m.xm_set(1, 2, 3.0);
        m.xm_set(1, 3, 7.0);
        let row = m.xm_row(1);
        assert_eq!(row.len(), 2);
        let col = m.xm_col(2);
        assert_eq!(col.len(), 1);
    }

    #[test]
    fn xm_53_sparse_transpose() {
        let mut m = super::Xm53MatrixSparse::xm_new(2, 3);
        m.xm_set(0, 2, 9.0);
        let t = m.xm_transpose();
        assert!((t.xm_get(2, 0) - 9.0).abs() < f64::EPSILON);
        assert_eq!(t.xm_dims(), (3, 2));
    }

    #[test]
    fn xm_53_sparse_multiply_vec() {
        let mut m = super::Xm53MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        let result = m.xm_multiply_vec(&[3.0, 4.0]);
        assert!((result[0] - 3.0).abs() < f64::EPSILON);
        assert!((result[1] - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_53_sparse_nnz_density() {
        let mut m = super::Xm53MatrixSparse::xm_new(10, 10);
        m.xm_set(0, 0, 1.0);
        m.xm_set(5, 5, 2.0);
        assert_eq!(m.xm_nnz(), 2);
        assert!((m.xm_density() - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_53_sparse_clear() {
        let mut m = super::Xm53MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        m.xm_clear();
        assert_eq!(m.xm_nnz(), 0);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_53_sparse_overwrite_zero() {
        let mut m = super::Xm53MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 5.0);
        assert_eq!(m.xm_nnz(), 1);
        m.xm_set(0, 0, 0.0);
        assert_eq!(m.xm_nnz(), 0);
    }

    #[test]
    fn xm_53_tokenizer_basic() {
        let t = super::Xm53Tokenizer::xm_new("hello world foo");
        let tokens = t.xm_tokenize();
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn xm_53_tokenizer_count() {
        let t = super::Xm53Tokenizer::xm_new("a b c d e");
        assert_eq!(t.xm_token_count(), 5);
    }

    #[test]
    fn xm_53_tokenizer_unique() {
        let t = super::Xm53Tokenizer::xm_new("a b a c b");
        let u = t.xm_unique_tokens();
        assert_eq!(u.len(), 3);
    }

    #[test]
    fn xm_53_tokenizer_frequency() {
        let t = super::Xm53Tokenizer::xm_new("x y x x y z");
        let freq = t.xm_frequency_map();
        assert_eq!(freq.get("x"), Some(&3));
        assert_eq!(freq.get("y"), Some(&2));
        assert_eq!(freq.get("z"), Some(&1));
    }

    #[test]
    fn xm_53_tokenizer_delimiter() {
        let t = super::Xm53Tokenizer::xm_new("a,b,,c");
        let parts = t.xm_split_by_delimiter(',');
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn xm_53_tokenizer_whitespace() {
        let t = super::Xm53Tokenizer::xm_new("one  two  three");
        let parts = t.xm_split_by_whitespace();
        assert_eq!(parts, vec!["one", "two", "three"]);
    }

    #[test]
    fn xm_53_tokenizer_empty() {
        let t = super::Xm53Tokenizer::xm_new("");
        assert!(t.xm_is_empty());
        assert_eq!(t.xm_token_count(), 0);
    }


    // ---- Fenwick tree tests — crate 53 ----

    #[test]
    fn xn_53_fenwick_prefix_sum() {
        let mut ft = super::Xn53Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, (i + 1) as i64); }
        assert_eq!(ft.xn_prefix_sum(0), 1);
        assert_eq!(ft.xn_prefix_sum(4), 15);
    }

    #[test]
    fn xn_53_fenwick_range_sum() {
        let mut ft = super::Xn53Fenwick::xn_new(6);
        for i in 0..6 { ft.xn_update(i, (i * 2) as i64); }
        assert_eq!(ft.xn_range_sum(1, 3), 2 + 4 + 6);
    }

    #[test]
    fn xn_53_fenwick_point_query() {
        let mut ft = super::Xn53Fenwick::xn_new(4);
        ft.xn_update(2, 7);
        assert_eq!(ft.xn_point_query(2), 7);
        assert_eq!(ft.xn_point_query(0), 0);
    }

    #[test]
    fn xn_53_fenwick_len() {
        let ft = super::Xn53Fenwick::xn_new(10);
        assert_eq!(ft.xn_len(), 10);
    }

    #[test]
    fn xn_53_fenwick_multiple_updates() {
        let mut ft = super::Xn53Fenwick::xn_new(3);
        ft.xn_update(0, 5);
        ft.xn_update(0, 3);
        assert_eq!(ft.xn_point_query(0), 8);
    }

    #[test]
    fn xn_53_fenwick_single_element() {
        let mut ft = super::Xn53Fenwick::xn_new(1);
        ft.xn_update(0, 42);
        assert_eq!(ft.xn_prefix_sum(0), 42);
        assert_eq!(ft.xn_range_sum(0, 0), 42);
    }

    #[test]
    fn xn_53_fenwick_find_kth() {
        let mut ft = super::Xn53Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, 1); }
        assert_eq!(ft.xn_find_kth(3), Some(2));
    }

    #[test]
    fn xn_53_fenwick_negative_delta() {
        let mut ft = super::Xn53Fenwick::xn_new(3);
        ft.xn_update(1, 10);
        ft.xn_update(1, -4);
        assert_eq!(ft.xn_point_query(1), 6);
    }

    // ---- AVL tree tests — crate 53 ----

    #[test]
    fn xn_53_avl_insert_get() {
        let mut m = super::Xn53AVL::xn_new();
        m.xn_insert(3, "c");
        m.xn_insert(1, "a");
        m.xn_insert(2, "b");
        assert_eq!(m.xn_get(&2), Some(&"b"));
        assert_eq!(m.xn_len(), 3);
    }

    #[test]
    fn xn_53_avl_remove() {
        let mut m = super::Xn53AVL::xn_new();
        m.xn_insert(1, 10);
        m.xn_insert(2, 20);
        assert!(m.xn_remove(&1));
        assert!(!m.xn_contains(&1));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_53_avl_in_order() {
        let mut m = super::Xn53AVL::xn_new();
        for k in [5, 3, 7, 1, 4] { m.xn_insert(k, k * 10); }
        let keys: Vec<_> = m.xn_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xn_53_avl_min_max() {
        let mut m = super::Xn53AVL::xn_new();
        for k in [10, 5, 20, 3, 15] { m.xn_insert(k, k); }
        assert_eq!(m.xn_min(), Some(&3));
        assert_eq!(m.xn_max(), Some(&20));
    }

    #[test]
    fn xn_53_avl_floor_ceiling() {
        let mut m = super::Xn53AVL::xn_new();
        for k in [10, 20, 30] { m.xn_insert(k, k); }
        assert_eq!(m.xn_floor(&15), Some(&10));
        assert_eq!(m.xn_ceiling(&15), Some(&20));
    }

    #[test]
    fn xn_53_avl_height_balanced() {
        let mut m = super::Xn53AVL::xn_new();
        for k in 0..31 { m.xn_insert(k, k); }
        assert!(m.xn_height() <= 7);
    }

    #[test]
    fn xn_53_avl_overwrite() {
        let mut m = super::Xn53AVL::xn_new();
        m.xn_insert(1, "old");
        m.xn_insert(1, "new");
        assert_eq!(m.xn_get(&1), Some(&"new"));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_53_avl_empty() {
        let m: super::Xn53AVL<i32, i32> = super::Xn53AVL::xn_new();
        assert_eq!(m.xn_len(), 0);
        assert_eq!(m.xn_min(), None);
        assert_eq!(m.xn_max(), None);
        assert_eq!(m.xn_height(), 0);
    }

    // --- Xo53RedBlack tests ---

    #[test]
    fn xo_53_rb_insert_and_get() {
        let mut tree = super::Xo53RedBlack::xo_new();
        tree.xo_insert(10, "ten");
        tree.xo_insert(20, "twenty");
        tree.xo_insert(5, "five");
        assert_eq!(tree.xo_get(&10), Some(&"ten"));
        assert_eq!(tree.xo_get(&20), Some(&"twenty"));
        assert_eq!(tree.xo_get(&5), Some(&"five"));
        assert_eq!(tree.xo_get(&99), None);
    }

    #[test]
    fn xo_53_rb_len_and_empty() {
        let mut tree = super::Xo53RedBlack::<i32, i32>::xo_new();
        assert!(tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 0);
        tree.xo_insert(1, 100);
        tree.xo_insert(2, 200);
        assert!(!tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 2);
    }

    #[test]
    fn xo_53_rb_min_max() {
        let mut tree = super::Xo53RedBlack::xo_new();
        for k in [30, 10, 50, 20, 40] {
            tree.xo_insert(k, k * 10);
        }
        assert_eq!(tree.xo_min(), Some(&10));
        assert_eq!(tree.xo_max(), Some(&50));
    }

    #[test]
    fn xo_53_rb_contains() {
        let mut tree = super::Xo53RedBlack::xo_new();
        tree.xo_insert(42, "answer");
        assert!(tree.xo_contains(&42));
        assert!(!tree.xo_contains(&43));
    }

    #[test]
    fn xo_53_rb_remove() {
        let mut tree = super::Xo53RedBlack::xo_new();
        tree.xo_insert(1, "a");
        tree.xo_insert(2, "b");
        tree.xo_insert(3, "c");
        assert_eq!(tree.xo_remove(&2), Some("b"));
        assert_eq!(tree.xo_len(), 2);
        assert!(!tree.xo_contains(&2));
        assert_eq!(tree.xo_remove(&99), None);
    }

    #[test]
    fn xo_53_rb_in_order() {
        let mut tree = super::Xo53RedBlack::xo_new();
        for k in [5, 3, 7, 1, 4] {
            tree.xo_insert(k, k);
        }
        let keys: Vec<i32> = tree.xo_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xo_53_rb_black_height() {
        let mut tree = super::Xo53RedBlack::xo_new();
        for k in 0..15 {
            tree.xo_insert(k, k);
        }
        let bh = tree.xo_black_height();
        assert!(bh >= 2 && bh <= 6, "black height {bh} out of range");
    }

    #[test]
    fn xo_53_rb_overwrite() {
        let mut tree = super::Xo53RedBlack::xo_new();
        tree.xo_insert(1, "old");
        tree.xo_insert(1, "new");
        assert_eq!(tree.xo_get(&1), Some(&"new"));
        assert_eq!(tree.xo_len(), 1);
    }

    // --- Xo53ConsistentHash tests ---

    #[test]
    fn xo_53_ch_add_and_count() {
        let mut ring = super::Xo53ConsistentHash::xo_new(100);
        ring.xo_add_node("server-a");
        ring.xo_add_node("server-b");
        assert_eq!(ring.xo_node_count(), 2);
        assert_eq!(ring.xo_virtual_nodes(), 200);
    }

    #[test]
    fn xo_53_ch_remove_node() {
        let mut ring = super::Xo53ConsistentHash::xo_new(50);
        ring.xo_add_node("alpha");
        ring.xo_add_node("beta");
        ring.xo_remove_node("alpha");
        assert_eq!(ring.xo_node_count(), 1);
        assert_eq!(ring.xo_virtual_nodes(), 50);
    }

    #[test]
    fn xo_53_ch_get_node() {
        let mut ring = super::Xo53ConsistentHash::xo_new(50);
        ring.xo_add_node("node-1");
        let result = ring.xo_get_node("some-key");
        assert_eq!(result, Some("node-1"));
    }

    #[test]
    fn xo_53_ch_empty_ring() {
        let ring = super::Xo53ConsistentHash::xo_new(10);
        assert_eq!(ring.xo_get_node("key"), None);
        assert_eq!(ring.xo_node_count(), 0);
    }

    #[test]
    fn xo_53_ch_distribution() {
        let mut ring = super::Xo53ConsistentHash::xo_new(100);
        ring.xo_add_node("s1");
        ring.xo_add_node("s2");
        let keys: Vec<&str> = vec!["k1", "k2", "k3", "k4", "k5", "k6"];
        let dist = ring.xo_key_distribution(&keys);
        let total: usize = dist.values().sum();
        assert_eq!(total, 6);
    }

    #[test]
    fn xo_53_ch_rebalance() {
        let mut ring = super::Xo53ConsistentHash::xo_new(100);
        ring.xo_add_node("n1");
        ring.xo_add_node("n2");
        ring.xo_add_node("n3");
        let rf = ring.xo_rebalance_factor();
        assert!(rf >= 0.0, "rebalance factor should be non-negative");
    }

    #[test]
    fn xo_53_ch_virtual_nodes() {
        let mut ring = super::Xo53ConsistentHash::xo_new(75);
        ring.xo_add_node("host1");
        ring.xo_add_node("host2");
        assert_eq!(ring.xo_virtual_nodes(), 150);
    }

    #[test]
    fn xo_53_ch_consistent_lookup() {
        let mut ring = super::Xo53ConsistentHash::xo_new(50);
        ring.xo_add_node("srv-a");
        ring.xo_add_node("srv-b");
        let first = ring.xo_get_node("stable-key").unwrap().to_string();
        let second = ring.xo_get_node("stable-key").unwrap().to_string();
        assert_eq!(first, second, "same key must map to same node");
    }

}