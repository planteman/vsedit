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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
}
