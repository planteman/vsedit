//! LSP client integration for language server communication.
//!
//! Provides a JSON-RPC transport, an [`LspClient`](client::LspClient) for
//! communicating with a single language server, and an
//! [`LspManager`](manager::LspManager) that manages multiple servers (one per
//! language).

pub mod client;
pub mod manager;
pub mod transport;

pub use client::{LspClient, LspServerConfig};
pub use manager::LspManager;

/// Errors produced by the LSP subsystem.
#[derive(Debug, thiserror::Error)]
pub enum LspError {
    #[error("failed to spawn server: {0}")]
    SpawnFailed(String),
    #[error("server stdin not available")]
    NoStdin,
    #[error("server stdout not available")]
    NoStdout,
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("response channel closed")]
    ResponseChannelClosed,
    #[error("server error {code}: {message}")]
    ServerError { code: i64, message: String },
    #[error("invalid URI: {0}")]
    InvalidUri(String),
    #[error("failed to deserialize: {0}")]
    DeserializeFailed(String),
    #[error("no config registered for language: {0}")]
    NoConfig(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::LspServerConfig;
    use crate::manager::LspManager;
    use crate::transport::*;

    #[test]
    fn lsp_error_display() {
        let err = LspError::SpawnFailed("not found".into());
        assert!(err.to_string().contains("not found"));

        let err = LspError::NoStdin;
        assert!(err.to_string().contains("stdin"));

        let err = LspError::ServerError {
            code: -32600,
            message: "bad".into(),
        };
        assert!(err.to_string().contains("-32600"));
    }

    #[test]
    fn lsp_server_config_clone() {
        let cfg = LspServerConfig {
            command: "rust-analyzer".to_string(),
            args: vec!["--stdio".to_string()],
            language_ids: vec!["rust".to_string()],
            root_patterns: vec![".rs".to_string()],
        };
        let cfg2 = cfg.clone();
        assert_eq!(cfg2.command, "rust-analyzer");
        assert_eq!(cfg2.language_ids, vec!["rust"]);
    }

    #[test]
    fn manager_register_and_unregister() {
        let mut mgr = LspManager::new();
        let cfg = LspServerConfig {
            command: "rust-analyzer".to_string(),
            args: vec![],
            language_ids: vec!["rust".to_string()],
            root_patterns: vec![".rs".to_string()],
        };
        mgr.register("rust", cfg);
        assert!(mgr.registered_languages().contains(&"rust".to_string()));
        assert!(mgr.unregister("rust"));
        assert!(!mgr.unregister("rust"));
    }

    #[test]
    fn manager_language_for_file() {
        let mut mgr = LspManager::new();
        mgr.register(
            "rust",
            LspServerConfig {
                command: "rust-analyzer".to_string(),
                args: vec![],
                language_ids: vec!["rust".to_string()],
                root_patterns: vec![".rs".to_string()],
            },
        );
        mgr.register(
            "python",
            LspServerConfig {
                command: "pylsp".to_string(),
                args: vec![],
                language_ids: vec!["python".to_string()],
                root_patterns: vec![".py".to_string()],
            },
        );
        assert_eq!(mgr.language_for_file("main.rs"), Some("rust".to_string()));
        assert_eq!(mgr.language_for_file("app.py"), Some("python".to_string()));
        assert_eq!(mgr.language_for_file("style.css"), None);
    }

    #[test]
    fn manager_is_active_false_when_not_started() {
        let _mgr = LspManager::new();
        assert!(!_mgr.is_active("rust"));
    }

    #[test]
    fn manager_active_languages_empty() {
        let _mgr = LspManager::new();
        assert!(_mgr.active_languages().is_empty());
    }

    #[test]
    fn manager_default() {
        let _mgr = LspManager::default();
        assert!(_mgr.registered_languages().is_empty());
    }

    #[tokio::test]
    async fn manager_start_no_config() {
        let mut mgr = LspManager::new();
        let result = mgr.start("rust").await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), LspError::NoConfig(_)));
    }

    #[tokio::test]
    async fn manager_stop_nonexistent_is_ok() {
        let mut mgr = LspManager::new();
        let result = mgr.stop("rust").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn spawn_nonexistent_command_fails() {
        let result = LspClient::spawn_server("nonexistent-lsp-binary-12345", &[]).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), LspError::SpawnFailed(_)));
    }

    #[test]
    fn transport_encode_decode_notification() {
        let notif = JsonRpcNotification {
            jsonrpc: "2.0".to_string(),
            method: "textDocument/didSave".to_string(),
            params: Some(serde_json::json!({"uri": "file:///test.rs"})),
        };
        let encoded = encode_message(&notif);
        let (msg, _) = try_decode_message(&encoded).unwrap().unwrap();
        assert!(msg.is_notification());
        assert_eq!(msg.method.as_deref(), Some("textDocument/didSave"));
    }

    #[test]
    fn transport_encode_decode_response() {
        let resp = JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: Some(42),
            result: Some(serde_json::json!({"capabilities": {}})),
            error: None,
        };
        let encoded = encode_message(&resp);
        let (msg, _) = try_decode_message(&encoded).unwrap().unwrap();
        assert!(msg.is_response());
        assert_eq!(msg.id, Some(42));
    }

    #[test]
    fn lsp_error_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::BrokenPipe, "broken");
        let _lsp_err = LspError::from(io_err);
        assert!(_lsp_err.to_string().contains("broken"));
    }
}
