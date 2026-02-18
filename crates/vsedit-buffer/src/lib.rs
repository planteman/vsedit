//! Cross-platform buffer abstraction.
//!
//! Wraps `bytes::Bytes` to provide a VS Code `VSBuffer`-like interface.
//! Equivalent to VS Code's `vs/base/common/buffer.ts`.

use std::collections::HashMap;
use std::fmt;

pub use bytes::{Buf, BufMut, Bytes, BytesMut};

/// A reference-counted, immutable byte buffer.
///
/// Wraps `bytes::Bytes` with additional convenience methods matching
/// VS Code's `VSBuffer` interface.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VsBuffer {
    inner: Bytes,
}

impl VsBuffer {
    /// Create a new buffer from bytes.
    pub fn new(data: impl Into<Bytes>) -> Self {
        Self {
            inner: data.into(),
        }
    }

    /// Create an empty buffer.
    pub fn empty() -> Self {
        Self {
            inner: Bytes::new(),
        }
    }

    /// Create a buffer from a UTF-8 string.
    pub fn from_string(s: &str) -> Self {
        Self {
            inner: Bytes::copy_from_slice(s.as_bytes()),
        }
    }

    /// Try to convert the buffer to a UTF-8 string.
    pub fn to_string_lossy(&self) -> String {
        String::from_utf8_lossy(&self.inner).into_owned()
    }

    /// Get the raw bytes.
    pub fn as_bytes(&self) -> &[u8] {
        &self.inner
    }

    /// Get the length in bytes.
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Check if the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Get a slice of this buffer.
    pub fn slice(&self, range: std::ops::Range<usize>) -> Self {
        Self {
            inner: self.inner.slice(range),
        }
    }

    /// Concatenate multiple buffers.
    pub fn concat(buffers: &[VsBuffer]) -> Self {
        let total_len: usize = buffers.iter().map(|b| b.len()).sum();
        let mut result = BytesMut::with_capacity(total_len);
        for buf in buffers {
            result.extend_from_slice(buf.as_bytes());
        }
        Self {
            inner: result.freeze(),
        }
    }

    /// Consume and return the inner `Bytes`.
    pub fn into_bytes(self) -> Bytes {
        self.inner
    }
}

impl From<Vec<u8>> for VsBuffer {
    fn from(v: Vec<u8>) -> Self {
        Self::new(v)
    }
}

impl From<&[u8]> for VsBuffer {
    fn from(s: &[u8]) -> Self {
        Self::new(Bytes::copy_from_slice(s))
    }
}

impl From<&str> for VsBuffer {
    fn from(s: &str) -> Self {
        Self::from_string(s)
    }
}

impl AsRef<[u8]> for VsBuffer {
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors that can occur when working with a [`VsBuffer`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BufferError {
    /// The requested byte range is outside the buffer boundaries.
    SliceOutOfBounds {
        /// The length of the buffer.
        buf_len: usize,
        /// Start of the requested range.
        start: usize,
        /// End of the requested range.
        end: usize,
    },
    /// The buffer does not contain valid UTF-8.
    InvalidUtf8,
    /// The operation requires a non-empty buffer.
    EmptyBuffer,
}

impl fmt::Display for BufferError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BufferError::SliceOutOfBounds { buf_len, start, end } => {
                write!(
                    f,
                    "slice {}..{} out of bounds for buffer of length {}",
                    start, end, buf_len
                )
            }
            BufferError::InvalidUtf8 => write!(f, "buffer contains invalid UTF-8"),
            BufferError::EmptyBuffer => write!(f, "buffer is empty"),
        }
    }
}

impl std::error::Error for BufferError {}

// ---------------------------------------------------------------------------
// Additional VsBuffer methods
// ---------------------------------------------------------------------------

impl VsBuffer {
    /// Try to interpret the buffer as a UTF-8 string.
    ///
    /// Returns [`BufferError::InvalidUtf8`] when the bytes are not valid UTF-8
    /// and [`BufferError::EmptyBuffer`] when the buffer is empty.
    pub fn try_to_string(&self) -> Result<String, BufferError> {
        if self.is_empty() {
            return Err(BufferError::EmptyBuffer);
        }
        std::str::from_utf8(&self.inner)
            .map(|s| s.to_owned())
            .map_err(|_| BufferError::InvalidUtf8)
    }

    /// Return a sub-buffer for the given range, with bounds checking.
    pub fn try_slice(&self, range: std::ops::Range<usize>) -> Result<Self, BufferError> {
        if range.start > range.end || range.end > self.inner.len() {
            return Err(BufferError::SliceOutOfBounds {
                buf_len: self.inner.len(),
                start: range.start,
                end: range.end,
            });
        }
        Ok(Self {
            inner: self.inner.slice(range),
        })
    }

    /// Split the buffer into two at the given byte index.
    ///
    /// Returns `(left, right)` where `left` contains bytes `[0, mid)` and
    /// `right` contains bytes `[mid, len)`.
    pub fn split_at(&self, mid: usize) -> Result<(Self, Self), BufferError> {
        if mid > self.inner.len() {
            return Err(BufferError::SliceOutOfBounds {
                buf_len: self.inner.len(),
                start: 0,
                end: mid,
            });
        }
        let left = self.inner.slice(..mid);
        let right = self.inner.slice(mid..);
        Ok((Self { inner: left }, Self { inner: right }))
    }

    /// Returns `true` if the buffer starts with the given byte pattern.
    pub fn starts_with(&self, needle: &[u8]) -> bool {
        self.inner.starts_with(needle)
    }

    /// Returns `true` if the buffer ends with the given byte pattern.
    pub fn ends_with(&self, needle: &[u8]) -> bool {
        self.inner.ends_with(needle)
    }

    /// Find the first occurrence of `needle` in the buffer.
    ///
    /// Returns the byte offset of the match, or `None` if not found.
    pub fn find(&self, needle: &[u8]) -> Option<usize> {
        if needle.is_empty() {
            return Some(0);
        }
        self.inner
            .windows(needle.len())
            .position(|w| w == needle)
    }
}

// ---------------------------------------------------------------------------
// Display
// ---------------------------------------------------------------------------

impl fmt::Display for VsBuffer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match std::str::from_utf8(&self.inner) {
            Ok(s) => f.write_str(s),
            Err(_) => write!(f, "[binary {} bytes]", self.inner.len()),
        }
    }
}

// ---------------------------------------------------------------------------
// BufferBuilder
// ---------------------------------------------------------------------------

/// Incrementally build a [`VsBuffer`] by appending data.
#[derive(Debug, Clone)]
pub struct BufferBuilder {
    inner: BytesMut,
}

impl BufferBuilder {
    /// Create a new empty builder.
    pub fn new() -> Self {
        Self {
            inner: BytesMut::new(),
        }
    }

    /// Create a builder with the given byte capacity pre-allocated.
    pub fn with_capacity(cap: usize) -> Self {
        Self {
            inner: BytesMut::with_capacity(cap),
        }
    }

    /// Append raw bytes to the builder.
    pub fn append(&mut self, data: &[u8]) -> &mut Self {
        self.inner.extend_from_slice(data);
        self
    }

    /// Append a UTF-8 string to the builder.
    pub fn append_str(&mut self, s: &str) -> &mut Self {
        self.inner.extend_from_slice(s.as_bytes());
        self
    }

    /// Consume the builder and produce an immutable [`VsBuffer`].
    pub fn build(self) -> VsBuffer {
        VsBuffer {
            inner: self.inner.freeze(),
        }
    }

    /// Return the current length in bytes.
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Return `true` if the builder contains no bytes.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Clear all buffered data without releasing the allocation.
    pub fn clear(&mut self) {
        self.inner.clear();
    }
}

impl Default for BufferBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Additional VsBuffer convenience methods
// ---------------------------------------------------------------------------

impl VsBuffer {
    /// Count all non-overlapping occurrences of `needle` in the buffer.
    pub fn count_occurrences(&self, needle: &[u8]) -> usize {
        if needle.is_empty() {
            return 0;
        }
        let mut count = 0;
        let mut start = 0;
        while start + needle.len() <= self.inner.len() {
            if &self.inner[start..start + needle.len()] == needle {
                count += 1;
                start += needle.len();
            } else {
                start += 1;
            }
        }
        count
    }

    /// Replace all non-overlapping occurrences of `needle` with `replacement`.
    pub fn replace(&self, needle: &[u8], replacement: &[u8]) -> Self {
        if needle.is_empty() {
            return self.clone();
        }
        let mut result = BytesMut::new();
        let mut start = 0;
        while start < self.inner.len() {
            if start + needle.len() <= self.inner.len()
                && &self.inner[start..start + needle.len()] == needle
            {
                result.extend_from_slice(replacement);
                start += needle.len();
            } else {
                result.extend_from_slice(&self.inner[start..start + 1]);
                start += 1;
            }
        }
        Self {
            inner: result.freeze(),
        }
    }

    /// Return a lowercase hex string representation of the buffer bytes.
    pub fn to_hex_string(&self) -> String {
        self.inner
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect()
    }

    /// Get a single byte at the given index, or `None` if out of bounds.
    pub fn byte_at(&self, index: usize) -> Option<u8> {
        self.inner.get(index).copied()
    }

    /// Split the buffer by newline characters (`\n`) into sub-buffers.
    ///
    /// A trailing newline does **not** produce an extra empty element.
    pub fn lines(&self) -> Vec<VsBuffer> {
        if self.inner.is_empty() {
            return Vec::new();
        }
        let mut result = Vec::new();
        let mut start = 0;
        for (i, &b) in self.inner.iter().enumerate() {
            if b == b'\n' {
                result.push(Self {
                    inner: self.inner.slice(start..i),
                });
                start = i + 1;
            }
        }
        if start < self.inner.len() {
            result.push(Self {
                inner: self.inner.slice(start..),
            });
        }
        result
    }

    /// Trim leading and trailing ASCII whitespace bytes.
    pub fn trim_ascii(&self) -> Self {
        let bytes = self.as_bytes();
        let start = bytes.iter().position(|b| !b.is_ascii_whitespace());
        let end = bytes.iter().rposition(|b| !b.is_ascii_whitespace());
        match (start, end) {
            (Some(s), Some(e)) => Self {
                inner: self.inner.slice(s..=e),
            },
            _ => Self::empty(),
        }
    }
}

// ---------------------------------------------------------------------------
// PartialOrd
// ---------------------------------------------------------------------------

impl PartialOrd for VsBuffer {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.inner.as_ref().cmp(other.inner.as_ref()))
    }
}

/// Accumulated statistics for buffer operations.
#[derive(Debug, Clone, PartialEq)]
pub struct BufferStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl BufferStats {
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
    pub fn merge(&mut self, other: &BufferStats) {
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

impl Default for BufferStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for BufferStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "BufferStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for buffer.
#[derive(Debug, Clone)]
pub struct BufferValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl BufferValidator {
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

impl Default for BufferValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// BufferPool
// ---------------------------------------------------------------------------

/// A pool of reusable byte buffers to reduce allocation overhead.
pub struct BufferPool {
    pool: Vec<BytesMut>,
    default_capacity: usize,
    max_pool_size: usize,
    total_acquired: u64,
    total_released: u64,
}

impl BufferPool {
    /// Create a new buffer pool.
    pub fn new(default_capacity: usize, max_pool_size: usize) -> Self {
        Self {
            pool: Vec::new(),
            default_capacity,
            max_pool_size,
            total_acquired: 0,
            total_released: 0,
        }
    }

    /// Acquire a buffer from the pool, or allocate a new one.
    pub fn acquire(&mut self) -> BytesMut {
        self.total_acquired += 1;
        self.pool
            .pop()
            .unwrap_or_else(|| BytesMut::with_capacity(self.default_capacity))
    }

    /// Release a buffer back to the pool. The buffer is cleared before storing.
    pub fn release(&mut self, mut buf: BytesMut) {
        self.total_released += 1;
        if self.pool.len() < self.max_pool_size {
            buf.clear();
            self.pool.push(buf);
        }
    }

    /// Number of buffers currently available in the pool.
    pub fn available(&self) -> usize {
        self.pool.len()
    }

    /// Total number of buffers acquired from this pool.
    pub fn total_acquired(&self) -> u64 {
        self.total_acquired
    }

    /// Total number of buffers released back to this pool.
    pub fn total_released(&self) -> u64 {
        self.total_released
    }

    /// Pool hit rate (released / acquired), or 1.0 if no acquisitions.
    pub fn hit_rate(&self) -> f64 {
        if self.total_acquired == 0 {
            return 1.0;
        }
        self.total_released as f64 / self.total_acquired as f64
    }

    /// Clear all buffers from the pool.
    pub fn clear(&mut self) {
        self.pool.clear();
    }

    /// Whether the pool contains no buffers.
    pub fn is_empty(&self) -> bool {
        self.pool.is_empty()
    }

    /// Shrink the pool to at most `target` buffers, dropping the excess.
    pub fn shrink_pool(&mut self, target: usize) {
        self.pool.truncate(target);
    }
}

// ---------------------------------------------------------------------------
// BufferReader
// ---------------------------------------------------------------------------

/// A reader that tracks position while reading from a VsBuffer.
pub struct BufferReader {
    buffer: VsBuffer,
    position: usize,
    mark: Option<usize>,
}

impl BufferReader {
    /// Create a new reader over the given buffer.
    pub fn new(buffer: VsBuffer) -> Self {
        Self {
            buffer,
            position: 0,
            mark: None,
        }
    }

    /// Current read position.
    pub fn position(&self) -> usize {
        self.position
    }

    /// Number of bytes remaining.
    pub fn remaining(&self) -> usize {
        self.buffer.len().saturating_sub(self.position)
    }

    /// Whether the reader has reached the end of the buffer.
    pub fn is_eof(&self) -> bool {
        self.position >= self.buffer.len()
    }

    /// Read a single byte, advancing the position.
    pub fn read_byte(&mut self) -> Option<u8> {
        if self.is_eof() {
            return None;
        }
        let b = self.buffer.as_bytes()[self.position];
        self.position += 1;
        Some(b)
    }

    /// Read up to `n` bytes, returning what is available.
    pub fn read_bytes(&mut self, n: usize) -> VsBuffer {
        let start = self.position;
        let end = (start + n).min(self.buffer.len());
        self.position = end;
        if start >= self.buffer.len() {
            return VsBuffer::empty();
        }
        self.buffer.slice(start..end)
    }

    /// Peek at the next byte without advancing the position.
    pub fn peek_byte(&self) -> Option<u8> {
        if self.is_eof() {
            return None;
        }
        Some(self.buffer.as_bytes()[self.position])
    }

    /// Skip up to `n` bytes, returning the actual number skipped.
    pub fn skip(&mut self, n: usize) -> usize {
        let skipped = n.min(self.remaining());
        self.position += skipped;
        skipped
    }

    /// Save the current position as a mark.
    pub fn set_mark(&mut self) {
        self.mark = Some(self.position);
    }

    /// Reset the position to the previously saved mark.
    /// Returns `false` if no mark was set.
    pub fn reset_to_mark(&mut self) -> bool {
        match self.mark {
            Some(m) => {
                self.position = m;
                true
            }
            None => false,
        }
    }

    /// Read bytes up to and including the given delimiter.
    /// If the delimiter is not found, reads to the end.
    pub fn read_until(&mut self, delimiter: u8) -> VsBuffer {
        let start = self.position;
        let data = self.buffer.as_bytes();
        while self.position < data.len() {
            self.position += 1;
            if data[self.position - 1] == delimiter {
                break;
            }
        }
        if start >= self.buffer.len() {
            return VsBuffer::empty();
        }
        self.buffer.slice(start..self.position)
    }

    /// Read a line (up to and including `\n`), or to the end of buffer.
    /// Returns `None` if already at EOF.
    pub fn read_line(&mut self) -> Option<VsBuffer> {
        if self.is_eof() {
            return None;
        }
        Some(self.read_until(b'\n'))
    }

    /// Seek to an absolute position. Returns `false` if the position is
    /// beyond the buffer length.
    pub fn seek(&mut self, pos: usize) -> bool {
        if pos > self.buffer.len() {
            return false;
        }
        self.position = pos;
        true
    }
}

// ---------------------------------------------------------------------------
// buffer_concat helpers
// ---------------------------------------------------------------------------

/// Efficiently concatenate multiple VsBuffers with an optional separator.
pub fn buffer_concat(buffers: &[VsBuffer], separator: Option<&[u8]>) -> VsBuffer {
    if buffers.is_empty() {
        return VsBuffer::empty();
    }
    let sep = separator.unwrap_or(&[]);
    let total: usize = buffers.iter().map(|b| b.len()).sum::<usize>()
        + sep.len() * buffers.len().saturating_sub(1);
    let mut out = BytesMut::with_capacity(total);
    for (i, buf) in buffers.iter().enumerate() {
        if i > 0 && !sep.is_empty() {
            out.extend_from_slice(sep);
        }
        out.extend_from_slice(buf.as_bytes());
    }
    VsBuffer::new(out.freeze())
}

/// Concatenate multiple VsBuffers with a string separator.
pub fn buffer_concat_with_str(buffers: &[VsBuffer], separator: &str) -> VsBuffer {
    buffer_concat(buffers, Some(separator.as_bytes()))
}

/// Join buffer lines with newlines.
pub fn buffer_join_lines(lines: &[VsBuffer]) -> VsBuffer {
    buffer_concat(lines, Some(b"\n"))
}

// ---------------------------------------------------------------------------
// ChunkIterator
// ---------------------------------------------------------------------------

/// Iterator that yields fixed-size chunks of a [`VsBuffer`].
///
/// The last chunk may be shorter than `chunk_size` if the buffer length is not
/// evenly divisible.
pub struct ChunkIterator {
    buffer: VsBuffer,
    chunk_size: usize,
    offset: usize,
}

impl ChunkIterator {
    fn new(buffer: VsBuffer, chunk_size: usize) -> Self {
        assert!(chunk_size > 0, "chunk_size must be > 0");
        Self {
            buffer,
            chunk_size,
            offset: 0,
        }
    }
}

impl Iterator for ChunkIterator {
    type Item = VsBuffer;

    fn next(&mut self) -> Option<Self::Item> {
        if self.offset >= self.buffer.len() {
            return None;
        }
        let end = (self.offset + self.chunk_size).min(self.buffer.len());
        let chunk = self.buffer.slice(self.offset..end);
        self.offset = end;
        Some(chunk)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.buffer.len().saturating_sub(self.offset);
        let n = (remaining + self.chunk_size - 1) / self.chunk_size;
        (n, Some(n))
    }
}

impl ExactSizeIterator for ChunkIterator {}

// ---------------------------------------------------------------------------
// WindowIterator
// ---------------------------------------------------------------------------

/// Iterator that yields overlapping windows of a [`VsBuffer`].
pub struct WindowIterator {
    buffer: VsBuffer,
    window_size: usize,
    offset: usize,
}

impl WindowIterator {
    fn new(buffer: VsBuffer, window_size: usize) -> Self {
        assert!(window_size > 0, "window_size must be > 0");
        Self {
            buffer,
            window_size,
            offset: 0,
        }
    }
}

impl Iterator for WindowIterator {
    type Item = VsBuffer;

    fn next(&mut self) -> Option<Self::Item> {
        if self.offset + self.window_size > self.buffer.len() {
            return None;
        }
        let window = self.buffer.slice(self.offset..self.offset + self.window_size);
        self.offset += 1;
        Some(window)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self
            .buffer
            .len()
            .saturating_sub(self.offset)
            .saturating_sub(self.window_size - 1);
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for WindowIterator {}

// ---------------------------------------------------------------------------
// Additional VsBuffer methods: chunks, windows, xor, pad, find_byte,
// contains_pattern, reverse, repeat
// ---------------------------------------------------------------------------

impl VsBuffer {
    /// Return an iterator over fixed-size chunks of the buffer.
    pub fn chunks(&self, chunk_size: usize) -> ChunkIterator {
        ChunkIterator::new(self.clone(), chunk_size)
    }

    /// Return an iterator over overlapping windows of the buffer.
    pub fn windows(&self, window_size: usize) -> WindowIterator {
        WindowIterator::new(self.clone(), window_size)
    }

    /// Find the index of the first occurrence of a single byte.
    pub fn find_byte(&self, needle: u8) -> Option<usize> {
        self.inner.iter().position(|&b| b == needle)
    }

    /// Returns `true` if the buffer contains the given byte pattern.
    pub fn contains_pattern(&self, pattern: &[u8]) -> bool {
        self.find(pattern).is_some()
    }

    /// XOR every byte in this buffer with the corresponding byte in `other`.
    ///
    /// The result has the length of the shorter buffer.
    pub fn xor(&self, other: &VsBuffer) -> Self {
        let len = self.inner.len().min(other.inner.len());
        let mut out = BytesMut::with_capacity(len);
        for i in 0..len {
            out.extend_from_slice(&[self.inner[i] ^ other.inner[i]]);
        }
        Self {
            inner: out.freeze(),
        }
    }

    /// Pad the buffer to `target_len` by appending `pad_byte`.
    ///
    /// If the buffer is already at least `target_len`, returns a clone.
    pub fn pad(&self, target_len: usize, pad_byte: u8) -> Self {
        if self.inner.len() >= target_len {
            return self.clone();
        }
        let mut out = BytesMut::with_capacity(target_len);
        out.extend_from_slice(&self.inner);
        out.resize(target_len, pad_byte);
        Self {
            inner: out.freeze(),
        }
    }

    /// Return a new buffer with bytes in reverse order.
    pub fn reverse(&self) -> Self {
        let mut v: Vec<u8> = self.inner.to_vec();
        v.reverse();
        Self::new(v)
    }

    /// Repeat the buffer `n` times.
    pub fn repeat(&self, n: usize) -> Self {
        if n == 0 || self.is_empty() {
            return Self::empty();
        }
        let mut out = BytesMut::with_capacity(self.inner.len() * n);
        for _ in 0..n {
            out.extend_from_slice(&self.inner);
        }
        Self {
            inner: out.freeze(),
        }
    }

    /// Replace all occurrences of a single byte with another byte (in-place
    /// copy).
    pub fn replace_byte(&self, from: u8, to: u8) -> Self {
        let v: Vec<u8> = self.inner.iter().map(|&b| if b == from { to } else { b }).collect();
        Self::new(v)
    }
}

// ---------------------------------------------------------------------------
// BufferDiff – compare two buffers
// ---------------------------------------------------------------------------

/// Result of comparing two buffers byte-by-byte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BufferDiffResult {
    /// Whether the two buffers are byte-identical.
    pub identical: bool,
    /// Byte offset of the first difference, if any.
    pub first_diff_offset: Option<usize>,
    /// Length of the longest common prefix.
    pub common_prefix_len: usize,
    /// Length of the longest common suffix (non-overlapping with the prefix).
    pub common_suffix_len: usize,
}

/// Utilities for comparing two [`VsBuffer`] instances.
pub struct BufferDiff;

impl BufferDiff {
    /// Compare two buffers and return a detailed [`BufferDiffResult`].
    pub fn compare(a: &VsBuffer, b: &VsBuffer) -> BufferDiffResult {
        let a_bytes = a.as_bytes();
        let b_bytes = b.as_bytes();

        let min_len = a_bytes.len().min(b_bytes.len());

        // Common prefix length.
        let mut prefix_len = 0;
        for i in 0..min_len {
            if a_bytes[i] == b_bytes[i] {
                prefix_len += 1;
            } else {
                break;
            }
        }

        let identical = a_bytes.len() == b_bytes.len() && prefix_len == a_bytes.len();

        let first_diff_offset = if identical {
            None
        } else if prefix_len < min_len {
            Some(prefix_len)
        } else {
            // One buffer is a prefix of the other.
            Some(min_len)
        };

        // Common suffix length (must not overlap with the prefix region).
        let mut suffix_len = 0;
        let remaining = min_len - prefix_len;
        for i in 0..remaining {
            let ai = a_bytes.len() - 1 - i;
            let bi = b_bytes.len() - 1 - i;
            if a_bytes[ai] == b_bytes[bi] {
                suffix_len += 1;
            } else {
                break;
            }
        }

        BufferDiffResult {
            identical,
            first_diff_offset,
            common_prefix_len: prefix_len,
            common_suffix_len: suffix_len,
        }
    }

    /// Convenience check for byte-equality of two buffers.
    pub fn are_equal(a: &VsBuffer, b: &VsBuffer) -> bool {
        a.as_bytes() == b.as_bytes()
    }
}

// ---------------------------------------------------------------------------
// BufferCompression – simple RLE encoding / decoding
// ---------------------------------------------------------------------------

/// Simple run-length encoding helpers for byte slices.
pub struct BufferCompression;

impl BufferCompression {
    /// Encode `data` using run-length encoding.
    ///
    /// Each run of identical bytes is encoded as `(count, byte)` where count
    /// is stored as a single `u8` (maximum run length 255). Longer runs are
    /// split into multiple pairs.
    pub fn rle_encode(data: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        if data.is_empty() {
            return out;
        }

        let mut i = 0;
        while i < data.len() {
            let byte = data[i];
            let mut count: u8 = 1;
            while i + (count as usize) < data.len()
                && data[i + (count as usize)] == byte
                && count < 255
            {
                count += 1;
            }
            out.push(count);
            out.push(byte);
            i += count as usize;
        }
        out
    }

    /// Decode run-length encoded data produced by [`rle_encode`](Self::rle_encode).
    ///
    /// The input must contain an even number of bytes (count/byte pairs).
    pub fn rle_decode(encoded: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut i = 0;
        while i + 1 < encoded.len() {
            let count = encoded[i] as usize;
            let byte = encoded[i + 1];
            out.extend(std::iter::repeat(byte).take(count));
            i += 2;
        }
        out
    }

    /// Compute the compression ratio `compressed_len / original_len`.
    ///
    /// Returns `f64::INFINITY` when `original` is empty and `compressed` is
    /// not, or `1.0` when both are empty.
    pub fn compression_ratio(original: &[u8], compressed: &[u8]) -> f64 {
        if original.is_empty() {
            if compressed.is_empty() {
                return 1.0;
            }
            return f64::INFINITY;
        }
        compressed.len() as f64 / original.len() as f64
    }
}

// ---------------------------------------------------------------------------
// CircularBuffer – fixed-capacity ring buffer
// ---------------------------------------------------------------------------

/// A fixed-capacity circular (ring) buffer of bytes.
///
/// When the buffer is full, the oldest byte is silently overwritten by
/// [`push`](Self::push).
#[derive(Debug, Clone)]
pub struct CircularBuffer {
    buf: Vec<u8>,
    capacity: usize,
    head: usize,
    len: usize,
}

impl CircularBuffer {
    /// Create a new circular buffer with the given capacity.
    ///
    /// # Panics
    ///
    /// Panics if `capacity` is zero.
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "CircularBuffer capacity must be > 0");
        Self {
            buf: vec![0u8; capacity],
            capacity,
            head: 0,
            len: 0,
        }
    }

    /// Push a byte into the buffer, overwriting the oldest byte if full.
    pub fn push(&mut self, byte: u8) {
        let write_pos = (self.head + self.len) % self.capacity;
        if self.len == self.capacity {
            // Overwrite oldest – advance head.
            self.buf[write_pos] = byte;
            self.head = (self.head + 1) % self.capacity;
        } else {
            self.buf[write_pos] = byte;
            self.len += 1;
        }
    }

    /// Number of bytes currently stored.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether the buffer has reached its capacity.
    pub fn is_full(&self) -> bool {
        self.len == self.capacity
    }

    /// Whether the buffer contains no bytes.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Get the byte at logical `index` (0 = oldest).
    pub fn get(&self, index: usize) -> Option<u8> {
        if index >= self.len {
            return None;
        }
        Some(self.buf[(self.head + index) % self.capacity])
    }

    /// Return all stored bytes in order (oldest first).
    pub fn to_vec(&self) -> Vec<u8> {
        (0..self.len)
            .map(|i| self.buf[(self.head + i) % self.capacity])
            .collect()
    }

    /// Remove all stored bytes without changing the capacity.
    pub fn clear(&mut self) {
        self.head = 0;
        self.len = 0;
    }
}

// ---------------------------------------------------------------------------
// BufferSearch – pattern search inside a VsBuffer
// ---------------------------------------------------------------------------

/// Byte-pattern search utilities for [`VsBuffer`].
pub struct BufferSearch;

impl BufferSearch {
    /// Return the offset of the first occurrence of `needle` in `buf`, or
    /// `None` if not found.
    pub fn find_first(buf: &VsBuffer, needle: &[u8]) -> Option<usize> {
        if needle.is_empty() {
            return Some(0);
        }
        let haystack = buf.as_bytes();
        if needle.len() > haystack.len() {
            return None;
        }
        haystack
            .windows(needle.len())
            .position(|w| w == needle)
    }

    /// Return the starting offsets of every (non-overlapping) occurrence of
    /// `needle` in `buf`.
    pub fn find_all(buf: &VsBuffer, needle: &[u8]) -> Vec<usize> {
        if needle.is_empty() {
            return vec![];
        }
        let haystack = buf.as_bytes();
        if needle.len() > haystack.len() {
            return vec![];
        }
        let mut positions = Vec::new();
        let mut start = 0;
        while start + needle.len() <= haystack.len() {
            if &haystack[start..start + needle.len()] == needle {
                positions.push(start);
                start += needle.len(); // non-overlapping
            } else {
                start += 1;
            }
        }
        positions
    }

    /// Count the number of non-overlapping occurrences of `needle` in `buf`.
    pub fn count_occurrences(buf: &VsBuffer, needle: &[u8]) -> usize {
        Self::find_all(buf, needle).len()
    }
}


// ---------------------------------------------------------------------------
// BufferPoolStats
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct BufferPoolStats {
    entries: Vec<String>,
    index: usize,
    enabled: bool,
    config: HashMap<String, String>,
    stats_hits: u64,
    stats_misses: u64,
}

impl BufferPoolStats {
    pub fn new() -> Self { Self::default() }
    pub fn add_entry(&mut self, entry: impl Into<String>) { self.entries.push(entry.into()); }
    pub fn remove_entry(&mut self, idx: usize) -> Option<String> { if idx < self.entries.len() { Some(self.entries.remove(idx)) } else { None } }
    pub fn get_entry(&self, idx: usize) -> Option<&str> { self.entries.get(idx).map(|s| s.as_str()) }
    pub fn entry_count(&self) -> usize { self.entries.len() }
    pub fn set_enabled(&mut self, e: bool) { self.enabled = e; }
    pub fn is_enabled(&self) -> bool { self.enabled }
    pub fn set_config(&mut self, k: impl Into<String>, v: impl Into<String>) { self.config.insert(k.into(), v.into()); }
    pub fn get_config(&self, k: &str) -> Option<&str> { self.config.get(k).map(|s| s.as_str()) }
    pub fn config_count(&self) -> usize { self.config.len() }
    pub fn record_hit(&mut self) { self.stats_hits += 1; }
    pub fn record_miss(&mut self) { self.stats_misses += 1; }
    pub fn hit_rate(&self) -> f64 { let t = self.stats_hits + self.stats_misses; if t == 0 { 0.0 } else { self.stats_hits as f64 / t as f64 } }
    pub fn reset_stats(&mut self) { self.stats_hits = 0; self.stats_misses = 0; }
    pub fn select_next(&mut self) { if !self.entries.is_empty() { self.index = (self.index + 1) % self.entries.len(); } }
    pub fn select_prev(&mut self) { if !self.entries.is_empty() { self.index = if self.index == 0 { self.entries.len() - 1 } else { self.index - 1 }; } }
    pub fn current_index(&self) -> usize { self.index }
    pub fn current_entry(&self) -> Option<&str> { self.entries.get(self.index).map(|s| s.as_str()) }
    pub fn clear(&mut self) { self.entries.clear(); self.index = 0; }
    pub fn contains(&self, s: &str) -> bool { self.entries.iter().any(|e| e == s) }
    pub fn entries(&self) -> &[String] { &self.entries }
    pub fn filter_entries(&self, query: &str) -> Vec<&str> { self.entries.iter().filter(|e| e.contains(query)).map(|s| s.as_str()).collect() }
}

impl Default for BufferPoolStats {
    fn default() -> Self { Self { entries: Vec::new(), index: 0, enabled: true, config: HashMap::new(), stats_hits: 0, stats_misses: 0 } }
}

impl fmt::Display for BufferPoolStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "BufferPoolStats({} entries, enabled={})", self.entries.len(), self.enabled) }
}

// ---------------------------------------------------------------------------
// BufferCompareUtil
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct BufferCompareUtil {
    items: HashMap<String, Vec<String>>,
    active: Option<String>,
    max_items: usize,
    total_ops: u64,
    last_error: Option<String>,
}

impl BufferCompareUtil {
    pub fn new() -> Self { Self::default() }
    pub fn with_max(mut self, m: usize) -> Self { self.max_items = m; self }
    pub fn add_item(&mut self, group: impl Into<String>, value: impl Into<String>) {
        let g = group.into();
        let entry = self.items.entry(g).or_default();
        if entry.len() < self.max_items { entry.push(value.into()); }
        self.total_ops += 1;
    }
    pub fn remove_group(&mut self, group: &str) -> bool { self.items.remove(group).is_some() }
    pub fn get_group(&self, group: &str) -> Option<&Vec<String>> { self.items.get(group) }
    pub fn group_count(&self) -> usize { self.items.len() }
    pub fn total_items(&self) -> usize { self.items.values().map(|v| v.len()).sum() }
    pub fn set_active(&mut self, a: impl Into<String>) { self.active = Some(a.into()); }
    pub fn active(&self) -> Option<&str> { self.active.as_deref() }
    pub fn clear_active(&mut self) { self.active = None; }
    pub fn set_error(&mut self, e: impl Into<String>) { self.last_error = Some(e.into()); }
    pub fn last_error(&self) -> Option<&str> { self.last_error.as_deref() }
    pub fn clear_error(&mut self) { self.last_error = None; }
    pub fn total_ops(&self) -> u64 { self.total_ops }
    pub fn clear(&mut self) { self.items.clear(); self.active = None; self.total_ops = 0; self.last_error = None; }
    pub fn groups(&self) -> Vec<&str> { self.items.keys().map(|k| k.as_str()).collect() }
    pub fn contains_group(&self, g: &str) -> bool { self.items.contains_key(g) }
    pub fn is_empty(&self) -> bool { self.items.is_empty() }
}

impl Default for BufferCompareUtil {
    fn default() -> Self { Self { items: HashMap::new(), active: None, max_items: 1000, total_ops: 0, last_error: None } }
}

impl fmt::Display for BufferCompareUtil {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "BufferCompareUtil({} groups, {} items)", self.group_count(), self.total_items()) }
}


// ---------------------------------------------------------------------------
// BufferPoolStatsSnapshot — point-in-time snapshot of BufferPoolStats state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct BufferPoolStatsSnapshot {
    pub timestamp: u64,
    pub entry_count: usize,
    pub enabled: bool,
    pub config_snapshot: Vec<(String, String)>,
    pub hit_rate: f64,
}

impl BufferPoolStatsSnapshot {
    pub fn capture(source: &BufferPoolStats, timestamp: u64) -> Self {
        Self {
            timestamp,
            entry_count: source.entry_count(),
            enabled: source.is_enabled(),
            config_snapshot: Vec::new(),
            hit_rate: source.hit_rate(),
        }
    }

    pub fn age_since(&self, now: u64) -> u64 {
        now.saturating_sub(self.timestamp)
    }

    pub fn is_stale(&self, now: u64, max_age: u64) -> bool {
        self.age_since(now) > max_age
    }

    pub fn diff_entry_count(&self, other: &Self) -> i64 {
        self.entry_count as i64 - other.entry_count as i64
    }
}

impl fmt::Display for BufferPoolStatsSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Snapshot(t={}, entries={}, enabled={})", self.timestamp, self.entry_count, self.enabled)
    }
}

// ---------------------------------------------------------------------------
// BufferCompareUtilStats — aggregate statistics for BufferCompareUtil
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct BufferCompareUtilStats {
    pub total_adds: u64,
    pub total_removes: u64,
    pub total_lookups: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub peak_group_count: usize,
    pub peak_item_count: usize,
}

impl BufferCompareUtilStats {
    pub fn new() -> Self { Self::default() }

    pub fn record_add(&mut self) { self.total_adds += 1; }
    pub fn record_remove(&mut self) { self.total_removes += 1; }
    pub fn record_lookup(&mut self, hit: bool) {
        self.total_lookups += 1;
        if hit { self.cache_hits += 1; } else { self.cache_misses += 1; }
    }

    pub fn update_peaks(&mut self, groups: usize, items: usize) {
        if groups > self.peak_group_count { self.peak_group_count = groups; }
        if items > self.peak_item_count { self.peak_item_count = items; }
    }

    pub fn hit_ratio(&self) -> f64 {
        if self.total_lookups == 0 { 0.0 } else { self.cache_hits as f64 / self.total_lookups as f64 }
    }

    pub fn net_changes(&self) -> i64 {
        self.total_adds as i64 - self.total_removes as i64
    }

    pub fn reset(&mut self) { *self = Self::default(); }

    pub fn merge(&mut self, other: &Self) {
        self.total_adds += other.total_adds;
        self.total_removes += other.total_removes;
        self.total_lookups += other.total_lookups;
        self.cache_hits += other.cache_hits;
        self.cache_misses += other.cache_misses;
        if other.peak_group_count > self.peak_group_count { self.peak_group_count = other.peak_group_count; }
        if other.peak_item_count > self.peak_item_count { self.peak_item_count = other.peak_item_count; }
    }
}

impl fmt::Display for BufferCompareUtilStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Stats(adds={}, removes={}, hit_ratio={:.1}%)", self.total_adds, self.total_removes, self.hit_ratio() * 100.0)
    }
}

// ---------------------------------------------------------------------------
// BufferPoolStatsConfig — configuration for BufferPoolStats
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct BufferPoolStatsConfig {
    pub max_entries: usize,
    pub auto_cleanup: bool,
    pub cleanup_threshold: usize,
    pub debounce_ms: u64,
    pub labels: HashMap<String, String>,
}

impl BufferPoolStatsConfig {
    pub fn new() -> Self { Self::default() }
    pub fn with_max_entries(mut self, m: usize) -> Self { self.max_entries = m; self }
    pub fn with_auto_cleanup(mut self, a: bool) -> Self { self.auto_cleanup = a; self }
    pub fn with_debounce(mut self, ms: u64) -> Self { self.debounce_ms = ms; self }
    pub fn set_label(&mut self, key: impl Into<String>, val: impl Into<String>) { self.labels.insert(key.into(), val.into()); }
    pub fn get_label(&self, key: &str) -> Option<&str> { self.labels.get(key).map(|s| s.as_str()) }
    pub fn label_count(&self) -> usize { self.labels.len() }
    pub fn needs_cleanup(&self, current: usize) -> bool { self.auto_cleanup && current > self.cleanup_threshold }
}

impl Default for BufferPoolStatsConfig {
    fn default() -> Self {
        Self { max_entries: 10000, auto_cleanup: true, cleanup_threshold: 8000, debounce_ms: 100, labels: HashMap::new() }
    }
}

impl fmt::Display for BufferPoolStatsConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Config(max={}, auto_cleanup={}, debounce={}ms)", self.max_entries, self.auto_cleanup, self.debounce_ms)
    }
}

// ---------------------------------------------------------------------------
// BufferChunker — split buffers into chunks
// ---------------------------------------------------------------------------

/// Split a [`VsBuffer`] into fixed-size or line-delimited chunks.
pub struct BufferChunker;

impl BufferChunker {
    /// Split `buf` into chunks of exactly `chunk_size` bytes
    /// (the last chunk may be smaller).
    pub fn fixed_size(buf: &VsBuffer, chunk_size: usize) -> Vec<VsBuffer> {
        if chunk_size == 0 || buf.is_empty() {
            return vec![];
        }
        let data = buf.as_bytes();
        data.chunks(chunk_size)
            .map(|c| VsBuffer::new(Bytes::copy_from_slice(c)))
            .collect()
    }

    /// Split `buf` on newline boundaries (`\n`). Each chunk retains the
    /// trailing newline if present. Empty input yields an empty vec.
    pub fn by_lines(buf: &VsBuffer) -> Vec<VsBuffer> {
        if buf.is_empty() {
            return vec![];
        }
        let data = buf.as_bytes();
        let mut chunks = Vec::new();
        let mut start = 0;
        for (i, &b) in data.iter().enumerate() {
            if b == b'\n' {
                chunks.push(VsBuffer::new(Bytes::copy_from_slice(&data[start..=i])));
                start = i + 1;
            }
        }
        if start < data.len() {
            chunks.push(VsBuffer::new(Bytes::copy_from_slice(&data[start..])));
        }
        chunks
    }

    /// Number of fixed-size chunks that `len` bytes would produce.
    pub fn chunk_count(len: usize, chunk_size: usize) -> usize {
        if chunk_size == 0 { return 0; }
        (len + chunk_size - 1) / chunk_size
    }
}

// ---------------------------------------------------------------------------
// BufferEncoder — simple base-64-like encode/decode
// ---------------------------------------------------------------------------

/// Simple byte encoder using a 6-bit encoding (custom Base64-like alphabet).
pub struct BufferEncoder;

const B64_ALPHA: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

impl BufferEncoder {
    /// Encode `buf` into a base64-like string.
    pub fn encode(buf: &VsBuffer) -> String {
        let data = buf.as_bytes();
        let mut out = String::with_capacity((data.len() + 2) / 3 * 4);
        for chunk in data.chunks(3) {
            let b0 = chunk[0] as u32;
            let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
            let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
            let triple = (b0 << 16) | (b1 << 8) | b2;
            out.push(B64_ALPHA[((triple >> 18) & 0x3F) as usize] as char);
            out.push(B64_ALPHA[((triple >> 12) & 0x3F) as usize] as char);
            if chunk.len() > 1 {
                out.push(B64_ALPHA[((triple >> 6) & 0x3F) as usize] as char);
            } else {
                out.push('=');
            }
            if chunk.len() > 2 {
                out.push(B64_ALPHA[(triple & 0x3F) as usize] as char);
            } else {
                out.push('=');
            }
        }
        out
    }

    fn decode_char(c: u8) -> Option<u8> {
        match c {
            b'A'..=b'Z' => Some(c - b'A'),
            b'a'..=b'z' => Some(c - b'a' + 26),
            b'0'..=b'9' => Some(c - b'0' + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    }

    /// Decode a base64-like string back into a buffer.
    pub fn decode(encoded: &str) -> Result<VsBuffer, String> {
        let bytes = encoded.as_bytes();
        if bytes.len() % 4 != 0 {
            return Err("encoded length must be multiple of 4".into());
        }
        let mut out = Vec::with_capacity(bytes.len() / 4 * 3);
        for chunk in bytes.chunks(4) {
            let vals: Vec<Option<u8>> = chunk.iter().map(|&c| {
                if c == b'=' { Some(0) } else { Self::decode_char(c) }
            }).collect();
            if vals.iter().any(|v| v.is_none()) {
                return Err("invalid character in encoded data".into());
            }
            let v: Vec<u8> = vals.into_iter().map(|v| v.unwrap()).collect();
            let triple = (v[0] as u32) << 18 | (v[1] as u32) << 12 | (v[2] as u32) << 6 | v[3] as u32;
            out.push((triple >> 16) as u8);
            if chunk[2] != b'=' { out.push((triple >> 8) as u8); }
            if chunk[3] != b'=' { out.push(triple as u8); }
        }
        Ok(VsBuffer::new(Bytes::from(out)))
    }
}

// ---------------------------------------------------------------------------
// BufferSearch additional helpers
// ---------------------------------------------------------------------------

impl BufferSearch {
    /// Returns `true` if `buf` contains the byte pattern.
    pub fn contains_pattern(buf: &VsBuffer, needle: &[u8]) -> bool {
        Self::find_first(buf, needle).is_some()
    }
}

// ---------------------------------------------------------------------------
// BufferDiff additional helpers
// ---------------------------------------------------------------------------

impl BufferDiff {
    /// Returns a list of (offset, old_byte, new_byte) tuples for byte-level changes.
    pub fn byte_changes(a: &VsBuffer, b: &VsBuffer) -> Vec<(usize, u8, u8)> {
        let ab = a.as_bytes();
        let bb = b.as_bytes();
        let max_len = ab.len().max(bb.len());
        let mut changes = Vec::new();
        for i in 0..max_len {
            let va = if i < ab.len() { ab[i] } else { 0 };
            let vb = if i < bb.len() { bb[i] } else { 0 };
            if va != vb {
                changes.push((i, va, vb));
            }
        }
        changes
    }
}


/// Text buffer configuration manager.
#[derive(Debug, Clone)]
pub struct BufferConfig {
    entries: Vec<BufferEntry>,
    enabled: bool,
    max_entries: usize,
}

/// A single text buffer entry.
#[derive(Debug, Clone, PartialEq)]
pub struct BufferEntry {
    pub id: String,
    pub label: String,
    pub priority: i32,
    pub active: bool,
    pub metadata: Vec<(String, String)>,
}

impl BufferEntry {
    pub fn new(id: &str, label: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            priority: 0,
            active: true,
            metadata: Vec::new(),
        }
    }

    pub fn with_priority(mut self, p: i32) -> Self {
        self.priority = p;
        self
    }

    pub fn with_meta(mut self, key: &str, val: &str) -> Self {
        self.metadata.push((key.to_string(), val.to_string()));
        self
    }

    pub fn get_meta(&self, key: &str) -> Option<&str> {
        self.metadata.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }

    pub fn deactivate(&mut self) {
        self.active = false;
    }

    pub fn activate(&mut self) {
        self.active = true;
    }

    pub fn has_meta(&self, key: &str) -> bool {
        self.metadata.iter().any(|(k, _)| k == key)
    }

    pub fn meta_count(&self) -> usize {
        self.metadata.len()
    }

    pub fn remove_meta(&mut self, key: &str) -> bool {
        let len = self.metadata.len();
        self.metadata.retain(|(k, _)| k != key);
        self.metadata.len() < len
    }
}

impl BufferConfig {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            enabled: true,
            max_entries,
        }
    }

    pub fn add(&mut self, entry: BufferEntry) -> bool {
        if self.entries.len() >= self.max_entries {
            return false;
        }
        self.entries.push(entry);
        self.entries.sort_by(|a, b| b.priority.cmp(&a.priority));
        true
    }

    pub fn remove(&mut self, id: &str) -> bool {
        let len = self.entries.len();
        self.entries.retain(|e| e.id != id);
        self.entries.len() < len
    }

    pub fn get(&self, id: &str) -> Option<&BufferEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut BufferEntry> {
        self.entries.iter_mut().find(|e| e.id == id)
    }

    pub fn active_entries(&self) -> Vec<&BufferEntry> {
        self.entries.iter().filter(|e| e.active).collect()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn is_full(&self) -> bool {
        self.entries.len() >= self.max_entries
    }

    pub fn enable(&mut self) {
        self.enabled = true;
    }

    pub fn disable(&mut self) {
        self.enabled = false;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn ids(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.id.as_str()).collect()
    }

    pub fn top_n(&self, n: usize) -> Vec<&BufferEntry> {
        self.entries.iter().take(n).collect()
    }

    pub fn find_by_label(&self, label: &str) -> Option<&BufferEntry> {
        self.entries.iter().find(|e| e.label == label)
    }

    pub fn deactivate_all(&mut self) {
        for e in &mut self.entries {
            e.active = false;
        }
    }

    pub fn activate_all(&mut self) {
        for e in &mut self.entries {
            e.active = true;
        }
    }

    pub fn count_active(&self) -> usize {
        self.entries.iter().filter(|e| e.active).count()
    }

    pub fn highest_priority(&self) -> Option<i32> {
        self.entries.first().map(|e| e.priority)
    }

    pub fn contains(&self, id: &str) -> bool {
        self.entries.iter().any(|e| e.id == id)
    }

    pub fn labels(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.label.as_str()).collect()
    }

    pub fn reorder_by_label(&mut self) {
        self.entries.sort_by(|a, b| a.label.cmp(&b.label));
    }

    pub fn drain_inactive(&mut self) -> Vec<BufferEntry> {
        let (inactive, active): (Vec<_>, Vec<_>) =
            self.entries.drain(..).partition(|e| !e.active);
        self.entries = active;
        inactive
    }
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
// xa_ extended helpers for buffer
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaBufferRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaBufferRingBuf {
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
pub struct XaBufferCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaBufferCounter {
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

impl Default for XaBufferCounter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 9
// ---------------------------------------------------------------------------

/// Generic object pool `Xc9Pool<T>`.
pub struct Xc9Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc9Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc9PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc9Pool<T> {
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
    pub fn stats(&self) -> Xc9PoolStats {
        Xc9PoolStats {
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

impl<T> Default for Xc9Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc9Scheduler`.
pub struct Xc9Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc9Scheduler {
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

impl Default for Xc9Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_9 hash for the given byte slice.
pub fn xc_9_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_9 convention.
pub fn xc_9_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_63 deepening: state machine + event bus ---

/// States for the Xd63 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd63State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd63State {
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
pub struct Xd63Transition {
    pub from: Xd63State,
    pub to: Xd63State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd63StateMachine {
    current: Xd63State,
    history: Vec<Xd63Transition>,
    step_counter: usize,
}

impl Xd63StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd63State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd63State {
        self.current
    }

    pub fn history(&self) -> &[Xd63Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd63State) -> Result<Xd63State, String> {
        let allowed = match (self.current, target) {
            (Xd63State::Idle, Xd63State::Running) => true,
            (Xd63State::Running, Xd63State::Paused) => true,
            (Xd63State::Running, Xd63State::Done) => true,
            (Xd63State::Paused, Xd63State::Running) => true,
            (Xd63State::Paused, Xd63State::Done) => true,
            (Xd63State::Done, Xd63State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_63: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd63Transition {
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
            "Xd63SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd63State> {
        let prefix = "Xd63SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd63State::Idle),
            "Running" => Some(Xd63State::Running),
            "Paused" => Some(Xd63State::Paused),
            "Done" => Some(Xd63State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd63State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd63 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd63Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd63Event {
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

type Xd63HandlerFn = Box<dyn Fn(&Xd63Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd63EventBus {
    handlers: Vec<(usize, Option<String>, Xd63HandlerFn)>,
    next_id: usize,
    published: Vec<Xd63Event>,
}

impl Xd63EventBus {
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
        F: Fn(&Xd63Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd63Event) + Send + Sync + 'static,
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
    pub fn publish(&mut self, event: Xd63Event) {
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

    pub fn published_events(&self) -> &[Xd63Event] {
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
// xf_ data structures (Trie + BloomFilter) — unique instance #62
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf62Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf62TrieNode {
    children: std::collections::HashMap<char, Xf62TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf62Trie {
    root: Xf62TrieNode,
    count: usize,
}

impl Xf62Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf62TrieNode::default(), count: 0 }
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

    fn xf_remove_recursive(node: &mut Xf62TrieNode, word: &str, depth: usize) -> bool {
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

    fn xf_collect(node: &Xf62TrieNode, buf: &mut String, out: &mut Vec<String>) {
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
pub struct Xf62BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf62BloomFilter {
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


/// A probabilistic sorted list using a skip-list structure (variant 8).
pub struct Xh8SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh8SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 50 as u64,
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

/// A compact bit set supporting boolean operations (variant 8).
pub struct Xh8BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh8BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 8).
pub struct Xi8Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi8Deque<T> {
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
pub struct Xi8Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi8Interval {
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

/// A simple interval tree (variant 8).
pub struct Xi8IntervalTree {
    xi_intervals: Vec<Xi8Interval>,
}

impl Xi8IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi8Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi8Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi8Interval) -> Vec<&Xi8Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi8Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi8Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi8Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi8Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi8Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi8Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 8) ---

/// Disjoint set / union-find for crate 8.
pub struct Xj8UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj8UnionFind {
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

const XJ8_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 8.
pub struct Xj8BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj8BTreeNode<K, V>>>,
    len: usize,
}

struct Xj8BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj8BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj8BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ8_BTREE_ORDER - 1
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
        let mid = XJ8_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj8BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj8BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj8BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj8BTreeNode::xj_new_leaf();
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


// --- xk_8 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk8SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk8SegmentTree {
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
pub struct Xk8DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk8DisjointIntervals {
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

    #[test]
    fn buffer_from_string() {
        let buf = VsBuffer::from_string("hello");
        assert_eq!(buf.len(), 5);
        assert_eq!(buf.to_string_lossy(), "hello");
    }

    #[test]
    fn buffer_concat() {
        let a = VsBuffer::from_string("hello ");
        let b = VsBuffer::from_string("world");
        let c = VsBuffer::concat(&[a, b]);
        assert_eq!(c.to_string_lossy(), "hello world");
    }

    #[test]
    fn buffer_slice() {
        let buf = VsBuffer::from_string("hello world");
        let slice = buf.slice(0..5);
        assert_eq!(slice.to_string_lossy(), "hello");
    }

    #[test]
    fn buffer_empty() {
        let buf = VsBuffer::empty();
        assert!(buf.is_empty());
        assert_eq!(buf.len(), 0);
    }

    #[test]
    fn try_to_string_valid_utf8() {
        let buf = VsBuffer::from_string("café");
        assert_eq!(buf.try_to_string().unwrap(), "café");
    }

    #[test]
    fn try_to_string_invalid_utf8() {
        let buf = VsBuffer::new(Bytes::from_static(&[0xFF, 0xFE]));
        assert_eq!(buf.try_to_string(), Err(BufferError::InvalidUtf8));
    }

    #[test]
    fn try_to_string_empty() {
        let buf = VsBuffer::empty();
        assert_eq!(buf.try_to_string(), Err(BufferError::EmptyBuffer));
    }

    #[test]
    fn try_slice_success() {
        let buf = VsBuffer::from_string("hello world");
        let s = buf.try_slice(6..11).unwrap();
        assert_eq!(s.to_string_lossy(), "world");
    }

    #[test]
    fn try_slice_out_of_bounds() {
        let buf = VsBuffer::from_string("hi");
        assert!(buf.try_slice(0..10).is_err());
    }

    #[test]
    fn split_at_middle() {
        let buf = VsBuffer::from_string("abcdef");
        let (left, right) = buf.split_at(3).unwrap();
        assert_eq!(left.to_string_lossy(), "abc");
        assert_eq!(right.to_string_lossy(), "def");
    }

    #[test]
    fn split_at_out_of_bounds() {
        let buf = VsBuffer::from_string("ab");
        assert!(buf.split_at(5).is_err());
    }

    #[test]
    fn starts_with_and_ends_with() {
        let buf = VsBuffer::from_string("hello world");
        assert!(buf.starts_with(b"hello"));
        assert!(!buf.starts_with(b"world"));
        assert!(buf.ends_with(b"world"));
        assert!(!buf.ends_with(b"hello"));
    }

    #[test]
    fn find_pattern() {
        let buf = VsBuffer::from_string("the quick brown fox");
        assert_eq!(buf.find(b"quick"), Some(4));
        assert_eq!(buf.find(b"slow"), None);
        assert_eq!(buf.find(b""), Some(0));
    }

    #[test]
    fn display_utf8() {
        let buf = VsBuffer::from_string("hello");
        assert_eq!(format!("{buf}"), "hello");
    }

    #[test]
    fn display_binary() {
        let buf = VsBuffer::new(Bytes::from_static(&[0xFF, 0xFE, 0xFD]));
        assert_eq!(format!("{buf}"), "[binary 3 bytes]");
    }

    #[test]
    fn buffer_error_display() {
        let err = BufferError::SliceOutOfBounds {
            buf_len: 5,
            start: 0,
            end: 10,
        };
        assert_eq!(
            err.to_string(),
            "slice 0..10 out of bounds for buffer of length 5"
        );
        assert_eq!(BufferError::InvalidUtf8.to_string(), "buffer contains invalid UTF-8");
        assert_eq!(BufferError::EmptyBuffer.to_string(), "buffer is empty");
    }

    // --- BufferBuilder tests ---

    #[test]
    fn test_buffer_builder_empty() {
        let builder = BufferBuilder::new();
        assert!(builder.is_empty());
        assert_eq!(builder.len(), 0);
        let buf = builder.build();
        assert!(buf.is_empty());
    }

    #[test]
    fn test_buffer_builder_append() {
        let mut builder = BufferBuilder::new();
        builder.append(b"hello").append(b" world");
        assert_eq!(builder.len(), 11);
        let buf = builder.build();
        assert_eq!(buf.to_string_lossy(), "hello world");
    }

    #[test]
    fn test_buffer_builder_append_str() {
        let mut builder = BufferBuilder::new();
        builder.append_str("foo").append_str("bar");
        let buf = builder.build();
        assert_eq!(buf.to_string_lossy(), "foobar");
    }

    #[test]
    fn test_buffer_builder_with_capacity() {
        let builder = BufferBuilder::with_capacity(1024);
        assert!(builder.is_empty());
        assert_eq!(builder.len(), 0);
    }

    #[test]
    fn test_buffer_builder_clear() {
        let mut builder = BufferBuilder::new();
        builder.append(b"data");
        assert_eq!(builder.len(), 4);
        builder.clear();
        assert!(builder.is_empty());
    }

    // --- VsBuffer new method tests ---

    #[test]
    fn test_count_occurrences() {
        let buf = VsBuffer::from_string("abcabcabc");
        assert_eq!(buf.count_occurrences(b"abc"), 3);
        assert_eq!(buf.count_occurrences(b"ab"), 3);
    }

    #[test]
    fn test_count_occurrences_none() {
        let buf = VsBuffer::from_string("hello world");
        assert_eq!(buf.count_occurrences(b"xyz"), 0);
        assert_eq!(buf.count_occurrences(b""), 0);
    }

    #[test]
    fn test_replace_all() {
        let buf = VsBuffer::from_string("aabbcc");
        let replaced = buf.replace(b"bb", b"XX");
        assert_eq!(replaced.to_string_lossy(), "aaXXcc");

        let buf2 = VsBuffer::from_string("aaaa");
        let replaced2 = buf2.replace(b"aa", b"b");
        assert_eq!(replaced2.to_string_lossy(), "bb");
    }

    #[test]
    fn test_to_hex_string() {
        let buf = VsBuffer::new(Bytes::from_static(&[0x00, 0xff, 0xab]));
        assert_eq!(buf.to_hex_string(), "00ffab");

        let buf2 = VsBuffer::from_string("AB");
        assert_eq!(buf2.to_hex_string(), "4142");
    }

    #[test]
    fn test_byte_at_valid() {
        let buf = VsBuffer::from_string("abc");
        assert_eq!(buf.byte_at(0), Some(b'a'));
        assert_eq!(buf.byte_at(1), Some(b'b'));
        assert_eq!(buf.byte_at(2), Some(b'c'));
    }

    #[test]
    fn test_byte_at_out_of_bounds() {
        let buf = VsBuffer::from_string("ab");
        assert_eq!(buf.byte_at(2), None);
        assert_eq!(buf.byte_at(100), None);
    }

    #[test]
    fn test_lines_split() {
        let buf = VsBuffer::from_string("line1\nline2\nline3");
        let lines = buf.lines();
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0].to_string_lossy(), "line1");
        assert_eq!(lines[1].to_string_lossy(), "line2");
        assert_eq!(lines[2].to_string_lossy(), "line3");

        let buf2 = VsBuffer::from_string("trailing\n");
        let lines2 = buf2.lines();
        assert_eq!(lines2.len(), 1);
        assert_eq!(lines2[0].to_string_lossy(), "trailing");
    }

    #[test]
    fn test_trim_ascii() {
        let buf = VsBuffer::from_string("  hello  ");
        let trimmed = buf.trim_ascii();
        assert_eq!(trimmed.to_string_lossy(), "hello");

        let buf2 = VsBuffer::from_string("\t\n data \r\n");
        let trimmed2 = buf2.trim_ascii();
        assert_eq!(trimmed2.to_string_lossy(), "data");

        let buf3 = VsBuffer::from_string("   ");
        let trimmed3 = buf3.trim_ascii();
        assert!(trimmed3.is_empty());
    }

    #[test]
    fn test_partial_ord() {
        let a = VsBuffer::from_string("abc");
        let b = VsBuffer::from_string("abd");
        let c = VsBuffer::from_string("abc");
        assert!(a < b);
        assert!(b > a);
        assert!(a <= c);
        assert!(a >= c);
    }

    #[test]
    fn buffer_stats_new_defaults() {
        let stats = BufferStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn buffer_stats_record_success() {
        let mut stats = BufferStats::new();
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
    fn buffer_stats_record_failure() {
        let mut stats = BufferStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn buffer_stats_reset() {
        let mut stats = BufferStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn buffer_stats_merge() {
        let mut a = BufferStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = BufferStats::new();
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
    fn buffer_stats_display() {
        let mut stats = BufferStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn buffer_stats_default() {
        let stats = BufferStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn buffer_validator_accepts_valid_name() {
        let v = BufferValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn buffer_validator_rejects_empty() {
        let v = BufferValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn buffer_validator_rejects_too_long() {
        let v = BufferValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn buffer_validator_forbidden_prefix() {
        let v = BufferValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn buffer_validator_allowed_chars() {
        let v = BufferValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn buffer_validator_range() {
        let v = BufferValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn buffer_sanitize_removes_control() {
        let result = BufferValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn buffer_truncate_short_string() {
        assert_eq!(BufferValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn buffer_truncate_long_string() {
        let result = BufferValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn buffer_is_ascii_printable() {
        assert!(BufferValidator::is_ascii_printable("Hello World 123"));
        assert!(!BufferValidator::is_ascii_printable("Hello\x00World"));
    }

    // -----------------------------------------------------------------------
    // BufferPool tests
    // -----------------------------------------------------------------------

    #[test]
    fn pool_acquire_creates_new_buffer() {
        let mut pool = BufferPool::new(64, 4);
        let buf = pool.acquire();
        assert!(buf.is_empty());
        assert_eq!(pool.total_acquired(), 1);
    }

    #[test]
    fn pool_release_and_reuse() {
        let mut pool = BufferPool::new(64, 4);
        let mut buf = pool.acquire();
        buf.extend_from_slice(b"hello");
        pool.release(buf);
        assert_eq!(pool.available(), 1);
        let reused = pool.acquire();
        assert!(reused.is_empty()); // cleared on release
        assert_eq!(pool.available(), 0);
    }

    #[test]
    fn pool_respects_max_size() {
        let mut pool = BufferPool::new(16, 2);
        for _ in 0..5 {
            let buf = pool.acquire();
            pool.release(buf);
        }
        // acquire then release 5 times; pool capped at 2
        assert!(pool.available() <= 2);
    }

    #[test]
    fn pool_hit_rate_no_acquisitions() {
        let pool = BufferPool::new(16, 4);
        assert!((pool.hit_rate() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn pool_hit_rate_with_usage() {
        let mut pool = BufferPool::new(16, 4);
        let b1 = pool.acquire();
        let _b2 = pool.acquire();
        pool.release(b1);
        assert!((pool.hit_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn pool_clear() {
        let mut pool = BufferPool::new(16, 4);
        let buf = pool.acquire();
        pool.release(buf);
        assert!(!pool.is_empty());
        pool.clear();
        assert!(pool.is_empty());
    }

    #[test]
    fn pool_shrink() {
        let mut pool = BufferPool::new(16, 10);
        // Fill the pool with 8 buffers
        let bufs: Vec<_> = (0..8).map(|_| pool.acquire()).collect();
        for buf in bufs {
            pool.release(buf);
        }
        assert_eq!(pool.available(), 8);
        pool.shrink_pool(3);
        assert_eq!(pool.available(), 3);
    }

    #[test]
    fn pool_is_empty_initially() {
        let pool = BufferPool::new(16, 4);
        assert!(pool.is_empty());
        assert_eq!(pool.available(), 0);
        assert_eq!(pool.total_acquired(), 0);
        assert_eq!(pool.total_released(), 0);
    }

    // -----------------------------------------------------------------------
    // BufferReader tests
    // -----------------------------------------------------------------------

    #[test]
    fn reader_position_and_remaining() {
        let buf = VsBuffer::from_string("abcdef");
        let reader = BufferReader::new(buf);
        assert_eq!(reader.position(), 0);
        assert_eq!(reader.remaining(), 6);
        assert!(!reader.is_eof());
    }

    #[test]
    fn reader_read_byte() {
        let buf = VsBuffer::from_string("hi");
        let mut reader = BufferReader::new(buf);
        assert_eq!(reader.read_byte(), Some(b'h'));
        assert_eq!(reader.read_byte(), Some(b'i'));
        assert_eq!(reader.read_byte(), None);
        assert!(reader.is_eof());
    }

    #[test]
    fn reader_read_bytes() {
        let buf = VsBuffer::from_string("hello world");
        let mut reader = BufferReader::new(buf);
        let chunk = reader.read_bytes(5);
        assert_eq!(chunk.to_string_lossy(), "hello");
        assert_eq!(reader.position(), 5);
    }

    #[test]
    fn reader_read_bytes_past_end() {
        let buf = VsBuffer::from_string("ab");
        let mut reader = BufferReader::new(buf);
        let chunk = reader.read_bytes(10);
        assert_eq!(chunk.to_string_lossy(), "ab");
        assert!(reader.is_eof());
    }

    #[test]
    fn reader_peek_does_not_advance() {
        let buf = VsBuffer::from_string("xy");
        let reader = BufferReader::new(buf);
        assert_eq!(reader.peek_byte(), Some(b'x'));
        assert_eq!(reader.position(), 0);
    }

    #[test]
    fn reader_skip() {
        let buf = VsBuffer::from_string("abcdef");
        let mut reader = BufferReader::new(buf);
        assert_eq!(reader.skip(3), 3);
        assert_eq!(reader.position(), 3);
        assert_eq!(reader.skip(100), 3); // only 3 left
        assert!(reader.is_eof());
    }

    #[test]
    fn reader_mark_and_reset() {
        let buf = VsBuffer::from_string("abcdef");
        let mut reader = BufferReader::new(buf);
        reader.skip(2);
        reader.set_mark();
        reader.skip(3);
        assert_eq!(reader.position(), 5);
        assert!(reader.reset_to_mark());
        assert_eq!(reader.position(), 2);
    }

    #[test]
    fn reader_reset_without_mark() {
        let buf = VsBuffer::from_string("ab");
        let mut reader = BufferReader::new(buf);
        assert!(!reader.reset_to_mark());
    }

    #[test]
    fn reader_read_until() {
        let buf = VsBuffer::from_string("key=value;rest");
        let mut reader = BufferReader::new(buf);
        let chunk = reader.read_until(b'=');
        assert_eq!(chunk.to_string_lossy(), "key=");
        assert_eq!(reader.position(), 4);
    }

    #[test]
    fn reader_read_until_not_found() {
        let buf = VsBuffer::from_string("no-delim");
        let mut reader = BufferReader::new(buf);
        let chunk = reader.read_until(b'!');
        assert_eq!(chunk.to_string_lossy(), "no-delim");
        assert!(reader.is_eof());
    }

    #[test]
    fn reader_read_line() {
        let buf = VsBuffer::from_string("line1\nline2\n");
        let mut reader = BufferReader::new(buf);
        let l1 = reader.read_line().unwrap();
        assert_eq!(l1.to_string_lossy(), "line1\n");
        let l2 = reader.read_line().unwrap();
        assert_eq!(l2.to_string_lossy(), "line2\n");
        assert!(reader.read_line().is_none());
    }

    #[test]
    fn reader_seek() {
        let buf = VsBuffer::from_string("abcdef");
        let mut reader = BufferReader::new(buf);
        assert!(reader.seek(3));
        assert_eq!(reader.read_byte(), Some(b'd'));
        assert!(!reader.seek(100)); // beyond end
    }

    // -----------------------------------------------------------------------
    // buffer_concat / buffer_concat_with_str / buffer_join_lines tests
    // -----------------------------------------------------------------------

    #[test]
    fn free_concat_no_separator() {
        let bufs = vec![
            VsBuffer::from_string("ab"),
            VsBuffer::from_string("cd"),
        ];
        let result = super::buffer_concat(&bufs, None);
        assert_eq!(result.to_string_lossy(), "abcd");
    }

    #[test]
    fn free_concat_with_separator() {
        let bufs = vec![
            VsBuffer::from_string("a"),
            VsBuffer::from_string("b"),
            VsBuffer::from_string("c"),
        ];
        let result = super::buffer_concat(&bufs, Some(b","));
        assert_eq!(result.to_string_lossy(), "a,b,c");
    }

    #[test]
    fn free_concat_empty_list() {
        let result = super::buffer_concat(&[], Some(b","));
        assert!(result.is_empty());
    }

    #[test]
    fn free_concat_single_buffer() {
        let bufs = vec![VsBuffer::from_string("only")];
        let result = super::buffer_concat(&bufs, Some(b"-"));
        assert_eq!(result.to_string_lossy(), "only");
    }

    #[test]
    fn free_concat_with_str_separator() {
        let bufs = vec![
            VsBuffer::from_string("x"),
            VsBuffer::from_string("y"),
        ];
        let result = buffer_concat_with_str(&bufs, " | ");
        assert_eq!(result.to_string_lossy(), "x | y");
    }

    #[test]
    fn free_concat_with_str_empty_sep() {
        let bufs = vec![
            VsBuffer::from_string("a"),
            VsBuffer::from_string("b"),
        ];
        let result = buffer_concat_with_str(&bufs, "");
        assert_eq!(result.to_string_lossy(), "ab");
    }

    #[test]
    fn free_join_lines() {
        let lines = vec![
            VsBuffer::from_string("first"),
            VsBuffer::from_string("second"),
            VsBuffer::from_string("third"),
        ];
        let result = buffer_join_lines(&lines);
        assert_eq!(result.to_string_lossy(), "first\nsecond\nthird");
    }

    #[test]
    fn join_lines_single() {
        let lines = vec![VsBuffer::from_string("only")];
        let result = buffer_join_lines(&lines);
        assert_eq!(result.to_string_lossy(), "only");
    }

    #[test]
    fn free_concat_with_multi_byte_separator() {
        let bufs = vec![
            VsBuffer::from_string("a"),
            VsBuffer::from_string("b"),
        ];
        let result = super::buffer_concat(&bufs, Some(b"<=>"));
        assert_eq!(result.to_string_lossy(), "a<=>b");
    }

    // -----------------------------------------------------------------------
    // ChunkIterator / WindowIterator / new VsBuffer method tests
    // -----------------------------------------------------------------------

    #[test]
    fn chunks_even_split() {
        let buf = VsBuffer::from_string("abcdef");
        let chunks: Vec<_> = buf.chunks(2).collect();
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].to_string_lossy(), "ab");
        assert_eq!(chunks[1].to_string_lossy(), "cd");
        assert_eq!(chunks[2].to_string_lossy(), "ef");
    }

    #[test]
    fn chunks_uneven_split() {
        let buf = VsBuffer::from_string("abcde");
        let chunks: Vec<_> = buf.chunks(2).collect();
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[2].to_string_lossy(), "e");
    }

    #[test]
    fn chunks_exact_size_hint() {
        let buf = VsBuffer::from_string("abcdef");
        let iter = buf.chunks(2);
        assert_eq!(iter.len(), 3);
    }

    #[test]
    fn windows_basic() {
        let buf = VsBuffer::from_string("abcd");
        let wins: Vec<_> = buf.windows(2).collect();
        assert_eq!(wins.len(), 3);
        assert_eq!(wins[0].to_string_lossy(), "ab");
        assert_eq!(wins[1].to_string_lossy(), "bc");
        assert_eq!(wins[2].to_string_lossy(), "cd");
    }

    #[test]
    fn windows_too_large() {
        let buf = VsBuffer::from_string("ab");
        let wins: Vec<_> = buf.windows(5).collect();
        assert!(wins.is_empty());
    }

    #[test]
    fn find_byte_found() {
        let buf = VsBuffer::from_string("hello");
        assert_eq!(buf.find_byte(b'l'), Some(2));
    }

    #[test]
    fn find_byte_not_found() {
        let buf = VsBuffer::from_string("hello");
        assert_eq!(buf.find_byte(b'z'), None);
    }

    #[test]
    fn contains_pattern_true() {
        let buf = VsBuffer::from_string("the quick brown fox");
        assert!(buf.contains_pattern(b"brown"));
    }

    #[test]
    fn contains_pattern_false() {
        let buf = VsBuffer::from_string("the quick brown fox");
        assert!(!buf.contains_pattern(b"lazy"));
    }

    #[test]
    fn xor_same_length() {
        let a = VsBuffer::new(vec![0xAA, 0xBB, 0xCC]);
        let b = VsBuffer::new(vec![0xFF, 0x00, 0xCC]);
        let result = a.xor(&b);
        assert_eq!(result.as_bytes(), &[0x55, 0xBB, 0x00]);
    }

    #[test]
    fn xor_different_lengths() {
        let a = VsBuffer::new(vec![0xFF, 0x00]);
        let b = VsBuffer::new(vec![0x0F]);
        let result = a.xor(&b);
        assert_eq!(result.len(), 1);
        assert_eq!(result.as_bytes(), &[0xF0]);
    }

    #[test]
    fn pad_extends() {
        let buf = VsBuffer::from_string("hi");
        let padded = buf.pad(5, b'.');
        assert_eq!(padded.len(), 5);
        assert_eq!(padded.to_string_lossy(), "hi...");
    }

    #[test]
    fn pad_already_sufficient() {
        let buf = VsBuffer::from_string("hello");
        let padded = buf.pad(3, b'.');
        assert_eq!(padded.to_string_lossy(), "hello");
    }

    #[test]
    fn reverse_buffer() {
        let buf = VsBuffer::from_string("abcd");
        assert_eq!(buf.reverse().to_string_lossy(), "dcba");
    }

    #[test]
    fn reverse_empty() {
        let buf = VsBuffer::empty();
        assert!(buf.reverse().is_empty());
    }

    #[test]
    fn repeat_buffer() {
        let buf = VsBuffer::from_string("ab");
        let repeated = buf.repeat(3);
        assert_eq!(repeated.to_string_lossy(), "ababab");
    }

    #[test]
    fn repeat_zero() {
        let buf = VsBuffer::from_string("ab");
        assert!(buf.repeat(0).is_empty());
    }

    #[test]
    fn replace_byte_basic() {
        let buf = VsBuffer::from_string("hello");
        let result = buf.replace_byte(b'l', b'r');
        assert_eq!(result.to_string_lossy(), "herro");
    }

    // -----------------------------------------------------------------------
    // BufferDiff tests
    // -----------------------------------------------------------------------

    #[test]
    fn diff_identical_buffers() {
        let a = VsBuffer::from_string("hello");
        let b = VsBuffer::from_string("hello");
        let result = BufferDiff::compare(&a, &b);
        assert!(result.identical);
        assert_eq!(result.first_diff_offset, None);
        assert_eq!(result.common_prefix_len, 5);
        assert_eq!(result.common_suffix_len, 0);
        assert!(BufferDiff::are_equal(&a, &b));
    }

    #[test]
    fn diff_different_buffers() {
        let a = VsBuffer::from_string("hello");
        let b = VsBuffer::from_string("hxllo");
        let result = BufferDiff::compare(&a, &b);
        assert!(!result.identical);
        assert_eq!(result.first_diff_offset, Some(1));
        assert_eq!(result.common_prefix_len, 1);
        assert_eq!(result.common_suffix_len, 3); // "llo"
    }

    #[test]
    fn diff_different_lengths() {
        let a = VsBuffer::from_string("abc");
        let b = VsBuffer::from_string("abcdef");
        let result = BufferDiff::compare(&a, &b);
        assert!(!result.identical);
        assert_eq!(result.first_diff_offset, Some(3));
        assert_eq!(result.common_prefix_len, 3);
    }

    // -----------------------------------------------------------------------
    // BufferCompression tests
    // -----------------------------------------------------------------------

    #[test]
    fn rle_encode_decode_roundtrip() {
        let data = b"aaabbbcccdddd";
        let encoded = BufferCompression::rle_encode(data);
        let decoded = BufferCompression::rle_decode(&encoded);
        assert_eq!(decoded, data);
    }

    #[test]
    fn rle_encode_empty() {
        let encoded = BufferCompression::rle_encode(b"");
        assert!(encoded.is_empty());
        let decoded = BufferCompression::rle_decode(&encoded);
        assert!(decoded.is_empty());
    }

    #[test]
    fn rle_compression_ratio() {
        let data = b"aaaaaaaaaa"; // 10 identical bytes
        let encoded = BufferCompression::rle_encode(data);
        let ratio = BufferCompression::compression_ratio(data, &encoded);
        assert!(ratio < 1.0, "ratio should be < 1.0 for repeated data");
    }

    // -----------------------------------------------------------------------
    // CircularBuffer tests
    // -----------------------------------------------------------------------

    #[test]
    fn circular_buffer_basic() {
        let mut cb = CircularBuffer::new(3);
        assert!(cb.is_empty());
        cb.push(1);
        cb.push(2);
        cb.push(3);
        assert!(cb.is_full());
        assert_eq!(cb.len(), 3);
        assert_eq!(cb.to_vec(), vec![1, 2, 3]);
    }

    #[test]
    fn circular_buffer_overwrite() {
        let mut cb = CircularBuffer::new(3);
        cb.push(1);
        cb.push(2);
        cb.push(3);
        cb.push(4); // overwrites 1
        assert_eq!(cb.to_vec(), vec![2, 3, 4]);
        assert_eq!(cb.get(0), Some(2));
        assert_eq!(cb.get(2), Some(4));
        assert_eq!(cb.get(3), None);
    }

    #[test]
    fn circular_buffer_clear() {
        let mut cb = CircularBuffer::new(4);
        cb.push(10);
        cb.push(20);
        cb.clear();
        assert!(cb.is_empty());
        assert_eq!(cb.to_vec(), vec![]);
    }

    // -----------------------------------------------------------------------
    // BufferSearch tests
    // -----------------------------------------------------------------------

    #[test]
    fn search_find_first() {
        let buf = VsBuffer::from_string("hello world hello");
        assert_eq!(BufferSearch::find_first(&buf, b"world"), Some(6));
        assert_eq!(BufferSearch::find_first(&buf, b"xyz"), None);
        assert_eq!(BufferSearch::find_first(&buf, b""), Some(0));
    }

    #[test]
    fn search_find_all_non_overlapping() {
        let buf = VsBuffer::from_string("abcabcabc");
        let positions = BufferSearch::find_all(&buf, b"abc");
        assert_eq!(positions, vec![0, 3, 6]);
    }

    #[test]
    fn search_count_occurrences() {
        let buf = VsBuffer::from_string("aaa");
        assert_eq!(BufferSearch::count_occurrences(&buf, b"a"), 3);
        assert_eq!(BufferSearch::count_occurrences(&buf, b"aa"), 1); // non-overlapping
        assert_eq!(BufferSearch::count_occurrences(&buf, b"b"), 0);
    }

    #[test] fn bufferPoolStats_new() { let s = BufferPoolStats::new(); assert_eq!(s.entry_count(), 0); assert!(s.is_enabled()); }
    #[test] fn bufferPoolStats_add() { let mut s = BufferPoolStats::new(); s.add_entry("a"); s.add_entry("b"); assert_eq!(s.entry_count(), 2); }
    #[test] fn bufferPoolStats_remove() { let mut s = BufferPoolStats::new(); s.add_entry("a"); assert!(s.remove_entry(0).is_some()); assert_eq!(s.entry_count(), 0); }
    #[test] fn bufferPoolStats_config() { let mut s = BufferPoolStats::new(); s.set_config("k", "v"); assert_eq!(s.get_config("k"), Some("v")); }
    #[test] fn bufferPoolStats_nav() { let mut s = BufferPoolStats::new(); s.add_entry("a"); s.add_entry("b"); s.select_next(); assert_eq!(s.current_index(), 1); s.select_prev(); assert_eq!(s.current_index(), 0); }
    #[test] fn bufferPoolStats_filter() { let mut s = BufferPoolStats::new(); s.add_entry("hello"); s.add_entry("world"); assert_eq!(s.filter_entries("llo").len(), 1); }
    #[test] fn bufferPoolStats_display() { assert!(format!("{}", BufferPoolStats::new()).contains("BufferPoolStats")); }
    #[test] fn bufferCompareUtil_new() { let s = BufferCompareUtil::new(); assert!(s.is_empty()); }
    #[test] fn bufferCompareUtil_add() { let mut s = BufferCompareUtil::new(); s.add_item("g1", "v1"); s.add_item("g1", "v2"); assert_eq!(s.total_items(), 2); assert_eq!(s.group_count(), 1); }
    #[test] fn bufferCompareUtil_active() { let mut s = BufferCompareUtil::new(); s.set_active("g1"); assert_eq!(s.active(), Some("g1")); s.clear_active(); assert!(s.active().is_none()); }
    #[test] fn bufferCompareUtil_error() { let mut s = BufferCompareUtil::new(); s.set_error("fail"); assert_eq!(s.last_error(), Some("fail")); s.clear_error(); assert!(s.last_error().is_none()); }
    #[test] fn bufferCompareUtil_rm_group() { let mut s = BufferCompareUtil::new(); s.add_item("g", "v"); assert!(s.remove_group("g")); assert!(s.is_empty()); }
    #[test] fn bufferCompareUtil_display() { assert!(format!("{}", BufferCompareUtil::new()).contains("BufferCompareUtil")); }


    #[test] fn bufferPoolStats_snap_capture() {
        let s = BufferPoolStats::new();
        let snap = BufferPoolStatsSnapshot::capture(&s, 1000);
        assert_eq!(snap.entry_count, 0);
        assert_eq!(snap.timestamp, 1000);
    }
    #[test] fn bufferPoolStats_snap_stale() {
        let s = BufferPoolStats::new();
        let snap = BufferPoolStatsSnapshot::capture(&s, 100);
        assert!(snap.is_stale(300, 100));
        assert!(!snap.is_stale(150, 100));
    }
    #[test] fn bufferPoolStats_snap_diff() {
        let s = BufferPoolStats::new();
        let s1v = BufferPoolStatsSnapshot::capture(&s, 100);
        let mut s2v = s1v.clone();
        s2v.entry_count = 5;
        assert_eq!(s2v.diff_entry_count(&s1v), 5);
    }
    #[test] fn bufferPoolStats_snap_display() {
        let s = BufferPoolStats::new();
        let snap = BufferPoolStatsSnapshot::capture(&s, 0);
        assert!(format!("{}", snap).contains("Snapshot"));
    }
    #[test] fn bufferCompareUtil_stats_record() {
        let mut st = BufferCompareUtilStats::new();
        st.record_add();
        st.record_add();
        st.record_remove();
        assert_eq!(st.net_changes(), 1);
    }
    #[test] fn bufferCompareUtil_stats_hit_ratio() {
        let mut st = BufferCompareUtilStats::new();
        st.record_lookup(true);
        st.record_lookup(true);
        st.record_lookup(false);
        assert!((st.hit_ratio() - 2.0/3.0).abs() < 0.01);
    }
    #[test] fn bufferCompareUtil_stats_merge() {
        let mut a = BufferCompareUtilStats::new();
        a.total_adds = 5;
        let mut b = BufferCompareUtilStats::new();
        b.total_adds = 3;
        a.merge(&b);
        assert_eq!(a.total_adds, 8);
    }
    #[test] fn bufferCompareUtil_stats_display() {
        let st = BufferCompareUtilStats::new();
        assert!(format!("{}", st).contains("Stats"));
    }
    #[test] fn bufferPoolStats_config_default() {
        let c = BufferPoolStatsConfig::new();
        assert_eq!(c.max_entries, 10000);
        assert!(c.auto_cleanup);
    }
    #[test] fn bufferPoolStats_config_builder() {
        let c = BufferPoolStatsConfig::new().with_max_entries(500).with_auto_cleanup(false).with_debounce(200);
        assert_eq!(c.max_entries, 500);
        assert!(!c.auto_cleanup);
        assert_eq!(c.debounce_ms, 200);
    }
    #[test] fn bufferPoolStats_config_labels() {
        let mut c = BufferPoolStatsConfig::new();
        c.set_label("a", "b");
        assert_eq!(c.get_label("a"), Some("b"));
        assert_eq!(c.label_count(), 1);
    }
    #[test] fn bufferPoolStats_config_cleanup_threshold() {
        let c = BufferPoolStatsConfig::new();
        assert!(!c.needs_cleanup(100));
        assert!(c.needs_cleanup(9000));
    }
    #[test] fn bufferPoolStats_config_display() {
        assert!(format!("{}", BufferPoolStatsConfig::new()).contains("Config"));
    }
    #[test] fn bufferCompareUtil_stats_peaks() {
        let mut st = BufferCompareUtilStats::new();
        st.update_peaks(5, 20);
        st.update_peaks(3, 25);
        assert_eq!(st.peak_group_count, 5);
        assert_eq!(st.peak_item_count, 25);
    }

    // -- BufferChunker -------------------------------------------------------

    #[test]
    fn chunker_fixed_size() {
        let buf = VsBuffer::from_string("abcdefgh");
        let chunks = BufferChunker::fixed_size(&buf, 3);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].to_string_lossy(), "abc");
        assert_eq!(chunks[2].to_string_lossy(), "gh");
    }

    #[test]
    fn chunker_fixed_empty() {
        let buf = VsBuffer::from_string("");
        assert!(BufferChunker::fixed_size(&buf, 4).is_empty());
    }

    #[test]
    fn chunker_fixed_zero_size() {
        let buf = VsBuffer::from_string("abc");
        assert!(BufferChunker::fixed_size(&buf, 0).is_empty());
    }

    #[test]
    fn chunker_by_lines() {
        let buf = VsBuffer::from_string("line1\nline2\nline3");
        let lines = BufferChunker::by_lines(&buf);
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0].to_string_lossy(), "line1\n");
        assert_eq!(lines[2].to_string_lossy(), "line3");
    }

    #[test]
    fn chunker_chunk_count() {
        assert_eq!(BufferChunker::chunk_count(10, 3), 4);
        assert_eq!(BufferChunker::chunk_count(9, 3), 3);
        assert_eq!(BufferChunker::chunk_count(0, 3), 0);
    }

    // -- BufferEncoder -------------------------------------------------------

    #[test]
    fn encoder_roundtrip() {
        let buf = VsBuffer::from_string("Hello, world!");
        let encoded = BufferEncoder::encode(&buf);
        let decoded = BufferEncoder::decode(&encoded).unwrap();
        assert_eq!(decoded.to_string_lossy(), "Hello, world!");
    }

    #[test]
    fn encoder_empty() {
        let buf = VsBuffer::from_string("");
        let encoded = BufferEncoder::encode(&buf);
        assert!(encoded.is_empty());
    }

    #[test]
    fn encoder_invalid_length() {
        let result = BufferEncoder::decode("ABC");
        assert!(result.is_err());
    }

    #[test]
    fn encoder_padding() {
        let buf = VsBuffer::new(Bytes::from_static(b"A"));
        let encoded = BufferEncoder::encode(&buf);
        assert!(encoded.ends_with("=="));
        let decoded = BufferEncoder::decode(&encoded).unwrap();
        assert_eq!(decoded.as_bytes(), b"A");
    }

    // -- BufferSearch contains_pattern + BufferDiff byte_changes -------------

    #[test]
    fn search_contains_pattern() {
        let buf = VsBuffer::from_string("hello world");
        assert!(BufferSearch::contains_pattern(&buf, b"world"));
        assert!(!BufferSearch::contains_pattern(&buf, b"xyz"));
    }

    #[test]
    fn diff_byte_changes() {
        let a = VsBuffer::from_string("abc");
        let b = VsBuffer::from_string("axc");
        let changes = BufferDiff::byte_changes(&a, &b);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0], (1, b'b', b'x'));
    }

    #[test]
    fn diff_byte_changes_different_lengths() {
        let a = VsBuffer::from_string("ab");
        let b = VsBuffer::from_string("abcd");
        let changes = BufferDiff::byte_changes(&a, &b);
        assert_eq!(changes.len(), 2);
    }


    #[test]
    fn buffer_entry_creation() {
        let e = BufferEntry::new("e1", "Entry 1");
        assert_eq!(e.id, "e1");
        assert_eq!(e.label, "Entry 1");
        assert!(e.active);
        assert_eq!(e.priority, 0);
    }

    #[test]
    fn buffer_entry_with_priority() {
        let e = BufferEntry::new("e1", "E").with_priority(5);
        assert_eq!(e.priority, 5);
    }

    #[test]
    fn buffer_entry_metadata() {
        let e = BufferEntry::new("e1", "E").with_meta("key", "val");
        assert_eq!(e.get_meta("key"), Some("val"));
        assert_eq!(e.get_meta("missing"), None);
        assert!(e.has_meta("key"));
        assert_eq!(e.meta_count(), 1);
    }

    #[test]
    fn buffer_entry_remove_meta() {
        let mut e = BufferEntry::new("e1", "E").with_meta("k", "v");
        assert!(e.remove_meta("k"));
        assert!(!e.remove_meta("k"));
    }

    #[test]
    fn buffer_entry_activate_deactivate() {
        let mut e = BufferEntry::new("e1", "E");
        e.deactivate();
        assert!(!e.active);
        e.activate();
        assert!(e.active);
    }

    #[test]
    fn buffer_config_add_sorted() {
        let mut c = BufferConfig::new(10);
        c.add(BufferEntry::new("lo", "Lo").with_priority(1));
        c.add(BufferEntry::new("hi", "Hi").with_priority(10));
        assert_eq!(c.ids()[0], "hi");
    }

    #[test]
    fn buffer_config_capacity() {
        let mut c = BufferConfig::new(1);
        assert!(c.add(BufferEntry::new("a", "A")));
        assert!(!c.add(BufferEntry::new("b", "B")));
        assert!(c.is_full());
    }

    #[test]
    fn buffer_config_remove() {
        let mut c = BufferConfig::new(10);
        c.add(BufferEntry::new("a", "A"));
        assert!(c.remove("a"));
        assert!(!c.remove("a"));
        assert!(c.is_empty());
    }

    #[test]
    fn buffer_config_get() {
        let mut c = BufferConfig::new(10);
        c.add(BufferEntry::new("x", "X"));
        assert!(c.get("x").is_some());
        assert!(c.get("y").is_none());
    }

    #[test]
    fn buffer_config_active_entries() {
        let mut c = BufferConfig::new(10);
        c.add(BufferEntry::new("a", "A"));
        c.add(BufferEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        assert_eq!(c.active_entries().len(), 1);
        assert_eq!(c.count_active(), 1);
    }

    #[test]
    fn buffer_config_enable_disable() {
        let mut c = BufferConfig::new(10);
        c.disable();
        assert!(!c.is_enabled());
        c.enable();
        assert!(c.is_enabled());
    }

    #[test]
    fn buffer_config_clear() {
        let mut c = BufferConfig::new(10);
        c.add(BufferEntry::new("a", "A"));
        c.clear();
        assert!(c.is_empty());
    }

    #[test]
    fn buffer_config_find_by_label() {
        let mut c = BufferConfig::new(10);
        c.add(BufferEntry::new("a", "Alpha"));
        assert_eq!(c.find_by_label("Alpha").unwrap().id, "a");
        assert!(c.find_by_label("missing").is_none());
    }

    #[test]
    fn buffer_config_top_n() {
        let mut c = BufferConfig::new(10);
        c.add(BufferEntry::new("a", "A").with_priority(1));
        c.add(BufferEntry::new("b", "B").with_priority(2));
        c.add(BufferEntry::new("c", "C").with_priority(3));
        assert_eq!(c.top_n(2).len(), 2);
    }

    #[test]
    fn buffer_config_deactivate_activate_all() {
        let mut c = BufferConfig::new(10);
        c.add(BufferEntry::new("a", "A"));
        c.add(BufferEntry::new("b", "B"));
        c.deactivate_all();
        assert_eq!(c.count_active(), 0);
        c.activate_all();
        assert_eq!(c.count_active(), 2);
    }

    #[test]
    fn buffer_config_highest_priority() {
        let mut c = BufferConfig::new(10);
        assert!(c.highest_priority().is_none());
        c.add(BufferEntry::new("a", "A").with_priority(7));
        assert_eq!(c.highest_priority(), Some(7));
    }

    #[test]
    fn buffer_config_contains() {
        let mut c = BufferConfig::new(10);
        c.add(BufferEntry::new("a", "A"));
        assert!(c.contains("a"));
        assert!(!c.contains("b"));
    }

    #[test]
    fn buffer_config_labels() {
        let mut c = BufferConfig::new(10);
        c.add(BufferEntry::new("a", "Alpha"));
        c.add(BufferEntry::new("b", "Beta"));
        let labels = c.labels();
        assert!(labels.contains(&"Alpha"));
        assert!(labels.contains(&"Beta"));
    }

    #[test]
    fn buffer_config_drain_inactive() {
        let mut c = BufferConfig::new(10);
        c.add(BufferEntry::new("a", "A"));
        c.add(BufferEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        let drained = c.drain_inactive();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].id, "a");
        assert_eq!(c.len(), 1);
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


    // xa_ extended tests for buffer
    #[test]
    fn xa_buffer_ring_new() {
        let rb = super::XaBufferRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_buffer_ring_push_len() {
        let mut rb = super::XaBufferRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_buffer_ring_wrap() {
        let mut rb = super::XaBufferRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_buffer_ring_mean_empty() {
        let rb = super::XaBufferRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_buffer_ring_mean_values() {
        let mut rb = super::XaBufferRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_buffer_ring_min_max() {
        let mut rb = super::XaBufferRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_buffer_ring_iter() {
        let mut rb = super::XaBufferRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_buffer_counter_new() {
        let c = super::XaBufferCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_buffer_counter_inc() {
        let mut c = super::XaBufferCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_buffer_counter_inc_by() {
        let mut c = super::XaBufferCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_buffer_counter_reset() {
        let mut c = super::XaBufferCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_buffer_counter_clear() {
        let mut c = super::XaBufferCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_buffer_counter_default() {
        let c = super::XaBufferCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }


    // ---- xc_ pool / scheduler tests – block 9 ----

    #[test]
    fn xc_9_pool_new_empty() {
        let pool: super::Xc9Pool<i32> = super::Xc9Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_9_pool_release_acquire() {
        let mut pool = super::Xc9Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_9_pool_acquire_empty() {
        let mut pool: super::Xc9Pool<i32> = super::Xc9Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_9_pool_full() {
        let mut pool = super::Xc9Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_9_pool_drain() {
        let mut pool = super::Xc9Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_9_pool_stats() {
        let mut pool = super::Xc9Pool::new(8);
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
    fn xc_9_pool_clear() {
        let mut pool = super::Xc9Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_9_pool_shrink() {
        let mut pool = super::Xc9Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_9_pool_default() {
        let pool: super::Xc9Pool<String> = super::Xc9Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_9_pool_extend() {
        let mut pool = super::Xc9Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_9_pool_retain() {
        let mut pool = super::Xc9Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_9_scheduler_round_robin() {
        let mut sched = super::Xc9Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_9_scheduler_empty() {
        let mut sched = super::Xc9Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_9_scheduler_reset() {
        let mut sched = super::Xc9Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_9_scheduler_add_remove() {
        let mut sched = super::Xc9Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_9_scheduler_targets() {
        let sched = super::Xc9Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_9_hash_empty() {
        assert_eq!(super::xc_9_hash(b""), 5381);
    }

    #[test]
    fn xc_9_hash_data() {
        let h = super::xc_9_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_9_hash(b"hello"), h);
    }

    #[test]
    fn xc_9_reverse_str() {
        assert_eq!(super::xc_9_reverse("abc"), "cba");
        assert_eq!(super::xc_9_reverse(""), "");
    }


    // --- xd_63 deepening tests ---

    #[test]
    fn xd_63_sm_initial_state() {
        let sm = Xd63StateMachine::new();
        assert_eq!(sm.current_state(), Xd63State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_63_sm_valid_idle_to_running() {
        let mut sm = Xd63StateMachine::new();
        assert!(sm.transition(Xd63State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd63State::Running);
    }

    #[test]
    fn xd_63_sm_valid_running_to_paused() {
        let mut sm = Xd63StateMachine::new();
        sm.transition(Xd63State::Running).unwrap();
        assert!(sm.transition(Xd63State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd63State::Paused);
    }

    #[test]
    fn xd_63_sm_valid_running_to_done() {
        let mut sm = Xd63StateMachine::new();
        sm.transition(Xd63State::Running).unwrap();
        assert!(sm.transition(Xd63State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd63State::Done);
    }

    #[test]
    fn xd_63_sm_valid_paused_to_running() {
        let mut sm = Xd63StateMachine::new();
        sm.transition(Xd63State::Running).unwrap();
        sm.transition(Xd63State::Paused).unwrap();
        assert!(sm.transition(Xd63State::Running).is_ok());
    }

    #[test]
    fn xd_63_sm_valid_done_to_idle() {
        let mut sm = Xd63StateMachine::new();
        sm.transition(Xd63State::Running).unwrap();
        sm.transition(Xd63State::Done).unwrap();
        assert!(sm.transition(Xd63State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd63State::Idle);
    }

    #[test]
    fn xd_63_sm_invalid_idle_to_done() {
        let mut sm = Xd63StateMachine::new();
        assert!(sm.transition(Xd63State::Done).is_err());
    }

    #[test]
    fn xd_63_sm_invalid_idle_to_paused() {
        let mut sm = Xd63StateMachine::new();
        assert!(sm.transition(Xd63State::Paused).is_err());
    }

    #[test]
    fn xd_63_sm_history_tracking() {
        let mut sm = Xd63StateMachine::new();
        sm.transition(Xd63State::Running).unwrap();
        sm.transition(Xd63State::Paused).unwrap();
        sm.transition(Xd63State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd63State::Idle);
        assert_eq!(sm.history()[0].to, Xd63State::Running);
        assert_eq!(sm.history()[1].from, Xd63State::Running);
        assert_eq!(sm.history()[2].to, Xd63State::Done);
    }

    #[test]
    fn xd_63_sm_serialize_deserialize() {
        let mut sm = Xd63StateMachine::new();
        sm.transition(Xd63State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd63StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd63State::Running));
    }

    #[test]
    fn xd_63_sm_deserialize_invalid() {
        assert_eq!(Xd63StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_63_sm_reset() {
        let mut sm = Xd63StateMachine::new();
        sm.transition(Xd63State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd63State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_63_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd63EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd63Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_63_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd63EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd63Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd63Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_63_bus_unsubscribe() {
        let mut bus = Xd63EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_63_event_kind_and_payload() {
        let e = Xd63Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd63Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_63_bus_clear_history() {
        let mut bus = Xd63EventBus::new();
        bus.publish(Xd63Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_63_sm_step_counter_increments() {
        let mut sm = Xd63StateMachine::new();
        sm.transition(Xd63State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd63State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xf_ trie + bloom tests for instance #62 --

    #[test]
    fn xf62_trie_insert_search() {
        let mut t = Xf62Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf62_trie_starts_with() {
        let mut t = Xf62Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf62_trie_remove() {
        let mut t = Xf62Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf62_trie_word_count() {
        let mut t = Xf62Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf62_trie_longest_prefix() {
        let mut t = Xf62Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf62_trie_all_words() {
        let mut t = Xf62Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf62_trie_autocomplete() {
        let mut t = Xf62Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf62_trie_empty_search() {
        let t = Xf62Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf62_bloom_add_contains() {
        let mut bf = Xf62BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf62_bloom_probably_absent() {
        let bf = Xf62BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf62_bloom_false_positive_rate() {
        let mut bf = Xf62BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf62_bloom_clear() {
        let mut bf = Xf62BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf62_bloom_union() {
        let mut a = Xf62BloomFilter::xf_new(512, 2);
        let mut b = Xf62BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf62_bloom_intersection_estimate() {
        let mut a = Xf62BloomFilter::xf_new(512, 2);
        let mut b = Xf62BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf62_bloom_union_size_mismatch() {
        let a = Xf62BloomFilter::xf_new(256, 2);
        let b = Xf62BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }


    #[test]
    fn xh8_skip_insert_contains() {
        let mut sl = super::Xh8SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh8_skip_remove() {
        let mut sl = super::Xh8SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh8_skip_len() {
        let mut sl = super::Xh8SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh8_skip_range_query() {
        let mut sl = super::Xh8SkipList::xh_new(4);
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
    fn xh8_skip_floor_ceiling() {
        let mut sl = super::Xh8SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh8_skip_rank() {
        let mut sl = super::Xh8SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh8_skip_empty() {
        let sl = super::Xh8SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh8_skip_duplicates() {
        let mut sl = super::Xh8SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh8_bitset_set_test() {
        let mut bs = super::Xh8BitSet::xh_new(256);
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
    fn xh8_bitset_clear_count() {
        let mut bs = super::Xh8BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh8_bitset_and_or_xor() {
        let mut a = super::Xh8BitSet::xh_new(128);
        let mut b = super::Xh8BitSet::xh_new(128);
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
    fn xh8_bitset_iter_ones() {
        let mut bs = super::Xh8BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh8_bitset_first_last() {
        let mut bs = super::Xh8BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh8_bitset_empty() {
        let bs = super::Xh8BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi8_deque_push_pop_back() {
        let mut dq = super::Xi8Deque::xi_new(4);
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
    fn xi8_deque_push_pop_front() {
        let mut dq = super::Xi8Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi8_deque_mixed_ops() {
        let mut dq = super::Xi8Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi8_deque_get_and_split() {
        let mut dq = super::Xi8Deque::xi_new(8);
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
    fn xi8_deque_rotate_left() {
        let mut dq = super::Xi8Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi8_deque_rotate_right() {
        let mut dq = super::Xi8Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi8_deque_grow() {
        let mut dq = super::Xi8Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi8_deque_empty() {
        let dq = super::Xi8Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi8_interval_tree_insert_query() {
        let mut tree = super::Xi8IntervalTree::xi_new();
        tree.xi_insert(super::Xi8Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi8Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi8Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi8_interval_tree_overlap() {
        let mut tree = super::Xi8IntervalTree::xi_new();
        tree.xi_insert(super::Xi8Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi8Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi8Interval::xi_new(12, 20));
        let q = super::Xi8Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi8_interval_tree_remove() {
        let mut tree = super::Xi8IntervalTree::xi_new();
        tree.xi_insert(super::Xi8Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi8Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi8_interval_tree_gaps() {
        let mut tree = super::Xi8IntervalTree::xi_new();
        tree.xi_insert(super::Xi8Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi8Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi8Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi8Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi8Interval::xi_new(8, 10));
    }

    #[test]
    fn xi8_interval_tree_merge() {
        let mut tree = super::Xi8IntervalTree::xi_new();
        tree.xi_insert(super::Xi8Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi8Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi8Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi8Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi8Interval::xi_new(10, 15));
    }

    #[test]
    fn xi8_interval_tree_all() {
        let mut tree = super::Xi8IntervalTree::xi_new();
        tree.xi_insert(super::Xi8Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi8Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi8_interval_tree_empty() {
        let tree = super::Xi8IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi8_interval_tree_contains_point() {
        let iv = super::Xi8Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 8) ---

    #[test]
    fn xj_8_uf_make_and_find() {
        let mut uf = super::Xj8UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_8_uf_union_connected() {
        let mut uf = super::Xj8UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_8_uf_component_count() {
        let mut uf = super::Xj8UnionFind::xj_new();
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
    fn xj_8_uf_component_size() {
        let mut uf = super::Xj8UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_8_uf_largest_component() {
        let mut uf = super::Xj8UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_8_uf_many_elements() {
        let mut uf = super::Xj8UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_8_uf_separate_components() {
        let mut uf = super::Xj8UnionFind::xj_new();
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
    fn xj_8_uf_path_compression() {
        let mut uf = super::Xj8UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_8_bt_insert_get() {
        let mut bt = super::Xj8BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_8_bt_contains_len() {
        let mut bt = super::Xj8BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_8_bt_replace() {
        let mut bt = super::Xj8BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_8_bt_remove() {
        let mut bt = super::Xj8BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_8_bt_keys_values() {
        let mut bt = super::Xj8BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_8_bt_range() {
        let mut bt = super::Xj8BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_8_bt_min_max() {
        let mut bt = super::Xj8BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_8_bt_many_inserts() {
        let mut bt = super::Xj8BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_8 segment tree tests ---

    #[test]
    fn xk_8_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk8SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_8_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk8SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_8_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk8SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_8_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk8SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_8_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk8SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_8_st_single_element() {
        let data = vec![42];
        let st = super::Xk8SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_8_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk8SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_8_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk8SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_8 disjoint intervals tests ---

    #[test]
    fn xk_8_di_add_and_count() {
        let mut di = super::Xk8DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_8_di_merge_overlap() {
        let mut di = super::Xk8DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_8_di_contains() {
        let mut di = super::Xk8DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_8_di_remove() {
        let mut di = super::Xk8DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_8_di_covered_length() {
        let mut di = super::Xk8DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_8_di_gaps() {
        let mut di = super::Xk8DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_8_di_merge_adjacent() {
        let mut di = super::Xk8DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_8_di_empty() {
        let di = super::Xk8DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }

}
