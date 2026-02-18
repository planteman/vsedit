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

}