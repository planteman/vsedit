//! Stream abstractions.
//!
//! Equivalent to VS Code's `vs/base/common/stream.ts`.

use std::fmt;
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

// ---------------------------------------------------------------------------
// MapStream adapter
// ---------------------------------------------------------------------------

/// A stream adapter that transforms each item with a mapping function.
pub struct MapStream<S, F> {
    inner: S,
    mapper: F,
}

impl<S, F> MapStream<S, F> {
    pub fn new(inner: S, mapper: F) -> Self {
        Self { inner, mapper }
    }
}

impl<S, F, T> ReadableStream for MapStream<S, F>
where
    S: ReadableStream,
    F: FnMut(S::Item) -> T,
    Self: Send,
{
    type Item = T;

    fn read(&mut self) -> Option<T> {
        self.inner.read().map(|item| (self.mapper)(item))
    }
}

// ---------------------------------------------------------------------------
// ChainStream adapter
// ---------------------------------------------------------------------------

/// A stream that reads from `first` until exhausted, then reads from `second`.
pub struct ChainStream<S1, S2> {
    first: S1,
    second: S2,
    first_exhausted: bool,
}

impl<S1, S2> ChainStream<S1, S2> {
    pub fn new(first: S1, second: S2) -> Self {
        Self {
            first,
            second,
            first_exhausted: false,
        }
    }
}

impl<T, S1, S2> ReadableStream for ChainStream<S1, S2>
where
    S1: ReadableStream<Item = T>,
    S2: ReadableStream<Item = T>,
    Self: Send,
{
    type Item = T;

    fn read(&mut self) -> Option<T> {
        if !self.first_exhausted {
            if let Some(item) = self.first.read() {
                return Some(item);
            }
            self.first_exhausted = true;
        }
        self.second.read()
    }
}

// ---------------------------------------------------------------------------
// TakeStream adapter
// ---------------------------------------------------------------------------

/// A stream that yields at most `limit` items from the inner stream.
pub struct TakeStream<S> {
    inner: S,
    remaining: usize,
}

impl<S> TakeStream<S> {
    pub fn new(inner: S, limit: usize) -> Self {
        Self {
            inner,
            remaining: limit,
        }
    }
}

impl<S: ReadableStream> ReadableStream for TakeStream<S>
where
    Self: Send,
{
    type Item = S::Item;

    fn read(&mut self) -> Option<S::Item> {
        if self.remaining == 0 {
            return None;
        }
        self.remaining -= 1;
        self.inner.read()
    }
}

// ---------------------------------------------------------------------------
// StringWriter
// ---------------------------------------------------------------------------

/// A writable stream that collects strings.
pub struct StringWriter {
    items: Vec<String>,
    ended: bool,
}

impl StringWriter {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            ended: false,
        }
    }

    pub fn items(&self) -> &[String] {
        &self.items
    }

    pub fn into_string(self) -> String {
        self.items.join("")
    }

    pub fn is_ended(&self) -> bool {
        self.ended
    }
}

impl Default for StringWriter {
    fn default() -> Self {
        Self::new()
    }
}

impl WritableStream for StringWriter {
    type Item = String;

    fn write(&mut self, data: String) {
        if !self.ended {
            self.items.push(data);
        }
    }

    fn end(&mut self) {
        self.ended = true;
    }
}

/// Accumulated statistics for stream operations.
#[derive(Debug, Clone, PartialEq)]
pub struct StreamStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl StreamStats {
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
    pub fn merge(&mut self, other: &StreamStats) {
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

impl Default for StreamStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for StreamStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "StreamStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for stream.
#[derive(Debug, Clone)]
pub struct StreamValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl StreamValidator {
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

impl Default for StreamValidator {
    fn default() -> Self {
        Self::new()
    }
}

/// A buffer that accumulates string chunks and can be read back.
pub struct StreamBuffer {
    chunks: Vec<String>,
    total_bytes: usize,
}

impl StreamBuffer {
    /// Create a new empty stream buffer.
    pub fn new() -> Self {
        Self {
            chunks: Vec::new(),
            total_bytes: 0,
        }
    }

    /// Append a chunk to the buffer.
    pub fn push(&mut self, chunk: impl Into<String>) {
        let s = chunk.into();
        self.total_bytes += s.len();
        self.chunks.push(s);
    }

    /// Get all accumulated chunks.
    pub fn chunks(&self) -> &[String] {
        &self.chunks
    }

    /// Get total byte count across all chunks.
    pub fn total_bytes(&self) -> usize {
        self.total_bytes
    }

    /// Drain all chunks, returning them and resetting the buffer.
    pub fn drain(&mut self) -> Vec<String> {
        self.total_bytes = 0;
        std::mem::take(&mut self.chunks)
    }

    /// Concatenate all chunks into a single string.
    pub fn concat(&self) -> String {
        self.chunks.join("")
    }

    /// Number of chunks accumulated.
    pub fn chunk_count(&self) -> usize {
        self.chunks.len()
    }

    /// Whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.chunks.is_empty()
    }
}

impl Default for StreamBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for StreamBuffer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "StreamBuffer({} chunks, {} bytes)", self.chunks.len(), self.total_bytes)
    }
}

/// A transform that processes string chunks through a transformation function.
pub struct StreamTransform {
    transforms: Vec<Box<dyn Fn(&str) -> String + Send>>,
}

impl StreamTransform {
    /// Create a new empty transform pipeline.
    pub fn new() -> Self {
        Self { transforms: Vec::new() }
    }

    /// Add a transform step to the pipeline.
    pub fn pipe(mut self, f: impl Fn(&str) -> String + Send + 'static) -> Self {
        self.transforms.push(Box::new(f));
        self
    }

    /// Apply all transforms to a single chunk.
    pub fn apply(&self, input: &str) -> String {
        let mut result = input.to_string();
        for transform in &self.transforms {
            result = transform(&result);
        }
        result
    }

    /// Apply all transforms to a stream of chunks.
    pub fn apply_all(&self, chunks: &[String]) -> Vec<String> {
        chunks.iter().map(|c| self.apply(c)).collect()
    }

    /// Number of transform steps in the pipeline.
    pub fn step_count(&self) -> usize {
        self.transforms.len()
    }
}

impl Default for StreamTransform {
    fn default() -> Self {
        Self::new()
    }
}

/// Split a stream of string chunks into complete lines.
/// Handles chunks that span line boundaries (a chunk might end mid-line).
pub fn stream_lines(chunks: &[String]) -> Vec<String> {
    let combined: String = chunks.join("");
    combined.lines().map(|l| l.to_string()).collect()
}

/// Split a stream of string chunks into lines, preserving partial last line.
/// Returns (complete_lines, pending_partial).
pub fn stream_lines_with_pending(chunks: &[String]) -> (Vec<String>, Option<String>) {
    let combined: String = chunks.join("");
    if combined.is_empty() {
        return (Vec::new(), None);
    }
    let ends_with_newline = combined.ends_with('\n');
    let mut lines: Vec<String> = combined.lines().map(|l| l.to_string()).collect();
    if ends_with_newline || lines.is_empty() {
        (lines, None)
    } else {
        let pending = lines.pop();
        (lines, pending)
    }
}

// ---------------------------------------------------------------------------
// BufferStream helpers
// ---------------------------------------------------------------------------

impl BufferStream {
    /// Total bytes remaining across all unread chunks.
    pub fn total_remaining_bytes(&self) -> usize {
        self.chunks[self.position..].iter().map(|c| c.len()).sum()
    }

    /// Whether all chunks have been read.
    pub fn is_exhausted(&self) -> bool {
        self.position >= self.chunks.len()
    }
}

// ---------------------------------------------------------------------------
// StringStream helpers
// ---------------------------------------------------------------------------

impl StringStream {
    /// Number of items remaining to be read.
    pub fn remaining(&self) -> usize {
        self.items.len().saturating_sub(self.position)
    }

    /// Create a StringStream by splitting a string on newlines.
    pub fn from_str(s: &str) -> Self {
        let items: Vec<String> = s.lines().map(|l| l.to_string()).collect();
        Self { items, position: 0 }
    }

    /// Whether all items have been read.
    pub fn is_exhausted(&self) -> bool {
        self.position >= self.items.len()
    }
}

// ---------------------------------------------------------------------------
// StringWriter helpers
// ---------------------------------------------------------------------------

impl StringWriter {
    /// Total character count across all written items.
    pub fn total_char_count(&self) -> usize {
        self.items.iter().map(|s| s.len()).sum()
    }

    /// Number of items written so far.
    pub fn item_count(&self) -> usize {
        self.items.len()
    }
}

impl fmt::Display for StringWriter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "StringWriter({} items, {} chars, ended={})",
            self.items.len(),
            self.total_char_count(),
            self.ended,
        )
    }
}

// ---------------------------------------------------------------------------
// BufferWriter helpers
// ---------------------------------------------------------------------------

impl BufferWriter {
    /// Reset the writer, clearing all chunks and the ended flag.
    pub fn clear(&mut self) {
        self.chunks.clear();
        self.ended = false;
    }
}

impl fmt::Display for BufferWriter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "BufferWriter({} chunks, {} bytes, ended={})",
            self.chunks.len(),
            self.total_size(),
            self.ended,
        )
    }
}

/// Collect all remaining items from a readable stream into a Vec.
pub fn collect_all_items<S: ReadableStream>(stream: &mut S) -> Vec<S::Item> {
    stream.collect_all()
}

// ---------------------------------------------------------------------------
// SkipStream adapter
// ---------------------------------------------------------------------------

/// A stream that skips the first `n` items, then yields the rest.
pub struct SkipStream<S> {
    inner: S,
    remaining_skip: usize,
}

impl<S> SkipStream<S> {
    pub fn new(inner: S, n: usize) -> Self {
        Self {
            inner,
            remaining_skip: n,
        }
    }
}

impl<S: ReadableStream> ReadableStream for SkipStream<S>
where
    Self: Send,
{
    type Item = S::Item;

    fn read(&mut self) -> Option<S::Item> {
        while self.remaining_skip > 0 {
            if self.inner.read().is_none() {
                self.remaining_skip = 0;
                return None;
            }
            self.remaining_skip -= 1;
        }
        self.inner.read()
    }
}

// ---------------------------------------------------------------------------
// InspectStream adapter
// ---------------------------------------------------------------------------

/// A stream adapter that calls a side-effect closure on each item before
/// yielding it, without modifying the item. Useful for logging/debugging.
pub struct InspectStream<S, F> {
    inner: S,
    inspector: F,
}

impl<S, F> InspectStream<S, F> {
    pub fn new(inner: S, inspector: F) -> Self {
        Self { inner, inspector }
    }
}

impl<S, F> ReadableStream for InspectStream<S, F>
where
    S: ReadableStream,
    F: FnMut(&S::Item),
    Self: Send,
{
    type Item = S::Item;

    fn read(&mut self) -> Option<S::Item> {
        self.inner.read().map(|item| {
            (self.inspector)(&item);
            item
        })
    }
}

// ---------------------------------------------------------------------------
// FlatMapStream adapter
// ---------------------------------------------------------------------------

/// A stream adapter that maps each item to a Vec and flattens the results.
pub struct FlatMapStream<S, F, T> {
    inner: S,
    mapper: F,
    buffer: std::collections::VecDeque<T>,
}

impl<S, F, T> FlatMapStream<S, F, T> {
    pub fn new(inner: S, mapper: F) -> Self {
        Self {
            inner,
            mapper,
            buffer: std::collections::VecDeque::new(),
        }
    }
}

impl<S, F, T> ReadableStream for FlatMapStream<S, F, T>
where
    S: ReadableStream,
    F: FnMut(S::Item) -> Vec<T>,
    Self: Send,
{
    type Item = T;

    fn read(&mut self) -> Option<T> {
        loop {
            if let Some(item) = self.buffer.pop_front() {
                return Some(item);
            }
            match self.inner.read() {
                Some(item) => {
                    self.buffer = (self.mapper)(item).into();
                }
                None => return None,
            }
        }
    }
}

// ---------------------------------------------------------------------------
// ZipStream adapter
// ---------------------------------------------------------------------------

/// A stream that yields paired items from two streams. Stops when either
/// stream is exhausted.
pub struct ZipStream<S1, S2> {
    first: S1,
    second: S2,
}

impl<S1, S2> ZipStream<S1, S2> {
    pub fn new(first: S1, second: S2) -> Self {
        Self { first, second }
    }
}

impl<S1, S2> ReadableStream for ZipStream<S1, S2>
where
    S1: ReadableStream,
    S2: ReadableStream,
    Self: Send,
{
    type Item = (S1::Item, S2::Item);

    fn read(&mut self) -> Option<(S1::Item, S2::Item)> {
        let a = self.first.read()?;
        let b = self.second.read()?;
        Some((a, b))
    }
}

// ---------------------------------------------------------------------------
// EnumerateStream adapter
// ---------------------------------------------------------------------------

/// A stream that yields `(index, item)` pairs, counting from zero.
pub struct EnumerateStream<S> {
    inner: S,
    index: usize,
}

impl<S> EnumerateStream<S> {
    pub fn new(inner: S) -> Self {
        Self { inner, index: 0 }
    }
}

impl<S: ReadableStream> ReadableStream for EnumerateStream<S>
where
    Self: Send,
{
    type Item = (usize, S::Item);

    fn read(&mut self) -> Option<(usize, S::Item)> {
        self.inner.read().map(|item| {
            let idx = self.index;
            self.index += 1;
            (idx, item)
        })
    }
}

// ---------------------------------------------------------------------------
// PeekableStream adapter
// ---------------------------------------------------------------------------

/// A stream wrapper that allows peeking at the next item without consuming it.
pub struct PeekableStream<S: ReadableStream> {
    inner: S,
    peeked: Option<Option<S::Item>>,
}

impl<S: ReadableStream> PeekableStream<S> {
    pub fn new(inner: S) -> Self {
        Self {
            inner,
            peeked: None,
        }
    }

    /// Peek at the next item without consuming it. Returns `None` if the
    /// stream is exhausted. Subsequent calls return the same reference until
    /// `read()` is called.
    pub fn peek(&mut self) -> Option<&S::Item> {
        if self.peeked.is_none() {
            self.peeked = Some(self.inner.read());
        }
        self.peeked.as_ref().and_then(|o| o.as_ref())
    }
}

impl<S: ReadableStream> ReadableStream for PeekableStream<S>
where
    Self: Send,
{
    type Item = S::Item;

    fn read(&mut self) -> Option<S::Item> {
        match self.peeked.take() {
            Some(item) => item,
            None => self.inner.read(),
        }
    }
}

// ---------------------------------------------------------------------------
// Display impls for adapters
// ---------------------------------------------------------------------------

impl<S> fmt::Display for TakeStream<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TakeStream(remaining={})", self.remaining)
    }
}

impl<S> fmt::Display for SkipStream<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SkipStream(remaining_skip={})", self.remaining_skip)
    }
}

impl<S> fmt::Display for EnumerateStream<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "EnumerateStream(index={})", self.index)
    }
}

// ---------------------------------------------------------------------------
// ScanStream adapter
// ---------------------------------------------------------------------------

/// A stream adapter that maintains an accumulator and yields the running
/// result after processing each item (similar to `Iterator::scan`).
pub struct ScanStream<S, B, F> {
    inner: S,
    state: B,
    folder: F,
}

impl<S, B, F> ScanStream<S, B, F> {
    pub fn new(inner: S, initial: B, folder: F) -> Self {
        Self {
            inner,
            state: initial,
            folder,
        }
    }
}

impl<S, B, F> ReadableStream for ScanStream<S, B, F>
where
    S: ReadableStream,
    B: Clone + Send,
    F: FnMut(&mut B, S::Item) + Send,
    Self: Send,
{
    type Item = B;

    fn read(&mut self) -> Option<B> {
        let item = self.inner.read()?;
        (self.folder)(&mut self.state, item);
        Some(self.state.clone())
    }
}

// ---------------------------------------------------------------------------
// DeduplicateStream adapter
// ---------------------------------------------------------------------------

/// A stream that suppresses consecutive duplicate items.
pub struct DeduplicateStream<S: ReadableStream> {
    inner: S,
    last: Option<S::Item>,
}

impl<S: ReadableStream> DeduplicateStream<S> {
    pub fn new(inner: S) -> Self {
        Self { inner, last: None }
    }
}

impl<S> ReadableStream for DeduplicateStream<S>
where
    S: ReadableStream,
    S::Item: PartialEq + Clone,
    Self: Send,
{
    type Item = S::Item;

    fn read(&mut self) -> Option<S::Item> {
        loop {
            let item = self.inner.read()?;
            if self.last.as_ref() == Some(&item) {
                continue;
            }
            self.last = Some(item.clone());
            return Some(item);
        }
    }
}

// ---------------------------------------------------------------------------
// BufferStream – sliced reading
// ---------------------------------------------------------------------------

impl BufferStream {
    /// Read a contiguous byte range across all chunks. Returns `None` if the
    /// range exceeds the total remaining data. Does **not** advance the read
    /// position.
    pub fn slice_bytes(&self, start: usize, len: usize) -> Option<VsBuffer> {
        let remaining = &self.chunks[self.position..];
        let total: usize = remaining.iter().map(|c| c.len()).sum();
        if start + len > total {
            return None;
        }

        let mut collected = Vec::with_capacity(len);
        let mut offset = 0;
        for chunk in remaining {
            let chunk_end = offset + chunk.len();
            if chunk_end <= start {
                offset = chunk_end;
                continue;
            }
            let local_start = start.saturating_sub(offset);
            let local_end = (start + len - offset).min(chunk.len());
            collected.extend_from_slice(&chunk.as_bytes()[local_start..local_end]);
            if collected.len() >= len {
                break;
            }
            offset = chunk_end;
        }
        Some(VsBuffer::new(collected))
    }

    /// Reset the stream position so it can be read again from the beginning.
    pub fn reset(&mut self) {
        self.position = 0;
    }

    /// Total number of chunks (read and unread).
    pub fn total_chunks(&self) -> usize {
        self.chunks.len()
    }
}

// ---------------------------------------------------------------------------
// StringStream – additional constructors
// ---------------------------------------------------------------------------

impl StringStream {
    /// Create a `StringStream` by splitting a string on a given delimiter.
    pub fn split(s: &str, delimiter: char) -> Self {
        let items: Vec<String> = s.split(delimiter).map(|p| p.to_string()).collect();
        Self { items, position: 0 }
    }

    /// Reset the stream position so it can be read again from the beginning.
    pub fn reset(&mut self) {
        self.position = 0;
    }
}

// ---------------------------------------------------------------------------
// Free-function utilities
// ---------------------------------------------------------------------------

/// Fold all items in a readable stream into a single accumulator value.
pub fn fold<S, B, F>(stream: &mut S, init: B, mut f: F) -> B
where
    S: ReadableStream,
    F: FnMut(B, S::Item) -> B,
{
    let mut acc = init;
    while let Some(item) = stream.read() {
        acc = f(acc, item);
    }
    acc
}

/// Check whether any item in the stream satisfies a predicate.
/// Short-circuits on the first match.
pub fn any_match<S, F>(stream: &mut S, mut predicate: F) -> bool
where
    S: ReadableStream,
    F: FnMut(&S::Item) -> bool,
{
    while let Some(item) = stream.read() {
        if predicate(&item) {
            return true;
        }
    }
    false
}

/// Check whether every item in the stream satisfies a predicate.
/// Short-circuits on the first failure. Returns `true` for an empty stream.
pub fn all_match<S, F>(stream: &mut S, mut predicate: F) -> bool
where
    S: ReadableStream,
    F: FnMut(&S::Item) -> bool,
{
    while let Some(item) = stream.read() {
        if !predicate(&item) {
            return false;
        }
    }
    true
}

/// Find the first item satisfying a predicate, if any.
pub fn find_first<S, F>(stream: &mut S, mut predicate: F) -> Option<S::Item>
where
    S: ReadableStream,
    F: FnMut(&S::Item) -> bool,
{
    while let Some(item) = stream.read() {
        if predicate(&item) {
            return Some(item);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// StreamSplitter – splits byte data on a delimiter
// ---------------------------------------------------------------------------

/// Splits byte data on a single-byte delimiter, yielding complete chunks.
///
/// Data is fed incrementally via [`feed`](StreamSplitter::feed) and complete
/// chunks (everything between delimiters) are retrieved one at a time via
/// [`next_chunk`](StreamSplitter::next_chunk).
pub struct StreamSplitter {
    delimiter: u8,
    buffer: Vec<u8>,
    chunks: std::collections::VecDeque<Vec<u8>>,
}

impl StreamSplitter {
    /// Create a splitter that splits on the given byte delimiter.
    pub fn new(delimiter: u8) -> Self {
        Self {
            delimiter,
            buffer: Vec::new(),
            chunks: std::collections::VecDeque::new(),
        }
    }

    /// Convenience constructor that splits on `\n`.
    pub fn line_splitter() -> Self {
        Self::new(b'\n')
    }

    /// Feed additional data into the splitter.
    ///
    /// Any complete chunks (terminated by the delimiter) become available
    /// through [`next_chunk`](StreamSplitter::next_chunk). Bytes after the
    /// last delimiter are retained internally as pending data.
    pub fn feed(&mut self, data: &[u8]) {
        for &byte in data {
            if byte == self.delimiter {
                let chunk = std::mem::take(&mut self.buffer);
                self.chunks.push_back(chunk);
            } else {
                self.buffer.push(byte);
            }
        }
    }

    /// Return the next complete chunk, or `None` if no complete chunk is ready.
    pub fn next_chunk(&mut self) -> Option<Vec<u8>> {
        self.chunks.pop_front()
    }

    /// Number of bytes currently buffered but not yet part of a complete chunk.
    pub fn pending_bytes(&self) -> usize {
        self.buffer.len()
    }
}

impl fmt::Display for StreamSplitter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "StreamSplitter(delimiter=0x{:02X}, ready={}, pending={})",
            self.delimiter,
            self.chunks.len(),
            self.buffer.len(),
        )
    }
}

impl Default for StreamSplitter {
    fn default() -> Self {
        Self::line_splitter()
    }
}

// ---------------------------------------------------------------------------
// ByteRingBuffer – fixed-capacity byte buffer with backpressure
// ---------------------------------------------------------------------------

/// A fixed-capacity byte buffer that provides backpressure by limiting writes
/// to the available space.
///
/// Internally uses a ring buffer so that reads free space for future writes
/// without copying data on every operation.
pub struct ByteRingBuffer {
    buf: Vec<u8>,
    capacity: usize,
    head: usize,
    len: usize,
}

impl ByteRingBuffer {
    /// Create a buffer with the given maximum capacity in bytes.
    pub fn new(capacity: usize) -> Self {
        Self {
            buf: vec![0u8; capacity],
            capacity,
            head: 0,
            len: 0,
        }
    }

    /// Write as many bytes from `data` as fit into the buffer.
    ///
    /// Returns the number of bytes actually written. When the buffer is full
    /// zero is returned, providing backpressure to the producer.
    pub fn write(&mut self, data: &[u8]) -> usize {
        let writable = data.len().min(self.available_space());
        for &b in &data[..writable] {
            let idx = (self.head + self.len) % self.capacity;
            self.buf[idx] = b;
            self.len += 1;
        }
        writable
    }

    /// Read up to `buf.len()` bytes from the buffer into `buf`.
    ///
    /// Returns the number of bytes actually read.
    pub fn read(&mut self, buf: &mut [u8]) -> usize {
        let readable = buf.len().min(self.len);
        for slot in buf.iter_mut().take(readable) {
            *slot = self.buf[self.head];
            self.head = (self.head + 1) % self.capacity;
            self.len -= 1;
        }
        readable
    }

    /// Number of bytes currently stored.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether the buffer has reached its capacity.
    pub fn is_full(&self) -> bool {
        self.len == self.capacity
    }

    /// Whether the buffer contains no data.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Number of additional bytes that can be written before the buffer is full.
    pub fn available_space(&self) -> usize {
        self.capacity - self.len
    }
}

impl fmt::Display for ByteRingBuffer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ByteRingBuffer(used={}, capacity={})",
            self.len, self.capacity,
        )
    }
}

// ---------------------------------------------------------------------------
// StreamTee – duplicates data to multiple output buffers
// ---------------------------------------------------------------------------

/// Duplicates incoming data to multiple independent output buffers.
///
/// Each output is an index-addressed `Vec<u8>` that accumulates bytes written
/// through [`write`](StreamTee::write) and can be drained independently via
/// [`read_output`](StreamTee::read_output).
pub struct StreamTee {
    outputs: Vec<Vec<u8>>,
}

impl StreamTee {
    /// Create a new tee with no outputs.
    pub fn new() -> Self {
        Self {
            outputs: Vec::new(),
        }
    }

    /// Register a new output buffer and return its index.
    pub fn add_output(&mut self) -> usize {
        let idx = self.outputs.len();
        self.outputs.push(Vec::new());
        idx
    }

    /// Write `data` to every registered output buffer.
    pub fn write(&mut self, data: &[u8]) {
        for output in &mut self.outputs {
            output.extend_from_slice(data);
        }
    }

    /// Drain and return all buffered bytes for the output at `index`.
    ///
    /// Returns an empty `Vec` if the index is out of range.
    pub fn read_output(&mut self, index: usize) -> Vec<u8> {
        match self.outputs.get_mut(index) {
            Some(buf) => std::mem::take(buf),
            None => Vec::new(),
        }
    }

    /// Number of registered output buffers.
    pub fn output_count(&self) -> usize {
        self.outputs.len()
    }
}

impl Default for StreamTee {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for StreamTee {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let total: usize = self.outputs.iter().map(|o| o.len()).sum();
        write!(
            f,
            "StreamTee(outputs={}, buffered_bytes={})",
            self.outputs.len(),
            total,
        )
    }
}

// ---------------------------------------------------------------------------
// StreamProgressReporter – tracks byte-level streaming progress
// ---------------------------------------------------------------------------

/// Tracks progress of a streaming operation with a known total size.
pub struct StreamProgressReporter {
    total_bytes: u64,
    processed: u64,
}

impl StreamProgressReporter {
    /// Create a reporter for an operation that will process `total_bytes`.
    pub fn new(total_bytes: u64) -> Self {
        Self {
            total_bytes,
            processed: 0,
        }
    }

    /// Record that `bytes` more bytes have been processed.
    pub fn record(&mut self, bytes: u64) {
        self.processed = self.processed.saturating_add(bytes);
        if self.processed > self.total_bytes {
            self.processed = self.total_bytes;
        }
    }

    /// Total bytes processed so far.
    pub fn bytes_processed(&self) -> u64 {
        self.processed
    }

    /// Completion percentage in the range `0.0..=100.0`.
    pub fn percentage(&self) -> f64 {
        if self.total_bytes == 0 {
            return 100.0;
        }
        (self.processed as f64 / self.total_bytes as f64) * 100.0
    }

    /// Whether the operation is considered complete.
    pub fn is_complete(&self) -> bool {
        self.processed >= self.total_bytes
    }

    /// Bytes remaining to be processed.
    pub fn remaining_bytes(&self) -> u64 {
        self.total_bytes.saturating_sub(self.processed)
    }
}

impl fmt::Display for StreamProgressReporter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "StreamProgressReporter({}/{} bytes, {:.1}%)",
            self.processed,
            self.total_bytes,
            self.percentage(),
        )
    }
}


// ─── StmBuf Ring Buffer ──────────────────────────────────────

/// A fixed-capacity ring buffer for stream chunks.
#[derive(Debug, Clone)]
pub struct StmBufRingBuffer<T> {
    buf: Vec<Option<T>>,
    head: usize,
    len: usize,
}

impl<T: Clone> StmBufRingBuffer<T> {
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

impl<T: Clone + fmt::Display> fmt::Display for StmBufRingBuffer<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "StmBufRingBuffer(len={}, cap={})", self.len, self.capacity())
    }
}

// ─── StmBld Builder & Validator ─────────────────────────────

/// Builder for constructing stream configurations.
#[derive(Debug, Clone)]
pub struct StmBldBuilder {
    name: String,
    properties: std::collections::HashMap<String, String>,
    tags: Vec<String>,
    enabled: bool,
    priority: i32,
    max_items: usize,
}

impl StmBldBuilder {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(), properties: std::collections::HashMap::new(),
            tags: Vec::new(), enabled: true, priority: 0, max_items: 100,
        }
    }

    pub fn property(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.properties.insert(key.into(), value.into()); self
    }
    pub fn tag(mut self, tag: impl Into<String>) -> Self { self.tags.push(tag.into()); self }
    pub fn enabled(mut self, enabled: bool) -> Self { self.enabled = enabled; self }
    pub fn priority(mut self, priority: i32) -> Self { self.priority = priority; self }
    pub fn max_items(mut self, max: usize) -> Self { self.max_items = max; self }

    pub fn build(self) -> Result<StmBldCfg, StmBldBuildErr> {
        let mut errors = Vec::new();
        if self.name.is_empty() { errors.push("name must not be empty".into()); }
        if self.max_items == 0 { errors.push("max_items must be > 0".into()); }
        if self.priority < -100 || self.priority > 100 {
            errors.push(format!("priority {} out of range [-100, 100]", self.priority));
        }
        if !errors.is_empty() { return Err(StmBldBuildErr { errors }); }
        Ok(StmBldCfg {
            name: self.name, properties: self.properties, tags: self.tags,
            enabled: self.enabled, priority: self.priority, max_items: self.max_items,
        })
    }
}

/// Validated stream configuration.
#[derive(Debug, Clone)]
pub struct StmBldCfg {
    pub name: String,
    pub properties: std::collections::HashMap<String, String>,
    pub tags: Vec<String>,
    pub enabled: bool,
    pub priority: i32,
    pub max_items: usize,
}

impl StmBldCfg {
    pub fn has_tag(&self, tag: &str) -> bool { self.tags.iter().any(|t| t == tag) }
    pub fn get_property(&self, key: &str) -> Option<&str> {
        self.properties.get(key).map(|s| s.as_str())
    }
    pub fn property_count(&self) -> usize { self.properties.len() }
    pub fn merge_properties(&mut self, other: &StmBldCfg) {
        for (k, v) in &other.properties { self.properties.insert(k.clone(), v.clone()); }
    }
}

impl fmt::Display for StmBldCfg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "StmBldCfg({}, enabled={}, priority={}, tags={})",
            self.name, self.enabled, self.priority, self.tags.len())
    }
}

#[derive(Debug, Clone)]
pub struct StmBldBuildErr { pub errors: Vec<String> }

impl fmt::Display for StmBldBuildErr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "StmBldBuildErr: {}", self.errors.join("; "))
    }
}
impl std::error::Error for StmBldBuildErr {}


/// Data stream configuration manager.
#[derive(Debug, Clone)]
pub struct StreamConfig {
    entries: Vec<StreamEntry>,
    enabled: bool,
    max_entries: usize,
}

/// A single data stream entry.
#[derive(Debug, Clone, PartialEq)]
pub struct StreamEntry {
    pub id: String,
    pub label: String,
    pub priority: i32,
    pub active: bool,
    pub metadata: Vec<(String, String)>,
}

impl StreamEntry {
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

impl StreamConfig {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            enabled: true,
            max_entries,
        }
    }

    pub fn add(&mut self, entry: StreamEntry) -> bool {
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

    pub fn get(&self, id: &str) -> Option<&StreamEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut StreamEntry> {
        self.entries.iter_mut().find(|e| e.id == id)
    }

    pub fn active_entries(&self) -> Vec<&StreamEntry> {
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

    pub fn top_n(&self, n: usize) -> Vec<&StreamEntry> {
        self.entries.iter().take(n).collect()
    }

    pub fn find_by_label(&self, label: &str) -> Option<&StreamEntry> {
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

    pub fn drain_inactive(&mut self) -> Vec<StreamEntry> {
        let (inactive, active): (Vec<_>, Vec<_>) =
            self.entries.drain(..).partition(|e| !e.active);
        self.entries = active;
        inactive
    }
}


// ---------------------------------------------------------------------------
// Async byte and line streaming — extended utilities (yv)
// ---------------------------------------------------------------------------

/// Metric accumulator for stream operations.
#[derive(Debug, Clone)]
pub struct YvMetrics {
    samples: Vec<f64>,
    label: String,
}

impl YvMetrics {
    pub fn new(label: &str) -> Self {
        Self { samples: Vec::new(), label: label.to_string() }
    }

    pub fn record(&mut self, value: f64) {
        self.samples.push(value);
    }

    pub fn mean(&self) -> f64 {
        if self.samples.is_empty() { return 0.0; }
        self.samples.iter().sum::<f64>() / self.samples.len() as f64
    }

    pub fn max_val(&self) -> f64 {
        self.samples.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
    }

    pub fn min_val(&self) -> f64 {
        self.samples.iter().cloned().fold(f64::INFINITY, f64::min)
    }

    pub fn count(&self) -> usize {
        self.samples.len()
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn reset(&mut self) {
        self.samples.clear();
    }

    pub fn variance(&self) -> f64 {
        if self.samples.len() < 2 { return 0.0; }
        let m = self.mean();
        let sq: f64 = self.samples.iter().map(|v| (v - m).powi(2)).sum();
        sq / (self.samples.len() as f64 - 1.0)
    }

    pub fn std_dev(&self) -> f64 {
        self.variance().sqrt()
    }

    pub fn percentile(&self, p: f64) -> f64 {
        if self.samples.is_empty() { return 0.0; }
        let mut sorted = self.samples.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let idx = ((p / 100.0) * (sorted.len() as f64 - 1.0)).round() as usize;
        sorted[idx.min(sorted.len() - 1)]
    }

    pub fn sum(&self) -> f64 {
        self.samples.iter().sum()
    }

    pub fn merge(&mut self, other: &Self) {
        self.samples.extend_from_slice(&other.samples);
    }
}

/// Sliding-window rate counter for stream.
#[derive(Debug, Clone)]
pub struct YvRateWindow {
    timestamps: Vec<u64>,
    window_ms: u64,
}

impl YvRateWindow {
    pub fn new(window_ms: u64) -> Self {
        Self { timestamps: Vec::new(), window_ms }
    }

    pub fn tick(&mut self, now_ms: u64) {
        self.timestamps.push(now_ms);
        self.prune(now_ms);
    }

    fn prune(&mut self, now_ms: u64) {
        let cutoff = now_ms.saturating_sub(self.window_ms);
        self.timestamps.retain(|&t| t >= cutoff);
    }

    pub fn rate(&mut self, now_ms: u64) -> usize {
        self.prune(now_ms);
        self.timestamps.len()
    }

    pub fn clear(&mut self) {
        self.timestamps.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.timestamps.is_empty()
    }

    pub fn window_ms(&self) -> u64 {
        self.window_ms
    }
}

/// A small LRU-style cache for stream lookups.
#[derive(Debug, Clone)]
pub struct YvLruCache {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl YvLruCache {
    pub fn new(capacity: usize) -> Self {
        Self { entries: Vec::new(), capacity }
    }

    pub fn get(&mut self, key: &str) -> Option<String> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            let entry = self.entries.remove(pos);
            let val = entry.1.clone();
            self.entries.push(entry);
            Some(val)
        } else {
            None
        }
    }

    pub fn put(&mut self, key: String, value: String) {
        self.entries.retain(|(k, _)| k != &key);
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value));
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.entries.iter().any(|(k, _)| k == key)
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    pub fn remove(&mut self, key: &str) -> Option<String> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            Some(self.entries.remove(pos).1)
        } else {
            None
        }
    }
}


// ---------------------------------------------------------------------------
// xa_ extended helpers for stream
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaStreamRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaStreamRingBuf {
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
pub struct XaStreamCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaStreamCounter {
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

impl Default for XaStreamCounter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 167
// ---------------------------------------------------------------------------

/// Generic object pool `Xc167Pool<T>`.
pub struct Xc167Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc167Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc167PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc167Pool<T> {
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
    pub fn stats(&self) -> Xc167PoolStats {
        Xc167PoolStats {
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

impl<T> Default for Xc167Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc167Scheduler`.
pub struct Xc167Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc167Scheduler {
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

impl Default for Xc167Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_167 hash for the given byte slice.
pub fn xc_167_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_167 convention.
pub fn xc_167_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_107 deepening: state machine + event bus ---

/// States for the Xd107 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd107State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd107State {
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
pub struct Xd107Transition {
    pub from: Xd107State,
    pub to: Xd107State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd107StateMachine {
    current: Xd107State,
    history: Vec<Xd107Transition>,
    step_counter: usize,
}

impl Xd107StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd107State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd107State {
        self.current
    }

    pub fn history(&self) -> &[Xd107Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd107State) -> Result<Xd107State, String> {
        let allowed = match (self.current, target) {
            (Xd107State::Idle, Xd107State::Running) => true,
            (Xd107State::Running, Xd107State::Paused) => true,
            (Xd107State::Running, Xd107State::Done) => true,
            (Xd107State::Paused, Xd107State::Running) => true,
            (Xd107State::Paused, Xd107State::Done) => true,
            (Xd107State::Done, Xd107State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_107: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd107Transition {
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
            "Xd107SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd107State> {
        let prefix = "Xd107SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd107State::Idle),
            "Running" => Some(Xd107State::Running),
            "Paused" => Some(Xd107State::Paused),
            "Done" => Some(Xd107State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd107State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd107 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd107Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd107Event {
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

type Xd107HandlerFn = Box<dyn Fn(&Xd107Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd107EventBus {
    handlers: Vec<(usize, Option<String>, Xd107HandlerFn)>,
    next_id: usize,
    published: Vec<Xd107Event>,
}

impl Xd107EventBus {
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
        F: Fn(&Xd107Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd107Event) + Send + Sync + 'static,
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
    pub fn publish(&mut self, event: Xd107Event) {
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

    pub fn published_events(&self) -> &[Xd107Event] {
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
// xg_31: Directed graph
// ---------------------------------------------------------------------------

/// A directed graph with adjacency-list representation.
#[derive(Debug, Clone)]
pub struct Xg31Graph {
    adj: std::collections::HashMap<usize, Vec<usize>>,
    edge_cnt: usize,
}

impl Xg31Graph {
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

impl Default for Xg31Graph {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// xg_31: Min-heap
// ---------------------------------------------------------------------------

/// A min-heap backed by a `Vec`.
#[derive(Debug, Clone)]
pub struct Xg31Heap<T: Ord> {
    data: Vec<T>,
}

impl<T: Ord> Xg31Heap<T> {
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
    pub fn merge(&mut self, other: &mut Xg31Heap<T>) {
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

impl<T: Ord> Default for Xg31Heap<T> {
    fn default() -> Self { Self::new() }
}


/// A probabilistic sorted list using a skip-list structure (variant 166).
pub struct Xh166SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh166SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 208 as u64,
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

/// A compact bit set supporting boolean operations (variant 166).
pub struct Xh166BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh166BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 166).
pub struct Xi166Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi166Deque<T> {
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
pub struct Xi166Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi166Interval {
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

/// A simple interval tree (variant 166).
pub struct Xi166IntervalTree {
    xi_intervals: Vec<Xi166Interval>,
}

impl Xi166IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi166Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi166Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi166Interval) -> Vec<&Xi166Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi166Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi166Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi166Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi166Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi166Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi166Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 166) ---

/// Disjoint set / union-find for crate 166.
pub struct Xj166UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj166UnionFind {
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

const XJ166_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 166.
pub struct Xj166BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj166BTreeNode<K, V>>>,
    len: usize,
}

struct Xj166BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj166BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj166BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ166_BTREE_ORDER - 1
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
        let mid = XJ166_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj166BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj166BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj166BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj166BTreeNode::xj_new_leaf();
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


// --- xk_166 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk166SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk166SegmentTree {
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
pub struct Xk166DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk166DisjointIntervals {
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

    #[test]
    fn map_stream_transforms_items() {
        let inner = StringStream::from_strings(vec![
            "hello".into(),
            "world".into(),
        ]);
        let mut mapped = MapStream::new(inner, |s: String| s.len());
        assert_eq!(mapped.read(), Some(5));
        assert_eq!(mapped.read(), Some(5));
        assert!(mapped.read().is_none());
    }

    #[test]
    fn map_stream_empty() {
        let inner = StringStream::from_strings(vec![]);
        let mut mapped = MapStream::new(inner, |s: String| s.to_uppercase());
        assert!(mapped.read().is_none());
    }

    #[test]
    fn chain_stream_concatenates() {
        let first = StringStream::from_strings(vec!["a".into(), "b".into()]);
        let second = StringStream::from_strings(vec!["c".into(), "d".into()]);
        let mut chained = ChainStream::new(first, second);
        let all = chained.collect_all();
        assert_eq!(all, vec!["a", "b", "c", "d"]);
    }

    #[test]
    fn chain_stream_first_empty() {
        let first = StringStream::from_strings(vec![]);
        let second = StringStream::from_strings(vec!["x".into()]);
        let mut chained = ChainStream::new(first, second);
        assert_eq!(chained.read(), Some("x".to_string()));
        assert!(chained.read().is_none());
    }

    #[test]
    fn chain_stream_second_empty() {
        let first = StringStream::from_strings(vec!["y".into()]);
        let second = StringStream::from_strings(vec![]);
        let mut chained = ChainStream::new(first, second);
        assert_eq!(chained.read(), Some("y".to_string()));
        assert!(chained.read().is_none());
    }

    #[test]
    fn take_stream_limits_output() {
        let inner = StringStream::from_strings(vec![
            "a".into(), "b".into(), "c".into(), "d".into(),
        ]);
        let mut taken = TakeStream::new(inner, 2);
        assert_eq!(taken.read(), Some("a".to_string()));
        assert_eq!(taken.read(), Some("b".to_string()));
        assert!(taken.read().is_none());
    }

    #[test]
    fn take_stream_zero_limit() {
        let inner = StringStream::from_strings(vec!["a".into()]);
        let mut taken = TakeStream::new(inner, 0);
        assert!(taken.read().is_none());
    }

    #[test]
    fn string_writer_basic() {
        let mut writer = StringWriter::new();
        writer.write("hello ".into());
        writer.write("world".into());
        assert_eq!(writer.items().len(), 2);
        assert!(!writer.is_ended());
        writer.end();
        assert!(writer.is_ended());
        // Writes after end are ignored
        writer.write("ignored".into());
        assert_eq!(writer.items().len(), 2);
    }

    #[test]
    fn string_writer_into_string() {
        let mut writer = StringWriter::new();
        writer.write("foo".into());
        writer.write("bar".into());
        assert_eq!(writer.into_string(), "foobar");
    }

    #[test]
    fn pipe_string_streams() {
        let mut source = StringStream::from_strings(vec!["x".into(), "y".into()]);
        let mut dest = StringWriter::new();
        pipe(&mut source, &mut dest);
        assert!(dest.is_ended());
        assert_eq!(dest.into_string(), "xy");
    }

    #[test]
    fn buffer_stream_empty_consume() {
        let stream = BufferStream::empty();
        let result = stream.consume();
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn stream_stats_new_defaults() {
        let stats = StreamStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn stream_stats_record_success() {
        let mut stats = StreamStats::new();
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
    fn stream_stats_record_failure() {
        let mut stats = StreamStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn stream_stats_reset() {
        let mut stats = StreamStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn stream_stats_merge() {
        let mut a = StreamStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = StreamStats::new();
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
    fn stream_stats_display() {
        let mut stats = StreamStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn stream_stats_default() {
        let stats = StreamStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn stream_validator_accepts_valid_name() {
        let v = StreamValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn stream_validator_rejects_empty() {
        let v = StreamValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn stream_validator_rejects_too_long() {
        let v = StreamValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn stream_validator_forbidden_prefix() {
        let v = StreamValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn stream_validator_allowed_chars() {
        let v = StreamValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn stream_validator_range() {
        let v = StreamValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn stream_sanitize_removes_control() {
        let result = StreamValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn stream_truncate_short_string() {
        assert_eq!(StreamValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn stream_truncate_long_string() {
        let result = StreamValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn stream_is_ascii_printable() {
        assert!(StreamValidator::is_ascii_printable("Hello World 123"));
        assert!(!StreamValidator::is_ascii_printable("Hello\x00World"));
    }

    #[test]
    fn stream_buffer_push_and_concat() {
        let mut buf = StreamBuffer::new();
        buf.push("hello ");
        buf.push("world");
        assert_eq!(buf.concat(), "hello world");
        assert_eq!(buf.chunk_count(), 2);
        assert_eq!(buf.total_bytes(), 11);
    }

    #[test]
    fn stream_buffer_drain() {
        let mut buf = StreamBuffer::new();
        buf.push("a");
        buf.push("b");
        let drained = buf.drain();
        assert_eq!(drained, vec!["a", "b"]);
        assert!(buf.is_empty());
        assert_eq!(buf.total_bytes(), 0);
    }

    #[test]
    fn stream_buffer_display() {
        let buf = StreamBuffer::new();
        assert_eq!(buf.to_string(), "StreamBuffer(0 chunks, 0 bytes)");
    }

    #[test]
    fn stream_transform_single() {
        let t = StreamTransform::new()
            .pipe(|s| s.to_uppercase());
        assert_eq!(t.apply("hello"), "HELLO");
    }

    #[test]
    fn stream_transform_chained() {
        let t = StreamTransform::new()
            .pipe(|s| s.trim().to_string())
            .pipe(|s| s.to_uppercase());
        assert_eq!(t.apply("  hello  "), "HELLO");
        assert_eq!(t.step_count(), 2);
    }

    #[test]
    fn stream_transform_apply_all() {
        let t = StreamTransform::new()
            .pipe(|s| s.to_uppercase());
        let result = t.apply_all(&["hello".into(), "world".into()]);
        assert_eq!(result, vec!["HELLO", "WORLD"]);
    }

    #[test]
    fn stream_lines_basic() {
        let chunks = vec!["hello\nworld".to_string()];
        let lines = stream_lines(&chunks);
        assert_eq!(lines, vec!["hello", "world"]);
    }

    #[test]
    fn stream_lines_with_pending_partial() {
        let chunks = vec!["line1\nline2\npartial".to_string()];
        let (complete, pending) = stream_lines_with_pending(&chunks);
        assert_eq!(complete, vec!["line1", "line2"]);
        assert_eq!(pending, Some("partial".to_string()));
    }

    // -- BufferStream helpers ----------------------------------------------

    #[test]
    fn buffer_stream_total_remaining_bytes() {
        let mut stream = BufferStream::from_chunks(vec![
            VsBuffer::from_string("abc"),
            VsBuffer::from_string("de"),
        ]);
        assert_eq!(stream.total_remaining_bytes(), 5);
        stream.read();
        assert_eq!(stream.total_remaining_bytes(), 2);
        stream.read();
        assert_eq!(stream.total_remaining_bytes(), 0);
        assert!(stream.is_exhausted());
    }

    // -- StringStream helpers ----------------------------------------------

    #[test]
    fn string_stream_remaining() {
        let mut stream = StringStream::from_strings(vec!["a".into(), "b".into(), "c".into()]);
        assert_eq!(stream.remaining(), 3);
        stream.read();
        assert_eq!(stream.remaining(), 2);
    }

    #[test]
    fn string_stream_from_str() {
        let mut stream = StringStream::from_str("hello\nworld\nfoo");
        assert_eq!(stream.remaining(), 3);
        assert_eq!(stream.read().unwrap(), "hello");
        assert_eq!(stream.read().unwrap(), "world");
        assert_eq!(stream.read().unwrap(), "foo");
        assert!(stream.is_exhausted());
    }

    // -- StringWriter helpers ----------------------------------------------

    #[test]
    fn string_writer_total_char_count() {
        let mut w = StringWriter::new();
        w.write("hello".into());
        w.write("world".into());
        assert_eq!(w.total_char_count(), 10);
        assert_eq!(w.item_count(), 2);
    }

    #[test]
    fn string_writer_display() {
        let mut w = StringWriter::new();
        w.write("test".into());
        let s = format!("{}", w);
        assert!(s.contains("1 items"));
        assert!(s.contains("4 chars"));
    }

    // -- BufferWriter helpers ----------------------------------------------

    #[test]
    fn buffer_writer_clear() {
        let mut w = BufferWriter::new();
        w.write(VsBuffer::from_string("data"));
        w.end();
        assert!(w.is_ended());
        assert_eq!(w.chunk_count(), 1);
        w.clear();
        assert!(!w.is_ended());
        assert_eq!(w.chunk_count(), 0);
    }

    #[test]
    fn buffer_writer_display() {
        let mut w = BufferWriter::new();
        w.write(VsBuffer::from_string("abc"));
        let s = format!("{}", w);
        assert!(s.contains("1 chunks"));
        assert!(s.contains("3 bytes"));
    }

    // -- collect_all_items -------------------------------------------------

    #[test]
    fn collect_all_items_works() {
        let mut stream = StringStream::from_strings(vec!["x".into(), "y".into()]);
        let items = collect_all_items(&mut stream);
        assert_eq!(items, vec!["x", "y"]);
    }

    // -- SkipStream --------------------------------------------------------

    #[test]
    fn skip_stream_skips_first_n() {
        let inner = StringStream::from_strings(vec![
            "a".into(), "b".into(), "c".into(), "d".into(),
        ]);
        let mut skipped = SkipStream::new(inner, 2);
        assert_eq!(skipped.read(), Some("c".to_string()));
        assert_eq!(skipped.read(), Some("d".to_string()));
        assert!(skipped.read().is_none());
    }

    #[test]
    fn skip_stream_skip_more_than_available() {
        let inner = StringStream::from_strings(vec!["a".into()]);
        let mut skipped = SkipStream::new(inner, 5);
        assert!(skipped.read().is_none());
    }

    #[test]
    fn skip_stream_skip_zero() {
        let inner = StringStream::from_strings(vec!["a".into(), "b".into()]);
        let mut skipped = SkipStream::new(inner, 0);
        assert_eq!(skipped.collect_all(), vec!["a", "b"]);
    }

    // -- InspectStream -----------------------------------------------------

    #[test]
    fn inspect_stream_observes_without_modifying() {
        let inner = StringStream::from_strings(vec!["x".into(), "y".into()]);
        // InspectStream should yield items unchanged; the inspector is a no-op here.
        let mut inspected = InspectStream::new(inner, |_item: &String| {});
        assert_eq!(inspected.read(), Some("x".to_string()));
        assert_eq!(inspected.read(), Some("y".to_string()));
        assert!(inspected.read().is_none());
    }

    // -- ZipStream ---------------------------------------------------------

    #[test]
    fn zip_stream_pairs_items() {
        let s1 = StringStream::from_strings(vec!["a".into(), "b".into(), "c".into()]);
        let s2 = StringStream::from_strings(vec!["1".into(), "2".into()]);
        let mut zipped = ZipStream::new(s1, s2);
        assert_eq!(
            zipped.read(),
            Some(("a".to_string(), "1".to_string()))
        );
        assert_eq!(
            zipped.read(),
            Some(("b".to_string(), "2".to_string()))
        );
        // s2 exhausted, so zip stops
        assert!(zipped.read().is_none());
    }

    // -- EnumerateStream ---------------------------------------------------

    #[test]
    fn enumerate_stream_adds_indices() {
        let inner = StringStream::from_strings(vec!["x".into(), "y".into(), "z".into()]);
        let mut enumerated = EnumerateStream::new(inner);
        assert_eq!(enumerated.read(), Some((0, "x".to_string())));
        assert_eq!(enumerated.read(), Some((1, "y".to_string())));
        assert_eq!(enumerated.read(), Some((2, "z".to_string())));
        assert!(enumerated.read().is_none());
    }

    // -- PeekableStream ----------------------------------------------------

    #[test]
    fn peekable_stream_peek_does_not_consume() {
        let inner = StringStream::from_strings(vec!["a".into(), "b".into()]);
        let mut peekable = PeekableStream::new(inner);
        assert_eq!(peekable.peek(), Some(&"a".to_string()));
        assert_eq!(peekable.peek(), Some(&"a".to_string())); // idempotent
        assert_eq!(peekable.read(), Some("a".to_string()));
        assert_eq!(peekable.read(), Some("b".to_string()));
        assert!(peekable.peek().is_none());
        assert!(peekable.read().is_none());
    }

    // -- Display impls -----------------------------------------------------

    #[test]
    fn adapter_display_impls() {
        let take = TakeStream::new(StringStream::from_strings(vec![]), 5);
        assert_eq!(take.to_string(), "TakeStream(remaining=5)");

        let skip = SkipStream::new(StringStream::from_strings(vec![]), 3);
        assert_eq!(skip.to_string(), "SkipStream(remaining_skip=3)");

        let en = EnumerateStream::new(StringStream::from_strings(vec![]));
        assert_eq!(en.to_string(), "EnumerateStream(index=0)");
    }

    // -- ScanStream --------------------------------------------------------

    #[test]
    fn scan_stream_running_sum() {
        let inner = StringStream::from_strings(vec![
            "ab".into(), "cde".into(), "f".into(),
        ]);
        let mut scan = ScanStream::new(inner, 0usize, |acc: &mut usize, s: String| {
            *acc += s.len();
        });
        assert_eq!(scan.read(), Some(2));
        assert_eq!(scan.read(), Some(5));
        assert_eq!(scan.read(), Some(6));
        assert!(scan.read().is_none());
    }

    #[test]
    fn scan_stream_empty() {
        let inner = StringStream::from_strings(vec![]);
        let mut scan = ScanStream::new(inner, 0usize, |acc: &mut usize, _s: String| {
            *acc += 1;
        });
        assert!(scan.read().is_none());
    }

    // -- DeduplicateStream -------------------------------------------------

    #[test]
    fn deduplicate_stream_removes_consecutive_dups() {
        let inner = StringStream::from_strings(vec![
            "a".into(), "a".into(), "b".into(), "b".into(), "b".into(), "a".into(),
        ]);
        let mut dedup = DeduplicateStream::new(inner);
        let result = dedup.collect_all();
        assert_eq!(result, vec!["a", "b", "a"]);
    }

    #[test]
    fn deduplicate_stream_no_dups() {
        let inner = StringStream::from_strings(vec!["x".into(), "y".into(), "z".into()]);
        let mut dedup = DeduplicateStream::new(inner);
        assert_eq!(dedup.collect_all(), vec!["x", "y", "z"]);
    }

    // -- BufferStream slice_bytes / reset / total_chunks --------------------

    #[test]
    fn buffer_stream_slice_bytes_single_chunk() {
        let stream = BufferStream::from_buffer(VsBuffer::from_string("hello world"));
        let sliced = stream.slice_bytes(6, 5).unwrap();
        assert_eq!(sliced.to_string_lossy(), "world");
    }

    #[test]
    fn buffer_stream_slice_bytes_across_chunks() {
        let stream = BufferStream::from_chunks(vec![
            VsBuffer::from_string("hel"),
            VsBuffer::from_string("lo "),
            VsBuffer::from_string("world"),
        ]);
        let sliced = stream.slice_bytes(2, 6).unwrap();
        assert_eq!(sliced.to_string_lossy(), "llo wo");
    }

    #[test]
    fn buffer_stream_slice_bytes_out_of_range() {
        let stream = BufferStream::from_buffer(VsBuffer::from_string("abc"));
        assert!(stream.slice_bytes(0, 10).is_none());
    }

    #[test]
    fn buffer_stream_reset() {
        let mut stream = BufferStream::from_chunks(vec![
            VsBuffer::from_string("a"),
            VsBuffer::from_string("b"),
        ]);
        assert_eq!(stream.read().unwrap().to_string_lossy(), "a");
        assert_eq!(stream.remaining(), 1);
        stream.reset();
        assert_eq!(stream.remaining(), 2);
        assert_eq!(stream.read().unwrap().to_string_lossy(), "a");
    }

    #[test]
    fn buffer_stream_total_chunks() {
        let mut stream = BufferStream::from_chunks(vec![
            VsBuffer::from_string("a"),
            VsBuffer::from_string("b"),
        ]);
        assert_eq!(stream.total_chunks(), 2);
        stream.read();
        assert_eq!(stream.total_chunks(), 2); // total stays constant
    }

    // -- StringStream split / reset ----------------------------------------

    #[test]
    fn string_stream_split() {
        let mut stream = StringStream::split("one,two,three", ',');
        assert_eq!(stream.read(), Some("one".to_string()));
        assert_eq!(stream.read(), Some("two".to_string()));
        assert_eq!(stream.read(), Some("three".to_string()));
        assert!(stream.read().is_none());
    }

    #[test]
    fn string_stream_reset() {
        let mut stream = StringStream::from_strings(vec!["a".into(), "b".into()]);
        stream.read();
        stream.read();
        assert!(stream.is_exhausted());
        stream.reset();
        assert_eq!(stream.remaining(), 2);
        assert_eq!(stream.read(), Some("a".to_string()));
    }

    // -- fold, any, all, find_first ----------------------------------------

    #[test]
    fn fold_concatenates_strings() {
        let mut stream = StringStream::from_strings(vec![
            "hello".into(), " ".into(), "world".into(),
        ]);
        let result = fold(&mut stream, String::new(), |mut acc: String, s: String| {
            acc.push_str(&s);
            acc
        });
        assert_eq!(result, "hello world");
    }

    #[test]
    fn fold_empty_stream() {
        let mut stream = StringStream::from_strings(vec![]);
        let result = fold(&mut stream, 42, |acc, _s: String| acc + 1);
        assert_eq!(result, 42);
    }

    #[test]
    fn any_finds_match() {
        let mut stream = StringStream::from_strings(vec![
            "foo".into(), "bar".into(), "baz".into(),
        ]);
        assert!(any_match(&mut stream, |s: &String| s.starts_with('b')));
    }

    #[test]
    fn any_no_match() {
        let mut stream = StringStream::from_strings(vec!["foo".into(), "far".into()]);
        assert!(!any_match(&mut stream, |s: &String| s.starts_with('z')));
    }

    #[test]
    fn all_true() {
        let mut stream = StringStream::from_strings(vec![
            "abc".into(), "ab".into(), "a".into(),
        ]);
        assert!(all_match(&mut stream, |s: &String| s.starts_with('a')));
    }

    #[test]
    fn all_false() {
        let mut stream = StringStream::from_strings(vec![
            "abc".into(), "xyz".into(),
        ]);
        assert!(!all_match(&mut stream, |s: &String| s.starts_with('a')));
    }

    #[test]
    fn all_empty_stream_is_true() {
        let mut stream = StringStream::from_strings(vec![]);
        assert!(all_match(&mut stream, |_s: &String| false));
    }

    #[test]
    fn find_first_returns_match() {
        let mut stream = StringStream::from_strings(vec![
            "apple".into(), "banana".into(), "avocado".into(),
        ]);
        let found = find_first(&mut stream, |s: &String| s.len() > 5);
        assert_eq!(found, Some("banana".to_string()));
    }

    // -- StreamSplitter -------------------------------------------------------

    #[test]
    fn splitter_splits_on_delimiter() {
        let mut sp = StreamSplitter::new(b',');
        sp.feed(b"hello,world,foo");
        assert_eq!(sp.next_chunk(), Some(b"hello".to_vec()));
        assert_eq!(sp.next_chunk(), Some(b"world".to_vec()));
        assert_eq!(sp.next_chunk(), None);
        assert_eq!(sp.pending_bytes(), 3); // "foo"
    }

    #[test]
    fn splitter_line_splitter_and_pending() {
        let mut sp = StreamSplitter::line_splitter();
        sp.feed(b"line1\nline2\npartial");
        assert_eq!(sp.next_chunk(), Some(b"line1".to_vec()));
        assert_eq!(sp.next_chunk(), Some(b"line2".to_vec()));
        assert_eq!(sp.next_chunk(), None);
        assert_eq!(sp.pending_bytes(), 7); // "partial"
    }

    #[test]
    fn splitter_incremental_feed() {
        let mut sp = StreamSplitter::new(b'\n');
        sp.feed(b"hel");
        assert_eq!(sp.next_chunk(), None);
        sp.feed(b"lo\nwor");
        assert_eq!(sp.next_chunk(), Some(b"hello".to_vec()));
        assert_eq!(sp.next_chunk(), None);
        sp.feed(b"ld\n");
        assert_eq!(sp.next_chunk(), Some(b"world".to_vec()));
    }

    #[test]
    fn splitter_display_and_default() {
        let sp = StreamSplitter::default();
        let s = sp.to_string();
        assert!(s.contains("0x0A")); // '\n'
        assert!(s.contains("ready=0"));
    }

    // -- ByteRingBuffer -------------------------------------------------------

    #[test]
    fn ring_buffer_write_and_read() {
        let mut rb = ByteRingBuffer::new(8);
        assert_eq!(rb.write(b"hello"), 5);
        assert_eq!(rb.len(), 5);
        assert_eq!(rb.available_space(), 3);
        let mut out = [0u8; 5];
        assert_eq!(rb.read(&mut out), 5);
        assert_eq!(&out, b"hello");
        assert!(rb.is_empty());
    }

    #[test]
    fn ring_buffer_backpressure() {
        let mut rb = ByteRingBuffer::new(4);
        assert_eq!(rb.write(b"abcdef"), 4); // only 4 fit
        assert!(rb.is_full());
        assert_eq!(rb.write(b"x"), 0); // backpressure
        let mut out = [0u8; 2];
        rb.read(&mut out);
        assert_eq!(&out, b"ab");
        assert_eq!(rb.available_space(), 2);
        assert_eq!(rb.write(b"xy"), 2); // wraps around
        let mut out2 = [0u8; 4];
        rb.read(&mut out2);
        assert_eq!(&out2, b"cdxy");
    }

    #[test]
    fn ring_buffer_display() {
        let rb = ByteRingBuffer::new(16);
        let s = rb.to_string();
        assert!(s.contains("used=0"));
        assert!(s.contains("capacity=16"));
    }

    // -- StreamTee -------------------------------------------------------------

    #[test]
    fn tee_duplicates_to_outputs() {
        let mut tee = StreamTee::new();
        let a = tee.add_output();
        let b = tee.add_output();
        tee.write(b"hello");
        tee.write(b" world");
        assert_eq!(tee.output_count(), 2);
        assert_eq!(tee.read_output(a), b"hello world");
        assert_eq!(tee.read_output(b), b"hello world");
        // after drain, outputs are empty
        assert_eq!(tee.read_output(a), b"");
    }

    #[test]
    fn tee_out_of_range_returns_empty() {
        let mut tee = StreamTee::default();
        assert_eq!(tee.read_output(99), b"");
    }

    #[test]
    fn tee_display() {
        let mut tee = StreamTee::new();
        tee.add_output();
        tee.write(b"abc");
        let s = tee.to_string();
        assert!(s.contains("outputs=1"));
        assert!(s.contains("buffered_bytes=3"));
    }

    // -- StreamProgressReporter ------------------------------------------------

    #[test]
    fn progress_tracks_bytes() {
        let mut pr = StreamProgressReporter::new(200);
        assert_eq!(pr.bytes_processed(), 0);
        assert!(!pr.is_complete());
        assert_eq!(pr.remaining_bytes(), 200);
        pr.record(50);
        assert!((pr.percentage() - 25.0).abs() < f64::EPSILON);
        pr.record(150);
        assert!(pr.is_complete());
        assert_eq!(pr.remaining_bytes(), 0);
    }

    #[test]
    fn progress_clamps_over_total() {
        let mut pr = StreamProgressReporter::new(10);
        pr.record(999);
        assert_eq!(pr.bytes_processed(), 10);
        assert!(pr.is_complete());
    }

    #[test]
    fn progress_zero_total_is_complete() {
        let pr = StreamProgressReporter::new(0);
        assert!(pr.is_complete());
        assert!((pr.percentage() - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn progress_display() {
        let mut pr = StreamProgressReporter::new(100);
        pr.record(30);
        let s = pr.to_string();
        assert!(s.contains("30/100"));
        assert!(s.contains("30.0%"));
    }

    #[test]
    fn find_first_returns_none() {
        let mut stream = StringStream::from_strings(vec!["a".into(), "b".into()]);
        let found = find_first(&mut stream, |s: &String| s.len() > 5);
        assert!(found.is_none());
    }

    #[test]
    fn stmbuf_ringbuf_push_get() {
        let mut rb = StmBufRingBuffer::new(3);
        rb.push(10); rb.push(20); rb.push(30);
        assert_eq!(rb.get(0), Some(&10));
        assert_eq!(rb.get(2), Some(&30));
        assert_eq!(rb.len(), 3);
    }

    #[test]
    fn stmbuf_ringbuf_overflow() {
        let mut rb = StmBufRingBuffer::<i32>::new(2);
        rb.push(1); rb.push(2); rb.push(3);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(&2));
        assert_eq!(rb.get(1), Some(&3));
    }

    #[test]
    fn stmbuf_ringbuf_clear() {
        let mut rb = StmBufRingBuffer::new(5);
        rb.push("a".to_string()); rb.push("b".to_string());
        rb.clear();
        assert!(rb.is_empty());
    }

    #[test]
    fn stmbuf_ringbuf_newest_oldest() {
        let mut rb = StmBufRingBuffer::new(4);
        rb.push(100); rb.push(200); rb.push(300);
        assert_eq!(rb.oldest(), Some(&100));
        assert_eq!(rb.newest(), Some(&300));
    }

    #[test]
    fn stmbuf_ringbuf_to_vec() {
        let mut rb = StmBufRingBuffer::new(3);
        rb.push(1); rb.push(2);
        assert_eq!(rb.to_vec(), vec![1, 2]);
    }

    #[test]
    fn stmbuf_ringbuf_is_full() {
        let mut rb = StmBufRingBuffer::new(2);
        assert!(!rb.is_full());
        rb.push(1); rb.push(2);
        assert!(rb.is_full());
    }

    #[test]
    fn stmbld_builder_valid() {
        let cfg = StmBldBuilder::new("test").property("key", "val")
            .tag("important").priority(5).build();
        assert!(cfg.is_ok());
        let cfg = cfg.unwrap();
        assert_eq!(cfg.name, "test");
        assert!(cfg.has_tag("important"));
        assert_eq!(cfg.get_property("key"), Some("val"));
    }

    #[test]
    fn stmbld_builder_empty_name() {
        let r = StmBldBuilder::new("").build();
        assert!(r.is_err());
        assert!(r.unwrap_err().to_string().contains("name"));
    }

    #[test]
    fn stmbld_builder_bad_priority() {
        assert!(StmBldBuilder::new("x").priority(200).build().is_err());
    }

    #[test]
    fn stmbld_builder_zero_max() {
        assert!(StmBldBuilder::new("x").max_items(0).build().is_err());
    }

    #[test]
    fn stmbld_cfg_merge() {
        let mut a = StmBldBuilder::new("a").property("x", "1").build().unwrap();
        let b = StmBldBuilder::new("b").property("x", "2").property("y", "3").build().unwrap();
        a.merge_properties(&b);
        assert_eq!(a.get_property("x"), Some("2"));
        assert_eq!(a.get_property("y"), Some("3"));
    }

    #[test]
    fn stmbld_cfg_display() {
        let cfg = StmBldBuilder::new("test").tag("a").tag("b")
            .enabled(false).build().unwrap();
        let s = format!("{}", cfg);
        assert!(s.contains("test"));
        assert!(s.contains("false"));
    }


    #[test]
    fn stream_entry_creation() {
        let e = StreamEntry::new("e1", "Entry 1");
        assert_eq!(e.id, "e1");
        assert_eq!(e.label, "Entry 1");
        assert!(e.active);
        assert_eq!(e.priority, 0);
    }

    #[test]
    fn stream_entry_with_priority() {
        let e = StreamEntry::new("e1", "E").with_priority(5);
        assert_eq!(e.priority, 5);
    }

    #[test]
    fn stream_entry_metadata() {
        let e = StreamEntry::new("e1", "E").with_meta("key", "val");
        assert_eq!(e.get_meta("key"), Some("val"));
        assert_eq!(e.get_meta("missing"), None);
        assert!(e.has_meta("key"));
        assert_eq!(e.meta_count(), 1);
    }

    #[test]
    fn stream_entry_remove_meta() {
        let mut e = StreamEntry::new("e1", "E").with_meta("k", "v");
        assert!(e.remove_meta("k"));
        assert!(!e.remove_meta("k"));
    }

    #[test]
    fn stream_entry_activate_deactivate() {
        let mut e = StreamEntry::new("e1", "E");
        e.deactivate();
        assert!(!e.active);
        e.activate();
        assert!(e.active);
    }

    #[test]
    fn stream_config_add_sorted() {
        let mut c = StreamConfig::new(10);
        c.add(StreamEntry::new("lo", "Lo").with_priority(1));
        c.add(StreamEntry::new("hi", "Hi").with_priority(10));
        assert_eq!(c.ids()[0], "hi");
    }

    #[test]
    fn stream_config_capacity() {
        let mut c = StreamConfig::new(1);
        assert!(c.add(StreamEntry::new("a", "A")));
        assert!(!c.add(StreamEntry::new("b", "B")));
        assert!(c.is_full());
    }

    #[test]
    fn stream_config_remove() {
        let mut c = StreamConfig::new(10);
        c.add(StreamEntry::new("a", "A"));
        assert!(c.remove("a"));
        assert!(!c.remove("a"));
        assert!(c.is_empty());
    }

    #[test]
    fn stream_config_get() {
        let mut c = StreamConfig::new(10);
        c.add(StreamEntry::new("x", "X"));
        assert!(c.get("x").is_some());
        assert!(c.get("y").is_none());
    }

    #[test]
    fn stream_config_active_entries() {
        let mut c = StreamConfig::new(10);
        c.add(StreamEntry::new("a", "A"));
        c.add(StreamEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        assert_eq!(c.active_entries().len(), 1);
        assert_eq!(c.count_active(), 1);
    }

    #[test]
    fn stream_config_enable_disable() {
        let mut c = StreamConfig::new(10);
        c.disable();
        assert!(!c.is_enabled());
        c.enable();
        assert!(c.is_enabled());
    }

    #[test]
    fn stream_config_clear() {
        let mut c = StreamConfig::new(10);
        c.add(StreamEntry::new("a", "A"));
        c.clear();
        assert!(c.is_empty());
    }

    #[test]
    fn stream_config_find_by_label() {
        let mut c = StreamConfig::new(10);
        c.add(StreamEntry::new("a", "Alpha"));
        assert_eq!(c.find_by_label("Alpha").unwrap().id, "a");
        assert!(c.find_by_label("missing").is_none());
    }

    #[test]
    fn stream_config_top_n() {
        let mut c = StreamConfig::new(10);
        c.add(StreamEntry::new("a", "A").with_priority(1));
        c.add(StreamEntry::new("b", "B").with_priority(2));
        c.add(StreamEntry::new("c", "C").with_priority(3));
        assert_eq!(c.top_n(2).len(), 2);
    }

    #[test]
    fn stream_config_deactivate_activate_all() {
        let mut c = StreamConfig::new(10);
        c.add(StreamEntry::new("a", "A"));
        c.add(StreamEntry::new("b", "B"));
        c.deactivate_all();
        assert_eq!(c.count_active(), 0);
        c.activate_all();
        assert_eq!(c.count_active(), 2);
    }

    #[test]
    fn stream_config_highest_priority() {
        let mut c = StreamConfig::new(10);
        assert!(c.highest_priority().is_none());
        c.add(StreamEntry::new("a", "A").with_priority(7));
        assert_eq!(c.highest_priority(), Some(7));
    }

    #[test]
    fn stream_config_contains() {
        let mut c = StreamConfig::new(10);
        c.add(StreamEntry::new("a", "A"));
        assert!(c.contains("a"));
        assert!(!c.contains("b"));
    }

    #[test]
    fn stream_config_labels() {
        let mut c = StreamConfig::new(10);
        c.add(StreamEntry::new("a", "Alpha"));
        c.add(StreamEntry::new("b", "Beta"));
        let labels = c.labels();
        assert!(labels.contains(&"Alpha"));
        assert!(labels.contains(&"Beta"));
    }

    #[test]
    fn stream_config_drain_inactive() {
        let mut c = StreamConfig::new(10);
        c.add(StreamEntry::new("a", "A"));
        c.add(StreamEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        let drained = c.drain_inactive();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].id, "a");
        assert_eq!(c.len(), 1);
    }


    #[test]
    fn yv_metrics_empty() {
        let m = YvMetrics::new("stream");
        assert_eq!(m.count(), 0);
        assert!((m.mean() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn yv_metrics_record_and_mean() {
        let mut m = YvMetrics::new("stream");
        m.record(10.0);
        m.record(20.0);
        m.record(30.0);
        assert_eq!(m.count(), 3);
        assert!((m.mean() - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn yv_metrics_min_max() {
        let mut m = YvMetrics::new("test");
        m.record(5.0);
        m.record(15.0);
        m.record(10.0);
        assert!((m.min_val() - 5.0).abs() < f64::EPSILON);
        assert!((m.max_val() - 15.0).abs() < f64::EPSILON);
    }

    #[test]
    fn yv_metrics_variance_and_std() {
        let mut m = YvMetrics::new("v");
        m.record(2.0);
        m.record(4.0);
        m.record(4.0);
        m.record(4.0);
        m.record(5.0);
        m.record(5.0);
        m.record(7.0);
        m.record(9.0);
        assert!(m.variance() > 0.0);
        assert!(m.std_dev() > 0.0);
    }

    #[test]
    fn yv_metrics_percentile() {
        let mut m = YvMetrics::new("p");
        for i in 1..=100 {
            m.record(i as f64);
        }
        let p50 = m.percentile(50.0);
        assert!(p50 >= 49.0 && p50 <= 51.0);
    }

    #[test]
    fn yv_metrics_merge() {
        let mut a = YvMetrics::new("a");
        a.record(1.0);
        let mut b = YvMetrics::new("b");
        b.record(2.0);
        b.record(3.0);
        a.merge(&b);
        assert_eq!(a.count(), 3);
    }

    #[test]
    fn yv_metrics_reset() {
        let mut m = YvMetrics::new("r");
        m.record(42.0);
        m.reset();
        assert_eq!(m.count(), 0);
    }

    #[test]
    fn yv_rate_window_empty() {
        let rw = YvRateWindow::new(1000);
        assert!(rw.is_empty());
        assert_eq!(rw.window_ms(), 1000);
    }

    #[test]
    fn yv_rate_window_tick_and_rate() {
        let mut rw = YvRateWindow::new(1000);
        rw.tick(100);
        rw.tick(200);
        rw.tick(300);
        assert_eq!(rw.rate(500), 3);
        assert_eq!(rw.rate(1500), 0);
    }

    #[test]
    fn yv_lru_cache_basic() {
        let mut c = YvLruCache::new(2);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        assert_eq!(c.get("a"), Some("1".to_string()));
        c.put("c".into(), "3".into());
        assert_eq!(c.get("b"), None);
    }

    #[test]
    fn yv_lru_cache_contains_and_keys() {
        let mut c = YvLruCache::new(3);
        c.put("x".into(), "10".into());
        c.put("y".into(), "20".into());
        assert!(c.contains_key("x"));
        assert!(!c.contains_key("z"));
        assert_eq!(c.keys().len(), 2);
    }

    #[test]
    fn yv_lru_cache_remove() {
        let mut c = YvLruCache::new(3);
        c.put("k".into(), "v".into());
        assert_eq!(c.remove("k"), Some("v".to_string()));
        assert!(c.is_empty());
        assert_eq!(c.remove("k"), None);
    }

    #[test]
    fn yv_metrics_sum() {
        let mut m = YvMetrics::new("s");
        m.record(1.0);
        m.record(2.0);
        m.record(3.0);
        assert!((m.sum() - 6.0).abs() < f64::EPSILON);
    }

    #[test]
    fn yv_metrics_label() {
        let m = YvMetrics::new("my_label");
        assert_eq!(m.label(), "my_label");
    }

    #[test]
    fn yv_lru_cache_clear() {
        let mut c = YvLruCache::new(5);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // xa_ extended tests for stream
    #[test]
    fn xa_stream_ring_new() {
        let rb = super::XaStreamRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_stream_ring_push_len() {
        let mut rb = super::XaStreamRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_stream_ring_wrap() {
        let mut rb = super::XaStreamRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_stream_ring_mean_empty() {
        let rb = super::XaStreamRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_stream_ring_mean_values() {
        let mut rb = super::XaStreamRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_stream_ring_min_max() {
        let mut rb = super::XaStreamRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_stream_ring_iter() {
        let mut rb = super::XaStreamRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_stream_counter_new() {
        let c = super::XaStreamCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_stream_counter_inc() {
        let mut c = super::XaStreamCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_stream_counter_inc_by() {
        let mut c = super::XaStreamCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_stream_counter_reset() {
        let mut c = super::XaStreamCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_stream_counter_clear() {
        let mut c = super::XaStreamCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_stream_counter_default() {
        let c = super::XaStreamCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }


    // ---- xc_ pool / scheduler tests – block 167 ----

    #[test]
    fn xc_167_pool_new_empty() {
        let pool: super::Xc167Pool<i32> = super::Xc167Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_167_pool_release_acquire() {
        let mut pool = super::Xc167Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_167_pool_acquire_empty() {
        let mut pool: super::Xc167Pool<i32> = super::Xc167Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_167_pool_full() {
        let mut pool = super::Xc167Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_167_pool_drain() {
        let mut pool = super::Xc167Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_167_pool_stats() {
        let mut pool = super::Xc167Pool::new(8);
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
    fn xc_167_pool_clear() {
        let mut pool = super::Xc167Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_167_pool_shrink() {
        let mut pool = super::Xc167Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_167_pool_default() {
        let pool: super::Xc167Pool<String> = super::Xc167Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_167_pool_extend() {
        let mut pool = super::Xc167Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_167_pool_retain() {
        let mut pool = super::Xc167Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_167_scheduler_round_robin() {
        let mut sched = super::Xc167Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_167_scheduler_empty() {
        let mut sched = super::Xc167Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_167_scheduler_reset() {
        let mut sched = super::Xc167Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_167_scheduler_add_remove() {
        let mut sched = super::Xc167Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_167_scheduler_targets() {
        let sched = super::Xc167Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_167_hash_empty() {
        assert_eq!(super::xc_167_hash(b""), 5381);
    }

    #[test]
    fn xc_167_hash_data() {
        let h = super::xc_167_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_167_hash(b"hello"), h);
    }

    #[test]
    fn xc_167_reverse_str() {
        assert_eq!(super::xc_167_reverse("abc"), "cba");
        assert_eq!(super::xc_167_reverse(""), "");
    }


    // --- xd_107 deepening tests ---

    #[test]
    fn xd_107_sm_initial_state() {
        let sm = Xd107StateMachine::new();
        assert_eq!(sm.current_state(), Xd107State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_107_sm_valid_idle_to_running() {
        let mut sm = Xd107StateMachine::new();
        assert!(sm.transition(Xd107State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd107State::Running);
    }

    #[test]
    fn xd_107_sm_valid_running_to_paused() {
        let mut sm = Xd107StateMachine::new();
        sm.transition(Xd107State::Running).unwrap();
        assert!(sm.transition(Xd107State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd107State::Paused);
    }

    #[test]
    fn xd_107_sm_valid_running_to_done() {
        let mut sm = Xd107StateMachine::new();
        sm.transition(Xd107State::Running).unwrap();
        assert!(sm.transition(Xd107State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd107State::Done);
    }

    #[test]
    fn xd_107_sm_valid_paused_to_running() {
        let mut sm = Xd107StateMachine::new();
        sm.transition(Xd107State::Running).unwrap();
        sm.transition(Xd107State::Paused).unwrap();
        assert!(sm.transition(Xd107State::Running).is_ok());
    }

    #[test]
    fn xd_107_sm_valid_done_to_idle() {
        let mut sm = Xd107StateMachine::new();
        sm.transition(Xd107State::Running).unwrap();
        sm.transition(Xd107State::Done).unwrap();
        assert!(sm.transition(Xd107State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd107State::Idle);
    }

    #[test]
    fn xd_107_sm_invalid_idle_to_done() {
        let mut sm = Xd107StateMachine::new();
        assert!(sm.transition(Xd107State::Done).is_err());
    }

    #[test]
    fn xd_107_sm_invalid_idle_to_paused() {
        let mut sm = Xd107StateMachine::new();
        assert!(sm.transition(Xd107State::Paused).is_err());
    }

    #[test]
    fn xd_107_sm_history_tracking() {
        let mut sm = Xd107StateMachine::new();
        sm.transition(Xd107State::Running).unwrap();
        sm.transition(Xd107State::Paused).unwrap();
        sm.transition(Xd107State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd107State::Idle);
        assert_eq!(sm.history()[0].to, Xd107State::Running);
        assert_eq!(sm.history()[1].from, Xd107State::Running);
        assert_eq!(sm.history()[2].to, Xd107State::Done);
    }

    #[test]
    fn xd_107_sm_serialize_deserialize() {
        let mut sm = Xd107StateMachine::new();
        sm.transition(Xd107State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd107StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd107State::Running));
    }

    #[test]
    fn xd_107_sm_deserialize_invalid() {
        assert_eq!(Xd107StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_107_sm_reset() {
        let mut sm = Xd107StateMachine::new();
        sm.transition(Xd107State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd107State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_107_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd107EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd107Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_107_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd107EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd107Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd107Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_107_bus_unsubscribe() {
        let mut bus = Xd107EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_107_event_kind_and_payload() {
        let e = Xd107Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd107Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_107_bus_clear_history() {
        let mut bus = Xd107EventBus::new();
        bus.publish(Xd107Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_107_sm_step_counter_increments() {
        let mut sm = Xd107StateMachine::new();
        sm.transition(Xd107State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd107State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xg_31 graph tests ------------------------------------------------

    #[test]
    fn xg_31_graph_empty() {
        let g = super::Xg31Graph::new();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn xg_31_graph_add_node() {
        let mut g = super::Xg31Graph::new();
        g.add_node(1);
        g.add_node(2);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_31_graph_add_edge() {
        let mut g = super::Xg31Graph::new();
        g.add_edge(0, 1);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_31_graph_neighbors() {
        let mut g = super::Xg31Graph::new();
        g.add_edge(0, 1);
        g.add_edge(0, 2);
        assert_eq!(g.neighbors(0).len(), 2);
    }

    #[test]
    fn xg_31_graph_has_path() {
        let mut g = super::Xg31Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(g.has_path(0, 2));
        assert!(!g.has_path(2, 0));
    }

    #[test]
    fn xg_31_graph_self_path() {
        let g = super::Xg31Graph::new();
        assert!(g.has_path(5, 5));
    }

    #[test]
    fn xg_31_graph_topo_sort() {
        let mut g = super::Xg31Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        let sorted = g.topological_sort().unwrap();
        let pos: std::collections::HashMap<usize, usize> =
            sorted.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&1] < pos[&2]);
    }

    #[test]
    fn xg_31_graph_cycle_detect_false() {
        let mut g = super::Xg31Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(!g.cycle_detect());
    }

    #[test]
    fn xg_31_graph_cycle_detect_true() {
        let mut g = super::Xg31Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        g.add_edge(2, 0);
        assert!(g.cycle_detect());
    }

    // -- xg_31 heap tests -------------------------------------------------

    #[test]
    fn xg_31_heap_empty() {
        let h: super::Xg31Heap<i32> = super::Xg31Heap::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn xg_31_heap_push_pop() {
        let mut h = super::Xg31Heap::new();
        h.push(3);
        h.push(1);
        h.push(2);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(3));
    }

    #[test]
    fn xg_31_heap_peek() {
        let mut h = super::Xg31Heap::new();
        h.push(5);
        h.push(2);
        assert_eq!(h.peek(), Some(&2));
    }

    #[test]
    fn xg_31_heap_drain_sorted() {
        let mut h = super::Xg31Heap::new();
        for v in [4, 1, 7, 2, 9] { h.push(v); }
        assert_eq!(h.drain_sorted(), vec![1, 2, 4, 7, 9]);
        assert!(h.is_empty());
    }

    #[test]
    fn xg_31_heap_merge() {
        let mut a = super::Xg31Heap::new();
        let mut b = super::Xg31Heap::new();
        a.push(5); a.push(3);
        b.push(4); b.push(1);
        a.merge(&mut b);
        assert_eq!(a.len(), 4);
        assert_eq!(a.pop(), Some(1));
    }

    #[test]
    fn xg_31_heap_default() {
        let h: super::Xg31Heap<u64> = Default::default();
        assert!(h.is_empty());
    }

    #[test]
    fn xg_31_graph_default() {
        let g: super::Xg31Graph = Default::default();
        assert_eq!(g.node_count(), 0);
    }


    #[test]
    fn xh166_skip_insert_contains() {
        let mut sl = super::Xh166SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh166_skip_remove() {
        let mut sl = super::Xh166SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh166_skip_len() {
        let mut sl = super::Xh166SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh166_skip_range_query() {
        let mut sl = super::Xh166SkipList::xh_new(4);
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
    fn xh166_skip_floor_ceiling() {
        let mut sl = super::Xh166SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh166_skip_rank() {
        let mut sl = super::Xh166SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh166_skip_empty() {
        let sl = super::Xh166SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh166_skip_duplicates() {
        let mut sl = super::Xh166SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh166_bitset_set_test() {
        let mut bs = super::Xh166BitSet::xh_new(256);
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
    fn xh166_bitset_clear_count() {
        let mut bs = super::Xh166BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh166_bitset_and_or_xor() {
        let mut a = super::Xh166BitSet::xh_new(128);
        let mut b = super::Xh166BitSet::xh_new(128);
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
    fn xh166_bitset_iter_ones() {
        let mut bs = super::Xh166BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh166_bitset_first_last() {
        let mut bs = super::Xh166BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh166_bitset_empty() {
        let bs = super::Xh166BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi166_deque_push_pop_back() {
        let mut dq = super::Xi166Deque::xi_new(4);
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
    fn xi166_deque_push_pop_front() {
        let mut dq = super::Xi166Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi166_deque_mixed_ops() {
        let mut dq = super::Xi166Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi166_deque_get_and_split() {
        let mut dq = super::Xi166Deque::xi_new(8);
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
    fn xi166_deque_rotate_left() {
        let mut dq = super::Xi166Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi166_deque_rotate_right() {
        let mut dq = super::Xi166Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi166_deque_grow() {
        let mut dq = super::Xi166Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi166_deque_empty() {
        let dq = super::Xi166Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi166_interval_tree_insert_query() {
        let mut tree = super::Xi166IntervalTree::xi_new();
        tree.xi_insert(super::Xi166Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi166Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi166Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi166_interval_tree_overlap() {
        let mut tree = super::Xi166IntervalTree::xi_new();
        tree.xi_insert(super::Xi166Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi166Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi166Interval::xi_new(12, 20));
        let q = super::Xi166Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi166_interval_tree_remove() {
        let mut tree = super::Xi166IntervalTree::xi_new();
        tree.xi_insert(super::Xi166Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi166Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi166_interval_tree_gaps() {
        let mut tree = super::Xi166IntervalTree::xi_new();
        tree.xi_insert(super::Xi166Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi166Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi166Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi166Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi166Interval::xi_new(8, 10));
    }

    #[test]
    fn xi166_interval_tree_merge() {
        let mut tree = super::Xi166IntervalTree::xi_new();
        tree.xi_insert(super::Xi166Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi166Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi166Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi166Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi166Interval::xi_new(10, 15));
    }

    #[test]
    fn xi166_interval_tree_all() {
        let mut tree = super::Xi166IntervalTree::xi_new();
        tree.xi_insert(super::Xi166Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi166Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi166_interval_tree_empty() {
        let tree = super::Xi166IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi166_interval_tree_contains_point() {
        let iv = super::Xi166Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 166) ---

    #[test]
    fn xj_166_uf_make_and_find() {
        let mut uf = super::Xj166UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_166_uf_union_connected() {
        let mut uf = super::Xj166UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_166_uf_component_count() {
        let mut uf = super::Xj166UnionFind::xj_new();
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
    fn xj_166_uf_component_size() {
        let mut uf = super::Xj166UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_166_uf_largest_component() {
        let mut uf = super::Xj166UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_166_uf_many_elements() {
        let mut uf = super::Xj166UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_166_uf_separate_components() {
        let mut uf = super::Xj166UnionFind::xj_new();
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
    fn xj_166_uf_path_compression() {
        let mut uf = super::Xj166UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_166_bt_insert_get() {
        let mut bt = super::Xj166BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_166_bt_contains_len() {
        let mut bt = super::Xj166BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_166_bt_replace() {
        let mut bt = super::Xj166BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_166_bt_remove() {
        let mut bt = super::Xj166BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_166_bt_keys_values() {
        let mut bt = super::Xj166BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_166_bt_range() {
        let mut bt = super::Xj166BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_166_bt_min_max() {
        let mut bt = super::Xj166BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_166_bt_many_inserts() {
        let mut bt = super::Xj166BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_166 segment tree tests ---

    #[test]
    fn xk_166_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk166SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_166_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk166SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_166_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk166SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_166_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk166SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_166_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk166SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_166_st_single_element() {
        let data = vec![42];
        let st = super::Xk166SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_166_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk166SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_166_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk166SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_166 disjoint intervals tests ---

    #[test]
    fn xk_166_di_add_and_count() {
        let mut di = super::Xk166DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_166_di_merge_overlap() {
        let mut di = super::Xk166DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_166_di_contains() {
        let mut di = super::Xk166DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_166_di_remove() {
        let mut di = super::Xk166DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_166_di_covered_length() {
        let mut di = super::Xk166DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_166_di_gaps() {
        let mut di = super::Xk166DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_166_di_merge_adjacent() {
        let mut di = super::Xk166DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_166_di_empty() {
        let di = super::Xk166DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }

}