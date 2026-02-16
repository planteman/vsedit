//! Manager for multiple language server instances.

use std::collections::HashMap;

use crate::client::{LspClient, LspServerConfig};
use crate::LspError;

/// Manages multiple language server instances, one per language.
pub struct LspManager {
    configs: HashMap<String, LspServerConfig>,
    clients: HashMap<String, LspClient>,
}

impl LspManager {
    /// Create a new empty manager.
    pub fn new() -> Self {
        Self {
            configs: HashMap::new(),
            clients: HashMap::new(),
        }
    }

    /// Register a server configuration for a language.
    pub fn register(&mut self, language_id: &str, config: LspServerConfig) {
        self.configs.insert(language_id.to_string(), config);
    }

    /// Unregister a language server configuration.
    pub fn unregister(&mut self, language_id: &str) -> bool {
        self.configs.remove(language_id).is_some()
    }

    /// Start a language server for the given language id.
    pub async fn start(&mut self, language_id: &str) -> Result<(), LspError> {
        let config = self
            .configs
            .get(language_id)
            .ok_or_else(|| LspError::NoConfig(language_id.to_string()))?
            .clone();

        let args: Vec<&str> = config.args.iter().map(|s| s.as_str()).collect();
        let client = LspClient::spawn_server(&config.command, &args).await?;
        self.clients.insert(language_id.to_string(), client);
        Ok(())
    }

    /// Get a reference to a running client for a language.
    pub fn client(&self, language_id: &str) -> Option<&LspClient> {
        self.clients.get(language_id)
    }

    /// Stop a running language server.
    pub async fn stop(&mut self, language_id: &str) -> Result<(), LspError> {
        if let Some(client) = self.clients.remove(language_id) {
            client.shutdown().await?;
        }
        Ok(())
    }

    /// Stop all running language servers.
    pub async fn stop_all(&mut self) -> Result<(), LspError> {
        let keys: Vec<String> = self.clients.keys().cloned().collect();
        for key in keys {
            self.stop(&key).await?;
        }
        Ok(())
    }

    /// Returns the list of registered language ids.
    pub fn registered_languages(&self) -> Vec<String> {
        self.configs.keys().cloned().collect()
    }

    /// Returns the list of active (running) language ids.
    pub fn active_languages(&self) -> Vec<String> {
        self.clients.keys().cloned().collect()
    }

    /// Check if a language server is running for the given language.
    pub fn is_active(&self, language_id: &str) -> bool {
        self.clients.contains_key(language_id)
    }

    /// Find the best language id for a file path based on registered configs.
    pub fn language_for_file(&self, path: &str) -> Option<String> {
        for (lang_id, config) in &self.configs {
            for pattern in &config.root_patterns {
                if path.ends_with(pattern) {
                    return Some(lang_id.clone());
                }
            }
        }
        None
    }
}

impl Default for LspManager {
    fn default() -> Self {
        Self::new()
    }
}
