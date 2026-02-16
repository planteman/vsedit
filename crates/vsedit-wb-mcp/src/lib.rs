//! Model Context Protocol service.

use std::fmt;

/// A tool exposed by an MCP server.
#[derive(Debug, Clone, PartialEq)]
pub struct McpTool {
    pub name: String,
    pub description: String,
    pub input_schema: Option<String>,
}

impl fmt::Display for McpTool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.name, self.description)
    }
}

/// A resource exposed by an MCP server.
#[derive(Debug, Clone, PartialEq)]
pub struct McpResource {
    pub uri: String,
    pub name: String,
    pub description: Option<String>,
    pub mime_type: Option<String>,
}

/// An MCP server instance.
#[derive(Debug, Clone, PartialEq)]
pub struct McpServer {
    pub id: String,
    pub name: String,
    pub tools: Vec<McpTool>,
    pub resources: Vec<McpResource>,
    pub connected: bool,
}

impl McpServer {
    pub fn tool_count(&self) -> usize {
        self.tools.len()
    }

    pub fn has_tool(&self, name: &str) -> bool {
        self.tools.iter().any(|t| t.name == name)
    }
}

impl fmt::Display for McpServer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let status = if self.connected { "connected" } else { "disconnected" };
        write!(f, "{} ({})", self.name, status)
    }
}

/// Service for managing MCP servers and their tools.
pub struct McpService {
    servers: Vec<McpServer>,
}

impl McpService {
    pub fn new() -> Self {
        Self {
            servers: Vec::new(),
        }
    }

    pub fn add_server(&mut self, server: McpServer) {
        self.servers.push(server);
    }

    pub fn remove_server(&mut self, id: &str) -> bool {
        let len = self.servers.len();
        self.servers.retain(|s| s.id != id);
        self.servers.len() < len
    }

    pub fn connect(&mut self, id: &str) {
        if let Some(s) = self.servers.iter_mut().find(|s| s.id == id) {
            s.connected = true;
        }
    }

    pub fn disconnect(&mut self, id: &str) {
        if let Some(s) = self.servers.iter_mut().find(|s| s.id == id) {
            s.connected = false;
        }
    }

    pub fn get_server(&self, id: &str) -> Option<&McpServer> {
        self.servers.iter().find(|s| s.id == id)
    }

    pub fn list_tools(&self) -> Vec<&McpTool> {
        self.servers
            .iter()
            .filter(|s| s.connected)
            .flat_map(|s| &s.tools)
            .collect()
    }

    pub fn list_resources(&self) -> Vec<&McpResource> {
        self.servers
            .iter()
            .filter(|s| s.connected)
            .flat_map(|s| &s.resources)
            .collect()
    }

    pub fn get_server_mut(&mut self, id: &str) -> Option<&mut McpServer> {
        self.servers.iter_mut().find(|s| s.id == id)
    }

    pub fn find_tool(&self, name: &str) -> Option<(&McpServer, &McpTool)> {
        self.servers
            .iter()
            .filter(|s| s.connected)
            .find_map(|s| s.tools.iter().find(|t| t.name == name).map(|t| (s, t)))
    }

    pub fn server_count(&self) -> usize {
        self.servers.len()
    }

    pub fn connected_count(&self) -> usize {
        self.servers.iter().filter(|s| s.connected).count()
    }

    pub fn disconnected_count(&self) -> usize {
        self.servers.iter().filter(|s| !s.connected).count()
    }

    pub fn get_all_servers(&self) -> &[McpServer] {
        &self.servers
    }

    pub fn disconnect_all(&mut self) {
        for s in &mut self.servers {
            s.connected = false;
        }
    }
}

impl Default for McpService {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for McpResource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "McpResource({})", self.uri)
    }
}

/// A record of a single MCP tool invocation.
#[derive(Debug, Clone, PartialEq)]
pub struct McpToolInvocation {
    pub tool_name: String,
    pub server_id: String,
    pub arguments: String,
    pub timestamp: u64,
    pub success: Option<bool>,
}

/// Tracks a log of MCP tool invocations.
#[derive(Debug, Clone)]
pub struct McpInvocationLog {
    invocations: Vec<McpToolInvocation>,
}

impl McpInvocationLog {
    pub fn new() -> Self {
        Self {
            invocations: Vec::new(),
        }
    }

    pub fn record(&mut self, invocation: McpToolInvocation) {
        self.invocations.push(invocation);
    }

    pub fn get_all(&self) -> &[McpToolInvocation] {
        &self.invocations
    }

    pub fn get_by_tool(&self, tool_name: &str) -> Vec<&McpToolInvocation> {
        self.invocations
            .iter()
            .filter(|i| i.tool_name == tool_name)
            .collect()
    }

    pub fn get_by_server(&self, server_id: &str) -> Vec<&McpToolInvocation> {
        self.invocations
            .iter()
            .filter(|i| i.server_id == server_id)
            .collect()
    }

    pub fn success_count(&self) -> usize {
        self.invocations
            .iter()
            .filter(|i| i.success == Some(true))
            .count()
    }

    pub fn failure_count(&self) -> usize {
        self.invocations
            .iter()
            .filter(|i| i.success == Some(false))
            .count()
    }

    pub fn pending_count(&self) -> usize {
        self.invocations
            .iter()
            .filter(|i| i.success.is_none())
            .count()
    }

    pub fn clear(&mut self) {
        self.invocations.clear();
    }

    pub fn count(&self) -> usize {
        self.invocations.len()
    }

    pub fn get_by_time_range(&self, start: u64, end: u64) -> Vec<&McpToolInvocation> {
        self.invocations
            .iter()
            .filter(|i| i.timestamp >= start && i.timestamp <= end)
            .collect()
    }
}

impl Default for McpInvocationLog {
    fn default() -> Self {
        Self::new()
    }
}

/// Tracks the health of an MCP server.
#[derive(Debug, Clone, PartialEq)]
pub struct McpServerHealth {
    pub server_id: String,
    pub consecutive_failures: u32,
    pub last_success_ts: Option<u64>,
    pub last_failure_ts: Option<u64>,
    pub total_invocations: u64,
    total_failures: u64,
}

impl McpServerHealth {
    pub fn new(server_id: String) -> Self {
        Self {
            server_id,
            consecutive_failures: 0,
            last_success_ts: None,
            last_failure_ts: None,
            total_invocations: 0,
            total_failures: 0,
        }
    }

    pub fn record_success(&mut self, ts: u64) {
        self.total_invocations += 1;
        self.consecutive_failures = 0;
        self.last_success_ts = Some(ts);
    }

    pub fn record_failure(&mut self, ts: u64) {
        self.total_invocations += 1;
        self.total_failures += 1;
        self.consecutive_failures += 1;
        self.last_failure_ts = Some(ts);
    }

    pub fn is_healthy(&self) -> bool {
        self.consecutive_failures < 3
    }

    pub fn failure_rate(&self) -> f64 {
        if self.total_invocations > 0 {
            self.total_failures as f64 / self.total_invocations as f64
        } else {
            0.0
        }
    }
}

impl McpService {
    pub fn find_resource(&self, uri: &str) -> Option<(&McpServer, &McpResource)> {
        self.servers
            .iter()
            .filter(|s| s.connected)
            .find_map(|s| {
                s.resources
                    .iter()
                    .find(|r| r.uri == uri)
                    .map(|r| (s, r))
            })
    }

    pub fn total_tool_count(&self) -> usize {
        self.servers.iter().map(|s| s.tools.len()).sum()
    }

    pub fn total_resource_count(&self) -> usize {
        self.servers.iter().map(|s| s.resources.len()).sum()
    }

    pub fn get_connected_servers(&self) -> Vec<&McpServer> {
        self.servers.iter().filter(|s| s.connected).collect()
    }

    pub fn connect_all(&mut self) {
        for s in &mut self.servers {
            s.connected = true;
        }
    }
}

/// Accumulated statistics for wb-mcp operations.
#[derive(Debug, Clone, PartialEq)]
pub struct WbMcpStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl WbMcpStats {
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
    pub fn merge(&mut self, other: &WbMcpStats) {
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

impl Default for WbMcpStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for WbMcpStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "WbMcpStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for wb-mcp.
#[derive(Debug, Clone)]
pub struct WbMcpValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl WbMcpValidator {
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

impl Default for WbMcpValidator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_server(id: &str, tools: Vec<McpTool>) -> McpServer {
        McpServer {
            id: id.to_string(),
            name: format!("Server {id}"),
            tools,
            resources: vec![],
            connected: false,
        }
    }

    #[test]
    fn add_and_remove_server() {
        let mut svc = McpService::new();
        svc.add_server(make_server("s1", vec![]));
        assert_eq!(svc.server_count(), 1);
        assert!(svc.remove_server("s1"));
        assert_eq!(svc.server_count(), 0);
        assert!(!svc.remove_server("s1"));
    }

    #[test]
    fn connect_disconnect() {
        let mut svc = McpService::new();
        svc.add_server(make_server("s1", vec![]));
        assert!(!svc.get_server("s1").unwrap().connected);
        svc.connect("s1");
        assert!(svc.get_server("s1").unwrap().connected);
        svc.disconnect("s1");
        assert!(!svc.get_server("s1").unwrap().connected);
    }

    #[test]
    fn list_tools_only_connected() {
        let mut svc = McpService::new();
        let tool = McpTool {
            name: "read".to_string(),
            description: "Read a file".to_string(),
            input_schema: None,
        };
        svc.add_server(make_server("s1", vec![tool]));
        assert!(svc.list_tools().is_empty());
        svc.connect("s1");
        assert_eq!(svc.list_tools().len(), 1);
        assert_eq!(svc.list_tools()[0].name, "read");
    }

    fn make_tool(name: &str) -> McpTool {
        McpTool {
            name: name.to_string(),
            description: format!("{name} description"),
            input_schema: None,
        }
    }

    #[test]
    fn find_tool_across_servers() {
        let mut svc = McpService::new();
        svc.add_server(make_server("s1", vec![make_tool("alpha")]));
        svc.add_server(make_server("s2", vec![make_tool("beta")]));
        svc.connect("s1");
        svc.connect("s2");
        let (server, tool) = svc.find_tool("beta").unwrap();
        assert_eq!(server.id, "s2");
        assert_eq!(tool.name, "beta");
        assert!(svc.find_tool("gamma").is_none());
    }

    #[test]
    fn find_tool_ignores_disconnected() {
        let mut svc = McpService::new();
        svc.add_server(make_server("s1", vec![make_tool("alpha")]));
        assert!(svc.find_tool("alpha").is_none());
    }

    #[test]
    fn connected_and_disconnected_counts() {
        let mut svc = McpService::new();
        svc.add_server(make_server("s1", vec![]));
        svc.add_server(make_server("s2", vec![]));
        svc.add_server(make_server("s3", vec![]));
        assert_eq!(svc.connected_count(), 0);
        assert_eq!(svc.disconnected_count(), 3);
        svc.connect("s1");
        svc.connect("s2");
        assert_eq!(svc.connected_count(), 2);
        assert_eq!(svc.disconnected_count(), 1);
    }

    #[test]
    fn disconnect_all_servers() {
        let mut svc = McpService::new();
        svc.add_server(make_server("s1", vec![]));
        svc.add_server(make_server("s2", vec![]));
        svc.connect("s1");
        svc.connect("s2");
        assert_eq!(svc.connected_count(), 2);
        svc.disconnect_all();
        assert_eq!(svc.connected_count(), 0);
        assert_eq!(svc.disconnected_count(), 2);
    }

    #[test]
    fn get_server_mut_modifies() {
        let mut svc = McpService::new();
        svc.add_server(make_server("s1", vec![]));
        let server = svc.get_server_mut("s1").unwrap();
        server.tools.push(make_tool("new_tool"));
        assert_eq!(svc.get_server("s1").unwrap().tool_count(), 1);
        assert!(svc.get_server("s1").unwrap().has_tool("new_tool"));
    }

    #[test]
    fn list_resources_only_connected() {
        let mut svc = McpService::new();
        let mut server = make_server("s1", vec![]);
        server.resources.push(McpResource {
            uri: "file:///test.txt".to_string(),
            name: "test".to_string(),
            description: Some("A test resource".to_string()),
            mime_type: Some("text/plain".to_string()),
        });
        svc.add_server(server);
        assert!(svc.list_resources().is_empty());
        svc.connect("s1");
        assert_eq!(svc.list_resources().len(), 1);
        assert_eq!(svc.list_resources()[0].uri, "file:///test.txt");
    }

    #[test]
    fn server_display_format() {
        let mut server = make_server("s1", vec![]);
        assert_eq!(format!("{server}"), "Server s1 (disconnected)");
        server.connected = true;
        assert_eq!(format!("{server}"), "Server s1 (connected)");
    }

    #[test]
    fn tool_display_format() {
        let tool = make_tool("read_file");
        assert_eq!(format!("{tool}"), "read_file: read_file description");
    }

    #[test]
    fn get_all_servers_returns_slice() {
        let mut svc = McpService::new();
        svc.add_server(make_server("s1", vec![]));
        svc.add_server(make_server("s2", vec![]));
        let all = svc.get_all_servers();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].id, "s1");
        assert_eq!(all[1].id, "s2");
    }

    fn make_invocation(tool: &str, server: &str, ts: u64, success: Option<bool>) -> McpToolInvocation {
        McpToolInvocation {
            tool_name: tool.to_string(),
            server_id: server.to_string(),
            arguments: "{}".to_string(),
            timestamp: ts,
            success,
        }
    }

    fn make_server_with_resources(id: &str, resources: Vec<McpResource>) -> McpServer {
        McpServer {
            id: id.to_string(),
            name: format!("Server {id}"),
            tools: vec![],
            resources,
            connected: false,
        }
    }

    fn make_resource(uri: &str) -> McpResource {
        McpResource {
            uri: uri.to_string(),
            name: "res".to_string(),
            description: None,
            mime_type: None,
        }
    }

    #[test]
    fn test_invocation_log_record_and_get() {
        let mut log = McpInvocationLog::new();
        assert_eq!(log.count(), 0);
        log.record(make_invocation("read", "s1", 100, Some(true)));
        assert_eq!(log.count(), 1);
        assert_eq!(log.get_all()[0].tool_name, "read");
    }

    #[test]
    fn test_invocation_log_by_tool() {
        let mut log = McpInvocationLog::new();
        log.record(make_invocation("read", "s1", 100, Some(true)));
        log.record(make_invocation("write", "s1", 101, Some(true)));
        log.record(make_invocation("read", "s2", 102, Some(false)));
        let reads = log.get_by_tool("read");
        assert_eq!(reads.len(), 2);
        assert_eq!(reads[0].server_id, "s1");
        assert_eq!(reads[1].server_id, "s2");
    }

    #[test]
    fn test_invocation_log_by_server() {
        let mut log = McpInvocationLog::new();
        log.record(make_invocation("read", "s1", 100, Some(true)));
        log.record(make_invocation("write", "s2", 101, Some(true)));
        log.record(make_invocation("exec", "s1", 102, None));
        let s1 = log.get_by_server("s1");
        assert_eq!(s1.len(), 2);
        assert_eq!(s1[0].tool_name, "read");
        assert_eq!(s1[1].tool_name, "exec");
    }

    #[test]
    fn test_invocation_log_success_failure_pending() {
        let mut log = McpInvocationLog::new();
        log.record(make_invocation("a", "s1", 1, Some(true)));
        log.record(make_invocation("b", "s1", 2, Some(false)));
        log.record(make_invocation("c", "s1", 3, None));
        log.record(make_invocation("d", "s1", 4, Some(true)));
        assert_eq!(log.success_count(), 2);
        assert_eq!(log.failure_count(), 1);
        assert_eq!(log.pending_count(), 1);
    }

    #[test]
    fn test_invocation_log_time_range() {
        let mut log = McpInvocationLog::new();
        log.record(make_invocation("a", "s1", 10, Some(true)));
        log.record(make_invocation("b", "s1", 20, Some(true)));
        log.record(make_invocation("c", "s1", 30, Some(true)));
        log.record(make_invocation("d", "s1", 40, Some(true)));
        let range = log.get_by_time_range(15, 35);
        assert_eq!(range.len(), 2);
        assert_eq!(range[0].tool_name, "b");
        assert_eq!(range[1].tool_name, "c");
    }

    #[test]
    fn test_invocation_log_clear() {
        let mut log = McpInvocationLog::new();
        log.record(make_invocation("a", "s1", 1, Some(true)));
        log.record(make_invocation("b", "s1", 2, Some(false)));
        assert_eq!(log.count(), 2);
        log.clear();
        assert_eq!(log.count(), 0);
        assert!(log.get_all().is_empty());
    }

    #[test]
    fn test_server_health_success() {
        let mut health = McpServerHealth::new("s1".to_string());
        health.record_success(100);
        assert_eq!(health.total_invocations, 1);
        assert_eq!(health.last_success_ts, Some(100));
        assert_eq!(health.consecutive_failures, 0);
    }

    #[test]
    fn test_server_health_failure() {
        let mut health = McpServerHealth::new("s1".to_string());
        health.record_failure(200);
        assert_eq!(health.total_invocations, 1);
        assert_eq!(health.last_failure_ts, Some(200));
        assert_eq!(health.consecutive_failures, 1);
    }

    #[test]
    fn test_server_health_is_healthy() {
        let mut health = McpServerHealth::new("s1".to_string());
        assert!(health.is_healthy());
        health.record_failure(1);
        health.record_failure(2);
        assert!(health.is_healthy());
        health.record_failure(3);
        assert!(!health.is_healthy());
        health.record_success(4);
        assert!(health.is_healthy());
    }

    #[test]
    fn test_server_health_failure_rate() {
        let mut health = McpServerHealth::new("s1".to_string());
        assert_eq!(health.failure_rate(), 0.0);
        health.record_success(1);
        health.record_success(2);
        health.record_failure(3);
        health.record_failure(4);
        assert!((health.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_find_resource() {
        let mut svc = McpService::new();
        let mut server = make_server_with_resources("s1", vec![make_resource("file:///a.txt")]);
        server.connected = false;
        svc.add_server(server);
        assert!(svc.find_resource("file:///a.txt").is_none());
        svc.connect("s1");
        let (srv, res) = svc.find_resource("file:///a.txt").unwrap();
        assert_eq!(srv.id, "s1");
        assert_eq!(res.uri, "file:///a.txt");
        assert!(svc.find_resource("file:///missing.txt").is_none());
    }

    #[test]
    fn test_total_tool_count() {
        let mut svc = McpService::new();
        svc.add_server(make_server("s1", vec![make_tool("a"), make_tool("b")]));
        svc.add_server(make_server("s2", vec![make_tool("c")]));
        assert_eq!(svc.total_tool_count(), 3);
    }

    #[test]
    fn test_total_resource_count() {
        let mut svc = McpService::new();
        svc.add_server(make_server_with_resources(
            "s1",
            vec![make_resource("file:///a"), make_resource("file:///b")],
        ));
        svc.add_server(make_server_with_resources(
            "s2",
            vec![make_resource("file:///c")],
        ));
        assert_eq!(svc.total_resource_count(), 3);
    }

    #[test]
    fn test_connect_all() {
        let mut svc = McpService::new();
        svc.add_server(make_server("s1", vec![]));
        svc.add_server(make_server("s2", vec![]));
        svc.add_server(make_server("s3", vec![]));
        assert_eq!(svc.connected_count(), 0);
        svc.connect_all();
        assert_eq!(svc.connected_count(), 3);
    }

    #[test]
    fn test_get_connected_servers() {
        let mut svc = McpService::new();
        svc.add_server(make_server("s1", vec![]));
        svc.add_server(make_server("s2", vec![]));
        svc.add_server(make_server("s3", vec![]));
        svc.connect("s1");
        svc.connect("s3");
        let connected = svc.get_connected_servers();
        assert_eq!(connected.len(), 2);
        assert_eq!(connected[0].id, "s1");
        assert_eq!(connected[1].id, "s3");
    }

    #[test]
    fn test_mcp_resource_display() {
        let res = McpResource {
            uri: "file:///hello.txt".to_string(),
            name: "hello".to_string(),
            description: None,
            mime_type: None,
        };
        assert_eq!(format!("{res}"), "McpResource(file:///hello.txt)");
    }

    #[test]
    fn wb_mcp_stats_new_defaults() {
        let stats = WbMcpStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn wb_mcp_stats_record_success() {
        let mut stats = WbMcpStats::new();
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
    fn wb_mcp_stats_record_failure() {
        let mut stats = WbMcpStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn wb_mcp_stats_reset() {
        let mut stats = WbMcpStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn wb_mcp_stats_merge() {
        let mut a = WbMcpStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = WbMcpStats::new();
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
    fn wb_mcp_stats_display() {
        let mut stats = WbMcpStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn wb_mcp_stats_default() {
        let stats = WbMcpStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn wb_mcp_validator_accepts_valid_name() {
        let v = WbMcpValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn wb_mcp_validator_rejects_empty() {
        let v = WbMcpValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn wb_mcp_validator_rejects_too_long() {
        let v = WbMcpValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn wb_mcp_validator_forbidden_prefix() {
        let v = WbMcpValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn wb_mcp_validator_allowed_chars() {
        let v = WbMcpValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn wb_mcp_validator_range() {
        let v = WbMcpValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn wb_mcp_sanitize_removes_control() {
        let result = WbMcpValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn wb_mcp_truncate_short_string() {
        assert_eq!(WbMcpValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn wb_mcp_truncate_long_string() {
        let result = WbMcpValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn wb_mcp_is_ascii_printable() {
        assert!(WbMcpValidator::is_ascii_printable("Hello World 123"));
        assert!(!WbMcpValidator::is_ascii_printable("Hello\x00World"));
    }
}
