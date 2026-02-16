//! Cross-platform buffer abstraction.
//!
//! Wraps `bytes::Bytes` to provide a VS Code `VSBuffer`-like interface.
//! Equivalent to VS Code's `vs/base/common/buffer.ts`.

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
}
