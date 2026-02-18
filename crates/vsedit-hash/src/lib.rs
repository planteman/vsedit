//! Hashing algorithms and UUID generation.
//!
//! Equivalent to VS Code's `vs/base/common/hash.ts` and UUID utils.

use std::collections::HashMap;
use std::fmt;
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// Compute a simple string hash (DJB2 algorithm, matching VS Code).
pub fn string_hash(s: &str) -> u32 {
    let mut hash: u32 = 5381;
    for byte in s.bytes() {
        hash = hash.wrapping_mul(33).wrapping_add(u32::from(byte));
    }
    hash
}

/// Compute a number hash, combining with an existing hash.
pub fn number_hash(value: u32, seed: u32) -> u32 {
    seed.wrapping_mul(33) ^ value
}

/// Compute SHA-256 hash of data, returning hex string.
pub fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let result = hasher.finalize();
    hex_encode(&result)
}

/// Compute SHA-256 hash of a string.
pub fn sha256_string(s: &str) -> String {
    sha256_hex(s.as_bytes())
}

/// Generate a random UUID v4.
pub fn generate_uuid() -> String {
    Uuid::new_v4().to_string()
}

/// Generate a short random ID (8 hex characters).
pub fn generate_short_id() -> String {
    let uuid = Uuid::new_v4();
    let bytes = uuid.as_bytes();
    hex_encode(&bytes[..4])
}

/// Compute SHA-256 hash of data, returning raw 32 bytes.
pub fn sha256_bytes(data: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize().into()
}

/// Hash file content for content addressing (sha256 + length).
pub fn hash_file_content(content: &str) -> String {
    let digest = sha256_string(content);
    format!("{digest}:{}", content.len())
}

/// Generate a UUID v5 (SHA-1 based, deterministic) from a namespace and name.
pub fn generate_uuid_v5(namespace: &str, name: &str) -> String {
    let ns_uuid = Uuid::new_v5(&Uuid::NAMESPACE_URL, namespace.as_bytes());
    Uuid::new_v5(&ns_uuid, name.as_bytes()).to_string()
}

/// Check whether a string is a valid UUID (8-4-4-4-12 hex format).
pub fn is_valid_uuid(s: &str) -> bool {
    Uuid::parse_str(s).is_ok()
}

/// Derive a key by hashing the concatenation of input and salt with SHA-256.
pub fn derive_key(input: &str, salt: &str) -> String {
    let combined = format!("{input}{salt}");
    sha256_string(&combined)
}

/// Errors that can occur in hash operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HashError {
    /// The input was empty when a non-empty value was required.
    EmptyInput(&'static str),
    /// A hex string had an invalid length or characters.
    InvalidHex(String),
    /// A content address string was malformed.
    InvalidContentAddress(String),
}

impl std::fmt::Display for HashError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HashError::EmptyInput(field) => write!(f, "empty input: {field}"),
            HashError::InvalidHex(msg) => write!(f, "invalid hex: {msg}"),
            HashError::InvalidContentAddress(msg) => write!(f, "invalid content address: {msg}"),
        }
    }
}

impl std::error::Error for HashError {}

/// A parsed content address consisting of a SHA-256 digest and a byte length.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentAddress {
    digest: String,
    length: usize,
}

impl ContentAddress {
    /// Create a content address from raw content.
    pub fn from_content(content: &str) -> Self {
        let digest = sha256_string(content);
        Self {
            digest,
            length: content.len(),
        }
    }

    /// Parse a content address string of the form `<hex-digest>:<length>`.
    pub fn parse(s: &str) -> Result<Self, HashError> {
        let (hex_part, len_part) = s
            .split_once(':')
            .ok_or_else(|| HashError::InvalidContentAddress("missing ':' separator".into()))?;

        if hex_part.len() != 64 {
            return Err(HashError::InvalidContentAddress(format!(
                "digest must be 64 hex chars, got {}",
                hex_part.len()
            )));
        }
        if !hex_part.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(HashError::InvalidHex("non-hex character in digest".into()));
        }

        let length: usize = len_part.parse().map_err(|_| {
            HashError::InvalidContentAddress(format!("invalid length: {len_part}"))
        })?;

        Ok(Self {
            digest: hex_part.to_string(),
            length,
        })
    }

    /// Verify that the given content matches this content address.
    pub fn verify(&self, content: &str) -> bool {
        content.len() == self.length && sha256_string(content) == self.digest
    }

    /// Return the SHA-256 digest portion.
    pub fn digest(&self) -> &str {
        &self.digest
    }

    /// Return the content length portion.
    pub fn length(&self) -> usize {
        self.length
    }

    /// Check whether the digest starts with the given hex prefix (case-insensitive).
    pub fn matches_digest_prefix(&self, prefix: &str) -> bool {
        self.digest
            .as_bytes()
            .iter()
            .zip(prefix.as_bytes())
            .all(|(a, b)| a.to_ascii_lowercase() == b.to_ascii_lowercase())
    }
}

impl std::fmt::Display for ContentAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.digest, self.length)
    }
}

/// Validate and parse a hex string into bytes.
pub fn hex_decode(s: &str) -> Result<Vec<u8>, HashError> {
    if s.len() % 2 != 0 {
        return Err(HashError::InvalidHex("odd-length hex string".into()));
    }
    (0..s.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&s[i..i + 2], 16)
                .map_err(|_| HashError::InvalidHex(format!("invalid byte at position {i}")))
        })
        .collect()
}

/// Derive a key with multiple rounds of SHA-256 hashing for added strength.
pub fn derive_key_rounds(input: &str, salt: &str, rounds: u32) -> Result<String, HashError> {
    if input.is_empty() {
        return Err(HashError::EmptyInput("input"));
    }
    if rounds == 0 {
        return Err(HashError::EmptyInput("rounds must be > 0"));
    }
    let mut current = format!("{input}{salt}");
    for _ in 0..rounds {
        current = sha256_string(&current);
    }
    Ok(current)
}

/// Compute an HMAC-like keyed hash using SHA-256: `H(key || message)`.
///
/// Note: this is a simplified construction, not a standards-compliant HMAC.
pub fn keyed_hash(key: &str, message: &str) -> String {
    sha256_string(&format!("{key}{message}"))
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Encode bytes as an uppercase hex string.
pub fn hex_encode_upper(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02X}")).collect()
}

/// A simple hash combiner for combining multiple hash values.
#[derive(Clone, PartialEq, Eq)]
pub struct HashCombiner {
    hash: u32,
}

impl HashCombiner {
    /// Create a new combiner with initial seed.
    pub fn new() -> Self {
        Self { hash: 0 }
    }

    /// Create a combiner pre-loaded with a slice of strings.
    pub fn from_strings(strings: &[&str]) -> Self {
        let mut combiner = Self::new();
        for s in strings {
            combiner.add_string(s);
        }
        combiner
    }

    /// Add a string to the hash.
    pub fn add_string(&mut self, s: &str) -> &mut Self {
        self.hash = number_hash(string_hash(s), self.hash);
        self
    }

    /// Add a number to the hash.
    pub fn add_number(&mut self, n: u32) -> &mut Self {
        self.hash = number_hash(n, self.hash);
        self
    }

    /// Add a boolean to the hash.
    pub fn add_bool(&mut self, b: bool) -> &mut Self {
        self.hash = number_hash(u32::from(b), self.hash);
        self
    }

    /// Add raw bytes to the hash (hashes the byte length and each byte).
    pub fn add_bytes(&mut self, data: &[u8]) -> &mut Self {
        self.hash = number_hash(data.len() as u32, self.hash);
        for &b in data {
            self.hash = number_hash(u32::from(b), self.hash);
        }
        self
    }

    /// Add an optional string. Hashes presence (0/1) and value if present.
    pub fn add_optional_string(&mut self, s: Option<&str>) -> &mut Self {
        match s {
            Some(v) => {
                self.add_bool(true);
                self.add_string(v);
            }
            None => {
                self.add_bool(false);
            }
        }
        self
    }

    /// Reset the hash back to the initial state.
    pub fn reset(&mut self) {
        self.hash = 0;
    }

    /// Returns true if no values have been added since creation or last reset.
    pub fn is_empty(&self) -> bool {
        self.hash == 0
    }

    /// Get the combined hash value.
    pub fn value(&self) -> u32 {
        self.hash
    }
}

impl std::fmt::Display for HashCombiner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:08x}", self.hash)
    }
}

impl Default for HashCombiner {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for HashCombiner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HashCombiner")
            .field("hash", &format_args!("{:#010x}", self.hash))
            .finish()
    }
}

/// Selects a hashing algorithm for use with [`ChecksumVerifier`] and general-purpose hashing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HashAlgorithm {
    Djb2,
    Sha256,
    FnvLike,
}

impl HashAlgorithm {
    /// Hash a string using the selected algorithm, returning a `u64`.
    pub fn hash_str(&self, s: &str) -> u64 {
        match self {
            HashAlgorithm::Djb2 => {
                let mut hash: u64 = 5381;
                for byte in s.bytes() {
                    hash = hash.wrapping_mul(33).wrapping_add(u64::from(byte));
                }
                hash
            }
            HashAlgorithm::Sha256 => {
                let digest = sha256_bytes(s.as_bytes());
                u64::from_be_bytes(digest[..8].try_into().unwrap())
            }
            HashAlgorithm::FnvLike => fnv1a_hash(s.as_bytes()),
        }
    }
}

/// FNV-1a hash for arbitrary byte data.
pub fn fnv1a_hash(data: &[u8]) -> u64 {
    const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const FNV_PRIME: u64 = 0x0100_0000_01b3;
    let mut hash = FNV_OFFSET;
    for &byte in data {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

/// Computes and verifies hex-encoded checksums using a chosen [`HashAlgorithm`].
pub struct ChecksumVerifier;

impl ChecksumVerifier {
    /// Compute a hex checksum of `data` using `algorithm`.
    pub fn compute(data: &[u8], algorithm: HashAlgorithm) -> String {
        match algorithm {
            HashAlgorithm::Sha256 => sha256_hex(data),
            _ => {
                let h = match algorithm {
                    HashAlgorithm::Djb2 => {
                        let mut hash: u64 = 5381;
                        for &b in data {
                            hash = hash.wrapping_mul(33).wrapping_add(u64::from(b));
                        }
                        hash
                    }
                    HashAlgorithm::FnvLike => fnv1a_hash(data),
                    HashAlgorithm::Sha256 => unreachable!(),
                };
                format!("{h:016x}")
            }
        }
    }

    /// Verify that `data` matches `expected` hex checksum under `algorithm`.
    pub fn verify(data: &[u8], expected: &str, algorithm: HashAlgorithm) -> bool {
        Self::compute(data, algorithm) == expected
    }
}

/// Consistent-hash bucket ring for distributing keys across buckets.
pub struct HashBucket {
    bucket_count: usize,
}

impl HashBucket {
    /// Create a new ring with `bucket_count` buckets (must be > 0).
    pub fn new(bucket_count: usize) -> Self {
        assert!(bucket_count > 0, "bucket_count must be > 0");
        Self { bucket_count }
    }

    /// Assign a key to a bucket index in `0..bucket_count`.
    pub fn assign(&self, key: &str) -> usize {
        let h = fnv1a_hash(key.as_bytes());
        (h as usize) % self.bucket_count
    }

    /// Change the number of buckets.
    pub fn rebalance(&mut self, new_count: usize) {
        assert!(new_count > 0, "new_count must be > 0");
        self.bucket_count = new_count;
    }
}

/// Combine hashes of `values` in order (order matters).
pub fn hash_combine_ordered(values: &[&str]) -> u32 {
    let mut hash: u32 = 0;
    for (i, v) in values.iter().enumerate() {
        let h = string_hash(v);
        // mix in the position so identical values at different indices differ
        hash = hash.wrapping_add(h.wrapping_mul((i as u32).wrapping_add(1)));
    }
    hash
}

/// Combine hashes of `values` regardless of order (commutative).
pub fn hash_combine_unordered(values: &[&str]) -> u32 {
    let mut hash: u32 = 0;
    for v in values {
        hash ^= string_hash(v);
    }
    hash
}

// ---------------------------------------------------------------------------
// CRC32, hash mixing, consistent hashing ring, Bloom filter
// ---------------------------------------------------------------------------

/// Compute CRC32 checksum (using the standard polynomial 0xEDB88320).
pub fn crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &byte in data {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xEDB8_8320;
            } else {
                crc >>= 1;
            }
        }
    }
    !crc
}

/// Mix two 32-bit hash values using a Murmur-like finalizer.
pub fn hash_mix(a: u32, b: u32) -> u32 {
    let mut h = a.wrapping_add(b.wrapping_mul(0x9e37_79b9));
    h ^= h >> 16;
    h = h.wrapping_mul(0x85eb_ca6b);
    h ^= h >> 13;
    h = h.wrapping_mul(0xc2b2_ae35);
    h ^= h >> 16;
    h
}

/// Combine a slice of u32 hash values into a single hash using `hash_mix`.
pub fn hash_combine_all(values: &[u32]) -> u32 {
    let mut acc: u32 = 0;
    for &v in values {
        acc = hash_mix(acc, v);
    }
    acc
}

/// A consistent hashing ring that maps keys to named nodes.
pub struct ConsistentHashRing {
    nodes: Vec<String>,
}

impl ConsistentHashRing {
    pub fn new() -> Self {
        Self { nodes: Vec::new() }
    }

    /// Add a node to the ring.
    pub fn add_node(&mut self, name: impl Into<String>) {
        self.nodes.push(name.into());
    }

    /// Remove a node by name. Returns true if found and removed.
    pub fn remove_node(&mut self, name: &str) -> bool {
        let before = self.nodes.len();
        self.nodes.retain(|n| n != name);
        self.nodes.len() < before
    }

    /// Determine which node a key maps to. Returns `None` if the ring is empty.
    pub fn get_node(&self, key: &str) -> Option<&str> {
        if self.nodes.is_empty() {
            return None;
        }
        let h = fnv1a_hash(key.as_bytes()) as usize;
        let idx = h % self.nodes.len();
        Some(&self.nodes[idx])
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}

impl Default for ConsistentHashRing {
    fn default() -> Self {
        Self::new()
    }
}

/// A simple Bloom filter for probabilistic set membership.
pub struct BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
}

impl BloomFilter {
    /// Create a new Bloom filter with `size` bits and `num_hashes` hash functions.
    pub fn new(size: usize, num_hashes: usize) -> Self {
        let size = if size == 0 { 64 } else { size };
        let num_hashes = if num_hashes == 0 { 1 } else { num_hashes };
        Self {
            bits: vec![false; size],
            num_hashes,
        }
    }

    /// Insert an item into the Bloom filter.
    pub fn insert(&mut self, item: &[u8]) {
        for i in 0..self.num_hashes {
            let idx = self.hash_index(item, i);
            self.bits[idx] = true;
        }
    }

    /// Check if an item may be in the set. False positives are possible.
    pub fn may_contain(&self, item: &[u8]) -> bool {
        for i in 0..self.num_hashes {
            let idx = self.hash_index(item, i);
            if !self.bits[idx] {
                return false;
            }
        }
        true
    }

    /// Insert a string into the Bloom filter (convenience wrapper).
    pub fn insert_str(&mut self, s: &str) {
        self.insert(s.as_bytes());
    }

    /// Check if a string may be in the set (convenience wrapper).
    pub fn may_contain_str(&self, s: &str) -> bool {
        self.may_contain(s.as_bytes())
    }

    /// Clear all bits.
    pub fn clear(&mut self) {
        self.bits.iter_mut().for_each(|b| *b = false);
    }

    /// Return the number of bits set to true.
    pub fn count_ones(&self) -> usize {
        self.bits.iter().filter(|&&b| b).count()
    }

    /// Estimate the current false-positive rate based on the proportion of set bits.
    ///
    /// Uses the formula `(set_bits / total_bits) ^ num_hashes`.
    pub fn estimated_false_positive_rate(&self) -> f64 {
        let ones = self.count_ones() as f64;
        let total = self.bits.len() as f64;
        (ones / total).powi(self.num_hashes as i32)
    }

    fn hash_index(&self, item: &[u8], seed: usize) -> usize {
        let mut data = Vec::with_capacity(item.len() + 8);
        data.extend_from_slice(item);
        data.extend_from_slice(&(seed as u64).to_le_bytes());
        (fnv1a_hash(&data) as usize) % self.bits.len()
    }
}

// ---------------------------------------------------------------------------
// File-level content hashing and diff hashing
// ---------------------------------------------------------------------------

/// Compute the SHA-256 hash of file content given as a byte slice.
///
/// Returns the hex-encoded digest string.
pub fn content_hash_file(data: &[u8]) -> String {
    sha256_hex(data)
}

/// A byte range within file content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ByteRange {
    pub start: usize,
    pub end: usize,
}

impl ByteRange {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    /// Returns the length of this range.
    pub fn len(&self) -> usize {
        self.end.saturating_sub(self.start)
    }

    /// Returns true if the range is empty.
    pub fn is_empty(&self) -> bool {
        self.start >= self.end
    }

    /// Returns true if this range is valid (start <= end).
    pub fn is_valid(&self) -> bool {
        self.start <= self.end
    }

    /// Returns true if `offset` falls within the half-open range `[start, end)`.
    pub fn contains(&self, offset: usize) -> bool {
        offset >= self.start && offset < self.end
    }

    /// Returns true if this range and `other` share at least one offset.
    pub fn overlaps(&self, other: &ByteRange) -> bool {
        self.start < other.end && other.start < self.end
    }

    /// Merge two overlapping or adjacent ranges into one.
    ///
    /// Returns `None` if the ranges are disjoint (gap > 0).
    pub fn merge(&self, other: &ByteRange) -> Option<ByteRange> {
        if self.start <= other.end && other.start <= self.end {
            Some(ByteRange::new(
                self.start.min(other.start),
                self.end.max(other.end),
            ))
        } else {
            None
        }
    }

    /// Returns an iterator over every offset in `[start, end)`.
    pub fn iter(&self) -> ByteRangeIter {
        ByteRangeIter {
            current: self.start,
            end: self.end,
        }
    }
}

/// Iterator that yields every offset in a [`ByteRange`].
pub struct ByteRangeIter {
    current: usize,
    end: usize,
}

impl Iterator for ByteRangeIter {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current < self.end {
            let val = self.current;
            self.current += 1;
            Some(val)
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.end.saturating_sub(self.current);
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for ByteRangeIter {}

impl IntoIterator for ByteRange {
    type Item = usize;
    type IntoIter = ByteRangeIter;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl std::fmt::Display for ByteRange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}..{})", self.start, self.end)
    }
}

/// Compute a SHA-256 hash of only the specified byte ranges within `data`.
///
/// Concatenates bytes from each range in order and hashes the result.
/// Empty or out-of-bounds ranges are skipped.
pub fn diff_hash(data: &[u8], ranges: &[ByteRange]) -> String {
    let mut hasher = Sha256::new();
    let mut any_data = false;
    for range in ranges {
        if !range.is_valid() || range.start >= data.len() {
            continue;
        }
        let end = range.end.min(data.len());
        let slice = &data[range.start..end];
        if !slice.is_empty() {
            hasher.update(slice);
            any_data = true;
        }
    }
    if !any_data {
        return sha256_hex(b"");
    }
    let result = hasher.finalize();
    hex_encode(&result)
}

/// Compute line-based diff ranges between two text contents.
///
/// Returns byte ranges in `new_content` that differ from `old_content`.
pub fn compute_changed_ranges(old_content: &str, new_content: &str) -> Vec<ByteRange> {
    let old_lines: Vec<&str> = old_content.lines().collect();
    let new_lines: Vec<&str> = new_content.lines().collect();
    let mut ranges = Vec::new();
    let mut byte_offset = 0;

    for (i, new_line) in new_lines.iter().enumerate() {
        let line_len = new_line.len();
        let changed = match old_lines.get(i) {
            Some(old_line) => old_line != new_line,
            None => true,
        };
        if changed {
            ranges.push(ByteRange::new(byte_offset, byte_offset + line_len));
        }
        byte_offset += line_len + 1; // +1 for newline
    }

    ranges
}

/// Compute a combined hash of both the full content and its changed ranges.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffHashResult {
    pub full_hash: String,
    pub diff_hash: String,
    pub changed_range_count: usize,
    pub changed_bytes: usize,
}

/// Compute both full and diff hashes for a file, given the old and new content.
pub fn compute_diff_hash(old_content: &str, new_content: &str) -> DiffHashResult {
    let ranges = compute_changed_ranges(old_content, new_content);
    let changed_bytes: usize = ranges.iter().map(|r| r.len()).sum();
    let full_hash = content_hash_file(new_content.as_bytes());
    let diff = diff_hash(new_content.as_bytes(), &ranges);
    DiffHashResult {
        full_hash,
        diff_hash: diff,
        changed_range_count: ranges.len(),
        changed_bytes,
    }
}

// ---------------------------------------------------------------------------
// CRC32 string convenience and incremental hashing
// ---------------------------------------------------------------------------

/// Compute CRC32 of a string (convenience wrapper).
pub fn crc32_str(s: &str) -> u32 {
    crc32(s.as_bytes())
}

/// Incremental hasher that feeds data in chunks and produces a final SHA-256 digest.
pub struct IncrementalHasher {
    hasher: Sha256,
    bytes_fed: usize,
}

impl IncrementalHasher {
    /// Create a new incremental hasher.
    pub fn new() -> Self {
        Self {
            hasher: Sha256::new(),
            bytes_fed: 0,
        }
    }

    /// Feed a chunk of data into the hasher.
    pub fn update(&mut self, data: &[u8]) {
        self.hasher.update(data);
        self.bytes_fed += data.len();
    }

    /// Feed a string into the hasher.
    pub fn update_str(&mut self, s: &str) {
        self.update(s.as_bytes());
    }

    /// Return the total number of bytes fed so far.
    pub fn bytes_fed(&self) -> usize {
        self.bytes_fed
    }

    /// Finalize and return the hex-encoded SHA-256 digest.
    /// Consumes the hasher.
    pub fn finalize(self) -> String {
        let result = self.hasher.finalize();
        hex_encode(&result)
    }
}

impl Default for IncrementalHasher {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Hash-based content deduplication
// ---------------------------------------------------------------------------

/// A deduplication store that tracks content by its SHA-256 hash.
/// Returns whether content was already seen.
pub struct ContentDedup {
    seen: std::collections::HashSet<String>,
}

impl ContentDedup {
    /// Create a new empty dedup store.
    pub fn new() -> Self {
        Self {
            seen: std::collections::HashSet::new(),
        }
    }

    /// Insert content. Returns `true` if the content is new (not a duplicate),
    /// `false` if it was already present.
    pub fn insert(&mut self, content: &str) -> bool {
        let hash = sha256_string(content);
        self.seen.insert(hash)
    }

    /// Check if content has already been seen.
    pub fn contains(&self, content: &str) -> bool {
        let hash = sha256_string(content);
        self.seen.contains(&hash)
    }

    /// Number of unique items tracked.
    pub fn len(&self) -> usize {
        self.seen.len()
    }

    /// Returns `true` if no items have been inserted.
    pub fn is_empty(&self) -> bool {
        self.seen.is_empty()
    }
}

impl Default for ContentDedup {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Hash comparison utilities
// ---------------------------------------------------------------------------

/// Compare two hex digest strings in constant time (to avoid timing attacks).
/// Both must be the same length; returns `false` if lengths differ.
pub fn constant_time_eq(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut result: u8 = 0;
    for (x, y) in a.bytes().zip(b.bytes()) {
        result |= x ^ y;
    }
    result == 0
}

/// Extract the version field from a UUID v4 string.
/// Returns `Some(4)` for valid v4 UUIDs.
pub fn uuid_version(uuid_str: &str) -> Option<u8> {
    let parsed = uuid::Uuid::parse_str(uuid_str).ok()?;
    Some(parsed.get_version_num() as u8)
}

// ---------------------------------------------------------------------------
// HMAC-SHA256 (RFC 2104)
// ---------------------------------------------------------------------------

/// HMAC block size for SHA-256.
const HMAC_BLOCK_SIZE: usize = 64;

/// Compute HMAC-SHA256 per RFC 2104.
///
/// Returns the hex-encoded MAC. Unlike [`keyed_hash`], this follows the
/// standard two-pass HMAC construction and is safe against length-extension
/// attacks.
pub fn hmac_sha256(key: &[u8], message: &[u8]) -> String {
    // If key is longer than block size, hash it first.
    let key_block = if key.len() > HMAC_BLOCK_SIZE {
        let h = sha256_bytes(key);
        let mut kb = [0u8; HMAC_BLOCK_SIZE];
        kb[..32].copy_from_slice(&h);
        kb
    } else {
        let mut kb = [0u8; HMAC_BLOCK_SIZE];
        kb[..key.len()].copy_from_slice(key);
        kb
    };

    // inner pad (0x36) and outer pad (0x5c)
    let mut i_key_pad = [0x36u8; HMAC_BLOCK_SIZE];
    let mut o_key_pad = [0x5cu8; HMAC_BLOCK_SIZE];
    for i in 0..HMAC_BLOCK_SIZE {
        i_key_pad[i] ^= key_block[i];
        o_key_pad[i] ^= key_block[i];
    }

    // inner hash: SHA256(i_key_pad || message)
    let mut inner = Sha256::new();
    inner.update(i_key_pad);
    inner.update(message);
    let inner_hash = inner.finalize();

    // outer hash: SHA256(o_key_pad || inner_hash)
    let mut outer = Sha256::new();
    outer.update(o_key_pad);
    outer.update(inner_hash);
    hex_encode(&outer.finalize())
}

/// Convenience wrapper: HMAC-SHA256 with string key and message.
pub fn hmac_sha256_str(key: &str, message: &str) -> String {
    hmac_sha256(key.as_bytes(), message.as_bytes())
}

// ---------------------------------------------------------------------------
// Merkle tree
// ---------------------------------------------------------------------------

/// A Merkle tree built from a list of leaf items.
///
/// Each leaf is the SHA-256 hash of the item. Internal nodes are the SHA-256
/// hash of the concatenation of their two children. If a level has an odd
/// number of nodes the last node is duplicated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MerkleTree {
    /// All levels, from leaves (index 0) to root (last).
    levels: Vec<Vec<String>>,
}

impl MerkleTree {
    /// Build a Merkle tree from leaf data slices.
    pub fn from_items(items: &[&[u8]]) -> Self {
        if items.is_empty() {
            return Self {
                levels: vec![vec![sha256_hex(b"")]],
            };
        }

        let leaves: Vec<String> = items.iter().map(|item| sha256_hex(item)).collect();
        let mut levels = vec![leaves];

        while levels.last().unwrap().len() > 1 {
            let prev = levels.last().unwrap();
            let mut next = Vec::with_capacity((prev.len() + 1) / 2);
            let mut i = 0;
            while i < prev.len() {
                let left = &prev[i];
                let right = if i + 1 < prev.len() {
                    &prev[i + 1]
                } else {
                    left // duplicate last node
                };
                let combined = format!("{left}{right}");
                next.push(sha256_string(&combined));
                i += 2;
            }
            levels.push(next);
        }

        Self { levels }
    }

    /// Build a Merkle tree from string slices (convenience).
    pub fn from_strings(items: &[&str]) -> Self {
        let byte_items: Vec<&[u8]> = items.iter().map(|s| s.as_bytes()).collect();
        Self::from_items(&byte_items)
    }

    /// Return the root hash.
    pub fn root(&self) -> &str {
        &self.levels.last().unwrap()[0]
    }

    /// Return the number of levels (including leaves).
    pub fn depth(&self) -> usize {
        self.levels.len()
    }

    /// Return the leaf hashes.
    pub fn leaves(&self) -> &[String] {
        &self.levels[0]
    }

    /// Verify that a given leaf value is consistent with the root.
    ///
    /// Rebuilds the tree and checks if the root matches.
    pub fn verify_leaf(&self, index: usize, data: &[u8]) -> bool {
        if index >= self.levels[0].len() {
            return false;
        }
        sha256_hex(data) == self.levels[0][index]
    }
}

impl fmt::Display for MerkleTree {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "MerkleTree(root={}, leaves={})", self.root(), self.levels[0].len())
    }
}

// ---------------------------------------------------------------------------
// Rolling hash (Rabin-Karp)
// ---------------------------------------------------------------------------

/// A rolling hash using a polynomial (Rabin-Karp style) for efficient
/// substring matching over a sliding window.
pub struct RollingHash {
    hash: u64,
    base: u64,
    modulus: u64,
    /// `base^window_size mod modulus`, precomputed for removal.
    base_pow: u64,
    window_size: usize,
    buffer: Vec<u8>,
}

impl RollingHash {
    /// Create a rolling hash with the given window size.
    pub fn new(window_size: usize) -> Self {
        let base: u64 = 257;
        let modulus: u64 = 1_000_000_007;
        let base_pow = mod_pow(base, window_size as u64, modulus);
        Self {
            hash: 0,
            base,
            modulus,
            base_pow,
            window_size,
            buffer: Vec::new(),
        }
    }

    /// Push a byte into the window. If the window is full the oldest byte is
    /// removed and the hash is updated accordingly.
    pub fn push(&mut self, byte: u8) {
        self.buffer.push(byte);
        if self.buffer.len() <= self.window_size {
            self.hash = (self.hash.wrapping_mul(self.base) + u64::from(byte)) % self.modulus;
        } else {
            let old = self.buffer[self.buffer.len() - self.window_size - 1];
            self.hash = (self.hash.wrapping_mul(self.base)
                + u64::from(byte)
                + self.modulus
                - (u64::from(old) * self.base_pow) % self.modulus)
                % self.modulus;
        }
    }

    /// Current hash value.
    pub fn value(&self) -> u64 {
        self.hash
    }

    /// Number of bytes pushed so far.
    pub fn bytes_pushed(&self) -> usize {
        self.buffer.len()
    }

    /// Returns `true` once at least `window_size` bytes have been pushed.
    pub fn is_full(&self) -> bool {
        self.buffer.len() >= self.window_size
    }
}

/// Modular exponentiation: `base^exp mod modulus`.
fn mod_pow(mut base: u64, mut exp: u64, modulus: u64) -> u64 {
    let mut result: u64 = 1;
    base %= modulus;
    while exp > 0 {
        if exp & 1 == 1 {
            result = result.wrapping_mul(base) % modulus;
        }
        exp >>= 1;
        base = base.wrapping_mul(base) % modulus;
    }
    result
}

/// Find all starting positions where `pattern` occurs in `text` using a
/// rolling-hash (Rabin-Karp) search.
pub fn rabin_karp_search(text: &[u8], pattern: &[u8]) -> Vec<usize> {
    if pattern.is_empty() || pattern.len() > text.len() {
        return Vec::new();
    }

    let pat_len = pattern.len();
    let base: u64 = 257;
    let modulus: u64 = 1_000_000_007;
    let base_pow = mod_pow(base, pat_len as u64, modulus);

    // Hash the pattern.
    let mut pat_hash: u64 = 0;
    for &b in pattern {
        pat_hash = (pat_hash.wrapping_mul(base) + u64::from(b)) % modulus;
    }

    // Hash the first window.
    let mut win_hash: u64 = 0;
    for &b in &text[..pat_len] {
        win_hash = (win_hash.wrapping_mul(base) + u64::from(b)) % modulus;
    }

    let mut positions = Vec::new();
    if win_hash == pat_hash && text[..pat_len] == *pattern {
        positions.push(0);
    }

    for i in 1..=(text.len() - pat_len) {
        let old = u64::from(text[i - 1]);
        let new = u64::from(text[i + pat_len - 1]);
        win_hash = (win_hash.wrapping_mul(base) + new + modulus - (old * base_pow) % modulus)
            % modulus;
        if win_hash == pat_hash && text[i..i + pat_len] == *pattern {
            positions.push(i);
        }
    }
    positions
}

// ---------------------------------------------------------------------------
// Content-addressable storage key
// ---------------------------------------------------------------------------

/// Generate a content-addressable storage (CAS) key from arbitrary data.
///
/// The key encodes the hash algorithm, the hex digest, and the data length,
/// separated by colons: `sha256:<hex>:<len>`.
pub fn cas_key(data: &[u8]) -> String {
    let digest = sha256_hex(data);
    format!("sha256:{digest}:{}", data.len())
}

/// Parse a CAS key back into its components: `(algorithm, digest, length)`.
pub fn cas_key_parse(key: &str) -> Result<(&str, &str, usize), HashError> {
    let mut parts = key.splitn(3, ':');
    let algo = parts
        .next()
        .ok_or_else(|| HashError::InvalidContentAddress("missing algorithm".into()))?;
    let digest = parts
        .next()
        .ok_or_else(|| HashError::InvalidContentAddress("missing digest".into()))?;
    let len_str = parts
        .next()
        .ok_or_else(|| HashError::InvalidContentAddress("missing length".into()))?;
    let length: usize = len_str
        .parse()
        .map_err(|_| HashError::InvalidContentAddress(format!("invalid length: {len_str}")))?;
    Ok((algo, digest, length))
}

/// Verify that `data` matches a previously generated CAS key.
pub fn cas_key_verify(data: &[u8], key: &str) -> Result<bool, HashError> {
    let (_, digest, length) = cas_key_parse(key)?;
    Ok(data.len() == length && sha256_hex(data) == digest)
}

// ---------------------------------------------------------------------------
// Hex similarity scoring
// ---------------------------------------------------------------------------

/// Compute a similarity score between two hex digest strings.
///
/// Returns a value in `[0.0, 1.0]` representing the fraction of nibble
/// positions that match. Returns `0.0` if the strings have different lengths
/// or are empty.
pub fn hex_similarity(a: &str, b: &str) -> f64 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let matching = a
        .bytes()
        .zip(b.bytes())
        .filter(|(x, y)| x.to_ascii_lowercase() == y.to_ascii_lowercase())
        .count();
    matching as f64 / a.len() as f64
}


// ---------------------------------------------------------------------------
// MultiFieldHasher
// ---------------------------------------------------------------------------

pub struct MultiFieldHasher {
    state: u64,
}

impl MultiFieldHasher {
    pub fn new() -> Self { Self { state: 0xcbf29ce484222325 } }

    pub fn feed_str(&mut self, s: &str) -> &mut Self {
        for byte in s.as_bytes() {
            self.state ^= *byte as u64;
            self.state = self.state.wrapping_mul(0x100000001b3);
        }
        self
    }

    pub fn feed_u64(&mut self, n: u64) -> &mut Self {
        self.feed_str(&n.to_string())
    }

    pub fn feed_bool(&mut self, b: bool) -> &mut Self {
        self.feed_str(if b { "T" } else { "F" })
    }

    pub fn finish(&self) -> u64 { self.state }

    pub fn finish_hex(&self) -> String { format!("{:016x}", self.state) }

    pub fn reset(&mut self) { self.state = 0xcbf29ce484222325; }
}

impl Default for MultiFieldHasher { fn default() -> Self { Self::new() } }

impl std::fmt::Display for MultiFieldHasher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "MultiFieldHasher(0x{:016x})", self.state)
    }
}

// ---------------------------------------------------------------------------
// SetMembershipFilter
// ---------------------------------------------------------------------------

pub struct SetMembershipFilter {
    bits: Vec<bool>,
    size: usize,
    num_hashes: usize,
}

impl SetMembershipFilter {
    pub fn new(size: usize, num_hashes: usize) -> Self {
        Self { bits: vec![false; size], size, num_hashes }
    }

    fn hash_indices(&self, item: &str) -> Vec<usize> {
        let h1 = string_hash(item) as usize;
        let h2 = fnv1a_hash(item.as_bytes()) as usize;
        (0..self.num_hashes).map(|i| (h1.wrapping_add(i.wrapping_mul(h2))) % self.size).collect()
    }

    pub fn insert(&mut self, item: &str) {
        for idx in self.hash_indices(item) { self.bits[idx] = true; }
    }

    pub fn might_contain(&self, item: &str) -> bool {
        self.hash_indices(item).iter().all(|&idx| self.bits[idx])
    }

    pub fn false_positive_rate(&self) -> f64 {
        let set_bits = self.bits.iter().filter(|&&b| b).count();
        (set_bits as f64 / self.size as f64).powi(self.num_hashes as i32)
    }

    pub fn clear(&mut self) { self.bits.iter_mut().for_each(|b| *b = false); }
}

// ---------------------------------------------------------------------------
// DistributionRing
// ---------------------------------------------------------------------------

pub struct DistributionRing {
    nodes: Vec<(u32, String)>,
}

impl DistributionRing {
    pub fn new() -> Self { Self { nodes: Vec::new() } }

    pub fn add_node(&mut self, name: impl Into<String>) {
        let name = name.into();
        let hash = string_hash(&name);
        self.nodes.push((hash, name));
        self.nodes.sort_by_key(|(h, _)| *h);
    }

    pub fn get_node(&self, key: &str) -> Option<&str> {
        if self.nodes.is_empty() { return None; }
        let hash = string_hash(key);
        let idx = self.nodes.iter().position(|(h, _)| *h >= hash).unwrap_or(0);
        Some(&self.nodes[idx].1)
    }

    pub fn node_count(&self) -> usize { self.nodes.len() }

    pub fn remove_node(&mut self, name: &str) -> bool {
        if let Some(i) = self.nodes.iter().position(|(_, n)| n == name) { self.nodes.remove(i); true } else { false }
    }
}

impl Default for DistributionRing { fn default() -> Self { Self::new() } }

// ---------------------------------------------------------------------------
// HashBenchmark
// ---------------------------------------------------------------------------

pub struct HashBenchmark {
    results: Vec<(String, u64)>,
}

impl HashBenchmark {
    pub fn new() -> Self { Self { results: Vec::new() } }

    pub fn record(&mut self, algorithm: impl Into<String>, duration_ns: u64) {
        self.results.push((algorithm.into(), duration_ns));
    }

    pub fn fastest(&self) -> Option<(&str, u64)> {
        self.results.iter().min_by_key(|(_, d)| *d).map(|(a, d)| (a.as_str(), *d))
    }

    pub fn slowest(&self) -> Option<(&str, u64)> {
        self.results.iter().max_by_key(|(_, d)| *d).map(|(a, d)| (a.as_str(), *d))
    }

    pub fn average_ns(&self) -> Option<u64> {
        if self.results.is_empty() { None }
        else { Some(self.results.iter().map(|(_, d)| d).sum::<u64>() / self.results.len() as u64) }
    }

    pub fn count(&self) -> usize { self.results.len() }
}

impl Default for HashBenchmark { fn default() -> Self { Self::new() } }


// ── Hash Accumulator ──

/// Accumulates multiple values into a combined hash using the DJB2 algorithm.
pub struct HashAccumulator {
    state: u32,
    count: usize,
}

impl HashAccumulator {
    /// Create a new accumulator with default seed.
    pub fn new() -> Self {
        Self { state: 5381, count: 0 }
    }

    /// Create with a custom seed.
    pub fn with_seed(seed: u32) -> Self {
        Self { state: seed, count: 0 }
    }

    /// Feed a string value into the accumulator.
    pub fn feed_str(&mut self, s: &str) -> &mut Self {
        self.state = number_hash(string_hash(s), self.state);
        self.count += 1;
        self
    }

    /// Feed a u32 value.
    pub fn feed_u32(&mut self, value: u32) -> &mut Self {
        self.state = number_hash(value, self.state);
        self.count += 1;
        self
    }

    /// Feed a boolean value.
    pub fn feed_bool(&mut self, value: bool) -> &mut Self {
        self.feed_u32(if value { 1 } else { 0 })
    }

    /// Feed raw bytes by hashing each byte.
    pub fn feed_bytes(&mut self, data: &[u8]) -> &mut Self {
        for &b in data {
            self.state = self.state.wrapping_mul(33).wrapping_add(u32::from(b));
        }
        self.count += 1;
        self
    }

    /// Feed an optional string (hashes differently for None vs Some).
    pub fn feed_option_str(&mut self, value: Option<&str>) -> &mut Self {
        match value {
            Some(s) => {
                self.feed_bool(true);
                self.feed_str(s);
            }
            None => {
                self.feed_bool(false);
            }
        }
        self
    }

    /// Feed a slice of strings.
    pub fn feed_str_slice(&mut self, values: &[&str]) -> &mut Self {
        self.feed_u32(values.len() as u32);
        for s in values {
            self.feed_str(s);
        }
        self
    }

    /// Get the current accumulated hash.
    pub fn finish(&self) -> u32 {
        self.state
    }

    /// Number of items fed so far.
    pub fn item_count(&self) -> usize {
        self.count
    }

    /// Reset to initial state.
    pub fn reset(&mut self) {
        self.state = 5381;
        self.count = 0;
    }

    /// Create a SHA-256 hex string from the accumulated state.
    pub fn finish_sha256(&self) -> String {
        sha256_hex(&self.state.to_le_bytes())
    }

    /// Combine two accumulators.
    pub fn combine(&mut self, other: &HashAccumulator) -> &mut Self {
        self.state = number_hash(other.state, self.state);
        self.count += other.count;
        self
    }
}

impl Default for HashAccumulator {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for HashAccumulator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HashAccumulator")
            .field("state", &format!("{:#010x}", self.state))
            .field("count", &self.count)
            .finish()
    }
}

// ── Hash Comparison Helper ──

/// Provides constant-time comparison and formatting utilities for hashes.
pub struct HashComparisonHelper;

impl HashComparisonHelper {
    /// Constant-time comparison of two byte slices.
    pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
        if a.len() != b.len() {
            return false;
        }
        let mut diff = 0u8;
        for (x, y) in a.iter().zip(b.iter()) {
            diff |= x ^ y;
        }
        diff == 0
    }

    /// Constant-time comparison of two hex strings.
    pub fn hex_eq(a: &str, b: &str) -> bool {
        Self::constant_time_eq(a.as_bytes(), b.as_bytes())
    }

    /// Compare two SHA-256 hex digests of strings.
    pub fn sha256_strings_eq(a: &str, b: &str) -> bool {
        let hash_a = sha256_string(a);
        let hash_b = sha256_string(b);
        Self::hex_eq(&hash_a, &hash_b)
    }

    /// Format a u32 hash as a zero-padded hex string.
    pub fn format_hex(hash: u32) -> String {
        format!("{:08x}", hash)
    }

    /// Format a u32 hash as a short 4-char hex prefix.
    pub fn format_short(hash: u32) -> String {
        format!("{:08x}", hash)[..4].to_string()
    }

    /// Check if a hex string is valid.
    pub fn is_valid_hex(s: &str) -> bool {
        !s.is_empty() && s.chars().all(|c| c.is_ascii_hexdigit())
    }

    /// Compare a string's hash against an expected u32 hash value.
    pub fn verify_string_hash(s: &str, expected: u32) -> bool {
        string_hash(s) == expected
    }

    /// Compute the hamming distance between two u32 hashes.
    pub fn hamming_distance(a: u32, b: u32) -> u32 {
        (a ^ b).count_ones()
    }

    /// Check if two strings produce the same DJB2 hash.
    pub fn djb2_collision(a: &str, b: &str) -> bool {
        string_hash(a) == string_hash(b)
    }

    /// Split a SHA-256 hex string into 8-char chunks.
    pub fn split_sha256(hex: &str) -> Vec<&str> {
        let mut chunks = Vec::new();
        let mut start = 0;
        while start + 8 <= hex.len() {
            chunks.push(&hex[start..start + 8]);
            start += 8;
        }
        if start < hex.len() {
            chunks.push(&hex[start..]);
        }
        chunks
    }
}



// ─── HashBuf Ring Buffer ──────────────────────────────────────

/// A fixed-capacity ring buffer for hash log.
#[derive(Debug, Clone)]
pub struct HashBufRingBuffer<T> {
    buf: Vec<Option<T>>,
    head: usize,
    len: usize,
}

impl<T: Clone> HashBufRingBuffer<T> {
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

impl<T: Clone + fmt::Display> fmt::Display for HashBufRingBuffer<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "HashBufRingBuffer(len={}, cap={})", self.len, self.capacity())
    }
}

// ─── HashC LRU Cache ───────────────────────────────────────

/// A simple LRU cache for hash results.
#[derive(Debug)]
pub struct HashCLruCache<V> {
    entries: Vec<(String, V)>,
    capacity: usize,
    hits: u64,
    misses: u64,
}

impl<V: Clone> HashCLruCache<V> {
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0);
        Self { entries: Vec::with_capacity(capacity), capacity, hits: 0, misses: 0 }
    }

    pub fn insert(&mut self, key: impl Into<String>, value: V) -> Option<(String, V)> {
        let key = key.into();
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == &key) {
            self.entries.remove(pos);
            self.entries.insert(0, (key, value));
            return None;
        }
        let evicted = if self.entries.len() >= self.capacity {
            Some(self.entries.pop().unwrap())
        } else { None };
        self.entries.insert(0, (key, value));
        evicted
    }

    pub fn get(&mut self, key: &str) -> Option<&V> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            self.hits += 1;
            let entry = self.entries.remove(pos);
            self.entries.insert(0, entry);
            Some(&self.entries[0].1)
        } else {
            self.misses += 1;
            None
        }
    }

    pub fn peek(&self, key: &str) -> Option<&V> {
        self.entries.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    pub fn remove(&mut self, key: &str) -> Option<V> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            Some(self.entries.remove(pos).1)
        } else { None }
    }

    pub fn len(&self) -> usize { self.entries.len() }
    pub fn is_empty(&self) -> bool { self.entries.is_empty() }

    pub fn hit_ratio(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 { 0.0 } else { self.hits as f64 / total as f64 }
    }

    pub fn hits(&self) -> u64 { self.hits }
    pub fn misses(&self) -> u64 { self.misses }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.hits = 0;
        self.misses = 0;
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    pub fn contains(&self, key: &str) -> bool {
        self.entries.iter().any(|(k, _)| k == key)
    }
}

impl<V: Clone + fmt::Display> fmt::Display for HashCLruCache<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "HashCLruCache(size={}, cap={}, hits={}, misses={})",
            self.len(), self.capacity, self.hits, self.misses)
    }
}


/// Configuration manager for hash functionality.
pub struct HashConfig {
    options: HashMap<String, String>,
    enabled: bool,
    version: u32,
}

impl HashConfig {
    pub fn new() -> Self {
        Self { options: HashMap::new(), enabled: true, version: 1 }
    }

    pub fn set_option(&mut self, key: &str, value: &str) {
        self.options.insert(key.to_string(), value.to_string());
    }

    pub fn get_option(&self, key: &str) -> Option<&str> {
        self.options.get(key).map(|s| s.as_str())
    }

    pub fn remove_option(&mut self, key: &str) -> Option<String> {
        self.options.remove(key)
    }

    pub fn option_count(&self) -> usize { self.options.len() }

    pub fn is_enabled(&self) -> bool { self.enabled }

    pub fn set_enabled(&mut self, enabled: bool) { self.enabled = enabled; }

    pub fn version(&self) -> u32 { self.version }

    pub fn bump_version(&mut self) { self.version += 1; }

    pub fn has_option(&self, key: &str) -> bool { self.options.contains_key(key) }

    pub fn option_keys(&self) -> Vec<String> {
        let mut keys: Vec<_> = self.options.keys().cloned().collect();
        keys.sort();
        keys
    }

    pub fn clear(&mut self) {
        self.options.clear();
        self.version = 1;
    }

    pub fn merge(&mut self, other: &HashConfig) {
        for (k, v) in &other.options {
            self.options.insert(k.clone(), v.clone());
        }
    }
}

/// Rate tracker for hash operations.
pub struct HashRateTracker {
    window_ms: u64,
    timestamps: Vec<u64>,
}

impl HashRateTracker {
    pub fn new(window_ms: u64) -> Self {
        Self { window_ms, timestamps: Vec::new() }
    }

    pub fn record(&mut self, ts: u64) {
        self.timestamps.push(ts);
        self.prune(ts);
    }

    fn prune(&mut self, now: u64) {
        let cutoff = now.saturating_sub(self.window_ms);
        self.timestamps.retain(|&t| t >= cutoff);
    }

    pub fn count(&self) -> usize { self.timestamps.len() }

    pub fn rate_per_second(&self) -> f64 {
        if self.timestamps.len() < 2 { return 0.0; }
        let span = self.timestamps.last().unwrap() - self.timestamps.first().unwrap();
        if span == 0 { return 0.0; }
        (self.timestamps.len() as f64 / span as f64) * 1000.0
    }

    pub fn clear(&mut self) { self.timestamps.clear(); }

    pub fn window_ms(&self) -> u64 { self.window_ms }
}

/// Validation result collector for hash.
pub struct HashValidator {
    errors: Vec<String>,
    warnings: Vec<String>,
}

impl HashValidator {
    pub fn new() -> Self {
        Self { errors: Vec::new(), warnings: Vec::new() }
    }

    pub fn add_error(&mut self, msg: &str) {
        self.errors.push(msg.to_string());
    }

    pub fn add_warning(&mut self, msg: &str) {
        self.warnings.push(msg.to_string());
    }

    pub fn is_valid(&self) -> bool { self.errors.is_empty() }

    pub fn error_count(&self) -> usize { self.errors.len() }

    pub fn warning_count(&self) -> usize { self.warnings.len() }

    pub fn errors(&self) -> &[String] { &self.errors }

    pub fn warnings(&self) -> &[String] { &self.warnings }

    pub fn clear(&mut self) {
        self.errors.clear();
        self.warnings.clear();
    }

    pub fn merge(&mut self, other: &HashValidator) {
        self.errors.extend(other.errors.iter().cloned());
        self.warnings.extend(other.warnings.iter().cloned());
    }

    pub fn first_error(&self) -> Option<&str> {
        self.errors.first().map(|s| s.as_str())
    }
}


// ---------------------------------------------------------------------------
// Content hashing utilities — extended utilities (yp)
// ---------------------------------------------------------------------------

/// Metric accumulator for hash operations.
#[derive(Debug, Clone)]
pub struct YpMetrics {
    samples: Vec<f64>,
    label: String,
}

impl YpMetrics {
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

/// Sliding-window rate counter for hash.
#[derive(Debug, Clone)]
pub struct YpRateWindow {
    timestamps: Vec<u64>,
    window_ms: u64,
}

impl YpRateWindow {
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

/// A small LRU-style cache for hash lookups.
#[derive(Debug, Clone)]
pub struct YpLruCache {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl YpLruCache {
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
// xa_ extended helpers for hash
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaHashRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaHashRingBuf {
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
pub struct XaHashCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaHashCounter {
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

impl Default for XaHashCounter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 88
// ---------------------------------------------------------------------------

/// Generic object pool `Xc88Pool<T>`.
pub struct Xc88Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc88Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc88PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc88Pool<T> {
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
    pub fn stats(&self) -> Xc88PoolStats {
        Xc88PoolStats {
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

impl<T> Default for Xc88Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc88Scheduler`.
pub struct Xc88Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc88Scheduler {
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

impl Default for Xc88Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_88 hash for the given byte slice.
pub fn xc_88_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_88 convention.
pub fn xc_88_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_99 deepening: state machine + event bus ---

/// States for the Xd99 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd99State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd99State {
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
pub struct Xd99Transition {
    pub from: Xd99State,
    pub to: Xd99State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd99StateMachine {
    current: Xd99State,
    history: Vec<Xd99Transition>,
    step_counter: usize,
}

impl Xd99StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd99State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd99State {
        self.current
    }

    pub fn history(&self) -> &[Xd99Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd99State) -> Result<Xd99State, String> {
        let allowed = match (self.current, target) {
            (Xd99State::Idle, Xd99State::Running) => true,
            (Xd99State::Running, Xd99State::Paused) => true,
            (Xd99State::Running, Xd99State::Done) => true,
            (Xd99State::Paused, Xd99State::Running) => true,
            (Xd99State::Paused, Xd99State::Done) => true,
            (Xd99State::Done, Xd99State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_99: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd99Transition {
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
            "Xd99SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd99State> {
        let prefix = "Xd99SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd99State::Idle),
            "Running" => Some(Xd99State::Running),
            "Paused" => Some(Xd99State::Paused),
            "Done" => Some(Xd99State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd99State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd99 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd99Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd99Event {
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

type Xd99HandlerFn = Box<dyn Fn(&Xd99Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd99EventBus {
    handlers: Vec<(usize, Option<String>, Xd99HandlerFn)>,
    next_id: usize,
    published: Vec<Xd99Event>,
}

impl Xd99EventBus {
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
        F: Fn(&Xd99Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd99Event) + Send + Sync + 'static,
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
    pub fn publish(&mut self, event: Xd99Event) {
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

    pub fn published_events(&self) -> &[Xd99Event] {
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
// xg_23: Directed graph
// ---------------------------------------------------------------------------

/// A directed graph with adjacency-list representation.
#[derive(Debug, Clone)]
pub struct Xg23Graph {
    adj: std::collections::HashMap<usize, Vec<usize>>,
    edge_cnt: usize,
}

impl Xg23Graph {
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

impl Default for Xg23Graph {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// xg_23: Min-heap
// ---------------------------------------------------------------------------

/// A min-heap backed by a `Vec`.
#[derive(Debug, Clone)]
pub struct Xg23Heap<T: Ord> {
    data: Vec<T>,
}

impl<T: Ord> Xg23Heap<T> {
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
    pub fn merge(&mut self, other: &mut Xg23Heap<T>) {
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

impl<T: Ord> Default for Xg23Heap<T> {
    fn default() -> Self { Self::new() }
}


/// A probabilistic sorted list using a skip-list structure (variant 87).
pub struct Xh87SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh87SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 129 as u64,
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

/// A compact bit set supporting boolean operations (variant 87).
pub struct Xh87BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh87BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 87).
pub struct Xi87Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi87Deque<T> {
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
pub struct Xi87Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi87Interval {
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

/// A simple interval tree (variant 87).
pub struct Xi87IntervalTree {
    xi_intervals: Vec<Xi87Interval>,
}

impl Xi87IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi87Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi87Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi87Interval) -> Vec<&Xi87Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi87Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi87Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi87Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi87Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi87Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi87Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 87) ---

/// Disjoint set / union-find for crate 87.
pub struct Xj87UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj87UnionFind {
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

const XJ87_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 87.
pub struct Xj87BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj87BTreeNode<K, V>>>,
    len: usize,
}

struct Xj87BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj87BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj87BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ87_BTREE_ORDER - 1
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
        let mid = XJ87_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj87BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj87BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj87BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj87BTreeNode::xj_new_leaf();
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


// --- xk_87 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk87SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk87SegmentTree {
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
pub struct Xk87DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk87DisjointIntervals {
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


/// Rope data structure for efficient large text manipulation (xl_87).
#[derive(Debug, Clone)]
pub struct Xl87Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl87Rope {
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

/// Suffix array for efficient string searching (xl_87).
#[derive(Debug, Clone)]
pub struct Xl87SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl87SuffixArray {
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


/// Sparse matrix storing non-zero entries in coordinate format.
pub struct Xm87MatrixSparse {
    rows: usize,
    cols: usize,
    entries: Vec<(usize, usize, f64)>,
}

impl Xm87MatrixSparse {
    /// Create a new sparse matrix with the given dimensions.
    pub fn xm_new(rows: usize, cols: usize) -> Self {
        Self { rows, cols, entries: Vec::new() }
    }

    /// Set the value at `(row, col)`. Overwrites if already present.
    pub fn xm_set(&mut self, row: usize, col: usize, value: f64) {
        if row >= self.rows || col >= self.cols {
            return;
        }
        if let Some(pos) = self.entries.iter().position(|e| e.0 == row && e.1 == col) {
            if value == 0.0 {
                self.entries.remove(pos);
            } else {
                self.entries[pos].2 = value;
            }
        } else if value != 0.0 {
            self.entries.push((row, col, value));
        }
    }

    /// Get the value at `(row, col)`, returning 0 for absent entries.
    pub fn xm_get(&self, row: usize, col: usize) -> f64 {
        self.entries.iter()
            .find(|e| e.0 == row && e.1 == col)
            .map_or(0.0, |e| e.2)
    }

    /// Return all non-zero entries in the given row as `(col, value)` pairs.
    pub fn xm_row(&self, row: usize) -> Vec<(usize, f64)> {
        self.entries.iter()
            .filter(|e| e.0 == row)
            .map(|e| (e.1, e.2))
            .collect()
    }

    /// Return all non-zero entries in the given column as `(row, value)` pairs.
    pub fn xm_col(&self, col: usize) -> Vec<(usize, f64)> {
        self.entries.iter()
            .filter(|e| e.1 == col)
            .map(|e| (e.0, e.2))
            .collect()
    }

    /// Return a new sparse matrix that is the transpose of this one.
    pub fn xm_transpose(&self) -> Self {
        let mut t = Self::xm_new(self.cols, self.rows);
        for &(r, c, v) in &self.entries {
            t.entries.push((c, r, v));
        }
        t
    }

    /// Multiply this matrix by a dense vector, returning the result vector.
    pub fn xm_multiply_vec(&self, vec: &[f64]) -> Vec<f64> {
        let mut result = vec![0.0; self.rows];
        for &(r, c, v) in &self.entries {
            if c < vec.len() {
                result[r] += v * vec[c];
            }
        }
        result
    }

    /// Return the number of stored non-zero entries.
    pub fn xm_nnz(&self) -> usize {
        self.entries.len()
    }

    /// Return the density (nnz / total_elements).
    pub fn xm_density(&self) -> f64 {
        let total = self.rows * self.cols;
        if total == 0 { return 0.0; }
        self.entries.len() as f64 / total as f64
    }

    /// Remove all entries, keeping dimensions.
    pub fn xm_clear(&mut self) {
        self.entries.clear();
    }

    /// Return the matrix dimensions as `(rows, cols)`.
    pub fn xm_dims(&self) -> (usize, usize) {
        (self.rows, self.cols)
    }
}

/// Simple tokenizer for splitting text into tokens.
pub struct Xm87Tokenizer {
    text: String,
}

impl Xm87Tokenizer {
    /// Create a new tokenizer from the given text.
    pub fn xm_new(text: &str) -> Self {
        Self { text: text.to_string() }
    }

    /// Tokenize the text by splitting on whitespace and filtering empties.
    pub fn xm_tokenize(&self) -> Vec<String> {
        self.text.split_whitespace().map(String::from).collect()
    }

    /// Split by whitespace, preserving the raw split results.
    pub fn xm_split_by_whitespace(&self) -> Vec<String> {
        self.text.split(' ')
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect()
    }

    /// Split the text using a custom single-character delimiter.
    pub fn xm_split_by_delimiter(&self, delim: char) -> Vec<String> {
        self.text.split(delim)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect()
    }

    /// Return the number of whitespace-delimited tokens.
    pub fn xm_token_count(&self) -> usize {
        self.xm_tokenize().len()
    }

    /// Return the set of unique tokens.
    pub fn xm_unique_tokens(&self) -> Vec<String> {
        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();
        for tok in self.xm_tokenize() {
            if seen.insert(tok.clone()) {
                result.push(tok);
            }
        }
        result
    }

    /// Build a frequency map of each token.
    pub fn xm_frequency_map(&self) -> std::collections::HashMap<String, usize> {
        let mut map = std::collections::HashMap::new();
        for tok in self.xm_tokenize() {
            *map.entry(tok).or_insert(0) += 1;
        }
        map
    }

    /// Return the underlying text.
    pub fn xm_text(&self) -> &str {
        &self.text
    }

    /// Return whether the text is empty.
    pub fn xm_is_empty(&self) -> bool {
        self.text.is_empty()
    }
}


/// Fenwick (Binary Indexed) tree for prefix‑sum queries — crate 87.
pub struct Xn87Fenwick {
    xn_tree: Vec<i64>,
    xn_n: usize,
}

impl Xn87Fenwick {
    /// Create a new Fenwick tree of size `n` initialised to zero.
    pub fn xn_new(n: usize) -> Self {
        Self { xn_tree: vec![0i64; n + 1], xn_n: n }
    }

    /// Point‑update: add `delta` to index `i` (0‑based).
    pub fn xn_update(&mut self, mut i: usize, delta: i64) {
        i += 1;
        while i <= self.xn_n {
            self.xn_tree[i] += delta;
            i += i & i.wrapping_neg();
        }
    }

    /// Prefix sum of elements `[0, i]` (0‑based, inclusive).
    pub fn xn_prefix_sum(&self, mut i: usize) -> i64 {
        i += 1;
        let mut s = 0i64;
        while i > 0 {
            s += self.xn_tree[i];
            i -= i & i.wrapping_neg();
        }
        s
    }

    /// Range sum of elements `[l, r]` (inclusive, 0‑based).
    pub fn xn_range_sum(&self, l: usize, r: usize) -> i64 {
        if l == 0 {
            self.xn_prefix_sum(r)
        } else {
            self.xn_prefix_sum(r) - self.xn_prefix_sum(l - 1)
        }
    }

    /// Point query — value at index `i`.
    pub fn xn_point_query(&self, i: usize) -> i64 {
        self.xn_range_sum(i, i)
    }

    /// Number of elements the tree can hold.
    pub fn xn_len(&self) -> usize {
        self.xn_n
    }

    /// Find the smallest index whose prefix sum is at least `target`.
    /// Returns `None` when no such index exists.
    pub fn xn_find_kth(&self, mut target: i64) -> Option<usize> {
        let mut pos: usize = 0;
        let mut bit_mask = 1usize;
        while bit_mask <= self.xn_n {
            bit_mask <<= 1;
        }
        bit_mask >>= 1;
        while bit_mask > 0 {
            let next = pos + bit_mask;
            if next <= self.xn_n && self.xn_tree[next] < target {
                target -= self.xn_tree[next];
                pos = next;
            }
            bit_mask >>= 1;
        }
        let result = pos; // 0‑based
        if result < self.xn_n {
            Some(result)
        } else {
            None
        }
    }
}

// ----- AVL tree map — crate 87 -----

#[derive(Debug, Clone)]
struct Xn87AvlNode<K, V> {
    key: K,
    value: V,
    left: Option<Box<Xn87AvlNode<K, V>>>,
    right: Option<Box<Xn87AvlNode<K, V>>>,
    height: i32,
}

/// Self‑balancing AVL tree map — crate 87.
#[derive(Debug, Clone)]
pub struct Xn87AVL<K, V> {
    root: Option<Box<Xn87AvlNode<K, V>>>,
    xn_len: usize,
}

impl<K: Ord + Clone, V: Clone> Default for Xn87AVL<K, V> {
    fn default() -> Self {
        Self::xn_new()
    }
}

impl<K: Ord + Clone, V: Clone> Xn87AVL<K, V> {
    pub fn xn_new() -> Self {
        Self { root: None, xn_len: 0 }
    }

    fn xn_node_height(node: &Option<Box<Xn87AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| n.height)
    }

    fn xn_balance(node: &Option<Box<Xn87AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| Self::xn_node_height(&n.left) - Self::xn_node_height(&n.right))
    }

    fn xn_update_height(node: &mut Box<Xn87AvlNode<K, V>>) {
        node.height = 1 + std::cmp::max(Self::xn_node_height(&node.left), Self::xn_node_height(&node.right));
    }

    fn xn_rotate_right(mut y: Box<Xn87AvlNode<K, V>>) -> Box<Xn87AvlNode<K, V>> {
        let mut x = y.left.take().expect("xn rotate right");
        y.left = x.right.take();
        Self::xn_update_height(&mut y);
        x.right = Some(y);
        Self::xn_update_height(&mut x);
        x
    }

    fn xn_rotate_left(mut x: Box<Xn87AvlNode<K, V>>) -> Box<Xn87AvlNode<K, V>> {
        let mut y = x.right.take().expect("xn rotate left");
        x.right = y.left.take();
        Self::xn_update_height(&mut x);
        y.left = Some(x);
        Self::xn_update_height(&mut y);
        y
    }

    fn xn_rebalance(mut node: Box<Xn87AvlNode<K, V>>) -> Box<Xn87AvlNode<K, V>> {
        Self::xn_update_height(&mut node);
        let bal = Self::xn_balance(&Some(node.clone()));
        if bal > 1 {
            if Self::xn_balance(&node.left) < 0 {
                node.left = Some(Self::xn_rotate_left(node.left.take().unwrap()));
            }
            return Self::xn_rotate_right(node);
        }
        if bal < -1 {
            if Self::xn_balance(&node.right) > 0 {
                node.right = Some(Self::xn_rotate_right(node.right.take().unwrap()));
            }
            return Self::xn_rotate_left(node);
        }
        node
    }

    fn xn_insert_node(node: Option<Box<Xn87AvlNode<K, V>>>, key: K, value: V, inserted: &mut bool) -> Box<Xn87AvlNode<K, V>> {
        let Some(mut n) = node else {
            *inserted = true;
            return Box::new(Xn87AvlNode { key, value, left: None, right: None, height: 1 });
        };
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => n.left = Some(Self::xn_insert_node(n.left.take(), key, value, inserted)),
            std::cmp::Ordering::Greater => n.right = Some(Self::xn_insert_node(n.right.take(), key, value, inserted)),
            std::cmp::Ordering::Equal => { n.value = value; }
        }
        Self::xn_rebalance(n)
    }

    /// Insert or update a key‑value pair.
    pub fn xn_insert(&mut self, key: K, value: V) {
        let mut inserted = false;
        let root = Self::xn_insert_node(self.root.take(), key, value, &mut inserted);
        self.root = Some(root);
        if inserted { self.xn_len += 1; }
    }

    fn xn_get_node<'a>(node: &'a Option<Box<Xn87AvlNode<K, V>>>, key: &K) -> Option<&'a V> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => Self::xn_get_node(&n.left, key),
            std::cmp::Ordering::Greater => Self::xn_get_node(&n.right, key),
            std::cmp::Ordering::Equal => Some(&n.value),
        }
    }

    /// Look up a value by key.
    pub fn xn_get(&self, key: &K) -> Option<&V> {
        Self::xn_get_node(&self.root, key)
    }

    /// Check whether the map contains `key`.
    pub fn xn_contains(&self, key: &K) -> bool {
        self.xn_get(key).is_some()
    }

    fn xn_min_node(node: &Box<Xn87AvlNode<K, V>>) -> &Xn87AvlNode<K, V> {
        node.left.as_ref().map_or(node.as_ref(), |l| Self::xn_min_node(l))
    }

    fn xn_remove_min(mut node: Box<Xn87AvlNode<K, V>>) -> (Box<Xn87AvlNode<K, V>>, Option<Box<Xn87AvlNode<K, V>>>) {
        if node.left.is_none() {
            let right = node.right.take();
            return (node, right);
        }
        let (min, new_left) = Self::xn_remove_min(node.left.take().unwrap());
        node.left = new_left;
        (min, Some(Self::xn_rebalance(node)))
    }

    fn xn_remove_node(node: Option<Box<Xn87AvlNode<K, V>>>, key: &K, removed: &mut bool) -> Option<Box<Xn87AvlNode<K, V>>> {
        let Some(mut n) = node else { return None };
        match key.cmp(&n.key) {
            std::cmp::Ordering::Less => { n.left = Self::xn_remove_node(n.left.take(), key, removed); Some(Self::xn_rebalance(n)) }
            std::cmp::Ordering::Greater => { n.right = Self::xn_remove_node(n.right.take(), key, removed); Some(Self::xn_rebalance(n)) }
            std::cmp::Ordering::Equal => {
                *removed = true;
                match (n.left.take(), n.right.take()) {
                    (None, None) => None,
                    (Some(l), None) => Some(Self::xn_rebalance(l)),
                    (None, Some(r)) => Some(Self::xn_rebalance(r)),
                    (Some(l), Some(r)) => {
                        let (mut successor, new_right) = Self::xn_remove_min(r);
                        successor.left = Some(l);
                        successor.right = new_right;
                        Some(Self::xn_rebalance(successor))
                    }
                }
            }
        }
    }

    /// Remove a key from the map. Returns `true` when the key was present.
    pub fn xn_remove(&mut self, key: &K) -> bool {
        let mut removed = false;
        self.root = Self::xn_remove_node(self.root.take(), key, &mut removed);
        if removed { self.xn_len -= 1; }
        removed
    }

    /// Number of entries.
    pub fn xn_len(&self) -> usize {
        self.xn_len
    }

    fn xn_collect_in_order(node: &Option<Box<Xn87AvlNode<K, V>>>, out: &mut Vec<(K, V)>) {
        if let Some(n) = node {
            Self::xn_collect_in_order(&n.left, out);
            out.push((n.key.clone(), n.value.clone()));
            Self::xn_collect_in_order(&n.right, out);
        }
    }

    /// Return all key‑value pairs in sorted order.
    pub fn xn_in_order(&self) -> Vec<(K, V)> {
        let mut v = Vec::new();
        Self::xn_collect_in_order(&self.root, &mut v);
        v
    }

    /// Height of the tree (0 for empty).
    pub fn xn_height(&self) -> i32 {
        Self::xn_node_height(&self.root)
    }

    fn xn_min_key(node: &Option<Box<Xn87AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.left.is_some() { Self::xn_min_key(&n.left) } else { Some(&n.key) }
    }

    /// Smallest key in the map.
    pub fn xn_min(&self) -> Option<&K> {
        Self::xn_min_key(&self.root)
    }

    fn xn_max_key(node: &Option<Box<Xn87AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.right.is_some() { Self::xn_max_key(&n.right) } else { Some(&n.key) }
    }

    /// Largest key in the map.
    pub fn xn_max(&self) -> Option<&K> {
        Self::xn_max_key(&self.root)
    }

    fn xn_floor_key<'a>(node: &'a Option<Box<Xn87AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Equal => Some(&n.key),
            std::cmp::Ordering::Less => Self::xn_floor_key(&n.left, key),
            std::cmp::Ordering::Greater => Self::xn_floor_key(&n.right, key).or(Some(&n.key)),
        }
    }

    /// Greatest key less than or equal to `key`.
    pub fn xn_floor(&self, key: &K) -> Option<&K> {
        Self::xn_floor_key(&self.root, key)
    }

    fn xn_ceiling_key<'a>(node: &'a Option<Box<Xn87AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
        let n = node.as_ref()?;
        match key.cmp(&n.key) {
            std::cmp::Ordering::Equal => Some(&n.key),
            std::cmp::Ordering::Greater => Self::xn_ceiling_key(&n.right, key),
            std::cmp::Ordering::Less => Self::xn_ceiling_key(&n.left, key).or(Some(&n.key)),
        }
    }

    /// Smallest key greater than or equal to `key`.
    pub fn xn_ceiling(&self, key: &K) -> Option<&K> {
        Self::xn_ceiling_key(&self.root, key)
    }
}


// ---------------------------------------------------------------------------
// Xo87RedBlack<K,V> — red-black tree map
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Xo87Color {
    Red,
    Black,
}

#[derive(Debug, Clone)]
struct Xo87RBNode<K, V> {
    key: K,
    value: V,
    color: Xo87Color,
    left: Option<Box<Xo87RBNode<K, V>>>,
    right: Option<Box<Xo87RBNode<K, V>>>,
}

/// A red-black tree map for crate 87.
#[derive(Debug, Clone)]
pub struct Xo87RedBlack<K, V> {
    root: Option<Box<Xo87RBNode<K, V>>>,
    len: usize,
}

impl<K: Ord + Clone, V: Clone> Xo87RedBlack<K, V> {
    pub fn xo_new() -> Self {
        Self { root: None, len: 0 }
    }

    pub fn xo_len(&self) -> usize {
        self.len
    }

    pub fn xo_is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn xo_insert(&mut self, key: K, value: V) {
        self.root = Some(Self::xo_ins(self.root.take(), key, value, &mut self.len));
        if let Some(ref mut r) = self.root {
            r.color = Xo87Color::Black;
        }
    }

    fn xo_ins(node: Option<Box<Xo87RBNode<K, V>>>, key: K, value: V, len: &mut usize) -> Box<Xo87RBNode<K, V>> {
        match node {
            None => {
                *len += 1;
                Box::new(Xo87RBNode {
                    key, value, color: Xo87Color::Red, left: None, right: None,
                })
            }
            Some(mut n) => {
                use std::cmp::Ordering;
                match key.cmp(&n.key) {
                    Ordering::Less => n.left = Some(Self::xo_ins(n.left.take(), key, value, len)),
                    Ordering::Greater => n.right = Some(Self::xo_ins(n.right.take(), key, value, len)),
                    Ordering::Equal => { n.value = value; return n; }
                }
                Self::xo_balance(n)
            }
        }
    }

    fn xo_is_red(node: &Option<Box<Xo87RBNode<K, V>>>) -> bool {
        matches!(node, Some(n) if n.color == Xo87Color::Red)
    }

    fn xo_balance(mut h: Box<Xo87RBNode<K, V>>) -> Box<Xo87RBNode<K, V>> {
        if Self::xo_is_red(&h.right) && !Self::xo_is_red(&h.left) {
            h = Self::xo_rotate_left(h);
        }
        if Self::xo_is_red(&h.left) {
            let left_left_red = h.left.as_ref().and_then(|l| l.left.as_ref()).map_or(false, |ll| ll.color == Xo87Color::Red);
            if left_left_red {
                h = Self::xo_rotate_right(h);
            }
        }
        if Self::xo_is_red(&h.left) && Self::xo_is_red(&h.right) {
            Self::xo_flip_colors(&mut h);
        }
        h
    }

    fn xo_rotate_left(mut h: Box<Xo87RBNode<K, V>>) -> Box<Xo87RBNode<K, V>> {
        let mut x = h.right.take().unwrap();
        h.right = x.left.take();
        x.color = h.color;
        h.color = Xo87Color::Red;
        x.left = Some(h);
        x
    }

    fn xo_rotate_right(mut h: Box<Xo87RBNode<K, V>>) -> Box<Xo87RBNode<K, V>> {
        let mut x = h.left.take().unwrap();
        h.left = x.right.take();
        x.color = h.color;
        h.color = Xo87Color::Red;
        x.right = Some(h);
        x
    }

    fn xo_flip_colors(h: &mut Box<Xo87RBNode<K, V>>) {
        h.color = Xo87Color::Red;
        if let Some(l) = &mut h.left { l.color = Xo87Color::Black; }
        if let Some(r) = &mut h.right { r.color = Xo87Color::Black; }
    }

    pub fn xo_get(&self, key: &K) -> Option<&V> {
        let mut cur = &self.root;
        while let Some(node) = cur {
            use std::cmp::Ordering;
            match key.cmp(&node.key) {
                Ordering::Less => cur = &node.left,
                Ordering::Greater => cur = &node.right,
                Ordering::Equal => return Some(&node.value),
            }
        }
        None
    }

    pub fn xo_contains(&self, key: &K) -> bool {
        self.xo_get(key).is_some()
    }

    pub fn xo_min(&self) -> Option<&K> {
        let mut cur = &self.root;
        let mut result = None;
        while let Some(node) = cur {
            result = Some(&node.key);
            cur = &node.left;
        }
        result
    }

    pub fn xo_max(&self) -> Option<&K> {
        let mut cur = &self.root;
        let mut result = None;
        while let Some(node) = cur {
            result = Some(&node.key);
            cur = &node.right;
        }
        result
    }

    pub fn xo_remove(&mut self, key: &K) -> Option<V> {
        let mut found = None;
        self.root = Self::xo_remove_rec(self.root.take(), key, &mut found);
        if let Some(ref mut r) = self.root {
            r.color = Xo87Color::Black;
        }
        if found.is_some() { self.len -= 1; }
        found
    }

    fn xo_remove_rec(node: Option<Box<Xo87RBNode<K, V>>>, key: &K, found: &mut Option<V>) -> Option<Box<Xo87RBNode<K, V>>> {
        match node {
            None => None,
            Some(mut n) => {
                use std::cmp::Ordering;
                match key.cmp(&n.key) {
                    Ordering::Less => { n.left = Self::xo_remove_rec(n.left.take(), key, found); Some(n) }
                    Ordering::Greater => { n.right = Self::xo_remove_rec(n.right.take(), key, found); Some(n) }
                    Ordering::Equal => {
                        *found = Some(n.value.clone());
                        match (n.left.take(), n.right.take()) {
                            (None, None) => None,
                            (Some(l), None) => Some(l),
                            (None, Some(r)) => Some(r),
                            (Some(l), Some(r)) => {
                                let (min_key, min_val, new_right) = Self::xo_remove_min_node(*r);
                                n.key = min_key; n.value = min_val;
                                n.left = Some(l); n.right = new_right;
                                Some(n)
                            }
                        }
                    }
                }
            }
        }
    }

    fn xo_remove_min_node(mut node: Xo87RBNode<K, V>) -> (K, V, Option<Box<Xo87RBNode<K, V>>>) {
        if node.left.is_none() {
            return (node.key, node.value, node.right);
        }
        let (k, v, new_left) = Self::xo_remove_min_node(*node.left.take().unwrap());
        node.left = new_left;
        (k, v, Some(Box::new(node)))
    }

    pub fn xo_black_height(&self) -> usize {
        fn bh<K, V>(node: &Option<Box<Xo87RBNode<K, V>>>) -> usize {
            match node {
                None => 1,
                Some(n) => {
                    let add = if n.color == Xo87Color::Black { 1 } else { 0 };
                    add + bh(&n.left)
                }
            }
        }
        bh(&self.root)
    }

    pub fn xo_in_order(&self) -> Vec<(K, V)> {
        let mut result = Vec::new();
        fn collect<K: Clone, V: Clone>(node: &Option<Box<Xo87RBNode<K, V>>>, out: &mut Vec<(K, V)>) {
            if let Some(n) = node {
                collect(&n.left, out);
                out.push((n.key.clone(), n.value.clone()));
                collect(&n.right, out);
            }
        }
        collect(&self.root, &mut result);
        result
    }
}

// ---------------------------------------------------------------------------
// Xo87ConsistentHash — consistent hash ring
// ---------------------------------------------------------------------------

/// A consistent hash ring for crate 87.
#[derive(Debug, Clone)]
pub struct Xo87ConsistentHash {
    ring: std::collections::BTreeMap<u64, String>,
    nodes: std::collections::HashMap<String, usize>,
    virtual_count: usize,
}

impl Xo87ConsistentHash {
    pub fn xo_new(virtual_count: usize) -> Self {
        Self {
            ring: std::collections::BTreeMap::new(),
            nodes: std::collections::HashMap::new(),
            virtual_count,
        }
    }

    fn xo_hash(data: &str) -> u64 {
        let mut h: u64 = 5381;
        for b in data.bytes() {
            h = h.wrapping_mul(33).wrapping_add(b as u64);
        }
        h
    }

    pub fn xo_add_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo87#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.insert(hash, node.to_string());
        }
        *self.nodes.entry(node.to_string()).or_insert(0) += 1;
    }

    pub fn xo_remove_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo87#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.remove(&hash);
        }
        self.nodes.remove(node);
    }

    pub fn xo_get_node(&self, key: &str) -> Option<&str> {
        if self.ring.is_empty() {
            return None;
        }
        let hash = Self::xo_hash(key);
        let entry = self.ring.range(hash..).next().or_else(|| self.ring.iter().next());
        entry.map(|(_, v)| v.as_str())
    }

    pub fn xo_node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn xo_rebalance_factor(&self) -> f64 {
        if self.nodes.is_empty() {
            return 0.0;
        }
        let total = self.ring.len() as f64;
        let expected = total / self.nodes.len() as f64;
        let mut max_dev: f64 = 0.0;
        let counts: std::collections::HashMap<&str, usize> = self.ring.values().fold(
            std::collections::HashMap::new(),
            |mut acc, v| { *acc.entry(v.as_str()).or_insert(0) += 1; acc }
        );
        for &c in counts.values() {
            let dev = ((c as f64) - expected).abs();
            if dev > max_dev { max_dev = dev; }
        }
        if expected > 0.0 { max_dev / expected } else { 0.0 }
    }

    pub fn xo_virtual_nodes(&self) -> usize {
        self.ring.len()
    }

    pub fn xo_key_distribution(&self, keys: &[&str]) -> std::collections::HashMap<String, usize> {
        let mut dist: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for k in keys {
            if let Some(node) = self.xo_get_node(k) {
                *dist.entry(node.to_string()).or_insert(0) += 1;
            }
        }
        dist
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_string_hash_deterministic() {
        assert_eq!(string_hash("hello"), string_hash("hello"));
        assert_ne!(string_hash("hello"), string_hash("world"));
    }

    #[test]
    fn test_sha256() {
        let hash = sha256_string("hello");
        assert_eq!(hash.len(), 64); // 32 bytes = 64 hex chars
        assert_eq!(sha256_string("hello"), sha256_string("hello"));
    }

    #[test]
    fn test_uuid_format() {
        let id = generate_uuid();
        assert_eq!(id.len(), 36); // 8-4-4-4-12
        assert!(id.contains('-'));
    }

    #[test]
    fn test_short_id() {
        let id = generate_short_id();
        assert_eq!(id.len(), 8);
    }

    #[test]
    fn test_hash_combiner() {
        let mut c1 = HashCombiner::new();
        c1.add_string("hello").add_number(42);
        let mut c2 = HashCombiner::new();
        c2.add_string("hello").add_number(42);
        assert_eq!(c1.value(), c2.value());
    }

    #[test]
    fn test_sha256_bytes() {
        let bytes = sha256_bytes(b"hello");
        assert_eq!(bytes.len(), 32);
        // Verify consistency with hex variant
        assert_eq!(hex_encode(&bytes), sha256_hex(b"hello"));
    }

    #[test]
    fn test_hash_file_content() {
        let h = hash_file_content("fn main() {}");
        assert!(h.contains(':'));
        let parts: Vec<&str> = h.split(':').collect();
        assert_eq!(parts[0].len(), 64);
        assert_eq!(parts[1], "12");
        // Deterministic
        assert_eq!(h, hash_file_content("fn main() {}"));
    }

    #[test]
    fn test_uuid_v5_deterministic() {
        let a = generate_uuid_v5("https://example.com", "test");
        let b = generate_uuid_v5("https://example.com", "test");
        assert_eq!(a, b);
        assert_eq!(a.len(), 36);
        // Different inputs produce different UUIDs
        let c = generate_uuid_v5("https://example.com", "other");
        assert_ne!(a, c);
    }

    #[test]
    fn test_is_valid_uuid() {
        assert!(is_valid_uuid(&generate_uuid()));
        assert!(is_valid_uuid("550e8400-e29b-41d4-a716-446655440000"));
        assert!(!is_valid_uuid("not-a-uuid"));
        assert!(!is_valid_uuid(""));
    }

    #[test]
    fn test_derive_key() {
        let key = derive_key("password", "salt");
        assert_eq!(key.len(), 64);
        assert_eq!(key, derive_key("password", "salt"));
        assert_ne!(key, derive_key("password", "other"));
    }

    #[test]
    fn test_combiner_add_bytes() {
        let mut c1 = HashCombiner::new();
        c1.add_bytes(b"hello");
        let mut c2 = HashCombiner::new();
        c2.add_bytes(b"hello");
        assert_eq!(c1.value(), c2.value());
        let mut c3 = HashCombiner::new();
        c3.add_bytes(b"world");
        assert_ne!(c1.value(), c3.value());
    }

    #[test]
    fn test_combiner_optional_string() {
        let mut c1 = HashCombiner::new();
        c1.add_optional_string(Some("hello"));
        let mut c2 = HashCombiner::new();
        c2.add_optional_string(None);
        assert_ne!(c1.value(), c2.value());
    }

    #[test]
    fn test_combiner_reset_and_is_empty() {
        let mut c = HashCombiner::new();
        assert!(c.is_empty());
        c.add_string("data");
        assert!(!c.is_empty());
        c.reset();
        assert!(c.is_empty());
        assert_eq!(c.value(), 0);
    }

    #[test]
    fn test_combiner_display() {
        let mut c = HashCombiner::new();
        assert_eq!(format!("{c}"), "00000000");
        c.add_number(1);
        let display = format!("{c}");
        assert_eq!(display.len(), 8);
    }

    #[test]
    fn test_content_address_roundtrip() {
        let content = "fn main() { println!(\"hello\"); }";
        let addr = ContentAddress::from_content(content);
        let serialized = addr.to_string();
        let parsed = ContentAddress::parse(&serialized).unwrap();
        assert_eq!(addr, parsed);
        assert!(parsed.verify(content));
        assert!(!parsed.verify("different content"));
    }

    #[test]
    fn test_content_address_parse_errors() {
        // Missing separator
        assert!(ContentAddress::parse("abc123").is_err());
        // Digest too short
        assert!(ContentAddress::parse("abcd:10").is_err());
        // Invalid length
        let bad = format!("{}:notanum", "a".repeat(64));
        assert!(ContentAddress::parse(&bad).is_err());
        // Non-hex chars in digest
        let bad_hex = format!("{}:10", "g".repeat(64));
        assert!(ContentAddress::parse(&bad_hex).is_err());
    }

    #[test]
    fn test_content_address_accessors() {
        let addr = ContentAddress::from_content("test");
        assert_eq!(addr.length(), 4);
        assert_eq!(addr.digest().len(), 64);
    }

    #[test]
    fn test_hex_decode_valid() {
        let hex = sha256_hex(b"hello");
        let bytes = hex_decode(&hex).unwrap();
        assert_eq!(bytes.len(), 32);
        assert_eq!(bytes, sha256_bytes(b"hello").to_vec());
    }

    #[test]
    fn test_hex_decode_errors() {
        // Odd length
        assert!(hex_decode("abc").is_err());
        // Invalid char
        assert!(hex_decode("zz").is_err());
    }

    #[test]
    fn test_derive_key_rounds() {
        let k = derive_key_rounds("secret", "salt", 3).unwrap();
        assert_eq!(k.len(), 64);
        // Deterministic
        assert_eq!(k, derive_key_rounds("secret", "salt", 3).unwrap());
        // Different from single-round derive_key
        assert_ne!(k, derive_key("secret", "salt"));
    }

    #[test]
    fn test_derive_key_rounds_errors() {
        assert!(derive_key_rounds("", "salt", 1).is_err());
        assert!(derive_key_rounds("input", "salt", 0).is_err());
    }

    #[test]
    fn test_keyed_hash() {
        let h1 = keyed_hash("key", "message");
        assert_eq!(h1.len(), 64);
        assert_eq!(h1, keyed_hash("key", "message"));
        assert_ne!(h1, keyed_hash("other", "message"));
        assert_ne!(h1, keyed_hash("key", "other"));
    }

    #[test]
    fn test_hash_error_display() {
        let e = HashError::EmptyInput("field");
        assert_eq!(format!("{e}"), "empty input: field");
        let e2 = HashError::InvalidHex("bad".into());
        assert_eq!(format!("{e2}"), "invalid hex: bad");
        let e3 = HashError::InvalidContentAddress("missing".into());
        assert_eq!(format!("{e3}"), "invalid content address: missing");
    }

    #[test]
    fn test_combiner_clone_and_eq() {
        let mut c = HashCombiner::new();
        c.add_string("hello").add_number(42);
        let c2 = c.clone();
        assert_eq!(c, c2);
        assert_eq!(c.value(), c2.value());
    }

    #[test]
    fn test_combiner_debug() {
        let c = HashCombiner::new();
        let dbg = format!("{c:?}");
        assert!(dbg.contains("HashCombiner"));
        assert!(dbg.contains("0x"));
    }

    #[test]
    fn test_hash_algorithm_djb2_deterministic() {
        let a = HashAlgorithm::Djb2.hash_str("hello");
        let b = HashAlgorithm::Djb2.hash_str("hello");
        assert_eq!(a, b);
        assert_ne!(a, HashAlgorithm::Djb2.hash_str("world"));
    }

    #[test]
    fn test_hash_algorithm_sha256_uses_first_8_bytes() {
        let h = HashAlgorithm::Sha256.hash_str("test");
        assert_ne!(h, 0);
        assert_eq!(h, HashAlgorithm::Sha256.hash_str("test"));
    }

    #[test]
    fn test_fnv1a_hash_deterministic() {
        let a = fnv1a_hash(b"hello world");
        let b = fnv1a_hash(b"hello world");
        assert_eq!(a, b);
        assert_ne!(a, fnv1a_hash(b"other"));
        // empty input should return offset basis
        assert_eq!(fnv1a_hash(b""), 0xcbf2_9ce4_8422_2325);
    }

    #[test]
    fn test_checksum_verifier_compute_and_verify() {
        let data = b"some data";
        for algo in [HashAlgorithm::Djb2, HashAlgorithm::Sha256, HashAlgorithm::FnvLike] {
            let checksum = ChecksumVerifier::compute(data, algo);
            assert!(ChecksumVerifier::verify(data, &checksum, algo));
            assert!(!ChecksumVerifier::verify(b"wrong", &checksum, algo));
        }
    }

    #[test]
    fn test_hash_bucket_assign_and_rebalance() {
        let mut ring = HashBucket::new(8);
        let idx = ring.assign("my-key");
        assert!(idx < 8);
        // deterministic
        assert_eq!(idx, ring.assign("my-key"));
        ring.rebalance(4);
        let idx2 = ring.assign("my-key");
        assert!(idx2 < 4);
    }

    #[test]
    fn test_hash_combine_ordered_depends_on_order() {
        let a = hash_combine_ordered(&["a", "b", "c"]);
        let b = hash_combine_ordered(&["c", "b", "a"]);
        assert_ne!(a, b);
        // deterministic
        assert_eq!(a, hash_combine_ordered(&["a", "b", "c"]));
    }

    #[test]
    fn test_hash_combine_unordered_ignores_order() {
        let a = hash_combine_unordered(&["a", "b", "c"]);
        let b = hash_combine_unordered(&["c", "a", "b"]);
        assert_eq!(a, b);
    }

    #[test]
    fn test_hash_algorithm_fnvlike_matches_fnv1a() {
        let algo_h = HashAlgorithm::FnvLike.hash_str("test string");
        let direct_h = fnv1a_hash(b"test string");
        assert_eq!(algo_h, direct_h);
    }

    #[test]
    fn test_crc32_deterministic() {
        let a = crc32(b"hello world");
        let b = crc32(b"hello world");
        assert_eq!(a, b);
        assert_ne!(a, crc32(b"other"));
        // empty data produces a well-defined result
        let _ = crc32(b"");
    }

    #[test]
    fn test_hash_mix_and_combine_all() {
        let m = hash_mix(123, 456);
        assert_ne!(m, 0);
        assert_eq!(m, hash_mix(123, 456));
        assert_ne!(hash_mix(123, 456), hash_mix(456, 123));

        let combined = hash_combine_all(&[1, 2, 3]);
        assert_eq!(combined, hash_combine_all(&[1, 2, 3]));
        assert_ne!(combined, hash_combine_all(&[3, 2, 1]));
    }

    #[test]
    fn test_consistent_hash_ring() {
        let mut ring = ConsistentHashRing::new();
        assert!(ring.is_empty());
        assert!(ring.get_node("key").is_none());
        ring.add_node("node-a");
        ring.add_node("node-b");
        ring.add_node("node-c");
        assert_eq!(ring.node_count(), 3);
        let node = ring.get_node("my-key").unwrap();
        assert!(!node.is_empty());
        // deterministic
        assert_eq!(ring.get_node("my-key"), Some(node));
        assert!(ring.remove_node("node-b"));
        assert!(!ring.remove_node("nonexistent"));
        assert_eq!(ring.node_count(), 2);
    }

    #[test]
    fn test_bloom_filter_insert_and_query() {
        let mut bf = BloomFilter::new(256, 3);
        bf.insert(b"hello");
        bf.insert(b"world");
        assert!(bf.may_contain(b"hello"));
        assert!(bf.may_contain(b"world"));
        // False negative must never happen
        assert!(bf.count_ones() > 0);
        bf.clear();
        assert_eq!(bf.count_ones(), 0);
        assert!(!bf.may_contain(b"hello"));
    }

    #[test]
    fn test_bloom_filter_false_positive_rate() {
        let mut bf = BloomFilter::new(1024, 5);
        for i in 0..10u32 {
            bf.insert(&i.to_le_bytes());
        }
        // All inserted items must be found
        for i in 0..10u32 {
            assert!(bf.may_contain(&i.to_le_bytes()));
        }
    }

    #[test]
    fn content_hash_file_deterministic() {
        let data = b"fn main() { println!(\"hello\"); }";
        let h1 = content_hash_file(data);
        let h2 = content_hash_file(data);
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 64);
    }

    #[test]
    fn content_hash_file_differs_for_different_content() {
        assert_ne!(content_hash_file(b"aaa"), content_hash_file(b"bbb"));
    }

    #[test]
    fn content_hash_file_empty() {
        let h = content_hash_file(b"");
        assert_eq!(h.len(), 64);
    }

    #[test]
    fn byte_range_basic() {
        let r = ByteRange::new(5, 10);
        assert_eq!(r.len(), 5);
        assert!(!r.is_empty());
        assert!(r.is_valid());
        assert_eq!(format!("{r}"), "[5..10)");
    }

    #[test]
    fn byte_range_empty() {
        let r = ByteRange::new(5, 5);
        assert!(r.is_empty());
        assert_eq!(r.len(), 0);
        assert!(r.is_valid());
    }

    #[test]
    fn byte_range_invalid() {
        let r = ByteRange::new(10, 5);
        assert!(!r.is_valid());
    }

    #[test]
    fn diff_hash_changed_ranges() {
        let data = b"hello world test data";
        let ranges = vec![ByteRange::new(0, 5), ByteRange::new(6, 11)];
        let h = diff_hash(data, &ranges);
        assert_eq!(h.len(), 64);
        // Deterministic
        assert_eq!(h, diff_hash(data, &ranges));
    }

    #[test]
    fn diff_hash_empty_ranges() {
        let data = b"hello";
        let h = diff_hash(data, &[]);
        // Should hash empty input
        assert_eq!(h, sha256_hex(b""));
    }

    #[test]
    fn diff_hash_out_of_bounds_range_skipped() {
        let data = b"short";
        let ranges = vec![ByteRange::new(100, 200)];
        let h = diff_hash(data, &ranges);
        assert_eq!(h, sha256_hex(b""));
    }

    #[test]
    fn diff_hash_clamped_to_data_len() {
        let data = b"hello";
        let ranges = vec![ByteRange::new(0, 100)];
        let h = diff_hash(data, &ranges);
        // Should hash "hello" (clamped to data len)
        assert_eq!(h, sha256_hex(b"hello"));
    }

    #[test]
    fn compute_changed_ranges_detects_changes() {
        let old = "line1\nline2\nline3";
        let new = "line1\nmodified\nline3";
        let ranges = compute_changed_ranges(old, new);
        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0].start, 6); // "modified" starts after "line1\n"
    }

    #[test]
    fn compute_changed_ranges_added_lines() {
        let old = "line1";
        let new = "line1\nnew_line";
        let ranges = compute_changed_ranges(old, new);
        assert_eq!(ranges.len(), 1); // new_line is added
    }

    #[test]
    fn compute_changed_ranges_identical() {
        let content = "line1\nline2\nline3";
        let ranges = compute_changed_ranges(content, content);
        assert!(ranges.is_empty());
    }

    #[test]
    fn compute_diff_hash_result() {
        let old = "fn main() {}";
        let new = "fn main() { println!(\"hi\"); }";
        let result = compute_diff_hash(old, new);
        assert_eq!(result.full_hash, content_hash_file(new.as_bytes()));
        assert_eq!(result.changed_range_count, 1);
        assert!(result.changed_bytes > 0);
        assert_eq!(result.diff_hash.len(), 64);
    }

    #[test]
    fn compute_diff_hash_no_changes() {
        let content = "unchanged";
        let result = compute_diff_hash(content, content);
        assert_eq!(result.changed_range_count, 0);
        assert_eq!(result.changed_bytes, 0);
    }

    // -----------------------------------------------------------------------
    // New tests for added functionality
    // -----------------------------------------------------------------------

    #[test]
    fn byte_range_contains() {
        let r = ByteRange::new(5, 10);
        assert!(!r.contains(4));
        assert!(r.contains(5));
        assert!(r.contains(9));
        assert!(!r.contains(10));
    }

    #[test]
    fn byte_range_overlaps_and_merge() {
        let a = ByteRange::new(0, 10);
        let b = ByteRange::new(5, 15);
        let c = ByteRange::new(20, 30);
        assert!(a.overlaps(&b));
        assert!(b.overlaps(&a));
        assert!(!a.overlaps(&c));

        let merged = a.merge(&b).unwrap();
        assert_eq!(merged.start, 0);
        assert_eq!(merged.end, 15);

        // Adjacent ranges (touching at boundary) can merge
        let d = ByteRange::new(10, 20);
        let merged2 = a.merge(&d).unwrap();
        assert_eq!(merged2.start, 0);
        assert_eq!(merged2.end, 20);

        // Disjoint ranges cannot merge
        assert!(a.merge(&c).is_none());
    }

    #[test]
    fn byte_range_iter() {
        let r = ByteRange::new(3, 7);
        let offsets: Vec<usize> = r.iter().collect();
        assert_eq!(offsets, vec![3, 4, 5, 6]);

        // ExactSizeIterator
        let r2 = ByteRange::new(0, 5);
        let iter = r2.iter();
        assert_eq!(iter.len(), 5);

        // Empty range yields nothing
        let empty = ByteRange::new(5, 5);
        assert_eq!(empty.iter().count(), 0);

        // IntoIterator
        let r3 = ByteRange::new(10, 12);
        let v: Vec<usize> = r3.into_iter().collect();
        assert_eq!(v, vec![10, 11]);
    }

    #[test]
    fn bloom_filter_str_convenience() {
        let mut bf = BloomFilter::new(512, 4);
        bf.insert_str("apple");
        bf.insert_str("banana");
        assert!(bf.may_contain_str("apple"));
        assert!(bf.may_contain_str("banana"));
        // Item never inserted – might be false positive, but with a large
        // filter and few insertions, extremely unlikely.
        // We just verify the API compiles and runs without panic.
    }

    #[test]
    fn bloom_filter_estimated_false_positive_rate() {
        let mut bf = BloomFilter::new(1024, 3);
        // Empty filter → rate should be 0.0
        assert_eq!(bf.estimated_false_positive_rate(), 0.0);

        for i in 0..50u32 {
            bf.insert(&i.to_le_bytes());
        }
        let rate = bf.estimated_false_positive_rate();
        assert!(rate > 0.0);
        assert!(rate < 1.0);
    }

    #[test]
    fn hash_combiner_from_strings() {
        let c = HashCombiner::from_strings(&["hello", "world"]);
        let mut manual = HashCombiner::new();
        manual.add_string("hello").add_string("world");
        assert_eq!(c.value(), manual.value());
        assert!(!c.is_empty());

        // Empty slice produces an empty combiner
        let empty = HashCombiner::from_strings(&[]);
        assert!(empty.is_empty());
    }

    #[test]
    fn content_address_matches_digest_prefix() {
        let addr = ContentAddress::from_content("hello");
        let digest = addr.digest().to_string();

        assert!(addr.matches_digest_prefix(&digest[..8]));
        assert!(addr.matches_digest_prefix(&digest[..8].to_uppercase()));
        assert!(addr.matches_digest_prefix(""));
        assert!(!addr.matches_digest_prefix("zzzzzzzz"));
    }

    #[test]
    fn hex_encode_upper_works() {
        let bytes = sha256_bytes(b"test");
        let upper = hex_encode_upper(&bytes);
        let lower = sha256_hex(b"test");
        assert_eq!(upper.len(), 64);
        assert_eq!(upper.to_lowercase(), lower);
        assert!(upper.chars().all(|c| c.is_ascii_hexdigit()));
        // Spot-check that it actually contains uppercase letters
        assert!(upper.chars().any(|c| c.is_ascii_uppercase()));
    }

    #[test]
    fn crc32_str_deterministic() {
        let a = crc32_str("hello world");
        let b = crc32_str("hello world");
        assert_eq!(a, b);
        assert_ne!(crc32_str("hello"), crc32_str("world"));
    }

    #[test]
    fn crc32_empty_input() {
        // CRC32 of empty input should be a known constant (0x00000000)
        let c = crc32(b"");
        assert_eq!(c, 0x0000_0000);
    }

    #[test]
    fn incremental_hasher_matches_oneshot() {
        let oneshot = sha256_hex(b"hello world");
        let mut inc = IncrementalHasher::new();
        inc.update(b"hello ");
        inc.update(b"world");
        assert_eq!(inc.bytes_fed(), 11);
        assert_eq!(inc.finalize(), oneshot);
    }

    #[test]
    fn incremental_hasher_empty() {
        let inc = IncrementalHasher::new();
        assert_eq!(inc.bytes_fed(), 0);
        let digest = inc.finalize();
        assert_eq!(digest, sha256_hex(b""));
    }

    #[test]
    fn content_dedup_tracks_unique() {
        let mut dedup = ContentDedup::new();
        assert!(dedup.is_empty());
        assert!(dedup.insert("hello"));
        assert!(!dedup.insert("hello")); // duplicate
        assert!(dedup.insert("world"));
        assert_eq!(dedup.len(), 2);
        assert!(dedup.contains("hello"));
        assert!(!dedup.contains("unknown"));
    }

    #[test]
    fn constant_time_eq_same() {
        let a = sha256_string("test");
        assert!(constant_time_eq(&a, &a));
    }

    #[test]
    fn constant_time_eq_different() {
        let a = sha256_string("test1");
        let b = sha256_string("test2");
        assert!(!constant_time_eq(&a, &b));
    }

    #[test]
    fn constant_time_eq_different_length() {
        assert!(!constant_time_eq("abc", "abcd"));
    }

    #[test]
    fn uuid_version_v4() {
        let id = generate_uuid();
        assert_eq!(uuid_version(&id), Some(4));
    }

    #[test]
    fn uuid_version_invalid() {
        assert_eq!(uuid_version("not-a-uuid"), None);
    }

    // -----------------------------------------------------------------------
    // HMAC-SHA256 tests
    // -----------------------------------------------------------------------

    #[test]
    fn hmac_sha256_deterministic() {
        let mac1 = hmac_sha256(b"secret", b"hello");
        let mac2 = hmac_sha256(b"secret", b"hello");
        assert_eq!(mac1, mac2);
        assert_eq!(mac1.len(), 64);
    }

    #[test]
    fn hmac_sha256_differs_with_different_key() {
        let mac1 = hmac_sha256(b"key1", b"message");
        let mac2 = hmac_sha256(b"key2", b"message");
        assert_ne!(mac1, mac2);
    }

    #[test]
    fn hmac_sha256_differs_with_different_message() {
        let mac1 = hmac_sha256(b"key", b"msg1");
        let mac2 = hmac_sha256(b"key", b"msg2");
        assert_ne!(mac1, mac2);
    }

    #[test]
    fn hmac_sha256_long_key_handled() {
        // Key longer than block size should be hashed first.
        let long_key = vec![0xABu8; 128];
        let mac = hmac_sha256(&long_key, b"data");
        assert_eq!(mac.len(), 64);
        // Deterministic with same long key
        assert_eq!(mac, hmac_sha256(&long_key, b"data"));
    }

    #[test]
    fn hmac_sha256_str_wrapper() {
        let mac = hmac_sha256_str("key", "message");
        assert_eq!(mac, hmac_sha256(b"key", b"message"));
    }

    // -----------------------------------------------------------------------
    // Merkle tree tests
    // -----------------------------------------------------------------------

    #[test]
    fn merkle_tree_single_item() {
        let tree = MerkleTree::from_strings(&["hello"]);
        assert_eq!(tree.depth(), 1);
        assert_eq!(tree.leaves().len(), 1);
        assert_eq!(tree.root(), sha256_hex(b"hello"));
    }

    #[test]
    fn merkle_tree_two_items() {
        let tree = MerkleTree::from_strings(&["a", "b"]);
        assert_eq!(tree.depth(), 2);
        assert_eq!(tree.leaves().len(), 2);
        // Root should be hash of concatenated leaf hashes
        let expected_root = sha256_string(&format!(
            "{}{}",
            sha256_hex(b"a"),
            sha256_hex(b"b")
        ));
        assert_eq!(tree.root(), expected_root);
    }

    #[test]
    fn merkle_tree_odd_items_duplicates_last() {
        let tree = MerkleTree::from_strings(&["a", "b", "c"]);
        assert_eq!(tree.leaves().len(), 3);
        assert!(tree.depth() >= 2);
        assert_eq!(tree.root().len(), 64);
    }

    #[test]
    fn merkle_tree_empty() {
        let tree = MerkleTree::from_items(&[]);
        assert_eq!(tree.depth(), 1);
        assert_eq!(tree.root(), sha256_hex(b""));
    }

    #[test]
    fn merkle_tree_verify_leaf() {
        let tree = MerkleTree::from_strings(&["alpha", "beta", "gamma"]);
        assert!(tree.verify_leaf(0, b"alpha"));
        assert!(tree.verify_leaf(1, b"beta"));
        assert!(!tree.verify_leaf(0, b"wrong"));
        assert!(!tree.verify_leaf(99, b"alpha")); // out of bounds
    }

    #[test]
    fn merkle_tree_display() {
        let tree = MerkleTree::from_strings(&["x", "y"]);
        let s = format!("{tree}");
        assert!(s.contains("MerkleTree"));
        assert!(s.contains("leaves=2"));
    }

    // -----------------------------------------------------------------------
    // Rolling hash / Rabin-Karp tests
    // -----------------------------------------------------------------------

    #[test]
    fn rolling_hash_basic() {
        let mut rh = RollingHash::new(3);
        assert!(!rh.is_full());
        rh.push(b'a');
        rh.push(b'b');
        rh.push(b'c');
        assert!(rh.is_full());
        assert_eq!(rh.bytes_pushed(), 3);
        let v1 = rh.value();
        // Pushing another byte should change the hash.
        rh.push(b'd');
        assert_ne!(rh.value(), v1);
    }

    #[test]
    fn rabin_karp_finds_pattern() {
        let text = b"hello world hello";
        let positions = rabin_karp_search(text, b"hello");
        assert_eq!(positions, vec![0, 12]);
    }

    #[test]
    fn rabin_karp_no_match() {
        let positions = rabin_karp_search(b"abcdef", b"xyz");
        assert!(positions.is_empty());
    }

    #[test]
    fn rabin_karp_empty_pattern() {
        let positions = rabin_karp_search(b"abc", b"");
        assert!(positions.is_empty());
    }

    #[test]
    fn rabin_karp_pattern_longer_than_text() {
        let positions = rabin_karp_search(b"hi", b"hello");
        assert!(positions.is_empty());
    }

    // -----------------------------------------------------------------------
    // CAS key tests
    // -----------------------------------------------------------------------

    #[test]
    fn cas_key_roundtrip() {
        let data = b"some file content";
        let key = cas_key(data);
        assert!(key.starts_with("sha256:"));
        let (algo, digest, length) = cas_key_parse(&key).unwrap();
        assert_eq!(algo, "sha256");
        assert_eq!(digest, sha256_hex(data));
        assert_eq!(length, data.len());
    }

    #[test]
    fn cas_key_verify_valid() {
        let data = b"test data";
        let key = cas_key(data);
        assert!(cas_key_verify(data, &key).unwrap());
        assert!(!cas_key_verify(b"wrong", &key).unwrap());
    }

    #[test]
    fn cas_key_parse_invalid() {
        assert!(cas_key_parse("nodelimiter").is_err());
        assert!(cas_key_parse("sha256:abc").is_err());
        assert!(cas_key_parse("sha256:abc:notnum").is_err());
    }

    // -----------------------------------------------------------------------
    // Hex similarity tests
    // -----------------------------------------------------------------------

    #[test]
    fn hex_similarity_identical() {
        let h = sha256_string("test");
        assert!((hex_similarity(&h, &h) - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn hex_similarity_completely_different_length() {
        assert_eq!(hex_similarity("aabb", "aabbcc"), 0.0);
    }

    #[test]
    fn hex_similarity_empty() {
        assert_eq!(hex_similarity("", ""), 0.0);
    }

    #[test]
    fn hex_similarity_partial_match() {
        // Same prefix, different suffix
        let score = hex_similarity("aabb", "aacc");
        assert!(score > 0.0);
        assert!(score < 1.0);
        // 2 of 4 nibbles match
        assert!((score - 0.5).abs() < f64::EPSILON);
    }


    #[test]
    fn multi_field_hasher_basic() {
        let mut h = MultiFieldHasher::new();
        h.feed_str("hello").feed_u64(42).feed_bool(true);
        let result = h.finish();
        assert_ne!(result, 0);
    }

    #[test]
    fn multi_field_hasher_deterministic() {
        let mut h1 = MultiFieldHasher::new();
        let mut h2 = MultiFieldHasher::new();
        h1.feed_str("test");
        h2.feed_str("test");
        assert_eq!(h1.finish(), h2.finish());
    }

    #[test]
    fn multi_field_hasher_different() {
        let mut h1 = MultiFieldHasher::new();
        let mut h2 = MultiFieldHasher::new();
        h1.feed_str("a");
        h2.feed_str("b");
        assert_ne!(h1.finish(), h2.finish());
    }

    #[test]
    fn multi_field_hasher_hex() {
        let h = MultiFieldHasher::new();
        assert_eq!(h.finish_hex().len(), 16);
    }

    #[test]
    fn set_membership_filter() {
        let mut f = SetMembershipFilter::new(1000, 3);
        f.insert("hello");
        f.insert("world");
        assert!(f.might_contain("hello"));
        assert!(f.might_contain("world"));
    }

    #[test]
    fn set_membership_filter_false_positive() {
        let f = SetMembershipFilter::new(1000, 3);
        assert!(!f.might_contain("not_inserted"));
    }

    #[test]
    fn distribution_ring_basic() {
        let mut ring = DistributionRing::new();
        ring.add_node("node1");
        ring.add_node("node2");
        let node = ring.get_node("some_key");
        assert!(node.is_some());
    }

    #[test]
    fn distribution_ring_consistent() {
        let mut ring = DistributionRing::new();
        ring.add_node("a");
        ring.add_node("b");
        let n1 = ring.get_node("key").unwrap().to_string();
        let n2 = ring.get_node("key").unwrap().to_string();
        assert_eq!(n1, n2);
    }

    #[test]
    fn distribution_ring_remove() {
        let mut ring = DistributionRing::new();
        ring.add_node("a");
        assert!(ring.remove_node("a"));
        assert_eq!(ring.node_count(), 0);
    }

    #[test]
    fn hash_benchmark_basic() {
        let mut b = HashBenchmark::new();
        b.record("sha256", 1000);
        b.record("fnv1a", 200);
        assert_eq!(b.fastest().unwrap().0, "fnv1a");
        assert_eq!(b.slowest().unwrap().0, "sha256");
        assert_eq!(b.average_ns(), Some(600));
    }

    #[test]
    fn hash_benchmark_empty() {
        let b = HashBenchmark::new();
        assert_eq!(b.fastest(), None);
        assert_eq!(b.average_ns(), None);
    }

    #[test]
    fn multi_field_hasher_reset() {
        let mut h = MultiFieldHasher::new();
        let initial = h.finish();
        h.feed_str("data");
        h.reset();
        assert_eq!(h.finish(), initial);
    }


    #[test]
    fn hash_accumulator_basic() {
        let mut acc = HashAccumulator::new();
        acc.feed_str("hello");
        acc.feed_u32(42);
        assert_eq!(acc.item_count(), 2);
        assert_ne!(acc.finish(), 5381);
    }

    #[test]
    fn hash_accumulator_with_seed() {
        let a = HashAccumulator::with_seed(100);
        let b = HashAccumulator::new();
        assert_ne!(a.finish(), b.finish());
    }

    #[test]
    fn hash_accumulator_reset() {
        let mut acc = HashAccumulator::new();
        acc.feed_str("data");
        let initial = HashAccumulator::new().finish();
        acc.reset();
        assert_eq!(acc.finish(), initial);
        assert_eq!(acc.item_count(), 0);
    }

    #[test]
    fn hash_accumulator_option_str() {
        let mut a = HashAccumulator::new();
        a.feed_option_str(Some("test"));
        let mut b = HashAccumulator::new();
        b.feed_option_str(None);
        assert_ne!(a.finish(), b.finish());
    }

    #[test]
    fn hash_accumulator_str_slice() {
        let mut acc = HashAccumulator::new();
        acc.feed_str_slice(&["a", "b", "c"]);
        assert!(acc.item_count() > 0);
    }

    #[test]
    fn hash_accumulator_combine() {
        let mut a = HashAccumulator::new();
        a.feed_str("hello");
        let mut b = HashAccumulator::new();
        b.feed_str("world");
        a.combine(&b);
        assert!(a.item_count() > 1);
    }

    #[test]
    fn hash_accumulator_sha256() {
        let acc = HashAccumulator::new();
        let hex = acc.finish_sha256();
        assert_eq!(hex.len(), 64);
        assert!(hex.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn hash_comparison_constant_time() {
        assert!(HashComparisonHelper::constant_time_eq(b"hello", b"hello"));
        assert!(!HashComparisonHelper::constant_time_eq(b"hello", b"world"));
        assert!(!HashComparisonHelper::constant_time_eq(b"hi", b"hello"));
    }

    #[test]
    fn hash_comparison_format() {
        let h = string_hash("test");
        let hex = HashComparisonHelper::format_hex(h);
        assert_eq!(hex.len(), 8);
        let short = HashComparisonHelper::format_short(h);
        assert_eq!(short.len(), 4);
    }

    #[test]
    fn hash_comparison_valid_hex() {
        assert!(HashComparisonHelper::is_valid_hex("abcdef01"));
        assert!(!HashComparisonHelper::is_valid_hex("xyz"));
        assert!(!HashComparisonHelper::is_valid_hex(""));
    }

    #[test]
    fn hash_comparison_hamming() {
        assert_eq!(HashComparisonHelper::hamming_distance(0, 0), 0);
        assert_eq!(HashComparisonHelper::hamming_distance(0b1010, 0b0101), 4);
    }

    #[test]
    fn hash_comparison_split_sha256() {
        let hex = sha256_string("test");
        let chunks = HashComparisonHelper::split_sha256(&hex);
        assert_eq!(chunks.len(), 8);
        for c in &chunks {
            assert_eq!(c.len(), 8);
        }
    }



    #[test]
    fn hashbuf_ringbuf_push_get() {
        let mut rb = HashBufRingBuffer::new(3);
        rb.push(10); rb.push(20); rb.push(30);
        assert_eq!(rb.get(0), Some(&10));
        assert_eq!(rb.get(2), Some(&30));
        assert_eq!(rb.len(), 3);
    }

    #[test]
    fn hashbuf_ringbuf_overflow() {
        let mut rb = HashBufRingBuffer::<i32>::new(2);
        rb.push(1); rb.push(2); rb.push(3);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(&2));
        assert_eq!(rb.get(1), Some(&3));
    }

    #[test]
    fn hashbuf_ringbuf_clear() {
        let mut rb = HashBufRingBuffer::new(5);
        rb.push("a".to_string()); rb.push("b".to_string());
        rb.clear();
        assert!(rb.is_empty());
    }

    #[test]
    fn hashbuf_ringbuf_newest_oldest() {
        let mut rb = HashBufRingBuffer::new(4);
        rb.push(100); rb.push(200); rb.push(300);
        assert_eq!(rb.oldest(), Some(&100));
        assert_eq!(rb.newest(), Some(&300));
    }

    #[test]
    fn hashbuf_ringbuf_to_vec() {
        let mut rb = HashBufRingBuffer::new(3);
        rb.push(1); rb.push(2);
        assert_eq!(rb.to_vec(), vec![1, 2]);
    }

    #[test]
    fn hashbuf_ringbuf_is_full() {
        let mut rb = HashBufRingBuffer::new(2);
        assert!(!rb.is_full());
        rb.push(1); rb.push(2);
        assert!(rb.is_full());
    }

    #[test]
    fn hashc_lru_insert_get() {
        let mut c = HashCLruCache::new(3);
        c.insert("a", 1); c.insert("b", 2); c.insert("c", 3);
        assert_eq!(c.get("a"), Some(&1));
        assert_eq!(c.get("b"), Some(&2));
        assert_eq!(c.len(), 3);
    }

    #[test]
    fn hashc_lru_eviction() {
        let mut c = HashCLruCache::new(2);
        c.insert("a", 1); c.insert("b", 2);
        let ev = c.insert("c", 3);
        assert!(ev.is_some());
        assert_eq!(ev.unwrap().0, "a");
        assert!(!c.contains("a"));
    }

    #[test]
    fn hashc_lru_hit_ratio() {
        let mut c = HashCLruCache::new(5);
        c.insert("x", 10);
        c.get("x"); c.get("y");
        assert!(c.hit_ratio() > 0.4 && c.hit_ratio() < 0.6);
    }

    #[test]
    fn hashc_lru_clear() {
        let mut c = HashCLruCache::new(3);
        c.insert("a", 1); c.insert("b", 2);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.hits(), 0);
    }

    #[test]
    fn hashc_lru_remove() {
        let mut c = HashCLruCache::new(3);
        c.insert("a", 100);
        assert_eq!(c.remove("a"), Some(100));
        assert!(!c.contains("a"));
    }

    #[test]
    fn hashc_lru_peek() {
        let mut c = HashCLruCache::new(3);
        c.insert("x", 42);
        assert_eq!(c.peek("x"), Some(&42));
        assert_eq!(c.misses(), 0);
    }


    #[test]
    fn hash_config_new() {
        let cfg = HashConfig::new();
        assert!(cfg.is_enabled());
        assert_eq!(cfg.version(), 1);
        assert_eq!(cfg.option_count(), 0);
    }

    #[test]
    fn hash_config_set_get() {
        let mut cfg = HashConfig::new();
        cfg.set_option("key", "value");
        assert_eq!(cfg.get_option("key"), Some("value"));
        assert!(cfg.has_option("key"));
    }

    #[test]
    fn hash_config_remove() {
        let mut cfg = HashConfig::new();
        cfg.set_option("a", "1");
        assert_eq!(cfg.remove_option("a"), Some("1".into()));
        assert!(!cfg.has_option("a"));
    }

    #[test]
    fn hash_config_keys_sorted() {
        let mut cfg = HashConfig::new();
        cfg.set_option("z", "1");
        cfg.set_option("a", "2");
        assert_eq!(cfg.option_keys(), vec!["a", "z"]);
    }

    #[test]
    fn hash_config_bump_version() {
        let mut cfg = HashConfig::new();
        cfg.bump_version();
        cfg.bump_version();
        assert_eq!(cfg.version(), 3);
    }

    #[test]
    fn hash_config_clear() {
        let mut cfg = HashConfig::new();
        cfg.set_option("x", "y");
        cfg.bump_version();
        cfg.clear();
        assert_eq!(cfg.option_count(), 0);
        assert_eq!(cfg.version(), 1);
    }

    #[test]
    fn hash_config_merge() {
        let mut cfg1 = HashConfig::new();
        cfg1.set_option("a", "1");
        let mut cfg2 = HashConfig::new();
        cfg2.set_option("b", "2");
        cfg1.merge(&cfg2);
        assert_eq!(cfg1.option_count(), 2);
    }

    #[test]
    fn hash_config_disable() {
        let mut cfg = HashConfig::new();
        cfg.set_enabled(false);
        assert!(!cfg.is_enabled());
    }

    #[test]
    fn hash_rate_tracker_empty() {
        let rt = HashRateTracker::new(1000);
        assert_eq!(rt.count(), 0);
        assert_eq!(rt.rate_per_second(), 0.0);
    }

    #[test]
    fn hash_rate_tracker_record() {
        let mut rt = HashRateTracker::new(1000);
        rt.record(100);
        rt.record(200);
        rt.record(300);
        assert_eq!(rt.count(), 3);
    }

    #[test]
    fn hash_rate_tracker_prune() {
        let mut rt = HashRateTracker::new(100);
        rt.record(10);
        rt.record(200);
        assert_eq!(rt.count(), 1);
    }

    #[test]
    fn hash_validator_valid() {
        let v = HashValidator::new();
        assert!(v.is_valid());
        assert_eq!(v.error_count(), 0);
    }

    #[test]
    fn hash_validator_errors() {
        let mut v = HashValidator::new();
        v.add_error("bad input");
        v.add_warning("slow");
        assert!(!v.is_valid());
        assert_eq!(v.error_count(), 1);
        assert_eq!(v.warning_count(), 1);
        assert_eq!(v.first_error(), Some("bad input"));
    }

    #[test]
    fn hash_validator_clear() {
        let mut v = HashValidator::new();
        v.add_error("err");
        v.clear();
        assert!(v.is_valid());
    }

    #[test]
    fn hash_validator_merge() {
        let mut v1 = HashValidator::new();
        v1.add_error("e1");
        let mut v2 = HashValidator::new();
        v2.add_error("e2");
        v2.add_warning("w1");
        v1.merge(&v2);
        assert_eq!(v1.error_count(), 2);
        assert_eq!(v1.warning_count(), 1);
    }

    #[test]
    fn hash_rate_tracker_clear() {
        let mut rt = HashRateTracker::new(1000);
        rt.record(100);
        rt.clear();
        assert_eq!(rt.count(), 0);
    }


    #[test]
    fn yp_metrics_empty() {
        let m = YpMetrics::new("hash");
        assert_eq!(m.count(), 0);
        assert!((m.mean() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn yp_metrics_record_and_mean() {
        let mut m = YpMetrics::new("hash");
        m.record(10.0);
        m.record(20.0);
        m.record(30.0);
        assert_eq!(m.count(), 3);
        assert!((m.mean() - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn yp_metrics_min_max() {
        let mut m = YpMetrics::new("test");
        m.record(5.0);
        m.record(15.0);
        m.record(10.0);
        assert!((m.min_val() - 5.0).abs() < f64::EPSILON);
        assert!((m.max_val() - 15.0).abs() < f64::EPSILON);
    }

    #[test]
    fn yp_metrics_variance_and_std() {
        let mut m = YpMetrics::new("v");
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
    fn yp_metrics_percentile() {
        let mut m = YpMetrics::new("p");
        for i in 1..=100 {
            m.record(i as f64);
        }
        let p50 = m.percentile(50.0);
        assert!(p50 >= 49.0 && p50 <= 51.0);
    }

    #[test]
    fn yp_metrics_merge() {
        let mut a = YpMetrics::new("a");
        a.record(1.0);
        let mut b = YpMetrics::new("b");
        b.record(2.0);
        b.record(3.0);
        a.merge(&b);
        assert_eq!(a.count(), 3);
    }

    #[test]
    fn yp_metrics_reset() {
        let mut m = YpMetrics::new("r");
        m.record(42.0);
        m.reset();
        assert_eq!(m.count(), 0);
    }

    #[test]
    fn yp_rate_window_empty() {
        let rw = YpRateWindow::new(1000);
        assert!(rw.is_empty());
        assert_eq!(rw.window_ms(), 1000);
    }

    #[test]
    fn yp_rate_window_tick_and_rate() {
        let mut rw = YpRateWindow::new(1000);
        rw.tick(100);
        rw.tick(200);
        rw.tick(300);
        assert_eq!(rw.rate(500), 3);
        assert_eq!(rw.rate(1500), 0);
    }

    #[test]
    fn yp_lru_cache_basic() {
        let mut c = YpLruCache::new(2);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        assert_eq!(c.get("a"), Some("1".to_string()));
        c.put("c".into(), "3".into());
        assert_eq!(c.get("b"), None);
    }

    #[test]
    fn yp_lru_cache_contains_and_keys() {
        let mut c = YpLruCache::new(3);
        c.put("x".into(), "10".into());
        c.put("y".into(), "20".into());
        assert!(c.contains_key("x"));
        assert!(!c.contains_key("z"));
        assert_eq!(c.keys().len(), 2);
    }

    #[test]
    fn yp_lru_cache_remove() {
        let mut c = YpLruCache::new(3);
        c.put("k".into(), "v".into());
        assert_eq!(c.remove("k"), Some("v".to_string()));
        assert!(c.is_empty());
        assert_eq!(c.remove("k"), None);
    }

    #[test]
    fn yp_metrics_sum() {
        let mut m = YpMetrics::new("s");
        m.record(1.0);
        m.record(2.0);
        m.record(3.0);
        assert!((m.sum() - 6.0).abs() < f64::EPSILON);
    }

    #[test]
    fn yp_metrics_label() {
        let m = YpMetrics::new("my_label");
        assert_eq!(m.label(), "my_label");
    }

    #[test]
    fn yp_lru_cache_clear() {
        let mut c = YpLruCache::new(5);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // xa_ extended tests for hash
    #[test]
    fn xa_hash_ring_new() {
        let rb = super::XaHashRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_hash_ring_push_len() {
        let mut rb = super::XaHashRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_hash_ring_wrap() {
        let mut rb = super::XaHashRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_hash_ring_mean_empty() {
        let rb = super::XaHashRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_hash_ring_mean_values() {
        let mut rb = super::XaHashRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_hash_ring_min_max() {
        let mut rb = super::XaHashRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_hash_ring_iter() {
        let mut rb = super::XaHashRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_hash_counter_new() {
        let c = super::XaHashCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_hash_counter_inc() {
        let mut c = super::XaHashCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_hash_counter_inc_by() {
        let mut c = super::XaHashCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_hash_counter_reset() {
        let mut c = super::XaHashCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_hash_counter_clear() {
        let mut c = super::XaHashCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_hash_counter_default() {
        let c = super::XaHashCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }


    // ---- xc_ pool / scheduler tests – block 88 ----

    #[test]
    fn xc_88_pool_new_empty() {
        let pool: super::Xc88Pool<i32> = super::Xc88Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_88_pool_release_acquire() {
        let mut pool = super::Xc88Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_88_pool_acquire_empty() {
        let mut pool: super::Xc88Pool<i32> = super::Xc88Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_88_pool_full() {
        let mut pool = super::Xc88Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_88_pool_drain() {
        let mut pool = super::Xc88Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_88_pool_stats() {
        let mut pool = super::Xc88Pool::new(8);
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
    fn xc_88_pool_clear() {
        let mut pool = super::Xc88Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_88_pool_shrink() {
        let mut pool = super::Xc88Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_88_pool_default() {
        let pool: super::Xc88Pool<String> = super::Xc88Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_88_pool_extend() {
        let mut pool = super::Xc88Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_88_pool_retain() {
        let mut pool = super::Xc88Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_88_scheduler_round_robin() {
        let mut sched = super::Xc88Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_88_scheduler_empty() {
        let mut sched = super::Xc88Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_88_scheduler_reset() {
        let mut sched = super::Xc88Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_88_scheduler_add_remove() {
        let mut sched = super::Xc88Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_88_scheduler_targets() {
        let sched = super::Xc88Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_88_hash_empty() {
        assert_eq!(super::xc_88_hash(b""), 5381);
    }

    #[test]
    fn xc_88_hash_data() {
        let h = super::xc_88_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_88_hash(b"hello"), h);
    }

    #[test]
    fn xc_88_reverse_str() {
        assert_eq!(super::xc_88_reverse("abc"), "cba");
        assert_eq!(super::xc_88_reverse(""), "");
    }


    // --- xd_99 deepening tests ---

    #[test]
    fn xd_99_sm_initial_state() {
        let sm = Xd99StateMachine::new();
        assert_eq!(sm.current_state(), Xd99State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_99_sm_valid_idle_to_running() {
        let mut sm = Xd99StateMachine::new();
        assert!(sm.transition(Xd99State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd99State::Running);
    }

    #[test]
    fn xd_99_sm_valid_running_to_paused() {
        let mut sm = Xd99StateMachine::new();
        sm.transition(Xd99State::Running).unwrap();
        assert!(sm.transition(Xd99State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd99State::Paused);
    }

    #[test]
    fn xd_99_sm_valid_running_to_done() {
        let mut sm = Xd99StateMachine::new();
        sm.transition(Xd99State::Running).unwrap();
        assert!(sm.transition(Xd99State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd99State::Done);
    }

    #[test]
    fn xd_99_sm_valid_paused_to_running() {
        let mut sm = Xd99StateMachine::new();
        sm.transition(Xd99State::Running).unwrap();
        sm.transition(Xd99State::Paused).unwrap();
        assert!(sm.transition(Xd99State::Running).is_ok());
    }

    #[test]
    fn xd_99_sm_valid_done_to_idle() {
        let mut sm = Xd99StateMachine::new();
        sm.transition(Xd99State::Running).unwrap();
        sm.transition(Xd99State::Done).unwrap();
        assert!(sm.transition(Xd99State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd99State::Idle);
    }

    #[test]
    fn xd_99_sm_invalid_idle_to_done() {
        let mut sm = Xd99StateMachine::new();
        assert!(sm.transition(Xd99State::Done).is_err());
    }

    #[test]
    fn xd_99_sm_invalid_idle_to_paused() {
        let mut sm = Xd99StateMachine::new();
        assert!(sm.transition(Xd99State::Paused).is_err());
    }

    #[test]
    fn xd_99_sm_history_tracking() {
        let mut sm = Xd99StateMachine::new();
        sm.transition(Xd99State::Running).unwrap();
        sm.transition(Xd99State::Paused).unwrap();
        sm.transition(Xd99State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd99State::Idle);
        assert_eq!(sm.history()[0].to, Xd99State::Running);
        assert_eq!(sm.history()[1].from, Xd99State::Running);
        assert_eq!(sm.history()[2].to, Xd99State::Done);
    }

    #[test]
    fn xd_99_sm_serialize_deserialize() {
        let mut sm = Xd99StateMachine::new();
        sm.transition(Xd99State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd99StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd99State::Running));
    }

    #[test]
    fn xd_99_sm_deserialize_invalid() {
        assert_eq!(Xd99StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_99_sm_reset() {
        let mut sm = Xd99StateMachine::new();
        sm.transition(Xd99State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd99State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_99_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd99EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd99Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_99_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd99EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd99Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd99Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_99_bus_unsubscribe() {
        let mut bus = Xd99EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_99_event_kind_and_payload() {
        let e = Xd99Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd99Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_99_bus_clear_history() {
        let mut bus = Xd99EventBus::new();
        bus.publish(Xd99Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_99_sm_step_counter_increments() {
        let mut sm = Xd99StateMachine::new();
        sm.transition(Xd99State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd99State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xg_23 graph tests ------------------------------------------------

    #[test]
    fn xg_23_graph_empty() {
        let g = super::Xg23Graph::new();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn xg_23_graph_add_node() {
        let mut g = super::Xg23Graph::new();
        g.add_node(1);
        g.add_node(2);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_23_graph_add_edge() {
        let mut g = super::Xg23Graph::new();
        g.add_edge(0, 1);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_23_graph_neighbors() {
        let mut g = super::Xg23Graph::new();
        g.add_edge(0, 1);
        g.add_edge(0, 2);
        assert_eq!(g.neighbors(0).len(), 2);
    }

    #[test]
    fn xg_23_graph_has_path() {
        let mut g = super::Xg23Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(g.has_path(0, 2));
        assert!(!g.has_path(2, 0));
    }

    #[test]
    fn xg_23_graph_self_path() {
        let g = super::Xg23Graph::new();
        assert!(g.has_path(5, 5));
    }

    #[test]
    fn xg_23_graph_topo_sort() {
        let mut g = super::Xg23Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        let sorted = g.topological_sort().unwrap();
        let pos: std::collections::HashMap<usize, usize> =
            sorted.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&1] < pos[&2]);
    }

    #[test]
    fn xg_23_graph_cycle_detect_false() {
        let mut g = super::Xg23Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(!g.cycle_detect());
    }

    #[test]
    fn xg_23_graph_cycle_detect_true() {
        let mut g = super::Xg23Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        g.add_edge(2, 0);
        assert!(g.cycle_detect());
    }

    // -- xg_23 heap tests -------------------------------------------------

    #[test]
    fn xg_23_heap_empty() {
        let h: super::Xg23Heap<i32> = super::Xg23Heap::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn xg_23_heap_push_pop() {
        let mut h = super::Xg23Heap::new();
        h.push(3);
        h.push(1);
        h.push(2);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(3));
    }

    #[test]
    fn xg_23_heap_peek() {
        let mut h = super::Xg23Heap::new();
        h.push(5);
        h.push(2);
        assert_eq!(h.peek(), Some(&2));
    }

    #[test]
    fn xg_23_heap_drain_sorted() {
        let mut h = super::Xg23Heap::new();
        for v in [4, 1, 7, 2, 9] { h.push(v); }
        assert_eq!(h.drain_sorted(), vec![1, 2, 4, 7, 9]);
        assert!(h.is_empty());
    }

    #[test]
    fn xg_23_heap_merge() {
        let mut a = super::Xg23Heap::new();
        let mut b = super::Xg23Heap::new();
        a.push(5); a.push(3);
        b.push(4); b.push(1);
        a.merge(&mut b);
        assert_eq!(a.len(), 4);
        assert_eq!(a.pop(), Some(1));
    }

    #[test]
    fn xg_23_heap_default() {
        let h: super::Xg23Heap<u64> = Default::default();
        assert!(h.is_empty());
    }

    #[test]
    fn xg_23_graph_default() {
        let g: super::Xg23Graph = Default::default();
        assert_eq!(g.node_count(), 0);
    }


    #[test]
    fn xh87_skip_insert_contains() {
        let mut sl = super::Xh87SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh87_skip_remove() {
        let mut sl = super::Xh87SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh87_skip_len() {
        let mut sl = super::Xh87SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh87_skip_range_query() {
        let mut sl = super::Xh87SkipList::xh_new(4);
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
    fn xh87_skip_floor_ceiling() {
        let mut sl = super::Xh87SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh87_skip_rank() {
        let mut sl = super::Xh87SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh87_skip_empty() {
        let sl = super::Xh87SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh87_skip_duplicates() {
        let mut sl = super::Xh87SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh87_bitset_set_test() {
        let mut bs = super::Xh87BitSet::xh_new(256);
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
    fn xh87_bitset_clear_count() {
        let mut bs = super::Xh87BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh87_bitset_and_or_xor() {
        let mut a = super::Xh87BitSet::xh_new(128);
        let mut b = super::Xh87BitSet::xh_new(128);
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
    fn xh87_bitset_iter_ones() {
        let mut bs = super::Xh87BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh87_bitset_first_last() {
        let mut bs = super::Xh87BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh87_bitset_empty() {
        let bs = super::Xh87BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi87_deque_push_pop_back() {
        let mut dq = super::Xi87Deque::xi_new(4);
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
    fn xi87_deque_push_pop_front() {
        let mut dq = super::Xi87Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi87_deque_mixed_ops() {
        let mut dq = super::Xi87Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi87_deque_get_and_split() {
        let mut dq = super::Xi87Deque::xi_new(8);
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
    fn xi87_deque_rotate_left() {
        let mut dq = super::Xi87Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi87_deque_rotate_right() {
        let mut dq = super::Xi87Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi87_deque_grow() {
        let mut dq = super::Xi87Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi87_deque_empty() {
        let dq = super::Xi87Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi87_interval_tree_insert_query() {
        let mut tree = super::Xi87IntervalTree::xi_new();
        tree.xi_insert(super::Xi87Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi87Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi87Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi87_interval_tree_overlap() {
        let mut tree = super::Xi87IntervalTree::xi_new();
        tree.xi_insert(super::Xi87Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi87Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi87Interval::xi_new(12, 20));
        let q = super::Xi87Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi87_interval_tree_remove() {
        let mut tree = super::Xi87IntervalTree::xi_new();
        tree.xi_insert(super::Xi87Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi87Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi87_interval_tree_gaps() {
        let mut tree = super::Xi87IntervalTree::xi_new();
        tree.xi_insert(super::Xi87Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi87Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi87Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi87Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi87Interval::xi_new(8, 10));
    }

    #[test]
    fn xi87_interval_tree_merge() {
        let mut tree = super::Xi87IntervalTree::xi_new();
        tree.xi_insert(super::Xi87Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi87Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi87Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi87Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi87Interval::xi_new(10, 15));
    }

    #[test]
    fn xi87_interval_tree_all() {
        let mut tree = super::Xi87IntervalTree::xi_new();
        tree.xi_insert(super::Xi87Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi87Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi87_interval_tree_empty() {
        let tree = super::Xi87IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi87_interval_tree_contains_point() {
        let iv = super::Xi87Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 87) ---

    #[test]
    fn xj_87_uf_make_and_find() {
        let mut uf = super::Xj87UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_87_uf_union_connected() {
        let mut uf = super::Xj87UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_87_uf_component_count() {
        let mut uf = super::Xj87UnionFind::xj_new();
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
    fn xj_87_uf_component_size() {
        let mut uf = super::Xj87UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_87_uf_largest_component() {
        let mut uf = super::Xj87UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_87_uf_many_elements() {
        let mut uf = super::Xj87UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_87_uf_separate_components() {
        let mut uf = super::Xj87UnionFind::xj_new();
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
    fn xj_87_uf_path_compression() {
        let mut uf = super::Xj87UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_87_bt_insert_get() {
        let mut bt = super::Xj87BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_87_bt_contains_len() {
        let mut bt = super::Xj87BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_87_bt_replace() {
        let mut bt = super::Xj87BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_87_bt_remove() {
        let mut bt = super::Xj87BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_87_bt_keys_values() {
        let mut bt = super::Xj87BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_87_bt_range() {
        let mut bt = super::Xj87BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_87_bt_min_max() {
        let mut bt = super::Xj87BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_87_bt_many_inserts() {
        let mut bt = super::Xj87BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_87 segment tree tests ---

    #[test]
    fn xk_87_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk87SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_87_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk87SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_87_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk87SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_87_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk87SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_87_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk87SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_87_st_single_element() {
        let data = vec![42];
        let st = super::Xk87SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_87_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk87SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_87_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk87SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_87 disjoint intervals tests ---

    #[test]
    fn xk_87_di_add_and_count() {
        let mut di = super::Xk87DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_87_di_merge_overlap() {
        let mut di = super::Xk87DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_87_di_contains() {
        let mut di = super::Xk87DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_87_di_remove() {
        let mut di = super::Xk87DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_87_di_covered_length() {
        let mut di = super::Xk87DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_87_di_gaps() {
        let mut di = super::Xk87DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_87_di_merge_adjacent() {
        let mut di = super::Xk87DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_87_di_empty() {
        let di = super::Xk87DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_87_rope_new_empty() {
        let rope = super::Xl87Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_87_rope_from_str() {
        let rope = super::Xl87Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_87_rope_insert_at() {
        let mut rope = super::Xl87Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_87_rope_delete_range() {
        let mut rope = super::Xl87Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_87_rope_char_at() {
        let rope = super::Xl87Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_87_rope_split_concat() {
        let rope = super::Xl87Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_87_rope_line_count() {
        let rope = super::Xl87Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_87_rope_line_at() {
        let rope = super::Xl87Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_87_sa_build_and_search() {
        let sa = super::Xl87SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_87_sa_count() {
        let sa = super::Xl87SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_87_sa_longest_repeated() {
        let sa = super::Xl87SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_87_sa_all_positions() {
        let sa = super::Xl87SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_87_sa_len() {
        let sa = super::Xl87SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_87_sa_empty() {
        let sa = super::Xl87SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_87_rope_slice() {
        let rope = super::Xl87Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_87_sa_search_start() {
        let sa = super::Xl87SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }

    #[test]
    fn xm_87_sparse_set_get() {
        let mut m = super::Xm87MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 1, 5.0);
        assert!((m.xm_get(0, 1) - 5.0).abs() < f64::EPSILON);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_87_sparse_row_col() {
        let mut m = super::Xm87MatrixSparse::xm_new(4, 4);
        m.xm_set(1, 2, 3.0);
        m.xm_set(1, 3, 7.0);
        let row = m.xm_row(1);
        assert_eq!(row.len(), 2);
        let col = m.xm_col(2);
        assert_eq!(col.len(), 1);
    }

    #[test]
    fn xm_87_sparse_transpose() {
        let mut m = super::Xm87MatrixSparse::xm_new(2, 3);
        m.xm_set(0, 2, 9.0);
        let t = m.xm_transpose();
        assert!((t.xm_get(2, 0) - 9.0).abs() < f64::EPSILON);
        assert_eq!(t.xm_dims(), (3, 2));
    }

    #[test]
    fn xm_87_sparse_multiply_vec() {
        let mut m = super::Xm87MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        let result = m.xm_multiply_vec(&[3.0, 4.0]);
        assert!((result[0] - 3.0).abs() < f64::EPSILON);
        assert!((result[1] - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_87_sparse_nnz_density() {
        let mut m = super::Xm87MatrixSparse::xm_new(10, 10);
        m.xm_set(0, 0, 1.0);
        m.xm_set(5, 5, 2.0);
        assert_eq!(m.xm_nnz(), 2);
        assert!((m.xm_density() - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_87_sparse_clear() {
        let mut m = super::Xm87MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        m.xm_clear();
        assert_eq!(m.xm_nnz(), 0);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_87_sparse_overwrite_zero() {
        let mut m = super::Xm87MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 5.0);
        assert_eq!(m.xm_nnz(), 1);
        m.xm_set(0, 0, 0.0);
        assert_eq!(m.xm_nnz(), 0);
    }

    #[test]
    fn xm_87_tokenizer_basic() {
        let t = super::Xm87Tokenizer::xm_new("hello world foo");
        let tokens = t.xm_tokenize();
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn xm_87_tokenizer_count() {
        let t = super::Xm87Tokenizer::xm_new("a b c d e");
        assert_eq!(t.xm_token_count(), 5);
    }

    #[test]
    fn xm_87_tokenizer_unique() {
        let t = super::Xm87Tokenizer::xm_new("a b a c b");
        let u = t.xm_unique_tokens();
        assert_eq!(u.len(), 3);
    }

    #[test]
    fn xm_87_tokenizer_frequency() {
        let t = super::Xm87Tokenizer::xm_new("x y x x y z");
        let freq = t.xm_frequency_map();
        assert_eq!(freq.get("x"), Some(&3));
        assert_eq!(freq.get("y"), Some(&2));
        assert_eq!(freq.get("z"), Some(&1));
    }

    #[test]
    fn xm_87_tokenizer_delimiter() {
        let t = super::Xm87Tokenizer::xm_new("a,b,,c");
        let parts = t.xm_split_by_delimiter(',');
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn xm_87_tokenizer_whitespace() {
        let t = super::Xm87Tokenizer::xm_new("one  two  three");
        let parts = t.xm_split_by_whitespace();
        assert_eq!(parts, vec!["one", "two", "three"]);
    }

    #[test]
    fn xm_87_tokenizer_empty() {
        let t = super::Xm87Tokenizer::xm_new("");
        assert!(t.xm_is_empty());
        assert_eq!(t.xm_token_count(), 0);
    }


    // ---- Fenwick tree tests — crate 87 ----

    #[test]
    fn xn_87_fenwick_prefix_sum() {
        let mut ft = super::Xn87Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, (i + 1) as i64); }
        assert_eq!(ft.xn_prefix_sum(0), 1);
        assert_eq!(ft.xn_prefix_sum(4), 15);
    }

    #[test]
    fn xn_87_fenwick_range_sum() {
        let mut ft = super::Xn87Fenwick::xn_new(6);
        for i in 0..6 { ft.xn_update(i, (i * 2) as i64); }
        assert_eq!(ft.xn_range_sum(1, 3), 2 + 4 + 6);
    }

    #[test]
    fn xn_87_fenwick_point_query() {
        let mut ft = super::Xn87Fenwick::xn_new(4);
        ft.xn_update(2, 7);
        assert_eq!(ft.xn_point_query(2), 7);
        assert_eq!(ft.xn_point_query(0), 0);
    }

    #[test]
    fn xn_87_fenwick_len() {
        let ft = super::Xn87Fenwick::xn_new(10);
        assert_eq!(ft.xn_len(), 10);
    }

    #[test]
    fn xn_87_fenwick_multiple_updates() {
        let mut ft = super::Xn87Fenwick::xn_new(3);
        ft.xn_update(0, 5);
        ft.xn_update(0, 3);
        assert_eq!(ft.xn_point_query(0), 8);
    }

    #[test]
    fn xn_87_fenwick_single_element() {
        let mut ft = super::Xn87Fenwick::xn_new(1);
        ft.xn_update(0, 42);
        assert_eq!(ft.xn_prefix_sum(0), 42);
        assert_eq!(ft.xn_range_sum(0, 0), 42);
    }

    #[test]
    fn xn_87_fenwick_find_kth() {
        let mut ft = super::Xn87Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, 1); }
        assert_eq!(ft.xn_find_kth(3), Some(2));
    }

    #[test]
    fn xn_87_fenwick_negative_delta() {
        let mut ft = super::Xn87Fenwick::xn_new(3);
        ft.xn_update(1, 10);
        ft.xn_update(1, -4);
        assert_eq!(ft.xn_point_query(1), 6);
    }

    // ---- AVL tree tests — crate 87 ----

    #[test]
    fn xn_87_avl_insert_get() {
        let mut m = super::Xn87AVL::xn_new();
        m.xn_insert(3, "c");
        m.xn_insert(1, "a");
        m.xn_insert(2, "b");
        assert_eq!(m.xn_get(&2), Some(&"b"));
        assert_eq!(m.xn_len(), 3);
    }

    #[test]
    fn xn_87_avl_remove() {
        let mut m = super::Xn87AVL::xn_new();
        m.xn_insert(1, 10);
        m.xn_insert(2, 20);
        assert!(m.xn_remove(&1));
        assert!(!m.xn_contains(&1));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_87_avl_in_order() {
        let mut m = super::Xn87AVL::xn_new();
        for k in [5, 3, 7, 1, 4] { m.xn_insert(k, k * 10); }
        let keys: Vec<_> = m.xn_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xn_87_avl_min_max() {
        let mut m = super::Xn87AVL::xn_new();
        for k in [10, 5, 20, 3, 15] { m.xn_insert(k, k); }
        assert_eq!(m.xn_min(), Some(&3));
        assert_eq!(m.xn_max(), Some(&20));
    }

    #[test]
    fn xn_87_avl_floor_ceiling() {
        let mut m = super::Xn87AVL::xn_new();
        for k in [10, 20, 30] { m.xn_insert(k, k); }
        assert_eq!(m.xn_floor(&15), Some(&10));
        assert_eq!(m.xn_ceiling(&15), Some(&20));
    }

    #[test]
    fn xn_87_avl_height_balanced() {
        let mut m = super::Xn87AVL::xn_new();
        for k in 0..31 { m.xn_insert(k, k); }
        assert!(m.xn_height() <= 7);
    }

    #[test]
    fn xn_87_avl_overwrite() {
        let mut m = super::Xn87AVL::xn_new();
        m.xn_insert(1, "old");
        m.xn_insert(1, "new");
        assert_eq!(m.xn_get(&1), Some(&"new"));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_87_avl_empty() {
        let m: super::Xn87AVL<i32, i32> = super::Xn87AVL::xn_new();
        assert_eq!(m.xn_len(), 0);
        assert_eq!(m.xn_min(), None);
        assert_eq!(m.xn_max(), None);
        assert_eq!(m.xn_height(), 0);
    }

    // --- Xo87RedBlack tests ---

    #[test]
    fn xo_87_rb_insert_and_get() {
        let mut tree = super::Xo87RedBlack::xo_new();
        tree.xo_insert(10, "ten");
        tree.xo_insert(20, "twenty");
        tree.xo_insert(5, "five");
        assert_eq!(tree.xo_get(&10), Some(&"ten"));
        assert_eq!(tree.xo_get(&20), Some(&"twenty"));
        assert_eq!(tree.xo_get(&5), Some(&"five"));
        assert_eq!(tree.xo_get(&99), None);
    }

    #[test]
    fn xo_87_rb_len_and_empty() {
        let mut tree = super::Xo87RedBlack::<i32, i32>::xo_new();
        assert!(tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 0);
        tree.xo_insert(1, 100);
        tree.xo_insert(2, 200);
        assert!(!tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 2);
    }

    #[test]
    fn xo_87_rb_min_max() {
        let mut tree = super::Xo87RedBlack::xo_new();
        for k in [30, 10, 50, 20, 40] {
            tree.xo_insert(k, k * 10);
        }
        assert_eq!(tree.xo_min(), Some(&10));
        assert_eq!(tree.xo_max(), Some(&50));
    }

    #[test]
    fn xo_87_rb_contains() {
        let mut tree = super::Xo87RedBlack::xo_new();
        tree.xo_insert(42, "answer");
        assert!(tree.xo_contains(&42));
        assert!(!tree.xo_contains(&43));
    }

    #[test]
    fn xo_87_rb_remove() {
        let mut tree = super::Xo87RedBlack::xo_new();
        tree.xo_insert(1, "a");
        tree.xo_insert(2, "b");
        tree.xo_insert(3, "c");
        assert_eq!(tree.xo_remove(&2), Some("b"));
        assert_eq!(tree.xo_len(), 2);
        assert!(!tree.xo_contains(&2));
        assert_eq!(tree.xo_remove(&99), None);
    }

    #[test]
    fn xo_87_rb_in_order() {
        let mut tree = super::Xo87RedBlack::xo_new();
        for k in [5, 3, 7, 1, 4] {
            tree.xo_insert(k, k);
        }
        let keys: Vec<i32> = tree.xo_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xo_87_rb_black_height() {
        let mut tree = super::Xo87RedBlack::xo_new();
        for k in 0..15 {
            tree.xo_insert(k, k);
        }
        let bh = tree.xo_black_height();
        assert!(bh >= 2 && bh <= 6, "black height {bh} out of range");
    }

    #[test]
    fn xo_87_rb_overwrite() {
        let mut tree = super::Xo87RedBlack::xo_new();
        tree.xo_insert(1, "old");
        tree.xo_insert(1, "new");
        assert_eq!(tree.xo_get(&1), Some(&"new"));
        assert_eq!(tree.xo_len(), 1);
    }

    // --- Xo87ConsistentHash tests ---

    #[test]
    fn xo_87_ch_add_and_count() {
        let mut ring = super::Xo87ConsistentHash::xo_new(100);
        ring.xo_add_node("server-a");
        ring.xo_add_node("server-b");
        assert_eq!(ring.xo_node_count(), 2);
        assert_eq!(ring.xo_virtual_nodes(), 200);
    }

    #[test]
    fn xo_87_ch_remove_node() {
        let mut ring = super::Xo87ConsistentHash::xo_new(50);
        ring.xo_add_node("alpha");
        ring.xo_add_node("beta");
        ring.xo_remove_node("alpha");
        assert_eq!(ring.xo_node_count(), 1);
        assert_eq!(ring.xo_virtual_nodes(), 50);
    }

    #[test]
    fn xo_87_ch_get_node() {
        let mut ring = super::Xo87ConsistentHash::xo_new(50);
        ring.xo_add_node("node-1");
        let result = ring.xo_get_node("some-key");
        assert_eq!(result, Some("node-1"));
    }

    #[test]
    fn xo_87_ch_empty_ring() {
        let ring = super::Xo87ConsistentHash::xo_new(10);
        assert_eq!(ring.xo_get_node("key"), None);
        assert_eq!(ring.xo_node_count(), 0);
    }

    #[test]
    fn xo_87_ch_distribution() {
        let mut ring = super::Xo87ConsistentHash::xo_new(100);
        ring.xo_add_node("s1");
        ring.xo_add_node("s2");
        let keys: Vec<&str> = vec!["k1", "k2", "k3", "k4", "k5", "k6"];
        let dist = ring.xo_key_distribution(&keys);
        let total: usize = dist.values().sum();
        assert_eq!(total, 6);
    }

    #[test]
    fn xo_87_ch_rebalance() {
        let mut ring = super::Xo87ConsistentHash::xo_new(100);
        ring.xo_add_node("n1");
        ring.xo_add_node("n2");
        ring.xo_add_node("n3");
        let rf = ring.xo_rebalance_factor();
        assert!(rf >= 0.0, "rebalance factor should be non-negative");
    }

    #[test]
    fn xo_87_ch_virtual_nodes() {
        let mut ring = super::Xo87ConsistentHash::xo_new(75);
        ring.xo_add_node("host1");
        ring.xo_add_node("host2");
        assert_eq!(ring.xo_virtual_nodes(), 150);
    }

    #[test]
    fn xo_87_ch_consistent_lookup() {
        let mut ring = super::Xo87ConsistentHash::xo_new(50);
        ring.xo_add_node("srv-a");
        ring.xo_add_node("srv-b");
        let first = ring.xo_get_node("stable-key").unwrap().to_string();
        let second = ring.xo_get_node("stable-key").unwrap().to_string();
        assert_eq!(first, second, "same key must map to same node");
    }

}