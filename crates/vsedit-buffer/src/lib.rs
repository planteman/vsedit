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

}
