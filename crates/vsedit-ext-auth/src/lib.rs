//! Ext API: Authentication.
//!
//! RPC bridge between the extension host and the main thread for auth.

use serde::{Deserialize, Serialize};

/// Proxy identifier for this extension API namespace.
pub const PROXY_ID: &str = "ext_auth";

// ── RPC Messages ──

/// Messages sent over the RPC channel for authentication.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum AuthMessage {
    GetSessions {
        provider_id: String,
        scopes: Vec<String>,
    },
    SessionsChanged {
        provider_id: String,
    },
    RegisterProvider {
        provider_id: String,
        label: String,
    },
    UnregisterProvider {
        provider_id: String,
    },
}

// ── Core Types ──

/// An authentication session returned by a provider.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AuthSession {
    pub id: String,
    pub access_token: String,
    pub account: AuthAccount,
    pub scopes: Vec<String>,
}

/// Account information associated with an authentication session.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AuthAccount {
    pub id: String,
    pub label: String,
}

/// Describes an authentication provider registered by an extension.
pub trait AuthProvider {
    fn id(&self) -> &str;
    fn label(&self) -> &str;
    fn get_sessions(&self, scopes: &[String]) -> Vec<AuthSession>;
    fn create_session(&self, scopes: &[String]) -> Option<AuthSession>;
    fn remove_session(&self, session_id: &str) -> bool;
}

// ── Bridge ──

/// Bridge that routes authentication RPC messages.
pub struct AuthBridge {
    providers: Vec<String>,
}

impl AuthBridge {
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
        }
    }

    pub fn register_provider(&mut self, provider_id: &str) {
        if !self.providers.contains(&provider_id.to_string()) {
            self.providers.push(provider_id.to_string());
        }
    }

    pub fn unregister_provider(&mut self, provider_id: &str) {
        self.providers.retain(|p| p != provider_id);
    }

    pub fn has_provider(&self, provider_id: &str) -> bool {
        self.providers.iter().any(|p| p == provider_id)
    }

    pub fn handle_message(&mut self, msg: &AuthMessage) -> serde_json::Value {
        match msg {
            AuthMessage::RegisterProvider { provider_id, .. } => {
                self.register_provider(provider_id);
                serde_json::json!({"registered": true})
            }
            AuthMessage::UnregisterProvider { provider_id } => {
                self.unregister_provider(provider_id);
                serde_json::json!({"unregistered": true})
            }
            AuthMessage::GetSessions { provider_id, .. } => {
                serde_json::json!({"provider": provider_id, "sessions": []})
            }
            AuthMessage::SessionsChanged { provider_id } => {
                serde_json::json!({"provider": provider_id, "changed": true})
            }
        }
    }
}

impl Default for AuthBridge {
    fn default() -> Self {
        Self::new()
    }
}

/// Initialize the auth extension API bridge.
pub fn register() {
    // Registration will connect RPC handlers when extension host starts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proxy_id() {
        assert!(!PROXY_ID.is_empty());
    }

    #[test]
    fn message_roundtrip() {
        let msg = AuthMessage::GetSessions {
            provider_id: "github".into(),
            scopes: vec!["repo".into()],
        };
        let json = serde_json::to_string(&msg).unwrap();
        let back: AuthMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, back);
    }

    #[test]
    fn session_serialization() {
        let session = AuthSession {
            id: "s1".into(),
            access_token: "tok".into(),
            account: AuthAccount {
                id: "a1".into(),
                label: "user".into(),
            },
            scopes: vec!["read".into()],
        };
        let json = serde_json::to_string(&session).unwrap();
        let back: AuthSession = serde_json::from_str(&json).unwrap();
        assert_eq!(session, back);
    }

    #[test]
    fn bridge_register_unregister() {
        let mut bridge = AuthBridge::new();
        bridge.register_provider("github");
        assert!(bridge.has_provider("github"));
        bridge.unregister_provider("github");
        assert!(!bridge.has_provider("github"));
    }

    #[test]
    fn bridge_handle_register_message() {
        let mut bridge = AuthBridge::new();
        let msg = AuthMessage::RegisterProvider {
            provider_id: "github".into(),
            label: "GitHub".into(),
        };
        let result = bridge.handle_message(&msg);
        assert_eq!(result["registered"], true);
        assert!(bridge.has_provider("github"));
    }

    #[test]
    fn bridge_duplicate_register() {
        let mut bridge = AuthBridge::new();
        bridge.register_provider("github");
        bridge.register_provider("github");
        assert_eq!(bridge.providers.len(), 1);
    }
}
