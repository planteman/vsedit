//! Stream abstractions.
//!
//! Equivalent to VS Code's `vs/base/common/stream.ts`.

use vsedit_buffer::VsBuffer;

/// A readable stream that yields chunks of data.
pub trait ReadableStream: Send {
    type Item;

    /// Read the next chunk. Returns `None` when the stream ends.
    fn read(&mut self) -> Option<Self::Item>;

    /// Collect all remaining data into a vector.
    fn collect_all(&mut self) -> Vec<Self::Item> {
        let mut items = Vec::new();
        while let Some(item) = self.read() {
            items.push(item);
        }
        items
    }
}

/// A buffered stream that reads VsBuffer chunks.
pub struct BufferStream {
    chunks: Vec<VsBuffer>,
    position: usize,
}

impl BufferStream {
    /// Create a stream from a list of buffer chunks.
    pub fn from_chunks(chunks: Vec<VsBuffer>) -> Self {
        Self {
            chunks,
            position: 0,
        }
    }

    /// Create a stream from a single buffer.
    pub fn from_buffer(buffer: VsBuffer) -> Self {
        Self::from_chunks(vec![buffer])
    }

    /// Create an empty stream.
    pub fn empty() -> Self {
        Self::from_chunks(vec![])
    }

    /// Read all chunks and concatenate into a single buffer.
    pub fn consume(mut self) -> VsBuffer {
        let chunks: Vec<VsBuffer> = self.collect_all();
        VsBuffer::concat(&chunks)
    }
}

impl ReadableStream for BufferStream {
    type Item = VsBuffer;

    fn read(&mut self) -> Option<VsBuffer> {
        if self.position < self.chunks.len() {
            let chunk = self.chunks[self.position].clone();
            self.position += 1;
            Some(chunk)
        } else {
            None
        }
    }
}

/// A writable stream that accepts chunks of data.
pub trait WritableStream: Send {
    type Item;

    /// Write a chunk of data.
    fn write(&mut self, data: Self::Item);

    /// Signal that no more data will be written.
    fn end(&mut self);
}

/// A buffer writer that collects written chunks.
pub struct BufferWriter {
    chunks: Vec<VsBuffer>,
    ended: bool,
}

impl BufferWriter {
    /// Create a new buffer writer.
    pub fn new() -> Self {
        Self {
            chunks: Vec::new(),
            ended: false,
        }
    }

    /// Get all written chunks.
    pub fn chunks(&self) -> &[VsBuffer] {
        &self.chunks
    }

    /// Consume the writer and concatenate all chunks.
    pub fn into_buffer(self) -> VsBuffer {
        VsBuffer::concat(&self.chunks)
    }

    /// Check if the stream has ended.
    pub fn is_ended(&self) -> bool {
        self.ended
    }
}

impl Default for BufferWriter {
    fn default() -> Self {
        Self::new()
    }
}

impl WritableStream for BufferWriter {
    type Item = VsBuffer;

    fn write(&mut self, data: VsBuffer) {
        if !self.ended {
            self.chunks.push(data);
        }
    }

    fn end(&mut self) {
        self.ended = true;
    }
}

/// Pipe data from a readable stream to a writable stream.
pub fn pipe<T>(
    source: &mut impl ReadableStream<Item = T>,
    dest: &mut impl WritableStream<Item = T>,
) {
    while let Some(chunk) = source.read() {
        dest.write(chunk);
    }
    dest.end();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buffer_stream_read() {
        let mut stream = BufferStream::from_buffer(VsBuffer::from_string("hello"));
        assert_eq!(stream.read().unwrap().to_string_lossy(), "hello");
        assert!(stream.read().is_none());
    }

    #[test]
    fn buffer_stream_consume() {
        let stream = BufferStream::from_chunks(vec![
            VsBuffer::from_string("hello "),
            VsBuffer::from_string("world"),
        ]);
        let result = stream.consume();
        assert_eq!(result.to_string_lossy(), "hello world");
    }

    #[test]
    fn pipe_streams() {
        let mut source = BufferStream::from_chunks(vec![
            VsBuffer::from_string("a"),
            VsBuffer::from_string("b"),
        ]);
        let mut dest = BufferWriter::new();
        pipe(&mut source, &mut dest);
        assert!(dest.is_ended());
        assert_eq!(dest.into_buffer().to_string_lossy(), "ab");
    }
}
