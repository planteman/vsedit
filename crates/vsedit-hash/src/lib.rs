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

    /// Get the combined hash value.
    pub fn value(&self) -> u32 {
        self.hash
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
}
