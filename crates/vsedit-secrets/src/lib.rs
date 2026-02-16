//! Secure credential storage.
//!
//! Provides a trait-based abstraction for secret storage with an in-memory
//! implementation for testing and a keyring stub for system integration.

use std::collections::HashMap;

/// A single secret entry.
#[derive(Debug, Clone)]
pub struct SecretEntry {
    pub key: String,
    pub value: String,
}

/// Event emitted when a secret is changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretStorageChangeEvent {
    pub key: String,
    pub kind: SecretChangeKind,
}

/// What kind of change happened to a secret.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretChangeKind {
    Added,
    Updated,
    Deleted,
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
    /// A generic storage error.
    Other(String),
}

impl std::fmt::Display for SecretStorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BackendUnavailable(msg) => write!(f, "Backend unavailable: {}", msg),
            Self::AccessDenied => write!(f, "Access denied"),
            Self::Other(msg) => write!(f, "Secret storage error: {}", msg),
        }
    }
}

/// In-memory secret storage (for testing and fallback).
#[derive(Debug, Default)]
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
}
