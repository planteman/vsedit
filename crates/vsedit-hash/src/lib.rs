//! Hashing algorithms and UUID generation.
//!
//! Equivalent to VS Code's `vs/base/common/hash.ts` and UUID utils.

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

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// A simple hash combiner for combining multiple hash values.
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
}
