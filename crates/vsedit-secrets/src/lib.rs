//! Secure credential storage.
//!
//! Provides a trait-based abstraction for secret storage with an in-memory
//! implementation for testing and a keyring stub for system integration.

use std::fmt;
use std::collections::HashMap;

/// A single secret entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretEntry {
    pub key: String,
    pub value: String,
}

impl SecretEntry {
    /// Create a new secret entry after validating the key.
    pub fn new(key: impl Into<String>, value: impl Into<String>) -> Result<Self, SecretStorageError> {
        let key = key.into();
        let value = value.into();
        validate_key(&key)?;
        Ok(Self { key, value })
    }

    /// Returns the length of the secret value in bytes.
    pub fn value_len(&self) -> usize {
        self.value.len()
    }

    /// Returns true if the secret value is empty.
    pub fn is_empty(&self) -> bool {
        self.value.is_empty()
    }

    /// Redact the value for display purposes.
    pub fn redacted_value(&self) -> String {
        if self.value.len() <= 4 {
            "****".to_string()
        } else {
            let visible = &self.value[..2];
            format!("{}{}**", visible, "*".repeat(self.value.len() - 4))
        }
    }
}

impl std::fmt::Display for SecretEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}={}", self.key, self.redacted_value())
    }
}

/// Builder for constructing `SecretEntry` values with validation.
#[derive(Debug, Clone, Default)]
pub struct SecretEntryBuilder {
    key: Option<String>,
    value: Option<String>,
}

impl SecretEntryBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn key(mut self, key: impl Into<String>) -> Self {
        self.key = Some(key.into());
        self
    }

    pub fn value(mut self, value: impl Into<String>) -> Self {
        self.value = Some(value.into());
        self
    }

    pub fn build(self) -> Result<SecretEntry, SecretStorageError> {
        let key = self.key.ok_or_else(|| SecretStorageError::InvalidKey("key is required".into()))?;
        let value = self.value.ok_or_else(|| SecretStorageError::Other("value is required".into()))?;
        SecretEntry::new(key, value)
    }
}

/// Validate that a secret key is well-formed.
pub fn validate_key(key: &str) -> Result<(), SecretStorageError> {
    if key.is_empty() {
        return Err(SecretStorageError::InvalidKey("key must not be empty".into()));
    }
    if key.len() > 256 {
        return Err(SecretStorageError::InvalidKey("key must be at most 256 characters".into()));
    }
    if key.contains(char::is_whitespace) {
        return Err(SecretStorageError::InvalidKey("key must not contain whitespace".into()));
    }
    Ok(())
}

/// Event emitted when a secret is changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretStorageChangeEvent {
    pub key: String,
    pub kind: SecretChangeKind,
}

/// What kind of change happened to a secret.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SecretChangeKind {
    Added,
    Updated,
    Deleted,
}

impl std::fmt::Display for SecretChangeKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Added => write!(f, "added"),
            Self::Updated => write!(f, "updated"),
            Self::Deleted => write!(f, "deleted"),
        }
    }
}

/// Trait for secret storage backends.
pub trait SecretStorage: Send + Sync {
    /// Retrieve a secret by key.
    fn get(&self, key: &str) -> Option<String>;
    /// Store a secret, replacing any existing value.
    fn store(&mut self, key: &str, value: &str) -> Result<(), SecretStorageError>;
    /// Delete a secret by key.
    fn delete(&mut self, key: &str) -> Result<bool, SecretStorageError>;
    /// List all stored keys.
    fn keys(&self) -> Vec<String>;
}

/// Errors from secret storage operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SecretStorageError {
    /// The underlying storage backend is unavailable.
    BackendUnavailable(String),
    /// Access was denied by the system keyring.
    AccessDenied,
    /// The key failed validation.
    InvalidKey(String),
    /// A generic storage error.
    Other(String),
}

impl std::fmt::Display for SecretStorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BackendUnavailable(msg) => write!(f, "Backend unavailable: {}", msg),
            Self::AccessDenied => write!(f, "Access denied"),
            Self::InvalidKey(msg) => write!(f, "Invalid key: {}", msg),
            Self::Other(msg) => write!(f, "Secret storage error: {}", msg),
        }
    }
}

impl std::error::Error for SecretStorageError {}

/// In-memory secret storage (for testing and fallback).
#[derive(Debug, Default, Clone)]
pub struct InMemorySecretStorage {
    secrets: HashMap<String, String>,
    change_log: Vec<SecretStorageChangeEvent>,
}

impl InMemorySecretStorage {
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the change log (for testing/observation).
    pub fn change_log(&self) -> &[SecretStorageChangeEvent] {
        &self.change_log
    }

    pub fn has(&self, key: &str) -> bool {
        self.secrets.contains_key(key)
    }

    pub fn clear(&mut self) {
        self.secrets.clear();
    }

    pub fn key_count(&self) -> usize {
        self.secrets.len()
    }

    /// Return all entries as `SecretEntry` values, sorted by key.
    pub fn entries(&self) -> Vec<SecretEntry> {
        let mut entries: Vec<SecretEntry> = self
            .secrets
            .iter()
            .map(|(k, v)| SecretEntry {
                key: k.clone(),
                value: v.clone(),
            })
            .collect();
        entries.sort_by(|a, b| a.key.cmp(&b.key));
        entries
    }

    /// Store a pre-validated `SecretEntry`.
    pub fn store_entry(&mut self, entry: &SecretEntry) -> Result<(), SecretStorageError> {
        self.store(&entry.key, &entry.value)
    }

    /// Return only the change events matching the given kind.
    pub fn changes_of_kind(&self, kind: SecretChangeKind) -> Vec<&SecretStorageChangeEvent> {
        self.change_log.iter().filter(|e| e.kind == kind).collect()
    }

    /// Merge all entries from another storage into this one.
    pub fn merge_from(&mut self, other: &InMemorySecretStorage) -> Result<(), SecretStorageError> {
        for (key, value) in &other.secrets {
            self.store(key, value)?;
        }
        Ok(())
    }
}

impl SecretStorage for InMemorySecretStorage {
    fn get(&self, key: &str) -> Option<String> {
        self.secrets.get(key).cloned()
    }

    fn store(&mut self, key: &str, value: &str) -> Result<(), SecretStorageError> {
        let kind = if self.secrets.contains_key(key) {
            SecretChangeKind::Updated
        } else {
            SecretChangeKind::Added
        };
        self.secrets.insert(key.to_string(), value.to_string());
        self.change_log.push(SecretStorageChangeEvent {
            key: key.to_string(),
            kind,
        });
        Ok(())
    }

    fn delete(&mut self, key: &str) -> Result<bool, SecretStorageError> {
        let removed = self.secrets.remove(key).is_some();
        if removed {
            self.change_log.push(SecretStorageChangeEvent {
                key: key.to_string(),
                kind: SecretChangeKind::Deleted,
            });
        }
        Ok(removed)
    }

    fn keys(&self) -> Vec<String> {
        let mut keys: Vec<String> = self.secrets.keys().cloned().collect();
        keys.sort();
        keys
    }
}

/// Backward-compatible alias.
pub type SecretStorageService = InMemorySecretStorage;

// ---------------------------------------------------------------------------
// EncryptedFileStore
// ---------------------------------------------------------------------------

use serde::{Serialize, Deserialize};

/// XOR-based obfuscation + base64 encoding for simple file-based secret storage.
fn xor_encrypt(data: &[u8], key: &[u8]) -> Vec<u8> {
    data.iter()
        .enumerate()
        .map(|(i, b)| b ^ key[i % key.len()])
        .collect()
}

fn base64_encode(data: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::new();
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;
        result.push(CHARS[((triple >> 18) & 0x3F) as usize] as char);
        result.push(CHARS[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            result.push(CHARS[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
        if chunk.len() > 2 {
            result.push(CHARS[(triple & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
    }
    result
}

fn base64_decode(s: &str) -> Result<Vec<u8>, String> {
    fn char_val(c: u8) -> Result<u8, String> {
        match c {
            b'A'..=b'Z' => Ok(c - b'A'),
            b'a'..=b'z' => Ok(c - b'a' + 26),
            b'0'..=b'9' => Ok(c - b'0' + 52),
            b'+' => Ok(62),
            b'/' => Ok(63),
            _ => Err(format!("invalid base64 character: {}", c as char)),
        }
    }
    let bytes: Vec<u8> = s.bytes().filter(|&b| b != b'=').collect();
    let mut result = Vec::new();
    for chunk in bytes.chunks(4) {
        if chunk.is_empty() { break; }
        let vals: Vec<u8> = chunk.iter().map(|&b| char_val(b)).collect::<Result<_, _>>()?;
        let n = vals.len();
        if n >= 2 {
            result.push((vals[0] << 2) | (vals[1] >> 4));
        }
        if n >= 3 {
            result.push((vals[1] << 4) | (vals[2] >> 2));
        }
        if n >= 4 {
            result.push((vals[2] << 6) | vals[3]);
        }
    }
    Ok(result)
}

/// Serializable on-disk format for the encrypted secrets file.
#[derive(Serialize, Deserialize, Default)]
struct SecretsFile {
    secrets: HashMap<String, String>,
}

/// File-based encrypted secret storage.
///
/// Stores secrets in a JSON file with XOR + base64 obfuscation.
/// This is *not* cryptographically secure — it is a functional fallback
/// when no system keyring is available.
#[derive(Debug)]
pub struct EncryptedFileStore {
    path: std::path::PathBuf,
    key: Vec<u8>,
    cache: HashMap<String, String>,
}

impl EncryptedFileStore {
    /// Create a new file store at `path` using `user_key` for XOR encryption.
    pub fn new(path: impl Into<std::path::PathBuf>, user_key: &str) -> Self {
        let path = path.into();
        let key = if user_key.is_empty() {
            b"vsedit-default-key".to_vec()
        } else {
            user_key.as_bytes().to_vec()
        };
        let mut store = Self {
            path,
            key,
            cache: HashMap::new(),
        };
        store.load_from_disk();
        store
    }

    /// Default path: `~/.config/vsedit/secrets.json`.
    pub fn default_path() -> Option<std::path::PathBuf> {
        dirs::config_dir().map(|d| d.join("vsedit").join("secrets.json"))
    }

    fn load_from_disk(&mut self) {
        if let Ok(contents) = std::fs::read_to_string(&self.path) {
            if let Ok(file) = serde_json::from_str::<SecretsFile>(&contents) {
                self.cache.clear();
                for (k, encoded) in &file.secrets {
                    if let Ok(encrypted) = base64_decode(encoded) {
                        let decrypted = xor_encrypt(&encrypted, &self.key);
                        if let Ok(value) = String::from_utf8(decrypted) {
                            self.cache.insert(k.clone(), value);
                        }
                    }
                }
            }
        }
    }

    fn save_to_disk(&self) -> Result<(), SecretStorageError> {
        let mut file = SecretsFile::default();
        for (k, v) in &self.cache {
            let encrypted = xor_encrypt(v.as_bytes(), &self.key);
            file.secrets.insert(k.clone(), base64_encode(&encrypted));
        }
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                SecretStorageError::Other(format!("cannot create directory: {e}"))
            })?;
        }
        let json = serde_json::to_string_pretty(&file)
            .map_err(|e| SecretStorageError::Other(format!("serialization error: {e}")))?;
        std::fs::write(&self.path, json)
            .map_err(|e| SecretStorageError::Other(format!("write error: {e}")))?;
        Ok(())
    }

    /// Return the file path used by this store.
    pub fn path(&self) -> &std::path::Path {
        &self.path
    }
}

impl SecretStorage for EncryptedFileStore {
    fn get(&self, key: &str) -> Option<String> {
        self.cache.get(key).cloned()
    }

    fn store(&mut self, key: &str, value: &str) -> Result<(), SecretStorageError> {
        validate_key(key)?;
        self.cache.insert(key.to_string(), value.to_string());
        self.save_to_disk()
    }

    fn delete(&mut self, key: &str) -> Result<bool, SecretStorageError> {
        let removed = self.cache.remove(key).is_some();
        if removed {
            self.save_to_disk()?;
        }
        Ok(removed)
    }

    fn keys(&self) -> Vec<String> {
        let mut keys: Vec<String> = self.cache.keys().cloned().collect();
        keys.sort();
        keys
    }
}

// ---------------------------------------------------------------------------
// SecretService
// ---------------------------------------------------------------------------

/// High-level secret service that tries keyring first, then falls back to
/// file-based encrypted storage.
#[derive(Debug)]
pub struct SecretService {
    keyring: KeyringSecretStorage,
    file_store: EncryptedFileStore,
}

impl SecretService {
    /// Create a new `SecretService`.
    pub fn new(
        service_name: &str,
        file_path: impl Into<std::path::PathBuf>,
        user_key: &str,
    ) -> Self {
        Self {
            keyring: KeyringSecretStorage::new(service_name),
            file_store: EncryptedFileStore::new(file_path, user_key),
        }
    }

    /// Store a secret, trying keyring first.
    pub fn store_secret(&mut self, key: &str, value: &str) -> Result<(), SecretStorageError> {
        validate_key(key)?;
        if self.keyring.is_keyring_available() {
            self.keyring.store(key, value)
        } else {
            self.file_store.store(key, value)
        }
    }

    /// Retrieve a secret, trying keyring first.
    pub fn get_secret(&self, key: &str) -> Option<String> {
        if self.keyring.is_keyring_available() {
            self.keyring.get(key)
        } else {
            self.file_store.get(key)
        }
    }

    /// Delete a secret, trying keyring first.
    pub fn delete_secret(&mut self, key: &str) -> Result<bool, SecretStorageError> {
        if self.keyring.is_keyring_available() {
            self.keyring.delete(key)
        } else {
            self.file_store.delete(key)
        }
    }

    /// List all keys from the active backend.
    pub fn list_keys(&self) -> Vec<String> {
        if self.keyring.is_keyring_available() {
            self.keyring.keys()
        } else {
            self.file_store.keys()
        }
    }

    /// Whether the keyring backend is being used.
    pub fn is_using_keyring(&self) -> bool {
        self.keyring.is_keyring_available()
    }
}

/// Keyring-backed secret storage stub.
///
/// In a real implementation this would wrap the system keyring (e.g., via
/// the `keyring` crate). For now it delegates to in-memory storage and
/// records that keyring integration was requested.
#[derive(Debug)]
pub struct KeyringSecretStorage {
    service_name: String,
    inner: InMemorySecretStorage,
    keyring_available: bool,
}

impl KeyringSecretStorage {
    pub fn new(service_name: impl Into<String>) -> Self {
        Self {
            service_name: service_name.into(),
            inner: InMemorySecretStorage::new(),
            keyring_available: false, // stub: keyring not actually available
        }
    }

    pub fn service_name(&self) -> &str {
        &self.service_name
    }

    /// Whether the system keyring is available.
    pub fn is_keyring_available(&self) -> bool {
        self.keyring_available
    }
}

impl SecretStorage for KeyringSecretStorage {
    fn get(&self, key: &str) -> Option<String> {
        self.inner.get(key)
    }

    fn store(&mut self, key: &str, value: &str) -> Result<(), SecretStorageError> {
        if !self.keyring_available {
            // Fall back to in-memory
            self.inner.store(key, value)
        } else {
            Err(SecretStorageError::BackendUnavailable(
                "Keyring integration not yet implemented".into(),
            ))
        }
    }

    fn delete(&mut self, key: &str) -> Result<bool, SecretStorageError> {
        self.inner.delete(key)
    }

    fn keys(&self) -> Vec<String> {
        self.inner.keys()
    }
}

/// A scoped wrapper that prefixes all keys with a namespace.
///
/// Useful for isolating secrets belonging to different extensions or modules
/// within a shared storage backend.
#[derive(Debug)]
pub struct ScopedSecretStorage {
    prefix: String,
    inner: InMemorySecretStorage,
}

impl ScopedSecretStorage {
    /// Create a new scoped storage with the given namespace prefix.
    pub fn new(prefix: impl Into<String>) -> Self {
        Self {
            prefix: prefix.into(),
            inner: InMemorySecretStorage::new(),
        }
    }

    /// Returns the prefix used for scoping keys.
    pub fn prefix(&self) -> &str {
        &self.prefix
    }

    fn scoped_key(&self, key: &str) -> String {
        format!("{}.{}", self.prefix, key)
    }
}

impl SecretStorage for ScopedSecretStorage {
    fn get(&self, key: &str) -> Option<String> {
        self.inner.get(&self.scoped_key(key))
    }

    fn store(&mut self, key: &str, value: &str) -> Result<(), SecretStorageError> {
        let scoped = self.scoped_key(key);
        self.inner.store(&scoped, value)
    }

    fn delete(&mut self, key: &str) -> Result<bool, SecretStorageError> {
        let scoped = self.scoped_key(key);
        self.inner.delete(&scoped)
    }

    fn keys(&self) -> Vec<String> {
        self.inner
            .keys()
            .into_iter()
            .filter_map(|k| k.strip_prefix(&format!("{}.", self.prefix)).map(String::from))
            .collect()
    }
}

/// Accumulated statistics for secrets operations.
#[derive(Debug, Clone, PartialEq)]
pub struct SecretsStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl SecretsStats {
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
    pub fn merge(&mut self, other: &SecretsStats) {
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

impl Default for SecretsStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for SecretsStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "SecretsStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for secrets.
#[derive(Debug, Clone)]
pub struct SecretsValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl SecretsValidator {
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

impl Default for SecretsValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Secret masking
// ---------------------------------------------------------------------------

/// Masks known secret values in arbitrary text output.
pub struct SecretMasker {
    patterns: Vec<(String, String)>,
}

impl SecretMasker {
    /// Create a new masker. Each pattern is `(secret_value, replacement)`.
    pub fn new() -> Self {
        Self {
            patterns: Vec::new(),
        }
    }

    /// Register a secret value to be masked.
    pub fn add_secret(&mut self, value: &str) {
        if !value.is_empty() {
            self.patterns.push((value.to_string(), "***".to_string()));
        }
    }

    /// Register a secret with a custom replacement label.
    pub fn add_secret_with_label(&mut self, value: &str, label: &str) {
        if !value.is_empty() {
            self.patterns
                .push((value.to_string(), format!("[{}]", label)));
        }
    }

    /// Mask all registered secrets in the given text.
    pub fn mask(&self, text: &str) -> String {
        let mut result = text.to_string();
        for (secret, replacement) in &self.patterns {
            result = result.replace(secret.as_str(), replacement);
        }
        result
    }

    /// Number of registered secrets.
    pub fn secret_count(&self) -> usize {
        self.patterns.len()
    }
}

impl Default for SecretMasker {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Secret expiration tracking
// ---------------------------------------------------------------------------

/// Tracks when secrets expire so they can be rotated.
#[derive(Debug, Clone)]
pub struct SecretExpiration {
    pub key: String,
    /// Expiration timestamp (seconds since epoch).
    pub expires_at: u64,
}

/// Manages a collection of secret expiration records.
#[derive(Debug, Clone, Default)]
pub struct ExpirationTracker {
    records: Vec<SecretExpiration>,
}

impl ExpirationTracker {
    /// Create a new tracker.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the expiration for a secret key.
    pub fn set_expiration(&mut self, key: &str, expires_at: u64) {
        if let Some(rec) = self.records.iter_mut().find(|r| r.key == key) {
            rec.expires_at = expires_at;
        } else {
            self.records.push(SecretExpiration {
                key: key.to_string(),
                expires_at,
            });
        }
    }

    /// Remove expiration tracking for a key.
    pub fn remove(&mut self, key: &str) {
        self.records.retain(|r| r.key != key);
    }

    /// Get all secrets that have expired as of `now`.
    pub fn expired_keys(&self, now: u64) -> Vec<&str> {
        self.records
            .iter()
            .filter(|r| r.expires_at <= now)
            .map(|r| r.key.as_str())
            .collect()
    }

    /// Get all secrets expiring within `window` seconds of `now`.
    pub fn expiring_soon(&self, now: u64, window: u64) -> Vec<&str> {
        self.records
            .iter()
            .filter(|r| r.expires_at > now && r.expires_at <= now + window)
            .map(|r| r.key.as_str())
            .collect()
    }

    /// Number of tracked expirations.
    pub fn count(&self) -> usize {
        self.records.len()
    }
}

// ---------------------------------------------------------------------------
// Secret access auditing
// ---------------------------------------------------------------------------

/// Records an access event for auditing purposes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretAccessEvent {
    pub key: String,
    pub action: SecretAccessAction,
    pub timestamp: u64,
    pub caller: String,
}

/// The kind of access performed on a secret.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretAccessAction {
    Read,
    Write,
    Delete,
}

impl fmt::Display for SecretAccessAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read => write!(f, "read"),
            Self::Write => write!(f, "write"),
            Self::Delete => write!(f, "delete"),
        }
    }
}

/// Collects secret access events for auditing.
#[derive(Debug, Clone, Default)]
pub struct SecretAuditLog {
    events: Vec<SecretAccessEvent>,
}

impl SecretAuditLog {
    /// Create a new empty audit log.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record an access event.
    pub fn record(&mut self, key: &str, action: SecretAccessAction, timestamp: u64, caller: &str) {
        self.events.push(SecretAccessEvent {
            key: key.to_string(),
            action,
            timestamp,
            caller: caller.to_string(),
        });
    }

    /// Get all events for a specific key.
    pub fn events_for_key(&self, key: &str) -> Vec<&SecretAccessEvent> {
        self.events.iter().filter(|e| e.key == key).collect()
    }

    /// Get the most recent access event for a key.
    pub fn last_access(&self, key: &str) -> Option<&SecretAccessEvent> {
        self.events.iter().rev().find(|e| e.key == key)
    }

    /// Total number of events recorded.
    pub fn event_count(&self) -> usize {
        self.events.len()
    }

    /// Get all unique keys that have been accessed.
    pub fn accessed_keys(&self) -> Vec<&str> {
        let mut keys: Vec<&str> = self.events.iter().map(|e| e.key.as_str()).collect();
        keys.sort();
        keys.dedup();
        keys
    }
}

// ---------------------------------------------------------------------------
// SecretRotator — manage secret rotation
// ---------------------------------------------------------------------------

/// Tracks secrets that need rotation and generates rotation plans.
#[derive(Debug, Clone, Default)]
pub struct SecretRotator {
    /// Mapping from key to the number of times the secret has been rotated.
    rotation_counts: HashMap<String, u32>,
    /// Maximum age in seconds before a secret should be rotated.
    max_age_seconds: u64,
}

impl SecretRotator {
    /// Create a new rotator with a given max age policy.
    pub fn new(max_age_seconds: u64) -> Self {
        Self {
            rotation_counts: HashMap::new(),
            max_age_seconds,
        }
    }

    /// Record that a secret was rotated.
    pub fn record_rotation(&mut self, key: &str) {
        *self.rotation_counts.entry(key.to_string()).or_insert(0) += 1;
    }

    /// Get the number of times a secret has been rotated.
    pub fn rotation_count(&self, key: &str) -> u32 {
        self.rotation_counts.get(key).copied().unwrap_or(0)
    }

    /// Given expiration records and a current time, return keys that need rotation.
    pub fn needs_rotation<'a>(&self, tracker: &'a ExpirationTracker, now: u64) -> Vec<&'a str> {
        tracker.expired_keys(now)
    }

    /// Return keys expiring within the configured max_age_seconds window.
    pub fn expiring_within_policy<'a>(&self, tracker: &'a ExpirationTracker, now: u64) -> Vec<&'a str> {
        tracker.expiring_soon(now, self.max_age_seconds)
    }

    /// Total number of rotations across all keys.
    pub fn total_rotations(&self) -> u32 {
        self.rotation_counts.values().sum()
    }

    /// Get the max age policy in seconds.
    pub fn max_age(&self) -> u64 {
        self.max_age_seconds
    }
}

// ---------------------------------------------------------------------------
// SecretAuditLog — additional methods
// ---------------------------------------------------------------------------

impl SecretAuditLog {
    /// Return events for a given action type.
    pub fn events_by_action(&self, action: SecretAccessAction) -> Vec<&SecretAccessEvent> {
        self.events.iter().filter(|e| e.action == action).collect()
    }

    /// Return events within a time range [start, end] inclusive.
    pub fn events_in_range(&self, start: u64, end: u64) -> Vec<&SecretAccessEvent> {
        self.events
            .iter()
            .filter(|e| e.timestamp >= start && e.timestamp <= end)
            .collect()
    }

    /// Number of unique callers that have accessed secrets.
    pub fn unique_callers(&self) -> Vec<&str> {
        let mut callers: Vec<&str> = self.events.iter().map(|e| e.caller.as_str()).collect();
        callers.sort();
        callers.dedup();
        callers
    }

    /// Clear all recorded events.
    pub fn clear(&mut self) {
        self.events.clear();
    }
}

// ---------------------------------------------------------------------------
// InMemorySecretStorage — additional methods
// ---------------------------------------------------------------------------

impl InMemorySecretStorage {
    /// Return keys matching a prefix.
    pub fn keys_with_prefix(&self, prefix: &str) -> Vec<String> {
        self.secrets
            .keys()
            .filter(|k| k.starts_with(prefix))
            .cloned()
            .collect()
    }

    /// Rename a key. Returns error if the source key doesn't exist.
    pub fn rename_key(&mut self, old_key: &str, new_key: &str) -> Result<(), SecretStorageError> {
        validate_key(new_key)?;
        let value = self
            .secrets
            .remove(old_key)
            .ok_or_else(|| SecretStorageError::InvalidKey(format!("key '{}' not found", old_key)))?;
        self.secrets.insert(new_key.to_string(), value);
        self.change_log.push(SecretStorageChangeEvent {
            key: old_key.to_string(),
            kind: SecretChangeKind::Deleted,
        });
        self.change_log.push(SecretStorageChangeEvent {
            key: new_key.to_string(),
            kind: SecretChangeKind::Added,
        });
        Ok(())
    }

    /// Return the total size in bytes of all stored secret values.
    pub fn total_value_bytes(&self) -> usize {
        self.secrets.values().map(|v| v.len()).sum()
    }

    /// Delete all keys matching a prefix. Returns the number of keys removed.
    pub fn delete_with_prefix(&mut self, prefix: &str) -> usize {
        let matching: Vec<String> = self
            .secrets
            .keys()
            .filter(|k| k.starts_with(prefix))
            .cloned()
            .collect();
        let count = matching.len();
        for key in matching {
            self.secrets.remove(&key);
            self.change_log.push(SecretStorageChangeEvent {
                key,
                kind: SecretChangeKind::Deleted,
            });
        }
        count
    }

    /// Export all keys and their redacted values as a summary report.
    pub fn redacted_summary(&self) -> Vec<String> {
        let mut entries: Vec<_> = self.secrets.iter().collect();
        entries.sort_by_key(|(k, _)| k.clone());
        entries
            .into_iter()
            .map(|(k, v)| {
                let entry = SecretEntry {
                    key: k.clone(),
                    value: v.clone(),
                };
                entry.to_string()
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Secret value strength validation
// ---------------------------------------------------------------------------

/// Strength rating for a secret value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SecretStrength {
    Weak,
    Fair,
    Strong,
}

impl fmt::Display for SecretStrength {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Weak => write!(f, "weak"),
            Self::Fair => write!(f, "fair"),
            Self::Strong => write!(f, "strong"),
        }
    }
}

/// Evaluate the strength of a secret value based on length and character diversity.
pub fn evaluate_secret_strength(value: &str) -> SecretStrength {
    if value.len() < 8 {
        return SecretStrength::Weak;
    }
    let has_upper = value.chars().any(|c| c.is_ascii_uppercase());
    let has_lower = value.chars().any(|c| c.is_ascii_lowercase());
    let has_digit = value.chars().any(|c| c.is_ascii_digit());
    let has_special = value.chars().any(|c| !c.is_alphanumeric());
    let categories = [has_upper, has_lower, has_digit, has_special]
        .iter()
        .filter(|&&b| b)
        .count();
    if value.len() >= 16 && categories >= 3 {
        SecretStrength::Strong
    } else if value.len() >= 8 && categories >= 2 {
        SecretStrength::Fair
    } else {
        SecretStrength::Weak
    }
}

// ---------------------------------------------------------------------------
// Secret size policy
// ---------------------------------------------------------------------------

/// Enforces size limits on secret values.
#[derive(Debug, Clone)]
pub struct SecretSizePolicy {
    max_value_bytes: usize,
    max_total_bytes: usize,
}

impl SecretSizePolicy {
    /// Create a policy with given per-value and total limits.
    pub fn new(max_value_bytes: usize, max_total_bytes: usize) -> Self {
        Self {
            max_value_bytes,
            max_total_bytes,
        }
    }

    /// Check whether a single value exceeds the per-value limit.
    pub fn check_value(&self, value: &str) -> Result<(), SecretStorageError> {
        if value.len() > self.max_value_bytes {
            return Err(SecretStorageError::Other(format!(
                "secret value size {} exceeds maximum {}",
                value.len(),
                self.max_value_bytes
            )));
        }
        Ok(())
    }

    /// Check whether adding `new_value_bytes` would exceed the total storage limit
    /// given the current usage.
    pub fn check_total(
        &self,
        current_total_bytes: usize,
        new_value_bytes: usize,
    ) -> Result<(), SecretStorageError> {
        let projected = current_total_bytes.saturating_add(new_value_bytes);
        if projected > self.max_total_bytes {
            return Err(SecretStorageError::Other(format!(
                "total storage {} would exceed maximum {}",
                projected, self.max_total_bytes
            )));
        }
        Ok(())
    }

    /// Maximum bytes for a single value.
    pub fn max_value_bytes(&self) -> usize {
        self.max_value_bytes
    }

    /// Maximum total bytes across all values.
    pub fn max_total_bytes(&self) -> usize {
        self.max_total_bytes
    }
}

impl Default for SecretSizePolicy {
    fn default() -> Self {
        Self::new(64 * 1024, 1024 * 1024) // 64 KiB per value, 1 MiB total
    }
}

// ---------------------------------------------------------------------------
// Secret migration between backends
// ---------------------------------------------------------------------------

/// Migrate all secrets from one storage backend to another.
///
/// Returns the number of secrets migrated. Secrets that already exist in the
/// destination are overwritten.
pub fn migrate_secrets(
    source: &dyn SecretStorage,
    dest: &mut dyn SecretStorage,
) -> Result<usize, SecretStorageError> {
    let keys = source.keys();
    let mut count = 0;
    for key in &keys {
        if let Some(value) = source.get(key) {
            dest.store(key, &value)?;
            count += 1;
        }
    }
    Ok(count)
}

/// Migrate secrets matching a prefix from one backend to another.
pub fn migrate_secrets_with_prefix(
    source: &dyn SecretStorage,
    dest: &mut dyn SecretStorage,
    prefix: &str,
) -> Result<usize, SecretStorageError> {
    let keys = source.keys();
    let mut count = 0;
    for key in &keys {
        if key.starts_with(prefix) {
            if let Some(value) = source.get(key) {
                dest.store(key, &value)?;
                count += 1;
            }
        }
    }
    Ok(count)
}

// ---------------------------------------------------------------------------
// Secret snapshot / diff
// ---------------------------------------------------------------------------

/// A point-in-time snapshot of secret keys and their hashed values.
#[derive(Debug, Clone)]
pub struct SecretSnapshot {
    /// Mapping from key to a simple hash of the value (for change detection, not security).
    entries: HashMap<String, u64>,
    timestamp: u64,
}

impl SecretSnapshot {
    /// Capture a snapshot from a storage backend.
    pub fn capture(storage: &dyn SecretStorage, timestamp: u64) -> Self {
        let keys = storage.keys();
        let mut entries = HashMap::new();
        for key in &keys {
            if let Some(value) = storage.get(key) {
                entries.insert(key.clone(), Self::simple_hash(&value));
            }
        }
        Self { entries, timestamp }
    }

    /// Return the timestamp of this snapshot.
    pub fn timestamp(&self) -> u64 {
        self.timestamp
    }

    /// Number of secrets in the snapshot.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the snapshot is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Compute the diff between two snapshots: keys added, removed, and changed.
    pub fn diff(&self, newer: &SecretSnapshot) -> SecretDiff {
        let mut added = Vec::new();
        let mut removed = Vec::new();
        let mut changed = Vec::new();

        for key in newer.entries.keys() {
            match self.entries.get(key) {
                None => added.push(key.clone()),
                Some(old_hash) => {
                    if newer.entries.get(key) != Some(old_hash) {
                        changed.push(key.clone());
                    }
                }
            }
        }
        for key in self.entries.keys() {
            if !newer.entries.contains_key(key) {
                removed.push(key.clone());
            }
        }

        added.sort();
        removed.sort();
        changed.sort();

        SecretDiff {
            added,
            removed,
            changed,
        }
    }

    fn simple_hash(s: &str) -> u64 {
        let mut h: u64 = 5381;
        for b in s.bytes() {
            h = h.wrapping_mul(33).wrapping_add(b as u64);
        }
        h
    }
}

/// The difference between two secret snapshots.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretDiff {
    pub added: Vec<String>,
    pub removed: Vec<String>,
    pub changed: Vec<String>,
}

impl SecretDiff {
    /// True if no differences exist.
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty() && self.changed.is_empty()
    }

    /// Total number of differences.
    pub fn total(&self) -> usize {
        self.added.len() + self.removed.len() + self.changed.len()
    }
}

impl fmt::Display for SecretDiff {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "SecretDiff(+{} -{} ~{})",
            self.added.len(),
            self.removed.len(),
            self.changed.len()
        )
    }
}


// ---------------------------------------------------------------------------
// SecretsVaultMigrator
// ---------------------------------------------------------------------------

/// Describes a secret storage backend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VaultBackendKind {
    InMemory,
    EncryptedFile,
    Keyring,
    Custom(String),
}

impl fmt::Display for VaultBackendKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InMemory => write!(f, "in-memory"),
            Self::EncryptedFile => write!(f, "encrypted-file"),
            Self::Keyring => write!(f, "keyring"),
            Self::Custom(name) => write!(f, "custom({name})"),
        }
    }
}

/// A record of a single migrated secret.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationRecord {
    pub key: String,
    pub source: VaultBackendKind,
    pub destination: VaultBackendKind,
    pub success: bool,
    pub error_message: Option<String>,
}

/// Migrates secrets between storage backends.
#[derive(Debug)]
pub struct SecretsVaultMigrator {
    records: Vec<MigrationRecord>,
    dry_run: bool,
    overwrite_existing: bool,
    key_filter: Option<String>,
}

impl SecretsVaultMigrator {
    /// Create a new migrator.
    pub fn new() -> Self {
        Self {
            records: Vec::new(),
            dry_run: false,
            overwrite_existing: false,
            key_filter: None,
        }
    }

    /// Enable or disable dry-run mode (no actual migration).
    pub fn set_dry_run(&mut self, enabled: bool) -> &mut Self {
        self.dry_run = enabled;
        self
    }

    /// Allow or disallow overwriting existing secrets in the destination.
    pub fn set_overwrite(&mut self, enabled: bool) -> &mut Self {
        self.overwrite_existing = enabled;
        self
    }

    /// Set a prefix filter: only keys starting with this prefix will be migrated.
    pub fn set_key_filter(&mut self, prefix: &str) -> &mut Self {
        self.key_filter = Some(prefix.to_string());
        self
    }

    /// Whether a given key passes the current filter.
    pub fn key_matches_filter(&self, key: &str) -> bool {
        match &self.key_filter {
            Some(prefix) => key.starts_with(prefix),
            None => true,
        }
    }

    /// Simulate migration of a set of entries from one backend to another.
    pub fn migrate_entries(
        &mut self,
        entries: &[SecretEntry],
        source: VaultBackendKind,
        destination: VaultBackendKind,
        existing_keys: &[String],
    ) -> Vec<MigrationRecord> {
        let mut batch = Vec::new();
        for entry in entries {
            if !self.key_matches_filter(&entry.key) {
                continue;
            }
            let already_exists = existing_keys.contains(&entry.key);
            let record = if already_exists && !self.overwrite_existing {
                MigrationRecord {
                    key: entry.key.clone(),
                    source: source.clone(),
                    destination: destination.clone(),
                    success: false,
                    error_message: Some("key already exists in destination".into()),
                }
            } else if self.dry_run {
                MigrationRecord {
                    key: entry.key.clone(),
                    source: source.clone(),
                    destination: destination.clone(),
                    success: true,
                    error_message: None,
                }
            } else {
                MigrationRecord {
                    key: entry.key.clone(),
                    source: source.clone(),
                    destination: destination.clone(),
                    success: true,
                    error_message: None,
                }
            };
            batch.push(record.clone());
            self.records.push(record);
        }
        batch
    }

    /// Return all migration records.
    pub fn records(&self) -> &[MigrationRecord] {
        &self.records
    }

    /// Count of successful migrations.
    pub fn success_count(&self) -> usize {
        self.records.iter().filter(|r| r.success).count()
    }

    /// Count of failed migrations.
    pub fn failure_count(&self) -> usize {
        self.records.iter().filter(|r| !r.success).count()
    }

    /// Summary string.
    pub fn summary(&self) -> String {
        format!(
            "Migration complete: {} succeeded, {} failed out of {} total",
            self.success_count(),
            self.failure_count(),
            self.records.len(),
        )
    }
}

// ---------------------------------------------------------------------------
// SecretsAccessAuditor
// ---------------------------------------------------------------------------

/// The kind of access operation performed on a secret.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditAccessKind {
    Read,
    Write,
    Delete,
    List,
}

impl fmt::Display for AuditAccessKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read => write!(f, "READ"),
            Self::Write => write!(f, "WRITE"),
            Self::Delete => write!(f, "DELETE"),
            Self::List => write!(f, "LIST"),
        }
    }
}

/// A logged access event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditEvent {
    pub key: String,
    pub kind: AuditAccessKind,
    pub actor: String,
    pub timestamp_epoch_secs: u64,
    pub allowed: bool,
}

/// Audits and logs all secret access events.
#[derive(Debug)]
pub struct SecretsAccessAuditor {
    events: Vec<AuditEvent>,
    denied_actors: Vec<String>,
    max_events: usize,
}

impl SecretsAccessAuditor {
    pub fn new(max_events: usize) -> Self {
        Self {
            events: Vec::new(),
            denied_actors: Vec::new(),
            max_events,
        }
    }

    /// Deny all access from a given actor.
    pub fn deny_actor(&mut self, actor: &str) {
        self.denied_actors.push(actor.to_string());
    }

    /// Returns true if the actor is denied.
    pub fn is_denied(&self, actor: &str) -> bool {
        self.denied_actors.iter().any(|a| a == actor)
    }

    /// Log an access event, automatically checking the deny list.
    pub fn log_access(&mut self, key: &str, kind: AuditAccessKind, actor: &str, epoch: u64) -> bool {
        let allowed = !self.is_denied(actor);
        let event = AuditEvent {
            key: key.to_string(),
            kind,
            actor: actor.to_string(),
            timestamp_epoch_secs: epoch,
            allowed,
        };
        if self.events.len() >= self.max_events {
            self.events.remove(0);
        }
        self.events.push(event);
        allowed
    }

    /// Return all logged events.
    pub fn events(&self) -> &[AuditEvent] {
        &self.events
    }

    /// Filter events by actor.
    pub fn events_for_actor(&self, actor: &str) -> Vec<&AuditEvent> {
        self.events.iter().filter(|e| e.actor == actor).collect()
    }

    /// Filter events by key.
    pub fn events_for_key(&self, key: &str) -> Vec<&AuditEvent> {
        self.events.iter().filter(|e| e.key == key).collect()
    }

    /// Count of denied events.
    pub fn denied_count(&self) -> usize {
        self.events.iter().filter(|e| !e.allowed).count()
    }

    /// Summary for auditing purposes.
    pub fn summary(&self) -> String {
        format!(
            "Audit: {} total events, {} denied",
            self.events.len(),
            self.denied_count(),
        )
    }
}



// ---------------------------------------------------------------------------
// secrets – Platform service helpers
// ---------------------------------------------------------------------------

/// Capability flags for platform feature detection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XSecretsCapabilities {
    flags: std::collections::HashSet<String>,
}

impl XSecretsCapabilities {
    pub fn new() -> Self {
        Self { flags: std::collections::HashSet::new() }
    }

    pub fn register(&mut self, cap: impl Into<String>) {
        self.flags.insert(cap.into());
    }

    pub fn has(&self, cap: &str) -> bool {
        self.flags.contains(cap)
    }

    pub fn len(&self) -> usize {
        self.flags.len()
    }

    pub fn is_empty(&self) -> bool {
        self.flags.is_empty()
    }

    /// Return the intersection with another capability set.
    pub fn intersect(&self, other: &Self) -> Self {
        Self {
            flags: self.flags.intersection(&other.flags).cloned().collect(),
        }
    }

    /// Return capabilities present here but not in `other`.
    pub fn diff(&self, other: &Self) -> Self {
        Self {
            flags: self.flags.difference(&other.flags).cloned().collect(),
        }
    }

    pub fn all(&self) -> Vec<&str> {
        self.flags.iter().map(|s| s.as_str()).collect()
    }
}

impl Default for XSecretsCapabilities {
    fn default() -> Self {
        Self::new()
    }
}

/// A simple service registry keyed by name.
#[derive(Debug, Default)]
pub struct XSecretsServiceRegistry {
    services: std::collections::HashMap<String, String>,
}

impl XSecretsServiceRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a service. Returns the previous value if the key was already present.
    pub fn register(&mut self, name: impl Into<String>, descriptor: impl Into<String>) -> Option<String> {
        self.services.insert(name.into(), descriptor.into())
    }

    pub fn get(&self, name: &str) -> Option<&str> {
        self.services.get(name).map(|s| s.as_str())
    }

    pub fn contains(&self, name: &str) -> bool {
        self.services.contains_key(name)
    }

    pub fn remove(&mut self, name: &str) -> Option<String> {
        self.services.remove(name)
    }

    pub fn len(&self) -> usize {
        self.services.len()
    }

    pub fn is_empty(&self) -> bool {
        self.services.is_empty()
    }

    pub fn names(&self) -> Vec<&str> {
        self.services.keys().map(|s| s.as_str()).collect()
    }
}

/// Sanitize a path-like string by collapsing repeated separators and removing trailing ones.
pub fn x_secrets_sanitize_path(p: &str) -> String {
    let mut result = String::with_capacity(p.len());
    let mut last_was_sep = false;
    for ch in p.chars() {
        if ch == '/' || ch == '\\' {
            if !last_was_sep {
                result.push('/');
            }
            last_was_sep = true;
        } else {
            result.push(ch);
            last_was_sep = false;
        }
    }
    if result.len() > 1 && result.ends_with('/') {
        result.pop();
    }
    result
}


/// Configuration manager for secrets functionality.
pub struct SecretsConfig {
    options: HashMap<String, String>,
    enabled: bool,
    version: u32,
}

impl SecretsConfig {
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

    pub fn merge(&mut self, other: &SecretsConfig) {
        for (k, v) in &other.options {
            self.options.insert(k.clone(), v.clone());
        }
    }
}

/// Rate tracker for secrets operations.
pub struct SecretsRateTracker {
    window_ms: u64,
    timestamps: Vec<u64>,
}

impl SecretsRateTracker {
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

/// Validation result collector for secrets.
pub struct SecretsValidationCollector {
    errors: Vec<String>,
    warnings: Vec<String>,
}

impl SecretsValidationCollector {
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

    pub fn merge(&mut self, other: &SecretsValidationCollector) {
        self.errors.extend(other.errors.iter().cloned());
        self.warnings.extend(other.warnings.iter().cloned());
    }

    pub fn first_error(&self) -> Option<&str> {
        self.errors.first().map(|s| s.as_str())
    }
}


// ---------------------------------------------------------------------------
// Secret storage and retrieval — extended utilities (xl)
// ---------------------------------------------------------------------------

/// Metric accumulator for secrets operations.
#[derive(Debug, Clone)]
pub struct XlMetrics {
    samples: Vec<f64>,
    label: String,
}

impl XlMetrics {
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

/// Sliding-window rate counter for secrets.
#[derive(Debug, Clone)]
pub struct XlRateWindow {
    timestamps: Vec<u64>,
    window_ms: u64,
}

impl XlRateWindow {
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

/// A small LRU-style cache for secrets lookups.
#[derive(Debug, Clone)]
pub struct XlLruCache {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl XlLruCache {
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
// xb_ utilities – batch 30
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer30 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer30 {
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
pub fn xb_fnv1a_30(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_30<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
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
pub fn xb_rle_decode_30<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_30(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_30(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 156
// ---------------------------------------------------------------------------

/// Generic object pool `Xc156Pool<T>`.
pub struct Xc156Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc156Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc156PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc156Pool<T> {
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
    pub fn stats(&self) -> Xc156PoolStats {
        Xc156PoolStats {
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

impl<T> Default for Xc156Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc156Scheduler`.
pub struct Xc156Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc156Scheduler {
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

impl Default for Xc156Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_156 hash for the given byte slice.
pub fn xc_156_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_156 convention.
pub fn xc_156_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe42 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe42Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe42PipelineError {
    pub stage: Xe42Stage,
    pub message: String,
}

impl std::fmt::Display for Xe42PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe42Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe42Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe42PipelineError>>>,
    stage_names: Vec<Xe42Stage>,
}

impl Xe42Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe42PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe42Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe42PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe42Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe42PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe42Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe42PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe42Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe42PipelineError> {
        let mut data = input;
        for (i, stage_fn) in self.stages.iter().enumerate() {
            data = stage_fn(data).map_err(|mut e| {
                e.stage = self.stage_names[i].clone();
                e
            })?;
        }
        Ok(data)
    }

    pub fn stage_count(&self) -> usize {
        self.stages.len()
    }

    pub fn compose(mut self, other: Xe42Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe42CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe42CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe42Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe42CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe42CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe42Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe42CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_42_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe42CacheEntry {
            value,
            inserted_at: self.current_time,
            ttl,
        });
    }

    pub fn get(&mut self, key: &K) -> Option<V> {
        let now = self.current_time;
        if let Some(entry) = self.entries.get(key) {
            if now - entry.inserted_at < entry.ttl {
                self.stats.hits += 1;
                return Some(entry.value.clone());
            } else {
                self.stats.misses += 1;
                let key_clone = key.clone();
                self.entries.remove(&key_clone);
                return None;
            }
        }
        self.stats.misses += 1;
        None
    }

    pub fn evict(&mut self, key: &K) -> bool {
        if self.entries.remove(key).is_some() {
            self.stats.evictions += 1;
            true
        } else {
            false
        }
    }

    fn xe_42_evict_expired(&mut self) {
        let now = self.current_time;
        let expired: Vec<K> = self.entries.iter()
            .filter(|(_, e)| now - e.inserted_at >= e.ttl)
            .map(|(k, _)| k.clone())
            .collect();
        for k in &expired {
            self.entries.remove(k);
            self.stats.evictions += 1;
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn stats(&self) -> &Xe42CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_42_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe42PipelineError> {
    Ok(data)
}

pub fn xe_42_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe42PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_42_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe42PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_42_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe42PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_42_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe42PipelineError> {
    Err(Xe42PipelineError {
        stage: Xe42Stage::Parse,
        message: "intentional failure".to_string(),
    })
}


// ---------------------------------------------------------------------------
// xg_10: Directed graph
// ---------------------------------------------------------------------------

/// A directed graph with adjacency-list representation.
#[derive(Debug, Clone)]
pub struct Xg10Graph {
    adj: std::collections::HashMap<usize, Vec<usize>>,
    edge_cnt: usize,
}

impl Xg10Graph {
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

impl Default for Xg10Graph {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// xg_10: Min-heap
// ---------------------------------------------------------------------------

/// A min-heap backed by a `Vec`.
#[derive(Debug, Clone)]
pub struct Xg10Heap<T: Ord> {
    data: Vec<T>,
}

impl<T: Ord> Xg10Heap<T> {
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
    pub fn merge(&mut self, other: &mut Xg10Heap<T>) {
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

impl<T: Ord> Default for Xg10Heap<T> {
    fn default() -> Self { Self::new() }
}


/// A probabilistic sorted list using a skip-list structure (variant 155).
pub struct Xh155SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh155SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 197 as u64,
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

/// A compact bit set supporting boolean operations (variant 155).
pub struct Xh155BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh155BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 155).
pub struct Xi155Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi155Deque<T> {
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
pub struct Xi155Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi155Interval {
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

/// A simple interval tree (variant 155).
pub struct Xi155IntervalTree {
    xi_intervals: Vec<Xi155Interval>,
}

impl Xi155IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi155Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi155Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi155Interval) -> Vec<&Xi155Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi155Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi155Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi155Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi155Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi155Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi155Interval> = Vec::new();
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


// --- xj_ Union-Find and B-Tree (crate index 155) ---

/// Disjoint set / union-find for crate 155.
pub struct Xj155UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj155UnionFind {
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

const XJ155_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 155.
pub struct Xj155BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj155BTreeNode<K, V>>>,
    len: usize,
}

struct Xj155BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj155BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj155BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ155_BTREE_ORDER - 1
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
        let mid = XJ155_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj155BTreeNode::xj_new_leaf();
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

impl<K: Ord + Clone, V: Clone> Xj155BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj155BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj155BTreeNode::xj_new_leaf();
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


// --- xk_155 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk155SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk155SegmentTree {
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
pub struct Xk155DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk155DisjointIntervals {
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


/// Rope data structure for efficient large text manipulation (xl_155).
#[derive(Debug, Clone)]
pub struct Xl155Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl155Rope {
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

/// Suffix array for efficient string searching (xl_155).
#[derive(Debug, Clone)]
pub struct Xl155SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl155SuffixArray {
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
pub struct Xm155MatrixSparse {
    rows: usize,
    cols: usize,
    entries: Vec<(usize, usize, f64)>,
}

impl Xm155MatrixSparse {
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
pub struct Xm155Tokenizer {
    text: String,
}

impl Xm155Tokenizer {
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


/// Fenwick (Binary Indexed) tree for prefix‑sum queries — crate 155.
pub struct Xn155Fenwick {
    xn_tree: Vec<i64>,
    xn_n: usize,
}

impl Xn155Fenwick {
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

// ----- AVL tree map — crate 155 -----

#[derive(Debug, Clone)]
struct Xn155AvlNode<K, V> {
    key: K,
    value: V,
    left: Option<Box<Xn155AvlNode<K, V>>>,
    right: Option<Box<Xn155AvlNode<K, V>>>,
    height: i32,
}

/// Self‑balancing AVL tree map — crate 155.
#[derive(Debug, Clone)]
pub struct Xn155AVL<K, V> {
    root: Option<Box<Xn155AvlNode<K, V>>>,
    xn_len: usize,
}

impl<K: Ord + Clone, V: Clone> Default for Xn155AVL<K, V> {
    fn default() -> Self {
        Self::xn_new()
    }
}

impl<K: Ord + Clone, V: Clone> Xn155AVL<K, V> {
    pub fn xn_new() -> Self {
        Self { root: None, xn_len: 0 }
    }

    fn xn_node_height(node: &Option<Box<Xn155AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| n.height)
    }

    fn xn_balance(node: &Option<Box<Xn155AvlNode<K, V>>>) -> i32 {
        node.as_ref().map_or(0, |n| Self::xn_node_height(&n.left) - Self::xn_node_height(&n.right))
    }

    fn xn_update_height(node: &mut Box<Xn155AvlNode<K, V>>) {
        node.height = 1 + std::cmp::max(Self::xn_node_height(&node.left), Self::xn_node_height(&node.right));
    }

    fn xn_rotate_right(mut y: Box<Xn155AvlNode<K, V>>) -> Box<Xn155AvlNode<K, V>> {
        let mut x = y.left.take().expect("xn rotate right");
        y.left = x.right.take();
        Self::xn_update_height(&mut y);
        x.right = Some(y);
        Self::xn_update_height(&mut x);
        x
    }

    fn xn_rotate_left(mut x: Box<Xn155AvlNode<K, V>>) -> Box<Xn155AvlNode<K, V>> {
        let mut y = x.right.take().expect("xn rotate left");
        x.right = y.left.take();
        Self::xn_update_height(&mut x);
        y.left = Some(x);
        Self::xn_update_height(&mut y);
        y
    }

    fn xn_rebalance(mut node: Box<Xn155AvlNode<K, V>>) -> Box<Xn155AvlNode<K, V>> {
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

    fn xn_insert_node(node: Option<Box<Xn155AvlNode<K, V>>>, key: K, value: V, inserted: &mut bool) -> Box<Xn155AvlNode<K, V>> {
        let Some(mut n) = node else {
            *inserted = true;
            return Box::new(Xn155AvlNode { key, value, left: None, right: None, height: 1 });
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

    fn xn_get_node<'a>(node: &'a Option<Box<Xn155AvlNode<K, V>>>, key: &K) -> Option<&'a V> {
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

    fn xn_min_node(node: &Box<Xn155AvlNode<K, V>>) -> &Xn155AvlNode<K, V> {
        node.left.as_ref().map_or(node.as_ref(), |l| Self::xn_min_node(l))
    }

    fn xn_remove_min(mut node: Box<Xn155AvlNode<K, V>>) -> (Box<Xn155AvlNode<K, V>>, Option<Box<Xn155AvlNode<K, V>>>) {
        if node.left.is_none() {
            let right = node.right.take();
            return (node, right);
        }
        let (min, new_left) = Self::xn_remove_min(node.left.take().unwrap());
        node.left = new_left;
        (min, Some(Self::xn_rebalance(node)))
    }

    fn xn_remove_node(node: Option<Box<Xn155AvlNode<K, V>>>, key: &K, removed: &mut bool) -> Option<Box<Xn155AvlNode<K, V>>> {
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

    fn xn_collect_in_order(node: &Option<Box<Xn155AvlNode<K, V>>>, out: &mut Vec<(K, V)>) {
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

    fn xn_min_key(node: &Option<Box<Xn155AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.left.is_some() { Self::xn_min_key(&n.left) } else { Some(&n.key) }
    }

    /// Smallest key in the map.
    pub fn xn_min(&self) -> Option<&K> {
        Self::xn_min_key(&self.root)
    }

    fn xn_max_key(node: &Option<Box<Xn155AvlNode<K, V>>>) -> Option<&K> {
        let n = node.as_ref()?;
        if n.right.is_some() { Self::xn_max_key(&n.right) } else { Some(&n.key) }
    }

    /// Largest key in the map.
    pub fn xn_max(&self) -> Option<&K> {
        Self::xn_max_key(&self.root)
    }

    fn xn_floor_key<'a>(node: &'a Option<Box<Xn155AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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

    fn xn_ceiling_key<'a>(node: &'a Option<Box<Xn155AvlNode<K, V>>>, key: &K) -> Option<&'a K> {
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
// Xo155RedBlack<K,V> — red-black tree map
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Xo155Color {
    Red,
    Black,
}

#[derive(Debug, Clone)]
struct Xo155RBNode<K, V> {
    key: K,
    value: V,
    color: Xo155Color,
    left: Option<Box<Xo155RBNode<K, V>>>,
    right: Option<Box<Xo155RBNode<K, V>>>,
}

/// A red-black tree map for crate 155.
#[derive(Debug, Clone)]
pub struct Xo155RedBlack<K, V> {
    root: Option<Box<Xo155RBNode<K, V>>>,
    len: usize,
}

impl<K: Ord + Clone, V: Clone> Xo155RedBlack<K, V> {
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
            r.color = Xo155Color::Black;
        }
    }

    fn xo_ins(node: Option<Box<Xo155RBNode<K, V>>>, key: K, value: V, len: &mut usize) -> Box<Xo155RBNode<K, V>> {
        match node {
            None => {
                *len += 1;
                Box::new(Xo155RBNode {
                    key, value, color: Xo155Color::Red, left: None, right: None,
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

    fn xo_is_red(node: &Option<Box<Xo155RBNode<K, V>>>) -> bool {
        matches!(node, Some(n) if n.color == Xo155Color::Red)
    }

    fn xo_balance(mut h: Box<Xo155RBNode<K, V>>) -> Box<Xo155RBNode<K, V>> {
        if Self::xo_is_red(&h.right) && !Self::xo_is_red(&h.left) {
            h = Self::xo_rotate_left(h);
        }
        if Self::xo_is_red(&h.left) {
            let left_left_red = h.left.as_ref().and_then(|l| l.left.as_ref()).map_or(false, |ll| ll.color == Xo155Color::Red);
            if left_left_red {
                h = Self::xo_rotate_right(h);
            }
        }
        if Self::xo_is_red(&h.left) && Self::xo_is_red(&h.right) {
            Self::xo_flip_colors(&mut h);
        }
        h
    }

    fn xo_rotate_left(mut h: Box<Xo155RBNode<K, V>>) -> Box<Xo155RBNode<K, V>> {
        let mut x = h.right.take().unwrap();
        h.right = x.left.take();
        x.color = h.color;
        h.color = Xo155Color::Red;
        x.left = Some(h);
        x
    }

    fn xo_rotate_right(mut h: Box<Xo155RBNode<K, V>>) -> Box<Xo155RBNode<K, V>> {
        let mut x = h.left.take().unwrap();
        h.left = x.right.take();
        x.color = h.color;
        h.color = Xo155Color::Red;
        x.right = Some(h);
        x
    }

    fn xo_flip_colors(h: &mut Box<Xo155RBNode<K, V>>) {
        h.color = Xo155Color::Red;
        if let Some(l) = &mut h.left { l.color = Xo155Color::Black; }
        if let Some(r) = &mut h.right { r.color = Xo155Color::Black; }
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
            r.color = Xo155Color::Black;
        }
        if found.is_some() { self.len -= 1; }
        found
    }

    fn xo_remove_rec(node: Option<Box<Xo155RBNode<K, V>>>, key: &K, found: &mut Option<V>) -> Option<Box<Xo155RBNode<K, V>>> {
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

    fn xo_remove_min_node(mut node: Xo155RBNode<K, V>) -> (K, V, Option<Box<Xo155RBNode<K, V>>>) {
        if node.left.is_none() {
            return (node.key, node.value, node.right);
        }
        let (k, v, new_left) = Self::xo_remove_min_node(*node.left.take().unwrap());
        node.left = new_left;
        (k, v, Some(Box::new(node)))
    }

    pub fn xo_black_height(&self) -> usize {
        fn bh<K, V>(node: &Option<Box<Xo155RBNode<K, V>>>) -> usize {
            match node {
                None => 1,
                Some(n) => {
                    let add = if n.color == Xo155Color::Black { 1 } else { 0 };
                    add + bh(&n.left)
                }
            }
        }
        bh(&self.root)
    }

    pub fn xo_in_order(&self) -> Vec<(K, V)> {
        let mut result = Vec::new();
        fn collect<K: Clone, V: Clone>(node: &Option<Box<Xo155RBNode<K, V>>>, out: &mut Vec<(K, V)>) {
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
// Xo155ConsistentHash — consistent hash ring
// ---------------------------------------------------------------------------

/// A consistent hash ring for crate 155.
#[derive(Debug, Clone)]
pub struct Xo155ConsistentHash {
    ring: std::collections::BTreeMap<u64, String>,
    nodes: std::collections::HashMap<String, usize>,
    virtual_count: usize,
}

impl Xo155ConsistentHash {
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
            let vkey = format!("{}#xo155#{}", node, i);
            let hash = Self::xo_hash(&vkey);
            self.ring.insert(hash, node.to_string());
        }
        *self.nodes.entry(node.to_string()).or_insert(0) += 1;
    }

    pub fn xo_remove_node(&mut self, node: &str) {
        let vc = self.virtual_count;
        for i in 0..vc {
            let vkey = format!("{}#xo155#{}", node, i);
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


/// Splay tree data structure keyed by `K` with values `V` (variant 155).
#[derive(Debug)]
pub struct Xp155SplayTree<K: Ord, V> {
    xp_root: Option<Box<Xp155Node<K, V>>>,
    xp_len: usize,
    xp_splay_count: u64,
}

#[derive(Debug)]
struct Xp155Node<K: Ord, V> {
    xp_key: K,
    xp_val: V,
    xp_left: Option<Box<Xp155Node<K, V>>>,
    xp_right: Option<Box<Xp155Node<K, V>>>,
}

impl<K: Ord, V> Xp155Node<K, V> {
    fn xp_new(key: K, val: V) -> Self {
        Self { xp_key: key, xp_val: val, xp_left: None, xp_right: None }
    }

    fn xp_depth(&self) -> usize {
        let ld = self.xp_left.as_ref().map_or(0, |n| n.xp_depth());
        let rd = self.xp_right.as_ref().map_or(0, |n| n.xp_depth());
        1 + ld.max(rd)
    }

    fn xp_min_key(&self) -> &K {
        match &self.xp_left {
            Some(left) => left.xp_min_key(),
            None => &self.xp_key,
        }
    }

    fn xp_max_key(&self) -> &K {
        match &self.xp_right {
            Some(right) => right.xp_max_key(),
            None => &self.xp_key,
        }
    }
}

impl<K: Ord, V> Default for Xp155SplayTree<K, V> {
    fn default() -> Self {
        Self { xp_root: None, xp_len: 0, xp_splay_count: 0 }
    }
}

impl<K: Ord, V> Xp155SplayTree<K, V> {
    /// Creates a new empty splay tree.
    pub fn xp_new() -> Self {
        Self::default()
    }

    /// Returns the number of entries in the tree.
    pub fn xp_len(&self) -> usize {
        self.xp_len
    }

    /// Returns true when empty.
    pub fn xp_is_empty(&self) -> bool {
        self.xp_len == 0
    }

    /// Returns how many splay operations have been performed.
    pub fn xp_splay_count(&self) -> u64 {
        self.xp_splay_count
    }

    /// Returns the depth of the tree.
    pub fn xp_depth(&self) -> usize {
        self.xp_root.as_ref().map_or(0, |n| n.xp_depth())
    }

    /// Returns a reference to the minimum key, if any.
    pub fn xp_min(&self) -> Option<&K> {
        self.xp_root.as_ref().map(|n| n.xp_min_key())
    }

    /// Returns a reference to the maximum key, if any.
    pub fn xp_max(&self) -> Option<&K> {
        self.xp_root.as_ref().map(|n| n.xp_max_key())
    }

    fn xp_splay(&mut self, key: &K) {
        self.xp_splay_count += 1;
        let root = self.xp_root.take();
        self.xp_root = Self::xp_splay_node(root, key);
    }

    fn xp_splay_node(node: Option<Box<Xp155Node<K, V>>>, key: &K) -> Option<Box<Xp155Node<K, V>>> {
        let mut node = node?;
        use std::cmp::Ordering;
        match key.cmp(&node.xp_key) {
            Ordering::Equal => Some(node),
            Ordering::Less => {
                let mut left = match node.xp_left.take() {
                    Some(l) => l,
                    None => { return Some(node); }
                };
                if *key < left.xp_key {
                    left.xp_left = Self::xp_splay_node(left.xp_left.take(), key);
                    node.xp_left = Some(left);
                    node = Self::xp_rotate_right(node);
                } else if *key > left.xp_key {
                    left.xp_right = Self::xp_splay_node(left.xp_right.take(), key);
                    if left.xp_right.is_some() {
                        left = Self::xp_rotate_left(left);
                    }
                    node.xp_left = Some(left);
                } else {
                    node.xp_left = Some(left);
                }
                Some(Self::xp_rotate_right(node))
            }
            Ordering::Greater => {
                let mut right = match node.xp_right.take() {
                    Some(r) => r,
                    None => { return Some(node); }
                };
                if *key > right.xp_key {
                    right.xp_right = Self::xp_splay_node(right.xp_right.take(), key);
                    node.xp_right = Some(right);
                    node = Self::xp_rotate_left(node);
                } else if *key < right.xp_key {
                    right.xp_left = Self::xp_splay_node(right.xp_left.take(), key);
                    if right.xp_left.is_some() {
                        right = Self::xp_rotate_right(right);
                    }
                    node.xp_right = Some(right);
                } else {
                    node.xp_right = Some(right);
                }
                Some(Self::xp_rotate_left(node))
            }
        }
    }

    fn xp_rotate_right(mut node: Box<Xp155Node<K, V>>) -> Box<Xp155Node<K, V>> {
        match node.xp_left.take() {
            Some(mut left) => {
                node.xp_left = left.xp_right.take();
                left.xp_right = Some(node);
                left
            }
            None => node,
        }
    }

    fn xp_rotate_left(mut node: Box<Xp155Node<K, V>>) -> Box<Xp155Node<K, V>> {
        match node.xp_right.take() {
            Some(mut right) => {
                node.xp_right = right.xp_left.take();
                right.xp_left = Some(node);
                right
            }
            None => node,
        }
    }

    /// Inserts a key-value pair. Returns the old value if the key already existed.
    pub fn xp_insert(&mut self, key: K, val: V) -> Option<V> {
        if self.xp_root.is_none() {
            self.xp_root = Some(Box::new(Xp155Node::xp_new(key, val)));
            self.xp_len += 1;
            return None;
        }
        self.xp_splay(&key);
        let root = self.xp_root.as_mut().unwrap();
        use std::cmp::Ordering;
        match key.cmp(&root.xp_key) {
            Ordering::Equal => {
                let old = std::mem::replace(&mut root.xp_val, val);
                Some(old)
            }
            Ordering::Less => {
                let mut new_node = Box::new(Xp155Node::xp_new(key, val));
                new_node.xp_left = root.xp_left.take();
                new_node.xp_right = self.xp_root.take();
                self.xp_root = Some(new_node);
                self.xp_len += 1;
                None
            }
            Ordering::Greater => {
                let mut new_node = Box::new(Xp155Node::xp_new(key, val));
                new_node.xp_right = root.xp_right.take();
                new_node.xp_left = self.xp_root.take();
                self.xp_root = Some(new_node);
                self.xp_len += 1;
                None
            }
        }
    }

    /// Retrieves a reference to the value for the given key, splaying it to root.
    pub fn xp_get(&mut self, key: &K) -> Option<&V> {
        if self.xp_root.is_none() {
            return None;
        }
        self.xp_splay(key);
        let root = self.xp_root.as_ref().unwrap();
        if root.xp_key == *key { Some(&root.xp_val) } else { None }
    }

    /// Removes the entry for `key` and returns its value if present.
    pub fn xp_remove(&mut self, key: &K) -> Option<V> {
        if self.xp_root.is_none() {
            return None;
        }
        self.xp_splay(key);
        let root = self.xp_root.as_ref().unwrap();
        if root.xp_key != *key {
            return None;
        }
        let mut root = self.xp_root.take().unwrap();
        let val = root.xp_val;
        match root.xp_left.take() {
            None => { self.xp_root = root.xp_right.take(); }
            Some(left) => {
                self.xp_root = Some(left);
                self.xp_splay(key);
                self.xp_root.as_mut().unwrap().xp_right = root.xp_right.take();
            }
        }
        self.xp_len -= 1;
        Some(val)
    }
}


// --------------- Xq155Treap ---------------

use std::cmp::Ordering as Xq155Ord;

struct Xq155TreapNode<K, V> {
    key: K,
    value: V,
    priority: u64,
    left: Option<Box<Xq155TreapNode<K, V>>>,
    right: Option<Box<Xq155TreapNode<K, V>>>,
    size: usize,
}

pub struct Xq155Treap<K, V> {
    root: Option<Box<Xq155TreapNode<K, V>>>,
    seed: u64,
}

impl<K, V> Xq155TreapNode<K, V> {
    fn new(key: K, value: V, priority: u64) -> Self {
        Self { key, value, priority, left: None, right: None, size: 1 }
    }
}

fn xq_155_size<K, V>(node: &Option<Box<Xq155TreapNode<K, V>>>) -> usize {
    node.as_ref().map_or(0, |n| n.size)
}

fn xq_155_update_size<K, V>(node: &mut Xq155TreapNode<K, V>) {
    node.size = 1 + xq_155_size(&node.left) + xq_155_size(&node.right);
}

fn xq_155_rotate_right<K, V>(mut node: Box<Xq155TreapNode<K, V>>) -> Box<Xq155TreapNode<K, V>> {
    let mut left = node.left.take().unwrap();
    node.left = left.right.take();
    xq_155_update_size(&mut node);
    left.right = Some(node);
    xq_155_update_size(&mut left);
    left
}

fn xq_155_rotate_left<K, V>(mut node: Box<Xq155TreapNode<K, V>>) -> Box<Xq155TreapNode<K, V>> {
    let mut right = node.right.take().unwrap();
    node.right = right.left.take();
    xq_155_update_size(&mut node);
    right.left = Some(node);
    xq_155_update_size(&mut right);
    right
}

fn xq_155_insert_node<K: Ord, V>(
    node: Option<Box<Xq155TreapNode<K, V>>>,
    key: K,
    value: V,
    priority: u64,
) -> (Option<Box<Xq155TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (Some(Box::new(Xq155TreapNode::new(key, value, priority))), None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq155Ord::Equal => {
                let old = std::mem::replace(&mut n.value, value);
                (Some(n), Some(old))
            }
            Xq155Ord::Less => {
                let (new_left, old) = xq_155_insert_node(n.left.take(), key, value, priority);
                n.left = new_left;
                xq_155_update_size(&mut n);
                if n.left.as_ref().unwrap().priority > n.priority {
                    (Some(xq_155_rotate_right(n)), old)
                } else {
                    (Some(n), old)
                }
            }
            Xq155Ord::Greater => {
                let (new_right, old) = xq_155_insert_node(n.right.take(), key, value, priority);
                n.right = new_right;
                xq_155_update_size(&mut n);
                if n.right.as_ref().unwrap().priority > n.priority {
                    (Some(xq_155_rotate_left(n)), old)
                } else {
                    (Some(n), old)
                }
            }
        },
    }
}

fn xq_155_remove_node<K: Ord, V>(
    node: Option<Box<Xq155TreapNode<K, V>>>,
    key: &K,
) -> (Option<Box<Xq155TreapNode<K, V>>>, Option<V>) {
    match node {
        None => (None, None),
        Some(mut n) => match key.cmp(&n.key) {
            Xq155Ord::Less => {
                let (new_left, old) = xq_155_remove_node(n.left.take(), key);
                n.left = new_left;
                xq_155_update_size(&mut n);
                (Some(n), old)
            }
            Xq155Ord::Greater => {
                let (new_right, old) = xq_155_remove_node(n.right.take(), key);
                n.right = new_right;
                xq_155_update_size(&mut n);
                (Some(n), old)
            }
            Xq155Ord::Equal => {
                let has_left = n.left.is_some();
                let has_right = n.right.is_some();
                if !has_left && !has_right {
                    (None, Some(n.value))
                } else if !has_right
                    || (has_left
                        && n.left.as_ref().unwrap().priority > n.right.as_ref().unwrap().priority)
                {
                    let mut rotated = xq_155_rotate_right(n);
                    let (new_right, old) = xq_155_remove_node(rotated.right.take(), key);
                    rotated.right = new_right;
                    xq_155_update_size(&mut rotated);
                    (Some(rotated), old)
                } else {
                    let mut rotated = xq_155_rotate_left(n);
                    let (new_left, old) = xq_155_remove_node(rotated.left.take(), key);
                    rotated.left = new_left;
                    xq_155_update_size(&mut rotated);
                    (Some(rotated), old)
                }
            }
        },
    }
}

fn xq_155_find_min<K, V>(node: &Option<Box<Xq155TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.left.is_some() { xq_155_find_min(&n.left) } else { Some(&n.key) }
    }).flatten()
}

fn xq_155_find_max<K, V>(node: &Option<Box<Xq155TreapNode<K, V>>>) -> Option<&K> {
    node.as_ref().map(|n| {
        if n.right.is_some() { xq_155_find_max(&n.right) } else { Some(&n.key) }
    }).flatten()
}

fn xq_155_rank<K: Ord, V>(node: &Option<Box<Xq155TreapNode<K, V>>>, key: &K) -> usize {
    match node {
        None => 0,
        Some(n) => match key.cmp(&n.key) {
            Xq155Ord::Less => xq_155_rank(&n.left, key),
            Xq155Ord::Equal => xq_155_size(&n.left),
            Xq155Ord::Greater => 1 + xq_155_size(&n.left) + xq_155_rank(&n.right, key),
        },
    }
}

fn xq_155_kth<K, V>(node: &Option<Box<Xq155TreapNode<K, V>>>, k: usize) -> Option<&K> {
    node.as_ref().and_then(|n| {
        let left_size = xq_155_size(&n.left);
        if k < left_size {
            xq_155_kth(&n.left, k)
        } else if k == left_size {
            Some(&n.key)
        } else {
            xq_155_kth(&n.right, k - left_size - 1)
        }
    })
}

fn xq_155_in_order<K: Clone, V>(node: &Option<Box<Xq155TreapNode<K, V>>>, out: &mut Vec<K>) {
    if let Some(n) = node {
        xq_155_in_order(&n.left, out);
        out.push(n.key.clone());
        xq_155_in_order(&n.right, out);
    }
}

impl<K: Ord + Clone, V> Xq155Treap<K, V> {
    pub fn xq_new() -> Self {
        Self { root: None, seed: 12345 + 155 as u64 }
    }
    fn xq_next_priority(&mut self) -> u64 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        self.seed
    }
    pub fn xq_insert(&mut self, key: K, value: V) -> Option<V> {
        let p = self.xq_next_priority();
        let (new_root, old) = xq_155_insert_node(self.root.take(), key, value, p);
        self.root = new_root;
        old
    }
    pub fn xq_get(&self, key: &K) -> Option<&V> {
        let mut cur = &self.root;
        while let Some(n) = cur {
            match key.cmp(&n.key) {
                Xq155Ord::Equal => return Some(&n.value),
                Xq155Ord::Less => cur = &n.left,
                Xq155Ord::Greater => cur = &n.right,
            }
        }
        None
    }
    pub fn xq_remove(&mut self, key: &K) -> Option<V> {
        let (new_root, old) = xq_155_remove_node(self.root.take(), key);
        self.root = new_root;
        old
    }
    pub fn xq_len(&self) -> usize { xq_155_size(&self.root) }
    pub fn xq_min(&self) -> Option<&K> { xq_155_find_min(&self.root) }
    pub fn xq_max(&self) -> Option<&K> { xq_155_find_max(&self.root) }
    pub fn xq_rank(&self, key: &K) -> usize { xq_155_rank(&self.root, key) }
    pub fn xq_kth_element(&self, k: usize) -> Option<&K> { xq_155_kth(&self.root, k) }
    pub fn xq_in_order(&self) -> Vec<K> {
        let mut v = Vec::new();
        xq_155_in_order(&self.root, &mut v);
        v
    }
}

// --------------- Xq155VEBTree ---------------

pub struct Xq155VEBTree {
    universe: usize,
    min_val: Option<usize>,
    max_val: Option<usize>,
    count: usize,
    summary: Option<Box<Xq155VEBTree>>,
    clusters: Vec<Option<Box<Xq155VEBTree>>>,
    sqrt_hi: usize,
    sqrt_lo: usize,
}

impl Xq155VEBTree {
    pub fn xq_new(universe: usize) -> Self {
        let u = universe.max(2);
        let sqrt_hi = (1usize << ((u as f64).log2().ceil() as u32 / 2 + (u as f64).log2().ceil() as u32 % 2)).max(2);
        let sqrt_lo = (1usize << ((u as f64).log2().ceil() as u32 / 2)).max(2);
        let clusters = if u <= 2 {
            Vec::new()
        } else {
            (0..sqrt_hi).map(|_| None).collect()
        };
        let summary = if u <= 2 { None } else { Some(Box::new(Xq155VEBTree::xq_new(sqrt_hi))) };
        Self { universe: u, min_val: None, max_val: None, count: 0, summary, clusters, sqrt_hi, sqrt_lo }
    }

    fn xq_high(&self, x: usize) -> usize { x / self.sqrt_lo }
    fn xq_low(&self, x: usize) -> usize { x % self.sqrt_lo }
    fn xq_index(&self, hi: usize, lo: usize) -> usize { hi * self.sqrt_lo + lo }

    pub fn xq_insert(&mut self, x: usize) {
        if self.min_val.is_none() {
            self.min_val = Some(x);
            self.max_val = Some(x);
            self.count = 1;
            return;
        }
        let mut val = x;
        if val == self.min_val.unwrap() { return; }
        if val < self.min_val.unwrap() {
            std::mem::swap(&mut val, self.min_val.as_mut().unwrap());
        }
        if self.universe > 2 {
            let hi = self.xq_high(val);
            let lo = self.xq_low(val);
            if hi < self.clusters.len() {
                let need_summary = self.clusters[hi].is_none();
                if need_summary {
                    self.clusters[hi] = Some(Box::new(Xq155VEBTree::xq_new(self.sqrt_lo)));
                }
                let before = self.clusters[hi].as_ref().unwrap().count;
                self.clusters[hi].as_mut().unwrap().xq_insert(lo);
                let after = self.clusters[hi].as_ref().unwrap().count;
                if after > before {
                    self.count += 1;
                    if need_summary {
                        if let Some(ref mut s) = self.summary { s.xq_insert(hi); }
                    }
                }
            }
        } else if val != self.min_val.unwrap() {
            self.count += 1;
        }
        if val > self.max_val.unwrap() { self.max_val = Some(val); }
    }

    pub fn xq_contains(&self, x: usize) -> bool {
        if self.min_val == Some(x) || self.max_val == Some(x) { return true; }
        if self.universe <= 2 { return false; }
        let hi = self.xq_high(x);
        let lo = self.xq_low(x);
        if hi < self.clusters.len() {
            self.clusters[hi].as_ref().map_or(false, |c| c.xq_contains(lo))
        } else {
            false
        }
    }

    pub fn xq_delete(&mut self, x: usize) {
        if self.min_val.is_none() { return; }
        if self.min_val == self.max_val {
            if self.min_val == Some(x) {
                self.min_val = None;
                self.max_val = None;
                self.count = 0;
            }
            return;
        }
        if !self.xq_contains(x) && self.min_val != Some(x) { return; }
        self.count = self.count.saturating_sub(1);
        if self.universe <= 2 {
            if x == 0 { self.min_val = Some(1); } else { self.min_val = Some(0); }
            self.max_val = self.min_val;
            return;
        }
        let mut val = x;
        if val == self.min_val.unwrap() {
            if let Some(ref s) = self.summary {
                if let Some(first_cluster) = s.min_val {
                    if let Some(ref c) = self.clusters[first_cluster] {
                        if let Some(lo) = c.min_val {
                            val = self.xq_index(first_cluster, lo);
                            self.min_val = Some(val);
                        }
                    }
                } else { return; }
            } else { return; }
        }
        let hi = self.xq_high(val);
        let lo = self.xq_low(val);
        if hi < self.clusters.len() {
            if let Some(ref mut c) = self.clusters[hi] {
                c.xq_delete(lo);
                if c.min_val.is_none() {
                    if let Some(ref mut s) = self.summary { s.xq_delete(hi); }
                }
            }
        }
        if Some(val) == self.max_val {
            if let Some(ref s) = self.summary {
                if let Some(last) = s.max_val {
                    if let Some(ref c) = self.clusters[last] {
                        if let Some(m) = c.max_val {
                            self.max_val = Some(self.xq_index(last, m));
                        }
                    }
                } else {
                    self.max_val = self.min_val;
                }
            } else {
                self.max_val = self.min_val;
            }
        }
    }

    pub fn xq_successor(&self, x: usize) -> Option<usize> {
        if self.min_val.is_none() { return None; }
        if x < self.min_val.unwrap() { return self.min_val; }
        if self.universe <= 2 {
            if x == 0 && self.max_val == Some(1) { return Some(1); }
            return None;
        }
        let hi = self.xq_high(x);
        let lo = self.xq_low(x);
        if hi < self.clusters.len() {
            if let Some(ref c) = self.clusters[hi] {
                if let Some(m) = c.max_val {
                    if lo < m {
                        if let Some(offset) = c.xq_successor(lo) {
                            return Some(self.xq_index(hi, offset));
                        }
                    }
                }
            }
            if let Some(ref s) = self.summary {
                if let Some(next_hi) = s.xq_successor(hi) {
                    if next_hi < self.clusters.len() {
                        if let Some(ref nc) = self.clusters[next_hi] {
                            if let Some(lo2) = nc.min_val {
                                return Some(self.xq_index(next_hi, lo2));
                            }
                        }
                    }
                }
            }
        }
        None
    }

    pub fn xq_predecessor(&self, x: usize) -> Option<usize> {
        if self.min_val.is_none() { return None; }
        if x > self.max_val.unwrap() { return self.max_val; }
        if self.universe <= 2 {
            if x == 1 && self.min_val == Some(0) { return Some(0); }
            return None;
        }
        let hi = self.xq_high(x);
        let lo = self.xq_low(x);
        if hi < self.clusters.len() {
            if let Some(ref c) = self.clusters[hi] {
                if let Some(m) = c.min_val {
                    if lo > m {
                        if let Some(offset) = c.xq_predecessor(lo) {
                            return Some(self.xq_index(hi, offset));
                        }
                    }
                }
            }
            if let Some(ref s) = self.summary {
                if let Some(prev_hi) = s.xq_predecessor(hi) {
                    if prev_hi < self.clusters.len() {
                        if let Some(ref pc) = self.clusters[prev_hi] {
                            if let Some(m) = pc.max_val {
                                return Some(self.xq_index(prev_hi, m));
                            }
                        }
                    }
                }
            }
        }
        if self.min_val.is_some() && x > self.min_val.unwrap() { return self.min_val; }
        None
    }

    pub fn xq_min(&self) -> Option<usize> { self.min_val }
    pub fn xq_max(&self) -> Option<usize> { self.max_val }
    pub fn xq_count(&self) -> usize { self.count }
}


/// A 2D point for the k-d tree.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Xr155KDPoint {
    pub xr_x: f64,
    pub xr_y: f64,
}

impl Xr155KDPoint {
    pub fn xr_new(xr_x: f64, xr_y: f64) -> Self {
        Self { xr_x, xr_y }
    }

    fn xr_dist_sq(&self, other: &Self) -> f64 {
        let dx = self.xr_x - other.xr_x;
        let dy = self.xr_y - other.xr_y;
        dx * dx + dy * dy
    }
}

/// Bounding box result.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Xr155BoundingBox {
    pub xr_min_x: f64,
    pub xr_min_y: f64,
    pub xr_max_x: f64,
    pub xr_max_y: f64,
}

struct Xr155KDNode {
    xr_point: Xr155KDPoint,
    xr_left: Option<Box<Xr155KDNode>>,
    xr_right: Option<Box<Xr155KDNode>>,
}

/// K-d tree for 2D point queries.
pub struct Xr155KDTree {
    xr_root: Option<Box<Xr155KDNode>>,
    xr_size: usize,
}

impl Xr155KDTree {
    /// Creates an empty k-d tree.
    pub fn xr_new() -> Self {
        Self { xr_root: None, xr_size: 0 }
    }

    /// Inserts a point into the tree.
    pub fn xr_insert(&mut self, point: Xr155KDPoint) {
        self.xr_root = Some(Self::xr_insert_rec(self.xr_root.take(), point, 0));
        self.xr_size += 1;
    }

    fn xr_insert_rec(
        node: Option<Box<Xr155KDNode>>,
        point: Xr155KDPoint,
        depth: usize,
    ) -> Box<Xr155KDNode> {
        match node {
            None => Box::new(Xr155KDNode {
                xr_point: point,
                xr_left: None,
                xr_right: None,
            }),
            Some(mut n) => {
                let go_left = if depth % 2 == 0 {
                    point.xr_x < n.xr_point.xr_x
                } else {
                    point.xr_y < n.xr_point.xr_y
                };
                if go_left {
                    n.xr_left = Some(Self::xr_insert_rec(n.xr_left.take(), point, depth + 1));
                } else {
                    n.xr_right = Some(Self::xr_insert_rec(n.xr_right.take(), point, depth + 1));
                }
                n
            }
        }
    }

    /// Finds the nearest neighbor to the query point.
    pub fn xr_nearest_neighbor(&self, query: &Xr155KDPoint) -> Option<Xr155KDPoint> {
        self.xr_root.as_ref().map(|root| {
            let mut best = root.xr_point;
            let mut best_dist = query.xr_dist_sq(&best);
            Self::xr_nn_rec(root, query, 0, &mut best, &mut best_dist);
            best
        })
    }

    fn xr_nn_rec(
        node: &Box<Xr155KDNode>,
        query: &Xr155KDPoint,
        depth: usize,
        best: &mut Xr155KDPoint,
        best_dist: &mut f64,
    ) {
        let d = query.xr_dist_sq(&node.xr_point);
        if d < *best_dist {
            *best_dist = d;
            *best = node.xr_point;
        }
        let axis_val = if depth % 2 == 0 { query.xr_x - node.xr_point.xr_x } else { query.xr_y - node.xr_point.xr_y };
        let (first, second) = if axis_val < 0.0 {
            (&node.xr_left, &node.xr_right)
        } else {
            (&node.xr_right, &node.xr_left)
        };
        if let Some(child) = first.as_ref() {
            Self::xr_nn_rec(child, query, depth + 1, best, best_dist);
        }
        if axis_val * axis_val < *best_dist {
            if let Some(child) = second.as_ref() {
                Self::xr_nn_rec(child, query, depth + 1, best, best_dist);
            }
        }
    }

    /// Returns all points within the given rectangular range.
    pub fn xr_range_search(
        &self,
        xr_min_x: f64,
        xr_min_y: f64,
        xr_max_x: f64,
        xr_max_y: f64,
    ) -> Vec<Xr155KDPoint> {
        let mut result = Vec::new();
        if let Some(root) = &self.xr_root {
            Self::xr_range_rec(root, xr_min_x, xr_min_y, xr_max_x, xr_max_y, 0, &mut result);
        }
        result
    }

    fn xr_range_rec(
        node: &Box<Xr155KDNode>,
        xr_min_x: f64,
        xr_min_y: f64,
        xr_max_x: f64,
        xr_max_y: f64,
        depth: usize,
        result: &mut Vec<Xr155KDPoint>,
    ) {
        let p = &node.xr_point;
        if p.xr_x >= xr_min_x && p.xr_x <= xr_max_x && p.xr_y >= xr_min_y && p.xr_y <= xr_max_y {
            result.push(*p);
        }
        let (val, lo, hi) = if depth % 2 == 0 {
            (p.xr_x, xr_min_x, xr_max_x)
        } else {
            (p.xr_y, xr_min_y, xr_max_y)
        };
        if lo <= val {
            if let Some(left) = &node.xr_left {
                Self::xr_range_rec(left, xr_min_x, xr_min_y, xr_max_x, xr_max_y, depth + 1, result);
            }
        }
        if hi >= val {
            if let Some(right) = &node.xr_right {
                Self::xr_range_rec(right, xr_min_x, xr_min_y, xr_max_x, xr_max_y, depth + 1, result);
            }
        }
    }

    /// Number of points in the tree.
    pub fn xr_len(&self) -> usize {
        self.xr_size
    }

    /// Whether the tree is empty.
    pub fn xr_is_empty(&self) -> bool {
        self.xr_size == 0
    }

    /// Collects all points in the tree.
    pub fn xr_all_points(&self) -> Vec<Xr155KDPoint> {
        let mut pts = Vec::new();
        Self::xr_collect(&self.xr_root, &mut pts);
        pts
    }

    fn xr_collect(node: &Option<Box<Xr155KDNode>>, pts: &mut Vec<Xr155KDPoint>) {
        if let Some(n) = node {
            pts.push(n.xr_point);
            Self::xr_collect(&n.xr_left, pts);
            Self::xr_collect(&n.xr_right, pts);
        }
    }

    /// Returns the depth of the tree.
    pub fn xr_depth(&self) -> usize {
        Self::xr_depth_rec(&self.xr_root)
    }

    fn xr_depth_rec(node: &Option<Box<Xr155KDNode>>) -> usize {
        match node {
            None => 0,
            Some(n) => {
                let l = Self::xr_depth_rec(&n.xr_left);
                let r = Self::xr_depth_rec(&n.xr_right);
                1 + l.max(r)
            }
        }
    }

    /// Returns the bounding box of all points, or None if empty.
    pub fn xr_bounding_box(&self) -> Option<Xr155BoundingBox> {
        if self.xr_is_empty() {
            return None;
        }
        let pts = self.xr_all_points();
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        for p in &pts {
            if p.xr_x < min_x { min_x = p.xr_x; }
            if p.xr_y < min_y { min_y = p.xr_y; }
            if p.xr_x > max_x { max_x = p.xr_x; }
            if p.xr_y > max_y { max_y = p.xr_y; }
        }
        Some(Xr155BoundingBox { xr_min_x: min_x, xr_min_y: min_y, xr_max_x: max_x, xr_max_y: max_y })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn store_and_retrieve() {
        let mut svc = InMemorySecretStorage::new();
        svc.store("token", "abc123").unwrap();
        assert_eq!(svc.get("token"), Some("abc123".to_string()));
        assert!(svc.has("token"));
        assert_eq!(svc.key_count(), 1);
    }

    #[test]
    fn delete_secret_works() {
        let mut svc = InMemorySecretStorage::new();
        svc.store("key", "val").unwrap();
        assert_eq!(svc.delete("key").unwrap(), true);
        assert!(!svc.has("key"));
        assert_eq!(svc.delete("key").unwrap(), false);
    }

    #[test]
    fn clear_all() {
        let mut svc = InMemorySecretStorage::new();
        svc.store("a", "1").unwrap();
        svc.store("b", "2").unwrap();
        assert_eq!(svc.key_count(), 2);
        svc.clear();
        assert_eq!(svc.key_count(), 0);
    }

    #[test]
    fn change_log_events() {
        let mut svc = InMemorySecretStorage::new();
        svc.store("k", "v1").unwrap();
        svc.store("k", "v2").unwrap();
        svc.delete("k").unwrap();
        let log = svc.change_log();
        assert_eq!(log.len(), 3);
        assert_eq!(log[0].kind, SecretChangeKind::Added);
        assert_eq!(log[1].kind, SecretChangeKind::Updated);
        assert_eq!(log[2].kind, SecretChangeKind::Deleted);
    }

    #[test]
    fn keys_sorted() {
        let mut svc = InMemorySecretStorage::new();
        svc.store("z", "1").unwrap();
        svc.store("a", "2").unwrap();
        svc.store("m", "3").unwrap();
        assert_eq!(svc.keys(), vec!["a", "m", "z"]);
    }

    #[test]
    fn trait_object_usage() {
        let mut storage: Box<dyn SecretStorage> = Box::new(InMemorySecretStorage::new());
        storage.store("x", "y").unwrap();
        assert_eq!(storage.get("x"), Some("y".to_string()));
    }

    #[test]
    fn keyring_fallback() {
        let mut kr = KeyringSecretStorage::new("vsedit-test");
        assert_eq!(kr.service_name(), "vsedit-test");
        assert!(!kr.is_keyring_available());
        kr.store("tok", "secret").unwrap();
        assert_eq!(kr.get("tok"), Some("secret".to_string()));
    }

    #[test]
    fn secret_storage_error_display() {
        let e = SecretStorageError::BackendUnavailable("test".into());
        assert!(e.to_string().contains("Backend unavailable"));
        let e = SecretStorageError::AccessDenied;
        assert!(e.to_string().contains("Access denied"));
    }

    #[test]
    fn invalid_key_display() {
        let e = SecretStorageError::InvalidKey("bad key".into());
        assert!(e.to_string().contains("Invalid key"));
    }

    #[test]
    fn error_is_std_error() {
        let e: Box<dyn std::error::Error> =
            Box::new(SecretStorageError::Other("boom".into()));
        assert!(e.to_string().contains("boom"));
    }

    #[test]
    fn validate_key_rejects_empty() {
        assert_eq!(
            validate_key(""),
            Err(SecretStorageError::InvalidKey("key must not be empty".into()))
        );
    }

    #[test]
    fn validate_key_rejects_whitespace() {
        assert!(validate_key("has space").is_err());
        assert!(validate_key("has\ttab").is_err());
    }

    #[test]
    fn validate_key_rejects_too_long() {
        let long_key = "a".repeat(257);
        assert!(validate_key(&long_key).is_err());
    }

    #[test]
    fn validate_key_accepts_good_keys() {
        assert!(validate_key("my-secret").is_ok());
        assert!(validate_key("oauth.token").is_ok());
        assert!(validate_key("a").is_ok());
    }

    #[test]
    fn secret_entry_new_validates() {
        assert!(SecretEntry::new("good-key", "val").is_ok());
        assert!(SecretEntry::new("", "val").is_err());
        assert!(SecretEntry::new("bad key", "val").is_err());
    }

    #[test]
    fn secret_entry_redacted_value() {
        let entry = SecretEntry::new("k", "abcdef").unwrap();
        let redacted = entry.redacted_value();
        assert!(redacted.starts_with("ab"));
        assert!(!redacted.contains("cdef"));

        let short = SecretEntry::new("k", "ab").unwrap();
        assert_eq!(short.redacted_value(), "****");
    }

    #[test]
    fn secret_entry_display() {
        let entry = SecretEntry::new("tok", "secret123").unwrap();
        let display = entry.to_string();
        assert!(display.starts_with("tok="));
        assert!(!display.contains("secret123"));
    }

    #[test]
    fn secret_entry_builder() {
        let entry = SecretEntryBuilder::new()
            .key("my-key")
            .value("my-val")
            .build()
            .unwrap();
        assert_eq!(entry.key, "my-key");
        assert_eq!(entry.value, "my-val");
    }

    #[test]
    fn secret_entry_builder_missing_key() {
        let result = SecretEntryBuilder::new().value("v").build();
        assert!(result.is_err());
    }

    #[test]
    fn secret_entry_builder_missing_value() {
        let result = SecretEntryBuilder::new().key("k").build();
        assert!(result.is_err());
    }

    #[test]
    fn entries_returns_sorted() {
        let mut svc = InMemorySecretStorage::new();
        svc.store("z", "1").unwrap();
        svc.store("a", "2").unwrap();
        let entries = svc.entries();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].key, "a");
        assert_eq!(entries[1].key, "z");
    }

    #[test]
    fn store_entry_method() {
        let mut svc = InMemorySecretStorage::new();
        let entry = SecretEntry::new("my-token", "value").unwrap();
        svc.store_entry(&entry).unwrap();
        assert_eq!(svc.get("my-token"), Some("value".to_string()));
    }

    #[test]
    fn changes_of_kind_filter() {
        let mut svc = InMemorySecretStorage::new();
        svc.store("a", "1").unwrap();
        svc.store("b", "2").unwrap();
        svc.store("a", "3").unwrap();
        svc.delete("b").unwrap();
        assert_eq!(svc.changes_of_kind(SecretChangeKind::Added).len(), 2);
        assert_eq!(svc.changes_of_kind(SecretChangeKind::Updated).len(), 1);
        assert_eq!(svc.changes_of_kind(SecretChangeKind::Deleted).len(), 1);
    }

    #[test]
    fn merge_from_other() {
        let mut a = InMemorySecretStorage::new();
        a.store("x", "1").unwrap();
        let mut b = InMemorySecretStorage::new();
        b.store("y", "2").unwrap();
        b.store("z", "3").unwrap();
        a.merge_from(&b).unwrap();
        assert_eq!(a.key_count(), 3);
        assert_eq!(a.get("y"), Some("2".to_string()));
    }

    #[test]
    fn secret_change_kind_display() {
        assert_eq!(SecretChangeKind::Added.to_string(), "added");
        assert_eq!(SecretChangeKind::Updated.to_string(), "updated");
        assert_eq!(SecretChangeKind::Deleted.to_string(), "deleted");
    }

    #[test]
    fn scoped_storage_isolates_keys() {
        let mut scoped = ScopedSecretStorage::new("ext1");
        scoped.store("token", "abc").unwrap();
        assert_eq!(scoped.get("token"), Some("abc".to_string()));
        assert_eq!(scoped.keys(), vec!["token".to_string()]);
        // The inner key is namespaced
        assert!(scoped.inner.has("ext1.token"));
        assert!(!scoped.inner.has("token"));
    }

    #[test]
    fn scoped_storage_delete() {
        let mut scoped = ScopedSecretStorage::new("ns");
        scoped.store("k", "v").unwrap();
        assert_eq!(scoped.delete("k").unwrap(), true);
        assert_eq!(scoped.get("k"), None);
        assert_eq!(scoped.delete("k").unwrap(), false);
    }

    #[test]
    fn secret_entry_is_empty() {
        let empty = SecretEntry::new("k", "").unwrap();
        assert!(empty.is_empty());
        assert_eq!(empty.value_len(), 0);
        let notempty = SecretEntry::new("k", "val").unwrap();
        assert!(!notempty.is_empty());
        assert_eq!(notempty.value_len(), 3);
    }

    #[test]
    fn secrets_stats_new_defaults() {
        let stats = SecretsStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn secrets_stats_record_success() {
        let mut stats = SecretsStats::new();
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
    fn secrets_stats_record_failure() {
        let mut stats = SecretsStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn secrets_stats_reset() {
        let mut stats = SecretsStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn secrets_stats_merge() {
        let mut a = SecretsStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = SecretsStats::new();
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
    fn secrets_stats_display() {
        let mut stats = SecretsStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn secrets_stats_default() {
        let stats = SecretsStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn secrets_validator_accepts_and_rejects() {
        let mut v = SecretsValidationCollector::new();
        assert!(v.is_valid());
        v.add_error("bad secret");
        assert!(!v.is_valid());
        assert_eq!(v.error_count(), 1);
        assert_eq!(v.first_error(), Some("bad secret"));
    }

    #[test]
    fn secrets_validator_warnings() {
        let mut v = SecretsValidationCollector::new();
        v.add_warning("deprecated secret");
        assert!(v.is_valid());
        assert_eq!(v.warning_count(), 1);
    }

    #[test]
    fn secrets_validator_clear_and_merge() {
        let mut v = SecretsValidationCollector::new();
        v.add_error("e1");
        v.clear();
        assert!(v.is_valid());

        let mut a = SecretsValidationCollector::new();
        a.add_error("a_err");
        let mut b = SecretsValidationCollector::new();
        b.add_error("b_err");
        a.merge(&b);
        assert_eq!(a.error_count(), 2);
    }

    // -- EncryptedFileStore tests ----------------------------------------------

    #[test]
    fn encrypted_file_store_roundtrip() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("secrets.json");
        let mut store = EncryptedFileStore::new(&path, "test-key");
        store.store("token", "my-secret-value").unwrap();
        assert_eq!(store.get("token"), Some("my-secret-value".to_string()));

        // Re-open from disk
        let store2 = EncryptedFileStore::new(&path, "test-key");
        assert_eq!(store2.get("token"), Some("my-secret-value".to_string()));
    }

    #[test]
    fn encrypted_file_store_delete() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("secrets.json");
        let mut store = EncryptedFileStore::new(&path, "key");
        store.store("a", "1").unwrap();
        assert_eq!(store.delete("a").unwrap(), true);
        assert_eq!(store.get("a"), None);
        assert_eq!(store.delete("a").unwrap(), false);
    }

    #[test]
    fn encrypted_file_store_keys() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("secrets.json");
        let mut store = EncryptedFileStore::new(&path, "key");
        store.store("z-key", "1").unwrap();
        store.store("a-key", "2").unwrap();
        assert_eq!(store.keys(), vec!["a-key", "z-key"]);
    }

    #[test]
    fn encrypted_file_store_wrong_key_returns_nothing() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("secrets.json");
        let mut store = EncryptedFileStore::new(&path, "correct-key");
        store.store("token", "secret").unwrap();

        // Opening with wrong key should not recover the value correctly
        let store2 = EncryptedFileStore::new(&path, "wrong-key");
        let val = store2.get("token");
        // The value will be garbage (decrypted with wrong key), so it won't
        // match the original. It may or may not be valid UTF-8.
        assert_ne!(val, Some("secret".to_string()));
    }

    #[test]
    fn encrypted_file_store_validates_key() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("secrets.json");
        let mut store = EncryptedFileStore::new(&path, "key");
        assert!(store.store("", "val").is_err());
        assert!(store.store("has space", "val").is_err());
    }

    #[test]
    fn encrypted_file_store_default_key() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("secrets.json");
        let mut store = EncryptedFileStore::new(&path, "");
        store.store("tok", "val").unwrap();
        assert_eq!(store.get("tok"), Some("val".to_string()));
    }

    #[test]
    fn encrypted_file_store_path() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("secrets.json");
        let store = EncryptedFileStore::new(&path, "key");
        assert_eq!(store.path(), path.as_path());
    }

    #[test]
    fn encrypted_file_store_default_path() {
        let p = EncryptedFileStore::default_path();
        // Should return Some on most systems
        if let Some(path) = p {
            assert!(path.ends_with("vsedit/secrets.json") || path.ends_with("vsedit\\secrets.json"));
        }
    }

    // -- SecretService tests ---------------------------------------------------

    #[test]
    fn secret_service_falls_back_to_file() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("secrets.json");
        let mut svc = SecretService::new("vsedit-test", &path, "key");
        assert!(!svc.is_using_keyring());
        svc.store_secret("api-token", "abc123").unwrap();
        assert_eq!(svc.get_secret("api-token"), Some("abc123".to_string()));
    }

    #[test]
    fn secret_service_delete() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("secrets.json");
        let mut svc = SecretService::new("vsedit-test", &path, "key");
        svc.store_secret("k", "v").unwrap();
        assert_eq!(svc.delete_secret("k").unwrap(), true);
        assert_eq!(svc.get_secret("k"), None);
    }

    #[test]
    fn secret_service_list_keys() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("secrets.json");
        let mut svc = SecretService::new("vsedit-test", &path, "key");
        svc.store_secret("beta", "2").unwrap();
        svc.store_secret("alpha", "1").unwrap();
        assert_eq!(svc.list_keys(), vec!["alpha", "beta"]);
    }

    #[test]
    fn secret_service_validates_key() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join("secrets.json");
        let mut svc = SecretService::new("vsedit-test", &path, "key");
        assert!(svc.store_secret("", "val").is_err());
    }

    // -- base64 roundtrip test -------------------------------------------------

    #[test]
    fn base64_roundtrip() {
        let original = b"Hello, vsedit secrets!";
        let encoded = base64_encode(original);
        let decoded = base64_decode(&encoded).unwrap();
        assert_eq!(decoded, original);
    }

    // ── Secret masking ────────────────────────────────────────────

    #[test]
    fn masker_masks_secrets_in_text() {
        let mut masker = SecretMasker::new();
        masker.add_secret("s3cr3t_token");
        let output = masker.mask("Authorization: Bearer s3cr3t_token");
        assert_eq!(output, "Authorization: Bearer ***");
        assert!(!output.contains("s3cr3t_token"));
    }

    #[test]
    fn masker_with_label() {
        let mut masker = SecretMasker::new();
        masker.add_secret_with_label("abc123", "API_KEY");
        let output = masker.mask("key=abc123");
        assert_eq!(output, "key=[API_KEY]");
    }

    #[test]
    fn masker_empty_secret_ignored() {
        let mut masker = SecretMasker::new();
        masker.add_secret("");
        assert_eq!(masker.secret_count(), 0);
    }

    // ── Expiration tracking ───────────────────────────────────────

    #[test]
    fn expiration_tracker_expired_keys() {
        let mut tracker = ExpirationTracker::new();
        tracker.set_expiration("token-a", 1000);
        tracker.set_expiration("token-b", 2000);
        tracker.set_expiration("token-c", 3000);

        let expired = tracker.expired_keys(1500);
        assert_eq!(expired, vec!["token-a"]);
    }

    #[test]
    fn expiration_tracker_expiring_soon() {
        let mut tracker = ExpirationTracker::new();
        tracker.set_expiration("token-a", 1100);
        tracker.set_expiration("token-b", 2000);

        let soon = tracker.expiring_soon(1000, 200);
        assert_eq!(soon, vec!["token-a"]);
        assert_eq!(tracker.count(), 2);
    }

    #[test]
    fn expiration_tracker_remove() {
        let mut tracker = ExpirationTracker::new();
        tracker.set_expiration("k", 100);
        assert_eq!(tracker.count(), 1);
        tracker.remove("k");
        assert_eq!(tracker.count(), 0);
    }

    // ── Audit log ─────────────────────────────────────────────────

    #[test]
    fn audit_log_records_and_queries() {
        let mut log = SecretAuditLog::new();
        log.record("api-key", SecretAccessAction::Read, 100, "extension-a");
        log.record("api-key", SecretAccessAction::Write, 200, "extension-b");
        log.record("db-pass", SecretAccessAction::Read, 150, "extension-a");

        assert_eq!(log.event_count(), 3);
        assert_eq!(log.events_for_key("api-key").len(), 2);
        let last = log.last_access("api-key").unwrap();
        assert_eq!(last.action, SecretAccessAction::Write);
        assert_eq!(last.timestamp, 200);
    }

    #[test]
    fn audit_log_accessed_keys() {
        let mut log = SecretAuditLog::new();
        log.record("b-key", SecretAccessAction::Read, 1, "ext");
        log.record("a-key", SecretAccessAction::Write, 2, "ext");
        let keys = log.accessed_keys();
        assert_eq!(keys, vec!["a-key", "b-key"]);
    }

    #[test]
    fn secret_access_action_display() {
        assert_eq!(format!("{}", SecretAccessAction::Read), "read");
        assert_eq!(format!("{}", SecretAccessAction::Delete), "delete");
    }

    // -- SecretRotator tests --------------------------------------------------

    #[test]
    fn rotator_tracks_rotations() {
        let mut rotator = SecretRotator::new(3600);
        assert_eq!(rotator.rotation_count("api-key"), 0);
        rotator.record_rotation("api-key");
        rotator.record_rotation("api-key");
        rotator.record_rotation("db-pass");
        assert_eq!(rotator.rotation_count("api-key"), 2);
        assert_eq!(rotator.rotation_count("db-pass"), 1);
        assert_eq!(rotator.total_rotations(), 3);
    }

    #[test]
    fn rotator_needs_rotation() {
        let rotator = SecretRotator::new(3600);
        let mut tracker = ExpirationTracker::new();
        tracker.set_expiration("old-key", 100);
        tracker.set_expiration("fresh-key", 5000);
        let expired = rotator.needs_rotation(&tracker, 200);
        assert_eq!(expired, vec!["old-key"]);
    }

    #[test]
    fn rotator_expiring_within_policy() {
        let rotator = SecretRotator::new(1000);
        let mut tracker = ExpirationTracker::new();
        tracker.set_expiration("soon", 1500);
        tracker.set_expiration("later", 5000);
        let expiring = rotator.expiring_within_policy(&tracker, 1000);
        assert_eq!(expiring, vec!["soon"]);
    }

    // -- AuditLog extended tests ----------------------------------------------

    #[test]
    fn audit_log_events_by_action() {
        let mut log = SecretAuditLog::new();
        log.record("k1", SecretAccessAction::Read, 1, "svc");
        log.record("k2", SecretAccessAction::Write, 2, "svc");
        log.record("k1", SecretAccessAction::Read, 3, "svc");
        let reads = log.events_by_action(SecretAccessAction::Read);
        assert_eq!(reads.len(), 2);
    }

    #[test]
    fn audit_log_events_in_range() {
        let mut log = SecretAuditLog::new();
        log.record("k1", SecretAccessAction::Read, 10, "svc");
        log.record("k2", SecretAccessAction::Read, 20, "svc");
        log.record("k3", SecretAccessAction::Read, 30, "svc");
        let in_range = log.events_in_range(15, 25);
        assert_eq!(in_range.len(), 1);
        assert_eq!(in_range[0].key, "k2");
    }

    #[test]
    fn audit_log_unique_callers() {
        let mut log = SecretAuditLog::new();
        log.record("k", SecretAccessAction::Read, 1, "alpha");
        log.record("k", SecretAccessAction::Read, 2, "beta");
        log.record("k", SecretAccessAction::Read, 3, "alpha");
        let callers = log.unique_callers();
        assert_eq!(callers, vec!["alpha", "beta"]);
    }

    // -- InMemorySecretStorage extended tests ----------------------------------

    #[test]
    fn storage_keys_with_prefix() {
        let mut store = InMemorySecretStorage::new();
        store.store("db.host", "localhost").unwrap();
        store.store("db.port", "5432").unwrap();
        store.store("api.key", "secret").unwrap();
        let mut db_keys = store.keys_with_prefix("db.");
        db_keys.sort();
        assert_eq!(db_keys, vec!["db.host", "db.port"]);
    }

    #[test]
    fn storage_rename_key() {
        let mut store = InMemorySecretStorage::new();
        store.store("old-name", "value123").unwrap();
        store.rename_key("old-name", "new-name").unwrap();
        assert!(store.get("old-name").is_none());
        assert_eq!(store.get("new-name").unwrap(), "value123");
    }

    // -- Secret strength evaluation -------------------------------------------

    #[test]
    fn strength_weak_short_value() {
        assert_eq!(evaluate_secret_strength("abc"), SecretStrength::Weak);
        assert_eq!(evaluate_secret_strength("1234567"), SecretStrength::Weak);
    }

    #[test]
    fn strength_fair_mixed_value() {
        assert_eq!(evaluate_secret_strength("Abcdefg1"), SecretStrength::Fair);
        assert_eq!(evaluate_secret_strength("password1A"), SecretStrength::Fair);
    }

    #[test]
    fn strength_strong_long_diverse_value() {
        assert_eq!(
            evaluate_secret_strength("MyP@ssw0rd!LongEnough"),
            SecretStrength::Strong
        );
    }

    #[test]
    fn strength_display() {
        assert_eq!(SecretStrength::Weak.to_string(), "weak");
        assert_eq!(SecretStrength::Fair.to_string(), "fair");
        assert_eq!(SecretStrength::Strong.to_string(), "strong");
    }

    #[test]
    fn strength_ordering() {
        assert!(SecretStrength::Weak < SecretStrength::Fair);
        assert!(SecretStrength::Fair < SecretStrength::Strong);
    }

    // -- Secret size policy ---------------------------------------------------

    #[test]
    fn size_policy_rejects_large_value() {
        let policy = SecretSizePolicy::new(10, 1000);
        assert!(policy.check_value("short").is_ok());
        assert!(policy.check_value(&"x".repeat(11)).is_err());
    }

    #[test]
    fn size_policy_rejects_total_overflow() {
        let policy = SecretSizePolicy::new(100, 50);
        assert!(policy.check_total(40, 5).is_ok());
        assert!(policy.check_total(40, 20).is_err());
    }

    #[test]
    fn size_policy_default() {
        let policy = SecretSizePolicy::default();
        assert_eq!(policy.max_value_bytes(), 64 * 1024);
        assert_eq!(policy.max_total_bytes(), 1024 * 1024);
    }

    // -- Migration ------------------------------------------------------------

    #[test]
    fn migrate_all_secrets() {
        let mut src = InMemorySecretStorage::new();
        src.store("key1", "val1").unwrap();
        src.store("key2", "val2").unwrap();
        let mut dst = InMemorySecretStorage::new();
        let count = migrate_secrets(&src, &mut dst).unwrap();
        assert_eq!(count, 2);
        assert_eq!(dst.get("key1"), Some("val1".to_string()));
        assert_eq!(dst.get("key2"), Some("val2".to_string()));
    }

    #[test]
    fn migrate_secrets_with_prefix_filter() {
        let mut src = InMemorySecretStorage::new();
        src.store("prod.token", "t1").unwrap();
        src.store("prod.pass", "p1").unwrap();
        src.store("dev.token", "t2").unwrap();
        let mut dst = InMemorySecretStorage::new();
        let count = migrate_secrets_with_prefix(&src, &mut dst, "prod.").unwrap();
        assert_eq!(count, 2);
        assert!(dst.get("dev.token").is_none());
        assert_eq!(dst.get("prod.token"), Some("t1".to_string()));
    }

    // -- Snapshot / Diff ------------------------------------------------------

    #[test]
    fn snapshot_capture_and_diff() {
        let mut store = InMemorySecretStorage::new();
        store.store("alpha", "a1").unwrap();
        store.store("beta", "b1").unwrap();
        let snap1 = SecretSnapshot::capture(&store, 100);
        assert_eq!(snap1.len(), 2);
        assert!(!snap1.is_empty());

        // Modify storage
        store.store("beta", "b2").unwrap(); // changed
        store.store("gamma", "g1").unwrap(); // added
        store.delete("alpha").unwrap(); // removed
        let snap2 = SecretSnapshot::capture(&store, 200);

        let diff = snap1.diff(&snap2);
        assert_eq!(diff.added, vec!["gamma"]);
        assert_eq!(diff.removed, vec!["alpha"]);
        assert_eq!(diff.changed, vec!["beta"]);
        assert_eq!(diff.total(), 3);
        assert!(!diff.is_empty());
    }

    #[test]
    fn snapshot_diff_no_changes() {
        let mut store = InMemorySecretStorage::new();
        store.store("key", "val").unwrap();
        let snap1 = SecretSnapshot::capture(&store, 1);
        let snap2 = SecretSnapshot::capture(&store, 2);
        let diff = snap1.diff(&snap2);
        assert!(diff.is_empty());
        assert_eq!(diff.total(), 0);
    }

    #[test]
    fn snapshot_diff_display() {
        let diff = SecretDiff {
            added: vec!["a".into()],
            removed: vec!["b".into(), "c".into()],
            changed: vec![],
        };
        assert_eq!(diff.to_string(), "SecretDiff(+1 -2 ~0)");
    }

    // -- InMemorySecretStorage extended (new) ---------------------------------

    #[test]
    fn storage_total_value_bytes() {
        let mut store = InMemorySecretStorage::new();
        store.store("a", "12345").unwrap();
        store.store("b", "67").unwrap();
        assert_eq!(store.total_value_bytes(), 7);
    }

    #[test]
    fn storage_delete_with_prefix() {
        let mut store = InMemorySecretStorage::new();
        store.store("cache.a", "1").unwrap();
        store.store("cache.b", "2").unwrap();
        store.store("perm.c", "3").unwrap();
        let removed = store.delete_with_prefix("cache.");
        assert_eq!(removed, 2);
        assert_eq!(store.key_count(), 1);
        assert!(store.has("perm.c"));
    }

    #[test]
    fn storage_redacted_summary() {
        let mut store = InMemorySecretStorage::new();
        store.store("api-key", "supersecretvalue").unwrap();
        store.store("token", "ab").unwrap();
        let summary = store.redacted_summary();
        assert_eq!(summary.len(), 2);
        // Keys are sorted; api-key comes first
        assert!(summary[0].starts_with("api-key="));
        assert!(!summary[0].contains("supersecretvalue"));
        // Short value is fully redacted
        assert!(summary[1].contains("****"));
    }

    // -- SecretsVaultMigrator tests ------------------------------------------

    #[test]
    fn migrator_basic_migration() {
        let mut m = SecretsVaultMigrator::new();
        let entries = vec![
            SecretEntry { key: "db.password".into(), value: "s3cret".into() },
        ];
        let results = m.migrate_entries(&entries, VaultBackendKind::InMemory, VaultBackendKind::Keyring, &[]);
        assert_eq!(results.len(), 1);
        assert!(results[0].success);
    }

    #[test]
    fn migrator_key_filter() {
        let mut m = SecretsVaultMigrator::new();
        m.set_key_filter("db.");
        assert!(m.key_matches_filter("db.password"));
        assert!(!m.key_matches_filter("api.key"));
    }

    #[test]
    fn migrator_filter_applied_in_migration() {
        let mut m = SecretsVaultMigrator::new();
        m.set_key_filter("db.");
        let entries = vec![
            SecretEntry { key: "db.password".into(), value: "pw".into() },
            SecretEntry { key: "api.key".into(), value: "k".into() },
        ];
        let results = m.migrate_entries(&entries, VaultBackendKind::InMemory, VaultBackendKind::EncryptedFile, &[]);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].key, "db.password");
    }

    #[test]
    fn migrator_overwrite_disabled() {
        let mut m = SecretsVaultMigrator::new();
        let entries = vec![SecretEntry { key: "existing".into(), value: "v".into() }];
        let results = m.migrate_entries(&entries, VaultBackendKind::InMemory, VaultBackendKind::Keyring, &["existing".into()]);
        assert!(!results[0].success);
    }

    #[test]
    fn migrator_overwrite_enabled() {
        let mut m = SecretsVaultMigrator::new();
        m.set_overwrite(true);
        let entries = vec![SecretEntry { key: "existing".into(), value: "v".into() }];
        let results = m.migrate_entries(&entries, VaultBackendKind::InMemory, VaultBackendKind::Keyring, &["existing".into()]);
        assert!(results[0].success);
    }

    #[test]
    fn migrator_dry_run() {
        let mut m = SecretsVaultMigrator::new();
        m.set_dry_run(true);
        let entries = vec![SecretEntry { key: "k".into(), value: "v".into() }];
        let _ = m.migrate_entries(&entries, VaultBackendKind::InMemory, VaultBackendKind::Keyring, &[]);
        assert_eq!(m.success_count(), 1);
    }

    #[test]
    fn migrator_summary() {
        let mut m = SecretsVaultMigrator::new();
        let entries = vec![SecretEntry { key: "a".into(), value: "1".into() }];
        let _ = m.migrate_entries(&entries, VaultBackendKind::InMemory, VaultBackendKind::Keyring, &[]);
        assert!(m.summary().contains("1 succeeded"));
    }

    #[test]
    fn vault_backend_display() {
        assert_eq!(VaultBackendKind::InMemory.to_string(), "in-memory");
        assert_eq!(VaultBackendKind::Custom("vault".into()).to_string(), "custom(vault)");
    }

    // -- SecretsAccessAuditor tests ------------------------------------------

    #[test]
    fn auditor_log_access() {
        let mut a = SecretsAccessAuditor::new(100);
        let allowed = a.log_access("db.pw", AuditAccessKind::Read, "alice", 1000);
        assert!(allowed);
        assert_eq!(a.events().len(), 1);
    }

    #[test]
    fn auditor_deny_actor() {
        let mut a = SecretsAccessAuditor::new(100);
        a.deny_actor("mallory");
        let allowed = a.log_access("db.pw", AuditAccessKind::Read, "mallory", 1000);
        assert!(!allowed);
        assert_eq!(a.denied_count(), 1);
    }

    #[test]
    fn auditor_events_for_actor() {
        let mut a = SecretsAccessAuditor::new(100);
        a.log_access("k1", AuditAccessKind::Read, "alice", 1);
        a.log_access("k2", AuditAccessKind::Write, "bob", 2);
        a.log_access("k3", AuditAccessKind::Read, "alice", 3);
        assert_eq!(a.events_for_actor("alice").len(), 2);
    }

    #[test]
    fn auditor_max_events_eviction() {
        let mut a = SecretsAccessAuditor::new(2);
        a.log_access("k1", AuditAccessKind::Read, "a", 1);
        a.log_access("k2", AuditAccessKind::Read, "a", 2);
        a.log_access("k3", AuditAccessKind::Read, "a", 3);
        assert_eq!(a.events().len(), 2);
        assert_eq!(a.events()[0].key, "k2");
    }

    #[test]
    fn auditor_summary_format() {
        let mut a = SecretsAccessAuditor::new(100);
        a.deny_actor("bad");
        a.log_access("k1", AuditAccessKind::Read, "good", 1);
        a.log_access("k2", AuditAccessKind::Write, "bad", 2);
        let s = a.summary();
        assert!(s.contains("2 total events"));
        assert!(s.contains("1 denied"));
    }

    #[test]
    fn audit_access_kind_display() {
        assert_eq!(AuditAccessKind::Read.to_string(), "READ");
        assert_eq!(AuditAccessKind::Delete.to_string(), "DELETE");
    }



    // -- secrets additional tests -------------------------------------------

    #[test]
    fn x_secrets_capabilities_register_and_has() {
        let mut caps = XSecretsCapabilities::new();
        caps.register("clipboard");
        assert!(caps.has("clipboard"));
        assert!(!caps.has("fs"));
    }

    #[test]
    fn x_secrets_capabilities_len() {
        let mut caps = XSecretsCapabilities::new();
        assert!(caps.is_empty());
        caps.register("a");
        caps.register("b");
        assert_eq!(caps.len(), 2);
    }

    #[test]
    fn x_secrets_capabilities_intersect() {
        let mut a = XSecretsCapabilities::new();
        a.register("x");
        a.register("y");
        let mut b = XSecretsCapabilities::new();
        b.register("y");
        b.register("z");
        let inter = a.intersect(&b);
        assert_eq!(inter.len(), 1);
        assert!(inter.has("y"));
    }

    #[test]
    fn x_secrets_capabilities_diff() {
        let mut a = XSecretsCapabilities::new();
        a.register("x");
        a.register("y");
        let mut b = XSecretsCapabilities::new();
        b.register("y");
        let d = a.diff(&b);
        assert_eq!(d.len(), 1);
        assert!(d.has("x"));
    }

    #[test]
    fn x_secrets_service_registry_basic() {
        let mut reg = XSecretsServiceRegistry::new();
        assert!(reg.is_empty());
        reg.register("clipboard", "v1");
        assert_eq!(reg.get("clipboard"), Some("v1"));
        assert!(reg.contains("clipboard"));
    }

    #[test]
    fn x_secrets_service_registry_replace() {
        let mut reg = XSecretsServiceRegistry::new();
        assert!(reg.register("svc", "old").is_none());
        assert_eq!(reg.register("svc", "new"), Some("old".into()));
        assert_eq!(reg.get("svc"), Some("new"));
    }

    #[test]
    fn x_secrets_service_registry_remove() {
        let mut reg = XSecretsServiceRegistry::new();
        reg.register("svc", "v1");
        assert_eq!(reg.remove("svc"), Some("v1".into()));
        assert!(reg.is_empty());
    }

    #[test]
    fn x_secrets_service_registry_names() {
        let mut reg = XSecretsServiceRegistry::new();
        reg.register("a", "1");
        reg.register("b", "2");
        let mut names = reg.names();
        names.sort();
        assert_eq!(names, vec!["a", "b"]);
    }

    #[test]
    fn x_secrets_sanitize_path_basic() {
        assert_eq!(x_secrets_sanitize_path("/a//b///c/"), "/a/b/c");
    }

    #[test]
    fn x_secrets_sanitize_path_backslash() {
        assert_eq!(x_secrets_sanitize_path("a\\b\\c"), "a/b/c");
    }

    #[test]
    fn x_secrets_sanitize_path_single() {
        assert_eq!(x_secrets_sanitize_path("/"), "/");
    }

    #[test]
    fn x_secrets_capabilities_default() {
        let caps = XSecretsCapabilities::default();
        assert!(caps.is_empty());
    }

    #[test]
    fn x_secrets_capabilities_all() {
        let mut caps = XSecretsCapabilities::new();
        caps.register("a");
        caps.register("b");
        let mut all = caps.all();
        all.sort();
        assert_eq!(all, vec!["a", "b"]);
    }


    #[test]
    fn secrets_config_new() {
        let cfg = SecretsConfig::new();
        assert!(cfg.is_enabled());
        assert_eq!(cfg.version(), 1);
        assert_eq!(cfg.option_count(), 0);
    }

    #[test]
    fn secrets_config_set_get() {
        let mut cfg = SecretsConfig::new();
        cfg.set_option("key", "value");
        assert_eq!(cfg.get_option("key"), Some("value"));
        assert!(cfg.has_option("key"));
    }

    #[test]
    fn secrets_config_remove() {
        let mut cfg = SecretsConfig::new();
        cfg.set_option("a", "1");
        assert_eq!(cfg.remove_option("a"), Some("1".into()));
        assert!(!cfg.has_option("a"));
    }

    #[test]
    fn secrets_config_keys_sorted() {
        let mut cfg = SecretsConfig::new();
        cfg.set_option("z", "1");
        cfg.set_option("a", "2");
        assert_eq!(cfg.option_keys(), vec!["a", "z"]);
    }

    #[test]
    fn secrets_config_bump_version() {
        let mut cfg = SecretsConfig::new();
        cfg.bump_version();
        cfg.bump_version();
        assert_eq!(cfg.version(), 3);
    }

    #[test]
    fn secrets_config_clear() {
        let mut cfg = SecretsConfig::new();
        cfg.set_option("x", "y");
        cfg.bump_version();
        cfg.clear();
        assert_eq!(cfg.option_count(), 0);
        assert_eq!(cfg.version(), 1);
    }

    #[test]
    fn secrets_config_merge() {
        let mut cfg1 = SecretsConfig::new();
        cfg1.set_option("a", "1");
        let mut cfg2 = SecretsConfig::new();
        cfg2.set_option("b", "2");
        cfg1.merge(&cfg2);
        assert_eq!(cfg1.option_count(), 2);
    }

    #[test]
    fn secrets_config_disable() {
        let mut cfg = SecretsConfig::new();
        cfg.set_enabled(false);
        assert!(!cfg.is_enabled());
    }

    #[test]
    fn secrets_rate_tracker_empty() {
        let rt = SecretsRateTracker::new(1000);
        assert_eq!(rt.count(), 0);
        assert_eq!(rt.rate_per_second(), 0.0);
    }

    #[test]
    fn secrets_rate_tracker_record() {
        let mut rt = SecretsRateTracker::new(1000);
        rt.record(100);
        rt.record(200);
        rt.record(300);
        assert_eq!(rt.count(), 3);
    }

    #[test]
    fn secrets_rate_tracker_prune() {
        let mut rt = SecretsRateTracker::new(100);
        rt.record(10);
        rt.record(200);
        assert_eq!(rt.count(), 1);
    }

    #[test]
    fn secrets_validator_valid() {
        let v = SecretsValidationCollector::new();
        assert!(v.is_valid());
        assert_eq!(v.error_count(), 0);
    }

    #[test]
    fn secrets_validator_errors() {
        let mut v = SecretsValidationCollector::new();
        v.add_error("bad input");
        v.add_warning("slow");
        assert!(!v.is_valid());
        assert_eq!(v.error_count(), 1);
        assert_eq!(v.warning_count(), 1);
        assert_eq!(v.first_error(), Some("bad input"));
    }

    #[test]
    fn secrets_validator_clear() {
        let mut v = SecretsValidationCollector::new();
        v.add_error("err");
        v.clear();
        assert!(v.is_valid());
    }

    #[test]
    fn secrets_validator_merge() {
        let mut v1 = SecretsValidationCollector::new();
        v1.add_error("e1");
        let mut v2 = SecretsValidationCollector::new();
        v2.add_error("e2");
        v2.add_warning("w1");
        v1.merge(&v2);
        assert_eq!(v1.error_count(), 2);
        assert_eq!(v1.warning_count(), 1);
    }

    #[test]
    fn secrets_rate_tracker_clear() {
        let mut rt = SecretsRateTracker::new(1000);
        rt.record(100);
        rt.clear();
        assert_eq!(rt.count(), 0);
    }


    #[test]
    fn xl_metrics_empty() {
        let m = XlMetrics::new("secrets");
        assert_eq!(m.count(), 0);
        assert!((m.mean() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xl_metrics_record_and_mean() {
        let mut m = XlMetrics::new("secrets");
        m.record(10.0);
        m.record(20.0);
        m.record(30.0);
        assert_eq!(m.count(), 3);
        assert!((m.mean() - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xl_metrics_min_max() {
        let mut m = XlMetrics::new("test");
        m.record(5.0);
        m.record(15.0);
        m.record(10.0);
        assert!((m.min_val() - 5.0).abs() < f64::EPSILON);
        assert!((m.max_val() - 15.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xl_metrics_variance_and_std() {
        let mut m = XlMetrics::new("v");
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
    fn xl_metrics_percentile() {
        let mut m = XlMetrics::new("p");
        for i in 1..=100 {
            m.record(i as f64);
        }
        let p50 = m.percentile(50.0);
        assert!(p50 >= 49.0 && p50 <= 51.0);
    }

    #[test]
    fn xl_metrics_merge() {
        let mut a = XlMetrics::new("a");
        a.record(1.0);
        let mut b = XlMetrics::new("b");
        b.record(2.0);
        b.record(3.0);
        a.merge(&b);
        assert_eq!(a.count(), 3);
    }

    #[test]
    fn xl_metrics_reset() {
        let mut m = XlMetrics::new("r");
        m.record(42.0);
        m.reset();
        assert_eq!(m.count(), 0);
    }

    #[test]
    fn xl_rate_window_empty() {
        let rw = XlRateWindow::new(1000);
        assert!(rw.is_empty());
        assert_eq!(rw.window_ms(), 1000);
    }

    #[test]
    fn xl_rate_window_tick_and_rate() {
        let mut rw = XlRateWindow::new(1000);
        rw.tick(100);
        rw.tick(200);
        rw.tick(300);
        assert_eq!(rw.rate(500), 3);
        assert_eq!(rw.rate(1500), 0);
    }

    #[test]
    fn xl_lru_cache_basic() {
        let mut c = XlLruCache::new(2);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        assert_eq!(c.get("a"), Some("1".to_string()));
        c.put("c".into(), "3".into());
        assert_eq!(c.get("b"), None);
    }

    #[test]
    fn xl_lru_cache_contains_and_keys() {
        let mut c = XlLruCache::new(3);
        c.put("x".into(), "10".into());
        c.put("y".into(), "20".into());
        assert!(c.contains_key("x"));
        assert!(!c.contains_key("z"));
        assert_eq!(c.keys().len(), 2);
    }

    #[test]
    fn xl_lru_cache_remove() {
        let mut c = XlLruCache::new(3);
        c.put("k".into(), "v".into());
        assert_eq!(c.remove("k"), Some("v".to_string()));
        assert!(c.is_empty());
        assert_eq!(c.remove("k"), None);
    }

    #[test]
    fn xl_metrics_sum() {
        let mut m = XlMetrics::new("s");
        m.record(1.0);
        m.record(2.0);
        m.record(3.0);
        assert!((m.sum() - 6.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xl_metrics_label() {
        let m = XlMetrics::new("my_label");
        assert_eq!(m.label(), "my_label");
    }

    #[test]
    fn xl_lru_cache_clear() {
        let mut c = XlLruCache::new(5);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    #[test]
    fn xb_ring_buffer_30_push_and_len() {
        let mut rb = super::XbRingBuffer30::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_30_overwrite() {
        let mut rb = super::XbRingBuffer30::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_30_get_out_of_bounds() {
        let rb = super::XbRingBuffer30::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_30_drain_all() {
        let mut rb = super::XbRingBuffer30::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_30_peek_front_back() {
        let mut rb = super::XbRingBuffer30::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_30_clear() {
        let mut rb = super::XbRingBuffer30::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_30_capacity() {
        let rb = super::XbRingBuffer30::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_30_basic() {
        let h = super::xb_fnv1a_30(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_30(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_30_different_inputs() {
        let h1 = super::xb_fnv1a_30(b"abc");
        let h2 = super::xb_fnv1a_30(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_30_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_30(&data);
        let dec = super::xb_rle_decode_30(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_30_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_30(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_30(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_30_values() {
        assert!((super::xb_clamp_30(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_30(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_30(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_30_values() {
        assert!((super::xb_lerp_30(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_30(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_30(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_30_wrap_around_twice() {
        let mut rb = super::XbRingBuffer30::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
    }


    // ---- xc_ pool / scheduler tests – block 156 ----

    #[test]
    fn xc_156_pool_new_empty() {
        let pool: super::Xc156Pool<i32> = super::Xc156Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_156_pool_release_acquire() {
        let mut pool = super::Xc156Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_156_pool_acquire_empty() {
        let mut pool: super::Xc156Pool<i32> = super::Xc156Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_156_pool_full() {
        let mut pool = super::Xc156Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_156_pool_drain() {
        let mut pool = super::Xc156Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_156_pool_stats() {
        let mut pool = super::Xc156Pool::new(8);
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
    fn xc_156_pool_clear() {
        let mut pool = super::Xc156Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_156_pool_shrink() {
        let mut pool = super::Xc156Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_156_pool_default() {
        let pool: super::Xc156Pool<String> = super::Xc156Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_156_pool_extend() {
        let mut pool = super::Xc156Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_156_pool_retain() {
        let mut pool = super::Xc156Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_156_scheduler_round_robin() {
        let mut sched = super::Xc156Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_156_scheduler_empty() {
        let mut sched = super::Xc156Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_156_scheduler_reset() {
        let mut sched = super::Xc156Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_156_scheduler_add_remove() {
        let mut sched = super::Xc156Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_156_scheduler_targets() {
        let sched = super::Xc156Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_156_hash_empty() {
        assert_eq!(super::xc_156_hash(b""), 5381);
    }

    #[test]
    fn xc_156_hash_data() {
        let h = super::xc_156_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_156_hash(b"hello"), h);
    }

    #[test]
    fn xc_156_reverse_str() {
        assert_eq!(super::xc_156_reverse("abc"), "cba");
        assert_eq!(super::xc_156_reverse(""), "");
    }


    #[test]
    fn xe_42_pipeline_empty() {
        let p = super::Xe42Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_42_pipeline_parse_stage() {
        let p = super::Xe42Pipeline::new()
            .add_parse(super::xe_42_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_42_pipeline_transform_double() {
        let p = super::Xe42Pipeline::new()
            .add_transform(super::xe_42_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_42_pipeline_validate_reverse() {
        let p = super::Xe42Pipeline::new()
            .add_validate(super::xe_42_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_42_pipeline_emit_filter() {
        let p = super::Xe42Pipeline::new()
            .add_emit(super::xe_42_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_42_pipeline_multi_stage() {
        let p = super::Xe42Pipeline::new()
            .add_parse(super::xe_42_pipeline_identity)
            .add_transform(super::xe_42_pipeline_double)
            .add_validate(super::xe_42_pipeline_reverse)
            .add_emit(super::xe_42_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_42_pipeline_error_propagation() {
        let p = super::Xe42Pipeline::new()
            .add_parse(super::xe_42_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe42Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_42_pipeline_compose() {
        let p1 = super::Xe42Pipeline::new()
            .add_parse(super::xe_42_pipeline_identity);
        let p2 = super::Xe42Pipeline::new()
            .add_transform(super::xe_42_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_42_pipeline_error_display() {
        let e = super::Xe42PipelineError {
            stage: super::Xe42Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_42_cache_put_get() {
        let mut c = super::Xe42Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_42_cache_miss() {
        let mut c: super::Xe42Cache<&str, i32> = super::Xe42Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_42_cache_ttl_expiry() {
        let mut c = super::Xe42Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_42_cache_evict() {
        let mut c = super::Xe42Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_42_cache_capacity() {
        let mut c = super::Xe42Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_42_cache_stats() {
        let mut c = super::Xe42Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_42_cache_clear() {
        let mut c = super::Xe42Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // -- xg_10 graph tests ------------------------------------------------

    #[test]
    fn xg_10_graph_empty() {
        let g = super::Xg10Graph::new();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn xg_10_graph_add_node() {
        let mut g = super::Xg10Graph::new();
        g.add_node(1);
        g.add_node(2);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_10_graph_add_edge() {
        let mut g = super::Xg10Graph::new();
        g.add_edge(0, 1);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_10_graph_neighbors() {
        let mut g = super::Xg10Graph::new();
        g.add_edge(0, 1);
        g.add_edge(0, 2);
        assert_eq!(g.neighbors(0).len(), 2);
    }

    #[test]
    fn xg_10_graph_has_path() {
        let mut g = super::Xg10Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(g.has_path(0, 2));
        assert!(!g.has_path(2, 0));
    }

    #[test]
    fn xg_10_graph_self_path() {
        let g = super::Xg10Graph::new();
        assert!(g.has_path(5, 5));
    }

    #[test]
    fn xg_10_graph_topo_sort() {
        let mut g = super::Xg10Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        let sorted = g.topological_sort().unwrap();
        let pos: std::collections::HashMap<usize, usize> =
            sorted.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&1] < pos[&2]);
    }

    #[test]
    fn xg_10_graph_cycle_detect_false() {
        let mut g = super::Xg10Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(!g.cycle_detect());
    }

    #[test]
    fn xg_10_graph_cycle_detect_true() {
        let mut g = super::Xg10Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        g.add_edge(2, 0);
        assert!(g.cycle_detect());
    }

    // -- xg_10 heap tests -------------------------------------------------

    #[test]
    fn xg_10_heap_empty() {
        let h: super::Xg10Heap<i32> = super::Xg10Heap::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn xg_10_heap_push_pop() {
        let mut h = super::Xg10Heap::new();
        h.push(3);
        h.push(1);
        h.push(2);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(3));
    }

    #[test]
    fn xg_10_heap_peek() {
        let mut h = super::Xg10Heap::new();
        h.push(5);
        h.push(2);
        assert_eq!(h.peek(), Some(&2));
    }

    #[test]
    fn xg_10_heap_drain_sorted() {
        let mut h = super::Xg10Heap::new();
        for v in [4, 1, 7, 2, 9] { h.push(v); }
        assert_eq!(h.drain_sorted(), vec![1, 2, 4, 7, 9]);
        assert!(h.is_empty());
    }

    #[test]
    fn xg_10_heap_merge() {
        let mut a = super::Xg10Heap::new();
        let mut b = super::Xg10Heap::new();
        a.push(5); a.push(3);
        b.push(4); b.push(1);
        a.merge(&mut b);
        assert_eq!(a.len(), 4);
        assert_eq!(a.pop(), Some(1));
    }

    #[test]
    fn xg_10_heap_default() {
        let h: super::Xg10Heap<u64> = Default::default();
        assert!(h.is_empty());
    }

    #[test]
    fn xg_10_graph_default() {
        let g: super::Xg10Graph = Default::default();
        assert_eq!(g.node_count(), 0);
    }


    #[test]
    fn xh155_skip_insert_contains() {
        let mut sl = super::Xh155SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh155_skip_remove() {
        let mut sl = super::Xh155SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh155_skip_len() {
        let mut sl = super::Xh155SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh155_skip_range_query() {
        let mut sl = super::Xh155SkipList::xh_new(4);
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
    fn xh155_skip_floor_ceiling() {
        let mut sl = super::Xh155SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh155_skip_rank() {
        let mut sl = super::Xh155SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh155_skip_empty() {
        let sl = super::Xh155SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh155_skip_duplicates() {
        let mut sl = super::Xh155SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh155_bitset_set_test() {
        let mut bs = super::Xh155BitSet::xh_new(256);
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
    fn xh155_bitset_clear_count() {
        let mut bs = super::Xh155BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh155_bitset_and_or_xor() {
        let mut a = super::Xh155BitSet::xh_new(128);
        let mut b = super::Xh155BitSet::xh_new(128);
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
    fn xh155_bitset_iter_ones() {
        let mut bs = super::Xh155BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh155_bitset_first_last() {
        let mut bs = super::Xh155BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh155_bitset_empty() {
        let bs = super::Xh155BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi155_deque_push_pop_back() {
        let mut dq = super::Xi155Deque::xi_new(4);
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
    fn xi155_deque_push_pop_front() {
        let mut dq = super::Xi155Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi155_deque_mixed_ops() {
        let mut dq = super::Xi155Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi155_deque_get_and_split() {
        let mut dq = super::Xi155Deque::xi_new(8);
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
    fn xi155_deque_rotate_left() {
        let mut dq = super::Xi155Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi155_deque_rotate_right() {
        let mut dq = super::Xi155Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi155_deque_grow() {
        let mut dq = super::Xi155Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi155_deque_empty() {
        let dq = super::Xi155Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi155_interval_tree_insert_query() {
        let mut tree = super::Xi155IntervalTree::xi_new();
        tree.xi_insert(super::Xi155Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi155Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi155Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi155_interval_tree_overlap() {
        let mut tree = super::Xi155IntervalTree::xi_new();
        tree.xi_insert(super::Xi155Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi155Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi155Interval::xi_new(12, 20));
        let q = super::Xi155Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi155_interval_tree_remove() {
        let mut tree = super::Xi155IntervalTree::xi_new();
        tree.xi_insert(super::Xi155Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi155Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi155_interval_tree_gaps() {
        let mut tree = super::Xi155IntervalTree::xi_new();
        tree.xi_insert(super::Xi155Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi155Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi155Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi155Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi155Interval::xi_new(8, 10));
    }

    #[test]
    fn xi155_interval_tree_merge() {
        let mut tree = super::Xi155IntervalTree::xi_new();
        tree.xi_insert(super::Xi155Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi155Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi155Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi155Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi155Interval::xi_new(10, 15));
    }

    #[test]
    fn xi155_interval_tree_all() {
        let mut tree = super::Xi155IntervalTree::xi_new();
        tree.xi_insert(super::Xi155Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi155Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi155_interval_tree_empty() {
        let tree = super::Xi155IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi155_interval_tree_contains_point() {
        let iv = super::Xi155Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 155) ---

    #[test]
    fn xj_155_uf_make_and_find() {
        let mut uf = super::Xj155UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_155_uf_union_connected() {
        let mut uf = super::Xj155UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_155_uf_component_count() {
        let mut uf = super::Xj155UnionFind::xj_new();
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
    fn xj_155_uf_component_size() {
        let mut uf = super::Xj155UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_155_uf_largest_component() {
        let mut uf = super::Xj155UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_155_uf_many_elements() {
        let mut uf = super::Xj155UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_155_uf_separate_components() {
        let mut uf = super::Xj155UnionFind::xj_new();
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
    fn xj_155_uf_path_compression() {
        let mut uf = super::Xj155UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_155_bt_insert_get() {
        let mut bt = super::Xj155BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_155_bt_contains_len() {
        let mut bt = super::Xj155BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_155_bt_replace() {
        let mut bt = super::Xj155BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_155_bt_remove() {
        let mut bt = super::Xj155BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_155_bt_keys_values() {
        let mut bt = super::Xj155BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_155_bt_range() {
        let mut bt = super::Xj155BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_155_bt_min_max() {
        let mut bt = super::Xj155BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_155_bt_many_inserts() {
        let mut bt = super::Xj155BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_155 segment tree tests ---

    #[test]
    fn xk_155_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk155SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_155_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk155SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_155_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk155SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_155_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk155SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_155_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk155SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_155_st_single_element() {
        let data = vec![42];
        let st = super::Xk155SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_155_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk155SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_155_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk155SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_155 disjoint intervals tests ---

    #[test]
    fn xk_155_di_add_and_count() {
        let mut di = super::Xk155DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_155_di_merge_overlap() {
        let mut di = super::Xk155DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_155_di_contains() {
        let mut di = super::Xk155DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_155_di_remove() {
        let mut di = super::Xk155DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_155_di_covered_length() {
        let mut di = super::Xk155DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_155_di_gaps() {
        let mut di = super::Xk155DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_155_di_merge_adjacent() {
        let mut di = super::Xk155DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_155_di_empty() {
        let di = super::Xk155DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_155_rope_new_empty() {
        let rope = super::Xl155Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_155_rope_from_str() {
        let rope = super::Xl155Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_155_rope_insert_at() {
        let mut rope = super::Xl155Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_155_rope_delete_range() {
        let mut rope = super::Xl155Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_155_rope_char_at() {
        let rope = super::Xl155Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_155_rope_split_concat() {
        let rope = super::Xl155Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_155_rope_line_count() {
        let rope = super::Xl155Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_155_rope_line_at() {
        let rope = super::Xl155Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_155_sa_build_and_search() {
        let sa = super::Xl155SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_155_sa_count() {
        let sa = super::Xl155SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_155_sa_longest_repeated() {
        let sa = super::Xl155SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_155_sa_all_positions() {
        let sa = super::Xl155SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_155_sa_len() {
        let sa = super::Xl155SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_155_sa_empty() {
        let sa = super::Xl155SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_155_rope_slice() {
        let rope = super::Xl155Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_155_sa_search_start() {
        let sa = super::Xl155SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }

    #[test]
    fn xm_155_sparse_set_get() {
        let mut m = super::Xm155MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 1, 5.0);
        assert!((m.xm_get(0, 1) - 5.0).abs() < f64::EPSILON);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_155_sparse_row_col() {
        let mut m = super::Xm155MatrixSparse::xm_new(4, 4);
        m.xm_set(1, 2, 3.0);
        m.xm_set(1, 3, 7.0);
        let row = m.xm_row(1);
        assert_eq!(row.len(), 2);
        let col = m.xm_col(2);
        assert_eq!(col.len(), 1);
    }

    #[test]
    fn xm_155_sparse_transpose() {
        let mut m = super::Xm155MatrixSparse::xm_new(2, 3);
        m.xm_set(0, 2, 9.0);
        let t = m.xm_transpose();
        assert!((t.xm_get(2, 0) - 9.0).abs() < f64::EPSILON);
        assert_eq!(t.xm_dims(), (3, 2));
    }

    #[test]
    fn xm_155_sparse_multiply_vec() {
        let mut m = super::Xm155MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        let result = m.xm_multiply_vec(&[3.0, 4.0]);
        assert!((result[0] - 3.0).abs() < f64::EPSILON);
        assert!((result[1] - 8.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_155_sparse_nnz_density() {
        let mut m = super::Xm155MatrixSparse::xm_new(10, 10);
        m.xm_set(0, 0, 1.0);
        m.xm_set(5, 5, 2.0);
        assert_eq!(m.xm_nnz(), 2);
        assert!((m.xm_density() - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_155_sparse_clear() {
        let mut m = super::Xm155MatrixSparse::xm_new(3, 3);
        m.xm_set(0, 0, 1.0);
        m.xm_set(1, 1, 2.0);
        m.xm_clear();
        assert_eq!(m.xm_nnz(), 0);
        assert!((m.xm_get(0, 0) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_155_sparse_overwrite_zero() {
        let mut m = super::Xm155MatrixSparse::xm_new(2, 2);
        m.xm_set(0, 0, 5.0);
        assert_eq!(m.xm_nnz(), 1);
        m.xm_set(0, 0, 0.0);
        assert_eq!(m.xm_nnz(), 0);
    }

    #[test]
    fn xm_155_tokenizer_basic() {
        let t = super::Xm155Tokenizer::xm_new("hello world foo");
        let tokens = t.xm_tokenize();
        assert_eq!(tokens, vec!["hello", "world", "foo"]);
    }

    #[test]
    fn xm_155_tokenizer_count() {
        let t = super::Xm155Tokenizer::xm_new("a b c d e");
        assert_eq!(t.xm_token_count(), 5);
    }

    #[test]
    fn xm_155_tokenizer_unique() {
        let t = super::Xm155Tokenizer::xm_new("a b a c b");
        let u = t.xm_unique_tokens();
        assert_eq!(u.len(), 3);
    }

    #[test]
    fn xm_155_tokenizer_frequency() {
        let t = super::Xm155Tokenizer::xm_new("x y x x y z");
        let freq = t.xm_frequency_map();
        assert_eq!(freq.get("x"), Some(&3));
        assert_eq!(freq.get("y"), Some(&2));
        assert_eq!(freq.get("z"), Some(&1));
    }

    #[test]
    fn xm_155_tokenizer_delimiter() {
        let t = super::Xm155Tokenizer::xm_new("a,b,,c");
        let parts = t.xm_split_by_delimiter(',');
        assert_eq!(parts, vec!["a", "b", "c"]);
    }

    #[test]
    fn xm_155_tokenizer_whitespace() {
        let t = super::Xm155Tokenizer::xm_new("one  two  three");
        let parts = t.xm_split_by_whitespace();
        assert_eq!(parts, vec!["one", "two", "three"]);
    }

    #[test]
    fn xm_155_tokenizer_empty() {
        let t = super::Xm155Tokenizer::xm_new("");
        assert!(t.xm_is_empty());
        assert_eq!(t.xm_token_count(), 0);
    }


    // ---- Fenwick tree tests — crate 155 ----

    #[test]
    fn xn_155_fenwick_prefix_sum() {
        let mut ft = super::Xn155Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, (i + 1) as i64); }
        assert_eq!(ft.xn_prefix_sum(0), 1);
        assert_eq!(ft.xn_prefix_sum(4), 15);
    }

    #[test]
    fn xn_155_fenwick_range_sum() {
        let mut ft = super::Xn155Fenwick::xn_new(6);
        for i in 0..6 { ft.xn_update(i, (i * 2) as i64); }
        assert_eq!(ft.xn_range_sum(1, 3), 2 + 4 + 6);
    }

    #[test]
    fn xn_155_fenwick_point_query() {
        let mut ft = super::Xn155Fenwick::xn_new(4);
        ft.xn_update(2, 7);
        assert_eq!(ft.xn_point_query(2), 7);
        assert_eq!(ft.xn_point_query(0), 0);
    }

    #[test]
    fn xn_155_fenwick_len() {
        let ft = super::Xn155Fenwick::xn_new(10);
        assert_eq!(ft.xn_len(), 10);
    }

    #[test]
    fn xn_155_fenwick_multiple_updates() {
        let mut ft = super::Xn155Fenwick::xn_new(3);
        ft.xn_update(0, 5);
        ft.xn_update(0, 3);
        assert_eq!(ft.xn_point_query(0), 8);
    }

    #[test]
    fn xn_155_fenwick_single_element() {
        let mut ft = super::Xn155Fenwick::xn_new(1);
        ft.xn_update(0, 42);
        assert_eq!(ft.xn_prefix_sum(0), 42);
        assert_eq!(ft.xn_range_sum(0, 0), 42);
    }

    #[test]
    fn xn_155_fenwick_find_kth() {
        let mut ft = super::Xn155Fenwick::xn_new(5);
        for i in 0..5 { ft.xn_update(i, 1); }
        assert_eq!(ft.xn_find_kth(3), Some(2));
    }

    #[test]
    fn xn_155_fenwick_negative_delta() {
        let mut ft = super::Xn155Fenwick::xn_new(3);
        ft.xn_update(1, 10);
        ft.xn_update(1, -4);
        assert_eq!(ft.xn_point_query(1), 6);
    }

    // ---- AVL tree tests — crate 155 ----

    #[test]
    fn xn_155_avl_insert_get() {
        let mut m = super::Xn155AVL::xn_new();
        m.xn_insert(3, "c");
        m.xn_insert(1, "a");
        m.xn_insert(2, "b");
        assert_eq!(m.xn_get(&2), Some(&"b"));
        assert_eq!(m.xn_len(), 3);
    }

    #[test]
    fn xn_155_avl_remove() {
        let mut m = super::Xn155AVL::xn_new();
        m.xn_insert(1, 10);
        m.xn_insert(2, 20);
        assert!(m.xn_remove(&1));
        assert!(!m.xn_contains(&1));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_155_avl_in_order() {
        let mut m = super::Xn155AVL::xn_new();
        for k in [5, 3, 7, 1, 4] { m.xn_insert(k, k * 10); }
        let keys: Vec<_> = m.xn_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xn_155_avl_min_max() {
        let mut m = super::Xn155AVL::xn_new();
        for k in [10, 5, 20, 3, 15] { m.xn_insert(k, k); }
        assert_eq!(m.xn_min(), Some(&3));
        assert_eq!(m.xn_max(), Some(&20));
    }

    #[test]
    fn xn_155_avl_floor_ceiling() {
        let mut m = super::Xn155AVL::xn_new();
        for k in [10, 20, 30] { m.xn_insert(k, k); }
        assert_eq!(m.xn_floor(&15), Some(&10));
        assert_eq!(m.xn_ceiling(&15), Some(&20));
    }

    #[test]
    fn xn_155_avl_height_balanced() {
        let mut m = super::Xn155AVL::xn_new();
        for k in 0..31 { m.xn_insert(k, k); }
        assert!(m.xn_height() <= 7);
    }

    #[test]
    fn xn_155_avl_overwrite() {
        let mut m = super::Xn155AVL::xn_new();
        m.xn_insert(1, "old");
        m.xn_insert(1, "new");
        assert_eq!(m.xn_get(&1), Some(&"new"));
        assert_eq!(m.xn_len(), 1);
    }

    #[test]
    fn xn_155_avl_empty() {
        let m: super::Xn155AVL<i32, i32> = super::Xn155AVL::xn_new();
        assert_eq!(m.xn_len(), 0);
        assert_eq!(m.xn_min(), None);
        assert_eq!(m.xn_max(), None);
        assert_eq!(m.xn_height(), 0);
    }

    // --- Xo155RedBlack tests ---

    #[test]
    fn xo_155_rb_insert_and_get() {
        let mut tree = super::Xo155RedBlack::xo_new();
        tree.xo_insert(10, "ten");
        tree.xo_insert(20, "twenty");
        tree.xo_insert(5, "five");
        assert_eq!(tree.xo_get(&10), Some(&"ten"));
        assert_eq!(tree.xo_get(&20), Some(&"twenty"));
        assert_eq!(tree.xo_get(&5), Some(&"five"));
        assert_eq!(tree.xo_get(&99), None);
    }

    #[test]
    fn xo_155_rb_len_and_empty() {
        let mut tree = super::Xo155RedBlack::<i32, i32>::xo_new();
        assert!(tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 0);
        tree.xo_insert(1, 100);
        tree.xo_insert(2, 200);
        assert!(!tree.xo_is_empty());
        assert_eq!(tree.xo_len(), 2);
    }

    #[test]
    fn xo_155_rb_min_max() {
        let mut tree = super::Xo155RedBlack::xo_new();
        for k in [30, 10, 50, 20, 40] {
            tree.xo_insert(k, k * 10);
        }
        assert_eq!(tree.xo_min(), Some(&10));
        assert_eq!(tree.xo_max(), Some(&50));
    }

    #[test]
    fn xo_155_rb_contains() {
        let mut tree = super::Xo155RedBlack::xo_new();
        tree.xo_insert(42, "answer");
        assert!(tree.xo_contains(&42));
        assert!(!tree.xo_contains(&43));
    }

    #[test]
    fn xo_155_rb_remove() {
        let mut tree = super::Xo155RedBlack::xo_new();
        tree.xo_insert(1, "a");
        tree.xo_insert(2, "b");
        tree.xo_insert(3, "c");
        assert_eq!(tree.xo_remove(&2), Some("b"));
        assert_eq!(tree.xo_len(), 2);
        assert!(!tree.xo_contains(&2));
        assert_eq!(tree.xo_remove(&99), None);
    }

    #[test]
    fn xo_155_rb_in_order() {
        let mut tree = super::Xo155RedBlack::xo_new();
        for k in [5, 3, 7, 1, 4] {
            tree.xo_insert(k, k);
        }
        let keys: Vec<i32> = tree.xo_in_order().iter().map(|(k, _)| *k).collect();
        assert_eq!(keys, vec![1, 3, 4, 5, 7]);
    }

    #[test]
    fn xo_155_rb_black_height() {
        let mut tree = super::Xo155RedBlack::xo_new();
        for k in 0..15 {
            tree.xo_insert(k, k);
        }
        let bh = tree.xo_black_height();
        assert!(bh >= 2 && bh <= 6, "black height {bh} out of range");
    }

    #[test]
    fn xo_155_rb_overwrite() {
        let mut tree = super::Xo155RedBlack::xo_new();
        tree.xo_insert(1, "old");
        tree.xo_insert(1, "new");
        assert_eq!(tree.xo_get(&1), Some(&"new"));
        assert_eq!(tree.xo_len(), 1);
    }

    // --- Xo155ConsistentHash tests ---

    #[test]
    fn xo_155_ch_add_and_count() {
        let mut ring = super::Xo155ConsistentHash::xo_new(100);
        ring.xo_add_node("server-a");
        ring.xo_add_node("server-b");
        assert_eq!(ring.xo_node_count(), 2);
        assert_eq!(ring.xo_virtual_nodes(), 200);
    }

    #[test]
    fn xo_155_ch_remove_node() {
        let mut ring = super::Xo155ConsistentHash::xo_new(50);
        ring.xo_add_node("alpha");
        ring.xo_add_node("beta");
        ring.xo_remove_node("alpha");
        assert_eq!(ring.xo_node_count(), 1);
        assert_eq!(ring.xo_virtual_nodes(), 50);
    }

    #[test]
    fn xo_155_ch_get_node() {
        let mut ring = super::Xo155ConsistentHash::xo_new(50);
        ring.xo_add_node("node-1");
        let result = ring.xo_get_node("some-key");
        assert_eq!(result, Some("node-1"));
    }

    #[test]
    fn xo_155_ch_empty_ring() {
        let ring = super::Xo155ConsistentHash::xo_new(10);
        assert_eq!(ring.xo_get_node("key"), None);
        assert_eq!(ring.xo_node_count(), 0);
    }

    #[test]
    fn xo_155_ch_distribution() {
        let mut ring = super::Xo155ConsistentHash::xo_new(100);
        ring.xo_add_node("s1");
        ring.xo_add_node("s2");
        let keys: Vec<&str> = vec!["k1", "k2", "k3", "k4", "k5", "k6"];
        let dist = ring.xo_key_distribution(&keys);
        let total: usize = dist.values().sum();
        assert_eq!(total, 6);
    }

    #[test]
    fn xo_155_ch_rebalance() {
        let mut ring = super::Xo155ConsistentHash::xo_new(100);
        ring.xo_add_node("n1");
        ring.xo_add_node("n2");
        ring.xo_add_node("n3");
        let rf = ring.xo_rebalance_factor();
        assert!(rf >= 0.0, "rebalance factor should be non-negative");
    }

    #[test]
    fn xo_155_ch_virtual_nodes() {
        let mut ring = super::Xo155ConsistentHash::xo_new(75);
        ring.xo_add_node("host1");
        ring.xo_add_node("host2");
        assert_eq!(ring.xo_virtual_nodes(), 150);
    }

    #[test]
    fn xo_155_ch_consistent_lookup() {
        let mut ring = super::Xo155ConsistentHash::xo_new(50);
        ring.xo_add_node("srv-a");
        ring.xo_add_node("srv-b");
        let first = ring.xo_get_node("stable-key").unwrap().to_string();
        let second = ring.xo_get_node("stable-key").unwrap().to_string();
        assert_eq!(first, second, "same key must map to same node");
    }


    #[test]
    fn xp_155_splay_insert_get() {
        let mut t = super::Xp155SplayTree::xp_new();
        t.xp_insert(10, "ten");
        t.xp_insert(20, "twenty");
        t.xp_insert(5, "five");
        assert_eq!(t.xp_get(&10), Some(&"ten"));
        assert_eq!(t.xp_get(&20), Some(&"twenty"));
        assert_eq!(t.xp_get(&5), Some(&"five"));
    }

    #[test]
    fn xp_155_splay_remove() {
        let mut t = super::Xp155SplayTree::xp_new();
        t.xp_insert(1, "a");
        t.xp_insert(2, "b");
        t.xp_insert(3, "c");
        assert_eq!(t.xp_remove(&2), Some("b"));
        assert_eq!(t.xp_len(), 2);
        assert_eq!(t.xp_get(&2), None);
    }

    #[test]
    fn xp_155_splay_count_increases() {
        let mut t = super::Xp155SplayTree::xp_new();
        t.xp_insert(1, 100);
        t.xp_insert(2, 200);
        let before = t.xp_splay_count();
        t.xp_get(&1);
        assert!(t.xp_splay_count() > before);
    }

    #[test]
    fn xp_155_splay_depth() {
        let mut t = super::Xp155SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_depth(), 0);
        t.xp_insert(1, 1);
        assert!(t.xp_depth() >= 1);
        t.xp_insert(2, 2);
        t.xp_insert(3, 3);
        assert!(t.xp_depth() >= 1);
    }

    #[test]
    fn xp_155_splay_len_empty() {
        let t = super::Xp155SplayTree::<String, u8>::xp_new();
        assert!(t.xp_is_empty());
        assert_eq!(t.xp_len(), 0);
    }

    #[test]
    fn xp_155_splay_min_max() {
        let mut t = super::Xp155SplayTree::xp_new();
        assert!(t.xp_min().is_none());
        assert!(t.xp_max().is_none());
        t.xp_insert(30, "x");
        t.xp_insert(10, "y");
        t.xp_insert(50, "z");
        assert_eq!(t.xp_min(), Some(&10));
        assert_eq!(t.xp_max(), Some(&50));
    }

    #[test]
    fn xp_155_splay_overwrite() {
        let mut t = super::Xp155SplayTree::xp_new();
        assert!(t.xp_insert(5, "old").is_none());
        assert_eq!(t.xp_insert(5, "new"), Some("old"));
        assert_eq!(t.xp_get(&5), Some(&"new"));
        assert_eq!(t.xp_len(), 1);
    }

    #[test]
    fn xp_155_splay_remove_missing() {
        let mut t = super::Xp155SplayTree::<i32, i32>::xp_new();
        assert_eq!(t.xp_remove(&99), None);
        t.xp_insert(1, 1);
        assert_eq!(t.xp_remove(&99), None);
        assert_eq!(t.xp_len(), 1);
    }


    // ---- xq_155 treap tests ----
    #[test]
    fn xq_155_treap_empty() {
        let t = super::Xq155Treap::<i32, i32>::xq_new();
        assert_eq!(t.xq_len(), 0);
        assert!(t.xq_min().is_none());
        assert!(t.xq_max().is_none());
    }

    #[test]
    fn xq_155_treap_insert_get() {
        let mut t = super::Xq155Treap::xq_new();
        assert!(t.xq_insert(10, "ten").is_none());
        assert_eq!(t.xq_get(&10), Some(&"ten"));
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_155_treap_overwrite() {
        let mut t = super::Xq155Treap::xq_new();
        t.xq_insert(5, "old");
        assert_eq!(t.xq_insert(5, "new"), Some("old"));
        assert_eq!(t.xq_get(&5), Some(&"new"));
    }

    #[test]
    fn xq_155_treap_remove() {
        let mut t = super::Xq155Treap::xq_new();
        t.xq_insert(1, "a");
        t.xq_insert(2, "b");
        assert_eq!(t.xq_remove(&1), Some("a"));
        assert!(t.xq_get(&1).is_none());
        assert_eq!(t.xq_len(), 1);
    }

    #[test]
    fn xq_155_treap_min_max() {
        let mut t = super::Xq155Treap::xq_new();
        t.xq_insert(30, "x");
        t.xq_insert(10, "y");
        t.xq_insert(50, "z");
        assert_eq!(t.xq_min(), Some(&10));
        assert_eq!(t.xq_max(), Some(&50));
    }

    #[test]
    fn xq_155_treap_rank() {
        let mut t = super::Xq155Treap::xq_new();
        for i in 0..5 { t.xq_insert(i * 10, i); }
        assert_eq!(t.xq_rank(&20), 2);
        assert_eq!(t.xq_rank(&0), 0);
    }

    #[test]
    fn xq_155_treap_kth() {
        let mut t = super::Xq155Treap::xq_new();
        for i in [30, 10, 50, 20, 40] { t.xq_insert(i, i); }
        assert_eq!(t.xq_kth_element(0), Some(&10));
        assert_eq!(t.xq_kth_element(4), Some(&50));
    }

    #[test]
    fn xq_155_treap_in_order() {
        let mut t = super::Xq155Treap::xq_new();
        for i in [5, 3, 8, 1, 4] { t.xq_insert(i, i); }
        assert_eq!(t.xq_in_order(), vec![1, 3, 4, 5, 8]);
    }

    // ---- xq_155 VEB tree tests ----
    #[test]
    fn xq_155_veb_empty() {
        let v = super::Xq155VEBTree::xq_new(16);
        assert!(v.xq_min().is_none());
        assert!(v.xq_max().is_none());
        assert_eq!(v.xq_count(), 0);
    }

    #[test]
    fn xq_155_veb_insert_contains() {
        let mut v = super::Xq155VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        assert!(v.xq_contains(5));
        assert!(v.xq_contains(10));
        assert!(!v.xq_contains(7));
    }

    #[test]
    fn xq_155_veb_min_max() {
        let mut v = super::Xq155VEBTree::xq_new(16);
        v.xq_insert(3);
        v.xq_insert(12);
        v.xq_insert(7);
        assert_eq!(v.xq_min(), Some(3));
        assert_eq!(v.xq_max(), Some(12));
    }

    #[test]
    fn xq_155_veb_delete() {
        let mut v = super::Xq155VEBTree::xq_new(16);
        v.xq_insert(5);
        v.xq_insert(10);
        v.xq_delete(5);
        assert!(!v.xq_contains(5));
        assert!(v.xq_contains(10));
    }

    #[test]
    fn xq_155_veb_successor() {
        let mut v = super::Xq155VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_successor(2), Some(5));
        assert_eq!(v.xq_successor(5), Some(9));
    }

    #[test]
    fn xq_155_veb_predecessor() {
        let mut v = super::Xq155VEBTree::xq_new(16);
        v.xq_insert(2);
        v.xq_insert(5);
        v.xq_insert(9);
        assert_eq!(v.xq_predecessor(9), Some(5));
        assert_eq!(v.xq_predecessor(5), Some(2));
    }

    #[test]
    fn xq_155_veb_count() {
        let mut v = super::Xq155VEBTree::xq_new(16);
        v.xq_insert(1);
        v.xq_insert(3);
        v.xq_insert(7);
        assert!(v.xq_count() >= 2);
    }

    #[test]
    fn xq_155_veb_duplicate_insert() {
        let mut v = super::Xq155VEBTree::xq_new(16);
        v.xq_insert(4);
        let c1 = v.xq_count();
        v.xq_insert(4);
        assert_eq!(v.xq_count(), c1);
    }


    #[test]
    fn xr_155_kdtree_empty() {
        let tree = super::Xr155KDTree::xr_new();
        assert!(tree.xr_is_empty());
        assert_eq!(tree.xr_len(), 0);
    }

    #[test]
    fn xr_155_kdtree_insert_one() {
        let mut tree = super::Xr155KDTree::xr_new();
        tree.xr_insert(super::Xr155KDPoint::xr_new(1.0, 2.0));
        assert_eq!(tree.xr_len(), 1);
        assert!(!tree.xr_is_empty());
    }

    #[test]
    fn xr_155_kdtree_insert_multiple() {
        let mut tree = super::Xr155KDTree::xr_new();
        for i in 0..5 {
            tree.xr_insert(super::Xr155KDPoint::xr_new(i as f64, (i * 2) as f64));
        }
        assert_eq!(tree.xr_len(), 5);
    }

    #[test]
    fn xr_155_kdtree_nearest_neighbor() {
        let mut tree = super::Xr155KDTree::xr_new();
        tree.xr_insert(super::Xr155KDPoint::xr_new(0.0, 0.0));
        tree.xr_insert(super::Xr155KDPoint::xr_new(10.0, 10.0));
        let nn = tree.xr_nearest_neighbor(&super::Xr155KDPoint::xr_new(1.0, 1.0)).unwrap();
        assert!((nn.xr_x - 0.0).abs() < 1e-9);
        assert!((nn.xr_y - 0.0).abs() < 1e-9);
    }

    #[test]
    fn xr_155_kdtree_nn_empty() {
        let tree = super::Xr155KDTree::xr_new();
        assert!(tree.xr_nearest_neighbor(&super::Xr155KDPoint::xr_new(0.0, 0.0)).is_none());
    }

    #[test]
    fn xr_155_kdtree_range_search() {
        let mut tree = super::Xr155KDTree::xr_new();
        tree.xr_insert(super::Xr155KDPoint::xr_new(1.0, 1.0));
        tree.xr_insert(super::Xr155KDPoint::xr_new(5.0, 5.0));
        tree.xr_insert(super::Xr155KDPoint::xr_new(9.0, 9.0));
        let result = tree.xr_range_search(0.0, 0.0, 6.0, 6.0);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn xr_155_kdtree_range_empty() {
        let mut tree = super::Xr155KDTree::xr_new();
        tree.xr_insert(super::Xr155KDPoint::xr_new(1.0, 1.0));
        let result = tree.xr_range_search(5.0, 5.0, 10.0, 10.0);
        assert!(result.is_empty());
    }

    #[test]
    fn xr_155_kdtree_all_points() {
        let mut tree = super::Xr155KDTree::xr_new();
        tree.xr_insert(super::Xr155KDPoint::xr_new(3.0, 4.0));
        tree.xr_insert(super::Xr155KDPoint::xr_new(7.0, 8.0));
        let pts = tree.xr_all_points();
        assert_eq!(pts.len(), 2);
    }

    #[test]
    fn xr_155_kdtree_depth() {
        let mut tree = super::Xr155KDTree::xr_new();
        assert_eq!(tree.xr_depth(), 0);
        tree.xr_insert(super::Xr155KDPoint::xr_new(5.0, 5.0));
        assert_eq!(tree.xr_depth(), 1);
    }

    #[test]
    fn xr_155_kdtree_bounding_box() {
        let mut tree = super::Xr155KDTree::xr_new();
        assert!(tree.xr_bounding_box().is_none());
        tree.xr_insert(super::Xr155KDPoint::xr_new(1.0, 2.0));
        tree.xr_insert(super::Xr155KDPoint::xr_new(5.0, 8.0));
        let bb = tree.xr_bounding_box().unwrap();
        assert!((bb.xr_min_x - 1.0).abs() < 1e-9);
        assert!((bb.xr_max_y - 8.0).abs() < 1e-9);
    }

}
