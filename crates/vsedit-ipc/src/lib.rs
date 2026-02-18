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

// ---------------------------------------------------------------------------
// IpcFrameCodec – length-delimited message framing for stream protocols
// ---------------------------------------------------------------------------

/// Codec for framing/deframing messages on a byte stream using a 4-byte
/// big-endian length prefix. Handles incremental parsing of partial reads.
#[derive(Debug)]
pub struct IpcFrameCodec {
    buf: Vec<u8>,
}

impl IpcFrameCodec {
    pub fn new() -> Self {
        Self { buf: Vec::new() }
    }

    /// Encode a single message into a length-prefixed frame.
    pub fn encode(payload: &[u8]) -> Vec<u8> {
        let len = payload.len() as u32;
        let mut frame = Vec::with_capacity(4 + payload.len());
        frame.extend_from_slice(&len.to_be_bytes());
        frame.extend_from_slice(payload);
        frame
    }

    /// Feed incoming bytes into the internal buffer.
    pub fn feed(&mut self, data: &[u8]) {
        self.buf.extend_from_slice(data);
    }

    /// Try to extract the next complete frame from the buffer.
    /// Returns `None` if not enough data is available yet.
    pub fn decode_next(&mut self) -> Option<Vec<u8>> {
        if self.buf.len() < 4 {
            return None;
        }
        let len = u32::from_be_bytes([self.buf[0], self.buf[1], self.buf[2], self.buf[3]]) as usize;
        if self.buf.len() < 4 + len {
            return None;
        }
        let payload = self.buf[4..4 + len].to_vec();
        self.buf.drain(..4 + len);
        Some(payload)
    }

    /// Decode all complete frames currently available in the buffer.
    pub fn decode_all(&mut self) -> Vec<Vec<u8>> {
        let mut frames = Vec::new();
        while let Some(frame) = self.decode_next() {
            frames.push(frame);
        }
        frames
    }

    /// Number of buffered bytes not yet consumed.
    pub fn buffered(&self) -> usize {
        self.buf.len()
    }

    /// Discard all buffered data.
    pub fn reset(&mut self) {
        self.buf.clear();
    }
}

impl Default for IpcFrameCodec {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// IpcPendingRequests – request-response correlation tracker
// ---------------------------------------------------------------------------

/// Tracks in-flight requests so responses can be correlated back to them.
#[derive(Debug)]
pub struct IpcPendingRequests {
    pending: Vec<PendingEntry>,
}

#[derive(Debug, Clone)]
struct PendingEntry {
    id: u64,
    method: String,
    sent_tick: u64,
}

impl IpcPendingRequests {
    pub fn new() -> Self {
        Self { pending: Vec::new() }
    }

    /// Register a request as pending. `tick` is an opaque monotonic counter
    /// used for timeout detection.
    pub fn register(&mut self, id: u64, method: &str, tick: u64) {
        self.pending.push(PendingEntry {
            id,
            method: method.to_string(),
            sent_tick: tick,
        });
    }

    /// Complete a pending request, returning the method name if found.
    pub fn complete(&mut self, id: u64) -> Option<String> {
        let pos = self.pending.iter().position(|e| e.id == id)?;
        Some(self.pending.remove(pos).method)
    }

    /// Return the number of in-flight requests.
    pub fn count(&self) -> usize {
        self.pending.len()
    }

    /// Return IDs of all requests whose `sent_tick` is older than `deadline`.
    pub fn timed_out(&self, deadline: u64) -> Vec<u64> {
        self.pending
            .iter()
            .filter(|e| e.sent_tick < deadline)
            .map(|e| e.id)
            .collect()
    }

    /// Expire (remove) all requests older than `deadline`, returning their IDs.
    pub fn expire(&mut self, deadline: u64) -> Vec<u64> {
        let mut expired = Vec::new();
        self.pending.retain(|e| {
            if e.sent_tick < deadline {
                expired.push(e.id);
                false
            } else {
                true
            }
        });
        expired
    }

    /// Check whether a specific request ID is still pending.
    pub fn is_pending(&self, id: u64) -> bool {
        self.pending.iter().any(|e| e.id == id)
    }
}

impl Default for IpcPendingRequests {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// IpcProtocolNegotiator – version negotiation for IPC handshake
// ---------------------------------------------------------------------------

/// Negotiates protocol version between two peers during handshake.
#[derive(Debug, Clone)]
pub struct IpcProtocolNegotiator {
    supported: Vec<(u16, u16)>,
}

impl IpcProtocolNegotiator {
    /// Create a negotiator that supports the given `(major, minor)` versions.
    /// Versions should be added in order of preference (most preferred first).
    pub fn new(supported: &[(u16, u16)]) -> Self {
        Self {
            supported: supported.to_vec(),
        }
    }

    /// Given a remote peer's list of supported versions, pick the best common
    /// version. Preference follows the local ordering.
    pub fn negotiate(&self, remote: &[(u16, u16)]) -> Option<(u16, u16)> {
        for &local_ver in &self.supported {
            if remote.contains(&local_ver) {
                return Some(local_ver);
            }
        }
        None
    }

    /// Encode the supported version list into a compact byte representation.
    /// Format: `[u16 count][major1 minor1][major2 minor2]...` all big-endian.
    pub fn encode_versions(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(2 + self.supported.len() * 4);
        buf.extend_from_slice(&(self.supported.len() as u16).to_be_bytes());
        for &(major, minor) in &self.supported {
            buf.extend_from_slice(&major.to_be_bytes());
            buf.extend_from_slice(&minor.to_be_bytes());
        }
        buf
    }

    /// Decode a version list produced by [`encode_versions`].
    pub fn decode_versions(data: &[u8]) -> Option<Vec<(u16, u16)>> {
        if data.len() < 2 {
            return None;
        }
        let count = u16::from_be_bytes([data[0], data[1]]) as usize;
        if data.len() < 2 + count * 4 {
            return None;
        }
        let mut versions = Vec::with_capacity(count);
        for i in 0..count {
            let off = 2 + i * 4;
            let major = u16::from_be_bytes([data[off], data[off + 1]]);
            let minor = u16::from_be_bytes([data[off + 2], data[off + 3]]);
            versions.push((major, minor));
        }
        Some(versions)
    }

    /// Check whether a specific version is supported locally.
    pub fn supports(&self, version: (u16, u16)) -> bool {
        self.supported.contains(&version)
    }
}

// ---------------------------------------------------------------------------
// IpcConnectionState – finite state machine for connection lifecycle
// ---------------------------------------------------------------------------

/// States a connection can be in during its lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    Idle,
    Connecting,
    Handshaking,
    Ready,
    Draining,
    Disconnected,
    Failed,
}

impl fmt::Display for ConnectionState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConnectionState::Idle => write!(f, "Idle"),
            ConnectionState::Connecting => write!(f, "Connecting"),
            ConnectionState::Handshaking => write!(f, "Handshaking"),
            ConnectionState::Ready => write!(f, "Ready"),
            ConnectionState::Draining => write!(f, "Draining"),
            ConnectionState::Disconnected => write!(f, "Disconnected"),
            ConnectionState::Failed => write!(f, "Failed"),
        }
    }
}

/// Manages connection state transitions with validation.
#[derive(Debug, Clone)]
pub struct IpcConnectionStateMachine {
    state: ConnectionState,
    transition_count: u64,
}

impl IpcConnectionStateMachine {
    pub fn new() -> Self {
        Self {
            state: ConnectionState::Idle,
            transition_count: 0,
        }
    }

    pub fn state(&self) -> ConnectionState {
        self.state
    }

    pub fn transitions(&self) -> u64 {
        self.transition_count
    }

    /// Attempt a state transition. Returns `Err` with a description if the
    /// transition is invalid.
    pub fn transition(&mut self, to: ConnectionState) -> Result<(), String> {
        if !Self::valid_transition(self.state, to) {
            return Err(format!(
                "invalid transition: {} -> {}",
                self.state, to
            ));
        }
        self.state = to;
        self.transition_count += 1;
        Ok(())
    }

    /// Whether a transition from `from` to `to` is allowed.
    pub fn valid_transition(from: ConnectionState, to: ConnectionState) -> bool {
        matches!(
            (from, to),
            (ConnectionState::Idle, ConnectionState::Connecting)
                | (ConnectionState::Connecting, ConnectionState::Handshaking)
                | (ConnectionState::Connecting, ConnectionState::Failed)
                | (ConnectionState::Handshaking, ConnectionState::Ready)
                | (ConnectionState::Handshaking, ConnectionState::Failed)
                | (ConnectionState::Ready, ConnectionState::Draining)
                | (ConnectionState::Ready, ConnectionState::Disconnected)
                | (ConnectionState::Ready, ConnectionState::Failed)
                | (ConnectionState::Draining, ConnectionState::Disconnected)
                | (ConnectionState::Draining, ConnectionState::Failed)
                | (ConnectionState::Disconnected, ConnectionState::Connecting)
                | (ConnectionState::Failed, ConnectionState::Connecting)
        )
    }

    /// Whether the connection is in a terminal state (Disconnected or Failed).
    pub fn is_terminal(&self) -> bool {
        matches!(
            self.state,
            ConnectionState::Disconnected | ConnectionState::Failed
        )
    }

    /// Whether the connection is active and can send/receive messages.
    pub fn is_active(&self) -> bool {
        self.state == ConnectionState::Ready
    }

    /// Reset to Idle.
    pub fn reset(&mut self) {
        self.state = ConnectionState::Idle;
        self.transition_count = 0;
    }
}

impl Default for IpcConnectionStateMachine {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// IpcMultiplexer – multiplex multiple logical channels over a single stream
// ---------------------------------------------------------------------------

/// Multiplexes multiple logical channels over a single transport, tagging
/// each frame with a channel ID.
#[derive(Debug)]
pub struct IpcMultiplexer {
    channels: Vec<String>,
}

impl IpcMultiplexer {
    pub fn new() -> Self {
        Self {
            channels: Vec::new(),
        }
    }

    /// Register a logical channel, returning its numeric ID.
    pub fn open_channel(&mut self, name: &str) -> Result<u16, IpcError> {
        if self.channels.iter().any(|n| n == name) {
            return Err(IpcError::DuplicateChannel(name.to_string()));
        }
        if self.channels.len() >= u16::MAX as usize {
            return Err(IpcError::MessageTooLarge {
                size: self.channels.len() + 1,
                max: u16::MAX as usize,
            });
        }
        let id = self.channels.len() as u16;
        self.channels.push(name.to_string());
        Ok(id)
    }

    /// Look up the numeric ID for a channel name.
    pub fn channel_id(&self, name: &str) -> Option<u16> {
        self.channels.iter().position(|n| n == name).map(|p| p as u16)
    }

    /// Look up the channel name for a numeric ID.
    pub fn channel_name(&self, id: u16) -> Option<&str> {
        self.channels.get(id as usize).map(|s| s.as_str())
    }

    /// Wrap a payload with a 2-byte channel ID header for multiplexing.
    pub fn wrap(&self, channel_id: u16, payload: &[u8]) -> Vec<u8> {
        let mut frame = Vec::with_capacity(2 + payload.len());
        frame.extend_from_slice(&channel_id.to_be_bytes());
        frame.extend_from_slice(payload);
        frame
    }

    /// Unwrap a multiplexed frame, returning `(channel_id, payload)`.
    pub fn unwrap(data: &[u8]) -> Option<(u16, &[u8])> {
        if data.len() < 2 {
            return None;
        }
        let ch = u16::from_be_bytes([data[0], data[1]]);
        Some((ch, &data[2..]))
    }

    /// Number of open channels.
    pub fn channel_count(&self) -> usize {
        self.channels.len()
    }
}

impl Default for IpcMultiplexer {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// vsedit-ipc: Extended configuration, caching, and iteration utilities
// ---------------------------------------------------------------------------

/// Configuration entry with key-value metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IpcXConfig {
    pub key: String,
    pub value: String,
    pub tags: Vec<String>,
    pub weight: u32,
    pub active: bool,
}

impl IpcXConfig {
    pub fn new(key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: String::new(),
            tags: Vec::new(),
            weight: 0,
            active: true,
        }
    }

    pub fn with_value(mut self, v: impl Into<String>) -> Self {
        self.value = v.into();
        self
    }

    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    pub fn with_weight(mut self, w: u32) -> Self {
        self.weight = w;
        self
    }

    pub fn deactivate(mut self) -> Self {
        self.active = false;
        self
    }

    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }

    pub fn tag_count(&self) -> usize {
        self.tags.len()
    }
}

impl std::fmt::Display for IpcXConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}={}", self.key, self.value)
    }
}

/// Registry that stores and indexes configuration entries.
#[derive(Debug, Default)]
pub struct IpcXRegistry {
    entries: Vec<IpcXConfig>,
    index: std::collections::HashMap<String, usize>,
}

impl IpcXRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, entry: IpcXConfig) -> Result<(), String> {
        if self.index.contains_key(&entry.key) {
            return Err(format!("duplicate key: {}", entry.key));
        }
        let idx = self.entries.len();
        self.index.insert(entry.key.clone(), idx);
        self.entries.push(entry);
        Ok(())
    }

    pub fn get(&self, key: &str) -> Option<&IpcXConfig> {
        self.index.get(key).map(|&i| &self.entries[i])
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut IpcXConfig> {
        self.index.get(key).copied().map(move |i| &mut self.entries[i])
    }

    pub fn remove(&mut self, key: &str) -> Option<IpcXConfig> {
        if let Some(&idx) = self.index.get(key) {
            self.index.remove(key);
            let removed = self.entries.remove(idx);
            for val in self.index.values_mut() {
                if *val > idx {
                    *val -= 1;
                }
            }
            Some(removed)
        } else {
            None
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.key.as_str()).collect()
    }

    pub fn active_entries(&self) -> Vec<&IpcXConfig> {
        self.entries.iter().filter(|e| e.active).collect()
    }

    pub fn by_weight_desc(&self) -> Vec<&IpcXConfig> {
        let mut sorted: Vec<&IpcXConfig> = self.entries.iter().collect();
        sorted.sort_by(|a, b| b.weight.cmp(&a.weight));
        sorted
    }

    pub fn entries_with_tag(&self, tag: &str) -> Vec<&IpcXConfig> {
        self.entries.iter().filter(|e| e.has_tag(tag)).collect()
    }

    pub fn contains(&self, key: &str) -> bool {
        self.index.contains_key(key)
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.index.clear();
    }

    pub fn total_weight(&self) -> u32 {
        self.entries.iter().map(|e| e.weight).sum()
    }

    pub fn iter(&self) -> IpcXIterator<'_> {
        IpcXIterator { inner: self.entries.iter() }
    }
}

/// Iterator over registry entries.
pub struct IpcXIterator<'a> {
    inner: std::slice::Iter<'a, IpcXConfig>,
}

impl<'a> Iterator for IpcXIterator<'a> {
    type Item = &'a IpcXConfig;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

/// LRU cache with capacity limit.
#[derive(Debug)]
pub struct IpcXCache {
    capacity: usize,
    entries: Vec<(String, String)>,
}

impl IpcXCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            entries: Vec::new(),
        }
    }

    pub fn get(&mut self, key: &str) -> Option<&str> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            let entry = self.entries.remove(pos);
            self.entries.push(entry);
            self.entries.last().map(|(_, v)| v.as_str())
        } else {
            None
        }
    }

    pub fn put(&mut self, key: impl Into<String>, value: impl Into<String>) {
        let key = key.into();
        self.entries.retain(|(k, _)| k != &key);
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value.into()));
    }

    pub fn contains(&self, key: &str) -> bool {
        self.entries.iter().any(|(k, _)| k == key)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    pub fn most_recent(&self) -> Option<(&str, &str)> {
        self.entries.last().map(|(k, v)| (k.as_str(), v.as_str()))
    }

    pub fn least_recent(&self) -> Option<(&str, &str)> {
        self.entries.first().map(|(k, v)| (k.as_str(), v.as_str()))
    }
}

/// Formatter for rendering entries as text.
pub struct IpcXFormatter {
    separator: String,
    show_inactive: bool,
    max_value_len: usize,
}

impl IpcXFormatter {
    pub fn new() -> Self {
        Self {
            separator: ", ".to_string(),
            show_inactive: false,
            max_value_len: 80,
        }
    }

    pub fn separator(mut self, sep: impl Into<String>) -> Self {
        self.separator = sep.into();
        self
    }

    pub fn show_inactive(mut self, show: bool) -> Self {
        self.show_inactive = show;
        self
    }

    pub fn max_value_len(mut self, len: usize) -> Self {
        self.max_value_len = len;
        self
    }

    pub fn format_entry(&self, entry: &IpcXConfig) -> String {
        let val = if entry.value.len() > self.max_value_len {
            format!("{}…", &entry.value[..self.max_value_len])
        } else {
            entry.value.clone()
        };
        let status = if entry.active { "✓" } else { "✗" };
        format!("[{}] {}={}", status, entry.key, val)
    }

    pub fn format_list(&self, registry: &IpcXRegistry) -> String {
        let items: Vec<String> = registry.entries.iter()
            .filter(|e| self.show_inactive || e.active)
            .map(|e| self.format_entry(e))
            .collect();
        items.join(&self.separator)
    }

    pub fn format_summary(&self, registry: &IpcXRegistry) -> String {
        let active = registry.active_entries().len();
        let total = registry.len();
        format!("{} active / {} total (weight: {})", active, total, registry.total_weight())
    }
}

impl Default for IpcXFormatter {
    fn default() -> Self {
        Self::new()
    }
}

/// Validator for configuration entries.
pub struct IpcXValidator {
    max_key_len: usize,
    require_value: bool,
    allowed_tags: Option<Vec<String>>,
}

impl IpcXValidator {
    pub fn new() -> Self {
        Self {
            max_key_len: 256,
            require_value: false,
            allowed_tags: None,
        }
    }

    pub fn max_key_len(mut self, len: usize) -> Self {
        self.max_key_len = len;
        self
    }

    pub fn require_value(mut self, req: bool) -> Self {
        self.require_value = req;
        self
    }

    pub fn allowed_tags(mut self, tags: Vec<String>) -> Self {
        self.allowed_tags = Some(tags);
        self
    }

    pub fn validate(&self, entry: &IpcXConfig) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        if entry.key.is_empty() {
            errors.push("key must not be empty".into());
        }
        if entry.key.len() > self.max_key_len {
            errors.push(format!("key exceeds max length {}", self.max_key_len));
        }
        if self.require_value && entry.value.is_empty() {
            errors.push("value is required".into());
        }
        if let Some(ref allowed) = self.allowed_tags {
            for tag in &entry.tags {
                if !allowed.contains(tag) {
                    errors.push(format!("tag '{}' is not allowed", tag));
                }
            }
        }
        if errors.is_empty() { Ok(()) } else { Err(errors) }
    }

    pub fn validate_all(&self, registry: &IpcXRegistry) -> Vec<(String, Vec<String>)> {
        let mut results = Vec::new();
        for entry in &registry.entries {
            if let Err(errs) = self.validate(entry) {
                results.push((entry.key.clone(), errs));
            }
        }
        results
    }
}

impl Default for IpcXValidator {
    fn default() -> Self {
        Self::new()
    }
}



// ---------------------------------------------------------------------------
// ipc – Extended IPC flow control helpers
// ---------------------------------------------------------------------------

/// Priority levels for IPC flow control.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZIpcPriority {
    Idle,
    Low,
    Normal,
    High,
    Realtime,
}

impl ZIpcPriority {
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
    pub fn all_asc() -> [ZIpcPriority; 5] {
        [Self::Idle, Self::Low, Self::Normal, Self::High, Self::Realtime]
    }
}

impl fmt::Display for ZIpcPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks IPC flow control data.
#[derive(Debug, Clone)]
pub struct ZIpcIpcFlowControl {
    pub window_sizes: Vec<u32>,
    pub paused: bool,
    pub credits: u64,
}

impl ZIpcIpcFlowControl {
    /// Create with default values.
    pub fn new() -> Self {
        Self {
            window_sizes: Vec::new(),
            paused: false,
            credits: 0,
        }
    }

    /// Number of items in the primary collection.
    pub fn len(&self) -> usize {
        self.window_sizes.len()
    }

    /// Whether the primary collection is empty.
    pub fn is_empty(&self) -> bool {
        self.window_sizes.is_empty()
    }

    /// Clear the primary collection.
    pub fn clear(&mut self) {
        self.window_sizes.clear();
    }

    /// Produce a debug summary string.
    pub fn summary(&self) -> String {
        format!("ZIpcIpcFlowControl[paused={:?}, credits={:?}]", self.paused, self.credits)
    }

    /// Clone with the third field toggled (if bool) or kept as-is.
    pub fn toggled_clone(&self) -> Self {
        let c = self.clone();
        c
    }
}

/// Compute a simple rolling hash for IPC flow control.
pub fn z_ipc_rolling_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Pad `s` to exactly `width` chars, truncating or right-padding with spaces.
pub fn z_ipc_pad_to(s: &str, width: usize) -> String {
    if s.len() >= width {
        s[..width].to_string()
    } else {
        format!("{:<width$}", s, width = width)
    }
}

/// Check whether all characters in `s` are ASCII alphanumeric or underscore.
pub fn z_ipc_is_identifier(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// Compute the Levenshtein distance between two strings (simple O(n*m) impl).
pub fn z_ipc_levenshtein(a: &str, b: &str) -> usize {
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
pub fn z_ipc_unique_words(text: &str) -> Vec<&str> {
    let mut seen = std::collections::HashSet::new();
    text.split_whitespace().filter(|w| seen.insert(*w)).collect()
}

/// Chunk a slice into groups of `size`.
pub fn z_ipc_chunk_slice<T>(slice: &[T], size: usize) -> Vec<&[T]> {
    if size == 0 { return vec![]; }
    slice.chunks(size).collect()
}

/// Return the longest common prefix of two strings.
pub fn z_ipc_common_prefix<'a>(a: &'a str, b: &str) -> &'a str {
    let end = a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count();
    &a[..end]
}


// ── zq extended utilities ──

/// A lightweight tagged-value store for zq operations.
#[derive(Debug, Clone, PartialEq)]
pub struct ZqStore {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl ZqStore {
    /// Create a new store with the given capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: Vec::new(),
            capacity,
        }
    }

    /// Insert a key-value pair, evicting the oldest if at capacity.
    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<String>) -> bool {
        let key = key.into();
        let value = value.into();
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value));
        true
    }

    /// Look up a value by key.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries
            .iter()
            .rev()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    /// Remove all entries matching the given key, returning how many were removed.
    pub fn remove(&mut self, key: &str) -> usize {
        let before = self.entries.len();
        self.entries.retain(|(k, _)| k != key);
        before - self.entries.len()
    }

    /// Return the number of stored entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check whether the store is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Collect all keys in insertion order.
    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    /// Collect all values in insertion order.
    pub fn values(&self) -> Vec<&str> {
        self.entries.iter().map(|(_, v)| v.as_str()).collect()
    }

    /// Drain entries whose key starts with the given prefix.
    pub fn drain_prefix(&mut self, pfx: &str) -> Vec<(String, String)> {
        let mut drained = Vec::new();
        let mut i = 0;
        while i < self.entries.len() {
            if self.entries[i].0.starts_with(pfx) {
                drained.push(self.entries.remove(i));
            } else {
                i += 1;
            }
        }
        drained
    }

    /// Retain only entries satisfying the predicate.
    pub fn retain<F: Fn(&str, &str) -> bool>(&mut self, f: F) {
        self.entries.retain(|(k, v)| f(k, v));
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Return remaining capacity.
    pub fn remaining(&self) -> usize {
        self.capacity.saturating_sub(self.entries.len())
    }

    /// Merge another store into this one, respecting capacity.
    pub fn merge(&mut self, other: &ZqStore) {
        for (k, v) in &other.entries {
            if self.entries.len() >= self.capacity {
                break;
            }
            self.entries.push((k.clone(), v.clone()));
        }
    }
}

/// Format a byte count as a human-readable string for zq display.
pub fn zq_format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Truncate a string to `max_len` characters, appending an ellipsis if needed.
pub fn zq_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut result = s[..max_len.saturating_sub(3)].to_string();
        result.push_str("...");
        result
    }
}


// ---------------------------------------------------------------------------
// xb_ utilities – batch 90
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer90 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer90 {
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
pub fn xb_fnv1a_90(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_90<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
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
pub fn xb_rle_decode_90<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_90(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_90(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 99
// ---------------------------------------------------------------------------

/// Generic object pool `Xc99Pool<T>`.
pub struct Xc99Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc99Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc99PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc99Pool<T> {
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
    pub fn stats(&self) -> Xc99PoolStats {
        Xc99PoolStats {
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

impl<T> Default for Xc99Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc99Scheduler`.
pub struct Xc99Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc99Scheduler {
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

impl Default for Xc99Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_99 hash for the given byte slice.
pub fn xc_99_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_99 convention.
pub fn xc_99_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe103 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe103Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe103PipelineError {
    pub stage: Xe103Stage,
    pub message: String,
}

impl std::fmt::Display for Xe103PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe103Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe103Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe103PipelineError>>>,
    stage_names: Vec<Xe103Stage>,
}

impl Xe103Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe103PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe103Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe103PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe103Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe103PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe103Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe103PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe103Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe103PipelineError> {
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

    pub fn compose(mut self, other: Xe103Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe103CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe103CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe103Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe103CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe103CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe103Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe103CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_103_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe103CacheEntry {
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

    fn xe_103_evict_expired(&mut self) {
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

    pub fn stats(&self) -> &Xe103CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_103_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe103PipelineError> {
    Ok(data)
}

pub fn xe_103_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe103PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_103_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe103PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_103_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe103PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_103_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe103PipelineError> {
    Err(Xe103PipelineError {
        stage: Xe103Stage::Parse,
        message: "intentional failure".to_string(),
    })
}


// ---------------------------------------------------------------------------
// xg_101: Directed graph
// ---------------------------------------------------------------------------

/// A directed graph with adjacency-list representation.
#[derive(Debug, Clone)]
pub struct Xg101Graph {
    adj: std::collections::HashMap<usize, Vec<usize>>,
    edge_cnt: usize,
}

impl Xg101Graph {
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

impl Default for Xg101Graph {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// xg_101: Min-heap
// ---------------------------------------------------------------------------

/// A min-heap backed by a `Vec`.
#[derive(Debug, Clone)]
pub struct Xg101Heap<T: Ord> {
    data: Vec<T>,
}

impl<T: Ord> Xg101Heap<T> {
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
    pub fn merge(&mut self, other: &mut Xg101Heap<T>) {
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

impl<T: Ord> Default for Xg101Heap<T> {
    fn default() -> Self { Self::new() }
}


/// A probabilistic sorted list using a skip-list structure (variant 98).
pub struct Xh98SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh98SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 140 as u64,
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

/// A compact bit set supporting boolean operations (variant 98).
pub struct Xh98BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh98BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 98).
pub struct Xi98Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi98Deque<T> {
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
pub struct Xi98Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi98Interval {
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

/// A simple interval tree (variant 98).
pub struct Xi98IntervalTree {
    xi_intervals: Vec<Xi98Interval>,
}

impl Xi98IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi98Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi98Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi98Interval) -> Vec<&Xi98Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi98Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi98Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi98Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi98Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi98Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi98Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 98) ---

/// Disjoint set / union-find for crate 98.
pub struct Xj98UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj98UnionFind {
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

const XJ98_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 98.
pub struct Xj98BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj98BTreeNode<K, V>>>,
    len: usize,
}

struct Xj98BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj98BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj98BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ98_BTREE_ORDER - 1
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
        let mid = XJ98_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj98BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj98BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj98BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj98BTreeNode::xj_new_leaf();
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


// --- xk_98 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk98SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk98SegmentTree {
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
pub struct Xk98DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk98DisjointIntervals {
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


/// Rope data structure for efficient large text manipulation (xl_98).
#[derive(Debug, Clone)]
pub struct Xl98Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl98Rope {
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

/// Suffix array for efficient string searching (xl_98).
#[derive(Debug, Clone)]
pub struct Xl98SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl98SuffixArray {
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

    // ---- IpcFrameCodec tests ----

    #[test]
    fn frame_codec_encode_decode_single() {
        let payload = b"hello world";
        let frame = IpcFrameCodec::encode(payload);
        assert_eq!(frame.len(), 4 + payload.len());

        let mut codec = IpcFrameCodec::new();
        codec.feed(&frame);
        let decoded = codec.decode_next().unwrap();
        assert_eq!(decoded, payload);
        assert_eq!(codec.buffered(), 0);
    }

    #[test]
    fn frame_codec_partial_feed() {
        let payload = b"partial test";
        let frame = IpcFrameCodec::encode(payload);

        let mut codec = IpcFrameCodec::new();
        // Feed only the length prefix first
        codec.feed(&frame[..4]);
        assert!(codec.decode_next().is_none());
        // Feed the rest
        codec.feed(&frame[4..]);
        let decoded = codec.decode_next().unwrap();
        assert_eq!(decoded, payload);
    }

    #[test]
    fn frame_codec_multiple_frames() {
        let mut codec = IpcFrameCodec::new();
        let f1 = IpcFrameCodec::encode(b"one");
        let f2 = IpcFrameCodec::encode(b"two");
        let f3 = IpcFrameCodec::encode(b"three");

        // Feed all three at once (simulating a large read)
        let mut combined = Vec::new();
        combined.extend_from_slice(&f1);
        combined.extend_from_slice(&f2);
        combined.extend_from_slice(&f3);
        codec.feed(&combined);

        let frames = codec.decode_all();
        assert_eq!(frames.len(), 3);
        assert_eq!(frames[0], b"one");
        assert_eq!(frames[1], b"two");
        assert_eq!(frames[2], b"three");
        assert_eq!(codec.buffered(), 0);
    }

    #[test]
    fn frame_codec_empty_payload() {
        let frame = IpcFrameCodec::encode(b"");
        let mut codec = IpcFrameCodec::new();
        codec.feed(&frame);
        let decoded = codec.decode_next().unwrap();
        assert!(decoded.is_empty());
    }

    #[test]
    fn frame_codec_reset() {
        let mut codec = IpcFrameCodec::new();
        codec.feed(b"garbage");
        assert_eq!(codec.buffered(), 7);
        codec.reset();
        assert_eq!(codec.buffered(), 0);
    }

    // ---- IpcPendingRequests tests ----

    #[test]
    fn pending_requests_register_and_complete() {
        let mut pr = IpcPendingRequests::new();
        pr.register(1, "textDocument/completion", 100);
        pr.register(2, "textDocument/hover", 110);
        assert_eq!(pr.count(), 2);
        assert!(pr.is_pending(1));

        let method = pr.complete(1).unwrap();
        assert_eq!(method, "textDocument/completion");
        assert!(!pr.is_pending(1));
        assert_eq!(pr.count(), 1);

        assert!(pr.complete(999).is_none());
    }

    #[test]
    fn pending_requests_timeout_detection() {
        let mut pr = IpcPendingRequests::new();
        pr.register(1, "a", 10);
        pr.register(2, "b", 20);
        pr.register(3, "c", 30);

        let timed_out = pr.timed_out(25);
        assert_eq!(timed_out, vec![1, 2]);
        // timed_out doesn't remove them
        assert_eq!(pr.count(), 3);
    }

    #[test]
    fn pending_requests_expire() {
        let mut pr = IpcPendingRequests::new();
        pr.register(1, "a", 10);
        pr.register(2, "b", 20);
        pr.register(3, "c", 30);

        let expired = pr.expire(25);
        assert_eq!(expired, vec![1, 2]);
        assert_eq!(pr.count(), 1);
        assert!(pr.is_pending(3));
    }

    // ---- IpcProtocolNegotiator tests ----

    #[test]
    fn protocol_negotiator_picks_best_common() {
        let local = IpcProtocolNegotiator::new(&[(2, 0), (1, 1), (1, 0)]);
        let remote = &[(1, 0), (1, 1)];
        // Local prefers (2,0) but remote doesn't have it, so (1,1) wins
        assert_eq!(local.negotiate(remote), Some((1, 1)));
    }

    #[test]
    fn protocol_negotiator_no_common_version() {
        let local = IpcProtocolNegotiator::new(&[(3, 0)]);
        let remote = &[(1, 0), (2, 0)];
        assert_eq!(local.negotiate(remote), None);
    }

    #[test]
    fn protocol_negotiator_encode_decode_roundtrip() {
        let versions = vec![(1, 0), (1, 1), (2, 0)];
        let neg = IpcProtocolNegotiator::new(&versions);
        let encoded = neg.encode_versions();
        let decoded = IpcProtocolNegotiator::decode_versions(&encoded).unwrap();
        assert_eq!(decoded, versions);
    }

    #[test]
    fn protocol_negotiator_decode_too_short() {
        assert!(IpcProtocolNegotiator::decode_versions(&[]).is_none());
        assert!(IpcProtocolNegotiator::decode_versions(&[0, 2, 0, 1]).is_none());
    }

    #[test]
    fn protocol_negotiator_supports() {
        let neg = IpcProtocolNegotiator::new(&[(1, 0), (2, 0)]);
        assert!(neg.supports((1, 0)));
        assert!(!neg.supports((3, 0)));
    }

    // ---- IpcConnectionStateMachine tests ----

    #[test]
    fn state_machine_happy_path() {
        let mut sm = IpcConnectionStateMachine::new();
        assert_eq!(sm.state(), ConnectionState::Idle);
        assert!(!sm.is_active());
        assert!(!sm.is_terminal());

        sm.transition(ConnectionState::Connecting).unwrap();
        sm.transition(ConnectionState::Handshaking).unwrap();
        sm.transition(ConnectionState::Ready).unwrap();
        assert!(sm.is_active());
        assert_eq!(sm.transitions(), 3);

        sm.transition(ConnectionState::Draining).unwrap();
        assert!(!sm.is_active());
        sm.transition(ConnectionState::Disconnected).unwrap();
        assert!(sm.is_terminal());
    }

    #[test]
    fn state_machine_invalid_transition() {
        let mut sm = IpcConnectionStateMachine::new();
        let err = sm.transition(ConnectionState::Ready).unwrap_err();
        assert!(err.contains("invalid transition"));
    }

    #[test]
    fn state_machine_reconnect_after_failure() {
        let mut sm = IpcConnectionStateMachine::new();
        sm.transition(ConnectionState::Connecting).unwrap();
        sm.transition(ConnectionState::Failed).unwrap();
        assert!(sm.is_terminal());
        // Can reconnect from Failed
        sm.transition(ConnectionState::Connecting).unwrap();
        assert!(!sm.is_terminal());
    }

    #[test]
    fn state_machine_reset() {
        let mut sm = IpcConnectionStateMachine::new();
        sm.transition(ConnectionState::Connecting).unwrap();
        sm.transition(ConnectionState::Handshaking).unwrap();
        sm.reset();
        assert_eq!(sm.state(), ConnectionState::Idle);
        assert_eq!(sm.transitions(), 0);
    }

    #[test]
    fn connection_state_display() {
        assert_eq!(ConnectionState::Idle.to_string(), "Idle");
        assert_eq!(ConnectionState::Ready.to_string(), "Ready");
        assert_eq!(ConnectionState::Failed.to_string(), "Failed");
    }

    // ---- IpcMultiplexer tests ----

    #[test]
    fn multiplexer_open_and_wrap_unwrap() {
        let mut mux = IpcMultiplexer::new();
        let ch0 = mux.open_channel("editor").unwrap();
        let ch1 = mux.open_channel("lsp").unwrap();
        assert_eq!(ch0, 0);
        assert_eq!(ch1, 1);
        assert_eq!(mux.channel_count(), 2);

        assert_eq!(mux.channel_id("editor"), Some(0));
        assert_eq!(mux.channel_name(1), Some("lsp"));
        assert_eq!(mux.channel_id("unknown"), None);

        let frame = mux.wrap(ch0, b"payload");
        let (ch, payload) = IpcMultiplexer::unwrap(&frame).unwrap();
        assert_eq!(ch, ch0);
        assert_eq!(payload, b"payload");
    }

    #[test]
    fn multiplexer_duplicate_channel_error() {
        let mut mux = IpcMultiplexer::new();
        mux.open_channel("ch").unwrap();
        let err = mux.open_channel("ch").unwrap_err();
        assert!(matches!(err, IpcError::DuplicateChannel(_)));
    }

    #[test]
    fn multiplexer_unwrap_too_short() {
        assert!(IpcMultiplexer::unwrap(&[0]).is_none());
        assert!(IpcMultiplexer::unwrap(&[]).is_none());
        // Exactly 2 bytes = channel ID with empty payload
        let result = IpcMultiplexer::unwrap(&[0, 0]).unwrap();
        assert_eq!(result, (0, &[][..]));
    }

    #[test]
    fn ipc_x_config_new() {
        let c = IpcXConfig::new("mykey");
        assert_eq!(c.key, "mykey");
        assert!(c.active);
        assert_eq!(c.weight, 0);
        assert!(c.tags.is_empty());
    }

    #[test]
    fn ipc_x_config_builder() {
        let c = IpcXConfig::new("k")
            .with_value("v")
            .with_tag("t1")
            .with_tag("t2")
            .with_weight(5)
            .deactivate();
        assert_eq!(c.value, "v");
        assert_eq!(c.tag_count(), 2);
        assert!(c.has_tag("t1"));
        assert_eq!(c.weight, 5);
        assert!(!c.active);
    }

    #[test]
    fn ipc_x_config_display() {
        let c = IpcXConfig::new("k").with_value("v");
        assert_eq!(format!("{c}"), "k=v");
    }

    #[test]
    fn ipc_x_registry_insert_get() {
        let mut reg = IpcXRegistry::new();
        reg.insert(IpcXConfig::new("a").with_value("1")).unwrap();
        assert_eq!(reg.get("a").unwrap().value, "1");
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn ipc_x_registry_duplicate() {
        let mut reg = IpcXRegistry::new();
        reg.insert(IpcXConfig::new("a")).unwrap();
        assert!(reg.insert(IpcXConfig::new("a")).is_err());
    }

    #[test]
    fn ipc_x_registry_remove() {
        let mut reg = IpcXRegistry::new();
        reg.insert(IpcXConfig::new("a")).unwrap();
        reg.insert(IpcXConfig::new("b")).unwrap();
        reg.remove("a");
        assert!(!reg.contains("a"));
        assert!(reg.contains("b"));
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn ipc_x_registry_active_entries() {
        let mut reg = IpcXRegistry::new();
        reg.insert(IpcXConfig::new("a")).unwrap();
        reg.insert(IpcXConfig::new("b").deactivate()).unwrap();
        assert_eq!(reg.active_entries().len(), 1);
    }

    #[test]
    fn ipc_x_registry_by_weight() {
        let mut reg = IpcXRegistry::new();
        reg.insert(IpcXConfig::new("lo").with_weight(1)).unwrap();
        reg.insert(IpcXConfig::new("hi").with_weight(10)).unwrap();
        let sorted = reg.by_weight_desc();
        assert_eq!(sorted[0].key, "hi");
    }

    #[test]
    fn ipc_x_registry_tags() {
        let mut reg = IpcXRegistry::new();
        reg.insert(IpcXConfig::new("a").with_tag("x")).unwrap();
        reg.insert(IpcXConfig::new("b").with_tag("y")).unwrap();
        assert_eq!(reg.entries_with_tag("x").len(), 1);
    }

    #[test]
    fn ipc_x_registry_total_weight() {
        let mut reg = IpcXRegistry::new();
        reg.insert(IpcXConfig::new("a").with_weight(3)).unwrap();
        reg.insert(IpcXConfig::new("b").with_weight(7)).unwrap();
        assert_eq!(reg.total_weight(), 10);
    }

    #[test]
    fn ipc_x_registry_iterator() {
        let mut reg = IpcXRegistry::new();
        reg.insert(IpcXConfig::new("a")).unwrap();
        reg.insert(IpcXConfig::new("b")).unwrap();
        let keys: Vec<&str> = reg.iter().map(|e| e.key.as_str()).collect();
        assert_eq!(keys, vec!["a", "b"]);
    }

    #[test]
    fn ipc_x_cache_put_get() {
        let mut cache = IpcXCache::new(3);
        cache.put("a", "1");
        cache.put("b", "2");
        assert_eq!(cache.get("a"), Some("1"));
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn ipc_x_cache_eviction() {
        let mut cache = IpcXCache::new(2);
        cache.put("a", "1");
        cache.put("b", "2");
        cache.put("c", "3");
        assert!(!cache.contains("a"));
        assert!(cache.contains("b"));
        assert!(cache.contains("c"));
    }

    #[test]
    fn ipc_x_cache_lru_order() {
        let mut cache = IpcXCache::new(3);
        cache.put("a", "1");
        cache.put("b", "2");
        cache.put("c", "3");
        cache.get("a"); // promote a
        cache.put("d", "4"); // evicts b
        assert!(cache.contains("a"));
        assert!(!cache.contains("b"));
    }

    #[test]
    fn ipc_x_cache_most_least_recent() {
        let mut cache = IpcXCache::new(5);
        cache.put("x", "1");
        cache.put("y", "2");
        assert_eq!(cache.most_recent().unwrap().0, "y");
        assert_eq!(cache.least_recent().unwrap().0, "x");
    }

    #[test]
    fn ipc_x_formatter_entry() {
        let e = IpcXConfig::new("k").with_value("v");
        let fmt = IpcXFormatter::new();
        let output = fmt.format_entry(&e);
        assert!(output.contains("[✓]"));
        assert!(output.contains("k=v"));
    }

    #[test]
    fn ipc_x_formatter_summary() {
        let mut reg = IpcXRegistry::new();
        reg.insert(IpcXConfig::new("a").with_weight(5)).unwrap();
        let fmt = IpcXFormatter::new();
        let summary = fmt.format_summary(&reg);
        assert!(summary.contains("1 active"));
    }

    #[test]
    fn ipc_x_validator_valid() {
        let v = IpcXValidator::new();
        let c = IpcXConfig::new("ok");
        assert!(v.validate(&c).is_ok());
    }

    #[test]
    fn ipc_x_validator_empty_key() {
        let v = IpcXValidator::new();
        let c = IpcXConfig::new("");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn ipc_x_validator_require_value() {
        let v = IpcXValidator::new().require_value(true);
        let c = IpcXConfig::new("k");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn ipc_x_validator_allowed_tags() {
        let v = IpcXValidator::new()
            .allowed_tags(vec!["ok".into()]);
        let c = IpcXConfig::new("k").with_tag("bad");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn ipc_x_validator_validate_all() {
        let v = IpcXValidator::new();
        let mut reg = IpcXRegistry::new();
        reg.insert(IpcXConfig::new("ok")).unwrap();
        let errs = v.validate_all(&reg);
        assert!(errs.is_empty());
    }


    // -- ipc Z-extended tests -----------------------------------------------

    #[test]
    fn z_ipc_priority_weight() {
        assert_eq!(ZIpcPriority::Idle.weight(), 0);
        assert_eq!(ZIpcPriority::Normal.weight(), 2);
        assert_eq!(ZIpcPriority::Realtime.weight(), 4);
    }

    #[test]
    fn z_ipc_priority_label() {
        assert_eq!(ZIpcPriority::Low.label(), "low");
        assert_eq!(ZIpcPriority::High.label(), "high");
    }

    #[test]
    fn z_ipc_priority_is_elevated() {
        assert!(!ZIpcPriority::Normal.is_elevated());
        assert!(ZIpcPriority::High.is_elevated());
        assert!(ZIpcPriority::Realtime.is_elevated());
    }

    #[test]
    fn z_ipc_priority_display() {
        assert_eq!(format!("{}", ZIpcPriority::Idle), "idle");
    }

    #[test]
    fn z_ipc_priority_all_asc() {
        let all = ZIpcPriority::all_asc();
        assert_eq!(all.len(), 5);
        assert_eq!(all[0], ZIpcPriority::Idle);
        assert_eq!(all[4], ZIpcPriority::Realtime);
    }

    #[test]
    fn z_ipc_struct_new() {
        let s = ZIpcIpcFlowControl::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn z_ipc_struct_toggled_clone() {
        let s = ZIpcIpcFlowControl::new();
        let t = s.toggled_clone();
        let _ = t.credits;
    }

    #[test]
    fn z_ipc_rolling_hash_deterministic() {
        let h1 = z_ipc_rolling_hash(b"test");
        let h2 = z_ipc_rolling_hash(b"test");
        assert_eq!(h1, h2);
        assert_ne!(z_ipc_rolling_hash(b"a"), z_ipc_rolling_hash(b"b"));
    }

    #[test]
    fn z_ipc_pad_to_basic() {
        assert_eq!(z_ipc_pad_to("hi", 5), "hi   ");
        assert_eq!(z_ipc_pad_to("hello world", 5), "hello");
    }

    #[test]
    fn z_ipc_is_identifier_basic() {
        assert!(z_ipc_is_identifier("foo_bar"));
        assert!(z_ipc_is_identifier("abc123"));
        assert!(!z_ipc_is_identifier(""));
        assert!(!z_ipc_is_identifier("has space"));
    }

    #[test]
    fn z_ipc_levenshtein_basic() {
        assert_eq!(z_ipc_levenshtein("", ""), 0);
        assert_eq!(z_ipc_levenshtein("abc", "abc"), 0);
        assert_eq!(z_ipc_levenshtein("kitten", "sitting"), 3);
    }

    #[test]
    fn z_ipc_unique_words_basic() {
        let w = z_ipc_unique_words("the cat sat on the mat");
        assert_eq!(w.len(), 5);
        assert_eq!(w[0], "the");
    }

    #[test]
    fn z_ipc_chunk_slice_basic() {
        let data = vec![1, 2, 3, 4, 5];
        let chunks = z_ipc_chunk_slice(&data, 2);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], &[1, 2]);
        assert_eq!(chunks[2], &[5]);
    }

    #[test]
    fn z_ipc_common_prefix_basic() {
        assert_eq!(z_ipc_common_prefix("abcdef", "abcxyz"), "abc");
        assert_eq!(z_ipc_common_prefix("xyz", "abc"), "");
    }

    #[test]
    fn z_ipc_struct_clear() {
        let mut s = ZIpcIpcFlowControl::new();
        s.window_sizes.push(Default::default());
        assert_eq!(s.len(), 1);
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn z_ipc_rolling_hash_empty() {
        let h = z_ipc_rolling_hash(b"");
        assert_eq!(h, 0xcbf29ce484222325);
    }

    #[test]
    fn zq_store_new_empty() {
        let store = super::ZqStore::new(8);
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);
        assert_eq!(store.remaining(), 8);
    }

    #[test]
    fn zq_store_insert_and_get() {
        let mut store = super::ZqStore::new(8);
        assert!(store.insert("color", "red"));
        assert_eq!(store.get("color"), Some("red"));
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_eviction() {
        let mut store = super::ZqStore::new(2);
        store.insert("a", "1");
        store.insert("b", "2");
        store.insert("c", "3");
        assert_eq!(store.len(), 2);
        assert!(store.get("a").is_none());
        assert_eq!(store.get("b"), Some("2"));
        assert_eq!(store.get("c"), Some("3"));
    }

    #[test]
    fn zq_store_remove() {
        let mut store = super::ZqStore::new(8);
        store.insert("x", "10");
        store.insert("x", "20");
        store.insert("y", "30");
        let removed = store.remove("x");
        assert_eq!(removed, 2);
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_keys_values() {
        let mut store = super::ZqStore::new(8);
        store.insert("k1", "v1");
        store.insert("k2", "v2");
        assert_eq!(store.keys(), vec!["k1", "k2"]);
        assert_eq!(store.values(), vec!["v1", "v2"]);
    }

    #[test]
    fn zq_store_drain_prefix() {
        let mut store = super::ZqStore::new(8);
        store.insert("pre_a", "1");
        store.insert("pre_b", "2");
        store.insert("other", "3");
        let drained = store.drain_prefix("pre_");
        assert_eq!(drained.len(), 2);
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_retain() {
        let mut store = super::ZqStore::new(8);
        store.insert("a", "keep");
        store.insert("b", "drop");
        store.insert("c", "keep");
        store.retain(|_k, v| v == "keep");
        assert_eq!(store.len(), 2);
    }

    #[test]
    fn zq_store_clear() {
        let mut store = super::ZqStore::new(8);
        store.insert("a", "1");
        store.insert("b", "2");
        store.clear();
        assert!(store.is_empty());
        assert_eq!(store.remaining(), 8);
    }

    #[test]
    fn zq_store_merge() {
        let mut s1 = super::ZqStore::new(3);
        s1.insert("a", "1");
        let mut s2 = super::ZqStore::new(8);
        s2.insert("b", "2");
        s2.insert("c", "3");
        s2.insert("d", "4");
        s1.merge(&s2);
        assert_eq!(s1.len(), 3);
        assert!(s1.get("d").is_none());
    }

    #[test]
    fn zq_format_bytes_units() {
        assert_eq!(super::zq_format_bytes(500), "500 B");
        assert_eq!(super::zq_format_bytes(2048), "2.00 KB");
        assert_eq!(super::zq_format_bytes(5 * 1024 * 1024), "5.00 MB");
        assert_eq!(super::zq_format_bytes(3 * 1024 * 1024 * 1024), "3.00 GB");
    }

    #[test]
    fn zq_truncate_short() {
        assert_eq!(super::zq_truncate("hi", 10), "hi");
    }

    #[test]
    fn zq_truncate_long() {
        let long = "abcdefghijklmnop";
        let t = super::zq_truncate(long, 10);
        assert!(t.ends_with("..."));
        assert!(t.len() <= 10);
    }


    #[test]
    fn xb_ring_buffer_90_push_and_len() {
        let mut rb = super::XbRingBuffer90::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_90_overwrite() {
        let mut rb = super::XbRingBuffer90::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_90_get_out_of_bounds() {
        let rb = super::XbRingBuffer90::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_90_drain_all() {
        let mut rb = super::XbRingBuffer90::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_90_peek_front_back() {
        let mut rb = super::XbRingBuffer90::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_90_clear() {
        let mut rb = super::XbRingBuffer90::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_90_capacity() {
        let rb = super::XbRingBuffer90::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_90_basic() {
        let h = super::xb_fnv1a_90(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_90(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_90_different_inputs() {
        let h1 = super::xb_fnv1a_90(b"abc");
        let h2 = super::xb_fnv1a_90(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_90_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_90(&data);
        let dec = super::xb_rle_decode_90(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_90_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_90(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_90(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_90_values() {
        assert!((super::xb_clamp_90(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_90(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_90(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_90_values() {
        assert!((super::xb_lerp_90(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_90(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_90(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_90_wrap_around_twice() {
        let mut rb = super::XbRingBuffer90::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
    }


    // ---- xc_ pool / scheduler tests – block 99 ----

    #[test]
    fn xc_99_pool_new_empty() {
        let pool: super::Xc99Pool<i32> = super::Xc99Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_99_pool_release_acquire() {
        let mut pool = super::Xc99Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_99_pool_acquire_empty() {
        let mut pool: super::Xc99Pool<i32> = super::Xc99Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_99_pool_full() {
        let mut pool = super::Xc99Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_99_pool_drain() {
        let mut pool = super::Xc99Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_99_pool_stats() {
        let mut pool = super::Xc99Pool::new(8);
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
    fn xc_99_pool_clear() {
        let mut pool = super::Xc99Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_99_pool_shrink() {
        let mut pool = super::Xc99Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_99_pool_default() {
        let pool: super::Xc99Pool<String> = super::Xc99Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_99_pool_extend() {
        let mut pool = super::Xc99Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_99_pool_retain() {
        let mut pool = super::Xc99Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_99_scheduler_round_robin() {
        let mut sched = super::Xc99Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_99_scheduler_empty() {
        let mut sched = super::Xc99Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_99_scheduler_reset() {
        let mut sched = super::Xc99Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_99_scheduler_add_remove() {
        let mut sched = super::Xc99Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_99_scheduler_targets() {
        let sched = super::Xc99Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_99_hash_empty() {
        assert_eq!(super::xc_99_hash(b""), 5381);
    }

    #[test]
    fn xc_99_hash_data() {
        let h = super::xc_99_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_99_hash(b"hello"), h);
    }

    #[test]
    fn xc_99_reverse_str() {
        assert_eq!(super::xc_99_reverse("abc"), "cba");
        assert_eq!(super::xc_99_reverse(""), "");
    }


    #[test]
    fn xe_103_pipeline_empty() {
        let p = super::Xe103Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_103_pipeline_parse_stage() {
        let p = super::Xe103Pipeline::new()
            .add_parse(super::xe_103_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_103_pipeline_transform_double() {
        let p = super::Xe103Pipeline::new()
            .add_transform(super::xe_103_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_103_pipeline_validate_reverse() {
        let p = super::Xe103Pipeline::new()
            .add_validate(super::xe_103_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_103_pipeline_emit_filter() {
        let p = super::Xe103Pipeline::new()
            .add_emit(super::xe_103_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_103_pipeline_multi_stage() {
        let p = super::Xe103Pipeline::new()
            .add_parse(super::xe_103_pipeline_identity)
            .add_transform(super::xe_103_pipeline_double)
            .add_validate(super::xe_103_pipeline_reverse)
            .add_emit(super::xe_103_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_103_pipeline_error_propagation() {
        let p = super::Xe103Pipeline::new()
            .add_parse(super::xe_103_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe103Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_103_pipeline_compose() {
        let p1 = super::Xe103Pipeline::new()
            .add_parse(super::xe_103_pipeline_identity);
        let p2 = super::Xe103Pipeline::new()
            .add_transform(super::xe_103_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_103_pipeline_error_display() {
        let e = super::Xe103PipelineError {
            stage: super::Xe103Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_103_cache_put_get() {
        let mut c = super::Xe103Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_103_cache_miss() {
        let mut c: super::Xe103Cache<&str, i32> = super::Xe103Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_103_cache_ttl_expiry() {
        let mut c = super::Xe103Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_103_cache_evict() {
        let mut c = super::Xe103Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_103_cache_capacity() {
        let mut c = super::Xe103Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_103_cache_stats() {
        let mut c = super::Xe103Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_103_cache_clear() {
        let mut c = super::Xe103Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // -- xg_101 graph tests ------------------------------------------------

    #[test]
    fn xg_101_graph_empty() {
        let g = super::Xg101Graph::new();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn xg_101_graph_add_node() {
        let mut g = super::Xg101Graph::new();
        g.add_node(1);
        g.add_node(2);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_101_graph_add_edge() {
        let mut g = super::Xg101Graph::new();
        g.add_edge(0, 1);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_101_graph_neighbors() {
        let mut g = super::Xg101Graph::new();
        g.add_edge(0, 1);
        g.add_edge(0, 2);
        assert_eq!(g.neighbors(0).len(), 2);
    }

    #[test]
    fn xg_101_graph_has_path() {
        let mut g = super::Xg101Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(g.has_path(0, 2));
        assert!(!g.has_path(2, 0));
    }

    #[test]
    fn xg_101_graph_self_path() {
        let g = super::Xg101Graph::new();
        assert!(g.has_path(5, 5));
    }

    #[test]
    fn xg_101_graph_topo_sort() {
        let mut g = super::Xg101Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        let sorted = g.topological_sort().unwrap();
        let pos: std::collections::HashMap<usize, usize> =
            sorted.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&1] < pos[&2]);
    }

    #[test]
    fn xg_101_graph_cycle_detect_false() {
        let mut g = super::Xg101Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(!g.cycle_detect());
    }

    #[test]
    fn xg_101_graph_cycle_detect_true() {
        let mut g = super::Xg101Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        g.add_edge(2, 0);
        assert!(g.cycle_detect());
    }

    // -- xg_101 heap tests -------------------------------------------------

    #[test]
    fn xg_101_heap_empty() {
        let h: super::Xg101Heap<i32> = super::Xg101Heap::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn xg_101_heap_push_pop() {
        let mut h = super::Xg101Heap::new();
        h.push(3);
        h.push(1);
        h.push(2);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(3));
    }

    #[test]
    fn xg_101_heap_peek() {
        let mut h = super::Xg101Heap::new();
        h.push(5);
        h.push(2);
        assert_eq!(h.peek(), Some(&2));
    }

    #[test]
    fn xg_101_heap_drain_sorted() {
        let mut h = super::Xg101Heap::new();
        for v in [4, 1, 7, 2, 9] { h.push(v); }
        assert_eq!(h.drain_sorted(), vec![1, 2, 4, 7, 9]);
        assert!(h.is_empty());
    }

    #[test]
    fn xg_101_heap_merge() {
        let mut a = super::Xg101Heap::new();
        let mut b = super::Xg101Heap::new();
        a.push(5); a.push(3);
        b.push(4); b.push(1);
        a.merge(&mut b);
        assert_eq!(a.len(), 4);
        assert_eq!(a.pop(), Some(1));
    }

    #[test]
    fn xg_101_heap_default() {
        let h: super::Xg101Heap<u64> = Default::default();
        assert!(h.is_empty());
    }

    #[test]
    fn xg_101_graph_default() {
        let g: super::Xg101Graph = Default::default();
        assert_eq!(g.node_count(), 0);
    }


    #[test]
    fn xh98_skip_insert_contains() {
        let mut sl = super::Xh98SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh98_skip_remove() {
        let mut sl = super::Xh98SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh98_skip_len() {
        let mut sl = super::Xh98SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh98_skip_range_query() {
        let mut sl = super::Xh98SkipList::xh_new(4);
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
    fn xh98_skip_floor_ceiling() {
        let mut sl = super::Xh98SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh98_skip_rank() {
        let mut sl = super::Xh98SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh98_skip_empty() {
        let sl = super::Xh98SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh98_skip_duplicates() {
        let mut sl = super::Xh98SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh98_bitset_set_test() {
        let mut bs = super::Xh98BitSet::xh_new(256);
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
    fn xh98_bitset_clear_count() {
        let mut bs = super::Xh98BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh98_bitset_and_or_xor() {
        let mut a = super::Xh98BitSet::xh_new(128);
        let mut b = super::Xh98BitSet::xh_new(128);
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
    fn xh98_bitset_iter_ones() {
        let mut bs = super::Xh98BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh98_bitset_first_last() {
        let mut bs = super::Xh98BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh98_bitset_empty() {
        let bs = super::Xh98BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi98_deque_push_pop_back() {
        let mut dq = super::Xi98Deque::xi_new(4);
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
    fn xi98_deque_push_pop_front() {
        let mut dq = super::Xi98Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi98_deque_mixed_ops() {
        let mut dq = super::Xi98Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi98_deque_get_and_split() {
        let mut dq = super::Xi98Deque::xi_new(8);
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
    fn xi98_deque_rotate_left() {
        let mut dq = super::Xi98Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi98_deque_rotate_right() {
        let mut dq = super::Xi98Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi98_deque_grow() {
        let mut dq = super::Xi98Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi98_deque_empty() {
        let dq = super::Xi98Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi98_interval_tree_insert_query() {
        let mut tree = super::Xi98IntervalTree::xi_new();
        tree.xi_insert(super::Xi98Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi98Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi98Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi98_interval_tree_overlap() {
        let mut tree = super::Xi98IntervalTree::xi_new();
        tree.xi_insert(super::Xi98Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi98Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi98Interval::xi_new(12, 20));
        let q = super::Xi98Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi98_interval_tree_remove() {
        let mut tree = super::Xi98IntervalTree::xi_new();
        tree.xi_insert(super::Xi98Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi98Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi98_interval_tree_gaps() {
        let mut tree = super::Xi98IntervalTree::xi_new();
        tree.xi_insert(super::Xi98Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi98Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi98Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi98Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi98Interval::xi_new(8, 10));
    }

    #[test]
    fn xi98_interval_tree_merge() {
        let mut tree = super::Xi98IntervalTree::xi_new();
        tree.xi_insert(super::Xi98Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi98Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi98Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi98Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi98Interval::xi_new(10, 15));
    }

    #[test]
    fn xi98_interval_tree_all() {
        let mut tree = super::Xi98IntervalTree::xi_new();
        tree.xi_insert(super::Xi98Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi98Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi98_interval_tree_empty() {
        let tree = super::Xi98IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi98_interval_tree_contains_point() {
        let iv = super::Xi98Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 98) ---

    #[test]
    fn xj_98_uf_make_and_find() {
        let mut uf = super::Xj98UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_98_uf_union_connected() {
        let mut uf = super::Xj98UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_98_uf_component_count() {
        let mut uf = super::Xj98UnionFind::xj_new();
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
    fn xj_98_uf_component_size() {
        let mut uf = super::Xj98UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_98_uf_largest_component() {
        let mut uf = super::Xj98UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_98_uf_many_elements() {
        let mut uf = super::Xj98UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_98_uf_separate_components() {
        let mut uf = super::Xj98UnionFind::xj_new();
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
    fn xj_98_uf_path_compression() {
        let mut uf = super::Xj98UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_98_bt_insert_get() {
        let mut bt = super::Xj98BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_98_bt_contains_len() {
        let mut bt = super::Xj98BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_98_bt_replace() {
        let mut bt = super::Xj98BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_98_bt_remove() {
        let mut bt = super::Xj98BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_98_bt_keys_values() {
        let mut bt = super::Xj98BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_98_bt_range() {
        let mut bt = super::Xj98BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_98_bt_min_max() {
        let mut bt = super::Xj98BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_98_bt_many_inserts() {
        let mut bt = super::Xj98BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_98 segment tree tests ---

    #[test]
    fn xk_98_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk98SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_98_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk98SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_98_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk98SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_98_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk98SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_98_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk98SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_98_st_single_element() {
        let data = vec![42];
        let st = super::Xk98SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_98_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk98SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_98_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk98SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_98 disjoint intervals tests ---

    #[test]
    fn xk_98_di_add_and_count() {
        let mut di = super::Xk98DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_98_di_merge_overlap() {
        let mut di = super::Xk98DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_98_di_contains() {
        let mut di = super::Xk98DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_98_di_remove() {
        let mut di = super::Xk98DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_98_di_covered_length() {
        let mut di = super::Xk98DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_98_di_gaps() {
        let mut di = super::Xk98DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_98_di_merge_adjacent() {
        let mut di = super::Xk98DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_98_di_empty() {
        let di = super::Xk98DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_98_rope_new_empty() {
        let rope = super::Xl98Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_98_rope_from_str() {
        let rope = super::Xl98Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_98_rope_insert_at() {
        let mut rope = super::Xl98Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_98_rope_delete_range() {
        let mut rope = super::Xl98Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_98_rope_char_at() {
        let rope = super::Xl98Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_98_rope_split_concat() {
        let rope = super::Xl98Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_98_rope_line_count() {
        let rope = super::Xl98Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_98_rope_line_at() {
        let rope = super::Xl98Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_98_sa_build_and_search() {
        let sa = super::Xl98SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_98_sa_count() {
        let sa = super::Xl98SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_98_sa_longest_repeated() {
        let sa = super::Xl98SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_98_sa_all_positions() {
        let sa = super::Xl98SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_98_sa_len() {
        let sa = super::Xl98SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_98_sa_empty() {
        let sa = super::Xl98SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_98_rope_slice() {
        let rope = super::Xl98Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_98_sa_search_start() {
        let sa = super::Xl98SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }
}
