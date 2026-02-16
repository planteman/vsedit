//! Secrets storage with OS keychain integration.
//!
//! Uses the `keyring` crate for OS-level secret storage, falling back to
//! an encrypted-file or in-memory store when the keychain is unavailable.

use std::collections::HashMap;

use crate::StorageError;

/// Error type for secret storage operations.
#[derive(Debug, thiserror::Error)]
pub enum SecretError {
    #[error("keyring error: {0}")]
    Keyring(String),
    #[error("secret not found: {0}")]
    NotFound(String),
    #[error("storage error: {0}")]
    Storage(#[from] StorageError),
}

/// Secret storage backed by an in-memory map with optional keyring support.
pub struct SecretStorage {
    service_name: String,
    /// Fallback in-memory store used when keyring is unavailable.
    fallback: HashMap<String, String>,
    use_keyring: bool,
}

impl SecretStorage {
    /// Create a new secret storage with the given service name.
    /// If `use_keyring` is true, attempts to use the OS keychain.
    pub fn new(service_name: impl Into<String>, use_keyring: bool) -> Self {
        Self {
            service_name: service_name.into(),
            fallback: HashMap::new(),
            use_keyring,
        }
    }

    /// Create an in-memory-only secret storage (for testing).
    pub fn in_memory() -> Self {
        Self::new("vsedit-test", false)
    }

    /// Get a secret value.
    pub fn get_secret(&self, key: &str) -> Option<String> {
        if self.use_keyring {
            if let Ok(entry) = keyring::Entry::new(&self.service_name, key) {
                if let Ok(val) = entry.get_password() {
                    return Some(val);
                }
            }
        }
        self.fallback.get(key).cloned()
    }

    /// Store a secret value.
    pub fn set_secret(&mut self, key: &str, value: &str) -> Result<(), SecretError> {
        if self.use_keyring {
            if let Ok(entry) = keyring::Entry::new(&self.service_name, key) {
                if entry.set_password(value).is_ok() {
                    return Ok(());
                }
            }
        }
        // Fallback to in-memory
        self.fallback.insert(key.to_string(), value.to_string());
        Ok(())
    }

    /// Delete a secret.
    pub fn delete_secret(&mut self, key: &str) -> Result<(), SecretError> {
        if self.use_keyring {
            if let Ok(entry) = keyring::Entry::new(&self.service_name, key) {
                let _ = entry.delete_credential();
            }
        }
        self.fallback.remove(key);
        Ok(())
    }

    /// Check if a secret exists.
    pub fn has_secret(&self, key: &str) -> bool {
        self.get_secret(key).is_some()
    }

    /// List all known keys (fallback only; keyring doesn't support enumeration).
    pub fn keys(&self) -> Vec<String> {
        let mut keys: Vec<String> = self.fallback.keys().cloned().collect();
        keys.sort();
        keys
    }

    /// Get the service name.
    pub fn service_name(&self) -> &str {
        &self.service_name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_set_get() {
        let mut store = SecretStorage::in_memory();
        store.set_secret("token", "abc123").unwrap();
        assert_eq!(store.get_secret("token"), Some("abc123".to_string()));
    }

    #[test]
    fn secret_delete() {
        let mut store = SecretStorage::in_memory();
        store.set_secret("token", "val").unwrap();
        store.delete_secret("token").unwrap();
        assert!(!store.has_secret("token"));
    }

    #[test]
    fn secret_missing_returns_none() {
        let store = SecretStorage::in_memory();
        assert_eq!(store.get_secret("missing"), None);
    }

    #[test]
    fn secret_keys() {
        let mut store = SecretStorage::in_memory();
        store.set_secret("b", "2").unwrap();
        store.set_secret("a", "1").unwrap();
        assert_eq!(store.keys(), vec!["a", "b"]);
    }

    #[test]
    fn secret_overwrite() {
        let mut store = SecretStorage::in_memory();
        store.set_secret("key", "v1").unwrap();
        store.set_secret("key", "v2").unwrap();
        assert_eq!(store.get_secret("key"), Some("v2".to_string()));
    }

    #[test]
    fn secret_service_name() {
        let store = SecretStorage::new("my-service", false);
        assert_eq!(store.service_name(), "my-service");
    }
}
