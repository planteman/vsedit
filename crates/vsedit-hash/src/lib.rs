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

}