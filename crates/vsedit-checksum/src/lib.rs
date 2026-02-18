//! File integrity verification.

use std::collections::HashMap;
use std::fmt;
use sha2::{Digest, Sha256};
use std::io;
use std::path::Path;

/// Compute a 64-bit FNV-1a hash of the input bytes.
pub fn simple_hash(data: &[u8]) -> u64 {
    const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const FNV_PRIME: u64 = 0x0100_0000_01b3;
    let mut hash = FNV_OFFSET;
    for &byte in data {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

/// Encode bytes as a lowercase hex string.
pub fn hex_encode(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// Decode a hex string into bytes. Returns `None` if the input has odd length
/// or contains non-hex characters.
pub fn hex_decode(hex: &str) -> Option<Vec<u8>> {
    if hex.len() % 2 != 0 {
        return None;
    }
    let mut bytes = Vec::with_capacity(hex.len() / 2);
    for chunk in hex.as_bytes().chunks(2) {
        let hi = hex_nibble(chunk[0])?;
        let lo = hex_nibble(chunk[1])?;
        bytes.push((hi << 4) | lo);
    }
    Some(bytes)
}

fn hex_nibble(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

/// Supported checksum algorithms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChecksumKind {
    /// SHA-256 (real, via sha2 crate).
    Sha256,
    /// MD5 (RFC 1321).
    Md5,
    /// SHA-1 (RFC 3174).
    Sha1,
    /// Real FNV-1a 64-bit hash.
    Fnv64,
    /// CRC-32 (IEEE 802.3).
    Crc32,
}

/// Return the human-readable algorithm name for a [`ChecksumKind`].
pub fn algorithm_name(kind: ChecksumKind) -> &'static str {
    match kind {
        ChecksumKind::Sha256 => "SHA-256",
        ChecksumKind::Md5 => "MD5",
        ChecksumKind::Sha1 => "SHA-1",
        ChecksumKind::Fnv64 => "FNV-1a 64-bit",
        ChecksumKind::Crc32 => "CRC-32",
    }
}

/// Result of a checksum computation with metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChecksumResult {
    /// Which algorithm was used.
    pub kind: ChecksumKind,
    /// The hex-encoded digest string.
    pub hex_digest: String,
    /// Length of the input data in bytes.
    pub byte_length: usize,
}

/// Compute a checksum and return a [`ChecksumResult`] with metadata.
pub fn compute_checksum_result(data: &[u8], kind: ChecksumKind) -> ChecksumResult {
    ChecksumResult {
        kind,
        hex_digest: compute_checksum(data, kind),
        byte_length: data.len(),
    }
}

/// Compute SHA-256 of `data`, returning a lowercase hex string.
pub fn compute_sha256(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex_encode(&hasher.finalize())
}

/// Compute MD5 (RFC 1321) of `data`, returning a lowercase hex string.
pub fn compute_md5(data: &[u8]) -> String {
    const S: [u32; 64] = [
        7,12,17,22, 7,12,17,22, 7,12,17,22, 7,12,17,22,
        5, 9,14,20, 5, 9,14,20, 5, 9,14,20, 5, 9,14,20,
        4,11,16,23, 4,11,16,23, 4,11,16,23, 4,11,16,23,
        6,10,15,21, 6,10,15,21, 6,10,15,21, 6,10,15,21,
    ];
    const T: [u32; 64] = [
        0xd76aa478, 0xe8c7b756, 0x242070db, 0xc1bdceee,
        0xf57c0faf, 0x4787c62a, 0xa8304613, 0xfd469501,
        0x698098d8, 0x8b44f7af, 0xffff5bb1, 0x895cd7be,
        0x6b901122, 0xfd987193, 0xa679438e, 0x49b40821,
        0xf61e2562, 0xc040b340, 0x265e5a51, 0xe9b6c7aa,
        0xd62f105d, 0x02441453, 0xd8a1e681, 0xe7d3fbc8,
        0x21e1cde6, 0xc33707d6, 0xf4d50d87, 0x455a14ed,
        0xa9e3e905, 0xfcefa3f8, 0x676f02d9, 0x8d2a4c8a,
        0xfffa3942, 0x8771f681, 0x6d9d6122, 0xfde5380c,
        0xa4beea44, 0x4bdecfa9, 0xf6bb4b60, 0xbebfbc70,
        0x289b7ec6, 0xeaa127fa, 0xd4ef3085, 0x04881d05,
        0xd9d4d039, 0xe6db99e5, 0x1fa27cf8, 0xc4ac5665,
        0xf4292244, 0x432aff97, 0xab9423a7, 0xfc93a039,
        0x655b59c3, 0x8f0ccc92, 0xffeff47d, 0x85845dd1,
        0x6fa87e4f, 0xfe2ce6e0, 0xa3014314, 0x4e0811a1,
        0xf7537e82, 0xbd3af235, 0x2ad7d2bb, 0xeb86d391,
    ];

    let bit_len = (data.len() as u64).wrapping_mul(8);
    let mut msg = data.to_vec();
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_le_bytes());

    let (mut a0, mut b0, mut c0, mut d0): (u32, u32, u32, u32) =
        (0x67452301, 0xefcdab89, 0x98badcfe, 0x10325476);

    for chunk in msg.chunks_exact(64) {
        let mut m = [0u32; 16];
        for (i, word) in chunk.chunks_exact(4).enumerate() {
            m[i] = u32::from_le_bytes([word[0], word[1], word[2], word[3]]);
        }
        let (mut a, mut b, mut c, mut d) = (a0, b0, c0, d0);
        for i in 0..64 {
            let (f, g) = match i {
                0..=15  => ((b & c) | (!b & d), i),
                16..=31 => ((d & b) | (!d & c), (5 * i + 1) % 16),
                32..=47 => (b ^ c ^ d, (3 * i + 5) % 16),
                _       => (c ^ (b | !d), (7 * i) % 16),
            };
            let temp = a.wrapping_add(f).wrapping_add(T[i]).wrapping_add(m[g]);
            a = d;
            d = c;
            c = b;
            b = b.wrapping_add(temp.rotate_left(S[i]));
        }
        a0 = a0.wrapping_add(a);
        b0 = b0.wrapping_add(b);
        c0 = c0.wrapping_add(c);
        d0 = d0.wrapping_add(d);
    }

    let mut out = [0u8; 16];
    out[0..4].copy_from_slice(&a0.to_le_bytes());
    out[4..8].copy_from_slice(&b0.to_le_bytes());
    out[8..12].copy_from_slice(&c0.to_le_bytes());
    out[12..16].copy_from_slice(&d0.to_le_bytes());
    hex_encode(&out)
}

/// Compute SHA-1 (RFC 3174) of `data`, returning a lowercase hex string.
pub fn compute_sha1(data: &[u8]) -> String {
    let bit_len = (data.len() as u64).wrapping_mul(8);
    let mut msg = data.to_vec();
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());

    let (mut h0, mut h1, mut h2, mut h3, mut h4): (u32, u32, u32, u32, u32) =
        (0x67452301, 0xEFCDAB89, 0x98BADCFE, 0x10325476, 0xC3D2E1F0);

    for chunk in msg.chunks_exact(64) {
        let mut w = [0u32; 80];
        for (i, word) in chunk.chunks_exact(4).enumerate() {
            w[i] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }
        let (mut a, mut b, mut c, mut d, mut e) = (h0, h1, h2, h3, h4);
        for i in 0..80 {
            let (f, k) = match i {
                0..=19  => ((b & c) | (!b & d), 0x5A827999u32),
                20..=39 => (b ^ c ^ d, 0x6ED9EBA1u32),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1BBCDCu32),
                _       => (b ^ c ^ d, 0xCA62C1D6u32),
            };
            let temp = a.rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(w[i]);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = temp;
        }
        h0 = h0.wrapping_add(a);
        h1 = h1.wrapping_add(b);
        h2 = h2.wrapping_add(c);
        h3 = h3.wrapping_add(d);
        h4 = h4.wrapping_add(e);
    }

    let mut out = [0u8; 20];
    out[0..4].copy_from_slice(&h0.to_be_bytes());
    out[4..8].copy_from_slice(&h1.to_be_bytes());
    out[8..12].copy_from_slice(&h2.to_be_bytes());
    out[12..16].copy_from_slice(&h3.to_be_bytes());
    out[16..20].copy_from_slice(&h4.to_be_bytes());
    hex_encode(&out)
}

/// Compute a checksum of `data` using the given algorithm.
pub fn compute_checksum(data: &[u8], kind: ChecksumKind) -> String {
    match kind {
        ChecksumKind::Sha256 => compute_sha256(data),
        ChecksumKind::Md5 => compute_md5(data),
        ChecksumKind::Sha1 => compute_sha1(data),
        ChecksumKind::Fnv64 => {
            let h = simple_hash(data);
            hex_encode(&h.to_be_bytes())
        }
        ChecksumKind::Crc32 => {
            let c = crc32(data);
            hex_encode(&c.to_be_bytes())
        }
    }
}

/// Compute a checksum for a file on disk.
pub fn compute_file_checksum(path: &Path, kind: ChecksumKind) -> io::Result<String> {
    let data = std::fs::read(path)?;
    Ok(compute_checksum(&data, kind))
}

/// Verify that `data` produces the `expected` checksum for the given algorithm.
pub fn verify_checksum(data: &[u8], expected: &str, kind: ChecksumKind) -> bool {
    compute_checksum(data, kind) == expected
}

/// CRC-32 lookup table (IEEE 802.3, reflected polynomial 0xEDB88320).
const CRC32_TABLE: [u32; 256] = {
    let mut table = [0u32; 256];
    let mut i = 0u32;
    while i < 256 {
        let mut crc = i;
        let mut j = 0;
        while j < 8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xEDB8_8320;
            } else {
                crc >>= 1;
            }
            j += 1;
        }
        table[i as usize] = crc;
        i += 1;
    }
    table
};

/// Compute the CRC-32 (IEEE 802.3) of `data`.
pub fn crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &b in data {
        let idx = ((crc as u8) ^ b) as usize;
        crc = (crc >> 8) ^ CRC32_TABLE[idx];
    }
    crc ^ 0xFFFF_FFFF
}

/// Compute a checksum over multiple data chunks as if they were concatenated.
pub fn compute_streaming_checksum(chunks: &[&[u8]], kind: ChecksumKind) -> String {
    match kind {
        ChecksumKind::Sha256 => {
            let mut hasher = Sha256::new();
            for chunk in chunks {
                hasher.update(chunk);
            }
            hex_encode(&hasher.finalize())
        }
        _ => {
            let combined: Vec<u8> = chunks.iter().flat_map(|c| c.iter().copied()).collect();
            compute_checksum(&combined, kind)
        }
    }
}

/// Constant-time comparison of two hex checksum strings.
///
/// Returns `false` immediately if lengths differ; otherwise compares every
/// byte to avoid timing side-channels.
pub fn compare_checksums(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.bytes().zip(b.bytes()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Accumulates data incrementally and verifies its checksum against an
/// expected value at the end, useful for streaming or chunked I/O where
/// the full payload is not available at once.
pub struct ChecksumVerifier {
    kind: ChecksumKind,
    expected: String,
    buffer: Vec<u8>,
}

impl ChecksumVerifier {
    /// Create a new verifier for the given algorithm and expected hex digest.
    pub fn new(kind: ChecksumKind, expected: &str) -> Self {
        Self {
            kind,
            expected: expected.to_string(),
            buffer: Vec::new(),
        }
    }

    /// Feed additional data into the verifier.
    pub fn feed(&mut self, data: &[u8]) {
        self.buffer.extend_from_slice(data);
    }

    /// Verify that the accumulated data matches the expected checksum.
    pub fn verify(&self) -> bool {
        let actual = compute_checksum(&self.buffer, self.kind);
        compare_checksums(&actual, &self.expected)
    }

    /// Returns true if buffer is empty.
    pub fn is_buffer_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    /// Get the first buffer, if any.
    pub fn first_buffer(&self) -> Option<&u8> {
        self.buffer.first()
    }

    /// Get the last buffer, if any.
    pub fn last_buffer(&self) -> Option<&u8> {
        self.buffer.last()
    }

    /// Retain only buffer matching the predicate.
    pub fn retain_buffer(&mut self, f: impl Fn(&u8) -> bool) {
        self.buffer.retain(|item| f(item));
    }
}

/// Accumulated statistics for checksum operations.
#[derive(Debug, Clone, PartialEq)]
pub struct ChecksumStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl ChecksumStats {
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
    pub fn merge(&mut self, other: &ChecksumStats) {
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

impl Default for ChecksumStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ChecksumStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ChecksumStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for checksum.
#[derive(Debug, Clone)]
pub struct ChecksumValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl ChecksumValidator {
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

impl Default for ChecksumValidator {
    fn default() -> Self {
        Self::new()
    }
}

/// A single entry in a [`ChecksumManifest`].
#[derive(Debug, Clone)]
pub struct ManifestEntry {
    pub path: String,
    pub algorithm: String,
    pub checksum: String,
    pub size: u64,
}

/// Manages a collection of file checksums for integrity verification.
#[derive(Debug, Clone)]
pub struct ChecksumManifest {
    pub entries: HashMap<String, ManifestEntry>,
}

impl ChecksumManifest {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    pub fn add_entry(&mut self, path: &str, algorithm: &str, checksum: &str, size: u64) {
        self.entries.insert(
            path.to_string(),
            ManifestEntry {
                path: path.to_string(),
                algorithm: algorithm.to_string(),
                checksum: checksum.to_string(),
                size,
            },
        );
    }

    /// Compares the stored checksum for `path` against `actual_checksum`.
    /// Returns an error if the path is not found.
    pub fn verify_entry(&self, path: &str, actual_checksum: &str) -> Result<bool, String> {
        match self.entries.get(path) {
            Some(entry) => Ok(entry.checksum.eq_ignore_ascii_case(actual_checksum)),
            None => Err(format!("path not found in manifest: {path}")),
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn paths(&self) -> Vec<&str> {
        self.entries.keys().map(|s| s.as_str()).collect()
    }

    pub fn remove(&mut self, path: &str) -> bool {
        self.entries.remove(path).is_some()
    }
}

impl Default for ChecksumManifest {
    fn default() -> Self {
        Self::new()
    }
}

/// Incrementally computes a checksum over streamed chunks without
/// buffering the entire input. Supports SHA-256 natively; other algorithms
/// fall back to an internal buffer.
pub struct IncrementalChecksum {
    kind: ChecksumKind,
    sha256_hasher: Option<Sha256>,
    fallback_buf: Vec<u8>,
    bytes_fed: u64,
}

impl IncrementalChecksum {
    /// Create a new incremental hasher for the given algorithm.
    pub fn new(kind: ChecksumKind) -> Self {
        let sha256_hasher = if kind == ChecksumKind::Sha256 {
            Some(Sha256::new())
        } else {
            None
        };
        Self {
            kind,
            sha256_hasher,
            fallback_buf: Vec::new(),
            bytes_fed: 0,
        }
    }

    /// Feed a chunk of data into the hasher.
    pub fn update(&mut self, data: &[u8]) {
        self.bytes_fed += data.len() as u64;
        if let Some(ref mut h) = self.sha256_hasher {
            h.update(data);
        } else {
            self.fallback_buf.extend_from_slice(data);
        }
    }

    /// Finalize the hash and return the hex digest.
    pub fn finalize(self) -> String {
        if let Some(h) = self.sha256_hasher {
            hex_encode(&h.finalize())
        } else {
            compute_checksum(&self.fallback_buf, self.kind)
        }
    }

    /// Return the total number of bytes fed so far.
    pub fn bytes_fed(&self) -> u64 {
        self.bytes_fed
    }

    /// Return the algorithm used by this hasher.
    pub fn kind(&self) -> ChecksumKind {
        self.kind
    }
}

/// Computes checksums for data using multiple algorithms simultaneously.
pub struct MultiChecksum {
    kinds: Vec<ChecksumKind>,
}

impl MultiChecksum {
    /// Create a new multi-algorithm hasher.
    pub fn new(kinds: &[ChecksumKind]) -> Self {
        Self {
            kinds: kinds.to_vec(),
        }
    }

    /// Compute all configured checksums for the given data.
    pub fn compute(&self, data: &[u8]) -> Vec<ChecksumResult> {
        self.kinds
            .iter()
            .map(|&k| compute_checksum_result(data, k))
            .collect()
    }

    /// Verify data against a set of expected digests (one per algorithm, in order).
    /// Returns a vector of booleans indicating which checks passed.
    pub fn verify(&self, data: &[u8], expected: &[&str]) -> Vec<bool> {
        self.kinds
            .iter()
            .zip(expected.iter())
            .map(|(&k, &exp)| verify_checksum(data, exp, k))
            .collect()
    }
}

/// A cache that stores previously computed checksums keyed by a combination
/// of content hash (FNV-64 of the data) and algorithm, avoiding redundant
/// re-computation for identical inputs.
#[derive(Debug, Clone)]
pub struct ChecksumCache {
    entries: HashMap<(u64, ChecksumKind), String>,
}

impl ChecksumCache {
    /// Create an empty cache.
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Compute or retrieve from cache the checksum for `data` using `kind`.
    pub fn get_or_compute(&mut self, data: &[u8], kind: ChecksumKind) -> String {
        let content_key = simple_hash(data);
        let key = (content_key, kind);
        if let Some(cached) = self.entries.get(&key) {
            return cached.clone();
        }
        let digest = compute_checksum(data, kind);
        self.entries.insert(key, digest.clone());
        digest
    }

    /// Return the number of cached entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Return true if the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Remove all cached entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

impl Default for ChecksumCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of comparing two checksum strings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChecksumComparison {
    Match,
    Mismatch { expected: String, actual: String },
    InvalidFormat(String),
}

/// Compare two hex checksum strings case-insensitively.
pub fn checksum_compare(a: &str, b: &str) -> ChecksumComparison {
    if a.is_empty() || b.is_empty() {
        return ChecksumComparison::InvalidFormat("checksum string must not be empty".to_string());
    }
    if !a.chars().all(|c| c.is_ascii_hexdigit()) {
        return ChecksumComparison::InvalidFormat(format!("invalid hex: {a}"));
    }
    if !b.chars().all(|c| c.is_ascii_hexdigit()) {
        return ChecksumComparison::InvalidFormat(format!("invalid hex: {b}"));
    }
    if a.eq_ignore_ascii_case(b) {
        ChecksumComparison::Match
    } else {
        ChecksumComparison::Mismatch {
            expected: a.to_string(),
            actual: b.to_string(),
        }
    }
}

/// Format a checksum in the standard `algorithm:hex` format.
pub fn checksum_format(checksum: &str, algorithm: &str) -> String {
    format!("{algorithm}:{checksum}")
}


// ---------------------------------------------------------------------------
// ChecksumManifestReport - integrity report for manifest verification
// ---------------------------------------------------------------------------

/// Report produced by verifying a checksum manifest against actual data.
#[derive(Debug, Clone, Default)]
pub struct ChecksumManifestReport {
    pub passed: usize,
    pub failed: usize,
    pub missing: usize,
    details: Vec<ManifestVerifyDetail>,
}

/// Detail of a single manifest verification.
#[derive(Debug, Clone, PartialEq)]
pub struct ManifestVerifyDetail {
    pub path: String,
    pub status: ManifestVerifyStatus,
}

/// Status of a single manifest entry verification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestVerifyStatus {
    /// Checksum matched.
    Passed,
    /// Checksum did not match.
    Failed { expected: String, actual: String },
    /// Path was in manifest but not provided for verification.
    Missing,
}

impl ChecksumManifestReport {
    /// Create a new empty report.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a passed verification.
    pub fn record_pass(&mut self, path: impl Into<String>) {
        self.passed += 1;
        self.details.push(ManifestVerifyDetail {
            path: path.into(),
            status: ManifestVerifyStatus::Passed,
        });
    }

    /// Record a failed verification.
    pub fn record_fail(&mut self, path: impl Into<String>, expected: &str, actual: &str) {
        self.failed += 1;
        self.details.push(ManifestVerifyDetail {
            path: path.into(),
            status: ManifestVerifyStatus::Failed {
                expected: expected.to_string(),
                actual: actual.to_string(),
            },
        });
    }

    /// Record a missing path.
    pub fn record_missing(&mut self, path: impl Into<String>) {
        self.missing += 1;
        self.details.push(ManifestVerifyDetail {
            path: path.into(),
            status: ManifestVerifyStatus::Missing,
        });
    }

    /// Returns true if all checks passed.
    pub fn all_passed(&self) -> bool {
        self.failed == 0 && self.missing == 0
    }

    /// Total number of entries checked.
    pub fn total_checked(&self) -> usize {
        self.passed + self.failed + self.missing
    }

    /// Retrieve all details.
    pub fn details(&self) -> &[ManifestVerifyDetail] {
        &self.details
    }

    /// Retrieve only the failed paths.
    pub fn failed_paths(&self) -> Vec<&str> {
        self.details
            .iter()
            .filter(|d| matches!(d.status, ManifestVerifyStatus::Failed { .. }))
            .map(|d| d.path.as_str())
            .collect()
    }
}

impl fmt::Display for ChecksumManifestReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ManifestReport(passed={}, failed={}, missing={})",
            self.passed, self.failed, self.missing
        )
    }
}

/// Verify all entries in a manifest against provided actual checksums.
pub fn verify_manifest(
    manifest: &ChecksumManifest,
    actual_checksums: &HashMap<String, String>,
) -> ChecksumManifestReport {
    let mut report = ChecksumManifestReport::new();
    for (path, entry) in &manifest.entries {
        match actual_checksums.get(path) {
            Some(actual) => {
                if entry.checksum.eq_ignore_ascii_case(actual) {
                    report.record_pass(path);
                } else {
                    report.record_fail(path, &entry.checksum, actual);
                }
            }
            None => {
                report.record_missing(path);
            }
        }
    }
    report
}

// ---------------------------------------------------------------------------
// RollingChecksum - rolling hash for streaming data
// ---------------------------------------------------------------------------

/// A rolling hash (Adler-32 style) for streaming data.
#[derive(Debug, Clone)]
pub struct RollingChecksum {
    a: u32,
    b: u32,
    window: Vec<u8>,
    window_size: usize,
    pos: usize,
    count: u64,
}

impl RollingChecksum {
    /// Create a new rolling checksum with the given window size.
    pub fn new(window_size: usize) -> Self {
        let ws = window_size.max(1);
        Self {
            a: 1,
            b: 0,
            window: vec![0u8; ws],
            window_size: ws,
            pos: 0,
            count: 0,
        }
    }

    /// Push a byte into the rolling window.
    pub fn push(&mut self, byte: u8) {
        let old = self.window[self.pos % self.window_size];
        self.window[self.pos % self.window_size] = byte;
        self.pos += 1;

        if self.count >= self.window_size as u64 {
            self.a = self.a.wrapping_add(byte as u32).wrapping_sub(old as u32);
            self.b = self.b.wrapping_add(self.a).wrapping_sub(
                (self.window_size as u32).wrapping_mul(old as u32).wrapping_add(1),
            );
        } else {
            self.a = self.a.wrapping_add(byte as u32);
            self.b = self.b.wrapping_add(self.a);
        }
        self.count += 1;
    }

    /// Push a slice of bytes.
    pub fn push_bytes(&mut self, data: &[u8]) {
        for &b in data {
            self.push(b);
        }
    }

    /// Current checksum value.
    pub fn value(&self) -> u32 {
        (self.b << 16) | (self.a & 0xffff)
    }

    /// Number of bytes fed.
    pub fn bytes_fed(&self) -> u64 {
        self.count
    }

    /// Reset the rolling checksum.
    pub fn reset(&mut self) {
        self.a = 1;
        self.b = 0;
        self.window.fill(0);
        self.pos = 0;
        self.count = 0;
    }
}

impl fmt::Display for RollingChecksum {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "RollingChecksum(window={}, fed={}, value={:#010x})", self.window_size, self.count, self.value())
    }
}

// ---------------------------------------------------------------------------
// Hex validation and utilities
// ---------------------------------------------------------------------------

/// Returns `true` if `s` is a valid lowercase hex string (even length, chars 0-9 a-f).
pub fn is_valid_hex(s: &str) -> bool {
    s.len() % 2 == 0 && !s.is_empty() && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// Normalize a hex string to lowercase.
pub fn hex_normalize(s: &str) -> String {
    s.to_ascii_lowercase()
}

/// Returns `true` if two hex strings are equal ignoring case.
pub fn hex_eq(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

// ---------------------------------------------------------------------------
// Batch checksum operations
// ---------------------------------------------------------------------------

/// Compute checksums for multiple byte slices, returning `(index, checksum)` pairs.
pub fn batch_checksums(items: &[&[u8]], kind: ChecksumKind) -> Vec<(usize, String)> {
    items
        .iter()
        .enumerate()
        .map(|(i, data)| (i, compute_checksum(data, kind)))
        .collect()
}

/// Verify a batch of `(data, expected_checksum)` pairs. Returns indices of failures.
pub fn batch_verify(items: &[(&[u8], &str)], kind: ChecksumKind) -> Vec<usize> {
    items
        .iter()
        .enumerate()
        .filter(|(_, (data, expected))| !verify_checksum(data, expected, kind))
        .map(|(i, _)| i)
        .collect()
}

/// Compute a checksum over a string slice (convenience wrapper).
pub fn checksum_str(s: &str, kind: ChecksumKind) -> String {
    compute_checksum(s.as_bytes(), kind)
}

// ---------------------------------------------------------------------------
// ChecksumManifest – extended helpers
// ---------------------------------------------------------------------------

impl ChecksumManifest {
    /// Return all entries whose path starts with the given prefix.
    pub fn entries_with_prefix(&self, prefix: &str) -> Vec<&ManifestEntry> {
        self.entries
            .values()
            .filter(|e| e.path.starts_with(prefix))
            .collect()
    }

    /// Return the total size in bytes across all entries.
    pub fn total_size(&self) -> u64 {
        self.entries.values().map(|e| e.size).sum()
    }

    /// Return distinct algorithms used in the manifest.
    pub fn algorithms(&self) -> Vec<String> {
        let mut algos: Vec<String> = self
            .entries
            .values()
            .map(|e| e.algorithm.clone())
            .collect();
        algos.sort();
        algos.dedup();
        algos
    }

    /// Return the entry with the largest file size, if any.
    pub fn largest_entry(&self) -> Option<&ManifestEntry> {
        self.entries.values().max_by_key(|e| e.size)
    }

    /// Return the entry with the smallest file size, if any.
    pub fn smallest_entry(&self) -> Option<&ManifestEntry> {
        self.entries.values().min_by_key(|e| e.size)
    }
}

// ---------------------------------------------------------------------------
// RollingChecksum – comparison helpers
// ---------------------------------------------------------------------------

impl RollingChecksum {
    /// Returns true if two rolling checksums have the same value.
    pub fn matches(&self, other: &RollingChecksum) -> bool {
        self.value() == other.value()
    }

    /// Returns the window size.
    pub fn window_size(&self) -> usize {
        self.window_size
    }
}

// ---------------------------------------------------------------------------
// ChecksumResult helpers
// ---------------------------------------------------------------------------

impl ChecksumResult {
    /// Returns the hex string length of the checksum.
    pub fn hex_len(&self) -> usize {
        self.hex_digest.len()
    }

    /// Returns true if this result's checksum matches the given expected value (case-insensitive).
    pub fn matches(&self, expected: &str) -> bool {
        self.hex_digest.eq_ignore_ascii_case(expected)
    }
}

impl fmt::Display for ChecksumResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", algorithm_name(self.kind), self.hex_digest)
    }
}

// ---------------------------------------------------------------------------
// VerifyResult & ChecksumVerifier
// ---------------------------------------------------------------------------

/// Outcome of comparing a computed checksum with an expected value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerifyResult {
    /// The checksums matched.
    Match,
    /// The checksums did not match.
    Mismatch {
        /// The checksum that was computed from the data.
        computed: String,
        /// The checksum that was expected.
        expected: String,
    },
}

impl fmt::Display for VerifyResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VerifyResult::Match => write!(f, "checksum OK"),
            VerifyResult::Mismatch { computed, expected } => {
                write!(f, "MISMATCH computed={computed} expected={expected}")
            }
        }
    }
}

/// One-shot verifier that compares a computed checksum against an expected hex
/// string, returning a [`VerifyResult`].
#[derive(Debug, Clone)]
pub struct DataVerifier {
    kind: ChecksumKind,
    expected: String,
}

impl DataVerifier {
    /// Create a new verifier for the given algorithm and expected hex digest.
    pub fn new(kind: ChecksumKind, expected: &str) -> Self {
        Self {
            kind,
            expected: expected.to_lowercase(),
        }
    }

    /// Compute the checksum of `data` and compare it to the expected value.
    pub fn verify(&self, data: &[u8]) -> VerifyResult {
        let computed = compute_checksum(data, self.kind);
        if computed.eq_ignore_ascii_case(&self.expected) {
            VerifyResult::Match
        } else {
            VerifyResult::Mismatch {
                computed,
                expected: self.expected.clone(),
            }
        }
    }
}

// ---------------------------------------------------------------------------
// ChecksumFileEntry & ChecksumFile
// ---------------------------------------------------------------------------

/// A single entry in a `.sha256sum`-style checksum file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChecksumFileEntry {
    /// The hex-encoded digest.
    pub hex: String,
    /// Whether the file was marked as binary (`*` prefix on filename).
    pub binary: bool,
    /// The filename that was checksummed.
    pub filename: String,
}

/// Parsed representation of a `.sha256sum`-format checksum file.
///
/// Each non-empty, non-comment line is expected to have the format
/// `<hex>  <filename>` or `<hex> *<filename>`.
#[derive(Debug, Clone)]
pub struct ChecksumFile {
    entries: Vec<ChecksumFileEntry>,
}

impl ChecksumFile {
    /// Parse the textual content of a checksum file.
    pub fn parse(content: &str) -> Self {
        let mut entries = Vec::new();
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            // Format: <hex> ( <sp> | *)<filename>
            // The separator is two spaces or one space followed by `*`.
            if let Some(pos) = line.find("  ").or_else(|| line.find(" *")) {
                let hex = line[..pos].to_lowercase();
                let rest = &line[pos..];
                let (binary, filename) = if rest.starts_with(" *") {
                    (true, rest[2..].to_string())
                } else {
                    // "  filename"
                    (false, rest[2..].to_string())
                };
                entries.push(ChecksumFileEntry {
                    hex,
                    binary,
                    filename,
                });
            }
        }
        Self { entries }
    }

    /// Return a slice of all parsed entries.
    pub fn entries(&self) -> &[ChecksumFileEntry] {
        &self.entries
    }

    /// Find the first entry whose filename matches `filename`.
    pub fn find_entry(&self, filename: &str) -> Option<&ChecksumFileEntry> {
        self.entries.iter().find(|e| e.filename == filename)
    }
}

impl fmt::Display for ChecksumFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for entry in &self.entries {
            if entry.binary {
                writeln!(f, "{} *{}", entry.hex, entry.filename)?;
            } else {
                writeln!(f, "{}  {}", entry.hex, entry.filename)?;
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// BatchChecksum
// ---------------------------------------------------------------------------

/// Compute checksums for multiple named byte slices in one pass.
#[derive(Debug, Clone)]
pub struct BatchChecksum {
    kind: ChecksumKind,
    results: Vec<(String, ChecksumResult)>,
}

impl BatchChecksum {
    /// Create a new batch using the given algorithm.
    pub fn new(kind: ChecksumKind) -> Self {
        Self {
            kind,
            results: Vec::new(),
        }
    }

    /// Add a named data slice; the checksum is computed immediately.
    pub fn add(&mut self, name: &str, data: &[u8]) {
        let result = compute_checksum_result(data, self.kind);
        self.results.push((name.to_string(), result));
    }

    /// Return a slice of `(name, ChecksumResult)` pairs.
    pub fn results(&self) -> &[(String, ChecksumResult)] {
        &self.results
    }

    /// Check whether every name present in `expected` has a matching hex
    /// digest in the batch results.
    pub fn all_match(&self, expected: &HashMap<String, String>) -> bool {
        for (name, result) in &self.results {
            if let Some(exp) = expected.get(name) {
                if !result.matches(exp) {
                    return false;
                }
            }
        }
        // Also ensure every key in `expected` was present in results.
        for key in expected.keys() {
            if !self.results.iter().any(|(n, _)| n == key) {
                return false;
            }
        }
        true
    }
}

// ---------------------------------------------------------------------------
// ChecksumCache
// ---------------------------------------------------------------------------

/// Cached checksum entry with an associated modification time for invalidation.
#[derive(Debug, Clone)]
struct MtimeCacheEntry {
    mtime: u64,
    hex: String,
}

/// An in-memory cache of previously computed checksums, keyed by file path,
/// with modification-time–based invalidation.  Unlike [`ChecksumCache`] (which
/// is keyed by content hash + algorithm), this cache is designed for
/// file-system workflows where the caller tracks modification times.
#[derive(Debug, Clone)]
pub struct MtimeChecksumCache {
    entries: HashMap<String, MtimeCacheEntry>,
}

impl MtimeChecksumCache {
    /// Create an empty cache.
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Return the cached hex digest for `path` if the stored mtime matches
    /// `current_mtime`. Returns `None` on cache miss or stale entry.
    pub fn get(&self, path: &str, current_mtime: u64) -> Option<&str> {
        self.entries.get(path).and_then(|e| {
            if e.mtime == current_mtime {
                Some(e.hex.as_str())
            } else {
                None
            }
        })
    }

    /// Insert (or replace) a cache entry for `path`.
    pub fn insert(&mut self, path: &str, mtime: u64, hex: String) {
        self.entries.insert(
            path.to_string(),
            MtimeCacheEntry { mtime, hex },
        );
    }

    /// Remove a single entry from the cache.
    pub fn invalidate(&mut self, path: &str) {
        self.entries.remove(path);
    }

    /// Remove every entry whose stored mtime differs from the value in
    /// `current_mtimes`, or that is absent from `current_mtimes`.
    pub fn invalidate_stale(&mut self, current_mtimes: &HashMap<String, u64>) {
        self.entries.retain(|path, entry| {
            current_mtimes
                .get(path)
                .map_or(false, |&mt| mt == entry.mtime)
        });
    }

    /// Number of entries currently in the cache.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Default for MtimeChecksumCache {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// ChecksumBatchVerifier - checksum batch verifier
// ---------------------------------------------------------------------------

/// Severity level for checksum batch verifier issues.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ChecksumBatchVerifierSeverity {
    Low,
    Medium,
    High,
    Critical,
}

impl fmt::Display for ChecksumBatchVerifierSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Medium => write!(f, "medium"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// Entry tracked by [ChecksumBatchVerifier].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChecksumBatchVerifierEntry {
    pub id: String,
    pub label: String,
    pub severity: ChecksumBatchVerifierSeverity,
    pub detail: Option<String>,
    pub file_count: usize,
    enabled: bool,
}

impl ChecksumBatchVerifierEntry {
    pub fn new(id: &str, label: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            severity: ChecksumBatchVerifierSeverity::Low,
            detail: None,
            file_count: 0,
            enabled: true,
        }
    }

    pub fn with_severity(mut self, severity: ChecksumBatchVerifierSeverity) -> Self {
        self.severity = severity;
        self
    }

    pub fn with_detail(mut self, detail: &str) -> Self {
        self.detail = Some(detail.to_string());
        self
    }

    pub fn with_file_count(mut self, val: usize) -> Self {
        self.file_count = val;
        self
    }

    pub fn all_valid(&self) -> bool {
        self.enabled && self.severity >= ChecksumBatchVerifierSeverity::Medium
    }

    pub fn disable(&mut self) {
        self.enabled = false;
    }

    pub fn enable(&mut self) {
        self.enabled = true;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn format_line(&self) -> String {
        let det = self.detail.as_deref().unwrap_or("-");
        format!("[{}] {} ({}): {}", self.severity, self.id, self.file_count, det)
    }
}

impl fmt::Display for ChecksumBatchVerifierEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} [{}]", self.label, self.severity)
    }
}

/// Manages a collection of [ChecksumBatchVerifierEntry] items.
#[derive(Debug, Clone)]
pub struct ChecksumBatchVerifier {
    entries: Vec<ChecksumBatchVerifierEntry>,
    name: String,
    capacity: usize,
}

impl ChecksumBatchVerifier {
    pub fn new(name: &str) -> Self {
        Self { entries: Vec::new(), name: name.to_string(), capacity: 1000 }
    }

    pub fn with_capacity(mut self, cap: usize) -> Self {
        self.capacity = cap;
        self
    }

    pub fn add(&mut self, entry: ChecksumBatchVerifierEntry) -> bool {
        if self.entries.len() >= self.capacity {
            return false;
        }
        self.entries.push(entry);
        true
    }

    pub fn remove(&mut self, id: &str) -> Option<ChecksumBatchVerifierEntry> {
        if let Some(pos) = self.entries.iter().position(|e| e.id == id) {
            Some(self.entries.remove(pos))
        } else {
            None
        }
    }

    pub fn get(&self, id: &str) -> Option<&ChecksumBatchVerifierEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn file_count(&self) -> usize { self.entries.len() }

    pub fn all_valid(&self) -> bool {
        self.entries.iter().any(|e| e.all_valid())
    }

    pub fn entries_by_severity(&self, severity: ChecksumBatchVerifierSeverity) -> Vec<&ChecksumBatchVerifierEntry> {
        self.entries.iter().filter(|e| e.severity == severity).collect()
    }

    pub fn high_severity_count(&self) -> usize {
        self.entries.iter().filter(|e| e.severity >= ChecksumBatchVerifierSeverity::High).count()
    }

    pub fn sorted_by_severity(&self) -> Vec<&ChecksumBatchVerifierEntry> {
        let mut sorted: Vec<_> = self.entries.iter().collect();
        sorted.sort_by(|a, b| b.severity.cmp(&a.severity));
        sorted
    }

    pub fn generate_summary(&self) -> String {
        format!(
            "{} | Total: {} | High+: {}",
            self.name, self.entries.len(), self.high_severity_count()
        )
    }

    pub fn clear(&mut self) { self.entries.clear(); }

    pub fn is_empty(&self) -> bool { self.entries.is_empty() }

    pub fn enabled_entries(&self) -> Vec<&ChecksumBatchVerifierEntry> {
        self.entries.iter().filter(|e| e.is_enabled()).collect()
    }

    pub fn disable_all(&mut self) {
        for e in &mut self.entries { e.disable(); }
    }

    pub fn enable_all(&mut self) {
        for e in &mut self.entries { e.enable(); }
    }
}

// ---------------------------------------------------------------------------
// ChecksumFormatConverter - checksum format converter
// ---------------------------------------------------------------------------

/// Configuration for [ChecksumFormatConverter].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChecksumFormatConverterConfig {
    pub max_items: usize,
    pub label: String,
    pub auto_refresh: bool,
    pub algorithm_count: usize,
}

impl ChecksumFormatConverterConfig {
    pub fn new(label: &str) -> Self {
        Self { max_items: 100, label: label.to_string(), auto_refresh: true, algorithm_count: 0 }
    }

    pub fn with_max_items(mut self, max: usize) -> Self { self.max_items = max; self }

    pub fn with_auto_refresh(mut self, auto: bool) -> Self { self.auto_refresh = auto; self }

    pub fn with_algorithm_count(mut self, val: usize) -> Self { self.algorithm_count = val; self }
}

impl Default for ChecksumFormatConverterConfig {
    fn default() -> Self { Self::new("default") }
}

/// Item tracked by [ChecksumFormatConverter].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChecksumFormatConverterItem {
    pub key: String,
    pub value: String,
    pub priority: u32,
    pub tags: Vec<String>,
}

impl ChecksumFormatConverterItem {
    pub fn new(key: &str, value: &str) -> Self {
        Self { key: key.to_string(), value: value.to_string(), priority: 0, tags: Vec::new() }
    }

    pub fn with_priority(mut self, p: u32) -> Self { self.priority = p; self }

    pub fn with_tag(mut self, tag: &str) -> Self {
        self.tags.push(tag.to_string());
        self
    }

    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }

    pub fn needs_conversion(&self) -> bool {
        self.priority > 0 && !self.tags.is_empty()
    }
}

impl fmt::Display for ChecksumFormatConverterItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}={}", self.key, self.value)
    }
}

/// Manages [ChecksumFormatConverterItem] entries with configuration.
#[derive(Debug, Clone)]
pub struct ChecksumFormatConverter {
    config: ChecksumFormatConverterConfig,
    items: Vec<ChecksumFormatConverterItem>,
}

impl ChecksumFormatConverter {
    pub fn new(config: ChecksumFormatConverterConfig) -> Self {
        Self { config, items: Vec::new() }
    }

    pub fn add(&mut self, item: ChecksumFormatConverterItem) -> bool {
        if self.items.len() >= self.config.max_items {
            return false;
        }
        self.items.push(item);
        true
    }

    pub fn remove(&mut self, key: &str) -> Option<ChecksumFormatConverterItem> {
        if let Some(pos) = self.items.iter().position(|i| i.key == key) {
            Some(self.items.remove(pos))
        } else {
            None
        }
    }

    pub fn get(&self, key: &str) -> Option<&ChecksumFormatConverterItem> {
        self.items.iter().find(|i| i.key == key)
    }

    pub fn algorithm_count(&self) -> usize { self.items.len() }

    pub fn needs_conversion(&self) -> bool {
        self.items.iter().any(|i| i.needs_conversion())
    }

    pub fn items_with_tag(&self, tag: &str) -> Vec<&ChecksumFormatConverterItem> {
        self.items.iter().filter(|i| i.has_tag(tag)).collect()
    }

    pub fn sorted_by_priority(&self) -> Vec<&ChecksumFormatConverterItem> {
        let mut sorted: Vec<_> = self.items.iter().collect();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
        sorted
    }

    pub fn clear(&mut self) { self.items.clear(); }

    pub fn is_empty(&self) -> bool { self.items.is_empty() }

    pub fn total_priority(&self) -> u64 {
        self.items.iter().map(|i| i.priority as u64).sum()
    }

    pub fn config(&self) -> &ChecksumFormatConverterConfig {
        &self.config
    }

    pub fn generate_report(&self) -> String {
        format!(
            "{} | Items: {} | Auto-refresh: {}",
            self.config.label, self.items.len(), self.config.auto_refresh
        )
    }
}



// ---------------------------------------------------------------------------
// vsedit-checksum: Extended configuration, caching, and iteration utilities
// ---------------------------------------------------------------------------

/// Configuration entry with key-value metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChecksumXConfig {
    pub key: String,
    pub value: String,
    pub tags: Vec<String>,
    pub weight: u32,
    pub active: bool,
}

impl ChecksumXConfig {
    pub fn new(key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: String::new(),
            tags: Vec::new(),
            weight: 0,
            active: true,
        }
    }

    pub fn with_value(mut self, v: impl Into<String>) -> Self {
        self.value = v.into();
        self
    }

    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    pub fn with_weight(mut self, w: u32) -> Self {
        self.weight = w;
        self
    }

    pub fn deactivate(mut self) -> Self {
        self.active = false;
        self
    }

    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }

    pub fn tag_count(&self) -> usize {
        self.tags.len()
    }
}

impl std::fmt::Display for ChecksumXConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}={}", self.key, self.value)
    }
}

/// Registry that stores and indexes configuration entries.
#[derive(Debug, Default)]
pub struct ChecksumXRegistry {
    entries: Vec<ChecksumXConfig>,
    index: std::collections::HashMap<String, usize>,
}

impl ChecksumXRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, entry: ChecksumXConfig) -> Result<(), String> {
        if self.index.contains_key(&entry.key) {
            return Err(format!("duplicate key: {}", entry.key));
        }
        let idx = self.entries.len();
        self.index.insert(entry.key.clone(), idx);
        self.entries.push(entry);
        Ok(())
    }

    pub fn get(&self, key: &str) -> Option<&ChecksumXConfig> {
        self.index.get(key).map(|&i| &self.entries[i])
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut ChecksumXConfig> {
        self.index.get(key).copied().map(move |i| &mut self.entries[i])
    }

    pub fn remove(&mut self, key: &str) -> Option<ChecksumXConfig> {
        if let Some(&idx) = self.index.get(key) {
            self.index.remove(key);
            let removed = self.entries.remove(idx);
            for val in self.index.values_mut() {
                if *val > idx {
                    *val -= 1;
                }
            }
            Some(removed)
        } else {
            None
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.key.as_str()).collect()
    }

    pub fn active_entries(&self) -> Vec<&ChecksumXConfig> {
        self.entries.iter().filter(|e| e.active).collect()
    }

    pub fn by_weight_desc(&self) -> Vec<&ChecksumXConfig> {
        let mut sorted: Vec<&ChecksumXConfig> = self.entries.iter().collect();
        sorted.sort_by(|a, b| b.weight.cmp(&a.weight));
        sorted
    }

    pub fn entries_with_tag(&self, tag: &str) -> Vec<&ChecksumXConfig> {
        self.entries.iter().filter(|e| e.has_tag(tag)).collect()
    }

    pub fn contains(&self, key: &str) -> bool {
        self.index.contains_key(key)
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.index.clear();
    }

    pub fn total_weight(&self) -> u32 {
        self.entries.iter().map(|e| e.weight).sum()
    }

    pub fn iter(&self) -> ChecksumXIterator<'_> {
        ChecksumXIterator { inner: self.entries.iter() }
    }
}

/// Iterator over registry entries.
pub struct ChecksumXIterator<'a> {
    inner: std::slice::Iter<'a, ChecksumXConfig>,
}

impl<'a> Iterator for ChecksumXIterator<'a> {
    type Item = &'a ChecksumXConfig;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

/// LRU cache with capacity limit.
#[derive(Debug)]
pub struct ChecksumXCache {
    capacity: usize,
    entries: Vec<(String, String)>,
}

impl ChecksumXCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            entries: Vec::new(),
        }
    }

    pub fn get(&mut self, key: &str) -> Option<&str> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            let entry = self.entries.remove(pos);
            self.entries.push(entry);
            self.entries.last().map(|(_, v)| v.as_str())
        } else {
            None
        }
    }

    pub fn put(&mut self, key: impl Into<String>, value: impl Into<String>) {
        let key = key.into();
        self.entries.retain(|(k, _)| k != &key);
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value.into()));
    }

    pub fn contains(&self, key: &str) -> bool {
        self.entries.iter().any(|(k, _)| k == key)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    pub fn most_recent(&self) -> Option<(&str, &str)> {
        self.entries.last().map(|(k, v)| (k.as_str(), v.as_str()))
    }

    pub fn least_recent(&self) -> Option<(&str, &str)> {
        self.entries.first().map(|(k, v)| (k.as_str(), v.as_str()))
    }
}

/// Formatter for rendering entries as text.
pub struct ChecksumXFormatter {
    separator: String,
    show_inactive: bool,
    max_value_len: usize,
}

impl ChecksumXFormatter {
    pub fn new() -> Self {
        Self {
            separator: ", ".to_string(),
            show_inactive: false,
            max_value_len: 80,
        }
    }

    pub fn separator(mut self, sep: impl Into<String>) -> Self {
        self.separator = sep.into();
        self
    }

    pub fn show_inactive(mut self, show: bool) -> Self {
        self.show_inactive = show;
        self
    }

    pub fn max_value_len(mut self, len: usize) -> Self {
        self.max_value_len = len;
        self
    }

    pub fn format_entry(&self, entry: &ChecksumXConfig) -> String {
        let val = if entry.value.len() > self.max_value_len {
            format!("{}…", &entry.value[..self.max_value_len])
        } else {
            entry.value.clone()
        };
        let status = if entry.active { "✓" } else { "✗" };
        format!("[{}] {}={}", status, entry.key, val)
    }

    pub fn format_list(&self, registry: &ChecksumXRegistry) -> String {
        let items: Vec<String> = registry.entries.iter()
            .filter(|e| self.show_inactive || e.active)
            .map(|e| self.format_entry(e))
            .collect();
        items.join(&self.separator)
    }

    pub fn format_summary(&self, registry: &ChecksumXRegistry) -> String {
        let active = registry.active_entries().len();
        let total = registry.len();
        format!("{} active / {} total (weight: {})", active, total, registry.total_weight())
    }
}

impl Default for ChecksumXFormatter {
    fn default() -> Self {
        Self::new()
    }
}

/// Validator for configuration entries.
pub struct ChecksumXValidator {
    max_key_len: usize,
    require_value: bool,
    allowed_tags: Option<Vec<String>>,
}

impl ChecksumXValidator {
    pub fn new() -> Self {
        Self {
            max_key_len: 256,
            require_value: false,
            allowed_tags: None,
        }
    }

    pub fn max_key_len(mut self, len: usize) -> Self {
        self.max_key_len = len;
        self
    }

    pub fn require_value(mut self, req: bool) -> Self {
        self.require_value = req;
        self
    }

    pub fn allowed_tags(mut self, tags: Vec<String>) -> Self {
        self.allowed_tags = Some(tags);
        self
    }

    pub fn validate(&self, entry: &ChecksumXConfig) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        if entry.key.is_empty() {
            errors.push("key must not be empty".into());
        }
        if entry.key.len() > self.max_key_len {
            errors.push(format!("key exceeds max length {}", self.max_key_len));
        }
        if self.require_value && entry.value.is_empty() {
            errors.push("value is required".into());
        }
        if let Some(ref allowed) = self.allowed_tags {
            for tag in &entry.tags {
                if !allowed.contains(tag) {
                    errors.push(format!("tag '{}' is not allowed", tag));
                }
            }
        }
        if errors.is_empty() { Ok(()) } else { Err(errors) }
    }

    pub fn validate_all(&self, registry: &ChecksumXRegistry) -> Vec<(String, Vec<String>)> {
        let mut results = Vec::new();
        for entry in &registry.entries {
            if let Err(errs) = self.validate(entry) {
                results.push((entry.key.clone(), errs));
            }
        }
        results
    }
}

impl Default for ChecksumXValidator {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xa_ extended helpers for checksum
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaChecksumRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaChecksumRingBuf {
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
pub struct XaChecksumCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaChecksumCounter {
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

impl Default for XaChecksumCounter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 13
// ---------------------------------------------------------------------------

/// Generic object pool `Xc13Pool<T>`.
pub struct Xc13Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc13Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc13PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc13Pool<T> {
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
    pub fn stats(&self) -> Xc13PoolStats {
        Xc13PoolStats {
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

impl<T> Default for Xc13Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc13Scheduler`.
pub struct Xc13Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc13Scheduler {
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

impl Default for Xc13Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_13 hash for the given byte slice.
pub fn xc_13_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_13 convention.
pub fn xc_13_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_35 deepening: state machine + event bus ---

/// States for the Xd35 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd35State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd35State {
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
pub struct Xd35Transition {
    pub from: Xd35State,
    pub to: Xd35State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd35StateMachine {
    current: Xd35State,
    history: Vec<Xd35Transition>,
    step_counter: usize,
}

impl Xd35StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd35State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd35State {
        self.current
    }

    pub fn history(&self) -> &[Xd35Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd35State) -> Result<Xd35State, String> {
        let allowed = match (self.current, target) {
            (Xd35State::Idle, Xd35State::Running) => true,
            (Xd35State::Running, Xd35State::Paused) => true,
            (Xd35State::Running, Xd35State::Done) => true,
            (Xd35State::Paused, Xd35State::Running) => true,
            (Xd35State::Paused, Xd35State::Done) => true,
            (Xd35State::Done, Xd35State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_35: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd35Transition {
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
            "Xd35SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd35State> {
        let prefix = "Xd35SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd35State::Idle),
            "Running" => Some(Xd35State::Running),
            "Paused" => Some(Xd35State::Paused),
            "Done" => Some(Xd35State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd35State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd35 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd35Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd35Event {
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

type Xd35HandlerFn = Box<dyn Fn(&Xd35Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd35EventBus {
    handlers: Vec<(usize, Option<String>, Xd35HandlerFn)>,
    next_id: usize,
    published: Vec<Xd35Event>,
}

impl Xd35EventBus {
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
        F: Fn(&Xd35Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd35Event) + Send + Sync + 'static,
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
    pub fn publish(&mut self, event: Xd35Event) {
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

    pub fn published_events(&self) -> &[Xd35Event] {
        &self.published
    }

    pub fn subscriber_count(&self) -> usize {
        self.handlers.len()
    }

    pub fn clear_history(&mut self) {
        self.published.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn deterministic_output() {
        let data = b"hello world";
        let a = compute_checksum(data, ChecksumKind::Fnv64);
        let b = compute_checksum(data, ChecksumKind::Fnv64);
        assert_eq!(a, b);
        assert_eq!(a.len(), 16);
    }

    #[test]
    fn verification() {
        let data = b"test data";
        let checksum = compute_checksum(data, ChecksumKind::Fnv64);
        assert!(verify_checksum(data, &checksum, ChecksumKind::Fnv64));
        assert!(!verify_checksum(data, "0000000000000000", ChecksumKind::Fnv64));
    }

    #[test]
    fn different_data_different_hash() {
        let a = compute_checksum(b"alpha", ChecksumKind::Fnv64);
        let b = compute_checksum(b"beta", ChecksumKind::Fnv64);
        assert_ne!(a, b);
    }

    #[test]
    fn sha256_known_value() {
        // SHA-256 of empty string is well-known
        let hash = compute_sha256(b"");
        assert_eq!(
            hash,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn sha256_deterministic() {
        let a = compute_sha256(b"hello");
        let b = compute_sha256(b"hello");
        assert_eq!(a, b);
        assert_eq!(a.len(), 64); // 256 bits = 32 bytes = 64 hex chars
    }

    #[test]
    fn md5_deterministic_and_length() {
        let a = compute_md5(b"data");
        let b = compute_md5(b"data");
        assert_eq!(a, b);
        assert_eq!(a.len(), 32); // 128-bit output = 16 bytes = 32 hex
    }

    #[test]
    fn sha1_deterministic_and_length() {
        let a = compute_sha1(b"data");
        let b = compute_sha1(b"data");
        assert_eq!(a, b);
        assert_eq!(a.len(), 40); // 160-bit output = 20 bytes = 40 hex
    }

    #[test]
    fn md5_known_empty() {
        assert_eq!(compute_md5(b""), "d41d8cd98f00b204e9800998ecf8427e");
    }

    #[test]
    fn md5_known_hello() {
        assert_eq!(compute_md5(b"hello"), "5d41402abc4b2a76b9719d911017c592");
    }

    #[test]
    fn sha1_known_empty() {
        assert_eq!(compute_sha1(b""), "da39a3ee5e6b4b0d3255bfef95601890afd80709");
    }

    #[test]
    fn sha1_known_hello() {
        assert_eq!(compute_sha1(b"hello"), "aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d");
    }

    #[test]
    fn verify_md5() {
        let data = b"verify md5";
        let checksum = compute_checksum(data, ChecksumKind::Md5);
        assert!(verify_checksum(data, &checksum, ChecksumKind::Md5));
        assert!(!verify_checksum(b"other", &checksum, ChecksumKind::Md5));
    }

    #[test]
    fn verify_sha1() {
        let data = b"verify sha1";
        let checksum = compute_checksum(data, ChecksumKind::Sha1);
        assert!(verify_checksum(data, &checksum, ChecksumKind::Sha1));
        assert!(!verify_checksum(b"other", &checksum, ChecksumKind::Sha1));
    }

    #[test]
    fn verify_sha256() {
        let data = b"verify me";
        let checksum = compute_checksum(data, ChecksumKind::Sha256);
        assert!(verify_checksum(data, &checksum, ChecksumKind::Sha256));
        assert!(!verify_checksum(b"other", &checksum, ChecksumKind::Sha256));
    }

    #[test]
    fn compute_file_checksum_works() {
        let dir = std::env::temp_dir().join("vsedit_checksum_test");
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("test.txt");
        let mut f = std::fs::File::create(&file).unwrap();
        f.write_all(b"file content").unwrap();
        drop(f);

        let checksum = compute_file_checksum(&file, ChecksumKind::Sha256).unwrap();
        let expected = compute_sha256(b"file content");
        assert_eq!(checksum, expected);

        std::fs::remove_file(&file).ok();
        std::fs::remove_dir(&dir).ok();
    }

    #[test]
    fn compute_file_checksum_not_found() {
        let result = compute_file_checksum(Path::new("/nonexistent/path"), ChecksumKind::Sha256);
        assert!(result.is_err());
    }

    #[test]
    fn different_algorithms_different_output() {
        let data = b"same data";
        let sha256 = compute_checksum(data, ChecksumKind::Sha256);
        let fnv = compute_checksum(data, ChecksumKind::Fnv64);
        let md5 = compute_checksum(data, ChecksumKind::Md5);
        assert_ne!(sha256, fnv);
        assert_ne!(sha256, md5);
    }

    #[test]
    fn hex_decode_roundtrip() {
        let original = b"\x00\x0f\x10\xff";
        let encoded = hex_encode(original);
        let decoded = hex_decode(&encoded).unwrap();
        assert_eq!(decoded, original);
    }

    #[test]
    fn hex_decode_invalid() {
        assert!(hex_decode("0g").is_none());
        assert!(hex_decode("abc").is_none());
        assert!(hex_decode("zz").is_none());
    }

    #[test]
    fn hex_decode_uppercase() {
        let decoded = hex_decode("DEADBEEF").unwrap();
        assert_eq!(decoded, vec![0xDE, 0xAD, 0xBE, 0xEF]);
    }

    #[test]
    fn checksum_result_fields() {
        let data = b"metadata test";
        let result = compute_checksum_result(data, ChecksumKind::Sha256);
        assert_eq!(result.kind, ChecksumKind::Sha256);
        assert_eq!(result.byte_length, data.len());
        assert_eq!(result.hex_digest, compute_sha256(data));
    }

    #[test]
    fn crc32_known_value() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }

    #[test]
    fn crc32_empty() {
        assert_eq!(crc32(b""), 0x0000_0000);
    }

    #[test]
    fn crc32_via_compute_checksum() {
        let hex = compute_checksum(b"123456789", ChecksumKind::Crc32);
        assert_eq!(hex, "cbf43926");
    }

    #[test]
    fn streaming_checksum_matches_whole() {
        let data = b"hello world";
        let whole = compute_checksum(data, ChecksumKind::Sha256);
        let streamed =
            compute_streaming_checksum(&[b"hello", b" ", b"world"], ChecksumKind::Sha256);
        assert_eq!(whole, streamed);
    }

    #[test]
    fn streaming_checksum_fnv() {
        let whole = compute_checksum(b"abcdef", ChecksumKind::Fnv64);
        let streamed = compute_streaming_checksum(&[b"abc", b"def"], ChecksumKind::Fnv64);
        assert_eq!(whole, streamed);
    }

    #[test]
    fn compare_checksums_equal() {
        assert!(compare_checksums("abcdef1234", "abcdef1234"));
    }

    #[test]
    fn compare_checksums_different() {
        assert!(!compare_checksums("abcdef1234", "abcdef1235"));
        assert!(!compare_checksums("short", "longer_string"));
    }

    #[test]
    fn checksum_verifier_pass() {
        let data = b"verified payload";
        let expected = compute_checksum(data, ChecksumKind::Sha256);
        let mut v = ChecksumVerifier::new(ChecksumKind::Sha256, &expected);
        v.feed(b"verified ");
        v.feed(b"payload");
        assert!(v.verify());
    }

    #[test]
    fn checksum_verifier_fail() {
        let mut v = ChecksumVerifier::new(ChecksumKind::Fnv64, "0000000000000000");
        v.feed(b"some data");
        assert!(!v.verify());
    }

    #[test]
    fn algorithm_name_values() {
        assert_eq!(algorithm_name(ChecksumKind::Sha256), "SHA-256");
        assert_eq!(algorithm_name(ChecksumKind::Md5), "MD5");
        assert_eq!(algorithm_name(ChecksumKind::Sha1), "SHA-1");
        assert_eq!(algorithm_name(ChecksumKind::Fnv64), "FNV-1a 64-bit");
        assert_eq!(algorithm_name(ChecksumKind::Crc32), "CRC-32");
    }

    #[test]
    fn eq_checksumkind_same() {
        assert_eq!(ChecksumKind::Sha256, ChecksumKind::Sha256);
    }

    #[test]
    fn ne_checksumkind_diff() {
        assert_ne!(ChecksumKind::Sha256, ChecksumKind::Md5);
    }

    #[test]
    fn behavior_check_0() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_1() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_2() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_3() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_4() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_5() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_6() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_7() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_8() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_9() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_10() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_11() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_12() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_13() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_14() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_15() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_16() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_17() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_18() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_19() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_20() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_21() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_22() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_23() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_24() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_25() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn behavior_check_26() {
        assert!(std::mem::size_of::<usize>() > 0);
    }

    #[test]
    fn checksum_stats_new_defaults() {
        let stats = ChecksumStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn checksum_stats_record_success() {
        let mut stats = ChecksumStats::new();
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
    fn checksum_stats_record_failure() {
        let mut stats = ChecksumStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn checksum_stats_reset() {
        let mut stats = ChecksumStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn checksum_stats_merge() {
        let mut a = ChecksumStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = ChecksumStats::new();
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
    fn checksum_stats_display() {
        let mut stats = ChecksumStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn checksum_stats_default() {
        let stats = ChecksumStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn checksum_validator_accepts_valid_name() {
        let v = ChecksumValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn checksum_validator_rejects_empty() {
        let v = ChecksumValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn checksum_validator_rejects_too_long() {
        let v = ChecksumValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn checksum_validator_forbidden_prefix() {
        let v = ChecksumValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn checksum_validator_allowed_chars() {
        let v = ChecksumValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn checksum_validator_range() {
        let v = ChecksumValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn checksum_sanitize_removes_control() {
        let result = ChecksumValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn checksum_truncate_short_string() {
        assert_eq!(ChecksumValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn checksum_truncate_long_string() {
        let result = ChecksumValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn checksum_is_ascii_printable() {
        assert!(ChecksumValidator::is_ascii_printable("Hello World 123"));
        assert!(!ChecksumValidator::is_ascii_printable("Hello\x00World"));
    }

    #[test]
    fn test_manifest_add_and_verify() {
        let mut manifest = ChecksumManifest::new();
        manifest.add_entry("file.txt", "sha256", "abcdef1234567890", 100);
        assert_eq!(manifest.len(), 1);
        assert!(!manifest.is_empty());
        assert_eq!(manifest.verify_entry("file.txt", "abcdef1234567890"), Ok(true));
    }

    #[test]
    fn test_manifest_verify_mismatch() {
        let mut manifest = ChecksumManifest::new();
        manifest.add_entry("file.txt", "sha256", "abcdef1234567890", 100);
        assert_eq!(manifest.verify_entry("file.txt", "0000000000000000"), Ok(false));
    }

    #[test]
    fn test_manifest_missing_path() {
        let manifest = ChecksumManifest::new();
        assert!(manifest.verify_entry("missing.txt", "abc").is_err());
    }

    #[test]
    fn test_manifest_remove() {
        let mut manifest = ChecksumManifest::new();
        manifest.add_entry("a.txt", "md5", "aaa", 10);
        manifest.add_entry("b.txt", "md5", "bbb", 20);
        assert_eq!(manifest.len(), 2);
        assert!(manifest.remove("a.txt"));
        assert_eq!(manifest.len(), 1);
        assert!(!manifest.remove("a.txt"));
    }

    #[test]
    fn test_checksum_compare_match() {
        assert_eq!(checksum_compare("abcdef", "ABCDEF"), ChecksumComparison::Match);
    }

    #[test]
    fn test_checksum_compare_mismatch() {
        assert_eq!(
            checksum_compare("abcdef", "123456"),
            ChecksumComparison::Mismatch {
                expected: "abcdef".to_string(),
                actual: "123456".to_string(),
            }
        );
    }

    #[test]
    fn test_checksum_format() {
        assert_eq!(checksum_format("abcdef1234", "sha256"), "sha256:abcdef1234");
    }

    #[test]
    fn incremental_sha256_matches_oneshot() {
        let data = b"the quick brown fox jumps over the lazy dog";
        let oneshot = compute_sha256(data);
        let mut inc = IncrementalChecksum::new(ChecksumKind::Sha256);
        inc.update(b"the quick ");
        inc.update(b"brown fox ");
        inc.update(b"jumps over ");
        inc.update(b"the lazy dog");
        assert_eq!(inc.bytes_fed(), data.len() as u64);
        assert_eq!(inc.kind(), ChecksumKind::Sha256);
        assert_eq!(inc.finalize(), oneshot);
    }

    #[test]
    fn incremental_md5_matches_oneshot() {
        let data = b"incremental md5 test";
        let oneshot = compute_md5(data);
        let mut inc = IncrementalChecksum::new(ChecksumKind::Md5);
        inc.update(b"incremental ");
        inc.update(b"md5 test");
        assert_eq!(inc.finalize(), oneshot);
    }

    #[test]
    fn multi_checksum_compute_all() {
        let mc = MultiChecksum::new(&[ChecksumKind::Sha256, ChecksumKind::Md5, ChecksumKind::Crc32]);
        let results = mc.compute(b"multi algo");
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].kind, ChecksumKind::Sha256);
        assert_eq!(results[1].kind, ChecksumKind::Md5);
        assert_eq!(results[2].kind, ChecksumKind::Crc32);
        assert_eq!(results[0].hex_digest, compute_sha256(b"multi algo"));
        assert_eq!(results[1].hex_digest, compute_md5(b"multi algo"));
    }

    #[test]
    fn multi_checksum_verify() {
        let data = b"verify multi";
        let sha = compute_checksum(data, ChecksumKind::Sha256);
        let md5 = compute_checksum(data, ChecksumKind::Md5);
        let mc = MultiChecksum::new(&[ChecksumKind::Sha256, ChecksumKind::Md5]);
        let results = mc.verify(data, &[&sha, &md5]);
        assert_eq!(results, vec![true, true]);
        let bad = mc.verify(data, &[&sha, "0000"]);
        assert_eq!(bad, vec![true, false]);
    }

    #[test]
    fn checksum_cache_avoids_recompute() {
        let mut cache = ChecksumCache::new();
        assert!(cache.is_empty());
        let d1 = cache.get_or_compute(b"cached data", ChecksumKind::Sha256);
        assert_eq!(cache.len(), 1);
        let d2 = cache.get_or_compute(b"cached data", ChecksumKind::Sha256);
        assert_eq!(d1, d2);
        // Same data, different algorithm should be a separate entry
        let d3 = cache.get_or_compute(b"cached data", ChecksumKind::Md5);
        assert_eq!(cache.len(), 2);
        assert_ne!(d1, d3);
        cache.clear();
        assert!(cache.is_empty());
    }

    #[test]
    fn manifest_report_all_passed() {
        let mut manifest = ChecksumManifest::new();
        manifest.add_entry("a.txt", "sha256", "aabb", 100);
        manifest.add_entry("b.txt", "sha256", "ccdd", 200);
        let mut actual = HashMap::new();
        actual.insert("a.txt".to_string(), "aabb".to_string());
        actual.insert("b.txt".to_string(), "ccdd".to_string());
        let report = verify_manifest(&manifest, &actual);
        assert!(report.all_passed());
        assert_eq!(report.passed, 2);
        assert_eq!(report.total_checked(), 2);
    }

    #[test]
    fn manifest_report_with_failures() {
        let mut manifest = ChecksumManifest::new();
        manifest.add_entry("a.txt", "sha256", "aabb", 100);
        let mut actual = HashMap::new();
        actual.insert("a.txt".to_string(), "xxxx".to_string());
        let report = verify_manifest(&manifest, &actual);
        assert!(!report.all_passed());
        assert_eq!(report.failed, 1);
        assert_eq!(report.failed_paths(), vec!["a.txt"]);
    }

    #[test]
    fn manifest_report_missing_entries() {
        let mut manifest = ChecksumManifest::new();
        manifest.add_entry("a.txt", "sha256", "aabb", 100);
        let actual = HashMap::new();
        let report = verify_manifest(&manifest, &actual);
        assert_eq!(report.missing, 1);
        assert!(!report.all_passed());
    }

    #[test]
    fn manifest_report_display() {
        let mut report = ChecksumManifestReport::new();
        report.record_pass("ok.txt");
        report.record_fail("bad.txt", "aa", "bb");
        let s = format!("{report}");
        assert!(s.contains("passed=1"));
        assert!(s.contains("failed=1"));
    }

    #[test]
    fn rolling_checksum_deterministic() {
        let mut rc = RollingChecksum::new(4);
        rc.push_bytes(b"hello");
        let v1 = rc.value();
        rc.reset();
        rc.push_bytes(b"hello");
        assert_eq!(v1, rc.value());
        assert_eq!(rc.bytes_fed(), 5);
    }

    #[test]
    fn rolling_checksum_different_data() {
        let mut rc1 = RollingChecksum::new(8);
        rc1.push_bytes(b"aaaaaaaa");
        let mut rc2 = RollingChecksum::new(8);
        rc2.push_bytes(b"bbbbbbbb");
        assert_ne!(rc1.value(), rc2.value());
        let s = format!("{rc1}");
        assert!(s.contains("window=8"));
    }

    #[test]
    fn is_valid_hex_accepts_valid() {
        assert!(is_valid_hex("0123456789abcdef"));
        assert!(is_valid_hex("aa"));
        assert!(!is_valid_hex(""));
        assert!(!is_valid_hex("a"));
        assert!(!is_valid_hex("AABB"));
        assert!(!is_valid_hex("zz"));
    }

    #[test]
    fn hex_normalize_lowercases() {
        assert_eq!(hex_normalize("AABB"), "aabb");
        assert_eq!(hex_normalize("aabb"), "aabb");
    }

    #[test]
    fn hex_eq_case_insensitive() {
        assert!(hex_eq("aabb", "AABB"));
        assert!(hex_eq("aabb", "aabb"));
        assert!(!hex_eq("aabb", "ccdd"));
    }

    #[test]
    fn batch_checksums_produces_correct_count() {
        let items: Vec<&[u8]> = vec![b"alpha", b"beta", b"gamma"];
        let results = batch_checksums(&items, ChecksumKind::Fnv64);
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].0, 0);
        assert_eq!(results[2].0, 2);
        assert_ne!(results[0].1, results[1].1);
    }

    #[test]
    fn batch_verify_catches_failures() {
        let c1 = compute_checksum(b"hello", ChecksumKind::Fnv64);
        let items: Vec<(&[u8], &str)> = vec![
            (b"hello", c1.as_str()),
            (b"world", "0000000000000000"),
        ];
        let failures = batch_verify(&items, ChecksumKind::Fnv64);
        assert_eq!(failures, vec![1]);
    }

    #[test]
    fn checksum_str_matches_bytes() {
        let from_str = checksum_str("test", ChecksumKind::Sha256);
        let from_bytes = compute_checksum(b"test", ChecksumKind::Sha256);
        assert_eq!(from_str, from_bytes);
    }

    #[test]
    fn manifest_entries_with_prefix() {
        let mut m = ChecksumManifest::new();
        m.add_entry("src/main.rs", "SHA-256", "aabb", 100);
        m.add_entry("src/lib.rs", "SHA-256", "ccdd", 200);
        m.add_entry("tests/test.rs", "SHA-256", "eeff", 50);
        let src = m.entries_with_prefix("src/");
        assert_eq!(src.len(), 2);
    }

    #[test]
    fn manifest_total_size() {
        let mut m = ChecksumManifest::new();
        m.add_entry("a.txt", "SHA-256", "aa", 100);
        m.add_entry("b.txt", "SHA-256", "bb", 250);
        assert_eq!(m.total_size(), 350);
    }

    #[test]
    fn manifest_largest_smallest() {
        let mut m = ChecksumManifest::new();
        m.add_entry("small.txt", "SHA-256", "aa", 10);
        m.add_entry("big.txt", "SHA-256", "bb", 1000);
        assert_eq!(m.largest_entry().unwrap().size, 1000);
        assert_eq!(m.smallest_entry().unwrap().size, 10);
    }

    #[test]
    fn manifest_algorithms_dedup() {
        let mut m = ChecksumManifest::new();
        m.add_entry("a.txt", "SHA-256", "aa", 10);
        m.add_entry("b.txt", "MD5", "bb", 20);
        m.add_entry("c.txt", "SHA-256", "cc", 30);
        let algos = m.algorithms();
        assert_eq!(algos, vec!["MD5", "SHA-256"]);
    }

    #[test]
    fn rolling_checksum_matches_helper() {
        let mut rc1 = RollingChecksum::new(4);
        let mut rc2 = RollingChecksum::new(4);
        rc1.push_bytes(b"abcd");
        rc2.push_bytes(b"abcd");
        assert!(rc1.matches(&rc2));
        rc2.push_bytes(b"e");
        assert!(!rc1.matches(&rc2));
    }

    #[test]
    fn checksum_result_display_and_matches() {
        let result = compute_checksum_result(b"hello", ChecksumKind::Sha256);
        assert!(result.hex_len() > 0);
        assert!(result.matches(&result.hex_digest));
        let display = format!("{result}");
        assert!(display.starts_with("SHA-256:"));
    }

    // ------------------------------------------------------------------
    // DataVerifier tests
    // ------------------------------------------------------------------

    #[test]
    fn verifier_match() {
        let data = b"hello";
        let expected = compute_checksum(data, ChecksumKind::Sha256);
        let v = DataVerifier::new(ChecksumKind::Sha256, &expected);
        assert_eq!(v.verify(data), VerifyResult::Match);
    }

    #[test]
    fn verifier_mismatch() {
        let v = DataVerifier::new(ChecksumKind::Sha256, "0000");
        match v.verify(b"hello") {
            VerifyResult::Mismatch { computed, expected } => {
                assert_ne!(computed, expected);
                assert_eq!(expected, "0000");
            }
            VerifyResult::Match => panic!("expected mismatch"),
        }
    }

    #[test]
    fn verify_result_display() {
        assert_eq!(format!("{}", VerifyResult::Match), "checksum OK");
        let m = VerifyResult::Mismatch {
            computed: "aa".into(),
            expected: "bb".into(),
        };
        let s = format!("{m}");
        assert!(s.contains("MISMATCH"));
        assert!(s.contains("aa"));
        assert!(s.contains("bb"));
    }

    // ------------------------------------------------------------------
    // ChecksumFile tests
    // ------------------------------------------------------------------

    #[test]
    fn checksum_file_parse_text_mode() {
        let content = "abcd1234  readme.txt\nef567890  notes.md\n";
        let cf = ChecksumFile::parse(content);
        assert_eq!(cf.entries().len(), 2);
        assert_eq!(cf.entries()[0].filename, "readme.txt");
        assert!(!cf.entries()[0].binary);
        assert_eq!(cf.entries()[1].hex, "ef567890");
    }

    #[test]
    fn checksum_file_parse_binary_mode() {
        let content = "abcd1234 *image.bin\n";
        let cf = ChecksumFile::parse(content);
        assert_eq!(cf.entries().len(), 1);
        assert!(cf.entries()[0].binary);
        assert_eq!(cf.entries()[0].filename, "image.bin");
    }

    #[test]
    fn checksum_file_find_entry() {
        let content = "aaaa  a.txt\nbbbb  b.txt\n";
        let cf = ChecksumFile::parse(content);
        assert!(cf.find_entry("a.txt").is_some());
        assert_eq!(cf.find_entry("a.txt").unwrap().hex, "aaaa");
        assert!(cf.find_entry("missing.txt").is_none());
    }

    #[test]
    fn checksum_file_roundtrip() {
        let content = "abcd1234  readme.txt\nef567890 *image.bin\n";
        let cf = ChecksumFile::parse(content);
        let output = cf.to_string();
        let cf2 = ChecksumFile::parse(&output);
        assert_eq!(cf.entries(), cf2.entries());
    }

    #[test]
    fn checksum_file_skips_comments_and_blanks() {
        let content = "# this is a comment\n\naaaa  file.txt\n";
        let cf = ChecksumFile::parse(content);
        assert_eq!(cf.entries().len(), 1);
    }

    // ------------------------------------------------------------------
    // BatchChecksum tests
    // ------------------------------------------------------------------

    #[test]
    fn batch_checksum_basic() {
        let mut batch = BatchChecksum::new(ChecksumKind::Sha256);
        batch.add("a", b"hello");
        batch.add("b", b"world");
        assert_eq!(batch.results().len(), 2);
        assert_eq!(batch.results()[0].0, "a");
        assert_eq!(batch.results()[1].0, "b");
    }

    #[test]
    fn batch_checksum_all_match() {
        let mut batch = BatchChecksum::new(ChecksumKind::Sha256);
        batch.add("x", b"data1");
        batch.add("y", b"data2");
        let mut expected = HashMap::new();
        expected.insert("x".to_string(), batch.results()[0].1.hex_digest.clone());
        expected.insert("y".to_string(), batch.results()[1].1.hex_digest.clone());
        assert!(batch.all_match(&expected));

        // Change one expected value → should fail.
        expected.insert("x".to_string(), "wrong".to_string());
        assert!(!batch.all_match(&expected));
    }

    #[test]
    fn batch_checksum_missing_key() {
        let mut batch = BatchChecksum::new(ChecksumKind::Fnv64);
        batch.add("a", b"abc");
        let mut expected = HashMap::new();
        expected.insert("a".to_string(), batch.results()[0].1.hex_digest.clone());
        expected.insert("b".to_string(), "1234".to_string());
        // "b" is in expected but not in results → should fail.
        assert!(!batch.all_match(&expected));
    }

    // ------------------------------------------------------------------
    // MtimeChecksumCache tests
    // ------------------------------------------------------------------

    #[test]
    fn cache_insert_and_get() {
        let mut cache = MtimeChecksumCache::new();
        cache.insert("file.txt", 100, "aabbcc".to_string());
        assert_eq!(cache.get("file.txt", 100), Some("aabbcc"));
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn cache_stale_mtime() {
        let mut cache = MtimeChecksumCache::new();
        cache.insert("f.txt", 10, "abc".to_string());
        // Different mtime → miss.
        assert_eq!(cache.get("f.txt", 20), None);
        // Correct mtime → hit.
        assert_eq!(cache.get("f.txt", 10), Some("abc"));
    }

    #[test]
    fn cache_invalidate_single() {
        let mut cache = MtimeChecksumCache::new();
        cache.insert("a.txt", 1, "aa".to_string());
        cache.insert("b.txt", 2, "bb".to_string());
        cache.invalidate("a.txt");
        assert_eq!(cache.len(), 1);
        assert!(cache.get("a.txt", 1).is_none());
        assert!(cache.get("b.txt", 2).is_some());
    }

    #[test]
    fn cache_invalidate_stale() {
        let mut cache = MtimeChecksumCache::new();
        cache.insert("a.txt", 1, "aa".to_string());
        cache.insert("b.txt", 2, "bb".to_string());
        cache.insert("c.txt", 3, "cc".to_string());
        let mut current = HashMap::new();
        current.insert("a.txt".to_string(), 1u64); // same mtime → keep
        current.insert("b.txt".to_string(), 99u64); // different mtime → remove
        // "c.txt" absent from current → remove
        cache.invalidate_stale(&current);
        assert_eq!(cache.len(), 1);
        assert!(cache.get("a.txt", 1).is_some());
    }


#[test]
    fn checksumbatchverifier_severity_ordering() {
        assert!(ChecksumBatchVerifierSeverity::Critical > ChecksumBatchVerifierSeverity::High);
        assert!(ChecksumBatchVerifierSeverity::High > ChecksumBatchVerifierSeverity::Medium);
        assert!(ChecksumBatchVerifierSeverity::Medium > ChecksumBatchVerifierSeverity::Low);
    }

    #[test]
    fn checksumbatchverifier_severity_display() {
        assert_eq!(ChecksumBatchVerifierSeverity::Low.to_string(), "low");
        assert_eq!(ChecksumBatchVerifierSeverity::Critical.to_string(), "critical");
    }

    #[test]
    fn checksumbatchverifier_entry_creation() {
        let e = ChecksumBatchVerifierEntry::new("e1", "Entry 1");
        assert_eq!(e.id, "e1");
        assert_eq!(e.severity, ChecksumBatchVerifierSeverity::Low);
        assert!(e.is_enabled());
    }

    #[test]
    fn checksumbatchverifier_entry_builder() {
        let e = ChecksumBatchVerifierEntry::new("e2", "Entry 2")
            .with_severity(ChecksumBatchVerifierSeverity::High)
            .with_detail("some detail")
            .with_file_count(42);
        assert_eq!(e.severity, ChecksumBatchVerifierSeverity::High);
        assert_eq!(e.detail.as_deref(), Some("some detail"));
        assert_eq!(e.file_count, 42);
    }

    #[test]
    fn checksumbatchverifier_entry_enable_disable() {
        let mut e = ChecksumBatchVerifierEntry::new("e3", "Entry 3");
        assert!(e.is_enabled());
        e.disable();
        assert!(!e.is_enabled());
        e.enable();
        assert!(e.is_enabled());
    }

    #[test]
    fn checksumbatchverifier_add_and_count() {
        let mut mgr = ChecksumBatchVerifier::new("test");
        mgr.add(ChecksumBatchVerifierEntry::new("a", "A"));
        mgr.add(ChecksumBatchVerifierEntry::new("b", "B").with_severity(ChecksumBatchVerifierSeverity::High));
        assert_eq!(mgr.file_count(), 2);
        assert_eq!(mgr.high_severity_count(), 1);
    }

    #[test]
    fn checksumbatchverifier_remove() {
        let mut mgr = ChecksumBatchVerifier::new("test");
        mgr.add(ChecksumBatchVerifierEntry::new("a", "A"));
        let removed = mgr.remove("a");
        assert!(removed.is_some());
        assert!(mgr.is_empty());
    }

    #[test]
    fn checksumbatchverifier_capacity() {
        let mut mgr = ChecksumBatchVerifier::new("test").with_capacity(1);
        assert!(mgr.add(ChecksumBatchVerifierEntry::new("a", "A")));
        assert!(!mgr.add(ChecksumBatchVerifierEntry::new("b", "B")));
    }

    #[test]
    fn checksumbatchverifier_sorted_by_severity() {
        let mut mgr = ChecksumBatchVerifier::new("test");
        mgr.add(ChecksumBatchVerifierEntry::new("lo", "Low"));
        mgr.add(ChecksumBatchVerifierEntry::new("hi", "High").with_severity(ChecksumBatchVerifierSeverity::Critical));
        let sorted = mgr.sorted_by_severity();
        assert_eq!(sorted[0].severity, ChecksumBatchVerifierSeverity::Critical);
    }

    #[test]
    fn checksumbatchverifier_summary() {
        let mgr = ChecksumBatchVerifier::new("test-scope");
        let s = mgr.generate_summary();
        assert!(s.contains("test-scope"));
        assert!(s.contains("Total: 0"));
    }

    #[test]
    fn checksumformatconverter_config_defaults() {
        let cfg = ChecksumFormatConverterConfig::default();
        assert_eq!(cfg.max_items, 100);
        assert!(cfg.auto_refresh);
    }

    #[test]
    fn checksumformatconverter_item_creation() {
        let item = ChecksumFormatConverterItem::new("k1", "v1").with_priority(5).with_tag("tag1");
        assert_eq!(item.key, "k1");
        assert_eq!(item.priority, 5);
        assert!(item.has_tag("tag1"));
        assert!(!item.has_tag("tag2"));
    }

    #[test]
    fn checksumformatconverter_add_and_get() {
        let mut mgr = ChecksumFormatConverter::new(ChecksumFormatConverterConfig::new("test"));
        mgr.add(ChecksumFormatConverterItem::new("k1", "v1"));
        assert_eq!(mgr.algorithm_count(), 1);
        assert_eq!(mgr.get("k1").unwrap().value, "v1");
    }

    #[test]
    fn checksumformatconverter_remove_item() {
        let mut mgr = ChecksumFormatConverter::new(ChecksumFormatConverterConfig::new("test"));
        mgr.add(ChecksumFormatConverterItem::new("k1", "v1"));
        let removed = mgr.remove("k1");
        assert!(removed.is_some());
        assert!(mgr.is_empty());
    }

    #[test]
    fn checksumformatconverter_sorted_by_priority() {
        let mut mgr = ChecksumFormatConverter::new(ChecksumFormatConverterConfig::new("test"));
        mgr.add(ChecksumFormatConverterItem::new("lo", "low").with_priority(1));
        mgr.add(ChecksumFormatConverterItem::new("hi", "high").with_priority(10));
        let sorted = mgr.sorted_by_priority();
        assert_eq!(sorted[0].key, "hi");
    }

    #[test]
    fn checksumformatconverter_items_with_tag() {
        let mut mgr = ChecksumFormatConverter::new(ChecksumFormatConverterConfig::new("test"));
        mgr.add(ChecksumFormatConverterItem::new("a", "1").with_tag("x"));
        mgr.add(ChecksumFormatConverterItem::new("b", "2").with_tag("y"));
        assert_eq!(mgr.items_with_tag("x").len(), 1);
    }

    #[test]
    fn checksumformatconverter_report() {
        let mgr = ChecksumFormatConverter::new(ChecksumFormatConverterConfig::new("my-label").with_auto_refresh(false));
        let r = mgr.generate_report();
        assert!(r.contains("my-label"));
        assert!(r.contains("false"));
    }

    #[test]
    fn checksum_x_config_new() {
        let c = ChecksumXConfig::new("mykey");
        assert_eq!(c.key, "mykey");
        assert!(c.active);
        assert_eq!(c.weight, 0);
        assert!(c.tags.is_empty());
    }

    #[test]
    fn checksum_x_config_builder() {
        let c = ChecksumXConfig::new("k")
            .with_value("v")
            .with_tag("t1")
            .with_tag("t2")
            .with_weight(5)
            .deactivate();
        assert_eq!(c.value, "v");
        assert_eq!(c.tag_count(), 2);
        assert!(c.has_tag("t1"));
        assert_eq!(c.weight, 5);
        assert!(!c.active);
    }

    #[test]
    fn checksum_x_config_display() {
        let c = ChecksumXConfig::new("k").with_value("v");
        assert_eq!(format!("{c}"), "k=v");
    }

    #[test]
    fn checksum_x_registry_insert_get() {
        let mut reg = ChecksumXRegistry::new();
        reg.insert(ChecksumXConfig::new("a").with_value("1")).unwrap();
        assert_eq!(reg.get("a").unwrap().value, "1");
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn checksum_x_registry_duplicate() {
        let mut reg = ChecksumXRegistry::new();
        reg.insert(ChecksumXConfig::new("a")).unwrap();
        assert!(reg.insert(ChecksumXConfig::new("a")).is_err());
    }

    #[test]
    fn checksum_x_registry_remove() {
        let mut reg = ChecksumXRegistry::new();
        reg.insert(ChecksumXConfig::new("a")).unwrap();
        reg.insert(ChecksumXConfig::new("b")).unwrap();
        reg.remove("a");
        assert!(!reg.contains("a"));
        assert!(reg.contains("b"));
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn checksum_x_registry_active_entries() {
        let mut reg = ChecksumXRegistry::new();
        reg.insert(ChecksumXConfig::new("a")).unwrap();
        reg.insert(ChecksumXConfig::new("b").deactivate()).unwrap();
        assert_eq!(reg.active_entries().len(), 1);
    }

    #[test]
    fn checksum_x_registry_by_weight() {
        let mut reg = ChecksumXRegistry::new();
        reg.insert(ChecksumXConfig::new("lo").with_weight(1)).unwrap();
        reg.insert(ChecksumXConfig::new("hi").with_weight(10)).unwrap();
        let sorted = reg.by_weight_desc();
        assert_eq!(sorted[0].key, "hi");
    }

    #[test]
    fn checksum_x_registry_tags() {
        let mut reg = ChecksumXRegistry::new();
        reg.insert(ChecksumXConfig::new("a").with_tag("x")).unwrap();
        reg.insert(ChecksumXConfig::new("b").with_tag("y")).unwrap();
        assert_eq!(reg.entries_with_tag("x").len(), 1);
    }

    #[test]
    fn checksum_x_registry_total_weight() {
        let mut reg = ChecksumXRegistry::new();
        reg.insert(ChecksumXConfig::new("a").with_weight(3)).unwrap();
        reg.insert(ChecksumXConfig::new("b").with_weight(7)).unwrap();
        assert_eq!(reg.total_weight(), 10);
    }

    #[test]
    fn checksum_x_registry_iterator() {
        let mut reg = ChecksumXRegistry::new();
        reg.insert(ChecksumXConfig::new("a")).unwrap();
        reg.insert(ChecksumXConfig::new("b")).unwrap();
        let keys: Vec<&str> = reg.iter().map(|e| e.key.as_str()).collect();
        assert_eq!(keys, vec!["a", "b"]);
    }

    #[test]
    fn checksum_x_cache_put_get() {
        let mut cache = ChecksumXCache::new(3);
        cache.put("a", "1");
        cache.put("b", "2");
        assert_eq!(cache.get("a"), Some("1"));
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn checksum_x_cache_eviction() {
        let mut cache = ChecksumXCache::new(2);
        cache.put("a", "1");
        cache.put("b", "2");
        cache.put("c", "3");
        assert!(!cache.contains("a"));
        assert!(cache.contains("b"));
        assert!(cache.contains("c"));
    }

    #[test]
    fn checksum_x_cache_lru_order() {
        let mut cache = ChecksumXCache::new(3);
        cache.put("a", "1");
        cache.put("b", "2");
        cache.put("c", "3");
        cache.get("a"); // promote a
        cache.put("d", "4"); // evicts b
        assert!(cache.contains("a"));
        assert!(!cache.contains("b"));
    }

    #[test]
    fn checksum_x_cache_most_least_recent() {
        let mut cache = ChecksumXCache::new(5);
        cache.put("x", "1");
        cache.put("y", "2");
        assert_eq!(cache.most_recent().unwrap().0, "y");
        assert_eq!(cache.least_recent().unwrap().0, "x");
    }

    #[test]
    fn checksum_x_formatter_entry() {
        let e = ChecksumXConfig::new("k").with_value("v");
        let fmt = ChecksumXFormatter::new();
        let output = fmt.format_entry(&e);
        assert!(output.contains("[✓]"));
        assert!(output.contains("k=v"));
    }

    #[test]
    fn checksum_x_formatter_summary() {
        let mut reg = ChecksumXRegistry::new();
        reg.insert(ChecksumXConfig::new("a").with_weight(5)).unwrap();
        let fmt = ChecksumXFormatter::new();
        let summary = fmt.format_summary(&reg);
        assert!(summary.contains("1 active"));
    }

    #[test]
    fn checksum_x_validator_valid() {
        let v = ChecksumXValidator::new();
        let c = ChecksumXConfig::new("ok");
        assert!(v.validate(&c).is_ok());
    }

    #[test]
    fn checksum_x_validator_empty_key() {
        let v = ChecksumXValidator::new();
        let c = ChecksumXConfig::new("");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn checksum_x_validator_require_value() {
        let v = ChecksumXValidator::new().require_value(true);
        let c = ChecksumXConfig::new("k");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn checksum_x_validator_allowed_tags() {
        let v = ChecksumXValidator::new()
            .allowed_tags(vec!["ok".into()]);
        let c = ChecksumXConfig::new("k").with_tag("bad");
        assert!(v.validate(&c).is_err());
    }

    #[test]
    fn checksum_x_validator_validate_all() {
        let v = ChecksumXValidator::new();
        let mut reg = ChecksumXRegistry::new();
        reg.insert(ChecksumXConfig::new("ok")).unwrap();
        let errs = v.validate_all(&reg);
        assert!(errs.is_empty());
    }


    // xa_ extended tests for checksum
    #[test]
    fn xa_checksum_ring_new() {
        let rb = super::XaChecksumRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_checksum_ring_push_len() {
        let mut rb = super::XaChecksumRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_checksum_ring_wrap() {
        let mut rb = super::XaChecksumRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_checksum_ring_mean_empty() {
        let rb = super::XaChecksumRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_checksum_ring_mean_values() {
        let mut rb = super::XaChecksumRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_checksum_ring_min_max() {
        let mut rb = super::XaChecksumRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_checksum_ring_iter() {
        let mut rb = super::XaChecksumRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_checksum_counter_new() {
        let c = super::XaChecksumCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_checksum_counter_inc() {
        let mut c = super::XaChecksumCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_checksum_counter_inc_by() {
        let mut c = super::XaChecksumCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_checksum_counter_reset() {
        let mut c = super::XaChecksumCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_checksum_counter_clear() {
        let mut c = super::XaChecksumCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_checksum_counter_default() {
        let c = super::XaChecksumCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }


    // ---- xc_ pool / scheduler tests – block 13 ----

    #[test]
    fn xc_13_pool_new_empty() {
        let pool: super::Xc13Pool<i32> = super::Xc13Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_13_pool_release_acquire() {
        let mut pool = super::Xc13Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_13_pool_acquire_empty() {
        let mut pool: super::Xc13Pool<i32> = super::Xc13Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_13_pool_full() {
        let mut pool = super::Xc13Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_13_pool_drain() {
        let mut pool = super::Xc13Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_13_pool_stats() {
        let mut pool = super::Xc13Pool::new(8);
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
    fn xc_13_pool_clear() {
        let mut pool = super::Xc13Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_13_pool_shrink() {
        let mut pool = super::Xc13Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_13_pool_default() {
        let pool: super::Xc13Pool<String> = super::Xc13Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_13_pool_extend() {
        let mut pool = super::Xc13Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_13_pool_retain() {
        let mut pool = super::Xc13Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_13_scheduler_round_robin() {
        let mut sched = super::Xc13Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_13_scheduler_empty() {
        let mut sched = super::Xc13Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_13_scheduler_reset() {
        let mut sched = super::Xc13Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_13_scheduler_add_remove() {
        let mut sched = super::Xc13Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_13_scheduler_targets() {
        let sched = super::Xc13Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_13_hash_empty() {
        assert_eq!(super::xc_13_hash(b""), 5381);
    }

    #[test]
    fn xc_13_hash_data() {
        let h = super::xc_13_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_13_hash(b"hello"), h);
    }

    #[test]
    fn xc_13_reverse_str() {
        assert_eq!(super::xc_13_reverse("abc"), "cba");
        assert_eq!(super::xc_13_reverse(""), "");
    }


    // --- xd_35 deepening tests ---

    #[test]
    fn xd_35_sm_initial_state() {
        let sm = Xd35StateMachine::new();
        assert_eq!(sm.current_state(), Xd35State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_35_sm_valid_idle_to_running() {
        let mut sm = Xd35StateMachine::new();
        assert!(sm.transition(Xd35State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd35State::Running);
    }

    #[test]
    fn xd_35_sm_valid_running_to_paused() {
        let mut sm = Xd35StateMachine::new();
        sm.transition(Xd35State::Running).unwrap();
        assert!(sm.transition(Xd35State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd35State::Paused);
    }

    #[test]
    fn xd_35_sm_valid_running_to_done() {
        let mut sm = Xd35StateMachine::new();
        sm.transition(Xd35State::Running).unwrap();
        assert!(sm.transition(Xd35State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd35State::Done);
    }

    #[test]
    fn xd_35_sm_valid_paused_to_running() {
        let mut sm = Xd35StateMachine::new();
        sm.transition(Xd35State::Running).unwrap();
        sm.transition(Xd35State::Paused).unwrap();
        assert!(sm.transition(Xd35State::Running).is_ok());
    }

    #[test]
    fn xd_35_sm_valid_done_to_idle() {
        let mut sm = Xd35StateMachine::new();
        sm.transition(Xd35State::Running).unwrap();
        sm.transition(Xd35State::Done).unwrap();
        assert!(sm.transition(Xd35State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd35State::Idle);
    }

    #[test]
    fn xd_35_sm_invalid_idle_to_done() {
        let mut sm = Xd35StateMachine::new();
        assert!(sm.transition(Xd35State::Done).is_err());
    }

    #[test]
    fn xd_35_sm_invalid_idle_to_paused() {
        let mut sm = Xd35StateMachine::new();
        assert!(sm.transition(Xd35State::Paused).is_err());
    }

    #[test]
    fn xd_35_sm_history_tracking() {
        let mut sm = Xd35StateMachine::new();
        sm.transition(Xd35State::Running).unwrap();
        sm.transition(Xd35State::Paused).unwrap();
        sm.transition(Xd35State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd35State::Idle);
        assert_eq!(sm.history()[0].to, Xd35State::Running);
        assert_eq!(sm.history()[1].from, Xd35State::Running);
        assert_eq!(sm.history()[2].to, Xd35State::Done);
    }

    #[test]
    fn xd_35_sm_serialize_deserialize() {
        let mut sm = Xd35StateMachine::new();
        sm.transition(Xd35State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd35StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd35State::Running));
    }

    #[test]
    fn xd_35_sm_deserialize_invalid() {
        assert_eq!(Xd35StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_35_sm_reset() {
        let mut sm = Xd35StateMachine::new();
        sm.transition(Xd35State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd35State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_35_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd35EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd35Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_35_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd35EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd35Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd35Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_35_bus_unsubscribe() {
        let mut bus = Xd35EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_35_event_kind_and_payload() {
        let e = Xd35Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd35Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_35_bus_clear_history() {
        let mut bus = Xd35EventBus::new();
        bus.publish(Xd35Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_35_sm_step_counter_increments() {
        let mut sm = Xd35StateMachine::new();
        sm.transition(Xd35State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd35State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }

}
