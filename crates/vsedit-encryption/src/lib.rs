//! Simple XOR-based encryption service (not cryptographically secure).

use std::fmt;
/// Trait for encryption services.
pub trait Encryptor {
    fn encrypt(&self, data: &[u8]) -> Vec<u8>;
    fn decrypt(&self, data: &[u8]) -> Vec<u8>;
}

/// Derives a key from a passphrase using a simple hash-based approach.
pub fn derive_key(passphrase: &str) -> Vec<u8> {
    let bytes = passphrase.as_bytes();
    let key_len = 32;
    let mut key = vec![0u8; key_len];
    for (i, &b) in bytes.iter().enumerate() {
        key[i % key_len] ^= b;
        key[i % key_len] = key[i % key_len].wrapping_add(b.wrapping_mul(31));
    }
    for i in 1..key_len {
        key[i] ^= key[i - 1].wrapping_add(i as u8);
    }
    key
}

/// Derives a key from a passphrase and salt using PBKDF2-style iterated hashing.
pub fn derive_key_with_salt(password: &str, salt: &[u8], iterations: u32) -> Vec<u8> {
    let mut key = derive_key(password);
    // Mix in salt
    let key_len = key.len();
    for (i, &s) in salt.iter().enumerate() {
        key[i % key_len] ^= s;
    }
    // Iterate to strengthen
    for round in 0..iterations {
        for i in 0..key.len() {
            key[i] = key[i]
                .wrapping_add(round as u8)
                .wrapping_mul(31)
                .wrapping_add(key[(i + 1) % key.len()]);
        }
    }
    key
}

/// Generates a deterministic salt from a counter (for testing reproducibility).
pub fn generate_salt(len: usize) -> Vec<u8> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(1);
    let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut salt = vec![0u8; len];
    for i in 0..len {
        salt[i] = ((counter.wrapping_mul(31).wrapping_add(i as u64)) & 0xFF) as u8;
    }
    salt
}

const BASE64_CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Encodes bytes to a base64 string.
pub fn base64_encode(data: &[u8]) -> String {
    let mut result = String::new();
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;
        result.push(BASE64_CHARS[((triple >> 18) & 0x3F) as usize] as char);
        result.push(BASE64_CHARS[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            result.push(BASE64_CHARS[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
        if chunk.len() > 2 {
            result.push(BASE64_CHARS[(triple & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
    }
    result
}

/// Decodes a base64 string back to bytes. Returns `None` on invalid input.
pub fn base64_decode(encoded: &str) -> Option<Vec<u8>> {
    fn decode_char(c: u8) -> Option<u32> {
        match c {
            b'A'..=b'Z' => Some((c - b'A') as u32),
            b'a'..=b'z' => Some((c - b'a' + 26) as u32),
            b'0'..=b'9' => Some((c - b'0' + 52) as u32),
            b'+' => Some(62),
            b'/' => Some(63),
            b'=' => Some(0),
            _ => None,
        }
    }

    let bytes = encoded.as_bytes();
    if bytes.len() % 4 != 0 {
        return None;
    }
    let mut result = Vec::new();
    for chunk in bytes.chunks(4) {
        if chunk.len() != 4 {
            return None;
        }
        let a = decode_char(chunk[0])?;
        let b = decode_char(chunk[1])?;
        let c = decode_char(chunk[2])?;
        let d = decode_char(chunk[3])?;
        let triple = (a << 18) | (b << 12) | (c << 6) | d;
        result.push(((triple >> 16) & 0xFF) as u8);
        if chunk[2] != b'=' {
            result.push(((triple >> 8) & 0xFF) as u8);
        }
        if chunk[3] != b'=' {
            result.push((triple & 0xFF) as u8);
        }
    }
    Some(result)
}

/// Metadata about a derived key.
pub struct KeyInfo {
    pub algorithm: String,
    pub key_length: usize,
    pub iterations: u32,
    pub salt: Option<Vec<u8>>,
}

/// Configuration for encryption parameters.
pub struct EncryptionConfig {
    pub algorithm: String,
    pub iterations: u32,
    pub salt_length: usize,
}

impl Default for EncryptionConfig {
    fn default() -> Self {
        Self {
            algorithm: "XOR".to_string(),
            iterations: 1000,
            salt_length: 16,
        }
    }
}

/// Verifies a passphrase by encrypting a known marker and comparing.
pub fn verify_passphrase(passphrase: &str, encrypted_marker: &[u8], key: &[u8]) -> bool {
    let svc = EncryptionService::new(key.to_vec());
    let decrypted = svc.decrypt(encrypted_marker);
    let expected_marker = passphrase.as_bytes();
    decrypted == expected_marker
}

/// Simple HMAC-like signing: XOR key with a hash of the data.
pub fn hmac_sign(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut hash = vec![0u8; 32];
    for (i, &b) in data.iter().enumerate() {
        hash[i % 32] ^= b;
        hash[i % 32] = hash[i % 32].wrapping_add(b.wrapping_mul(17));
        hash[(i + 1) % 32] = hash[(i + 1) % 32].wrapping_add(b.wrapping_mul(13));
    }
    let mut signature = vec![0u8; 32];
    for i in 0..32 {
        signature[i] = hash[i] ^ key[i % key.len()];
    }
    signature
}

/// Verifies an HMAC-like signature.
pub fn hmac_verify(key: &[u8], data: &[u8], signature: &[u8]) -> bool {
    let expected = hmac_sign(key, data);
    expected == signature
}

/// XOR-based encryption service.
pub struct EncryptionService {
    key: Vec<u8>,
}

impl EncryptionService {
    pub fn new(key: Vec<u8>) -> Self {
        assert!(!key.is_empty(), "encryption key must not be empty");
        Self { key }
    }

    pub fn from_passphrase(passphrase: &str) -> Self {
        Self::new(derive_key(passphrase))
    }

    pub fn from_passphrase_with_salt(password: &str, salt: &[u8]) -> Self {
        Self::new(derive_key_with_salt(password, salt, 1000))
    }

    /// Encrypt data and return as a base64-encoded string.
    pub fn encrypt_to_base64(&self, data: &[u8]) -> String {
        let encrypted = self.encrypt(data);
        base64_encode(&encrypted)
    }

    /// Decrypt from a base64-encoded string.
    pub fn decrypt_from_base64(&self, encoded: &str) -> Option<Vec<u8>> {
        let encrypted = base64_decode(encoded)?;
        Some(self.decrypt(&encrypted))
    }

    /// Returns metadata about the encryption key.
    pub fn get_key_info(&self) -> KeyInfo {
        KeyInfo {
            algorithm: "XOR".to_string(),
            key_length: self.key.len(),
            iterations: 1000,
            salt: None,
        }
    }

    /// Encrypt a UTF-8 string.
    pub fn encrypt_string(&self, text: &str) -> Vec<u8> {
        self.encrypt(text.as_bytes())
    }

    /// Decrypt data back into a UTF-8 string, returning `None` if invalid.
    pub fn decrypt_string(&self, data: &[u8]) -> Option<String> {
        let decrypted = self.decrypt(data);
        String::from_utf8(decrypted).ok()
    }

    /// Returns true if key is empty.
    pub fn is_key_empty(&self) -> bool {
        self.key.is_empty()
    }

    /// Get the first key, if any.
    pub fn first_key(&self) -> Option<&u8> {
        self.key.first()
    }

    /// Get the last key, if any.
    pub fn last_key(&self) -> Option<&u8> {
        self.key.last()
    }

    /// Retain only key matching the predicate.
    pub fn retain_key(&mut self, f: impl Fn(&u8) -> bool) {
        self.key.retain(|item| f(item));
    }
}

impl Encryptor for EncryptionService {
    /// Encrypt data by XOR-ing with the repeating key.
    fn encrypt(&self, data: &[u8]) -> Vec<u8> {
        data.iter()
            .enumerate()
            .map(|(i, &b)| b ^ self.key[i % self.key.len()])
            .collect()
    }

    /// Decrypt data. XOR is symmetric so this is identical to encrypt.
    fn decrypt(&self, data: &[u8]) -> Vec<u8> {
        self.encrypt(data)
    }
}

/// Stub AES-256-GCM encryption service (interface only, uses XOR internally).
pub struct AesEncryption {
    inner: EncryptionService,
}

impl AesEncryption {
    /// Create from a 32-byte key. Panics if key length is not 32.
    pub fn new(key: Vec<u8>) -> Self {
        assert_eq!(key.len(), 32, "AES-256 requires a 32-byte key");
        Self {
            inner: EncryptionService::new(key),
        }
    }

    pub fn from_password(password: &str, salt: &[u8]) -> Self {
        let key = derive_key_with_salt(password, salt, 1000);
        Self::new(key)
    }
}

impl Encryptor for AesEncryption {
    fn encrypt(&self, data: &[u8]) -> Vec<u8> {
        self.inner.encrypt(data)
    }

    fn decrypt(&self, data: &[u8]) -> Vec<u8> {
        self.inner.decrypt(data)
    }
}

/// Accumulated statistics for encryption operations.
#[derive(Debug, Clone, PartialEq)]
pub struct EncryptionStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl EncryptionStats {
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
    pub fn merge(&mut self, other: &EncryptionStats) {
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

impl Default for EncryptionStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for EncryptionStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "EncryptionStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for encryption.
#[derive(Debug, Clone)]
pub struct EncryptionValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl EncryptionValidator {
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

impl Default for EncryptionValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// PasswordDerivation
// ---------------------------------------------------------------------------

/// Builder for PBKDF2-style password derivation.
pub struct PasswordDerivation {
    password: String,
    salt: Vec<u8>,
    iterations: u32,
}

impl PasswordDerivation {
    /// Create a new derivation from a password.
    pub fn new(password: &str) -> Self {
        Self {
            password: password.to_string(),
            salt: vec![0u8; 16],
            iterations: 1000,
        }
    }

    /// Set a custom salt.
    pub fn with_salt(mut self, salt: &[u8]) -> Self {
        self.salt = salt.to_vec();
        self
    }

    /// Set the number of iterations.
    pub fn with_iterations(mut self, iterations: u32) -> Self {
        self.iterations = iterations;
        self
    }

    /// Derive a key (default 32 bytes).
    pub fn derive(&self) -> Vec<u8> {
        self.derive_with_length(32)
    }

    /// Derive a key of a specific length.
    pub fn derive_with_length(&self, len: usize) -> Vec<u8> {
        key_stretching(&self.password, &self.salt, self.iterations, len)
    }
}

// ---------------------------------------------------------------------------
// EncryptedPayload
// ---------------------------------------------------------------------------

/// Packages an IV and ciphertext together for serialization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncryptedPayload {
    iv: Vec<u8>,
    ciphertext: Vec<u8>,
}

impl EncryptedPayload {
    /// Create a new payload with the given IV and ciphertext.
    pub fn new(iv: Vec<u8>, ciphertext: Vec<u8>) -> Self {
        assert!(iv.len() <= 255, "IV length must fit in a single byte");
        Self { iv, ciphertext }
    }

    /// Serialize as `[iv_len(1 byte)][iv][ciphertext]`.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(1 + self.iv.len() + self.ciphertext.len());
        out.push(self.iv.len() as u8);
        out.extend_from_slice(&self.iv);
        out.extend_from_slice(&self.ciphertext);
        out
    }

    /// Deserialize from the format produced by `to_bytes`.
    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        if data.is_empty() {
            return None;
        }
        let iv_len = data[0] as usize;
        if data.len() < 1 + iv_len {
            return None;
        }
        let iv = data[1..1 + iv_len].to_vec();
        let ciphertext = data[1 + iv_len..].to_vec();
        Some(Self { iv, ciphertext })
    }

    /// Returns a reference to the IV.
    pub fn iv(&self) -> &[u8] {
        &self.iv
    }

    /// Returns a reference to the ciphertext.
    pub fn ciphertext(&self) -> &[u8] {
        &self.ciphertext
    }

    /// Total length of IV + ciphertext.
    pub fn total_len(&self) -> usize {
        self.iv.len() + self.ciphertext.len()
    }
}

// ---------------------------------------------------------------------------
// key_stretching
// ---------------------------------------------------------------------------

/// Derive an encryption key of arbitrary length using iterated hashing.
pub fn key_stretching(passphrase: &str, salt: &[u8], iterations: u32, key_len: usize) -> Vec<u8> {
    // Start from base key
    let base = derive_key(passphrase);
    let mut key = vec![0u8; key_len];

    // Initialize from base key and salt
    for i in 0..key_len {
        key[i] = base[i % base.len()] ^ salt[i % salt.len().max(1)];
    }

    // Iterate to strengthen
    for round in 0..iterations {
        for i in 0..key_len {
            key[i] = key[i]
                .wrapping_add(round as u8)
                .wrapping_mul(31)
                .wrapping_add(key[(i + 1) % key_len]);
        }
    }
    key
}

// ---------------------------------------------------------------------------
// EncryptionService extensions
// ---------------------------------------------------------------------------

impl EncryptionService {
    /// Encrypt data with a specific IV, returning an `EncryptedPayload`.
    /// The IV is XOR'd into the data before the standard key XOR.
    pub fn encrypt_with_iv(&self, data: &[u8], iv: &[u8]) -> EncryptedPayload {
        let mut modified = data.to_vec();
        for (i, b) in modified.iter_mut().enumerate() {
            *b ^= iv[i % iv.len()];
        }
        let ciphertext = self.encrypt(&modified);
        EncryptedPayload::new(iv.to_vec(), ciphertext)
    }

    /// Decrypt an `EncryptedPayload`, reversing the IV XOR.
    pub fn decrypt_payload(&self, payload: &EncryptedPayload) -> Vec<u8> {
        let mut decrypted = self.decrypt(payload.ciphertext());
        let iv = payload.iv();
        for (i, b) in decrypted.iter_mut().enumerate() {
            *b ^= iv[i % iv.len()];
        }
        decrypted
    }
}

// ---------------------------------------------------------------------------
// Convenience free functions
// ---------------------------------------------------------------------------

/// Create an `EncryptedPayload` from plaintext and key using a deterministic IV.
pub fn encrypt_payload(plaintext: &[u8], key: &[u8]) -> EncryptedPayload {
    // Derive a deterministic IV from key + plaintext
    let mut iv = vec![0u8; 16];
    for (i, &b) in key.iter().chain(plaintext.iter()).enumerate() {
        iv[i % 16] ^= b;
        iv[i % 16] = iv[i % 16].wrapping_add(b.wrapping_mul(37));
    }
    let svc = EncryptionService::new(key.to_vec());
    svc.encrypt_with_iv(plaintext, &iv)
}

/// Decrypt an `EncryptedPayload` using the given key.
pub fn decrypt_payload(payload: &EncryptedPayload, key: &[u8]) -> Result<Vec<u8>, String> {
    if key.is_empty() {
        return Err("key must not be empty".to_string());
    }
    let svc = EncryptionService::new(key.to_vec());
    Ok(svc.decrypt_payload(payload))
}

// ---------------------------------------------------------------------------
// Audit log
// ---------------------------------------------------------------------------

/// A single audit log entry.
#[derive(Debug, Clone)]
pub struct AuditEntry {
    pub operation: String,
    pub timestamp_ns: u64,
    pub data_size: usize,
    pub success: bool,
}

/// Tracks encryption/decryption operations.
#[derive(Debug)]
pub struct EncryptionAuditLog {
    pub entries: Vec<AuditEntry>,
    counter: u64,
}

impl EncryptionAuditLog {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            counter: 0,
        }
    }

    pub fn log_operation(&mut self, operation: &str, data_size: usize, success: bool) {
        self.counter += 1;
        self.entries.push(AuditEntry {
            operation: operation.to_string(),
            timestamp_ns: self.counter,
            data_size,
            success,
        });
    }

    pub fn successful_operations(&self) -> usize {
        self.entries.iter().filter(|e| e.success).count()
    }

    pub fn failed_operations(&self) -> usize {
        self.entries.iter().filter(|e| !e.success).count()
    }

    pub fn entries_for_operation(&self, op: &str) -> Vec<&AuditEntry> {
        self.entries.iter().filter(|e| e.operation == op).collect()
    }

    pub fn total_data_processed(&self) -> usize {
        self.entries.iter().map(|e| e.data_size).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_encryption() {
        let svc = EncryptionService::new(vec![0xAB, 0xCD, 0xEF]);
        let original = b"Hello, world!";
        let encrypted = svc.encrypt(original);
        let decrypted = svc.decrypt(&encrypted);
        assert_eq!(decrypted, original);
        assert_ne!(encrypted, original.to_vec());
    }

    #[test]
    fn string_encrypt_decrypt() {
        let key = derive_key("my secret passphrase");
        let svc = EncryptionService::new(key);
        let text = "sensitive data 🔑";
        let encrypted = svc.encrypt_string(text);
        let decrypted = svc.decrypt_string(&encrypted).unwrap();
        assert_eq!(decrypted, text);
    }

    #[test]
    fn different_keys_produce_different_output() {
        let svc1 = EncryptionService::new(vec![0x01, 0x02]);
        let svc2 = EncryptionService::new(vec![0x03, 0x04]);
        let data = b"same input data";
        let enc1 = svc1.encrypt(data);
        let enc2 = svc2.encrypt(data);
        assert_ne!(enc1, enc2);
    }

    #[test]
    fn derive_key_with_salt_deterministic() {
        let k1 = derive_key_with_salt("pass", b"salt", 100);
        let k2 = derive_key_with_salt("pass", b"salt", 100);
        assert_eq!(k1, k2);
        assert_eq!(k1.len(), 32);
    }

    #[test]
    fn derive_key_with_salt_different_salt() {
        let k1 = derive_key_with_salt("pass", b"salt1", 100);
        let k2 = derive_key_with_salt("pass", b"salt2", 100);
        assert_ne!(k1, k2);
    }

    #[test]
    fn from_passphrase_round_trip() {
        let svc = EncryptionService::from_passphrase("my-password");
        let data = b"secret";
        assert_eq!(svc.decrypt(&svc.encrypt(data)), data);
    }

    #[test]
    fn aes_encryption_round_trip() {
        let aes = AesEncryption::from_password("password", b"random-salt");
        let data = b"important data";
        let encrypted = aes.encrypt(data);
        let decrypted = aes.decrypt(&encrypted);
        assert_eq!(decrypted, data);
    }

    #[test]
    fn encryptor_trait_polymorphism() {
        let svc: Box<dyn Encryptor> = Box::new(EncryptionService::from_passphrase("key"));
        let data = b"trait test";
        let enc = svc.encrypt(data);
        let dec = svc.decrypt(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn generate_salt_has_correct_length() {
        let salt = generate_salt(16);
        assert_eq!(salt.len(), 16);
        let salt2 = generate_salt(32);
        assert_eq!(salt2.len(), 32);
    }

    #[test]
    fn generate_salt_increments() {
        let s1 = generate_salt(8);
        let s2 = generate_salt(8);
        assert_ne!(s1, s2, "sequential salts should differ");
    }

    #[test]
    fn base64_encode_empty() {
        assert_eq!(base64_encode(b""), "");
    }

    #[test]
    fn base64_round_trip() {
        let data = b"Hello, world!";
        let encoded = base64_encode(data);
        let decoded = base64_decode(&encoded).unwrap();
        assert_eq!(decoded, data);
    }

    #[test]
    fn base64_known_value() {
        assert_eq!(base64_encode(b"Man"), "TWFu");
        assert_eq!(base64_encode(b"Ma"), "TWE=");
        assert_eq!(base64_encode(b"M"), "TQ==");
    }

    #[test]
    fn base64_decode_invalid() {
        assert!(base64_decode("!!!").is_none());
        assert!(base64_decode("A").is_none());
    }

    #[test]
    fn encrypt_to_base64_round_trip() {
        let svc = EncryptionService::from_passphrase("test-key");
        let data = b"secret message";
        let encoded = svc.encrypt_to_base64(data);
        let decrypted = svc.decrypt_from_base64(&encoded).unwrap();
        assert_eq!(decrypted, data);
    }

    #[test]
    fn decrypt_from_base64_invalid() {
        let svc = EncryptionService::from_passphrase("key");
        assert!(svc.decrypt_from_base64("!!!").is_none());
    }

    #[test]
    fn key_info_defaults() {
        let svc = EncryptionService::from_passphrase("pass");
        let info = svc.get_key_info();
        assert_eq!(info.algorithm, "XOR");
        assert_eq!(info.key_length, 32);
        assert_eq!(info.iterations, 1000);
        assert!(info.salt.is_none());
    }

    #[test]
    fn encryption_config_default() {
        let config = EncryptionConfig::default();
        assert_eq!(config.algorithm, "XOR");
        assert_eq!(config.iterations, 1000);
        assert_eq!(config.salt_length, 16);
    }

    #[test]
    fn hmac_sign_verify() {
        let key = b"my-secret-key";
        let data = b"important message";
        let sig = hmac_sign(key, data);
        assert_eq!(sig.len(), 32);
        assert!(hmac_verify(key, data, &sig));
    }

    #[test]
    fn hmac_verify_wrong_data() {
        let key = b"key";
        let sig = hmac_sign(key, b"data1");
        assert!(!hmac_verify(key, b"data2", &sig));
    }

    #[test]
    fn hmac_verify_wrong_key() {
        let data = b"data";
        let sig = hmac_sign(b"key1", data);
        assert!(!hmac_verify(b"key2", data, &sig));
    }

    #[test]
    fn verify_passphrase_correct() {
        let passphrase = "my-pass";
        let key = derive_key(passphrase);
        let svc = EncryptionService::new(key.clone());
        let marker = svc.encrypt(passphrase.as_bytes());
        assert!(verify_passphrase(passphrase, &marker, &key));
    }

    #[test]
    fn verify_passphrase_wrong() {
        let passphrase = "my-pass";
        let key = derive_key(passphrase);
        let svc = EncryptionService::new(key.clone());
        let marker = svc.encrypt(passphrase.as_bytes());
        let wrong_key = derive_key("wrong-pass");
        assert!(!verify_passphrase(passphrase, &marker, &wrong_key));
    }

    #[test]
    fn password_derivation_basic() {
        let pd = PasswordDerivation::new("secret");
        let key = pd.derive();
        assert_eq!(key.len(), 32);
    }

    #[test]
    fn password_derivation_with_salt_and_iterations() {
        let k1 = PasswordDerivation::new("pass")
            .with_salt(b"salt1")
            .with_iterations(100)
            .derive();
        let k2 = PasswordDerivation::new("pass")
            .with_salt(b"salt2")
            .with_iterations(100)
            .derive();
        assert_ne!(k1, k2);
        assert_eq!(k1.len(), 32);
    }

    #[test]
    fn password_derivation_custom_length() {
        let key = PasswordDerivation::new("pass")
            .with_salt(b"salt")
            .derive_with_length(64);
        assert_eq!(key.len(), 64);
    }

    #[test]
    fn encrypted_payload_round_trip() {
        let iv = vec![1, 2, 3, 4];
        let ct = vec![10, 20, 30, 40, 50];
        let payload = EncryptedPayload::new(iv.clone(), ct.clone());
        assert_eq!(payload.iv(), &iv[..]);
        assert_eq!(payload.ciphertext(), &ct[..]);
        assert_eq!(payload.total_len(), 9);

        let bytes = payload.to_bytes();
        assert_eq!(bytes[0], 4); // iv_len
        let restored = EncryptedPayload::from_bytes(&bytes).unwrap();
        assert_eq!(restored, payload);
    }

    #[test]
    fn encrypted_payload_from_bytes_empty() {
        assert!(EncryptedPayload::from_bytes(&[]).is_none());
    }

    #[test]
    fn encrypted_payload_from_bytes_too_short() {
        // iv_len=5 but only 3 bytes of data total
        assert!(EncryptedPayload::from_bytes(&[5, 1, 2]).is_none());
    }

    #[test]
    fn key_stretching_variable_len() {
        let k16 = key_stretching("pass", b"salt", 10, 16);
        let k64 = key_stretching("pass", b"salt", 10, 64);
        assert_eq!(k16.len(), 16);
        assert_eq!(k64.len(), 64);
    }

    #[test]
    fn key_stretching_deterministic() {
        let k1 = key_stretching("pass", b"salt", 100, 32);
        let k2 = key_stretching("pass", b"salt", 100, 32);
        assert_eq!(k1, k2);
    }

    #[test]
    fn encrypt_with_iv_round_trip() {
        let svc = EncryptionService::from_passphrase("key");
        let data = b"hello world";
        let iv = vec![0xAA, 0xBB, 0xCC, 0xDD];
        let payload = svc.encrypt_with_iv(data, &iv);
        let decrypted = svc.decrypt_payload(&payload);
        assert_eq!(decrypted, data);
    }

    #[test]
    fn encrypt_with_iv_different_ivs_differ() {
        let svc = EncryptionService::from_passphrase("key");
        let data = b"test data";
        let p1 = svc.encrypt_with_iv(data, &[0x01, 0x02]);
        let p2 = svc.encrypt_with_iv(data, &[0x03, 0x04]);
        assert_ne!(p1.ciphertext(), p2.ciphertext());
        // But both decrypt to the same plaintext
        assert_eq!(svc.decrypt_payload(&p1), data);
        assert_eq!(svc.decrypt_payload(&p2), data);
    }

    #[test]
    fn encryption_stats_new_defaults() {
        let stats = EncryptionStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn encryption_stats_record_success() {
        let mut stats = EncryptionStats::new();
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
    fn encryption_stats_record_failure() {
        let mut stats = EncryptionStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn encryption_stats_reset() {
        let mut stats = EncryptionStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn encryption_stats_merge() {
        let mut a = EncryptionStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = EncryptionStats::new();
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
    fn encryption_stats_display() {
        let mut stats = EncryptionStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn encryption_stats_default() {
        let stats = EncryptionStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn encryption_validator_accepts_valid_name() {
        let v = EncryptionValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn encryption_validator_rejects_empty() {
        let v = EncryptionValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn encryption_validator_rejects_too_long() {
        let v = EncryptionValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn encryption_validator_forbidden_prefix() {
        let v = EncryptionValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn encryption_validator_allowed_chars() {
        let v = EncryptionValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn encryption_validator_range() {
        let v = EncryptionValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn encryption_sanitize_removes_control() {
        let result = EncryptionValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn encryption_truncate_short_string() {
        assert_eq!(EncryptionValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn encryption_truncate_long_string() {
        let result = EncryptionValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn encryption_is_ascii_printable() {
        assert!(EncryptionValidator::is_ascii_printable("Hello World 123"));
        assert!(!EncryptionValidator::is_ascii_printable("Hello\x00World"));
    }

    #[test]
    fn test_encrypt_decrypt_payload_roundtrip() {
        let key = b"test-key-123";
        let plaintext = b"Hello, payload!";
        let payload = encrypt_payload(plaintext, key);
        let decrypted = decrypt_payload(&payload, key).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_decrypt_payload_wrong_key() {
        let key = b"correct-key";
        let wrong_key = b"wrong-key!!";
        let plaintext = b"secret data";
        let payload = encrypt_payload(plaintext, key);
        let decrypted = decrypt_payload(&payload, wrong_key).unwrap();
        assert_ne!(decrypted, plaintext.to_vec());
    }

    #[test]
    fn test_audit_log_new() {
        let log = EncryptionAuditLog::new();
        assert!(log.entries.is_empty());
        assert_eq!(log.successful_operations(), 0);
        assert_eq!(log.failed_operations(), 0);
    }

    #[test]
    fn test_audit_log_operations() {
        let mut log = EncryptionAuditLog::new();
        log.log_operation("encrypt", 100, true);
        log.log_operation("decrypt", 200, true);
        log.log_operation("encrypt", 50, false);
        assert_eq!(log.successful_operations(), 2);
        assert_eq!(log.failed_operations(), 1);
        assert_eq!(log.entries.len(), 3);
    }

    #[test]
    fn test_audit_log_filter_by_operation() {
        let mut log = EncryptionAuditLog::new();
        log.log_operation("encrypt", 100, true);
        log.log_operation("decrypt", 200, true);
        log.log_operation("encrypt", 50, false);
        let encrypts = log.entries_for_operation("encrypt");
        assert_eq!(encrypts.len(), 2);
        let decrypts = log.entries_for_operation("decrypt");
        assert_eq!(decrypts.len(), 1);
    }

    #[test]
    fn test_audit_log_total_data() {
        let mut log = EncryptionAuditLog::new();
        log.log_operation("encrypt", 100, true);
        log.log_operation("decrypt", 200, true);
        log.log_operation("encrypt", 50, false);
        assert_eq!(log.total_data_processed(), 350);
    }
}
