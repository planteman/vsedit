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

// ---------------------------------------------------------------------------
// StringStream
// ---------------------------------------------------------------------------

/// A simple stream of owned strings.
pub struct StringStream {
    items: Vec<String>,
    position: usize,
}

impl StringStream {
    /// Create a stream from a vector of strings.
    pub fn from_strings(items: Vec<String>) -> Self {
        Self { items, position: 0 }
    }
}

impl ReadableStream for StringStream {
    type Item = String;

    fn read(&mut self) -> Option<String> {
        if self.position < self.items.len() {
            let item = self.items[self.position].clone();
            self.position += 1;
            Some(item)
        } else {
            None
        }
    }
}

// ---------------------------------------------------------------------------
// Extra methods
// ---------------------------------------------------------------------------

impl BufferStream {
    /// Number of chunks remaining to be read.
    pub fn remaining(&self) -> usize {
        self.chunks.len().saturating_sub(self.position)
    }
}

impl BufferWriter {
    /// Number of chunks written so far.
    pub fn chunk_count(&self) -> usize {
        self.chunks.len()
    }

    /// Total byte size across all written chunks.
    pub fn total_size(&self) -> usize {
        self.chunks.iter().map(|c| c.len()).sum()
    }
}

// ---------------------------------------------------------------------------
// Free functions
// ---------------------------------------------------------------------------

/// Read at most `n` chunks from a stream.
pub fn take<S: ReadableStream>(stream: &mut S, n: usize) -> Vec<S::Item> {
    let mut result = Vec::with_capacity(n);
    for _ in 0..n {
        match stream.read() {
            Some(item) => result.push(item),
            None => break,
        }
    }
    result
}

/// Consume the entire stream and return how many chunks it contained.
pub fn count_chunks<S: ReadableStream>(stream: &mut S) -> usize {
    let mut count = 0;
    while stream.read().is_some() {
        count += 1;
    }
    count
}

// ---------------------------------------------------------------------------
// FilterStream
// ---------------------------------------------------------------------------

/// A stream adapter that only yields chunks satisfying a predicate.
pub struct FilterStream<S, F> {
    inner: S,
    predicate: F,
}

impl<S, F> FilterStream<S, F> {
    pub fn new(inner: S, predicate: F) -> Self {
        Self { inner, predicate }
    }
}

impl<S, F> ReadableStream for FilterStream<S, F>
where
    S: ReadableStream,
    F: FnMut(&S::Item) -> bool,
    Self: Send,
{
    type Item = S::Item;

    fn read(&mut self) -> Option<Self::Item> {
        loop {
            match self.inner.read() {
                Some(item) if (self.predicate)(&item) => return Some(item),
                Some(_) => continue,
                None => return None,
            }
        }
    }
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

    #[test]
    fn string_stream_read_all() {
        let mut stream = StringStream::from_strings(vec![
            "hello".into(),
            "world".into(),
        ]);
        let all = stream.collect_all();
        assert_eq!(all, vec!["hello", "world"]);
    }

    #[test]
    fn string_stream_empty() {
        let mut stream = StringStream::from_strings(vec![]);
        assert!(stream.read().is_none());
    }

    #[test]
    fn buffer_stream_remaining() {
        let mut stream = BufferStream::from_chunks(vec![
            VsBuffer::from_string("a"),
            VsBuffer::from_string("b"),
            VsBuffer::from_string("c"),
        ]);
        assert_eq!(stream.remaining(), 3);
        stream.read();
        assert_eq!(stream.remaining(), 2);
        stream.read();
        stream.read();
        assert_eq!(stream.remaining(), 0);
    }

    #[test]
    fn buffer_writer_chunk_count_and_total_size() {
        let mut writer = BufferWriter::new();
        assert_eq!(writer.chunk_count(), 0);
        assert_eq!(writer.total_size(), 0);
        writer.write(VsBuffer::from_string("hello"));
        writer.write(VsBuffer::from_string("ab"));
        assert_eq!(writer.chunk_count(), 2);
        assert_eq!(writer.total_size(), 7);
    }

    #[test]
    fn take_partial() {
        let mut stream = BufferStream::from_chunks(vec![
            VsBuffer::from_string("a"),
            VsBuffer::from_string("b"),
            VsBuffer::from_string("c"),
        ]);
        let taken = take(&mut stream, 2);
        assert_eq!(taken.len(), 2);
        assert_eq!(taken[0].to_string_lossy(), "a");
        assert_eq!(taken[1].to_string_lossy(), "b");
        // one chunk remaining
        assert_eq!(stream.remaining(), 1);
    }

    #[test]
    fn take_more_than_available() {
        let mut stream = BufferStream::from_chunks(vec![
            VsBuffer::from_string("x"),
        ]);
        let taken = take(&mut stream, 10);
        assert_eq!(taken.len(), 1);
    }

    #[test]
    fn count_chunks_stream() {
        let mut stream = BufferStream::from_chunks(vec![
            VsBuffer::from_string("a"),
            VsBuffer::from_string("b"),
        ]);
        assert_eq!(count_chunks(&mut stream), 2);
        // stream exhausted
        assert!(stream.read().is_none());
    }

    #[test]
    fn filter_stream_basic() {
        let inner = StringStream::from_strings(vec![
            "apple".into(),
            "banana".into(),
            "avocado".into(),
            "cherry".into(),
        ]);
        let mut filtered = FilterStream::new(inner, |s: &String| s.starts_with('a'));
        assert_eq!(filtered.read().unwrap(), "apple");
        assert_eq!(filtered.read().unwrap(), "avocado");
        assert!(filtered.read().is_none());
    }

    #[test]
    fn filter_stream_none_match() {
        let inner = StringStream::from_strings(vec![
            "banana".into(),
            "cherry".into(),
        ]);
        let mut filtered = FilterStream::new(inner, |s: &String| s.starts_with('z'));
        assert!(filtered.read().is_none());
    }
}
