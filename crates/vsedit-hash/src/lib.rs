//! Hashing algorithms and UUID generation.
//!
//! Equivalent to VS Code's `vs/base/common/hash.ts` and UUID utils.

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
}
