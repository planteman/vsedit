//! Simple XOR-based encryption service (not cryptographically secure).

use std::collections::HashMap;
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
#[derive(Debug)]
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

// ---------------------------------------------------------------------------
// EncryptedPayload helpers
// ---------------------------------------------------------------------------

impl fmt::Display for EncryptedPayload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "EncryptedPayload(iv={} bytes, data={} bytes)",
            self.iv.len(),
            self.ciphertext.len()
        )
    }
}

impl Default for EncryptedPayload {
    fn default() -> Self {
        Self {
            iv: Vec::new(),
            ciphertext: Vec::new(),
        }
    }
}

impl EncryptedPayload {
    /// Total size of the payload in bytes.
    pub fn total_size(&self) -> usize {
        self.iv.len() + self.ciphertext.len()
    }

    /// Returns true if the payload appears to be empty.
    pub fn is_empty(&self) -> bool {
        self.ciphertext.is_empty()
    }

    /// Encode the payload as a hex string (iv:data).
    pub fn to_hex_string(&self) -> String {
        let iv_hex: String = self.iv.iter().map(|b| format!("{:02x}", b)).collect();
        let data_hex: String = self.ciphertext.iter().map(|b| format!("{:02x}", b)).collect();
        format!("{}:{}", iv_hex, data_hex)
    }
}

// ---------------------------------------------------------------------------
// Key validation
// ---------------------------------------------------------------------------

/// Validates that a key meets minimum strength requirements.
pub fn validate_key_strength(key: &[u8]) -> Result<(), String> {
    if key.len() < 16 {
        return Err(format!("key too short: {} bytes (minimum 16)", key.len()));
    }
    // Check for all-zero keys
    if key.iter().all(|&b| b == 0) {
        return Err("key is all zeros".to_string());
    }
    // Check for low entropy (all same byte)
    if key.iter().all(|&b| b == key[0]) {
        return Err("key has no entropy (all same byte)".to_string());
    }
    Ok(())
}

/// Validates password complexity.
pub fn validate_password(password: &str) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();
    if password.len() < 8 {
        errors.push("password must be at least 8 characters".to_string());
    }
    if !password.chars().any(|c| c.is_uppercase()) {
        errors.push("password must contain an uppercase letter".to_string());
    }
    if !password.chars().any(|c| c.is_lowercase()) {
        errors.push("password must contain a lowercase letter".to_string());
    }
    if !password.chars().any(|c| c.is_ascii_digit()) {
        errors.push("password must contain a digit".to_string());
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Compute the Shannon entropy of a byte slice.
pub fn byte_entropy(data: &[u8]) -> f64 {
    if data.is_empty() {
        return 0.0;
    }
    let mut counts = [0u32; 256];
    for &b in data {
        counts[b as usize] += 1;
    }
    let len = data.len() as f64;
    counts.iter()
        .filter(|&&c| c > 0)
        .map(|&c| {
            let p = c as f64 / len;
            -p * p.log2()
        })
        .sum()
}

// ---------------------------------------------------------------------------
// AuditEntry helpers
// ---------------------------------------------------------------------------

impl fmt::Display for AuditEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}: size={}", self.timestamp_ns, self.operation, self.data_size)
    }
}

impl Default for EncryptionAuditLog {
    fn default() -> Self {
        Self::new()
    }
}

impl EncryptionAuditLog {
    /// Returns the number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns true if the log is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns entries filtered by operation type.
    pub fn filter_by_operation(&self, op: &str) -> Vec<&AuditEntry> {
        self.entries.iter().filter(|e| e.operation == op).collect()
    }
}

// ---------------------------------------------------------------------------
// EncryptionConfig – builder-pattern configuration
// ---------------------------------------------------------------------------

/// Algorithm selection for the encryption service.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncryptionAlgorithm {
    Xor,
    DoubleXor,
    SubstitutionXor,
}

impl fmt::Display for EncryptionAlgorithm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Xor => write!(f, "XOR"),
            Self::DoubleXor => write!(f, "Double-XOR"),
            Self::SubstitutionXor => write!(f, "Substitution-XOR"),
        }
    }
}

/// Configuration for an encryption service instance with algorithm selection.
#[derive(Debug, Clone)]
pub struct EncryptionServiceConfig {
    pub algorithm: EncryptionAlgorithm,
    pub key_rotation_interval: u64,
    pub max_payload_size: usize,
    pub audit_enabled: bool,
}

impl Default for EncryptionServiceConfig {
    fn default() -> Self {
        Self {
            algorithm: EncryptionAlgorithm::Xor,
            key_rotation_interval: 3600,
            max_payload_size: 10 * 1024 * 1024,
            audit_enabled: true,
        }
    }
}

impl fmt::Display for EncryptionServiceConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "EncryptionServiceConfig(algo={}, rotation={}s, max={}B, audit={})",
            self.algorithm, self.key_rotation_interval, self.max_payload_size, self.audit_enabled
        )
    }
}

/// Builder for [`EncryptionServiceConfig`].
pub struct EncryptionServiceConfigBuilder {
    config: EncryptionServiceConfig,
}

impl EncryptionServiceConfigBuilder {
    pub fn new() -> Self {
        Self { config: EncryptionServiceConfig::default() }
    }

    pub fn algorithm(mut self, algo: EncryptionAlgorithm) -> Self {
        self.config.algorithm = algo;
        self
    }

    pub fn key_rotation_interval(mut self, secs: u64) -> Self {
        self.config.key_rotation_interval = secs;
        self
    }

    pub fn max_payload_size(mut self, bytes: usize) -> Self {
        self.config.max_payload_size = bytes;
        self
    }

    pub fn audit_enabled(mut self, enabled: bool) -> Self {
        self.config.audit_enabled = enabled;
        self
    }

    pub fn build(self) -> EncryptionServiceConfig {
        self.config
    }
}

// ---------------------------------------------------------------------------
// KeyRing – manages multiple named keys
// ---------------------------------------------------------------------------

/// A named encryption key.
#[derive(Debug, Clone)]
pub struct NamedKey {
    pub name: String,
    pub key: Vec<u8>,
    pub created_epoch: u64,
}

/// Manages multiple named encryption keys.
#[derive(Debug, Clone)]
pub struct KeyRing {
    keys: Vec<NamedKey>,
    active: Option<String>,
}

impl KeyRing {
    pub fn new() -> Self {
        Self { keys: Vec::new(), active: None }
    }

    pub fn add(&mut self, name: impl Into<String>, key: Vec<u8>) {
        let name = name.into();
        if !self.keys.iter().any(|k| k.name == name) {
            if self.keys.is_empty() {
                self.active = Some(name.clone());
            }
            self.keys.push(NamedKey { name, key, created_epoch: 0 });
        }
    }

    pub fn remove(&mut self, name: &str) -> bool {
        let len = self.keys.len();
        self.keys.retain(|k| k.name != name);
        if self.active.as_deref() == Some(name) {
            self.active = self.keys.first().map(|k| k.name.clone());
        }
        self.keys.len() < len
    }

    pub fn get(&self, name: &str) -> Option<&[u8]> {
        self.keys.iter().find(|k| k.name == name).map(|k| k.key.as_slice())
    }

    pub fn active_key(&self) -> Option<&[u8]> {
        self.active.as_ref().and_then(|name| self.get(name))
    }

    pub fn set_active(&mut self, name: &str) -> bool {
        if self.keys.iter().any(|k| k.name == name) {
            self.active = Some(name.to_string());
            true
        } else {
            false
        }
    }

    pub fn list_names(&self) -> Vec<&str> {
        self.keys.iter().map(|k| k.name.as_str()).collect()
    }

    pub fn len(&self) -> usize {
        self.keys.len()
    }

    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    pub fn rotate(&mut self, name: &str, new_key: Vec<u8>) -> bool {
        if let Some(k) = self.keys.iter_mut().find(|k| k.name == name) {
            k.key = new_key;
            true
        } else {
            false
        }
    }
}

impl fmt::Display for KeyRing {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "KeyRing({} keys, active={:?})",
            self.keys.len(),
            self.active
        )
    }
}

// ---------------------------------------------------------------------------
// EncryptionThroughputStats
// ---------------------------------------------------------------------------

/// Tracks encryption/decryption throughput statistics.
#[derive(Debug, Clone, Default)]
pub struct EncryptionThroughputStats {
    pub encrypt_count: u64,
    pub decrypt_count: u64,
    pub bytes_encrypted: u64,
    pub bytes_decrypted: u64,
}

impl EncryptionThroughputStats {
    pub fn record_encrypt(&mut self, bytes: u64) {
        self.encrypt_count += 1;
        self.bytes_encrypted += bytes;
    }

    pub fn record_decrypt(&mut self, bytes: u64) {
        self.decrypt_count += 1;
        self.bytes_decrypted += bytes;
    }

    pub fn total_operations(&self) -> u64 {
        self.encrypt_count + self.decrypt_count
    }

    pub fn total_bytes(&self) -> u64 {
        self.bytes_encrypted + self.bytes_decrypted
    }

    /// Reset all counters to zero.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Merge another stats instance into this one.
    pub fn merge(&mut self, other: &EncryptionThroughputStats) {
        self.encrypt_count += other.encrypt_count;
        self.decrypt_count += other.decrypt_count;
        self.bytes_encrypted += other.bytes_encrypted;
        self.bytes_decrypted += other.bytes_decrypted;
    }

    /// Average bytes per operation, or 0 if none recorded.
    pub fn avg_bytes_per_op(&self) -> u64 {
        let total_ops = self.total_operations();
        if total_ops == 0 {
            0
        } else {
            self.total_bytes() / total_ops
        }
    }
}

impl fmt::Display for EncryptionThroughputStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "EncryptionThroughputStats(enc={}/{} bytes, dec={}/{} bytes)",
            self.encrypt_count, self.bytes_encrypted,
            self.decrypt_count, self.bytes_decrypted
        )
    }
}

// ---------------------------------------------------------------------------
// KeyRing – re-encryption helpers
// ---------------------------------------------------------------------------

impl KeyRing {
    /// Re-encrypt data from one named key to another.
    /// Returns `Err` if either key is missing.
    pub fn re_encrypt(&self, data: &[u8], from: &str, to: &str) -> Result<Vec<u8>, String> {
        let from_key = self
            .get(from)
            .ok_or_else(|| format!("source key '{}' not found", from))?;
        let to_key = self
            .get(to)
            .ok_or_else(|| format!("target key '{}' not found", to))?;
        let from_svc = EncryptionService::new(from_key.to_vec());
        let to_svc = EncryptionService::new(to_key.to_vec());
        let plaintext = from_svc.decrypt(data);
        Ok(to_svc.encrypt(&plaintext))
    }

    /// Encrypt data using the currently active key.
    /// Returns `Err` if no active key is set.
    pub fn encrypt_with_active(&self, data: &[u8]) -> Result<Vec<u8>, String> {
        let key = self
            .active_key()
            .ok_or_else(|| "no active key set".to_string())?;
        let svc = EncryptionService::new(key.to_vec());
        Ok(svc.encrypt(data))
    }

    /// Decrypt data using the currently active key.
    /// Returns `Err` if no active key is set.
    pub fn decrypt_with_active(&self, data: &[u8]) -> Result<Vec<u8>, String> {
        let key = self
            .active_key()
            .ok_or_else(|| "no active key set".to_string())?;
        let svc = EncryptionService::new(key.to_vec());
        Ok(svc.decrypt(data))
    }

    /// Returns the names of all keys sorted alphabetically.
    pub fn sorted_names(&self) -> Vec<&str> {
        let mut names = self.list_names();
        names.sort();
        names
    }

    /// Returns `true` if a key with the given name exists.
    pub fn contains(&self, name: &str) -> bool {
        self.keys.iter().any(|k| k.name == name)
    }
}

// ---------------------------------------------------------------------------
// SecretStore – in-memory encrypted key-value store
// ---------------------------------------------------------------------------

/// An in-memory key-value store where values are encrypted at rest.
#[derive(Debug)]
pub struct SecretStore {
    entries: Vec<(String, Vec<u8>)>,
    service: EncryptionService,
}

impl SecretStore {
    /// Create a new store backed by the given encryption key.
    pub fn new(key: Vec<u8>) -> Self {
        Self {
            entries: Vec::new(),
            service: EncryptionService::new(key),
        }
    }

    /// Create a store from a passphrase.
    pub fn from_passphrase(passphrase: &str) -> Self {
        Self::new(derive_key(passphrase))
    }

    /// Store a secret under `name`. Overwrites if name already exists.
    pub fn set(&mut self, name: impl Into<String>, value: &[u8]) {
        let name = name.into();
        let encrypted = self.service.encrypt(value);
        if let Some(entry) = self.entries.iter_mut().find(|(n, _)| *n == name) {
            entry.1 = encrypted;
        } else {
            self.entries.push((name, encrypted));
        }
    }

    /// Retrieve and decrypt a secret by name. Returns `None` if not found.
    pub fn get(&self, name: &str) -> Option<Vec<u8>> {
        self.entries
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, enc)| self.service.decrypt(enc))
    }

    /// Retrieve a secret as a UTF-8 string.
    pub fn get_string(&self, name: &str) -> Option<String> {
        self.get(name)
            .and_then(|bytes| String::from_utf8(bytes).ok())
    }

    /// Remove a secret by name. Returns `true` if it existed.
    pub fn remove(&mut self, name: &str) -> bool {
        let len = self.entries.len();
        self.entries.retain(|(n, _)| n != name);
        self.entries.len() < len
    }

    /// Returns the number of stored secrets.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` if no secrets are stored.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns the names of all stored secrets.
    pub fn names(&self) -> Vec<&str> {
        self.entries.iter().map(|(n, _)| n.as_str()).collect()
    }

    /// Returns `true` if a secret with the given name exists.
    pub fn contains(&self, name: &str) -> bool {
        self.entries.iter().any(|(n, _)| n == name)
    }

    /// Clear all stored secrets.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

impl fmt::Display for SecretStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SecretStore({} secrets)", self.entries.len())
    }
}

// ---------------------------------------------------------------------------
// EncryptionPipeline – chain multiple encryption passes
// ---------------------------------------------------------------------------

/// Chains multiple encryption keys for layered encryption.
#[derive(Debug)]
pub struct EncryptionPipeline {
    layers: Vec<EncryptionService>,
}

impl EncryptionPipeline {
    /// Create an empty pipeline.
    pub fn new() -> Self {
        Self { layers: Vec::new() }
    }

    /// Add an encryption layer with the given key.
    pub fn add_layer(mut self, key: Vec<u8>) -> Self {
        self.layers.push(EncryptionService::new(key));
        self
    }

    /// Add a layer from a passphrase.
    pub fn add_passphrase_layer(self, passphrase: &str) -> Self {
        self.add_layer(derive_key(passphrase))
    }

    /// Encrypt data through all layers in order.
    pub fn encrypt(&self, data: &[u8]) -> Vec<u8> {
        let mut result = data.to_vec();
        for layer in &self.layers {
            result = layer.encrypt(&result);
        }
        result
    }

    /// Decrypt data through all layers in reverse order.
    pub fn decrypt(&self, data: &[u8]) -> Vec<u8> {
        let mut result = data.to_vec();
        for layer in self.layers.iter().rev() {
            result = layer.decrypt(&result);
        }
        result
    }

    /// Returns the number of encryption layers.
    pub fn depth(&self) -> usize {
        self.layers.len()
    }
}

impl Default for EncryptionPipeline {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for EncryptionPipeline {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "EncryptionPipeline({} layers)", self.layers.len())
    }
}

// ---------------------------------------------------------------------------
// Hex encoding utilities
// ---------------------------------------------------------------------------

/// Encode bytes as a lowercase hex string.
pub fn hex_encode(data: &[u8]) -> String {
    data.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Decode a hex string back to bytes. Returns `None` on invalid input.
pub fn hex_decode(hex: &str) -> Option<Vec<u8>> {
    if hex.len() % 2 != 0 {
        return None;
    }
    let mut result = Vec::with_capacity(hex.len() / 2);
    let bytes = hex.as_bytes();
    for pair in bytes.chunks(2) {
        let hi = hex_nibble(pair[0])?;
        let lo = hex_nibble(pair[1])?;
        result.push((hi << 4) | lo);
    }
    Some(result)
}

fn hex_nibble(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// EncryptionService – additional helpers
// ---------------------------------------------------------------------------

impl EncryptionService {
    /// Sign data with the service's key using the HMAC-like scheme.
    pub fn sign(&self, data: &[u8]) -> Vec<u8> {
        hmac_sign(&self.key, data)
    }

    /// Verify a signature against data using the service's key.
    pub fn verify(&self, data: &[u8], signature: &[u8]) -> bool {
        hmac_verify(&self.key, data, signature)
    }

    /// Encrypt and sign: returns `(ciphertext, signature)`.
    pub fn encrypt_and_sign(&self, data: &[u8]) -> (Vec<u8>, Vec<u8>) {
        let ciphertext = self.encrypt(data);
        let signature = self.sign(&ciphertext);
        (ciphertext, signature)
    }

    /// Verify signature, then decrypt. Returns `None` if signature is invalid.
    pub fn verify_and_decrypt(&self, ciphertext: &[u8], signature: &[u8]) -> Option<Vec<u8>> {
        if !self.verify(ciphertext, signature) {
            return None;
        }
        Some(self.decrypt(ciphertext))
    }

    /// Encrypt data and return as a hex-encoded string.
    pub fn encrypt_to_hex(&self, data: &[u8]) -> String {
        hex_encode(&self.encrypt(data))
    }

    /// Decrypt from a hex-encoded string. Returns `None` on invalid hex.
    pub fn decrypt_from_hex(&self, hex: &str) -> Option<Vec<u8>> {
        let encrypted = hex_decode(hex)?;
        Some(self.decrypt(&encrypted))
    }

    /// Returns the key length in bytes.
    pub fn key_len(&self) -> usize {
        self.key.len()
    }
}


// ---------------------------------------------------------------------------
// KeyRotationManager
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct KeyRotationManager {
    entries: Vec<String>,
    index: usize,
    enabled: bool,
    config: HashMap<String, String>,
    stats_hits: u64,
    stats_misses: u64,
}

impl KeyRotationManager {
    pub fn new() -> Self { Self::default() }
    pub fn add_entry(&mut self, entry: impl Into<String>) { self.entries.push(entry.into()); }
    pub fn remove_entry(&mut self, idx: usize) -> Option<String> { if idx < self.entries.len() { Some(self.entries.remove(idx)) } else { None } }
    pub fn get_entry(&self, idx: usize) -> Option<&str> { self.entries.get(idx).map(|s| s.as_str()) }
    pub fn entry_count(&self) -> usize { self.entries.len() }
    pub fn set_enabled(&mut self, e: bool) { self.enabled = e; }
    pub fn is_enabled(&self) -> bool { self.enabled }
    pub fn set_config(&mut self, k: impl Into<String>, v: impl Into<String>) { self.config.insert(k.into(), v.into()); }
    pub fn get_config(&self, k: &str) -> Option<&str> { self.config.get(k).map(|s| s.as_str()) }
    pub fn config_count(&self) -> usize { self.config.len() }
    pub fn record_hit(&mut self) { self.stats_hits += 1; }
    pub fn record_miss(&mut self) { self.stats_misses += 1; }
    pub fn hit_rate(&self) -> f64 { let t = self.stats_hits + self.stats_misses; if t == 0 { 0.0 } else { self.stats_hits as f64 / t as f64 } }
    pub fn reset_stats(&mut self) { self.stats_hits = 0; self.stats_misses = 0; }
    pub fn select_next(&mut self) { if !self.entries.is_empty() { self.index = (self.index + 1) % self.entries.len(); } }
    pub fn select_prev(&mut self) { if !self.entries.is_empty() { self.index = if self.index == 0 { self.entries.len() - 1 } else { self.index - 1 }; } }
    pub fn current_index(&self) -> usize { self.index }
    pub fn current_entry(&self) -> Option<&str> { self.entries.get(self.index).map(|s| s.as_str()) }
    pub fn clear(&mut self) { self.entries.clear(); self.index = 0; }
    pub fn contains(&self, s: &str) -> bool { self.entries.iter().any(|e| e == s) }
    pub fn entries(&self) -> &[String] { &self.entries }
    pub fn filter_entries(&self, query: &str) -> Vec<&str> { self.entries.iter().filter(|e| e.contains(query)).map(|s| s.as_str()).collect() }
}

impl Default for KeyRotationManager {
    fn default() -> Self { Self { entries: Vec::new(), index: 0, enabled: true, config: HashMap::new(), stats_hits: 0, stats_misses: 0 } }
}

impl fmt::Display for KeyRotationManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "KeyRotationManager({} entries, enabled={})", self.entries.len(), self.enabled) }
}

// ---------------------------------------------------------------------------
// EncryptedStorageAdapter
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct EncryptedStorageAdapter {
    items: HashMap<String, Vec<String>>,
    active: Option<String>,
    max_items: usize,
    total_ops: u64,
    last_error: Option<String>,
}

impl EncryptedStorageAdapter {
    pub fn new() -> Self { Self::default() }
    pub fn with_max(mut self, m: usize) -> Self { self.max_items = m; self }
    pub fn add_item(&mut self, group: impl Into<String>, value: impl Into<String>) {
        let g = group.into();
        let entry = self.items.entry(g).or_default();
        if entry.len() < self.max_items { entry.push(value.into()); }
        self.total_ops += 1;
    }
    pub fn remove_group(&mut self, group: &str) -> bool { self.items.remove(group).is_some() }
    pub fn get_group(&self, group: &str) -> Option<&Vec<String>> { self.items.get(group) }
    pub fn group_count(&self) -> usize { self.items.len() }
    pub fn total_items(&self) -> usize { self.items.values().map(|v| v.len()).sum() }
    pub fn set_active(&mut self, a: impl Into<String>) { self.active = Some(a.into()); }
    pub fn active(&self) -> Option<&str> { self.active.as_deref() }
    pub fn clear_active(&mut self) { self.active = None; }
    pub fn set_error(&mut self, e: impl Into<String>) { self.last_error = Some(e.into()); }
    pub fn last_error(&self) -> Option<&str> { self.last_error.as_deref() }
    pub fn clear_error(&mut self) { self.last_error = None; }
    pub fn total_ops(&self) -> u64 { self.total_ops }
    pub fn clear(&mut self) { self.items.clear(); self.active = None; self.total_ops = 0; self.last_error = None; }
    pub fn groups(&self) -> Vec<&str> { self.items.keys().map(|k| k.as_str()).collect() }
    pub fn contains_group(&self, g: &str) -> bool { self.items.contains_key(g) }
    pub fn is_empty(&self) -> bool { self.items.is_empty() }
}

impl Default for EncryptedStorageAdapter {
    fn default() -> Self { Self { items: HashMap::new(), active: None, max_items: 1000, total_ops: 0, last_error: None } }
}

impl fmt::Display for EncryptedStorageAdapter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "EncryptedStorageAdapter({} groups, {} items)", self.group_count(), self.total_items()) }
}


// ---------------------------------------------------------------------------
// KeyRotationManagerSnapshot — point-in-time snapshot of KeyRotationManager state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct KeyRotationManagerSnapshot {
    pub timestamp: u64,
    pub entry_count: usize,
    pub enabled: bool,
    pub config_snapshot: Vec<(String, String)>,
    pub hit_rate: f64,
}

impl KeyRotationManagerSnapshot {
    pub fn capture(source: &KeyRotationManager, timestamp: u64) -> Self {
        Self {
            timestamp,
            entry_count: source.entry_count(),
            enabled: source.is_enabled(),
            config_snapshot: Vec::new(),
            hit_rate: source.hit_rate(),
        }
    }

    pub fn age_since(&self, now: u64) -> u64 {
        now.saturating_sub(self.timestamp)
    }

    pub fn is_stale(&self, now: u64, max_age: u64) -> bool {
        self.age_since(now) > max_age
    }

    pub fn diff_entry_count(&self, other: &Self) -> i64 {
        self.entry_count as i64 - other.entry_count as i64
    }
}

impl fmt::Display for KeyRotationManagerSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Snapshot(t={}, entries={}, enabled={})", self.timestamp, self.entry_count, self.enabled)
    }
}

// ---------------------------------------------------------------------------
// EncryptedStorageAdapterStats — aggregate statistics for EncryptedStorageAdapter
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct EncryptedStorageAdapterStats {
    pub total_adds: u64,
    pub total_removes: u64,
    pub total_lookups: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub peak_group_count: usize,
    pub peak_item_count: usize,
}

impl EncryptedStorageAdapterStats {
    pub fn new() -> Self { Self::default() }

    pub fn record_add(&mut self) { self.total_adds += 1; }
    pub fn record_remove(&mut self) { self.total_removes += 1; }
    pub fn record_lookup(&mut self, hit: bool) {
        self.total_lookups += 1;
        if hit { self.cache_hits += 1; } else { self.cache_misses += 1; }
    }

    pub fn update_peaks(&mut self, groups: usize, items: usize) {
        if groups > self.peak_group_count { self.peak_group_count = groups; }
        if items > self.peak_item_count { self.peak_item_count = items; }
    }

    pub fn hit_ratio(&self) -> f64 {
        if self.total_lookups == 0 { 0.0 } else { self.cache_hits as f64 / self.total_lookups as f64 }
    }

    pub fn net_changes(&self) -> i64 {
        self.total_adds as i64 - self.total_removes as i64
    }

    pub fn reset(&mut self) { *self = Self::default(); }

    pub fn merge(&mut self, other: &Self) {
        self.total_adds += other.total_adds;
        self.total_removes += other.total_removes;
        self.total_lookups += other.total_lookups;
        self.cache_hits += other.cache_hits;
        self.cache_misses += other.cache_misses;
        if other.peak_group_count > self.peak_group_count { self.peak_group_count = other.peak_group_count; }
        if other.peak_item_count > self.peak_item_count { self.peak_item_count = other.peak_item_count; }
    }
}

impl fmt::Display for EncryptedStorageAdapterStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Stats(adds={}, removes={}, hit_ratio={:.1}%)", self.total_adds, self.total_removes, self.hit_ratio() * 100.0)
    }
}

// ---------------------------------------------------------------------------
// KeyRotationManagerConfig — configuration for KeyRotationManager
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct KeyRotationManagerConfig {
    pub max_entries: usize,
    pub auto_cleanup: bool,
    pub cleanup_threshold: usize,
    pub debounce_ms: u64,
    pub labels: HashMap<String, String>,
}

impl KeyRotationManagerConfig {
    pub fn new() -> Self { Self::default() }
    pub fn with_max_entries(mut self, m: usize) -> Self { self.max_entries = m; self }
    pub fn with_auto_cleanup(mut self, a: bool) -> Self { self.auto_cleanup = a; self }
    pub fn with_debounce(mut self, ms: u64) -> Self { self.debounce_ms = ms; self }
    pub fn set_label(&mut self, key: impl Into<String>, val: impl Into<String>) { self.labels.insert(key.into(), val.into()); }
    pub fn get_label(&self, key: &str) -> Option<&str> { self.labels.get(key).map(|s| s.as_str()) }
    pub fn label_count(&self) -> usize { self.labels.len() }
    pub fn needs_cleanup(&self, current: usize) -> bool { self.auto_cleanup && current > self.cleanup_threshold }
}

impl Default for KeyRotationManagerConfig {
    fn default() -> Self {
        Self { max_entries: 10000, auto_cleanup: true, cleanup_threshold: 8000, debounce_ms: 100, labels: HashMap::new() }
    }
}

impl fmt::Display for KeyRotationManagerConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Config(max={}, auto_cleanup={}, debounce={}ms)", self.max_entries, self.auto_cleanup, self.debounce_ms)
    }
}

// ── KeyDerivationConfig ──────────────────────────────────────────────────

/// Builder-pattern configuration for key derivation parameters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyDerivationConfig {
    pub iterations: u32,
    pub salt_len: usize,
    pub key_len: usize,
}

impl KeyDerivationConfig {
    pub fn new() -> Self {
        Self { iterations: 100_000, salt_len: 16, key_len: 32 }
    }

    pub fn with_iterations(mut self, iterations: u32) -> Self { self.iterations = iterations; self }
    pub fn with_salt_len(mut self, salt_len: usize) -> Self { self.salt_len = salt_len; self }
    pub fn with_key_len(mut self, key_len: usize) -> Self { self.key_len = key_len; self }

    pub fn is_strong(&self) -> bool {
        self.iterations >= 100_000 && self.key_len >= 32 && self.salt_len >= 16
    }

    pub fn estimated_time_ms(&self) -> f64 {
        self.iterations as f64 * 0.001
    }
}

impl Default for KeyDerivationConfig {
    fn default() -> Self { Self::new() }
}

impl fmt::Display for KeyDerivationConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "KDF(iter={}, salt={}B, key={}B)", self.iterations, self.salt_len, self.key_len)
    }
}

// ── SecureStringMasker ──────────────────────────────────────────────────

/// Masks sensitive strings for display/logging.
pub struct SecureStringMasker;

impl SecureStringMasker {
    /// Mask all but the last `n` characters.
    pub fn mask_except_last_n(s: &str, n: usize, mask_char: char) -> String {
        let chars: Vec<char> = s.chars().collect();
        if chars.len() <= n { return s.to_string(); }
        let masked_len = chars.len() - n;
        let masked: String = std::iter::repeat(mask_char).take(masked_len).collect();
        let visible: String = chars[masked_len..].iter().collect();
        format!("{}{}", masked, visible)
    }

    /// Mask the middle portion, keeping first and last `keep` characters.
    pub fn mask_middle(s: &str, keep: usize, mask_char: char) -> String {
        let chars: Vec<char> = s.chars().collect();
        if chars.len() <= keep * 2 { return s.to_string(); }
        let prefix: String = chars[..keep].iter().collect();
        let suffix: String = chars[chars.len() - keep..].iter().collect();
        let middle_len = chars.len() - keep * 2;
        let middle: String = std::iter::repeat(mask_char).take(middle_len).collect();
        format!("{}{}{}", prefix, middle, suffix)
    }

    /// Redact an email address: show first char + domain.
    pub fn redact_email(email: &str) -> String {
        if let Some(at_pos) = email.find('@') {
            if at_pos > 0 {
                let first_char = &email[..1];
                let domain = &email[at_pos..];
                let stars = "*".repeat(at_pos.saturating_sub(1));
                return format!("{}{}{}", first_char, stars, domain);
            }
        }
        "***".to_string()
    }

    /// Redact the entire string with a given character.
    pub fn redact_with_char(s: &str, mask_char: char) -> String {
        std::iter::repeat(mask_char).take(s.chars().count()).collect()
    }
}

// ── CryptoAuditLog ──────────────────────────────────────────────────

/// Records encryption/decryption operations for audit purposes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CryptoAuditKind { Encrypt, Decrypt }

#[derive(Debug, Clone)]
pub struct CryptoAuditRecord {
    pub kind: CryptoAuditKind,
    pub key_id: String,
    pub timestamp: u64,
}

#[derive(Debug, Clone)]
pub struct CryptoAuditLog {
    entries: Vec<CryptoAuditRecord>,
}

impl CryptoAuditLog {
    pub fn new() -> Self { Self { entries: Vec::new() } }

    pub fn record(&mut self, kind: CryptoAuditKind, key_id: &str, timestamp: u64) {
        self.entries.push(CryptoAuditRecord { kind, key_id: key_id.to_string(), timestamp });
    }

    pub fn query_by_time_range(&self, start: u64, end: u64) -> Vec<&CryptoAuditRecord> {
        self.entries.iter().filter(|e| e.timestamp >= start && e.timestamp <= end).collect()
    }

    pub fn operation_count(&self, kind: &CryptoAuditKind) -> usize {
        self.entries.iter().filter(|e| &e.kind == kind).count()
    }

    pub fn last_operation(&self) -> Option<&CryptoAuditRecord> { self.entries.last() }

    pub fn total_entries(&self) -> usize { self.entries.len() }

    pub fn entries_for_key(&self, key_id: &str) -> Vec<&CryptoAuditRecord> {
        self.entries.iter().filter(|e| e.key_id == key_id).collect()
    }
}

/// Cipher suite registry for encryption providers.
#[derive(Debug, Clone)]
pub struct CipherSuiteRegistry {
    entries: Vec<CipherSuiteEntry>,
    enabled: bool,
    max_entries: usize,
}

/// A single cipher suite entry.
#[derive(Debug, Clone, PartialEq)]
pub struct CipherSuiteEntry {
    pub id: String,
    pub label: String,
    pub priority: i32,
    pub active: bool,
    pub metadata: Vec<(String, String)>,
}

impl CipherSuiteEntry {
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

    pub fn deactivate(&mut self) { self.active = false; }
    pub fn activate(&mut self) { self.active = true; }

    pub fn has_meta(&self, key: &str) -> bool {
        self.metadata.iter().any(|(k, _)| k == key)
    }

    pub fn meta_count(&self) -> usize { self.metadata.len() }

    pub fn remove_meta(&mut self, key: &str) -> bool {
        let len = self.metadata.len();
        self.metadata.retain(|(k, _)| k != key);
        self.metadata.len() < len
    }
}

impl CipherSuiteRegistry {
    pub fn new(max_entries: usize) -> Self {
        Self { entries: Vec::new(), enabled: true, max_entries }
    }

    pub fn add(&mut self, entry: CipherSuiteEntry) -> bool {
        if self.entries.len() >= self.max_entries { return false; }
        self.entries.push(entry);
        self.entries.sort_by(|a, b| b.priority.cmp(&a.priority));
        true
    }

    pub fn remove(&mut self, id: &str) -> bool {
        let len = self.entries.len();
        self.entries.retain(|e| e.id != id);
        self.entries.len() < len
    }

    pub fn get(&self, id: &str) -> Option<&CipherSuiteEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut CipherSuiteEntry> {
        self.entries.iter_mut().find(|e| e.id == id)
    }

    pub fn active_entries(&self) -> Vec<&CipherSuiteEntry> {
        self.entries.iter().filter(|e| e.active).collect()
    }

    pub fn len(&self) -> usize { self.entries.len() }
    pub fn is_empty(&self) -> bool { self.entries.is_empty() }
    pub fn is_full(&self) -> bool { self.entries.len() >= self.max_entries }
    pub fn enable(&mut self) { self.enabled = true; }
    pub fn disable(&mut self) { self.enabled = false; }
    pub fn is_enabled(&self) -> bool { self.enabled }
    pub fn clear(&mut self) { self.entries.clear(); }

    pub fn ids(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.id.as_str()).collect()
    }

    pub fn top_n(&self, n: usize) -> Vec<&CipherSuiteEntry> {
        self.entries.iter().take(n).collect()
    }

    pub fn find_by_label(&self, label: &str) -> Option<&CipherSuiteEntry> {
        self.entries.iter().find(|e| e.label == label)
    }

    pub fn deactivate_all(&mut self) {
        for e in &mut self.entries { e.active = false; }
    }

    pub fn activate_all(&mut self) {
        for e in &mut self.entries { e.active = true; }
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

    pub fn drain_inactive(&mut self) -> Vec<CipherSuiteEntry> {
        let (inactive, active): (Vec<_>, Vec<_>) =
            self.entries.drain(..).partition(|e| !e.active);
        self.entries = active;
        inactive
    }
}



// ---------------------------------------------------------------------------
// encryption – Extended domain helpers
// ---------------------------------------------------------------------------

/// Extended mode for encryption utilities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum YEncryptionCipherAlgorithm {
    Aes128,
    Aes256,
    ChaCha20,
    None,
}

impl YEncryptionCipherAlgorithm {
    /// Return an index for this variant (0-based).
    pub fn index(&self) -> usize {
        match self {
            Self::Aes128 => 0,
            Self::Aes256 => 1,
            Self::ChaCha20 => 2,
            Self::None => 3,
        }
    }

    /// Human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Aes128 => "Aes128",
            Self::Aes256 => "Aes256",
            Self::ChaCha20 => "ChaCha20",
            Self::None => "None",
        }
    }

    /// List all variants.
    pub fn all() -> &'static [YEncryptionCipherAlgorithm] {
        &[
            YEncryptionCipherAlgorithm::Aes128,
            YEncryptionCipherAlgorithm::Aes256,
            YEncryptionCipherAlgorithm::ChaCha20,
            YEncryptionCipherAlgorithm::None,
        ]
    }

    /// Check if this is the first variant.
    pub fn is_default(&self) -> bool {
        self.index() == 0
    }
}

impl fmt::Display for YEncryptionCipherAlgorithm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks encryption context data.
#[derive(Debug, Clone)]
pub struct YEncryptionEncryptionContext {
    pub algorithm: String,
    pub key_size: usize,
    pub iv_length: usize,
}

impl YEncryptionEncryptionContext {
    /// Create a new instance with default values.
    pub fn new() -> Self {
        Self {
            algorithm: String::new(),
            key_size: 0,
            iv_length: 0,
        }
    }

    /// Summary string for debugging.
    pub fn summary(&self) -> String {
        format!("YEncryptionEncryptionContext({}: {:?})", "algorithm", self.algorithm)
    }
}

/// Compute a hash-like fingerprint from a label string.
pub fn y_encryption_fingerprint(label: &str) -> u64 {
    let mut h: u64 = 5381;
    for b in label.bytes() {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Truncate a string to at most `max_len` characters, appending '…' if truncated.
pub fn y_encryption_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut t = s[..max_len].to_string();
        t.push('…');
        t
    }
}

/// Normalize a key string: lowercase and replace spaces with underscores.
pub fn y_encryption_normalize_key(key: &str) -> String {
    key.to_lowercase().replace(' ', "_")
}

/// Split a dotted path into segments.
pub fn y_encryption_split_path(path: &str) -> Vec<&str> {
    path.split('.').collect()
}

/// Count occurrences of `needle` in `haystack`.
pub fn y_encryption_count_occurrences(haystack: &str, needle: &str) -> usize {
    if needle.is_empty() {
        return 0;
    }
    haystack.matches(needle).count()
}

/// Check whether `value` is within `[lo, hi]` inclusive.
pub fn y_encryption_in_range(value: i64, lo: i64, hi: i64) -> bool {
    value >= lo && value <= hi
}

/// Deduplicate a sorted slice, returning a new Vec.
pub fn y_encryption_dedup_sorted(items: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    for item in items {
        if result.last().map_or(true, |last: &String| last != item) {
            result.push(item.clone());
        }
    }
    result
}

/// Interleave two slices of strings.
pub fn y_encryption_interleave<'a>(a: &'a [String], b: &'a [String]) -> Vec<&'a String> {
    let mut out = Vec::new();
    let max = a.len().max(b.len());
    for i in 0..max {
        if i < a.len() { out.push(&a[i]); }
        if i < b.len() { out.push(&b[i]); }
    }
    out
}



// ---------------------------------------------------------------------------
// encryption – Extended encryption key ring helpers
// ---------------------------------------------------------------------------

/// Priority levels for encryption key ring.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZEncryptionPriority {
    Idle,
    Low,
    Normal,
    High,
    Realtime,
}

impl ZEncryptionPriority {
    /// Numeric weight (0–4).
    pub fn weight(&self) -> u8 {
        match self {
            Self::Idle => 0,
            Self::Low => 1,
            Self::Normal => 2,
            Self::High => 3,
            Self::Realtime => 4,
        }
    }

    /// Human-readable label for this priority.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Low => "low",
            Self::Normal => "normal",
            Self::High => "high",
            Self::Realtime => "realtime",
        }
    }

    /// Whether this priority is above Normal.
    pub fn is_elevated(&self) -> bool {
        self.weight() > 2
    }

    /// All variants in ascending order.
    pub fn all_asc() -> [ZEncryptionPriority; 5] {
        [Self::Idle, Self::Low, Self::Normal, Self::High, Self::Realtime]
    }
}

impl fmt::Display for ZEncryptionPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Tracks encryption key ring data.
#[derive(Debug, Clone)]
pub struct ZEncryptionEncryptionKeyRing {
    pub key_ids: Vec<(String, u64)>,
    pub active_key_id: String,
    pub rotation_due: bool,
}

impl ZEncryptionEncryptionKeyRing {
    /// Create with default values.
    pub fn new() -> Self {
        Self {
            key_ids: Vec::new(),
            active_key_id: String::new(),
            rotation_due: false,
        }
    }

    /// Number of items in the primary collection.
    pub fn len(&self) -> usize {
        self.key_ids.len()
    }

    /// Whether the primary collection is empty.
    pub fn is_empty(&self) -> bool {
        self.key_ids.is_empty()
    }

    /// Clear the primary collection.
    pub fn clear(&mut self) {
        self.key_ids.clear();
    }

    /// Produce a debug summary string.
    pub fn summary(&self) -> String {
        format!("ZEncryptionEncryptionKeyRing[active_key_id={:?}, rotation_due={:?}]", self.active_key_id, self.rotation_due)
    }

    /// Clone with the third field toggled (if bool) or kept as-is.
    pub fn toggled_clone(&self) -> Self {
        let mut c = self.clone();
        c.rotation_due = !c.rotation_due;
        c
    }
}

/// Compute a simple rolling hash for encryption key ring.
pub fn z_encryption_rolling_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Pad `s` to exactly `width` chars, truncating or right-padding with spaces.
pub fn z_encryption_pad_to(s: &str, width: usize) -> String {
    if s.len() >= width {
        s[..width].to_string()
    } else {
        format!("{:<width$}", s, width = width)
    }
}

/// Check whether all characters in `s` are ASCII alphanumeric or underscore.
pub fn z_encryption_is_identifier(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// Compute the Levenshtein distance between two strings (simple O(n*m) impl).
pub fn z_encryption_levenshtein(a: &str, b: &str) -> usize {
    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();
    let m = a_bytes.len();
    let n = b_bytes.len();
    let mut prev: Vec<usize> = (0..=n).collect();
    let mut curr = vec![0usize; n + 1];
    for i in 1..=m {
        curr[0] = i;
        for j in 1..=n {
            let cost = if a_bytes[i - 1] == b_bytes[j - 1] { 0 } else { 1 };
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[n]
}

/// Extract unique words from a whitespace-separated string.
pub fn z_encryption_unique_words(text: &str) -> Vec<&str> {
    let mut seen = std::collections::HashSet::new();
    text.split_whitespace().filter(|w| seen.insert(*w)).collect()
}

/// Chunk a slice into groups of `size`.
pub fn z_encryption_chunk_slice<T>(slice: &[T], size: usize) -> Vec<&[T]> {
    if size == 0 { return vec![]; }
    slice.chunks(size).collect()
}

/// Return the longest common prefix of two strings.
pub fn z_encryption_common_prefix<'a>(a: &'a str, b: &str) -> &'a str {
    let end = a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count();
    &a[..end]
}


// ---------------------------------------------------------------------------
// xb_ utilities – batch 91
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer91 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer91 {
    /// Create a new ring buffer with the given capacity.
    pub fn new(cap: usize) -> Self {
        Self {
            buf: vec![0i64; cap],
            cap,
            head: 0,
            len: 0,
        }
    }

    /// Push a value into the buffer, overwriting the oldest if full.
    pub fn push(&mut self, val: i64) {
        let pos = (self.head + self.len) % self.cap;
        self.buf[pos] = val;
        if self.len == self.cap {
            self.head = (self.head + 1) % self.cap;
        } else {
            self.len += 1;
        }
    }

    /// Return the number of elements currently stored.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Return whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Get element at logical index (0 = oldest).
    pub fn get(&self, index: usize) -> Option<i64> {
        if index >= self.len {
            return None;
        }
        Some(self.buf[(self.head + index) % self.cap])
    }

    /// Drain all elements oldest-first.
    pub fn drain_all(&mut self) -> Vec<i64> {
        let mut out = Vec::with_capacity(self.len);
        for i in 0..self.len {
            out.push(self.buf[(self.head + i) % self.cap]);
        }
        self.head = 0;
        self.len = 0;
        out
    }

    /// Peek at the oldest element.
    pub fn peek_front(&self) -> Option<i64> {
        self.get(0)
    }

    /// Peek at the newest element.
    pub fn peek_back(&self) -> Option<i64> {
        if self.len == 0 {
            None
        } else {
            self.get(self.len - 1)
        }
    }

    /// Clear the buffer.
    pub fn clear(&mut self) {
        self.head = 0;
        self.len = 0;
    }

    /// Return capacity.
    pub fn capacity(&self) -> usize {
        self.cap
    }
}

/// Compute a simple FNV-1a 64-bit hash over bytes.
pub fn xb_fnv1a_91(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_91<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
    let mut result = Vec::new();
    let mut i = 0;
    while i < items.len() {
        let val = &items[i];
        let mut count = 1;
        while i + count < items.len() && items[i + count] == *val {
            count += 1;
        }
        result.push((val.clone(), count));
        i += count;
    }
    result
}

/// Decode an RLE-encoded sequence.
pub fn xb_rle_decode_91<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_91(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_91(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 43
// ---------------------------------------------------------------------------

/// Generic object pool `Xc43Pool<T>`.
pub struct Xc43Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc43Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc43PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc43Pool<T> {
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
    pub fn stats(&self) -> Xc43PoolStats {
        Xc43PoolStats {
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

impl<T> Default for Xc43Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc43Scheduler`.
pub struct Xc43Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc43Scheduler {
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

impl Default for Xc43Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_43 hash for the given byte slice.
pub fn xc_43_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_43 convention.
pub fn xc_43_reverse(s: &str) -> String {
    s.chars().rev().collect()
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

    #[test]
    fn test_encrypted_payload_display() {
        let p = encrypt_payload(b"hello", &derive_key("test"));
        let s = format!("{p}");
        assert!(s.contains("bytes"));
    }

    #[test]
    fn test_encrypted_payload_default() {
        let p = EncryptedPayload::default();
        assert!(p.is_empty());
        assert_eq!(p.total_size(), 0);
    }

    #[test]
    fn test_encrypted_payload_to_hex() {
        let p = encrypt_payload(b"test", &derive_key("key"));
        let hex = p.to_hex_string();
        assert!(hex.contains(':'));
        assert!(!hex.is_empty());
    }

    #[test]
    fn test_validate_key_strength() {
        let good_key = derive_key("strong password");
        assert!(validate_key_strength(&good_key).is_ok());
        assert!(validate_key_strength(&[0u8; 10]).is_err()); // too short
        assert!(validate_key_strength(&[0u8; 32]).is_err()); // all zeros
        assert!(validate_key_strength(&[42u8; 32]).is_err()); // all same
    }

    #[test]
    fn test_validate_password() {
        assert!(validate_password("StrongPass1").is_ok());
        assert!(validate_password("short").is_err());
        assert!(validate_password("nouppercase1").is_err());
    }

    #[test]
    fn test_byte_entropy() {
        assert!((byte_entropy(&[]) - 0.0).abs() < f64::EPSILON);
        // Random-ish data should have higher entropy than constant data
        let constant = vec![42u8; 100];
        let varied: Vec<u8> = (0..=255).collect();
        assert!(byte_entropy(&varied) > byte_entropy(&constant));
    }

    #[test]
    fn test_audit_log_helpers() {
        let mut log = EncryptionAuditLog::default();
        assert!(log.is_empty());
        log.log_operation("encrypt", 1000, true);
        log.log_operation("decrypt", 1001, true);
        assert_eq!(log.len(), 2);
        assert_eq!(log.filter_by_operation("encrypt").len(), 1);
    }

    #[test]
    fn test_encryption_config_builder() {
        let cfg = EncryptionServiceConfigBuilder::new()
            .algorithm(EncryptionAlgorithm::DoubleXor)
            .key_rotation_interval(7200)
            .max_payload_size(1024)
            .audit_enabled(false)
            .build();
        assert_eq!(cfg.algorithm, EncryptionAlgorithm::DoubleXor);
        assert_eq!(cfg.key_rotation_interval, 7200);
        assert_eq!(cfg.max_payload_size, 1024);
        assert!(!cfg.audit_enabled);
        assert!(format!("{cfg}").contains("Double-XOR"));
    }

    #[test]
    fn test_encryption_config_default() {
        let cfg = EncryptionServiceConfig::default();
        assert_eq!(cfg.algorithm, EncryptionAlgorithm::Xor);
        assert!(cfg.audit_enabled);
    }

    #[test]
    fn test_keyring_operations() {
        let mut ring = KeyRing::new();
        assert!(ring.is_empty());
        ring.add("primary", vec![1, 2, 3]);
        ring.add("backup", vec![4, 5, 6]);
        assert_eq!(ring.len(), 2);
        assert_eq!(ring.list_names(), vec!["primary", "backup"]);
        assert_eq!(ring.get("primary"), Some([1u8, 2, 3].as_slice()));
        assert_eq!(ring.active_key(), Some([1u8, 2, 3].as_slice()));
        assert!(ring.set_active("backup"));
        assert_eq!(ring.active_key(), Some([4u8, 5, 6].as_slice()));
        assert!(ring.remove("primary"));
        assert_eq!(ring.len(), 1);
        assert!(format!("{ring}").contains("1 keys"));
    }

    #[test]
    fn test_keyring_rotate() {
        let mut ring = KeyRing::new();
        ring.add("k1", vec![10, 20]);
        assert!(ring.rotate("k1", vec![30, 40]));
        assert_eq!(ring.get("k1"), Some([30u8, 40].as_slice()));
        assert!(!ring.rotate("missing", vec![1]));
    }

    #[test]
    fn test_keyring_duplicate_add() {
        let mut ring = KeyRing::new();
        ring.add("k1", vec![1]);
        ring.add("k1", vec![2]);
        assert_eq!(ring.len(), 1);
        assert_eq!(ring.get("k1"), Some([1u8].as_slice()));
    }

    #[test]
    fn test_encryption_throughput_stats() {
        let mut stats = EncryptionThroughputStats::default();
        stats.record_encrypt(100);
        stats.record_encrypt(200);
        stats.record_decrypt(150);
        assert_eq!(stats.encrypt_count, 2);
        assert_eq!(stats.decrypt_count, 1);
        assert_eq!(stats.total_operations(), 3);
        assert_eq!(stats.total_bytes(), 450);
        assert_eq!(stats.bytes_encrypted, 300);
        let s = format!("{stats}");
        assert!(s.contains("enc=2/300"));
        assert!(s.contains("dec=1/150"));
    }

    // -----------------------------------------------------------------------
    // New tests for deepened functionality
    // -----------------------------------------------------------------------

    #[test]
    fn test_secret_store_set_get() {
        let mut store = SecretStore::from_passphrase("store-pass");
        assert!(store.is_empty());
        store.set("api-key", b"sk-12345");
        store.set("token", b"tok-abcdef");
        assert_eq!(store.len(), 2);
        assert!(store.contains("api-key"));
        assert!(!store.contains("missing"));
        assert_eq!(store.get("api-key").unwrap(), b"sk-12345");
        assert_eq!(store.get_string("token").unwrap(), "tok-abcdef");
        assert!(store.get("missing").is_none());
    }

    #[test]
    fn test_secret_store_overwrite() {
        let mut store = SecretStore::from_passphrase("pw");
        store.set("key", b"value1");
        store.set("key", b"value2");
        assert_eq!(store.len(), 1);
        assert_eq!(store.get("key").unwrap(), b"value2");
    }

    #[test]
    fn test_secret_store_remove_and_clear() {
        let mut store = SecretStore::from_passphrase("pw");
        store.set("a", b"1");
        store.set("b", b"2");
        assert!(store.remove("a"));
        assert!(!store.remove("a"));
        assert_eq!(store.len(), 1);
        assert_eq!(store.names(), vec!["b"]);
        store.clear();
        assert!(store.is_empty());
    }

    #[test]
    fn test_secret_store_display() {
        let store = SecretStore::from_passphrase("pw");
        let s = format!("{store}");
        assert!(s.contains("0 secrets"));
    }

    #[test]
    fn test_encryption_pipeline_round_trip() {
        let pipeline = EncryptionPipeline::new()
            .add_passphrase_layer("layer1")
            .add_passphrase_layer("layer2")
            .add_layer(vec![0xAA; 16]);
        assert_eq!(pipeline.depth(), 3);
        let data = b"multi-layer secret";
        let encrypted = pipeline.encrypt(data);
        assert_ne!(encrypted, data.to_vec());
        let decrypted = pipeline.decrypt(&encrypted);
        assert_eq!(decrypted, data);
    }

    #[test]
    fn test_encryption_pipeline_empty() {
        let pipeline = EncryptionPipeline::default();
        assert_eq!(pipeline.depth(), 0);
        let data = b"pass-through";
        assert_eq!(pipeline.encrypt(data), data.to_vec());
        assert_eq!(pipeline.decrypt(data), data.to_vec());
        assert!(format!("{pipeline}").contains("0 layers"));
    }

    #[test]
    fn test_hex_encode_decode_round_trip() {
        let data = vec![0x00, 0xFF, 0xAB, 0x12, 0x34];
        let hex = hex_encode(&data);
        assert_eq!(hex, "00ffab1234");
        let decoded = hex_decode(&hex).unwrap();
        assert_eq!(decoded, data);
    }

    #[test]
    fn test_hex_decode_invalid() {
        assert!(hex_decode("0").is_none()); // odd length
        assert!(hex_decode("zz").is_none()); // invalid chars
        assert!(hex_decode("").unwrap().is_empty()); // empty is valid
    }

    #[test]
    fn test_hex_decode_uppercase() {
        assert_eq!(hex_decode("ABCD").unwrap(), vec![0xAB, 0xCD]);
    }

    #[test]
    fn test_encrypt_to_hex_round_trip() {
        let svc = EncryptionService::from_passphrase("hex-key");
        let data = b"hex test data";
        let hex = svc.encrypt_to_hex(data);
        let decrypted = svc.decrypt_from_hex(&hex).unwrap();
        assert_eq!(decrypted, data);
    }

    #[test]
    fn test_decrypt_from_hex_invalid() {
        let svc = EncryptionService::from_passphrase("k");
        assert!(svc.decrypt_from_hex("zz").is_none());
    }

    #[test]
    fn test_encrypt_and_sign_verify_and_decrypt() {
        let svc = EncryptionService::from_passphrase("sign-key");
        let data = b"signed message";
        let (ct, sig) = svc.encrypt_and_sign(data);
        let plaintext = svc.verify_and_decrypt(&ct, &sig).unwrap();
        assert_eq!(plaintext, data);
    }

    #[test]
    fn test_verify_and_decrypt_bad_signature() {
        let svc = EncryptionService::from_passphrase("sign-key");
        let data = b"message";
        let (ct, _sig) = svc.encrypt_and_sign(data);
        let bad_sig = vec![0u8; 32];
        assert!(svc.verify_and_decrypt(&ct, &bad_sig).is_none());
    }

    #[test]
    fn test_service_sign_verify() {
        let svc = EncryptionService::from_passphrase("hmac-key");
        let data = b"to be signed";
        let sig = svc.sign(data);
        assert!(svc.verify(data, &sig));
        assert!(!svc.verify(b"different data", &sig));
    }

    #[test]
    fn test_service_key_len() {
        let svc = EncryptionService::new(vec![1, 2, 3, 4]);
        assert_eq!(svc.key_len(), 4);
        let svc2 = EncryptionService::from_passphrase("test");
        assert_eq!(svc2.key_len(), 32);
    }

    #[test]
    fn test_keyring_re_encrypt() {
        let mut ring = KeyRing::new();
        ring.add("k1", vec![0xAA, 0xBB, 0xCC]);
        ring.add("k2", vec![0x11, 0x22, 0x33]);
        let svc1 = EncryptionService::new(vec![0xAA, 0xBB, 0xCC]);
        let original = b"migrate me";
        let encrypted_k1 = svc1.encrypt(original);
        let encrypted_k2 = ring.re_encrypt(&encrypted_k1, "k1", "k2").unwrap();
        let svc2 = EncryptionService::new(vec![0x11, 0x22, 0x33]);
        assert_eq!(svc2.decrypt(&encrypted_k2), original);
    }

    #[test]
    fn test_keyring_re_encrypt_missing_key() {
        let ring = KeyRing::new();
        assert!(ring.re_encrypt(b"data", "k1", "k2").is_err());
    }

    #[test]
    fn test_keyring_encrypt_decrypt_with_active() {
        let mut ring = KeyRing::new();
        ring.add("main", derive_key("ring-key"));
        let data = b"active key test";
        let encrypted = ring.encrypt_with_active(data).unwrap();
        let decrypted = ring.decrypt_with_active(&encrypted).unwrap();
        assert_eq!(decrypted, data);
    }

    #[test]
    fn test_keyring_encrypt_no_active() {
        let ring = KeyRing::new();
        assert!(ring.encrypt_with_active(b"x").is_err());
        assert!(ring.decrypt_with_active(b"x").is_err());
    }

    #[test]
    fn test_keyring_sorted_names() {
        let mut ring = KeyRing::new();
        ring.add("charlie", vec![3]);
        ring.add("alpha", vec![1]);
        ring.add("bravo", vec![2]);
        assert_eq!(ring.sorted_names(), vec!["alpha", "bravo", "charlie"]);
    }

    #[test]
    fn test_keyring_contains() {
        let mut ring = KeyRing::new();
        ring.add("exists", vec![1]);
        assert!(ring.contains("exists"));
        assert!(!ring.contains("nope"));
    }

    #[test]
    fn test_throughput_stats_merge() {
        let mut a = EncryptionThroughputStats::default();
        a.record_encrypt(100);
        let mut b = EncryptionThroughputStats::default();
        b.record_decrypt(200);
        a.merge(&b);
        assert_eq!(a.encrypt_count, 1);
        assert_eq!(a.decrypt_count, 1);
        assert_eq!(a.total_bytes(), 300);
    }

    #[test]
    fn test_throughput_stats_reset() {
        let mut stats = EncryptionThroughputStats::default();
        stats.record_encrypt(500);
        stats.reset();
        assert_eq!(stats.total_operations(), 0);
        assert_eq!(stats.total_bytes(), 0);
    }

    #[test]
    fn test_throughput_stats_avg_bytes() {
        let mut stats = EncryptionThroughputStats::default();
        assert_eq!(stats.avg_bytes_per_op(), 0);
        stats.record_encrypt(100);
        stats.record_decrypt(200);
        assert_eq!(stats.avg_bytes_per_op(), 150);
    }

    #[test] fn keyRotationManager_new() { let s = KeyRotationManager::new(); assert_eq!(s.entry_count(), 0); assert!(s.is_enabled()); }
    #[test] fn keyRotationManager_add() { let mut s = KeyRotationManager::new(); s.add_entry("a"); s.add_entry("b"); assert_eq!(s.entry_count(), 2); }
    #[test] fn keyRotationManager_remove() { let mut s = KeyRotationManager::new(); s.add_entry("a"); assert!(s.remove_entry(0).is_some()); assert_eq!(s.entry_count(), 0); }
    #[test] fn keyRotationManager_config() { let mut s = KeyRotationManager::new(); s.set_config("k", "v"); assert_eq!(s.get_config("k"), Some("v")); }
    #[test] fn keyRotationManager_nav() { let mut s = KeyRotationManager::new(); s.add_entry("a"); s.add_entry("b"); s.select_next(); assert_eq!(s.current_index(), 1); s.select_prev(); assert_eq!(s.current_index(), 0); }
    #[test] fn keyRotationManager_filter() { let mut s = KeyRotationManager::new(); s.add_entry("hello"); s.add_entry("world"); assert_eq!(s.filter_entries("llo").len(), 1); }
    #[test] fn keyRotationManager_display() { assert!(format!("{}", KeyRotationManager::new()).contains("KeyRotationManager")); }
    #[test] fn encryptedStorageAdapter_new() { let s = EncryptedStorageAdapter::new(); assert!(s.is_empty()); }
    #[test] fn encryptedStorageAdapter_add() { let mut s = EncryptedStorageAdapter::new(); s.add_item("g1", "v1"); s.add_item("g1", "v2"); assert_eq!(s.total_items(), 2); assert_eq!(s.group_count(), 1); }
    #[test] fn encryptedStorageAdapter_active() { let mut s = EncryptedStorageAdapter::new(); s.set_active("g1"); assert_eq!(s.active(), Some("g1")); s.clear_active(); assert!(s.active().is_none()); }
    #[test] fn encryptedStorageAdapter_error() { let mut s = EncryptedStorageAdapter::new(); s.set_error("fail"); assert_eq!(s.last_error(), Some("fail")); s.clear_error(); assert!(s.last_error().is_none()); }
    #[test] fn encryptedStorageAdapter_rm_group() { let mut s = EncryptedStorageAdapter::new(); s.add_item("g", "v"); assert!(s.remove_group("g")); assert!(s.is_empty()); }
    #[test] fn encryptedStorageAdapter_display() { assert!(format!("{}", EncryptedStorageAdapter::new()).contains("EncryptedStorageAdapter")); }


    #[test] fn keyRotationManager_snap_capture() {
        let s = KeyRotationManager::new();
        let snap = KeyRotationManagerSnapshot::capture(&s, 1000);
        assert_eq!(snap.entry_count, 0);
        assert_eq!(snap.timestamp, 1000);
    }
    #[test] fn keyRotationManager_snap_stale() {
        let s = KeyRotationManager::new();
        let snap = KeyRotationManagerSnapshot::capture(&s, 100);
        assert!(snap.is_stale(300, 100));
        assert!(!snap.is_stale(150, 100));
    }
    #[test] fn keyRotationManager_snap_diff() {
        let s = KeyRotationManager::new();
        let s1v = KeyRotationManagerSnapshot::capture(&s, 100);
        let mut s2v = s1v.clone();
        s2v.entry_count = 5;
        assert_eq!(s2v.diff_entry_count(&s1v), 5);
    }
    #[test] fn keyRotationManager_snap_display() {
        let s = KeyRotationManager::new();
        let snap = KeyRotationManagerSnapshot::capture(&s, 0);
        assert!(format!("{}", snap).contains("Snapshot"));
    }
    #[test] fn encryptedStorageAdapter_stats_record() {
        let mut st = EncryptedStorageAdapterStats::new();
        st.record_add();
        st.record_add();
        st.record_remove();
        assert_eq!(st.net_changes(), 1);
    }
    #[test] fn encryptedStorageAdapter_stats_hit_ratio() {
        let mut st = EncryptedStorageAdapterStats::new();
        st.record_lookup(true);
        st.record_lookup(true);
        st.record_lookup(false);
        assert!((st.hit_ratio() - 2.0/3.0).abs() < 0.01);
    }
    #[test] fn encryptedStorageAdapter_stats_merge() {
        let mut a = EncryptedStorageAdapterStats::new();
        a.total_adds = 5;
        let mut b = EncryptedStorageAdapterStats::new();
        b.total_adds = 3;
        a.merge(&b);
        assert_eq!(a.total_adds, 8);
    }
    #[test] fn encryptedStorageAdapter_stats_display() {
        let st = EncryptedStorageAdapterStats::new();
        assert!(format!("{}", st).contains("Stats"));
    }
    #[test] fn keyRotationManager_config_default() {
        let c = KeyRotationManagerConfig::new();
        assert_eq!(c.max_entries, 10000);
        assert!(c.auto_cleanup);
    }
    #[test] fn keyRotationManager_config_builder() {
        let c = KeyRotationManagerConfig::new().with_max_entries(500).with_auto_cleanup(false).with_debounce(200);
        assert_eq!(c.max_entries, 500);
        assert!(!c.auto_cleanup);
        assert_eq!(c.debounce_ms, 200);
    }
    #[test] fn keyRotationManager_config_labels() {
        let mut c = KeyRotationManagerConfig::new();
        c.set_label("a", "b");
        assert_eq!(c.get_label("a"), Some("b"));
        assert_eq!(c.label_count(), 1);
    }
    #[test] fn keyRotationManager_config_cleanup_threshold() {
        let c = KeyRotationManagerConfig::new();
        assert!(!c.needs_cleanup(100));
        assert!(c.needs_cleanup(9000));
    }
    #[test] fn keyRotationManager_config_display() {
        assert!(format!("{}", KeyRotationManagerConfig::new()).contains("Config"));
    }
    #[test] fn encryptedStorageAdapter_stats_peaks() {
        let mut st = EncryptedStorageAdapterStats::new();
        st.update_peaks(5, 20);
        st.update_peaks(3, 25);
        assert_eq!(st.peak_group_count, 5);
        assert_eq!(st.peak_item_count, 25);
    }

    // ── KeyDerivationConfig tests ──

    #[test]
    fn kdf_defaults_are_strong() {
        let cfg = KeyDerivationConfig::new();
        assert!(cfg.is_strong());
        assert_eq!(cfg.iterations, 100_000);
    }

    #[test]
    fn kdf_builder_chain() {
        let cfg = KeyDerivationConfig::new().with_iterations(50_000).with_salt_len(8).with_key_len(16);
        assert_eq!(cfg.iterations, 50_000);
        assert_eq!(cfg.salt_len, 8);
        assert_eq!(cfg.key_len, 16);
        assert!(!cfg.is_strong());
    }

    #[test]
    fn kdf_display() {
        let cfg = KeyDerivationConfig::new();
        let s = format!("{}", cfg);
        assert!(s.contains("100000"));
    }

    #[test]
    fn kdf_estimated_time() {
        let cfg = KeyDerivationConfig::new();
        assert!(cfg.estimated_time_ms() > 0.0);
    }

    // ── SecureStringMasker tests ──

    #[test]
    fn mask_except_last_n() {
        assert_eq!(SecureStringMasker::mask_except_last_n("secret123", 3, '*'), "******123");
    }

    #[test]
    fn mask_except_last_n_short() {
        assert_eq!(SecureStringMasker::mask_except_last_n("ab", 5, '*'), "ab");
    }

    #[test]
    fn mask_middle() {
        assert_eq!(SecureStringMasker::mask_middle("abcdefgh", 2, '*'), "ab****gh");
    }

    #[test]
    fn redact_email() {
        assert_eq!(SecureStringMasker::redact_email("john@example.com"), "j***@example.com");
    }

    #[test]
    fn redact_with_char() {
        assert_eq!(SecureStringMasker::redact_with_char("hello", '#'), "#####");
    }

    // ── CryptoAuditLog tests ──

    #[test]
    fn crypto_audit_record_and_query() {
        let mut log = CryptoAuditLog::new();
        log.record(CryptoAuditKind::Encrypt, "key1", 100);
        log.record(CryptoAuditKind::Decrypt, "key1", 200);
        log.record(CryptoAuditKind::Encrypt, "key2", 300);
        assert_eq!(log.total_entries(), 3);
        assert_eq!(log.operation_count(&CryptoAuditKind::Encrypt), 2);
        assert_eq!(log.operation_count(&CryptoAuditKind::Decrypt), 1);
    }

    #[test]
    fn crypto_audit_time_range() {
        let mut log = CryptoAuditLog::new();
        log.record(CryptoAuditKind::Encrypt, "k", 100);
        log.record(CryptoAuditKind::Encrypt, "k", 200);
        log.record(CryptoAuditKind::Encrypt, "k", 300);
        let range = log.query_by_time_range(150, 250);
        assert_eq!(range.len(), 1);
        assert_eq!(range[0].timestamp, 200);
    }

    #[test]
    fn crypto_audit_last_and_by_key() {
        let mut log = CryptoAuditLog::new();
        log.record(CryptoAuditKind::Encrypt, "a", 10);
        log.record(CryptoAuditKind::Decrypt, "b", 20);
        assert_eq!(log.last_operation().unwrap().key_id, "b");
        assert_eq!(log.entries_for_key("a").len(), 1);
    }

    #[test]
    fn cipher_suite_entry_creation() {
        let e = CipherSuiteEntry::new("aes256", "AES-256");
        assert_eq!(e.id, "aes256");
        assert!(e.active);
    }

    #[test]
    fn cipher_suite_entry_priority() {
        let e = CipherSuiteEntry::new("e1", "E").with_priority(5);
        assert_eq!(e.priority, 5);
    }

    #[test]
    fn cipher_suite_entry_metadata() {
        let e = CipherSuiteEntry::new("e1", "E").with_meta("bits", "256");
        assert_eq!(e.get_meta("bits"), Some("256"));
        assert!(e.has_meta("bits"));
    }

    #[test]
    fn cipher_suite_entry_remove_meta() {
        let mut e = CipherSuiteEntry::new("e1", "E").with_meta("k", "v");
        assert!(e.remove_meta("k"));
        assert!(!e.remove_meta("k"));
    }

    #[test]
    fn cipher_suite_entry_activate_deactivate() {
        let mut e = CipherSuiteEntry::new("e1", "E");
        e.deactivate(); assert!(!e.active);
        e.activate(); assert!(e.active);
    }

    #[test]
    fn cipher_suite_registry_add_sorted() {
        let mut r = CipherSuiteRegistry::new(10);
        r.add(CipherSuiteEntry::new("lo", "Lo").with_priority(1));
        r.add(CipherSuiteEntry::new("hi", "Hi").with_priority(10));
        assert_eq!(r.ids()[0], "hi");
    }

    #[test]
    fn cipher_suite_registry_capacity() {
        let mut r = CipherSuiteRegistry::new(1);
        assert!(r.add(CipherSuiteEntry::new("a", "A")));
        assert!(!r.add(CipherSuiteEntry::new("b", "B")));
    }

    #[test]
    fn cipher_suite_registry_remove() {
        let mut r = CipherSuiteRegistry::new(10);
        r.add(CipherSuiteEntry::new("a", "A"));
        assert!(r.remove("a"));
        assert!(r.is_empty());
    }

    #[test]
    fn cipher_suite_registry_active_entries() {
        let mut r = CipherSuiteRegistry::new(10);
        r.add(CipherSuiteEntry::new("a", "A"));
        r.add(CipherSuiteEntry::new("b", "B"));
        r.get_mut("a").unwrap().deactivate();
        assert_eq!(r.count_active(), 1);
    }

    #[test]
    fn cipher_suite_registry_enable_disable() {
        let mut r = CipherSuiteRegistry::new(10);
        r.disable(); assert!(!r.is_enabled());
        r.enable(); assert!(r.is_enabled());
    }

    #[test]
    fn cipher_suite_registry_find_by_label() {
        let mut r = CipherSuiteRegistry::new(10);
        r.add(CipherSuiteEntry::new("a", "Alpha"));
        assert_eq!(r.find_by_label("Alpha").unwrap().id, "a");
    }

    #[test]
    fn cipher_suite_registry_drain_inactive() {
        let mut r = CipherSuiteRegistry::new(10);
        r.add(CipherSuiteEntry::new("a", "A"));
        r.add(CipherSuiteEntry::new("b", "B"));
        r.get_mut("a").unwrap().deactivate();
        let drained = r.drain_inactive();
        assert_eq!(drained.len(), 1);
        assert_eq!(r.len(), 1);
    }

    // -- encryption extended domain tests ----------------------------------------

    #[test]
    fn y_encryption_enum_index() {
        assert_eq!(YEncryptionCipherAlgorithm::Aes128.index(), 0);
        assert_eq!(YEncryptionCipherAlgorithm::Aes256.index(), 1);
        assert_eq!(YEncryptionCipherAlgorithm::ChaCha20.index(), 2);
        assert_eq!(YEncryptionCipherAlgorithm::None.index(), 3);
    }

    #[test]
    fn y_encryption_enum_label() {
        assert_eq!(YEncryptionCipherAlgorithm::Aes128.label(), "Aes128");
        assert_eq!(YEncryptionCipherAlgorithm::Aes256.label(), "Aes256");
        assert_eq!(YEncryptionCipherAlgorithm::ChaCha20.label(), "ChaCha20");
        assert_eq!(YEncryptionCipherAlgorithm::None.label(), "None");
    }

    #[test]
    fn y_encryption_enum_all() {
        let all = YEncryptionCipherAlgorithm::all();
        assert_eq!(all.len(), 4);
    }

    #[test]
    fn y_encryption_enum_is_default() {
        assert!(YEncryptionCipherAlgorithm::Aes128.is_default());
        assert!(!YEncryptionCipherAlgorithm::None.is_default());
    }

    #[test]
    fn y_encryption_enum_display() {
        assert_eq!(format!("{}", YEncryptionCipherAlgorithm::Aes128), "Aes128");
    }

    #[test]
    fn y_encryption_struct_new() {
        let s = YEncryptionEncryptionContext::new();
        let _ = s.summary();
    }

    #[test]
    fn y_encryption_fingerprint_deterministic() {
        let h1 = y_encryption_fingerprint("hello");
        let h2 = y_encryption_fingerprint("hello");
        assert_eq!(h1, h2);
        assert_ne!(y_encryption_fingerprint("a"), y_encryption_fingerprint("b"));
    }

    #[test]
    fn y_encryption_truncate_short() {
        assert_eq!(y_encryption_truncate("hi", 10), "hi");
    }

    #[test]
    fn y_encryption_truncate_long() {
        let r = y_encryption_truncate("abcdef", 3);
        assert!(r.starts_with("abc"));
        assert_eq!(r.len(), 3 + '…'.len_utf8());
    }

    #[test]
    fn y_encryption_normalize_key_basic() {
        assert_eq!(y_encryption_normalize_key("Hello World"), "hello_world");
    }

    #[test]
    fn y_encryption_split_path_basic() {
        let parts = y_encryption_split_path("a.b.c");
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn y_encryption_count_occurrences_basic() {
        assert_eq!(y_encryption_count_occurrences("abcabc", "abc"), 2);
        assert_eq!(y_encryption_count_occurrences("abc", "xyz"), 0);
        assert_eq!(y_encryption_count_occurrences("abc", ""), 0);
    }

    #[test]
    fn y_encryption_in_range_basic() {
        assert!(y_encryption_in_range(5, 1, 10));
        assert!(y_encryption_in_range(1, 1, 10));
        assert!(y_encryption_in_range(10, 1, 10));
        assert!(!y_encryption_in_range(0, 1, 10));
        assert!(!y_encryption_in_range(11, 1, 10));
    }

    #[test]
    fn y_encryption_dedup_sorted_basic() {
        let items: Vec<String> = vec!["a".into(), "a".into(), "b".into(), "c".into(), "c".into()];
        let deduped = y_encryption_dedup_sorted(&items);
        assert_eq!(deduped.len(), 3);
        assert_eq!(deduped[0], "a");
    }

    #[test]
    fn y_encryption_interleave_basic() {
        let a: Vec<String> = vec!["a".into(), "b".into()];
        let b: Vec<String> = vec!["1".into(), "2".into(), "3".into()];
        let r = y_encryption_interleave(&a, &b);
        assert_eq!(r.len(), 5);
        assert_eq!(r[0], "a");
        assert_eq!(r[1], "1");
    }

    // -- encryption Z-extended tests -----------------------------------------------

    #[test]
    fn z_encryption_priority_weight() {
        assert_eq!(ZEncryptionPriority::Idle.weight(), 0);
        assert_eq!(ZEncryptionPriority::Normal.weight(), 2);
        assert_eq!(ZEncryptionPriority::Realtime.weight(), 4);
    }

    #[test]
    fn z_encryption_priority_label() {
        assert_eq!(ZEncryptionPriority::Low.label(), "low");
        assert_eq!(ZEncryptionPriority::High.label(), "high");
    }

    #[test]
    fn z_encryption_priority_is_elevated() {
        assert!(!ZEncryptionPriority::Normal.is_elevated());
        assert!(ZEncryptionPriority::High.is_elevated());
        assert!(ZEncryptionPriority::Realtime.is_elevated());
    }

    #[test]
    fn z_encryption_priority_display() {
        assert_eq!(format!("{}", ZEncryptionPriority::Idle), "idle");
    }

    #[test]
    fn z_encryption_priority_all_asc() {
        let all = ZEncryptionPriority::all_asc();
        assert_eq!(all.len(), 5);
        assert_eq!(all[0], ZEncryptionPriority::Idle);
        assert_eq!(all[4], ZEncryptionPriority::Realtime);
    }

    #[test]
    fn z_encryption_struct_new() {
        let s = ZEncryptionEncryptionKeyRing::new();
        assert!(s.is_empty());
        let _ = s.summary();
    }

    #[test]
    fn z_encryption_struct_toggled_clone() {
        let s = ZEncryptionEncryptionKeyRing::new();
        let t = s.toggled_clone();
        assert_ne!(s.rotation_due, t.rotation_due);
    }

    #[test]
    fn z_encryption_rolling_hash_deterministic() {
        let h1 = z_encryption_rolling_hash(b"test");
        let h2 = z_encryption_rolling_hash(b"test");
        assert_eq!(h1, h2);
        assert_ne!(z_encryption_rolling_hash(b"a"), z_encryption_rolling_hash(b"b"));
    }

    #[test]
    fn z_encryption_pad_to_basic() {
        assert_eq!(z_encryption_pad_to("hi", 5), "hi   ");
        assert_eq!(z_encryption_pad_to("hello world", 5), "hello");
    }

    #[test]
    fn z_encryption_is_identifier_basic() {
        assert!(z_encryption_is_identifier("foo_bar"));
        assert!(z_encryption_is_identifier("abc123"));
        assert!(!z_encryption_is_identifier(""));
        assert!(!z_encryption_is_identifier("has space"));
    }

    #[test]
    fn z_encryption_levenshtein_basic() {
        assert_eq!(z_encryption_levenshtein("", ""), 0);
        assert_eq!(z_encryption_levenshtein("abc", "abc"), 0);
        assert_eq!(z_encryption_levenshtein("kitten", "sitting"), 3);
    }

    #[test]
    fn z_encryption_unique_words_basic() {
        let w = z_encryption_unique_words("the cat sat on the mat");
        assert_eq!(w.len(), 5);
        assert_eq!(w[0], "the");
    }

    #[test]
    fn z_encryption_chunk_slice_basic() {
        let data = vec![1, 2, 3, 4, 5];
        let chunks = z_encryption_chunk_slice(&data, 2);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], &[1, 2]);
        assert_eq!(chunks[2], &[5]);
    }

    #[test]
    fn z_encryption_common_prefix_basic() {
        assert_eq!(z_encryption_common_prefix("abcdef", "abcxyz"), "abc");
        assert_eq!(z_encryption_common_prefix("xyz", "abc"), "");
    }

    #[test]
    fn z_encryption_struct_clear() {
        let mut s = ZEncryptionEncryptionKeyRing::new();
        s.key_ids.push(Default::default());
        assert_eq!(s.len(), 1);
        s.clear();
        assert!(s.is_empty());
    }

    #[test]
    fn z_encryption_rolling_hash_empty() {
        let h = z_encryption_rolling_hash(b"");
        assert_eq!(h, 0xcbf29ce484222325);
    }

    #[test]
    fn xb_ring_buffer_91_push_and_len() {
        let mut rb = super::XbRingBuffer91::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_91_overwrite() {
        let mut rb = super::XbRingBuffer91::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_91_get_out_of_bounds() {
        let rb = super::XbRingBuffer91::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_91_drain_all() {
        let mut rb = super::XbRingBuffer91::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_91_peek_front_back() {
        let mut rb = super::XbRingBuffer91::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_91_clear() {
        let mut rb = super::XbRingBuffer91::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_91_capacity() {
        let rb = super::XbRingBuffer91::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_91_basic() {
        let h = super::xb_fnv1a_91(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_91(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_91_different_inputs() {
        let h1 = super::xb_fnv1a_91(b"abc");
        let h2 = super::xb_fnv1a_91(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_91_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_91(&data);
        let dec = super::xb_rle_decode_91(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_91_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_91(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_91(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_91_values() {
        assert!((super::xb_clamp_91(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_91(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_91(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_91_values() {
        assert!((super::xb_lerp_91(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_91(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_91(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_91_wrap_around_twice() {
        let mut rb = super::XbRingBuffer91::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
    }


    // ---- xc_ pool / scheduler tests – block 43 ----

    #[test]
    fn xc_43_pool_new_empty() {
        let pool: super::Xc43Pool<i32> = super::Xc43Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_43_pool_release_acquire() {
        let mut pool = super::Xc43Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_43_pool_acquire_empty() {
        let mut pool: super::Xc43Pool<i32> = super::Xc43Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_43_pool_full() {
        let mut pool = super::Xc43Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_43_pool_drain() {
        let mut pool = super::Xc43Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_43_pool_stats() {
        let mut pool = super::Xc43Pool::new(8);
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
    fn xc_43_pool_clear() {
        let mut pool = super::Xc43Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_43_pool_shrink() {
        let mut pool = super::Xc43Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_43_pool_default() {
        let pool: super::Xc43Pool<String> = super::Xc43Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_43_pool_extend() {
        let mut pool = super::Xc43Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_43_pool_retain() {
        let mut pool = super::Xc43Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_43_scheduler_round_robin() {
        let mut sched = super::Xc43Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_43_scheduler_empty() {
        let mut sched = super::Xc43Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_43_scheduler_reset() {
        let mut sched = super::Xc43Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_43_scheduler_add_remove() {
        let mut sched = super::Xc43Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_43_scheduler_targets() {
        let sched = super::Xc43Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_43_hash_empty() {
        assert_eq!(super::xc_43_hash(b""), 5381);
    }

    #[test]
    fn xc_43_hash_data() {
        let h = super::xc_43_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_43_hash(b"hello"), h);
    }

    #[test]
    fn xc_43_reverse_str() {
        assert_eq!(super::xc_43_reverse("abc"), "cba");
        assert_eq!(super::xc_43_reverse(""), "");
    }

}
