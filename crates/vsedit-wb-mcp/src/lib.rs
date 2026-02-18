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
}