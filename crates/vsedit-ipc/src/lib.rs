//! Inter-process communication.

use std::fmt;

/// Errors that can occur during IPC operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IpcError {
    ChannelNotFound(String),
    ChannelDisconnected(String),
    MessageTooLarge { size: usize, max: usize },
    DuplicateChannel(String),
}

impl fmt::Display for IpcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IpcError::ChannelNotFound(name) => write!(f, "channel not found: {name}"),
            IpcError::ChannelDisconnected(name) => write!(f, "channel disconnected: {name}"),
            IpcError::MessageTooLarge { size, max } => {
                write!(f, "message too large: {size} bytes (max {max})")
            }
            IpcError::DuplicateChannel(name) => write!(f, "duplicate channel: {name}"),
        }
    }
}

/// A message transmitted over an IPC channel.
#[derive(Debug, Clone)]
pub struct IpcMessage {
    pub id: u64,
    pub channel: String,
    pub payload: Vec<u8>,
}

impl IpcMessage {
    /// Attempt to interpret the payload as a UTF-8 string.
    pub fn payload_as_string(&self) -> Option<String> {
        std::str::from_utf8(&self.payload).ok().map(String::from)
    }
}

impl fmt::Display for IpcMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "IpcMessage(id={}, channel={}, {} bytes)",
            self.id,
            self.channel,
            self.payload.len()
        )
    }
}

/// Builder for constructing [`IpcMessage`] instances.
#[derive(Debug, Clone, Default)]
pub struct IpcMessageBuilder {
    id: Option<u64>,
    channel: Option<String>,
    payload: Option<Vec<u8>>,
}

impl IpcMessageBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn id(mut self, id: u64) -> Self {
        self.id = Some(id);
        self
    }

    pub fn channel(mut self, channel: &str) -> Self {
        self.channel = Some(channel.to_string());
        self
    }

    pub fn payload(mut self, payload: Vec<u8>) -> Self {
        self.payload = Some(payload);
        self
    }

    /// Consume the builder and produce an `IpcMessage`.
    /// Panics if `id` or `channel` have not been set.
    pub fn build(self) -> IpcMessage {
        IpcMessage {
            id: self.id.expect("IpcMessageBuilder: id is required"),
            channel: self.channel.expect("IpcMessageBuilder: channel is required"),
            payload: self.payload.unwrap_or_default(),
        }
    }
}

/// A named communication channel.
#[derive(Debug, Clone)]
pub struct IpcChannel {
    pub name: String,
    pub connected: bool,
}

/// Wire protocol for IPC communication.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IpcProtocol {
    JsonRpc,
    Binary,
    Custom(String),
}

/// The kind of an IPC message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageKind {
    Request,
    Response,
    Notification,
}

impl fmt::Display for MessageKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MessageKind::Request => write!(f, "Request"),
            MessageKind::Response => write!(f, "Response"),
            MessageKind::Notification => write!(f, "Notification"),
        }
    }
}

/// Header for an IPC message.
#[derive(Debug, Clone, PartialEq)]
pub struct IpcMessageHeader {
    pub kind: MessageKind,
    pub id: u64,
    pub method: Option<String>,
}

/// Service that manages IPC channels and message dispatch.
pub struct IpcService {
    channels: Vec<IpcChannel>,
    next_message_id: u64,
    pending_count: usize,
}

impl IpcService {
    pub fn new() -> Self {
        Self {
            channels: Vec::new(),
            next_message_id: 1,
            pending_count: 0,
        }
    }

    /// Register a new channel by name. Returns `true` if added, `false` if it
    /// already exists.
    pub fn register_channel(&mut self, name: &str) -> bool {
        if self.channels.iter().any(|c| c.name == name) {
            return false;
        }
        self.channels.push(IpcChannel {
            name: name.to_string(),
            connected: true,
        });
        true
    }

    /// Send a message on the named channel. Returns the assigned message id,
    /// or `None` if the channel does not exist or is disconnected.
    pub fn send_message(&mut self, channel: &str, payload: Vec<u8>) -> Option<u64> {
        let ch = self.channels.iter().find(|c| c.name == channel)?;
        if !ch.connected {
            return None;
        }
        let id = self.next_message_id;
        self.next_message_id += 1;
        self.pending_count += 1;
        // In a real implementation the message would be serialised and sent.
        let _ = IpcMessage {
            id,
            channel: channel.to_string(),
            payload,
        };
        Some(id)
    }

    /// Build a request message for a given method.
    pub fn create_request(&mut self, method: &str, payload: Vec<u8>) -> IpcMessage {
        let id = self.next_message_id;
        self.next_message_id += 1;
        IpcMessage {
            id,
            channel: method.to_string(),
            payload,
        }
    }

    /// Build a response message for a previously received request.
    pub fn create_response(&mut self, request_id: u64, payload: Vec<u8>) -> IpcMessage {
        IpcMessage {
            id: request_id,
            channel: String::new(),
            payload,
        }
    }

    /// Mark a channel as disconnected. Returns an error if the channel is not
    /// found.
    pub fn disconnect_channel(&mut self, name: &str) -> Result<(), IpcError> {
        let ch = self
            .channels
            .iter_mut()
            .find(|c| c.name == name)
            .ok_or_else(|| IpcError::ChannelNotFound(name.to_string()))?;
        ch.connected = false;
        Ok(())
    }

    /// Re-connect a previously disconnected channel. Returns an error if the
    /// channel is not found.
    pub fn reconnect_channel(&mut self, name: &str) -> Result<(), IpcError> {
        let ch = self
            .channels
            .iter_mut()
            .find(|c| c.name == name)
            .ok_or_else(|| IpcError::ChannelNotFound(name.to_string()))?;
        ch.connected = true;
        Ok(())
    }

    /// Remove a channel entirely. Returns an error if the channel is not found.
    pub fn unregister_channel(&mut self, name: &str) -> Result<(), IpcError> {
        let idx = self
            .channels
            .iter()
            .position(|c| c.name == name)
            .ok_or_else(|| IpcError::ChannelNotFound(name.to_string()))?;
        self.channels.remove(idx);
        Ok(())
    }

    /// Return a list of `(channel_name, connected)` pairs.
    pub fn list_channels(&self) -> Vec<(String, bool)> {
        self.channels
            .iter()
            .map(|c| (c.name.clone(), c.connected))
            .collect()
    }

    /// Create a notification message (fire-and-forget, no response expected).
    pub fn send_notification(&mut self, channel: &str, payload: Vec<u8>) -> Result<IpcMessage, IpcError> {
        let ch = self
            .channels
            .iter()
            .find(|c| c.name == channel)
            .ok_or_else(|| IpcError::ChannelNotFound(channel.to_string()))?;
        if !ch.connected {
            return Err(IpcError::ChannelDisconnected(channel.to_string()));
        }
        let id = self.next_message_id;
        self.next_message_id += 1;
        self.pending_count += 1;
        Ok(IpcMessage {
            id,
            channel: channel.to_string(),
            payload,
        })
    }

    /// Number of messages sent through the service so far.
    pub fn pending_count(&self) -> usize {
        self.pending_count
    }
}

impl Default for IpcService {
    fn default() -> Self {
        Self::new()
    }
}

/// Accumulated statistics for ipc operations.
#[derive(Debug, Clone, PartialEq)]
pub struct IpcStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl IpcStats {
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
    pub fn merge(&mut self, other: &IpcStats) {
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

impl Default for IpcStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for IpcStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "IpcStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for ipc.
#[derive(Debug, Clone)]
pub struct IpcValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl IpcValidator {
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

impl Default for IpcValidator {
    fn default() -> Self {
        Self::new()
    }
}

/// Buffered queue for batching IPC messages before sending.
pub struct IpcMessageQueue {
    queue: Vec<IpcMessage>,
    max_size: usize,
}

impl IpcMessageQueue {
    pub fn new(max_size: usize) -> Self {
        Self { queue: Vec::new(), max_size }
    }

    /// Enqueue a message. Returns Err if queue is full.
    pub fn enqueue(&mut self, msg: IpcMessage) -> Result<(), IpcError> {
        if self.queue.len() >= self.max_size {
            return Err(IpcError::MessageTooLarge { size: self.queue.len() + 1, max: self.max_size });
        }
        self.queue.push(msg);
        Ok(())
    }

    /// Drain all queued messages, returning them in order.
    pub fn drain(&mut self) -> Vec<IpcMessage> {
        std::mem::take(&mut self.queue)
    }

    /// Peek at the next message without removing it.
    pub fn peek(&self) -> Option<&IpcMessage> {
        self.queue.first()
    }

    /// Number of messages currently queued.
    pub fn len(&self) -> usize {
        self.queue.len()
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// Returns true if the queue is at capacity.
    pub fn is_full(&self) -> bool {
        self.queue.len() >= self.max_size
    }

    /// Remove and return the first message, if any.
    pub fn dequeue(&mut self) -> Option<IpcMessage> {
        if self.queue.is_empty() { None } else { Some(self.queue.remove(0)) }
    }

    /// Clear all queued messages without returning them.
    pub fn clear(&mut self) {
        self.queue.clear();
    }

    /// The maximum queue capacity.
    pub fn capacity(&self) -> usize {
        self.max_size
    }
}

/// Pool of named IPC connections with state tracking.
pub struct IpcConnectionPool {
    connections: Vec<IpcConnection>,
    max_connections: usize,
}

/// State of a single connection in the pool.
#[derive(Debug, Clone)]
pub struct IpcConnection {
    pub id: String,
    pub channel: String,
    pub connected: bool,
    pub messages_sent: u64,
    pub messages_received: u64,
}

impl IpcConnection {
    pub fn new(id: impl Into<String>, channel: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            channel: channel.into(),
            connected: true,
            messages_sent: 0,
            messages_received: 0,
        }
    }

    pub fn total_messages(&self) -> u64 {
        self.messages_sent + self.messages_received
    }
}

impl fmt::Display for IpcConnection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "IpcConnection(id={}, ch={}, connected={})", self.id, self.channel, self.connected)
    }
}

impl IpcConnectionPool {
    pub fn new(max_connections: usize) -> Self {
        Self { connections: Vec::new(), max_connections }
    }

    /// Add a connection. Returns error if pool is full or id already exists.
    pub fn add(&mut self, conn: IpcConnection) -> Result<(), IpcError> {
        if self.connections.iter().any(|c| c.id == conn.id) {
            return Err(IpcError::DuplicateChannel(conn.id));
        }
        if self.connections.len() >= self.max_connections {
            return Err(IpcError::MessageTooLarge { size: self.connections.len() + 1, max: self.max_connections });
        }
        self.connections.push(conn);
        Ok(())
    }

    /// Remove a connection by id.
    pub fn remove(&mut self, id: &str) -> Result<IpcConnection, IpcError> {
        let pos = self.connections.iter().position(|c| c.id == id)
            .ok_or_else(|| IpcError::ChannelNotFound(id.to_string()))?;
        Ok(self.connections.remove(pos))
    }

    /// Get a reference to a connection by id.
    pub fn get(&self, id: &str) -> Option<&IpcConnection> {
        self.connections.iter().find(|c| c.id == id)
    }

    /// Get a mutable reference to a connection by id.
    pub fn get_mut(&mut self, id: &str) -> Option<&mut IpcConnection> {
        self.connections.iter_mut().find(|c| c.id == id)
    }

    /// Return all connected connections.
    pub fn active_connections(&self) -> Vec<&IpcConnection> {
        self.connections.iter().filter(|c| c.connected).collect()
    }

    /// Number of connections in the pool.
    pub fn len(&self) -> usize {
        self.connections.len()
    }

    pub fn is_empty(&self) -> bool {
        self.connections.is_empty()
    }

    /// Disconnect a connection by id.
    pub fn disconnect(&mut self, id: &str) -> Result<(), IpcError> {
        let conn = self.connections.iter_mut().find(|c| c.id == id)
            .ok_or_else(|| IpcError::ChannelNotFound(id.to_string()))?;
        conn.connected = false;
        Ok(())
    }

    /// Reconnect a connection by id.
    pub fn reconnect(&mut self, id: &str) -> Result<(), IpcError> {
        let conn = self.connections.iter_mut().find(|c| c.id == id)
            .ok_or_else(|| IpcError::ChannelNotFound(id.to_string()))?;
        conn.connected = true;
        Ok(())
    }

    /// Record that a message was sent on the given connection.
    pub fn record_send(&mut self, id: &str) -> Result<(), IpcError> {
        let conn = self.connections.iter_mut().find(|c| c.id == id)
            .ok_or_else(|| IpcError::ChannelNotFound(id.to_string()))?;
        if !conn.connected {
            return Err(IpcError::ChannelDisconnected(id.to_string()));
        }
        conn.messages_sent += 1;
        Ok(())
    }

    /// Record that a message was received on the given connection.
    pub fn record_receive(&mut self, id: &str) -> Result<(), IpcError> {
        let conn = self.connections.iter_mut().find(|c| c.id == id)
            .ok_or_else(|| IpcError::ChannelNotFound(id.to_string()))?;
        conn.messages_received += 1;
        Ok(())
    }
}

/// Serialized envelope wrapping an IPC message with length-prefix framing.
#[derive(Debug, Clone, PartialEq)]
pub struct IpcEnvelope {
    pub header: IpcMessageHeader,
    pub payload: Vec<u8>,
    pub total_length: usize,
}

/// Serialize a message into a length-prefixed envelope.
/// Format: [4-byte big-endian length][1-byte kind][8-byte big-endian id][payload]
pub fn ipc_serialize_envelope(kind: MessageKind, id: u64, payload: &[u8]) -> Vec<u8> {
    let kind_byte: u8 = match kind {
        MessageKind::Request => 1,
        MessageKind::Response => 2,
        MessageKind::Notification => 3,
    };
    let content_len = 1 + 8 + payload.len(); // kind + id + payload
    let total_len = 4 + content_len; // length prefix + content
    let mut buf = Vec::with_capacity(total_len);
    buf.extend_from_slice(&(content_len as u32).to_be_bytes());
    buf.push(kind_byte);
    buf.extend_from_slice(&id.to_be_bytes());
    buf.extend_from_slice(payload);
    buf
}

/// Deserialize a length-prefixed envelope back into its parts.
/// Returns `(kind, id, payload)` or an error.
pub fn ipc_deserialize_envelope(data: &[u8]) -> Result<(MessageKind, u64, Vec<u8>), IpcError> {
    if data.len() < 13 { // 4 + 1 + 8 minimum
        return Err(IpcError::MessageTooLarge { size: data.len(), max: 13 });
    }
    let content_len = u32::from_be_bytes([data[0], data[1], data[2], data[3]]) as usize;
    if data.len() < 4 + content_len {
        return Err(IpcError::MessageTooLarge { size: data.len(), max: 4 + content_len });
    }
    let kind = match data[4] {
        1 => MessageKind::Request,
        2 => MessageKind::Response,
        3 => MessageKind::Notification,
        _ => return Err(IpcError::ChannelNotFound(format!("unknown kind byte: {}", data[4]))),
    };
    let id = u64::from_be_bytes([data[5], data[6], data[7], data[8], data[9], data[10], data[11], data[12]]);
    let payload = data[13..4 + content_len].to_vec();
    Ok((kind, id, payload))
}

// ---------------------------------------------------------------------------
// IpcRouter – route messages to handlers by channel pattern
// ---------------------------------------------------------------------------

/// A route entry mapping a channel pattern to a handler name.
#[derive(Debug, Clone)]
pub struct IpcRoute {
    /// Glob-like pattern, e.g. "textDocument/*" or exact "shutdown".
    pub pattern: String,
    /// Logical handler name this pattern routes to.
    pub handler: String,
}

/// Routes incoming IPC messages to named handlers based on channel patterns.
#[derive(Debug, Clone)]
pub struct IpcRouter {
    routes: Vec<IpcRoute>,
}

impl IpcRouter {
    pub fn new() -> Self {
        Self { routes: Vec::new() }
    }

    /// Register a route. `pattern` may end with `/*` to match any suffix.
    pub fn add_route(&mut self, pattern: &str, handler: &str) {
        self.routes.push(IpcRoute {
            pattern: pattern.to_string(),
            handler: handler.to_string(),
        });
    }

    /// Remove all routes targeting the given handler.
    pub fn remove_handler(&mut self, handler: &str) -> usize {
        let before = self.routes.len();
        self.routes.retain(|r| r.handler != handler);
        before - self.routes.len()
    }

    /// Find the handler name for a given channel. Returns the first match.
    pub fn resolve(&self, channel: &str) -> Option<&str> {
        for route in &self.routes {
            if route_matches(&route.pattern, channel) {
                return Some(&route.handler);
            }
        }
        None
    }

    /// Return all handler names that match the given channel.
    pub fn resolve_all(&self, channel: &str) -> Vec<&str> {
        self.routes
            .iter()
            .filter(|r| route_matches(&r.pattern, channel))
            .map(|r| r.handler.as_str())
            .collect()
    }

    /// Number of registered routes.
    pub fn route_count(&self) -> usize {
        self.routes.len()
    }
}

impl Default for IpcRouter {
    fn default() -> Self {
        Self::new()
    }
}

/// Simple pattern matching: exact match or prefix/* glob.
fn route_matches(pattern: &str, channel: &str) -> bool {
    if pattern == channel {
        return true;
    }
    if let Some(prefix) = pattern.strip_suffix("/*") {
        channel.starts_with(prefix) && channel.as_bytes().get(prefix.len()) == Some(&b'/')
    } else {
        false
    }
}

// ---------------------------------------------------------------------------
// IpcRateLimiter – token-bucket rate limiter
// ---------------------------------------------------------------------------

/// Token-bucket rate limiter for IPC message sending.
#[derive(Debug, Clone)]
pub struct IpcRateLimiter {
    /// Maximum number of tokens (burst size).
    capacity: u64,
    /// Current number of available tokens.
    tokens: u64,
    /// Total number of messages that were denied.
    denied_count: u64,
}

impl IpcRateLimiter {
    /// Create a new rate limiter with the given bucket capacity.
    pub fn new(capacity: u64) -> Self {
        Self {
            capacity,
            tokens: capacity,
            denied_count: 0,
        }
    }

    /// Try to consume one token. Returns `true` if allowed.
    pub fn try_acquire(&mut self) -> bool {
        if self.tokens > 0 {
            self.tokens -= 1;
            true
        } else {
            self.denied_count += 1;
            false
        }
    }

    /// Try to consume `n` tokens at once. All-or-nothing.
    pub fn try_acquire_n(&mut self, n: u64) -> bool {
        if self.tokens >= n {
            self.tokens -= n;
            true
        } else {
            self.denied_count += 1;
            false
        }
    }

    /// Refill tokens (simulating elapsed time). Capped at capacity.
    pub fn refill(&mut self, amount: u64) {
        self.tokens = (self.tokens + amount).min(self.capacity);
    }

    /// Current available tokens.
    pub fn available(&self) -> u64 {
        self.tokens
    }

    /// Total number of denied requests.
    pub fn denied(&self) -> u64 {
        self.denied_count
    }

    /// Reset limiter to full capacity.
    pub fn reset(&mut self) {
        self.tokens = self.capacity;
        self.denied_count = 0;
    }
}

// ---------------------------------------------------------------------------
// IpcMessageBatch – batch multiple messages for efficient sending
// ---------------------------------------------------------------------------

/// A batch of IPC messages to be sent together.
#[derive(Debug, Clone)]
pub struct IpcMessageBatch {
    messages: Vec<IpcMessage>,
    max_size: usize,
}

impl IpcMessageBatch {
    /// Create a new batch with the given maximum number of messages.
    pub fn new(max_size: usize) -> Self {
        Self {
            messages: Vec::new(),
            max_size,
        }
    }

    /// Add a message to the batch. Returns `Err` if the batch is full.
    pub fn add(&mut self, msg: IpcMessage) -> Result<(), IpcError> {
        if self.messages.len() >= self.max_size {
            return Err(IpcError::MessageTooLarge {
                size: self.messages.len() + 1,
                max: self.max_size,
            });
        }
        self.messages.push(msg);
        Ok(())
    }

    /// Number of messages in the batch.
    pub fn len(&self) -> usize {
        self.messages.len()
    }

    /// Whether the batch is empty.
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    /// Whether the batch is at capacity.
    pub fn is_full(&self) -> bool {
        self.messages.len() >= self.max_size
    }

    /// Total payload bytes across all messages.
    pub fn total_payload_bytes(&self) -> usize {
        self.messages.iter().map(|m| m.payload.len()).sum()
    }

    /// Drain all messages from the batch, returning them.
    pub fn drain(&mut self) -> Vec<IpcMessage> {
        std::mem::take(&mut self.messages)
    }

    /// Get distinct channels referenced by messages in this batch.
    pub fn channels(&self) -> Vec<&str> {
        let mut chs: Vec<&str> = self.messages.iter().map(|m| m.channel.as_str()).collect();
        chs.sort_unstable();
        chs.dedup();
        chs
    }

    /// Filter messages by channel name.
    pub fn messages_for_channel(&self, channel: &str) -> Vec<&IpcMessage> {
        self.messages.iter().filter(|m| m.channel == channel).collect()
    }

    /// Total number of distinct channels in this batch.
    pub fn channel_count(&self) -> usize {
        self.channels().len()
    }
}

// ---------------------------------------------------------------------------
// IpcMessageFilter — filter messages by criteria
// ---------------------------------------------------------------------------

/// Criteria for filtering IPC messages.
#[derive(Debug, Clone, Default)]
pub struct IpcMessageFilter {
    channel_prefix: Option<String>,
    min_payload_size: Option<usize>,
    max_payload_size: Option<usize>,
    id_range: Option<(u64, u64)>,
}

impl IpcMessageFilter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Only match messages whose channel starts with the given prefix.
    pub fn channel_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.channel_prefix = Some(prefix.into());
        self
    }

    /// Only match messages with payload at least this many bytes.
    pub fn min_payload(mut self, min: usize) -> Self {
        self.min_payload_size = Some(min);
        self
    }

    /// Only match messages with payload at most this many bytes.
    pub fn max_payload(mut self, max: usize) -> Self {
        self.max_payload_size = Some(max);
        self
    }

    /// Only match messages with id in [lo, hi] inclusive.
    pub fn id_range(mut self, lo: u64, hi: u64) -> Self {
        self.id_range = Some((lo, hi));
        self
    }

    /// Test whether a message matches all configured criteria.
    pub fn matches(&self, msg: &IpcMessage) -> bool {
        if let Some(ref prefix) = self.channel_prefix {
            if !msg.channel.starts_with(prefix.as_str()) {
                return false;
            }
        }
        if let Some(min) = self.min_payload_size {
            if msg.payload.len() < min {
                return false;
            }
        }
        if let Some(max) = self.max_payload_size {
            if msg.payload.len() > max {
                return false;
            }
        }
        if let Some((lo, hi)) = self.id_range {
            if msg.id < lo || msg.id > hi {
                return false;
            }
        }
        true
    }

    /// Filter a slice of messages, returning only those that match.
    pub fn apply<'a>(&self, msgs: &'a [IpcMessage]) -> Vec<&'a IpcMessage> {
        msgs.iter().filter(|m| self.matches(m)).collect()
    }
}

// ---------------------------------------------------------------------------
// IpcConnectionPool — additional methods
// ---------------------------------------------------------------------------

impl IpcConnectionPool {
    /// Return all connection IDs.
    pub fn connection_ids(&self) -> Vec<&str> {
        self.connections.iter().map(|c| c.id.as_str()).collect()
    }

    /// Total messages sent across all connections.
    pub fn total_sent(&self) -> u64 {
        self.connections.iter().map(|c| c.messages_sent).sum()
    }

    /// Total messages received across all connections.
    pub fn total_received(&self) -> u64 {
        self.connections.iter().map(|c| c.messages_received).sum()
    }

    /// Return the connection with the most total messages.
    pub fn busiest_connection(&self) -> Option<&IpcConnection> {
        self.connections.iter().max_by_key(|c| c.total_messages())
    }

    /// Disconnect all connections.
    pub fn disconnect_all(&mut self) {
        for conn in &mut self.connections {
            conn.connected = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_and_send() {
        let mut svc = IpcService::new();
        assert!(svc.register_channel("editor"));
        assert!(!svc.register_channel("editor")); // duplicate

        let id = svc.send_message("editor", b"hello".to_vec());
        assert!(id.is_some());

        assert!(svc.send_message("nonexistent", vec![]).is_none());
    }

    #[test]
    fn create_request_and_response() {
        let mut svc = IpcService::new();
        let req = svc.create_request("textDocument/completion", b"{}".to_vec());
        assert_eq!(req.channel, "textDocument/completion");

        let resp = svc.create_response(req.id, b"result".to_vec());
        assert_eq!(resp.id, req.id);
    }

    #[test]
    fn message_header_kinds() {
        let header = IpcMessageHeader {
            kind: MessageKind::Notification,
            id: 0,
            method: Some("initialized".into()),
        };
        assert_eq!(header.kind, MessageKind::Notification);
        assert_eq!(header.method.as_deref(), Some("initialized"));

        assert_ne!(IpcProtocol::JsonRpc, IpcProtocol::Binary);
        assert_eq!(
            IpcProtocol::Custom("grpc".into()),
            IpcProtocol::Custom("grpc".into())
        );
    }

    #[test]
    fn disconnect_and_reconnect_channel() {
        let mut svc = IpcService::new();
        svc.register_channel("lsp");
        assert!(svc.send_message("lsp", b"ping".to_vec()).is_some());

        svc.disconnect_channel("lsp").unwrap();
        assert!(svc.send_message("lsp", b"ping".to_vec()).is_none());

        svc.reconnect_channel("lsp").unwrap();
        assert!(svc.send_message("lsp", b"ping".to_vec()).is_some());
    }

    #[test]
    fn disconnect_unknown_channel_returns_error() {
        let mut svc = IpcService::new();
        let err = svc.disconnect_channel("ghost").unwrap_err();
        assert_eq!(err, IpcError::ChannelNotFound("ghost".into()));
    }

    #[test]
    fn unregister_channel() {
        let mut svc = IpcService::new();
        svc.register_channel("tmp");
        assert_eq!(svc.list_channels().len(), 1);
        svc.unregister_channel("tmp").unwrap();
        assert!(svc.list_channels().is_empty());
        assert_eq!(
            svc.unregister_channel("tmp").unwrap_err(),
            IpcError::ChannelNotFound("tmp".into())
        );
    }

    #[test]
    fn list_channels_shows_status() {
        let mut svc = IpcService::new();
        svc.register_channel("a");
        svc.register_channel("b");
        svc.disconnect_channel("b").unwrap();

        let list = svc.list_channels();
        assert_eq!(list, vec![("a".into(), true), ("b".into(), false)]);
    }

    #[test]
    fn send_notification_success_and_disconnected() {
        let mut svc = IpcService::new();
        svc.register_channel("events");

        let msg = svc.send_notification("events", b"data".to_vec()).unwrap();
        assert_eq!(msg.channel, "events");
        assert_eq!(msg.payload, b"data");

        svc.disconnect_channel("events").unwrap();
        let err = svc.send_notification("events", vec![]).unwrap_err();
        assert_eq!(err, IpcError::ChannelDisconnected("events".into()));
    }

    #[test]
    fn send_notification_unknown_channel() {
        let mut svc = IpcService::new();
        let err = svc.send_notification("nope", vec![]).unwrap_err();
        assert_eq!(err, IpcError::ChannelNotFound("nope".into()));
    }

    #[test]
    fn payload_as_string_valid_and_invalid() {
        let valid = IpcMessage {
            id: 1,
            channel: "ch".into(),
            payload: b"hello world".to_vec(),
        };
        assert_eq!(valid.payload_as_string(), Some("hello world".into()));

        let invalid = IpcMessage {
            id: 2,
            channel: "ch".into(),
            payload: vec![0xff, 0xfe],
        };
        assert!(invalid.payload_as_string().is_none());
    }

    #[test]
    fn ipc_message_display() {
        let msg = IpcMessage {
            id: 42,
            channel: "editor".into(),
            payload: vec![0; 10],
        };
        assert_eq!(format!("{msg}"), "IpcMessage(id=42, channel=editor, 10 bytes)");
    }

    #[test]
    fn message_kind_display() {
        assert_eq!(format!("{}", MessageKind::Request), "Request");
        assert_eq!(format!("{}", MessageKind::Response), "Response");
        assert_eq!(format!("{}", MessageKind::Notification), "Notification");
    }

    #[test]
    fn ipc_message_builder() {
        let msg = IpcMessageBuilder::new()
            .id(7)
            .channel("build")
            .payload(b"content".to_vec())
            .build();
        assert_eq!(msg.id, 7);
        assert_eq!(msg.channel, "build");
        assert_eq!(msg.payload, b"content");
    }

    #[test]
    fn ipc_message_builder_default_payload() {
        let msg = IpcMessageBuilder::new().id(1).channel("c").build();
        assert!(msg.payload.is_empty());
    }

    #[test]
    fn error_display() {
        assert_eq!(
            IpcError::ChannelNotFound("x".into()).to_string(),
            "channel not found: x"
        );
        assert_eq!(
            IpcError::ChannelDisconnected("y".into()).to_string(),
            "channel disconnected: y"
        );
        assert_eq!(
            IpcError::MessageTooLarge { size: 100, max: 50 }.to_string(),
            "message too large: 100 bytes (max 50)"
        );
        assert_eq!(
            IpcError::DuplicateChannel("z".into()).to_string(),
            "duplicate channel: z"
        );
    }

    #[test]
    fn pending_count_tracks_messages() {
        let mut svc = IpcService::new();
        svc.register_channel("ch");
        assert_eq!(svc.pending_count(), 0);

        svc.send_message("ch", b"a".to_vec());
        svc.send_message("ch", b"b".to_vec());
        assert_eq!(svc.pending_count(), 2);

        svc.send_notification("ch", b"c".to_vec()).unwrap();
        assert_eq!(svc.pending_count(), 3);
    }

    #[test]
    fn eq_ipcprotocol_same() {
        assert_eq!(IpcProtocol::JsonRpc, IpcProtocol::JsonRpc);
    }

    #[test]
    fn ne_ipcprotocol_diff() {
        assert_ne!(IpcProtocol::JsonRpc, IpcProtocol::Binary);
    }

    #[test]
    fn eq_messagekind_same() {
        assert_eq!(MessageKind::Request, MessageKind::Request);
    }

    #[test]
    fn ne_messagekind_diff() {
        assert_ne!(MessageKind::Request, MessageKind::Response);
    }

    #[test]
    fn display_ipcerror_variants() {
        assert!(std::mem::size_of::<IpcError>() > 0);
    }

    #[test]
    fn display_messagekind_variants() {
        assert!(!MessageKind::Request.to_string().is_empty());
        assert!(!MessageKind::Response.to_string().is_empty());
        assert!(!MessageKind::Notification.to_string().is_empty());
    }

    #[test]
    fn message_queue_enqueue_and_drain() {
        let mut q = IpcMessageQueue::new(3);
        assert!(q.is_empty());
        q.enqueue(IpcMessageBuilder::new().id(1).channel("a").build()).unwrap();
        q.enqueue(IpcMessageBuilder::new().id(2).channel("b").build()).unwrap();
        assert_eq!(q.len(), 2);
        let drained = q.drain();
        assert_eq!(drained.len(), 2);
        assert_eq!(drained[0].id, 1);
        assert_eq!(drained[1].id, 2);
        assert!(q.is_empty());
    }

    #[test]
    fn message_queue_full_rejects() {
        let mut q = IpcMessageQueue::new(1);
        q.enqueue(IpcMessageBuilder::new().id(1).channel("a").build()).unwrap();
        assert!(q.is_full());
        let err = q.enqueue(IpcMessageBuilder::new().id(2).channel("b").build()).unwrap_err();
        assert!(matches!(err, IpcError::MessageTooLarge { .. }));
    }

    #[test]
    fn message_queue_peek_and_dequeue() {
        let mut q = IpcMessageQueue::new(10);
        assert!(q.peek().is_none());
        q.enqueue(IpcMessageBuilder::new().id(42).channel("ch").payload(b"data".to_vec()).build()).unwrap();
        assert_eq!(q.peek().unwrap().id, 42);
        let msg = q.dequeue().unwrap();
        assert_eq!(msg.id, 42);
        assert!(q.is_empty());
    }

    #[test]
    fn message_queue_clear() {
        let mut q = IpcMessageQueue::new(10);
        q.enqueue(IpcMessageBuilder::new().id(1).channel("a").build()).unwrap();
        q.enqueue(IpcMessageBuilder::new().id(2).channel("b").build()).unwrap();
        assert_eq!(q.capacity(), 10);
        q.clear();
        assert!(q.is_empty());
    }

    #[test]
    fn connection_pool_add_and_get() {
        let mut pool = IpcConnectionPool::new(5);
        pool.add(IpcConnection::new("c1", "editor")).unwrap();
        let conn = pool.get("c1").unwrap();
        assert_eq!(conn.channel, "editor");
        assert!(conn.connected);
        assert_eq!(conn.total_messages(), 0);
    }

    #[test]
    fn connection_pool_duplicate_id_error() {
        let mut pool = IpcConnectionPool::new(5);
        pool.add(IpcConnection::new("c1", "editor")).unwrap();
        let err = pool.add(IpcConnection::new("c1", "other")).unwrap_err();
        assert!(matches!(err, IpcError::DuplicateChannel(_)));
    }

    #[test]
    fn connection_pool_full_error() {
        let mut pool = IpcConnectionPool::new(1);
        pool.add(IpcConnection::new("c1", "editor")).unwrap();
        let err = pool.add(IpcConnection::new("c2", "lsp")).unwrap_err();
        assert!(matches!(err, IpcError::MessageTooLarge { .. }));
    }

    #[test]
    fn connection_pool_remove() {
        let mut pool = IpcConnectionPool::new(5);
        pool.add(IpcConnection::new("c1", "editor")).unwrap();
        let removed = pool.remove("c1").unwrap();
        assert_eq!(removed.id, "c1");
        assert!(pool.is_empty());
        assert!(pool.remove("c1").is_err());
    }

    #[test]
    fn connection_pool_disconnect_reconnect() {
        let mut pool = IpcConnectionPool::new(5);
        pool.add(IpcConnection::new("c1", "editor")).unwrap();
        pool.disconnect("c1").unwrap();
        assert!(!pool.get("c1").unwrap().connected);
        assert!(pool.active_connections().is_empty());
        pool.reconnect("c1").unwrap();
        assert!(pool.get("c1").unwrap().connected);
        assert_eq!(pool.active_connections().len(), 1);
    }

    #[test]
    fn connection_pool_record_send_receive() {
        let mut pool = IpcConnectionPool::new(5);
        pool.add(IpcConnection::new("c1", "editor")).unwrap();
        pool.record_send("c1").unwrap();
        pool.record_send("c1").unwrap();
        pool.record_receive("c1").unwrap();
        let conn = pool.get("c1").unwrap();
        assert_eq!(conn.messages_sent, 2);
        assert_eq!(conn.messages_received, 1);
        assert_eq!(conn.total_messages(), 3);
    }

    #[test]
    fn connection_pool_send_disconnected_error() {
        let mut pool = IpcConnectionPool::new(5);
        pool.add(IpcConnection::new("c1", "editor")).unwrap();
        pool.disconnect("c1").unwrap();
        let err = pool.record_send("c1").unwrap_err();
        assert!(matches!(err, IpcError::ChannelDisconnected(_)));
    }

    #[test]
    fn connection_display() {
        let conn = IpcConnection::new("c1", "editor");
        let s = format!("{conn}");
        assert!(s.contains("c1"));
        assert!(s.contains("editor"));
        assert!(s.contains("true"));
    }

    #[test]
    fn serialize_deserialize_request_envelope() {
        let payload = b"hello world";
        let data = ipc_serialize_envelope(MessageKind::Request, 42, payload);
        let (kind, id, decoded_payload) = ipc_deserialize_envelope(&data).unwrap();
        assert_eq!(kind, MessageKind::Request);
        assert_eq!(id, 42);
        assert_eq!(decoded_payload, payload);
    }

    #[test]
    fn serialize_deserialize_response_envelope() {
        let data = ipc_serialize_envelope(MessageKind::Response, 100, b"result");
        let (kind, id, payload) = ipc_deserialize_envelope(&data).unwrap();
        assert_eq!(kind, MessageKind::Response);
        assert_eq!(id, 100);
        assert_eq!(payload, b"result");
    }

    #[test]
    fn serialize_deserialize_notification_envelope() {
        let data = ipc_serialize_envelope(MessageKind::Notification, 0, &[]);
        let (kind, id, payload) = ipc_deserialize_envelope(&data).unwrap();
        assert_eq!(kind, MessageKind::Notification);
        assert_eq!(id, 0);
        assert!(payload.is_empty());
    }

    #[test]
    fn deserialize_envelope_too_short() {
        let data = vec![0u8; 5];
        assert!(ipc_deserialize_envelope(&data).is_err());
    }

    #[test]
    fn serialize_envelope_roundtrip_large_payload() {
        let payload: Vec<u8> = (0..1000).map(|i| (i % 256) as u8).collect();
        let data = ipc_serialize_envelope(MessageKind::Request, u64::MAX, &payload);
        let (kind, id, decoded) = ipc_deserialize_envelope(&data).unwrap();
        assert_eq!(kind, MessageKind::Request);
        assert_eq!(id, u64::MAX);
        assert_eq!(decoded, payload);
    }

    #[test]
    fn connection_pool_active_filters_correctly() {
        let mut pool = IpcConnectionPool::new(5);
        pool.add(IpcConnection::new("c1", "a")).unwrap();
        pool.add(IpcConnection::new("c2", "b")).unwrap();
        pool.add(IpcConnection::new("c3", "c")).unwrap();
        pool.disconnect("c2").unwrap();
        let active = pool.active_connections();
        assert_eq!(active.len(), 2);
        assert!(active.iter().all(|c| c.connected));
    }

    #[test]
    fn message_queue_dequeue_returns_none_when_empty() {
        let mut q = IpcMessageQueue::new(10);
        assert!(q.dequeue().is_none());
    }

    #[test]
    fn connection_pool_get_nonexistent() {
        let pool = IpcConnectionPool::new(5);
        assert!(pool.get("nope").is_none());
    }

    #[test]
    fn connection_pool_record_receive_unknown() {
        let mut pool = IpcConnectionPool::new(5);
        assert!(pool.record_receive("unknown").is_err());
    }

    #[test]
    fn envelope_ipc_envelope_struct() {
        let env = IpcEnvelope {
            header: IpcMessageHeader { kind: MessageKind::Request, id: 1, method: Some("test".into()) },
            payload: vec![1, 2, 3],
            total_length: 16,
        };
        assert_eq!(env.total_length, 16);
        assert_eq!(env.payload.len(), 3);
    }

    #[test]
    fn ipc_stats_new_defaults() {
        let stats = IpcStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn ipc_stats_record_success() {
        let mut stats = IpcStats::new();
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
    fn ipc_stats_record_failure() {
        let mut stats = IpcStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn ipc_stats_reset() {
        let mut stats = IpcStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn ipc_stats_merge() {
        let mut a = IpcStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = IpcStats::new();
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
    fn ipc_stats_display() {
        let mut stats = IpcStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn ipc_stats_default() {
        let stats = IpcStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn ipc_validator_accepts_valid_name() {
        let v = IpcValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn ipc_validator_rejects_empty() {
        let v = IpcValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn ipc_validator_rejects_too_long() {
        let v = IpcValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn ipc_validator_forbidden_prefix() {
        let v = IpcValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn ipc_validator_allowed_chars() {
        let v = IpcValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn ipc_validator_range() {
        let v = IpcValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn ipc_sanitize_removes_control() {
        let result = IpcValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn ipc_truncate_short_string() {
        assert_eq!(IpcValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn ipc_truncate_long_string() {
        let result = IpcValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn ipc_is_ascii_printable() {
        assert!(IpcValidator::is_ascii_printable("Hello World 123"));
        assert!(!IpcValidator::is_ascii_printable("Hello\x00World"));
    }

    // ---- IpcRouter tests ----

    #[test]
    fn router_exact_match() {
        let mut router = IpcRouter::new();
        router.add_route("shutdown", "lifecycle");
        assert_eq!(router.resolve("shutdown"), Some("lifecycle"));
        assert_eq!(router.resolve("exit"), None);
    }

    #[test]
    fn router_glob_pattern() {
        let mut router = IpcRouter::new();
        router.add_route("textDocument/*", "editor");
        router.add_route("workspace/*", "workspace");
        assert_eq!(router.resolve("textDocument/completion"), Some("editor"));
        assert_eq!(router.resolve("workspace/symbol"), Some("workspace"));
        assert_eq!(router.resolve("textDocument"), None);
    }

    #[test]
    fn router_remove_handler() {
        let mut router = IpcRouter::new();
        router.add_route("a", "h1");
        router.add_route("b", "h1");
        router.add_route("c", "h2");
        assert_eq!(router.remove_handler("h1"), 2);
        assert_eq!(router.route_count(), 1);
    }

    #[test]
    fn router_resolve_all() {
        let mut router = IpcRouter::new();
        router.add_route("textDocument/*", "editor");
        router.add_route("textDocument/*", "logger");
        let handlers = router.resolve_all("textDocument/hover");
        assert_eq!(handlers.len(), 2);
        assert!(handlers.contains(&"editor"));
        assert!(handlers.contains(&"logger"));
    }

    // ---- IpcRateLimiter tests ----

    #[test]
    fn rate_limiter_basic() {
        let mut rl = IpcRateLimiter::new(3);
        assert!(rl.try_acquire());
        assert!(rl.try_acquire());
        assert!(rl.try_acquire());
        assert!(!rl.try_acquire());
        assert_eq!(rl.denied(), 1);
        rl.refill(2);
        assert_eq!(rl.available(), 2);
        assert!(rl.try_acquire());
    }

    #[test]
    fn rate_limiter_acquire_n() {
        let mut rl = IpcRateLimiter::new(10);
        assert!(rl.try_acquire_n(5));
        assert_eq!(rl.available(), 5);
        assert!(!rl.try_acquire_n(6));
        assert!(rl.try_acquire_n(5));
        assert_eq!(rl.available(), 0);
    }

    #[test]
    fn rate_limiter_refill_caps_at_capacity() {
        let mut rl = IpcRateLimiter::new(5);
        rl.try_acquire();
        rl.refill(100);
        assert_eq!(rl.available(), 5);
    }

    #[test]
    fn rate_limiter_reset() {
        let mut rl = IpcRateLimiter::new(5);
        rl.try_acquire();
        rl.try_acquire();
        rl.reset();
        assert_eq!(rl.available(), 5);
        assert_eq!(rl.denied(), 0);
    }

    // ---- IpcMessageBatch tests ----

    #[test]
    fn batch_add_and_drain() {
        let mut batch = IpcMessageBatch::new(3);
        assert!(batch.is_empty());
        let msg = IpcMessageBuilder::new().id(1).channel("ch").payload(b"hi".to_vec()).build();
        batch.add(msg).unwrap();
        assert_eq!(batch.len(), 1);
        let msg2 = IpcMessageBuilder::new().id(2).channel("ch").payload(b"there".to_vec()).build();
        batch.add(msg2).unwrap();
        assert_eq!(batch.total_payload_bytes(), 7); // "hi" + "there"
        let drained = batch.drain();
        assert_eq!(drained.len(), 2);
        assert!(batch.is_empty());
    }

    #[test]
    fn batch_full_error() {
        let mut batch = IpcMessageBatch::new(1);
        let msg = IpcMessageBuilder::new().id(1).channel("ch").build();
        batch.add(msg).unwrap();
        assert!(batch.is_full());
        let msg2 = IpcMessageBuilder::new().id(2).channel("ch").build();
        assert!(batch.add(msg2).is_err());
    }

    #[test]
    fn batch_channels() {
        let mut batch = IpcMessageBatch::new(10);
        batch.add(IpcMessageBuilder::new().id(1).channel("a").build()).unwrap();
        batch.add(IpcMessageBuilder::new().id(2).channel("b").build()).unwrap();
        batch.add(IpcMessageBuilder::new().id(3).channel("a").build()).unwrap();
        let chs = batch.channels();
        assert_eq!(chs, vec!["a", "b"]);
    }

    // -- IpcMessageFilter tests -----------------------------------------------

    #[test]
    fn filter_by_channel_prefix() {
        let msgs = vec![
            IpcMessageBuilder::new().id(1).channel("textDocument/completion").build(),
            IpcMessageBuilder::new().id(2).channel("textDocument/hover").build(),
            IpcMessageBuilder::new().id(3).channel("workspace/symbol").build(),
        ];
        let filter = IpcMessageFilter::new().channel_prefix("textDocument/");
        let matched = filter.apply(&msgs);
        assert_eq!(matched.len(), 2);
        assert_eq!(matched[0].id, 1);
        assert_eq!(matched[1].id, 2);
    }

    #[test]
    fn filter_by_payload_size() {
        let msgs = vec![
            IpcMessageBuilder::new().id(1).channel("a").payload(vec![1, 2, 3]).build(),
            IpcMessageBuilder::new().id(2).channel("a").payload(vec![1]).build(),
            IpcMessageBuilder::new().id(3).channel("a").payload(vec![1, 2, 3, 4, 5]).build(),
        ];
        let filter = IpcMessageFilter::new().min_payload(2).max_payload(4);
        let matched = filter.apply(&msgs);
        assert_eq!(matched.len(), 1);
        assert_eq!(matched[0].id, 1);
    }

    #[test]
    fn filter_by_id_range() {
        let msgs = vec![
            IpcMessageBuilder::new().id(5).channel("a").build(),
            IpcMessageBuilder::new().id(10).channel("a").build(),
            IpcMessageBuilder::new().id(15).channel("a").build(),
        ];
        let filter = IpcMessageFilter::new().id_range(6, 12);
        let matched = filter.apply(&msgs);
        assert_eq!(matched.len(), 1);
        assert_eq!(matched[0].id, 10);
    }

    #[test]
    fn filter_empty_criteria_matches_all() {
        let msgs = vec![
            IpcMessageBuilder::new().id(1).channel("x").build(),
            IpcMessageBuilder::new().id(2).channel("y").build(),
        ];
        let filter = IpcMessageFilter::new();
        assert_eq!(filter.apply(&msgs).len(), 2);
    }

    #[test]
    fn batch_messages_for_channel() {
        let mut batch = IpcMessageBatch::new(10);
        batch.add(IpcMessageBuilder::new().id(1).channel("a").build()).unwrap();
        batch.add(IpcMessageBuilder::new().id(2).channel("b").build()).unwrap();
        batch.add(IpcMessageBuilder::new().id(3).channel("a").build()).unwrap();
        let a_msgs = batch.messages_for_channel("a");
        assert_eq!(a_msgs.len(), 2);
        assert_eq!(batch.channel_count(), 2);
    }

    #[test]
    fn connection_pool_stats() {
        let mut pool = IpcConnectionPool::new(10);
        pool.add(IpcConnection::new("c1", "ch1")).unwrap();
        pool.add(IpcConnection::new("c2", "ch2")).unwrap();
        pool.record_send("c1").unwrap();
        pool.record_send("c1").unwrap();
        pool.record_receive("c2").unwrap();
        assert_eq!(pool.total_sent(), 2);
        assert_eq!(pool.total_received(), 1);
        let busiest = pool.busiest_connection().unwrap();
        assert_eq!(busiest.id, "c1");
    }

    #[test]
    fn connection_pool_disconnect_all() {
        let mut pool = IpcConnectionPool::new(10);
        pool.add(IpcConnection::new("c1", "ch1")).unwrap();
        pool.add(IpcConnection::new("c2", "ch2")).unwrap();
        pool.disconnect_all();
        assert_eq!(pool.active_connections().len(), 0);
    }
}
