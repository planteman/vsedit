//! Secure credential storage.

use std::collections::HashMap;

/// A single secret entry.
#[derive(Debug, Clone)]
pub struct SecretEntry {
    pub key: String,
    pub value: String,
}

/// In-memory secret storage service.
#[derive(Debug, Default)]
pub struct SecretStorageService {
    secrets: HashMap<String, String>,
}

impl SecretStorageService {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn store(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.secrets.insert(key.into(), value.into());
    }

    pub fn retrieve(&self, key: &str) -> Option<&str> {
        self.secrets.get(key).map(String::as_str)
    }

    pub fn delete(&mut self, key: &str) -> bool {
        self.secrets.remove(key).is_some()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn store_and_retrieve() {
        let mut svc = SecretStorageService::new();
        svc.store("token", "abc123");
        assert_eq!(svc.retrieve("token"), Some("abc123"));
        assert!(svc.has("token"));
        assert_eq!(svc.key_count(), 1);
    }

    #[test]
    fn delete_secret() {
        let mut svc = SecretStorageService::new();
        svc.store("key", "val");
        assert!(svc.delete("key"));
        assert!(!svc.has("key"));
        assert!(!svc.delete("key"));
    }

    #[test]
    fn clear_all() {
        let mut svc = SecretStorageService::new();
        svc.store("a", "1");
        svc.store("b", "2");
        assert_eq!(svc.key_count(), 2);
        svc.clear();
        assert_eq!(svc.key_count(), 0);
    }
}
