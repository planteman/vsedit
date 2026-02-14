//! Cross-platform buffer abstraction.
//!
//! Wraps `bytes::Bytes` to provide a VS Code `VSBuffer`-like interface.
//! Equivalent to VS Code's `vs/base/common/buffer.ts`.

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
}
