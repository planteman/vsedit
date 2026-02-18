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

}
