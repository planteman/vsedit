//! Secure credential storage.
//!
//! Provides a trait-based abstraction for secret storage with an in-memory
//! implementation for testing and a keyring stub for system integration.

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
    fn delete_secret() {
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
}
