//! Memento API — scoped key-value store for extension state.
//!
//! Equivalent to VS Code's `Memento` interface used by extensions to persist
//! state across sessions.

use std::collections::HashSet;

use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::{Storage, StorageResult};

/// A scoped key-value store backed by [`Storage`].
///
/// Values are serialized as JSON strings, allowing typed get/update operations.
pub struct Memento<'a> {
    prefix: String,
    storage: &'a Storage,
    sync_keys: HashSet<String>,
}

impl<'a> Memento<'a> {
    /// Create a new memento with the given scope prefix.
    pub fn new(prefix: impl Into<String>, storage: &'a Storage) -> Self {
        Self {
            prefix: prefix.into(),
            storage,
            sync_keys: HashSet::new(),
        }
    }

    fn scoped_key(&self, key: &str) -> String {
        format!("memento.{}.{}", self.prefix, key)
    }

    /// Get a typed value by key.
    pub fn get<T: DeserializeOwned>(&self, key: &str) -> Option<T> {
        let raw = self.storage.get(&self.scoped_key(key))?;
        serde_json::from_str(&raw).ok()
    }

    /// Store a typed value by key.
    pub fn update<T: Serialize>(&self, key: &str, value: &T) -> StorageResult<()> {
        let json = serde_json::to_string(value).map_err(|e| {
            crate::StorageError::Io(std::io::Error::new(std::io::ErrorKind::InvalidData, e))
        })?;
        self.storage.set(&self.scoped_key(key), &json)
    }

    /// Get all keys in this memento's scope.
    pub fn keys(&self) -> Vec<String> {
        let prefix = format!("memento.{}.", self.prefix);
        self.storage
            .keys_with_prefix(&prefix)
            .into_iter()
            .filter_map(|k| k.strip_prefix(&prefix).map(String::from))
            .collect()
    }

    /// Mark keys for settings sync.
    pub fn set_keys_for_sync(&mut self, keys: Vec<String>) {
        self.sync_keys = keys.into_iter().collect();
    }

    /// Get the set of keys marked for sync.
    pub fn keys_for_sync(&self) -> &HashSet<String> {
        &self.sync_keys
    }

    /// Delete a key from this memento.
    pub fn delete(&self, key: &str) -> StorageResult<()> {
        self.storage.remove(&self.scoped_key(key))
    }

    /// Check if a key exists.
    pub fn has(&self, key: &str) -> bool {
        self.storage.has(&self.scoped_key(key))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memento_get_update_typed() {
        let storage = Storage::in_memory().unwrap();
        let memento = Memento::new("ext1", &storage);
        memento.update("count", &42i64).unwrap();
        assert_eq!(memento.get::<i64>("count"), Some(42));
    }

    #[test]
    fn memento_get_missing_returns_none() {
        let storage = Storage::in_memory().unwrap();
        let memento = Memento::new("ext1", &storage);
        assert_eq!(memento.get::<String>("missing"), None);
    }

    #[test]
    fn memento_update_string() {
        let storage = Storage::in_memory().unwrap();
        let memento = Memento::new("ext1", &storage);
        memento.update("name", &"hello".to_string()).unwrap();
        assert_eq!(memento.get::<String>("name"), Some("hello".to_string()));
    }

    #[test]
    fn memento_update_vec() {
        let storage = Storage::in_memory().unwrap();
        let memento = Memento::new("ext2", &storage);
        let items = vec![1, 2, 3];
        memento.update("items", &items).unwrap();
        assert_eq!(memento.get::<Vec<i32>>("items"), Some(vec![1, 2, 3]));
    }

    #[test]
    fn memento_keys() {
        let storage = Storage::in_memory().unwrap();
        let memento = Memento::new("ext1", &storage);
        memento.update("alpha", &1).unwrap();
        memento.update("beta", &2).unwrap();
        let mut keys = memento.keys();
        keys.sort();
        assert_eq!(keys, vec!["alpha", "beta"]);
    }

    #[test]
    fn memento_scoped_isolation() {
        let storage = Storage::in_memory().unwrap();
        let m1 = Memento::new("ext1", &storage);
        let m2 = Memento::new("ext2", &storage);
        m1.update("key", &"from_ext1").unwrap();
        m2.update("key", &"from_ext2").unwrap();
        assert_eq!(m1.get::<String>("key"), Some("from_ext1".to_string()));
        assert_eq!(m2.get::<String>("key"), Some("from_ext2".to_string()));
    }

    #[test]
    fn memento_set_keys_for_sync() {
        let storage = Storage::in_memory().unwrap();
        let mut memento = Memento::new("ext1", &storage);
        memento.set_keys_for_sync(vec!["a".into(), "b".into()]);
        assert!(memento.keys_for_sync().contains("a"));
        assert!(memento.keys_for_sync().contains("b"));
        assert!(!memento.keys_for_sync().contains("c"));
    }

    #[test]
    fn memento_delete() {
        let storage = Storage::in_memory().unwrap();
        let memento = Memento::new("ext1", &storage);
        memento.update("key", &"val").unwrap();
        assert!(memento.has("key"));
        memento.delete("key").unwrap();
        assert!(!memento.has("key"));
    }

    #[test]
    fn memento_overwrite() {
        let storage = Storage::in_memory().unwrap();
        let memento = Memento::new("ext1", &storage);
        memento.update("key", &"v1").unwrap();
        memento.update("key", &"v2").unwrap();
        assert_eq!(memento.get::<String>("key"), Some("v2".to_string()));
    }

    #[test]
    fn memento_bool_value() {
        let storage = Storage::in_memory().unwrap();
        let memento = Memento::new("ext1", &storage);
        memento.update("flag", &true).unwrap();
        assert_eq!(memento.get::<bool>("flag"), Some(true));
    }
}
