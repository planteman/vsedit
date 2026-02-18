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

// ---------------------------------------------------------------------------
// mcp_tool_invoke — MCP tool execution
// ---------------------------------------------------------------------------

/// The result of invoking an MCP tool.
#[derive(Debug, Clone, PartialEq)]
pub struct McpToolResult {
    pub tool_name: String,
    pub server_id: String,
    pub success: bool,
    pub output: String,
    pub duration_ms: u64,
}

impl McpToolResult {
    pub fn is_error(&self) -> bool {
        !self.success
    }
}

impl fmt::Display for McpToolResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let status = if self.success { "OK" } else { "FAILED" };
        write!(
            f,
            "[{}] {}.{}: {} ({}ms)",
            status, self.server_id, self.tool_name, self.output, self.duration_ms,
        )
    }
}

/// Request to invoke an MCP tool.
#[derive(Debug, Clone)]
pub struct McpToolInvokeRequest {
    pub server_id: String,
    pub tool_name: String,
    pub arguments: std::collections::HashMap<String, String>,
}

/// Validate and prepare an MCP tool invocation. Returns an error string if
/// the server or tool is not found / not connected.
pub fn mcp_tool_invoke(
    service: &McpService,
    request: &McpToolInvokeRequest,
) -> Result<McpToolResult, String> {
    let server = service
        .get_server(&request.server_id)
        .ok_or_else(|| format!("server not found: {}", request.server_id))?;
    if !server.connected {
        return Err(format!("server not connected: {}", request.server_id));
    }
    if !server.has_tool(&request.tool_name) {
        return Err(format!(
            "tool '{}' not found on server '{}'",
            request.tool_name, request.server_id,
        ));
    }
    // In a real implementation this would make an RPC call.
    // For now we return a simulated successful result.
    Ok(McpToolResult {
        tool_name: request.tool_name.clone(),
        server_id: request.server_id.clone(),
        success: true,
        output: format!("invoked {} with {} args", request.tool_name, request.arguments.len()),
        duration_ms: 0,
    })
}

/// List all invocable tools across all connected servers.
pub fn mcp_list_invocable(service: &McpService) -> Vec<(&McpServer, &McpTool)> {
    let mut result = Vec::new();
    for server in service.get_all_servers() {
        if server.connected {
            for tool in &server.tools {
                result.push((server, tool));
            }
        }
    }
    result
}


// ---------------------------------------------------------------------------
// McpTool helpers
// ---------------------------------------------------------------------------

impl McpTool {
    /// Create a new tool with name and description.
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            input_schema: None,
        }
    }

    /// Set the input schema.
    pub fn with_schema(mut self, schema: impl Into<String>) -> Self {
        self.input_schema = Some(schema.into());
        self
    }

    /// Returns true if this tool has an input schema.
    pub fn has_schema(&self) -> bool {
        self.input_schema.is_some()
    }
}

// ---------------------------------------------------------------------------
// McpResource helpers
// ---------------------------------------------------------------------------

impl McpResource {
    /// Create a new resource.
    pub fn new(uri: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            uri: uri.into(),
            name: name.into(),
            description: None,
            mime_type: None,
        }
    }

    /// Set the description.
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    /// Set the MIME type.
    pub fn with_mime_type(mut self, mime: impl Into<String>) -> Self {
        self.mime_type = Some(mime.into());
        self
    }

    /// Returns the file extension from the URI, if any.
    pub fn extension(&self) -> Option<String> {
        self.uri.rsplit('.').next().map(|s| s.to_lowercase())
    }
}

// ---------------------------------------------------------------------------
// McpServer helpers
// ---------------------------------------------------------------------------

impl McpServer {
    /// Create a new connected server with no tools or resources.
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            tools: Vec::new(),
            resources: Vec::new(),
            connected: true,
        }
    }

    /// Add a tool to this server.
    pub fn with_tool(mut self, tool: McpTool) -> Self {
        self.tools.push(tool);
        self
    }

    /// Add a resource to this server.
    pub fn with_resource(mut self, resource: McpResource) -> Self {
        self.resources.push(resource);
        self
    }

    /// Returns the total number of capabilities (tools + resources).
    pub fn capability_count(&self) -> usize {
        self.tools.len() + self.resources.len()
    }

    /// Find a tool by name.
    pub fn find_tool(&self, name: &str) -> Option<&McpTool> {
        self.tools.iter().find(|t| t.name == name)
    }

    /// Find a resource by URI.
    pub fn find_resource(&self, uri: &str) -> Option<&McpResource> {
        self.resources.iter().find(|r| r.uri == uri)
    }
}

// ---------------------------------------------------------------------------
// MCP service summary
// ---------------------------------------------------------------------------

/// Summary of the MCP service state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpServiceSummary {
    pub total_servers: usize,
    pub connected_servers: usize,
    pub total_tools: usize,
    pub total_resources: usize,
}

impl McpServiceSummary {
    /// Generate a summary from the service.
    pub fn from_service(service: &McpService) -> Self {
        let servers = service.get_all_servers();
        Self {
            total_servers: servers.len(),
            connected_servers: servers.iter().filter(|s| s.connected).count(),
            total_tools: servers.iter().map(|s| s.tools.len()).sum(),
            total_resources: servers.iter().map(|s| s.resources.len()).sum(),
        }
    }
}

impl fmt::Display for McpServiceSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} servers ({} connected), {} tools, {} resources",
            self.total_servers, self.connected_servers,
            self.total_tools, self.total_resources
        )
    }
}

/// Search for tools across all servers by name substring.
pub fn search_tools<'a>(service: &'a McpService, query: &str) -> Vec<(&'a McpServer, &'a McpTool)> {
    let query_lower = query.to_lowercase();
    let mut results = Vec::new();
    for server in service.get_all_servers() {
        for tool in &server.tools {
            if tool.name.to_lowercase().contains(&query_lower)
                || tool.description.to_lowercase().contains(&query_lower)
            {
                results.push((server, tool));
            }
        }
    }
    results
}


// ---------------------------------------------------------------------------
// McpConnectionConfig
// ---------------------------------------------------------------------------

/// Connection configuration for an MCP server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpConnectionConfig {
    pub host: String,
    pub port: u16,
    pub tls: bool,
    pub timeout_ms: u64,
}

impl McpConnectionConfig {
    pub fn new(host: impl Into<String>, port: u16) -> Self {
        Self {
            host: host.into(),
            port,
            tls: false,
            timeout_ms: 5000,
        }
    }

    pub fn with_tls(mut self, tls: bool) -> Self {
        self.tls = tls;
        self
    }

    pub fn with_timeout_ms(mut self, ms: u64) -> Self {
        self.timeout_ms = ms;
        self
    }

    /// Build a URI string from the config.
    pub fn uri(&self) -> String {
        let scheme = if self.tls { "https" } else { "http" };
        format!("{scheme}://{}:{}", self.host, self.port)
    }
}

impl fmt::Display for McpConnectionConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} (timeout {}ms)", self.uri(), self.timeout_ms)
    }
}

// ---------------------------------------------------------------------------
// MCP prompt templates
// ---------------------------------------------------------------------------

/// A prompt template exposed by an MCP server.
#[derive(Debug, Clone, PartialEq)]
pub struct McpPromptTemplate {
    pub name: String,
    pub description: String,
    pub template: String,
    pub parameters: Vec<String>,
}

impl McpPromptTemplate {
    /// Create a new prompt template.
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        template: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            template: template.into(),
            parameters: Vec::new(),
        }
    }

    /// Add a parameter name that the template accepts.
    pub fn with_parameter(mut self, param: impl Into<String>) -> Self {
        self.parameters.push(param.into());
        self
    }

    /// Render the template by replacing `{{param}}` placeholders with values.
    pub fn render(&self, values: &std::collections::HashMap<String, String>) -> Result<String, String> {
        let mut result = self.template.clone();
        for param in &self.parameters {
            let placeholder = format!("{{{{{}}}}}", param);
            match values.get(param) {
                Some(val) => result = result.replace(&placeholder, val),
                None => return Err(format!("missing parameter: {}", param)),
            }
        }
        Ok(result)
    }

    /// Return the set of parameter names found as `{{name}}` in the template string.
    pub fn extract_placeholders(&self) -> Vec<String> {
        let mut placeholders = Vec::new();
        let bytes = self.template.as_bytes();
        let mut i = 0;
        while i + 4 < bytes.len() {
            if bytes[i] == b'{' && bytes[i + 1] == b'{' {
                if let Some(end) = self.template[i + 2..].find("}}") {
                    let name = &self.template[i + 2..i + 2 + end];
                    if !name.is_empty() && !placeholders.contains(&name.to_string()) {
                        placeholders.push(name.to_string());
                    }
                    i += end + 4;
                    continue;
                }
            }
            i += 1;
        }
        placeholders
    }
}

impl fmt::Display for McpPromptTemplate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}({})", self.name, self.parameters.join(", "))
    }
}

// ---------------------------------------------------------------------------
// MCP message types & validation
// ---------------------------------------------------------------------------

/// Supported JSON-RPC method kinds in the MCP protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpMethodKind {
    Initialize,
    ToolsList,
    ToolsCall,
    ResourcesList,
    ResourcesRead,
    PromptsList,
    PromptsGet,
    Ping,
}

impl McpMethodKind {
    /// Parse a method string into its kind.
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "initialize" => Some(Self::Initialize),
            "tools/list" => Some(Self::ToolsList),
            "tools/call" => Some(Self::ToolsCall),
            "resources/list" => Some(Self::ResourcesList),
            "resources/read" => Some(Self::ResourcesRead),
            "prompts/list" => Some(Self::PromptsList),
            "prompts/get" => Some(Self::PromptsGet),
            "ping" => Some(Self::Ping),
            _ => None,
        }
    }

    /// Return the canonical method string.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Initialize => "initialize",
            Self::ToolsList => "tools/list",
            Self::ToolsCall => "tools/call",
            Self::ResourcesList => "resources/list",
            Self::ResourcesRead => "resources/read",
            Self::PromptsList => "prompts/list",
            Self::PromptsGet => "prompts/get",
            Self::Ping => "ping",
        }
    }
}

/// An MCP JSON-RPC message envelope for validation purposes.
#[derive(Debug, Clone, PartialEq)]
pub struct McpMessage {
    pub id: Option<u64>,
    pub method: String,
    pub params_json: Option<String>,
}

impl McpMessage {
    /// Validate structural correctness of the message.
    pub fn validate(&self) -> Result<(), String> {
        if self.method.is_empty() {
            return Err("method must not be empty".into());
        }
        if McpMethodKind::from_str(&self.method).is_none() {
            return Err(format!("unknown method: {}", self.method));
        }
        // Requests must have an id
        if self.id.is_none() {
            return Err("request id is required".into());
        }
        Ok(())
    }

    /// Return the parsed method kind, if valid.
    pub fn method_kind(&self) -> Option<McpMethodKind> {
        McpMethodKind::from_str(&self.method)
    }
}

// ---------------------------------------------------------------------------
// MCP capability negotiation
// ---------------------------------------------------------------------------

/// Capabilities that a client or server can advertise.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct McpCapabilities {
    pub tools: bool,
    pub resources: bool,
    pub prompts: bool,
    pub logging: bool,
}

impl McpCapabilities {
    /// Create capabilities with everything enabled.
    pub fn all() -> Self {
        Self { tools: true, resources: true, prompts: true, logging: true }
    }

    /// Create capabilities with nothing enabled.
    pub fn none() -> Self {
        Self::default()
    }

    /// Negotiate the intersection of two capability sets.
    pub fn negotiate(&self, other: &Self) -> Self {
        Self {
            tools: self.tools && other.tools,
            resources: self.resources && other.resources,
            prompts: self.prompts && other.prompts,
            logging: self.logging && other.logging,
        }
    }

    /// Return the number of enabled capabilities.
    pub fn enabled_count(&self) -> usize {
        [self.tools, self.resources, self.prompts, self.logging]
            .iter()
            .filter(|&&v| v)
            .count()
    }

    /// Check whether a specific method is allowed under these capabilities.
    pub fn allows_method(&self, method: McpMethodKind) -> bool {
        match method {
            McpMethodKind::ToolsList | McpMethodKind::ToolsCall => self.tools,
            McpMethodKind::ResourcesList | McpMethodKind::ResourcesRead => self.resources,
            McpMethodKind::PromptsList | McpMethodKind::PromptsGet => self.prompts,
            McpMethodKind::Initialize | McpMethodKind::Ping => true,
        }
    }
}

impl fmt::Display for McpCapabilities {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut parts = Vec::new();
        if self.tools { parts.push("tools"); }
        if self.resources { parts.push("resources"); }
        if self.prompts { parts.push("prompts"); }
        if self.logging { parts.push("logging"); }
        if parts.is_empty() {
            write!(f, "(none)")
        } else {
            write!(f, "{}", parts.join(", "))
        }
    }
}

// ---------------------------------------------------------------------------
// MCP request/response correlation
// ---------------------------------------------------------------------------

/// Tracks in-flight MCP requests for correlation with responses.
#[derive(Debug)]
pub struct McpRequestTracker {
    pending: std::collections::HashMap<u64, PendingRequest>,
    next_id: u64,
}

/// Metadata for a pending request.
#[derive(Debug, Clone)]
pub struct PendingRequest {
    pub id: u64,
    pub method: String,
    pub server_id: String,
    pub issued_at: u64,
}

impl McpRequestTracker {
    /// Create a new empty tracker.
    pub fn new() -> Self {
        Self {
            pending: std::collections::HashMap::new(),
            next_id: 1,
        }
    }

    /// Issue a new request, returning the assigned id.
    pub fn issue(&mut self, method: impl Into<String>, server_id: impl Into<String>, now: u64) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.pending.insert(id, PendingRequest {
            id,
            method: method.into(),
            server_id: server_id.into(),
            issued_at: now,
        });
        id
    }

    /// Complete a pending request, returning its metadata.
    pub fn complete(&mut self, id: u64) -> Option<PendingRequest> {
        self.pending.remove(&id)
    }

    /// Return the number of pending requests.
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    /// Return all requests that have been pending longer than `threshold` relative to `now`.
    pub fn stale_requests(&self, now: u64, threshold: u64) -> Vec<&PendingRequest> {
        self.pending
            .values()
            .filter(|r| now.saturating_sub(r.issued_at) > threshold)
            .collect()
    }

    /// Cancel all pending requests for a given server, returning the cancelled count.
    pub fn cancel_for_server(&mut self, server_id: &str) -> usize {
        let before = self.pending.len();
        self.pending.retain(|_, r| r.server_id != server_id);
        before - self.pending.len()
    }
}

impl Default for McpRequestTracker {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// McpToolValidator – input schema validation
// ---------------------------------------------------------------------------

/// The type of a field in a tool input schema.
#[derive(Debug, Clone, PartialEq)]
pub enum SchemaFieldType {
    String,
    Number,
    Boolean,
    Array,
    Object,
}

impl fmt::Display for SchemaFieldType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::String => write!(f, "string"),
            Self::Number => write!(f, "number"),
            Self::Boolean => write!(f, "boolean"),
            Self::Array => write!(f, "array"),
            Self::Object => write!(f, "object"),
        }
    }
}

/// Result of validating tool input against a schema.
#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub valid: bool,
    pub errors: Vec<String>,
}

impl ValidationResult {
    pub fn is_valid(&self) -> bool {
        self.valid
    }

    pub fn error_count(&self) -> usize {
        self.errors.len()
    }

    pub fn summary(&self) -> String {
        if self.valid {
            "Validation passed".to_string()
        } else {
            format!(
                "Validation failed with {} error(s): {}",
                self.errors.len(),
                self.errors.join("; ")
            )
        }
    }
}

impl fmt::Display for ValidationResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.summary())
    }
}

/// Validates tool input fields against a declared schema.
#[derive(Debug, Clone)]
pub struct McpToolValidator {
    pub required_fields: Vec<String>,
    pub field_types: std::collections::HashMap<String, SchemaFieldType>,
}

impl McpToolValidator {
    pub fn new() -> Self {
        Self {
            required_fields: Vec::new(),
            field_types: std::collections::HashMap::new(),
        }
    }

    /// Register a required field with its expected type.
    pub fn require_field(&mut self, name: &str, field_type: SchemaFieldType) {
        let name = name.to_string();
        if !self.required_fields.contains(&name) {
            self.required_fields.push(name.clone());
        }
        self.field_types.insert(name, field_type);
    }

    /// Validate an input map against the schema.
    pub fn validate(
        &self,
        input: &std::collections::HashMap<String, String>,
    ) -> ValidationResult {
        let mut errors = Vec::new();

        for field in &self.required_fields {
            if !input.contains_key(field) {
                errors.push(format!("missing required field '{field}'"));
            }
        }

        for (field, expected) in &self.field_types {
            if let Some(value) = input.get(field) {
                let ok = match expected {
                    SchemaFieldType::String => true,
                    SchemaFieldType::Number => value.parse::<f64>().is_ok(),
                    SchemaFieldType::Boolean => {
                        value == "true" || value == "false"
                    }
                    SchemaFieldType::Array => {
                        value.starts_with('[') && value.ends_with(']')
                    }
                    SchemaFieldType::Object => {
                        value.starts_with('{') && value.ends_with('}')
                    }
                };
                if !ok {
                    errors.push(format!(
                        "field '{field}' expected type {expected}, got '{value}'"
                    ));
                }
            }
        }

        ValidationResult {
            valid: errors.is_empty(),
            errors,
        }
    }
}

impl Default for McpToolValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// McpRetryPolicy – retry / back-off logic
// ---------------------------------------------------------------------------

/// Configurable retry policy with exponential back-off.
#[derive(Debug, Clone)]
pub struct McpRetryPolicy {
    pub max_retries: u32,
    pub base_delay_ms: u64,
    pub max_delay_ms: u64,
    pub backoff_factor: f64,
}

impl McpRetryPolicy {
    pub fn new(max_retries: u32) -> Self {
        Self {
            max_retries,
            base_delay_ms: 1000,
            max_delay_ms: 30_000,
            backoff_factor: 2.0,
        }
    }

    pub fn with_exponential_backoff(max: u32, base_ms: u64, factor: f64) -> Self {
        Self {
            max_retries: max,
            base_delay_ms: base_ms,
            max_delay_ms: u64::MAX,
            backoff_factor: factor,
        }
    }

    /// Compute the delay (in ms) for the given zero-based attempt, capped at
    /// `max_delay_ms`.
    pub fn delay_for_attempt(&self, attempt: u32) -> u64 {
        let delay =
            (self.base_delay_ms as f64 * self.backoff_factor.powi(attempt as i32)) as u64;
        delay.min(self.max_delay_ms)
    }

    pub fn should_retry(&self, attempt: u32) -> bool {
        attempt < self.max_retries
    }

    /// Sum of delays across all retry attempts.
    pub fn total_max_delay(&self) -> u64 {
        (0..self.max_retries)
            .map(|a| self.delay_for_attempt(a))
            .sum()
    }
}

// ---------------------------------------------------------------------------
// McpToolCatalog – searchable tool registry
// ---------------------------------------------------------------------------

/// A single entry in the tool catalog.
#[derive(Debug, Clone)]
pub struct CatalogEntry {
    pub tool_name: String,
    pub server_id: String,
    pub description: String,
    pub tags: Vec<String>,
}

/// Searchable catalog of tools across MCP servers.
#[derive(Debug, Clone)]
pub struct McpToolCatalog {
    pub tools: Vec<CatalogEntry>,
}

impl McpToolCatalog {
    pub fn new() -> Self {
        Self { tools: Vec::new() }
    }

    pub fn register(&mut self, entry: CatalogEntry) {
        self.tools.push(entry);
    }

    /// Case-insensitive search over tool name and description.
    pub fn search(&self, query: &str) -> Vec<&CatalogEntry> {
        let q = query.to_lowercase();
        self.tools
            .iter()
            .filter(|e| {
                e.tool_name.to_lowercase().contains(&q)
                    || e.description.to_lowercase().contains(&q)
            })
            .collect()
    }

    pub fn by_tag(&self, tag: &str) -> Vec<&CatalogEntry> {
        self.tools
            .iter()
            .filter(|e| e.tags.iter().any(|t| t == tag))
            .collect()
    }

    pub fn by_server(&self, server_id: &str) -> Vec<&CatalogEntry> {
        self.tools
            .iter()
            .filter(|e| e.server_id == server_id)
            .collect()
    }

    pub fn len(&self) -> usize {
        self.tools.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }

    /// Collect all unique tags present in the catalog.
    pub fn all_tags(&self) -> Vec<&str> {
        let mut tags: Vec<&str> =
            self.tools.iter().flat_map(|e| e.tags.iter().map(|t| t.as_str())).collect();
        tags.sort_unstable();
        tags.dedup();
        tags
    }
}

impl Default for McpToolCatalog {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// McpConnectionState – connection state machine
// ---------------------------------------------------------------------------

/// Phase of an MCP connection lifecycle.
#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionPhase {
    Disconnected,
    Connecting,
    Initializing,
    Ready,
    Error(String),
}

impl fmt::Display for ConnectionPhase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Disconnected => write!(f, "disconnected"),
            Self::Connecting => write!(f, "connecting"),
            Self::Initializing => write!(f, "initializing"),
            Self::Ready => write!(f, "ready"),
            Self::Error(msg) => write!(f, "error: {msg}"),
        }
    }
}

/// Tracks the current connection phase and enforces valid transitions.
#[derive(Debug, Clone)]
pub struct McpConnectionState {
    phase: ConnectionPhase,
}

impl McpConnectionState {
    pub fn new() -> Self {
        Self {
            phase: ConnectionPhase::Disconnected,
        }
    }

    /// Attempt a state transition. Returns `true` if the transition is valid.
    pub fn transition(&mut self, to: ConnectionPhase) -> bool {
        let valid = match (&self.phase, &to) {
            (_, ConnectionPhase::Disconnected) => true,
            (_, ConnectionPhase::Error(_)) => true,
            (ConnectionPhase::Disconnected, ConnectionPhase::Connecting) => true,
            (ConnectionPhase::Connecting, ConnectionPhase::Initializing) => true,
            (ConnectionPhase::Initializing, ConnectionPhase::Ready) => true,
            _ => false,
        };
        if valid {
            self.phase = to;
        }
        valid
    }

    pub fn phase(&self) -> &ConnectionPhase {
        &self.phase
    }

    pub fn is_ready(&self) -> bool {
        self.phase == ConnectionPhase::Ready
    }

    pub fn is_error(&self) -> bool {
        matches!(self.phase, ConnectionPhase::Error(_))
    }

    pub fn can_send(&self) -> bool {
        self.phase == ConnectionPhase::Ready
    }
}

impl Default for McpConnectionState {
    fn default() -> Self {
        Self::new()
    }
}


// ─── McpC LRU Cache ───────────────────────────────────────

/// A simple LRU cache for MCP results.
#[derive(Debug)]
pub struct McpCLruCache<V> {
    entries: Vec<(String, V)>,
    capacity: usize,
    hits: u64,
    misses: u64,
}

impl<V: Clone> McpCLruCache<V> {
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

impl<V: Clone + fmt::Display> fmt::Display for McpCLruCache<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "McpCLruCache(size={}, cap={}, hits={}, misses={})",
            self.len(), self.capacity, self.hits, self.misses)
    }
}

// ─── McpBuf Ring Buffer ──────────────────────────────────────

/// A fixed-capacity ring buffer for MCP messages.
#[derive(Debug, Clone)]
pub struct McpBufRingBuffer<T> {
    buf: Vec<Option<T>>,
    head: usize,
    len: usize,
}

impl<T: Clone> McpBufRingBuffer<T> {
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

impl<T: Clone + fmt::Display> fmt::Display for McpBufRingBuffer<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "McpBufRingBuffer(len={}, cap={})", self.len, self.capacity())
    }
}


// ---------------------------------------------------------------------------
// wb_mcp – Workbench state helpers
// ---------------------------------------------------------------------------

/// Layout region within the workbench.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum XWbMcpLayoutRegion {
    Sidebar,
    Panel,
    Editor,
    Statusbar,
    Titlebar,
    Auxiliary,
}

/// Visibility state for a workbench panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XWbMcpPanelState {
    pub region: XWbMcpLayoutRegion,
    pub visible: bool,
    pub width: u32,
    pub height: u32,
    pub label: String,
}

impl XWbMcpPanelState {
    pub fn new(region: XWbMcpLayoutRegion, label: impl Into<String>) -> Self {
        Self { region, visible: true, width: 300, height: 200, label: label.into() }
    }

    pub fn area(&self) -> u64 {
        self.width as u64 * self.height as u64
    }

    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    pub fn resize(&mut self, w: u32, h: u32) {
        self.width = w;
        self.height = h;
    }

    pub fn is_narrow(&self) -> bool {
        self.width < 200
    }
}

/// Compute the total visible area across a set of panels.
pub fn x_wb_mcp_total_visible_area(panels: &[XWbMcpPanelState]) -> u64 {
    panels.iter().filter(|p| p.visible).map(|p| p.area()).sum()
}

/// Count panels visible in a specific region.
pub fn x_wb_mcp_count_in_region(
    panels: &[XWbMcpPanelState],
    region: XWbMcpLayoutRegion,
) -> usize {
    panels.iter().filter(|p| p.region == region && p.visible).count()
}

/// Find the widest visible panel.
pub fn x_wb_mcp_widest_panel(panels: &[XWbMcpPanelState]) -> Option<&XWbMcpPanelState> {
    panels.iter().filter(|p| p.visible).max_by_key(|p| p.width)
}

/// Collapse all panels in a given region (set visible = false).
pub fn x_wb_mcp_collapse_region(
    panels: &mut [XWbMcpPanelState],
    region: XWbMcpLayoutRegion,
) {
    for p in panels.iter_mut() {
        if p.region == region {
            p.visible = false;
        }
    }
}

/// Layout constraint: minimum and maximum dimensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct XWbMcpLayoutConstraint {
    pub min_width: u32,
    pub max_width: u32,
    pub min_height: u32,
    pub max_height: u32,
}

impl XWbMcpLayoutConstraint {
    pub fn new(min_w: u32, max_w: u32, min_h: u32, max_h: u32) -> Self {
        Self { min_width: min_w, max_width: max_w, min_height: min_h, max_height: max_h }
    }

    /// Clamp a width value to this constraint's range.
    pub fn clamp_width(&self, w: u32) -> u32 {
        w.clamp(self.min_width, self.max_width)
    }

    /// Clamp a height value to this constraint's range.
    pub fn clamp_height(&self, h: u32) -> u32 {
        h.clamp(self.min_height, self.max_height)
    }

    /// Returns true if both dimensions are within the constraint.
    pub fn is_satisfied(&self, w: u32, h: u32) -> bool {
        w >= self.min_width && w <= self.max_width && h >= self.min_height && h <= self.max_height
    }
}



// ---------------------------------------------------------------------------
// wb_mcp – Extended domain helpers
// ---------------------------------------------------------------------------

/// Extended mode for MCP protocol handling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum YWbMcpMcpConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Error,
}

impl YWbMcpMcpConnectionState {
    /// Return an index for this variant (0-based).
    pub fn index(&self) -> usize {
        match self {
            Self::Disconnected => 0,
            Self::Connecting => 1,
            Self::Connected => 2,
            Self::Error => 3,
        }
    }

    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Disconnected => "Disconnected",
            Self::Connecting => "Connecting",
            Self::Connected => "Connected",
            Self::Error => "Error",
        }
    }

    /// List all variants.
    pub fn all() -> &'static [YWbMcpMcpConnectionState] {
        &[
            YWbMcpMcpConnectionState::Disconnected,
            YWbMcpMcpConnectionState::Connecting,
            YWbMcpMcpConnectionState::Connected,
            YWbMcpMcpConnectionState::Error,
        ]
    }

    /// Check if this is the first variant.
    pub fn is_default(&self) -> bool {
        self.index() == 0
    }
}

impl fmt::Display for YWbMcpMcpConnectionState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks MCP message queue data.
#[derive(Debug, Clone)]
pub struct YWbMcpMcpMessageQueue {
    pub messages: Vec<(u64, String)>,
    pub capacity: usize,
    pub dropped: u64,
}

impl YWbMcpMcpMessageQueue {
    /// Create a new instance with default values.
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
            capacity: 0,
            dropped: 0,
        }
    }

    /// Number of items.
    pub fn len(&self) -> usize {
        self.messages.len()
    }

    /// Whether the collection is empty.
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    /// Clear all items.
    pub fn clear(&mut self) {
        self.messages.clear();
    }

    /// Summary string for debugging.
    pub fn summary(&self) -> String {
        format!("YWbMcpMcpMessageQueue({}: {:?})", "messages", self.messages)
    }
}

/// Compute a hash-like fingerprint from a label string.
pub fn y_wb_mcp_fingerprint(label: &str) -> u64 {
    let mut h: u64 = 5381;
    for b in label.bytes() {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Truncate a string to at most `max_len` characters, appending '…' if truncated.
pub fn y_wb_mcp_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut t = s[..max_len].to_string();
        t.push('…');
        t
    }
}

/// Normalize a key string: lowercase and replace spaces with underscores.
pub fn y_wb_mcp_normalize_key(key: &str) -> String {
    key.to_lowercase().replace(' ', "_")
}

/// Split a dotted path into segments.
pub fn y_wb_mcp_split_path(path: &str) -> Vec<&str> {
    path.split('.').collect()
}

/// Count occurrences of `needle` in `haystack`.
pub fn y_wb_mcp_count_occurrences(haystack: &str, needle: &str) -> usize {
    if needle.is_empty() {
        return 0;
    }
    haystack.matches(needle).count()
}

/// Check whether `value` is within `[lo, hi]` inclusive.
pub fn y_wb_mcp_in_range(value: i64, lo: i64, hi: i64) -> bool {
    value >= lo && value <= hi
}

/// Deduplicate a sorted slice, returning a new Vec.
pub fn y_wb_mcp_dedup_sorted(items: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    for item in items {
        if result.last().map_or(true, |last: &String| last != item) {
            result.push(item.clone());
        }
    }
    result
}

/// Interleave two slices of strings.
pub fn y_wb_mcp_interleave<'a>(a: &'a [String], b: &'a [String]) -> Vec<&'a String> {
    let mut out = Vec::new();
    let max = a.len().max(b.len());
    for i in 0..max {
        if i < a.len() { out.push(&a[i]); }
        if i < b.len() { out.push(&b[i]); }
    }
    out
}



// ---------------------------------------------------------------------------
// wb_mcp – Extended MCP tool registry helpers
// ---------------------------------------------------------------------------

/// Priority levels for MCP tool registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZWbMcpPriority {
    Idle,
    Low,
    Normal,
    High,
    Realtime,
}

impl ZWbMcpPriority {
    /// Numeric weight (0–4).
    pub fn weight(&self) -> u8 {
        match self {
            Self::Idle => 0,
            Self::Low => 1,
            Self::Normal => 2,
            Self::High => 3,
            Self::Realtime => 4,
        }
    }

    /// Human-readable label for this priority.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Low => "low",
            Self::Normal => "normal",
            Self::High => "high",
            Self::Realtime => "realtime",
        }
    }

    /// Whether this priority is above Normal.
    pub fn is_elevated(&self) -> bool {
        self.weight() > 2
    }

    /// All variants in ascending order.
    pub fn all_asc() -> [ZWbMcpPriority; 5] {
        [Self::Idle, Self::Low, Self::Normal, Self::High, Self::Realtime]
    }
}

impl fmt::Display for ZWbMcpPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks MCP tool registry data.
#[derive(Debug, Clone)]
pub struct ZWbMcpMcpToolRegistry {
    pub tools: Vec<(String, bool)>,
    pub version: u32,
    pub locked: bool,
}

impl ZWbMcpMcpToolRegistry {
    /// Create with default values.
    pub fn new() -> Self {
        Self {
            tools: Vec::new(),
            version: 0,
            locked: false,
        }
    }

    /// Number of items in the primary collection.
    pub fn len(&self) -> usize {
        self.tools.len()
    }

    /// Whether the primary collection is empty.
    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }

    /// Clear the primary collection.
    pub fn clear(&mut self) {
        self.tools.clear();
    }

    /// Produce a debug summary string.
    pub fn summary(&self) -> String {
        format!("ZWbMcpMcpToolRegistry[version={:?}, locked={:?}]", self.version, self.locked)
    }

    /// Clone with the third field toggled (if bool) or kept as-is.
    pub fn toggled_clone(&self) -> Self {
        let mut c = self.clone();
        c.locked = !c.locked;
        c
    }
}

/// Compute a simple rolling hash for MCP tool registry.
pub fn z_wb_mcp_rolling_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Pad `s` to exactly `width` chars, truncating or right-padding with spaces.
pub fn z_wb_mcp_pad_to(s: &str, width: usize) -> String {
    if s.len() >= width {
        s[..width].to_string()
    } else {
        format!("{:<width$}", s, width = width)
    }
}

/// Check whether all characters in `s` are ASCII alphanumeric or underscore.
pub fn z_wb_mcp_is_identifier(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// Compute the Levenshtein distance between two strings (simple O(n*m) impl).
pub fn z_wb_mcp_levenshtein(a: &str, b: &str) -> usize {
    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();
    let m = a_bytes.len();
    let n = b_bytes.len();
    let mut prev: Vec<usize> = (0..=n).collect();
    let mut curr = vec![0usize; n + 1];
    for i in 1..=m {
        curr[0] = i;
        for j in 1..=n {
            let cost = if a_bytes[i - 1] == b_bytes[j - 1] { 0 } else { 1 };
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[n]
}

/// Extract unique words from a whitespace-separated string.
pub fn z_wb_mcp_unique_words(text: &str) -> Vec<&str> {
    let mut seen = std::collections::HashSet::new();
    text.split_whitespace().filter(|w| seen.insert(*w)).collect()
}

/// Chunk a slice into groups of `size`.
pub fn z_wb_mcp_chunk_slice<T>(slice: &[T], size: usize) -> Vec<&[T]> {
    if size == 0 { return vec![]; }
    slice.chunks(size).collect()
}

/// Return the longest common prefix of two strings.
pub fn z_wb_mcp_common_prefix<'a>(a: &'a str, b: &str) -> &'a str {
    let end = a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count();
    &a[..end]
}


// ---------------------------------------------------------------------------
// xb_ utilities – batch 76
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer76 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer76 {
    /// Create a new ring buffer with the given capacity.
    pub fn new(cap: usize) -> Self {
        Self {
            buf: vec![0i64; cap],
            cap,
            head: 0,
            len: 0,
        }
    }

    /// Push a value into the buffer, overwriting the oldest if full.
    pub fn push(&mut self, val: i64) {
        let pos = (self.head + self.len) % self.cap;
        self.buf[pos] = val;
        if self.len == self.cap {
            self.head = (self.head + 1) % self.cap;
        } else {
            self.len += 1;
        }
    }

    /// Return the number of elements currently stored.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Return whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Get element at logical index (0 = oldest).
    pub fn get(&self, index: usize) -> Option<i64> {
        if index >= self.len {
            return None;
        }
        Some(self.buf[(self.head + index) % self.cap])
    }

    /// Drain all elements oldest-first.
    pub fn drain_all(&mut self) -> Vec<i64> {
        let mut out = Vec::with_capacity(self.len);
        for i in 0..self.len {
            out.push(self.buf[(self.head + i) % self.cap]);
        }
        self.head = 0;
        self.len = 0;
        out
    }

    /// Peek at the oldest element.
    pub fn peek_front(&self) -> Option<i64> {
        self.get(0)
    }

    /// Peek at the newest element.
    pub fn peek_back(&self) -> Option<i64> {
        if self.len == 0 {
            None
        } else {
            self.get(self.len - 1)
        }
    }

    /// Clear the buffer.
    pub fn clear(&mut self) {
        self.head = 0;
        self.len = 0;
    }

    /// Return capacity.
    pub fn capacity(&self) -> usize {
        self.cap
    }
}

/// Compute a simple FNV-1a 64-bit hash over bytes.
pub fn xb_fnv1a_76(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_76<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
    let mut result = Vec::new();
    let mut i = 0;
    while i < items.len() {
        let val = &items[i];
        let mut count = 1;
        while i + count < items.len() && items[i + count] == *val {
            count += 1;
        }
        result.push((val.clone(), count));
        i += count;
    }
    result
}

/// Decode an RLE-encoded sequence.
pub fn xb_rle_decode_76<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_76(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_76(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 215
// ---------------------------------------------------------------------------

/// Generic object pool `Xc215Pool<T>`.
pub struct Xc215Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc215Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc215PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc215Pool<T> {
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
    pub fn stats(&self) -> Xc215PoolStats {
        Xc215PoolStats {
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

impl<T> Default for Xc215Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc215Scheduler`.
pub struct Xc215Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc215Scheduler {
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

impl Default for Xc215Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_215 hash for the given byte slice.
pub fn xc_215_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_215 convention.
pub fn xc_215_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe89 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe89Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe89PipelineError {
    pub stage: Xe89Stage,
    pub message: String,
}

impl std::fmt::Display for Xe89PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe89Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe89Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe89PipelineError>>>,
    stage_names: Vec<Xe89Stage>,
}

impl Xe89Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe89PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe89Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe89PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe89Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe89PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe89Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe89PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe89Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe89PipelineError> {
        let mut data = input;
        for (i, stage_fn) in self.stages.iter().enumerate() {
            data = stage_fn(data).map_err(|mut e| {
                e.stage = self.stage_names[i].clone();
                e
            })?;
        }
        Ok(data)
    }

    pub fn stage_count(&self) -> usize {
        self.stages.len()
    }

    pub fn compose(mut self, other: Xe89Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe89CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe89CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe89Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe89CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe89CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe89Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe89CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_89_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe89CacheEntry {
            value,
            inserted_at: self.current_time,
            ttl,
        });
    }

    pub fn get(&mut self, key: &K) -> Option<V> {
        let now = self.current_time;
        if let Some(entry) = self.entries.get(key) {
            if now - entry.inserted_at < entry.ttl {
                self.stats.hits += 1;
                return Some(entry.value.clone());
            } else {
                self.stats.misses += 1;
                let key_clone = key.clone();
                self.entries.remove(&key_clone);
                return None;
            }
        }
        self.stats.misses += 1;
        None
    }

    pub fn evict(&mut self, key: &K) -> bool {
        if self.entries.remove(key).is_some() {
            self.stats.evictions += 1;
            true
        } else {
            false
        }
    }

    fn xe_89_evict_expired(&mut self) {
        let now = self.current_time;
        let expired: Vec<K> = self.entries.iter()
            .filter(|(_, e)| now - e.inserted_at >= e.ttl)
            .map(|(k, _)| k.clone())
            .collect();
        for k in &expired {
            self.entries.remove(k);
            self.stats.evictions += 1;
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn stats(&self) -> &Xe89CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_89_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe89PipelineError> {
    Ok(data)
}

pub fn xe_89_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe89PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_89_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe89PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_89_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe89PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_89_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe89PipelineError> {
    Err(Xe89PipelineError {
        stage: Xe89Stage::Parse,
        message: "intentional failure".to_string(),
    })
}


// ---------------------------------------------------------------------------
// xg_87: Directed graph
// ---------------------------------------------------------------------------

/// A directed graph with adjacency-list representation.
#[derive(Debug, Clone)]
pub struct Xg87Graph {
    adj: std::collections::HashMap<usize, Vec<usize>>,
    edge_cnt: usize,
}

impl Xg87Graph {
    /// Create an empty graph.
    pub fn new() -> Self {
        Self { adj: std::collections::HashMap::new(), edge_cnt: 0 }
    }

    /// Add a node (idempotent).
    pub fn add_node(&mut self, id: usize) {
        self.adj.entry(id).or_default();
    }

    /// Add a directed edge from `src` to `dst`, creating nodes if needed.
    pub fn add_edge(&mut self, src: usize, dst: usize) {
        self.adj.entry(dst).or_default();
        self.adj.entry(src).or_default().push(dst);
        self.edge_cnt += 1;
    }

    /// Return the neighbours of `node`.
    pub fn neighbors(&self, node: usize) -> &[usize] {
        self.adj.get(&node).map_or(&[], |v| v.as_slice())
    }

    /// BFS reachability check.
    pub fn has_path(&self, from: usize, to: usize) -> bool {
        if from == to { return true; }
        let mut visited = std::collections::HashSet::new();
        let mut queue = std::collections::VecDeque::new();
        queue.push_back(from);
        visited.insert(from);
        while let Some(cur) = queue.pop_front() {
            for &nb in self.neighbors(cur) {
                if nb == to { return true; }
                if visited.insert(nb) {
                    queue.push_back(nb);
                }
            }
        }
        false
    }

    /// Kahn's algorithm topological sort. Returns `None` if a cycle exists.
    pub fn topological_sort(&self) -> Option<Vec<usize>> {
        let mut in_deg: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
        for &n in self.adj.keys() { in_deg.entry(n).or_insert(0); }
        for edges in self.adj.values() {
            for &dst in edges { *in_deg.entry(dst).or_insert(0) += 1; }
        }
        let mut queue: std::collections::VecDeque<usize> = in_deg.iter()
            .filter(|&(_, &d)| d == 0).map(|(&n, _)| n).collect();
        let mut order = Vec::new();
        while let Some(n) = queue.pop_front() {
            order.push(n);
            if let Some(edges) = self.adj.get(&n) {
                for &dst in edges {
                    if let Some(d) = in_deg.get_mut(&dst) {
                        *d -= 1;
                        if *d == 0 { queue.push_back(dst); }
                    }
                }
            }
        }
        if order.len() == self.adj.len() { Some(order) } else { None }
    }

    /// Detect whether the graph contains a cycle.
    pub fn cycle_detect(&self) -> bool {
        self.topological_sort().is_none()
    }

    /// Number of nodes.
    pub fn node_count(&self) -> usize { self.adj.len() }

    /// Number of edges.
    pub fn edge_count(&self) -> usize { self.edge_cnt }
}

impl Default for Xg87Graph {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// xg_87: Min-heap
// ---------------------------------------------------------------------------

/// A min-heap backed by a `Vec`.
#[derive(Debug, Clone)]
pub struct Xg87Heap<T: Ord> {
    data: Vec<T>,
}

impl<T: Ord> Xg87Heap<T> {
    /// Create an empty heap.
    pub fn new() -> Self { Self { data: Vec::new() } }

    /// Number of elements.
    pub fn len(&self) -> usize { self.data.len() }

    /// Whether the heap is empty.
    pub fn is_empty(&self) -> bool { self.data.is_empty() }

    /// Push a value onto the heap.
    pub fn push(&mut self, val: T) {
        self.data.push(val);
        self.sift_up(self.data.len() - 1);
    }

    /// Peek at the minimum element.
    pub fn peek(&self) -> Option<&T> { self.data.first() }

    /// Remove and return the minimum element.
    pub fn pop(&mut self) -> Option<T> {
        if self.data.is_empty() { return None; }
        let last = self.data.len() - 1;
        self.data.swap(0, last);
        let val = self.data.pop();
        if !self.data.is_empty() { self.sift_down(0); }
        val
    }

    /// Drain all elements in sorted order.
    pub fn drain_sorted(&mut self) -> Vec<T> {
        let mut out = Vec::with_capacity(self.data.len());
        while let Some(v) = self.pop() { out.push(v); }
        out
    }

    /// Merge another heap into this one.
    pub fn merge(&mut self, other: &mut Xg87Heap<T>) {
        self.data.append(&mut other.data);
        let n = self.data.len();
        for i in (0..n / 2).rev() { self.sift_down(i); }
    }

    fn sift_up(&mut self, mut idx: usize) {
        while idx > 0 {
            let parent = (idx - 1) / 2;
            if self.data[idx] < self.data[parent] {
                self.data.swap(idx, parent);
                idx = parent;
            } else { break; }
        }
    }

    fn sift_down(&mut self, mut idx: usize) {
        let len = self.data.len();
        loop {
            let mut smallest = idx;
            let left = 2 * idx + 1;
            let right = 2 * idx + 2;
            if left < len && self.data[left] < self.data[smallest] { smallest = left; }
            if right < len && self.data[right] < self.data[smallest] { smallest = right; }
            if smallest != idx { self.data.swap(idx, smallest); idx = smallest; }
            else { break; }
        }
    }
}

impl<T: Ord> Default for Xg87Heap<T> {
    fn default() -> Self { Self::new() }
}


/// A probabilistic sorted list using a skip-list structure (variant 214).
pub struct Xh214SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh214SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 256 as u64,
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

/// A compact bit set supporting boolean operations (variant 214).
pub struct Xh214BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh214BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 214).
pub struct Xi214Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi214Deque<T> {
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
pub struct Xi214Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi214Interval {
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

/// A simple interval tree (variant 214).
pub struct Xi214IntervalTree {
    xi_intervals: Vec<Xi214Interval>,
}

impl Xi214IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi214Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi214Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi214Interval) -> Vec<&Xi214Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi214Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi214Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi214Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi214Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi214Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi214Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 214) ---

/// Disjoint set / union-find for crate 214.
pub struct Xj214UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj214UnionFind {
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

const XJ214_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 214.
pub struct Xj214BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj214BTreeNode<K, V>>>,
    len: usize,
}

struct Xj214BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj214BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj214BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ214_BTREE_ORDER - 1
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
        let mid = XJ214_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj214BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj214BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj214BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj214BTreeNode::xj_new_leaf();
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


// --- xk_214 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk214SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk214SegmentTree {
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
pub struct Xk214DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk214DisjointIntervals {
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

    // -- mcp_tool_invoke tests ----------------------------------------------

    fn test_mcp_service() -> McpService {
        let mut svc = McpService::new();
        svc.add_server(McpServer {
            id: "srv1".into(),
            name: "Test Server".into(),
            tools: vec![
                McpTool { name: "search".into(), description: "Search files".into(), input_schema: None },
                McpTool { name: "read".into(), description: "Read file".into(), input_schema: None },
            ],
            resources: vec![],
            connected: true,
        });
        svc.add_server(McpServer {
            id: "srv2".into(),
            name: "Offline".into(),
            tools: vec![
                McpTool { name: "write".into(), description: "Write".into(), input_schema: None },
            ],
            resources: vec![],
            connected: false,
        });
        svc
    }

    #[test]
    fn invoke_tool_success() {
        let svc = test_mcp_service();
        let req = McpToolInvokeRequest {
            server_id: "srv1".into(),
            tool_name: "search".into(),
            arguments: std::collections::HashMap::new(),
        };
        let result = mcp_tool_invoke(&svc, &req).unwrap();
        assert!(result.success);
        assert_eq!(result.tool_name, "search");
    }

    #[test]
    fn invoke_tool_server_not_found() {
        let svc = test_mcp_service();
        let req = McpToolInvokeRequest {
            server_id: "missing".into(),
            tool_name: "search".into(),
            arguments: std::collections::HashMap::new(),
        };
        assert!(mcp_tool_invoke(&svc, &req).is_err());
    }

    #[test]
    fn invoke_tool_server_disconnected() {
        let svc = test_mcp_service();
        let req = McpToolInvokeRequest {
            server_id: "srv2".into(),
            tool_name: "write".into(),
            arguments: std::collections::HashMap::new(),
        };
        let err = mcp_tool_invoke(&svc, &req).unwrap_err();
        assert!(err.contains("not connected"));
    }

    #[test]
    fn invoke_tool_not_found() {
        let svc = test_mcp_service();
        let req = McpToolInvokeRequest {
            server_id: "srv1".into(),
            tool_name: "nonexistent".into(),
            arguments: std::collections::HashMap::new(),
        };
        let err = mcp_tool_invoke(&svc, &req).unwrap_err();
        assert!(err.contains("not found"));
    }

    #[test]
    fn tool_result_display() {
        let result = McpToolResult {
            tool_name: "search".into(),
            server_id: "srv1".into(),
            success: true,
            output: "found 3 files".into(),
            duration_ms: 42,
        };
        let s = format!("{result}");
        assert!(s.contains("[OK]"));
        assert!(s.contains("42ms"));
    }

    #[test]
    fn list_invocable_only_connected() {
        let svc = test_mcp_service();
        let invocable = mcp_list_invocable(&svc);
        assert_eq!(invocable.len(), 2); // only srv1's tools
        assert!(invocable.iter().all(|(s, _)| s.connected));
    }

    #[test]
    fn test_mcp_tool_new() {
        let t = McpTool::new("read", "Read a file");
        assert_eq!(t.name, "read");
        assert!(!t.has_schema());
    }

    #[test]
    fn test_mcp_tool_with_schema() {
        let t = McpTool::new("read", "Read").with_schema(r#"{"type":"object"}"#);
        assert!(t.has_schema());
    }

    #[test]
    fn test_mcp_resource_new() {
        let r = McpResource::new("file:///test.rs", "test.rs")
            .with_description("Test file")
            .with_mime_type("text/plain");
        assert_eq!(r.name, "test.rs");
        assert_eq!(r.description.as_deref(), Some("Test file"));
        assert_eq!(r.extension().as_deref(), Some("rs"));
    }

    #[test]
    fn test_mcp_resource_display_new() {
        let r = McpResource::new("file:///test.rs", "test.rs");
        assert!(format!("{r}").contains("test.rs"));
    }

    #[test]
    fn test_mcp_server_builder() {
        let s = McpServer::new("s1", "Server 1")
            .with_tool(McpTool::new("read", "Read"))
            .with_resource(McpResource::new("file:///a", "a"));
        assert_eq!(s.capability_count(), 2);
        assert!(s.find_tool("read").is_some());
        assert!(s.find_tool("write").is_none());
        assert!(s.find_resource("file:///a").is_some());
    }

    #[test]
    fn test_mcp_service_summary() {
        let mut svc = McpService::new();
        svc.add_server(McpServer::new("s1", "Server 1").with_tool(McpTool::new("t1", "Tool 1")));
        let summary = McpServiceSummary::from_service(&svc);
        assert_eq!(summary.total_servers, 1);
        assert_eq!(summary.connected_servers, 1);
        assert_eq!(summary.total_tools, 1);
        assert!(format!("{summary}").contains("1 servers"));
    }

    #[test]
    fn test_search_tools_fn() {
        let mut svc = McpService::new();
        svc.add_server(
            McpServer::new("s1", "S1")
                .with_tool(McpTool::new("file_read", "Read a file"))
                .with_tool(McpTool::new("web_search", "Search the web"))
        );
        let results = search_tools(&svc, "file");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].1.name, "file_read");
    }

    // --- new tests ---

    #[test]
    fn invocation_log_record_and_by_tool() {
        let mut log = McpInvocationLog::new();
        log.record(make_invocation("read", "s1", 100, Some(true)));
        log.record(make_invocation("write", "s1", 200, Some(true)));
        log.record(make_invocation("read", "s1", 300, None));
        assert_eq!(log.count(), 3);
        assert_eq!(log.get_by_tool("read").len(), 2);
        assert_eq!(log.get_by_server("s1").len(), 3);
    }

    #[test]
    fn invocation_log_clear_and_count() {
        let mut log = McpInvocationLog::new();
        log.record(make_invocation("x", "s1", 0, None));
        log.clear();
        assert_eq!(log.count(), 0);
    }

    #[test]
    fn server_health_record_and_healthy() {
        let mut h = McpServerHealth::new("s1".to_string());
        assert!(h.is_healthy());
        h.record_success(100);
        h.record_success(200);
        assert!(h.is_healthy());
        assert_eq!(h.total_invocations, 2);
        assert!((h.failure_rate() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn server_health_failures_mark_unhealthy() {
        let mut h = McpServerHealth::new("s1".to_string());
        h.record_failure(100);
        h.record_failure(200);
        h.record_failure(300);
        assert!(!h.is_healthy());
        assert_eq!(h.consecutive_failures, 3);
    }

    #[test]
    fn connection_config_uri() {
        let cfg = McpConnectionConfig::new("localhost", 8080).with_tls(true).with_timeout_ms(3000);
        assert_eq!(cfg.uri(), "https://localhost:8080");
        assert_eq!(cfg.timeout_ms, 3000);
        let s = format!("{cfg}");
        assert!(s.contains("https"));
    }

    #[test]
    fn connection_config_default() {
        let cfg = McpConnectionConfig::new("example.com", 443);
        assert!(!cfg.tls);
        assert_eq!(cfg.timeout_ms, 5000);
        assert_eq!(cfg.uri(), "http://example.com:443");
    }

    // --- prompt template tests ---

    #[test]
    fn prompt_template_render_success() {
        let tmpl = McpPromptTemplate::new("greet", "Greet user", "Hello, {{name}}! Welcome to {{place}}.")
            .with_parameter("name")
            .with_parameter("place");
        let mut vals = std::collections::HashMap::new();
        vals.insert("name".into(), "Alice".into());
        vals.insert("place".into(), "Wonderland".into());
        let rendered = tmpl.render(&vals).unwrap();
        assert_eq!(rendered, "Hello, Alice! Welcome to Wonderland.");
    }

    #[test]
    fn prompt_template_render_missing_param() {
        let tmpl = McpPromptTemplate::new("greet", "Greet", "Hello {{name}}")
            .with_parameter("name");
        let vals = std::collections::HashMap::new();
        assert!(tmpl.render(&vals).is_err());
    }

    #[test]
    fn prompt_template_extract_placeholders() {
        let tmpl = McpPromptTemplate::new("t", "d", "{{a}} and {{b}} and {{a}}");
        let phs = tmpl.extract_placeholders();
        assert_eq!(phs, vec!["a".to_string(), "b".to_string()]);
    }

    #[test]
    fn prompt_template_display() {
        let tmpl = McpPromptTemplate::new("summarize", "Summarize", "{{text}}")
            .with_parameter("text")
            .with_parameter("length");
        assert_eq!(format!("{tmpl}"), "summarize(text, length)");
    }

    // --- MCP message validation tests ---

    #[test]
    fn mcp_message_validate_valid() {
        let msg = McpMessage { id: Some(1), method: "tools/list".into(), params_json: None };
        assert!(msg.validate().is_ok());
        assert_eq!(msg.method_kind(), Some(McpMethodKind::ToolsList));
    }

    #[test]
    fn mcp_message_validate_empty_method() {
        let msg = McpMessage { id: Some(1), method: "".into(), params_json: None };
        assert!(msg.validate().unwrap_err().contains("empty"));
    }

    #[test]
    fn mcp_message_validate_unknown_method() {
        let msg = McpMessage { id: Some(1), method: "foo/bar".into(), params_json: None };
        assert!(msg.validate().unwrap_err().contains("unknown"));
    }

    #[test]
    fn mcp_message_validate_missing_id() {
        let msg = McpMessage { id: None, method: "ping".into(), params_json: None };
        assert!(msg.validate().unwrap_err().contains("id"));
    }

    // --- method kind round-trip ---

    #[test]
    fn mcp_method_kind_round_trip() {
        let methods = [
            "initialize", "tools/list", "tools/call",
            "resources/list", "resources/read",
            "prompts/list", "prompts/get", "ping",
        ];
        for m in methods {
            let kind = McpMethodKind::from_str(m).unwrap();
            assert_eq!(kind.as_str(), m);
        }
        assert!(McpMethodKind::from_str("unknown").is_none());
    }

    // --- capability negotiation tests ---

    #[test]
    fn capabilities_negotiate_intersection() {
        let client = McpCapabilities { tools: true, resources: true, prompts: false, logging: true };
        let server = McpCapabilities { tools: true, resources: false, prompts: true, logging: true };
        let result = client.negotiate(&server);
        assert!(result.tools);
        assert!(!result.resources);
        assert!(!result.prompts);
        assert!(result.logging);
        assert_eq!(result.enabled_count(), 2);
    }

    #[test]
    fn capabilities_all_and_none() {
        let all = McpCapabilities::all();
        assert_eq!(all.enabled_count(), 4);
        let none = McpCapabilities::none();
        assert_eq!(none.enabled_count(), 0);
        let negotiated = all.negotiate(&none);
        assert_eq!(negotiated.enabled_count(), 0);
    }

    #[test]
    fn capabilities_allows_method() {
        let caps = McpCapabilities { tools: true, resources: false, prompts: false, logging: false };
        assert!(caps.allows_method(McpMethodKind::ToolsList));
        assert!(caps.allows_method(McpMethodKind::ToolsCall));
        assert!(!caps.allows_method(McpMethodKind::ResourcesList));
        assert!(!caps.allows_method(McpMethodKind::PromptsGet));
        // Initialize and Ping are always allowed
        assert!(caps.allows_method(McpMethodKind::Initialize));
        assert!(caps.allows_method(McpMethodKind::Ping));
    }

    #[test]
    fn capabilities_display() {
        let caps = McpCapabilities { tools: true, resources: false, prompts: true, logging: false };
        assert_eq!(format!("{caps}"), "tools, prompts");
        let none = McpCapabilities::none();
        assert_eq!(format!("{none}"), "(none)");
    }

    // --- request tracker tests ---

    #[test]
    fn request_tracker_issue_and_complete() {
        let mut tracker = McpRequestTracker::new();
        let id1 = tracker.issue("tools/list", "srv1", 100);
        let id2 = tracker.issue("ping", "srv1", 101);
        assert_eq!(tracker.pending_count(), 2);
        assert_ne!(id1, id2);

        let req = tracker.complete(id1).unwrap();
        assert_eq!(req.method, "tools/list");
        assert_eq!(req.server_id, "srv1");
        assert_eq!(tracker.pending_count(), 1);

        assert!(tracker.complete(id1).is_none()); // already completed
    }

    #[test]
    fn request_tracker_stale_requests() {
        let mut tracker = McpRequestTracker::new();
        tracker.issue("tools/list", "srv1", 100);
        tracker.issue("ping", "srv2", 200);
        let stale = tracker.stale_requests(250, 100);
        assert_eq!(stale.len(), 1);
        assert_eq!(stale[0].server_id, "srv1");
    }

    #[test]
    fn request_tracker_cancel_for_server() {
        let mut tracker = McpRequestTracker::new();
        tracker.issue("tools/list", "srv1", 100);
        tracker.issue("ping", "srv1", 101);
        tracker.issue("resources/list", "srv2", 102);
        assert_eq!(tracker.cancel_for_server("srv1"), 2);
        assert_eq!(tracker.pending_count(), 1);
    }

    // --- McpToolValidator tests ---

    #[test]
    fn validator_accepts_valid_input() {
        let mut v = McpToolValidator::new();
        v.require_field("name", SchemaFieldType::String);
        v.require_field("count", SchemaFieldType::Number);
        let mut input = std::collections::HashMap::new();
        input.insert("name".into(), "alice".into());
        input.insert("count".into(), "42".into());
        let res = v.validate(&input);
        assert!(res.is_valid());
        assert_eq!(res.error_count(), 0);
    }

    #[test]
    fn validator_detects_missing_field() {
        let mut v = McpToolValidator::new();
        v.require_field("name", SchemaFieldType::String);
        let input = std::collections::HashMap::new();
        let res = v.validate(&input);
        assert!(!res.is_valid());
        assert_eq!(res.error_count(), 1);
        assert!(res.summary().contains("missing"));
    }

    #[test]
    fn validator_detects_wrong_type() {
        let mut v = McpToolValidator::new();
        v.require_field("flag", SchemaFieldType::Boolean);
        let mut input = std::collections::HashMap::new();
        input.insert("flag".into(), "notbool".into());
        let res = v.validate(&input);
        assert!(!res.is_valid());
        assert!(res.errors[0].contains("expected type boolean"));
    }

    #[test]
    fn schema_field_type_display() {
        assert_eq!(format!("{}", SchemaFieldType::Array), "array");
        assert_eq!(format!("{}", SchemaFieldType::Object), "object");
    }

    // --- McpRetryPolicy tests ---

    #[test]
    fn retry_policy_basic() {
        let p = McpRetryPolicy::new(3);
        assert!(p.should_retry(0));
        assert!(p.should_retry(2));
        assert!(!p.should_retry(3));
    }

    #[test]
    fn retry_policy_exponential_delay() {
        let p = McpRetryPolicy::with_exponential_backoff(5, 100, 2.0);
        assert_eq!(p.delay_for_attempt(0), 100);
        assert_eq!(p.delay_for_attempt(1), 200);
        assert_eq!(p.delay_for_attempt(2), 400);
    }

    #[test]
    fn retry_policy_cap_at_max_delay() {
        let mut p = McpRetryPolicy::new(5);
        p.max_delay_ms = 500;
        p.base_delay_ms = 100;
        p.backoff_factor = 10.0;
        // attempt 2 => 100*100 = 10000, capped to 500
        assert_eq!(p.delay_for_attempt(2), 500);
    }

    #[test]
    fn retry_policy_total_max_delay() {
        let p = McpRetryPolicy::with_exponential_backoff(3, 100, 2.0);
        // attempts 0,1,2 => 100 + 200 + 400 = 700
        assert_eq!(p.total_max_delay(), 700);
    }

    // --- McpToolCatalog tests ---

    #[test]
    fn catalog_search_case_insensitive() {
        let mut cat = McpToolCatalog::new();
        cat.register(CatalogEntry {
            tool_name: "RunQuery".into(),
            server_id: "s1".into(),
            description: "Execute a database query".into(),
            tags: vec!["db".into()],
        });
        assert_eq!(cat.search("runquery").len(), 1);
        assert_eq!(cat.search("DATABASE").len(), 1);
        assert_eq!(cat.search("missing").len(), 0);
    }

    #[test]
    fn catalog_by_tag_and_server() {
        let mut cat = McpToolCatalog::new();
        cat.register(CatalogEntry {
            tool_name: "t1".into(),
            server_id: "s1".into(),
            description: "".into(),
            tags: vec!["a".into(), "b".into()],
        });
        cat.register(CatalogEntry {
            tool_name: "t2".into(),
            server_id: "s2".into(),
            description: "".into(),
            tags: vec!["b".into()],
        });
        assert_eq!(cat.by_tag("a").len(), 1);
        assert_eq!(cat.by_tag("b").len(), 2);
        assert_eq!(cat.by_server("s1").len(), 1);
        assert_eq!(cat.len(), 2);
        assert_eq!(cat.all_tags(), vec!["a", "b"]);
    }

    // --- McpConnectionState tests ---

    #[test]
    fn connection_state_valid_transitions() {
        let mut s = McpConnectionState::new();
        assert_eq!(*s.phase(), ConnectionPhase::Disconnected);
        assert!(s.transition(ConnectionPhase::Connecting));
        assert!(s.transition(ConnectionPhase::Initializing));
        assert!(!s.can_send());
        assert!(s.transition(ConnectionPhase::Ready));
        assert!(s.is_ready());
        assert!(s.can_send());
    }

    #[test]
    fn connection_state_invalid_transition_rejected() {
        let mut s = McpConnectionState::new();
        // Disconnected -> Ready is invalid
        assert!(!s.transition(ConnectionPhase::Ready));
        assert_eq!(*s.phase(), ConnectionPhase::Disconnected);
    }

    #[test]
    fn connection_state_error_from_any() {
        let mut s = McpConnectionState::new();
        assert!(s.transition(ConnectionPhase::Connecting));
        assert!(s.transition(ConnectionPhase::Error("timeout".into())));
        assert!(s.is_error());
        assert!(!s.can_send());
        // can go back to disconnected from error
        assert!(s.transition(ConnectionPhase::Disconnected));
    }

    #[test]
    fn connection_phase_display() {
        assert_eq!(format!("{}", ConnectionPhase::Ready), "ready");
        assert_eq!(
            format!("{}", ConnectionPhase::Error("boom".into())),
            "error: boom"
        );
    }


    #[test]
    fn mcpc_lru_insert_get() {
        let mut c = McpCLruCache::new(3);
        c.insert("a", 1); c.insert("b", 2); c.insert("c", 3);
        assert_eq!(c.get("a"), Some(&1));
        assert_eq!(c.get("b"), Some(&2));
        assert_eq!(c.len(), 3);
    }

    #[test]
    fn mcpc_lru_eviction() {
        let mut c = McpCLruCache::new(2);
        c.insert("a", 1); c.insert("b", 2);
        let ev = c.insert("c", 3);
        assert!(ev.is_some());
        assert_eq!(ev.unwrap().0, "a");
        assert!(!c.contains("a"));
    }

    #[test]
    fn mcpc_lru_hit_ratio() {
        let mut c = McpCLruCache::new(5);
        c.insert("x", 10);
        c.get("x"); c.get("y");
        assert!(c.hit_ratio() > 0.4 && c.hit_ratio() < 0.6);
    }

    #[test]
    fn mcpc_lru_clear() {
        let mut c = McpCLruCache::new(3);
        c.insert("a", 1); c.insert("b", 2);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.hits(), 0);
    }

    #[test]
    fn mcpc_lru_remove() {
        let mut c = McpCLruCache::new(3);
        c.insert("a", 100);
        assert_eq!(c.remove("a"), Some(100));
        assert!(!c.contains("a"));
    }

    #[test]
    fn mcpc_lru_peek() {
        let mut c = McpCLruCache::new(3);
        c.insert("x", 42);
        assert_eq!(c.peek("x"), Some(&42));
        assert_eq!(c.misses(), 0);
    }

    #[test]
    fn mcpbuf_ringbuf_push_get() {
        let mut rb = McpBufRingBuffer::new(3);
        rb.push(10); rb.push(20); rb.push(30);
        assert_eq!(rb.get(0), Some(&10));
        assert_eq!(rb.get(2), Some(&30));
        assert_eq!(rb.len(), 3);
    }

    #[test]
    fn mcpbuf_ringbuf_overflow() {
        let mut rb = McpBufRingBuffer::<i32>::new(2);
        rb.push(1); rb.push(2); rb.push(3);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(&2));
        assert_eq!(rb.get(1), Some(&3));
    }

    #[test]
    fn mcpbuf_ringbuf_clear() {
        let mut rb = McpBufRingBuffer::new(5);
        rb.push("a".to_string()); rb.push("b".to_string());
        rb.clear();
        assert!(rb.is_empty());
    }

    #[test]
    fn mcpbuf_ringbuf_newest_oldest() {
        let mut rb = McpBufRingBuffer::new(4);
        rb.push(100); rb.push(200); rb.push(300);
        assert_eq!(rb.oldest(), Some(&100));
        assert_eq!(rb.newest(), Some(&300));
    }

    #[test]
    fn mcpbuf_ringbuf_to_vec() {
        let mut rb = McpBufRingBuffer::new(3);
        rb.push(1); rb.push(2);
        assert_eq!(rb.to_vec(), vec![1, 2]);
    }

    #[test]
    fn mcpbuf_ringbuf_is_full() {
        let mut rb = McpBufRingBuffer::new(2);
        assert!(!rb.is_full());
        rb.push(1); rb.push(2);
        assert!(rb.is_full());
    }


    // -- wb_mcp additional tests -------------------------------------------

    #[test]
    fn x_wb_mcp_panel_state_new() {
        let p = XWbMcpPanelState::new(XWbMcpLayoutRegion::Sidebar, "Explorer");
        assert!(p.visible);
        assert_eq!(p.label, "Explorer");
        assert_eq!(p.region, XWbMcpLayoutRegion::Sidebar);
    }

    #[test]
    fn x_wb_mcp_panel_area() {
        let p = XWbMcpPanelState::new(XWbMcpLayoutRegion::Editor, "ed");
        assert_eq!(p.area(), 300 * 200);
    }

    #[test]
    fn x_wb_mcp_panel_toggle() {
        let mut p = XWbMcpPanelState::new(XWbMcpLayoutRegion::Panel, "terminal");
        assert!(p.visible);
        p.toggle();
        assert!(!p.visible);
        p.toggle();
        assert!(p.visible);
    }

    #[test]
    fn x_wb_mcp_panel_resize() {
        let mut p = XWbMcpPanelState::new(XWbMcpLayoutRegion::Sidebar, "files");
        p.resize(400, 600);
        assert_eq!(p.width, 400);
        assert_eq!(p.height, 600);
        assert_eq!(p.area(), 240_000);
    }

    #[test]
    fn x_wb_mcp_panel_is_narrow() {
        let mut p = XWbMcpPanelState::new(XWbMcpLayoutRegion::Sidebar, "x");
        assert!(!p.is_narrow());
        p.resize(100, 200);
        assert!(p.is_narrow());
    }

    #[test]
    fn x_wb_mcp_total_visible_area_basic() {
        let panels = vec![
            XWbMcpPanelState::new(XWbMcpLayoutRegion::Sidebar, "a"),
            XWbMcpPanelState::new(XWbMcpLayoutRegion::Editor, "b"),
        ];
        assert_eq!(x_wb_mcp_total_visible_area(&panels), 2 * 300 * 200);
    }

    #[test]
    fn x_wb_mcp_total_visible_area_hidden() {
        let mut panels = vec![
            XWbMcpPanelState::new(XWbMcpLayoutRegion::Sidebar, "a"),
            XWbMcpPanelState::new(XWbMcpLayoutRegion::Panel, "b"),
        ];
        panels[1].visible = false;
        assert_eq!(x_wb_mcp_total_visible_area(&panels), 300 * 200);
    }

    #[test]
    fn x_wb_mcp_count_in_region_basic() {
        let panels = vec![
            XWbMcpPanelState::new(XWbMcpLayoutRegion::Sidebar, "a"),
            XWbMcpPanelState::new(XWbMcpLayoutRegion::Sidebar, "b"),
            XWbMcpPanelState::new(XWbMcpLayoutRegion::Editor, "c"),
        ];
        assert_eq!(x_wb_mcp_count_in_region(&panels, XWbMcpLayoutRegion::Sidebar), 2);
        assert_eq!(x_wb_mcp_count_in_region(&panels, XWbMcpLayoutRegion::Editor), 1);
        assert_eq!(x_wb_mcp_count_in_region(&panels, XWbMcpLayoutRegion::Panel), 0);
    }

    #[test]
    fn x_wb_mcp_widest_panel_basic() {
        let mut panels = vec![
            XWbMcpPanelState::new(XWbMcpLayoutRegion::Sidebar, "narrow"),
            XWbMcpPanelState::new(XWbMcpLayoutRegion::Editor, "wide"),
        ];
        panels[1].resize(800, 600);
        let widest = x_wb_mcp_widest_panel(&panels).unwrap();
        assert_eq!(widest.label, "wide");
    }

    #[test]
    fn x_wb_mcp_collapse_region_basic() {
        let mut panels = vec![
            XWbMcpPanelState::new(XWbMcpLayoutRegion::Sidebar, "a"),
            XWbMcpPanelState::new(XWbMcpLayoutRegion::Sidebar, "b"),
            XWbMcpPanelState::new(XWbMcpLayoutRegion::Editor, "c"),
        ];
        x_wb_mcp_collapse_region(&mut panels, XWbMcpLayoutRegion::Sidebar);
        assert!(!panels[0].visible);
        assert!(!panels[1].visible);
        assert!(panels[2].visible);
    }

    #[test]
    fn x_wb_mcp_layout_constraint_clamp() {
        let lc = XWbMcpLayoutConstraint::new(100, 800, 50, 600);
        assert_eq!(lc.clamp_width(50), 100);
        assert_eq!(lc.clamp_width(500), 500);
        assert_eq!(lc.clamp_width(1000), 800);
        assert_eq!(lc.clamp_height(10), 50);
    }

    #[test]
    fn x_wb_mcp_layout_constraint_satisfied() {
        let lc = XWbMcpLayoutConstraint::new(100, 800, 50, 600);
        assert!(lc.is_satisfied(400, 300));
        assert!(!lc.is_satisfied(50, 300));
        assert!(!lc.is_satisfied(400, 700));
    }

    #[test]
    fn x_wb_mcp_widest_panel_empty() {
        let panels: Vec<XWbMcpPanelState> = vec![];
        assert!(x_wb_mcp_widest_panel(&panels).is_none());
    }

    #[test]
    fn x_wb_mcp_layout_region_eq() {
        assert_eq!(XWbMcpLayoutRegion::Sidebar, XWbMcpLayoutRegion::Sidebar);
        assert_ne!(XWbMcpLayoutRegion::Sidebar, XWbMcpLayoutRegion::Panel);
    }


    // -- wb_mcp extended domain tests ----------------------------------------

    #[test]
    fn y_wb_mcp_enum_index() {
        assert_eq!(YWbMcpMcpConnectionState::Disconnected.index(), 0);
        assert_eq!(YWbMcpMcpConnectionState::Connecting.index(), 1);
        assert_eq!(YWbMcpMcpConnectionState::Connected.index(), 2);
        assert_eq!(YWbMcpMcpConnectionState::Error.index(), 3);
    }

    #[test]
    fn y_wb_mcp_enum_label() {
        assert_eq!(YWbMcpMcpConnectionState::Disconnected.label(), "Disconnected");
        assert_eq!(YWbMcpMcpConnectionState::Connecting.label(), "Connecting");
        assert_eq!(YWbMcpMcpConnectionState::Connected.label(), "Connected");
        assert_eq!(YWbMcpMcpConnectionState::Error.label(), "Error");
    }

    #[test]
    fn y_wb_mcp_enum_all() {
        let all = YWbMcpMcpConnectionState::all();
        assert_eq!(all.len(), 4);
    }

    #[test]
    fn y_wb_mcp_enum_is_default() {
        assert!(YWbMcpMcpConnectionState::Disconnected.is_default());
        assert!(!YWbMcpMcpConnectionState::Error.is_default());
    }

    #[test]
    fn y_wb_mcp_enum_display() {
        assert_eq!(format!("{}", YWbMcpMcpConnectionState::Disconnected), "Disconnected");
    }

    #[test]
    fn y_wb_mcp_struct_new() {
        let s = YWbMcpMcpMessageQueue::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn y_wb_mcp_struct_clear() {
        let mut s = YWbMcpMcpMessageQueue::new();
        s.messages.push(Default::default());
        assert!(!s.is_empty());
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn y_wb_mcp_fingerprint_deterministic() {
        let h1 = y_wb_mcp_fingerprint("hello");
        let h2 = y_wb_mcp_fingerprint("hello");
        assert_eq!(h1, h2);
        assert_ne!(y_wb_mcp_fingerprint("a"), y_wb_mcp_fingerprint("b"));
    }

    #[test]
    fn y_wb_mcp_truncate_short() {
        assert_eq!(y_wb_mcp_truncate("hi", 10), "hi");
    }

    #[test]
    fn y_wb_mcp_truncate_long() {
        let r = y_wb_mcp_truncate("abcdef", 3);
        assert!(r.starts_with("abc"));
        assert_eq!(r.len(), 3 + '…'.len_utf8());
    }

    #[test]
    fn y_wb_mcp_normalize_key_basic() {
        assert_eq!(y_wb_mcp_normalize_key("Hello World"), "hello_world");
    }

    #[test]
    fn y_wb_mcp_split_path_basic() {
        let parts = y_wb_mcp_split_path("a.b.c");
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn y_wb_mcp_count_occurrences_basic() {
        assert_eq!(y_wb_mcp_count_occurrences("abcabc", "abc"), 2);
        assert_eq!(y_wb_mcp_count_occurrences("abc", "xyz"), 0);
        assert_eq!(y_wb_mcp_count_occurrences("abc", ""), 0);
    }

    #[test]
    fn y_wb_mcp_in_range_basic() {
        assert!(y_wb_mcp_in_range(5, 1, 10));
        assert!(y_wb_mcp_in_range(1, 1, 10));
        assert!(y_wb_mcp_in_range(10, 1, 10));
        assert!(!y_wb_mcp_in_range(0, 1, 10));
        assert!(!y_wb_mcp_in_range(11, 1, 10));
    }

    #[test]
    fn y_wb_mcp_dedup_sorted_basic() {
        let items: Vec<String> = vec!["a".into(), "a".into(), "b".into(), "c".into(), "c".into()];
        let deduped = y_wb_mcp_dedup_sorted(&items);
        assert_eq!(deduped.len(), 3);
        assert_eq!(deduped[0], "a");
    }

    #[test]
    fn y_wb_mcp_interleave_basic() {
        let a: Vec<String> = vec!["a".into(), "b".into()];
        let b: Vec<String> = vec!["1".into(), "2".into(), "3".into()];
        let r = y_wb_mcp_interleave(&a, &b);
        assert_eq!(r.len(), 5);
        assert_eq!(r[0], "a");
        assert_eq!(r[1], "1");
    }

    // -- wb_mcp Z-extended tests -----------------------------------------------

    #[test]
    fn z_wb_mcp_priority_weight() {
        assert_eq!(ZWbMcpPriority::Idle.weight(), 0);
        assert_eq!(ZWbMcpPriority::Normal.weight(), 2);
        assert_eq!(ZWbMcpPriority::Realtime.weight(), 4);
    }

    #[test]
    fn z_wb_mcp_priority_label() {
        assert_eq!(ZWbMcpPriority::Low.label(), "low");
        assert_eq!(ZWbMcpPriority::High.label(), "high");
    }

    #[test]
    fn z_wb_mcp_priority_is_elevated() {
        assert!(!ZWbMcpPriority::Normal.is_elevated());
        assert!(ZWbMcpPriority::High.is_elevated());
        assert!(ZWbMcpPriority::Realtime.is_elevated());
    }

    #[test]
    fn z_wb_mcp_priority_display() {
        assert_eq!(format!("{}", ZWbMcpPriority::Idle), "idle");
    }

    #[test]
    fn z_wb_mcp_priority_all_asc() {
        let all = ZWbMcpPriority::all_asc();
        assert_eq!(all.len(), 5);
        assert_eq!(all[0], ZWbMcpPriority::Idle);
        assert_eq!(all[4], ZWbMcpPriority::Realtime);
    }

    #[test]
    fn z_wb_mcp_struct_new() {
        let s = ZWbMcpMcpToolRegistry::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn z_wb_mcp_struct_toggled_clone() {
        let s = ZWbMcpMcpToolRegistry::new();
        let t = s.toggled_clone();
        assert_ne!(s.locked, t.locked);
    }

    #[test]
    fn z_wb_mcp_rolling_hash_deterministic() {
        let h1 = z_wb_mcp_rolling_hash(b"test");
        let h2 = z_wb_mcp_rolling_hash(b"test");
        assert_eq!(h1, h2);
        assert_ne!(z_wb_mcp_rolling_hash(b"a"), z_wb_mcp_rolling_hash(b"b"));
    }

    #[test]
    fn z_wb_mcp_pad_to_basic() {
        assert_eq!(z_wb_mcp_pad_to("hi", 5), "hi   ");
        assert_eq!(z_wb_mcp_pad_to("hello world", 5), "hello");
    }

    #[test]
    fn z_wb_mcp_is_identifier_basic() {
        assert!(z_wb_mcp_is_identifier("foo_bar"));
        assert!(z_wb_mcp_is_identifier("abc123"));
        assert!(!z_wb_mcp_is_identifier(""));
        assert!(!z_wb_mcp_is_identifier("has space"));
    }

    #[test]
    fn z_wb_mcp_levenshtein_basic() {
        assert_eq!(z_wb_mcp_levenshtein("", ""), 0);
        assert_eq!(z_wb_mcp_levenshtein("abc", "abc"), 0);
        assert_eq!(z_wb_mcp_levenshtein("kitten", "sitting"), 3);
    }

    #[test]
    fn z_wb_mcp_unique_words_basic() {
        let w = z_wb_mcp_unique_words("the cat sat on the mat");
        assert_eq!(w.len(), 5);
        assert_eq!(w[0], "the");
    }

    #[test]
    fn z_wb_mcp_chunk_slice_basic() {
        let data = vec![1, 2, 3, 4, 5];
        let chunks = z_wb_mcp_chunk_slice(&data, 2);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], &[1, 2]);
        assert_eq!(chunks[2], &[5]);
    }

    #[test]
    fn z_wb_mcp_common_prefix_basic() {
        assert_eq!(z_wb_mcp_common_prefix("abcdef", "abcxyz"), "abc");
        assert_eq!(z_wb_mcp_common_prefix("xyz", "abc"), "");
    }

    #[test]
    fn z_wb_mcp_struct_clear() {
        let mut s = ZWbMcpMcpToolRegistry::new();
        s.tools.push(Default::default());
        assert_eq!(s.len(), 1);
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn z_wb_mcp_rolling_hash_empty() {
        let h = z_wb_mcp_rolling_hash(b"");
        assert_eq!(h, 0xcbf29ce484222325);
    }

    #[test]
    fn xb_ring_buffer_76_push_and_len() {
        let mut rb = super::XbRingBuffer76::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_76_overwrite() {
        let mut rb = super::XbRingBuffer76::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_76_get_out_of_bounds() {
        let rb = super::XbRingBuffer76::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_76_drain_all() {
        let mut rb = super::XbRingBuffer76::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_76_peek_front_back() {
        let mut rb = super::XbRingBuffer76::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_76_clear() {
        let mut rb = super::XbRingBuffer76::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_76_capacity() {
        let rb = super::XbRingBuffer76::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_76_basic() {
        let h = super::xb_fnv1a_76(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_76(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_76_different_inputs() {
        let h1 = super::xb_fnv1a_76(b"abc");
        let h2 = super::xb_fnv1a_76(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_76_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_76(&data);
        let dec = super::xb_rle_decode_76(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_76_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_76(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_76(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_76_values() {
        assert!((super::xb_clamp_76(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_76(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_76(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_76_values() {
        assert!((super::xb_lerp_76(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_76(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_76(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_76_wrap_around_twice() {
        let mut rb = super::XbRingBuffer76::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
    }


    // ---- xc_ pool / scheduler tests – block 215 ----

    #[test]
    fn xc_215_pool_new_empty() {
        let pool: super::Xc215Pool<i32> = super::Xc215Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_215_pool_release_acquire() {
        let mut pool = super::Xc215Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_215_pool_acquire_empty() {
        let mut pool: super::Xc215Pool<i32> = super::Xc215Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_215_pool_full() {
        let mut pool = super::Xc215Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_215_pool_drain() {
        let mut pool = super::Xc215Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_215_pool_stats() {
        let mut pool = super::Xc215Pool::new(8);
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
    fn xc_215_pool_clear() {
        let mut pool = super::Xc215Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_215_pool_shrink() {
        let mut pool = super::Xc215Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_215_pool_default() {
        let pool: super::Xc215Pool<String> = super::Xc215Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_215_pool_extend() {
        let mut pool = super::Xc215Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_215_pool_retain() {
        let mut pool = super::Xc215Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_215_scheduler_round_robin() {
        let mut sched = super::Xc215Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_215_scheduler_empty() {
        let mut sched = super::Xc215Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_215_scheduler_reset() {
        let mut sched = super::Xc215Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_215_scheduler_add_remove() {
        let mut sched = super::Xc215Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_215_scheduler_targets() {
        let sched = super::Xc215Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_215_hash_empty() {
        assert_eq!(super::xc_215_hash(b""), 5381);
    }

    #[test]
    fn xc_215_hash_data() {
        let h = super::xc_215_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_215_hash(b"hello"), h);
    }

    #[test]
    fn xc_215_reverse_str() {
        assert_eq!(super::xc_215_reverse("abc"), "cba");
        assert_eq!(super::xc_215_reverse(""), "");
    }


    #[test]
    fn xe_89_pipeline_empty() {
        let p = super::Xe89Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_89_pipeline_parse_stage() {
        let p = super::Xe89Pipeline::new()
            .add_parse(super::xe_89_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_89_pipeline_transform_double() {
        let p = super::Xe89Pipeline::new()
            .add_transform(super::xe_89_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_89_pipeline_validate_reverse() {
        let p = super::Xe89Pipeline::new()
            .add_validate(super::xe_89_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_89_pipeline_emit_filter() {
        let p = super::Xe89Pipeline::new()
            .add_emit(super::xe_89_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_89_pipeline_multi_stage() {
        let p = super::Xe89Pipeline::new()
            .add_parse(super::xe_89_pipeline_identity)
            .add_transform(super::xe_89_pipeline_double)
            .add_validate(super::xe_89_pipeline_reverse)
            .add_emit(super::xe_89_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_89_pipeline_error_propagation() {
        let p = super::Xe89Pipeline::new()
            .add_parse(super::xe_89_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe89Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_89_pipeline_compose() {
        let p1 = super::Xe89Pipeline::new()
            .add_parse(super::xe_89_pipeline_identity);
        let p2 = super::Xe89Pipeline::new()
            .add_transform(super::xe_89_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_89_pipeline_error_display() {
        let e = super::Xe89PipelineError {
            stage: super::Xe89Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_89_cache_put_get() {
        let mut c = super::Xe89Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_89_cache_miss() {
        let mut c: super::Xe89Cache<&str, i32> = super::Xe89Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_89_cache_ttl_expiry() {
        let mut c = super::Xe89Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_89_cache_evict() {
        let mut c = super::Xe89Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_89_cache_capacity() {
        let mut c = super::Xe89Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_89_cache_stats() {
        let mut c = super::Xe89Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_89_cache_clear() {
        let mut c = super::Xe89Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // -- xg_87 graph tests ------------------------------------------------

    #[test]
    fn xg_87_graph_empty() {
        let g = super::Xg87Graph::new();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn xg_87_graph_add_node() {
        let mut g = super::Xg87Graph::new();
        g.add_node(1);
        g.add_node(2);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_87_graph_add_edge() {
        let mut g = super::Xg87Graph::new();
        g.add_edge(0, 1);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_87_graph_neighbors() {
        let mut g = super::Xg87Graph::new();
        g.add_edge(0, 1);
        g.add_edge(0, 2);
        assert_eq!(g.neighbors(0).len(), 2);
    }

    #[test]
    fn xg_87_graph_has_path() {
        let mut g = super::Xg87Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(g.has_path(0, 2));
        assert!(!g.has_path(2, 0));
    }

    #[test]
    fn xg_87_graph_self_path() {
        let g = super::Xg87Graph::new();
        assert!(g.has_path(5, 5));
    }

    #[test]
    fn xg_87_graph_topo_sort() {
        let mut g = super::Xg87Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        let sorted = g.topological_sort().unwrap();
        let pos: std::collections::HashMap<usize, usize> =
            sorted.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&1] < pos[&2]);
    }

    #[test]
    fn xg_87_graph_cycle_detect_false() {
        let mut g = super::Xg87Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(!g.cycle_detect());
    }

    #[test]
    fn xg_87_graph_cycle_detect_true() {
        let mut g = super::Xg87Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        g.add_edge(2, 0);
        assert!(g.cycle_detect());
    }

    // -- xg_87 heap tests -------------------------------------------------

    #[test]
    fn xg_87_heap_empty() {
        let h: super::Xg87Heap<i32> = super::Xg87Heap::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn xg_87_heap_push_pop() {
        let mut h = super::Xg87Heap::new();
        h.push(3);
        h.push(1);
        h.push(2);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(3));
    }

    #[test]
    fn xg_87_heap_peek() {
        let mut h = super::Xg87Heap::new();
        h.push(5);
        h.push(2);
        assert_eq!(h.peek(), Some(&2));
    }

    #[test]
    fn xg_87_heap_drain_sorted() {
        let mut h = super::Xg87Heap::new();
        for v in [4, 1, 7, 2, 9] { h.push(v); }
        assert_eq!(h.drain_sorted(), vec![1, 2, 4, 7, 9]);
        assert!(h.is_empty());
    }

    #[test]
    fn xg_87_heap_merge() {
        let mut a = super::Xg87Heap::new();
        let mut b = super::Xg87Heap::new();
        a.push(5); a.push(3);
        b.push(4); b.push(1);
        a.merge(&mut b);
        assert_eq!(a.len(), 4);
        assert_eq!(a.pop(), Some(1));
    }

    #[test]
    fn xg_87_heap_default() {
        let h: super::Xg87Heap<u64> = Default::default();
        assert!(h.is_empty());
    }

    #[test]
    fn xg_87_graph_default() {
        let g: super::Xg87Graph = Default::default();
        assert_eq!(g.node_count(), 0);
    }


    #[test]
    fn xh214_skip_insert_contains() {
        let mut sl = super::Xh214SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh214_skip_remove() {
        let mut sl = super::Xh214SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh214_skip_len() {
        let mut sl = super::Xh214SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh214_skip_range_query() {
        let mut sl = super::Xh214SkipList::xh_new(4);
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
    fn xh214_skip_floor_ceiling() {
        let mut sl = super::Xh214SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh214_skip_rank() {
        let mut sl = super::Xh214SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh214_skip_empty() {
        let sl = super::Xh214SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh214_skip_duplicates() {
        let mut sl = super::Xh214SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh214_bitset_set_test() {
        let mut bs = super::Xh214BitSet::xh_new(256);
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
    fn xh214_bitset_clear_count() {
        let mut bs = super::Xh214BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh214_bitset_and_or_xor() {
        let mut a = super::Xh214BitSet::xh_new(128);
        let mut b = super::Xh214BitSet::xh_new(128);
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
    fn xh214_bitset_iter_ones() {
        let mut bs = super::Xh214BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh214_bitset_first_last() {
        let mut bs = super::Xh214BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh214_bitset_empty() {
        let bs = super::Xh214BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi214_deque_push_pop_back() {
        let mut dq = super::Xi214Deque::xi_new(4);
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
    fn xi214_deque_push_pop_front() {
        let mut dq = super::Xi214Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi214_deque_mixed_ops() {
        let mut dq = super::Xi214Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi214_deque_get_and_split() {
        let mut dq = super::Xi214Deque::xi_new(8);
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
    fn xi214_deque_rotate_left() {
        let mut dq = super::Xi214Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi214_deque_rotate_right() {
        let mut dq = super::Xi214Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi214_deque_grow() {
        let mut dq = super::Xi214Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi214_deque_empty() {
        let dq = super::Xi214Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi214_interval_tree_insert_query() {
        let mut tree = super::Xi214IntervalTree::xi_new();
        tree.xi_insert(super::Xi214Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi214Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi214Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi214_interval_tree_overlap() {
        let mut tree = super::Xi214IntervalTree::xi_new();
        tree.xi_insert(super::Xi214Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi214Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi214Interval::xi_new(12, 20));
        let q = super::Xi214Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi214_interval_tree_remove() {
        let mut tree = super::Xi214IntervalTree::xi_new();
        tree.xi_insert(super::Xi214Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi214Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi214_interval_tree_gaps() {
        let mut tree = super::Xi214IntervalTree::xi_new();
        tree.xi_insert(super::Xi214Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi214Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi214Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi214Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi214Interval::xi_new(8, 10));
    }

    #[test]
    fn xi214_interval_tree_merge() {
        let mut tree = super::Xi214IntervalTree::xi_new();
        tree.xi_insert(super::Xi214Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi214Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi214Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi214Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi214Interval::xi_new(10, 15));
    }

    #[test]
    fn xi214_interval_tree_all() {
        let mut tree = super::Xi214IntervalTree::xi_new();
        tree.xi_insert(super::Xi214Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi214Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi214_interval_tree_empty() {
        let tree = super::Xi214IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi214_interval_tree_contains_point() {
        let iv = super::Xi214Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 214) ---

    #[test]
    fn xj_214_uf_make_and_find() {
        let mut uf = super::Xj214UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_214_uf_union_connected() {
        let mut uf = super::Xj214UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_214_uf_component_count() {
        let mut uf = super::Xj214UnionFind::xj_new();
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
    fn xj_214_uf_component_size() {
        let mut uf = super::Xj214UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_214_uf_largest_component() {
        let mut uf = super::Xj214UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_214_uf_many_elements() {
        let mut uf = super::Xj214UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_214_uf_separate_components() {
        let mut uf = super::Xj214UnionFind::xj_new();
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
    fn xj_214_uf_path_compression() {
        let mut uf = super::Xj214UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_214_bt_insert_get() {
        let mut bt = super::Xj214BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_214_bt_contains_len() {
        let mut bt = super::Xj214BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_214_bt_replace() {
        let mut bt = super::Xj214BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_214_bt_remove() {
        let mut bt = super::Xj214BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_214_bt_keys_values() {
        let mut bt = super::Xj214BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_214_bt_range() {
        let mut bt = super::Xj214BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_214_bt_min_max() {
        let mut bt = super::Xj214BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_214_bt_many_inserts() {
        let mut bt = super::Xj214BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_214 segment tree tests ---

    #[test]
    fn xk_214_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk214SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_214_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk214SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_214_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk214SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_214_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk214SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_214_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk214SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_214_st_single_element() {
        let data = vec![42];
        let st = super::Xk214SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_214_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk214SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_214_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk214SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_214 disjoint intervals tests ---

    #[test]
    fn xk_214_di_add_and_count() {
        let mut di = super::Xk214DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_214_di_merge_overlap() {
        let mut di = super::Xk214DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_214_di_contains() {
        let mut di = super::Xk214DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_214_di_remove() {
        let mut di = super::Xk214DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_214_di_covered_length() {
        let mut di = super::Xk214DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_214_di_gaps() {
        let mut di = super::Xk214DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_214_di_merge_adjacent() {
        let mut di = super::Xk214DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_214_di_empty() {
        let di = super::Xk214DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }

}