//! Ext API: Authentication.
//!
//! RPC bridge between the extension host and the main thread for auth.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

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

/// Options for requesting an authentication session.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AuthSessionOptions {
    pub scopes: Vec<String>,
    pub force_new_session: bool,
    pub clear_sessions_first: bool,
    pub silent_mode: bool,
}

impl Default for AuthSessionOptions {
    fn default() -> Self {
        Self {
            scopes: Vec::new(),
            force_new_session: false,
            clear_sessions_first: false,
            silent_mode: false,
        }
    }
}

/// Metadata about a registered authentication provider.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AuthProviderInfo {
    pub id: String,
    pub label: String,
    pub supports_multiple_accounts: bool,
}

/// Errors that can occur during authentication operations.
#[derive(Debug, Clone, PartialEq)]
pub enum AuthError {
    ProviderNotFound,
    SessionExpired,
    UserCancelled,
    NetworkError(String),
}

impl fmt::Display for AuthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AuthError::ProviderNotFound => write!(f, "authentication provider not found"),
            AuthError::SessionExpired => write!(f, "authentication session has expired"),
            AuthError::UserCancelled => write!(f, "authentication was cancelled by the user"),
            AuthError::NetworkError(msg) => write!(f, "network error: {}", msg),
        }
    }
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

    pub fn get_providers(&self) -> &[String] {
        &self.providers
    }

    pub fn provider_count(&self) -> usize {
        self.providers.len()
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

// ── Session Store ──

/// In-memory store for authentication sessions, keyed by provider id.
pub struct SessionStore {
    sessions: HashMap<String, Vec<AuthSession>>,
}

impl SessionStore {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
        }
    }

    pub fn add_session(&mut self, provider_id: &str, session: AuthSession) {
        self.sessions
            .entry(provider_id.to_string())
            .or_default()
            .push(session);
    }

    /// Returns sessions for a provider that contain all the required scopes.
    pub fn get_sessions(&self, provider_id: &str, scopes: &[String]) -> Vec<&AuthSession> {
        match self.sessions.get(provider_id) {
            Some(sessions) => sessions
                .iter()
                .filter(|s| scopes_match(scopes, &s.scopes))
                .collect(),
            None => Vec::new(),
        }
    }

    /// Removes a specific session by id, returning whether it was found.
    pub fn remove_session(&mut self, provider_id: &str, session_id: &str) -> bool {
        if let Some(sessions) = self.sessions.get_mut(provider_id) {
            let before = sessions.len();
            sessions.retain(|s| s.id != session_id);
            sessions.len() < before
        } else {
            false
        }
    }

    /// Removes all sessions for a given provider.
    pub fn clear_sessions(&mut self, provider_id: &str) {
        self.sessions.remove(provider_id);
    }
}

impl Default for SessionStore {
    fn default() -> Self {
        Self::new()
    }
}

// ── Helper Functions ──

/// Returns `true` if `available` scopes cover every scope in `required`.
pub fn scopes_match(required: &[String], available: &[String]) -> bool {
    required.iter().all(|r| available.contains(r))
}

/// A session is valid when its access token and scope list are non-empty.
pub fn validate_session(session: &AuthSession) -> bool {
    !session.access_token.is_empty() && !session.scopes.is_empty()
}

/// Finds the session whose scopes cover all `required_scopes` with the fewest
/// extra scopes. Sessions that don't cover all required scopes are ignored.
pub fn find_best_session<'a>(
    sessions: &'a [AuthSession],
    required_scopes: &[String],
) -> Option<&'a AuthSession> {
    sessions
        .iter()
        .filter(|s| scopes_match(required_scopes, &s.scopes))
        .min_by_key(|s| s.scopes.len())
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

    // ── New Tests ──

    #[test]
    fn bridge_get_providers_and_count() {
        let mut bridge = AuthBridge::new();
        assert_eq!(bridge.provider_count(), 0);
        assert!(bridge.get_providers().is_empty());
        bridge.register_provider("github");
        bridge.register_provider("gitlab");
        assert_eq!(bridge.provider_count(), 2);
        assert_eq!(bridge.get_providers(), &["github", "gitlab"]);
    }

    #[test]
    fn auth_session_options_default() {
        let opts = AuthSessionOptions::default();
        assert!(opts.scopes.is_empty());
        assert!(!opts.force_new_session);
        assert!(!opts.clear_sessions_first);
        assert!(!opts.silent_mode);
    }

    #[test]
    fn auth_session_options_roundtrip() {
        let opts = AuthSessionOptions {
            scopes: vec!["repo".into(), "user".into()],
            force_new_session: true,
            clear_sessions_first: false,
            silent_mode: true,
        };
        let json = serde_json::to_string(&opts).unwrap();
        let back: AuthSessionOptions = serde_json::from_str(&json).unwrap();
        assert_eq!(opts, back);
    }

    #[test]
    fn auth_provider_info_roundtrip() {
        let info = AuthProviderInfo {
            id: "github".into(),
            label: "GitHub".into(),
            supports_multiple_accounts: true,
        };
        let json = serde_json::to_string(&info).unwrap();
        let back: AuthProviderInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(info, back);
    }

    #[test]
    fn auth_error_display() {
        assert_eq!(
            AuthError::ProviderNotFound.to_string(),
            "authentication provider not found"
        );
        assert_eq!(
            AuthError::SessionExpired.to_string(),
            "authentication session has expired"
        );
        assert_eq!(
            AuthError::UserCancelled.to_string(),
            "authentication was cancelled by the user"
        );
        assert_eq!(
            AuthError::NetworkError("timeout".into()).to_string(),
            "network error: timeout"
        );
    }

    #[test]
    fn scopes_match_works() {
        let available: Vec<String> = vec!["repo".into(), "user".into(), "read:org".into()];
        assert!(scopes_match(&["repo".into()], &available));
        assert!(scopes_match(&["repo".into(), "user".into()], &available));
        assert!(!scopes_match(&["admin".into()], &available));
        assert!(scopes_match(&[], &available));
    }

    #[test]
    fn validate_session_checks() {
        let valid = AuthSession {
            id: "s1".into(),
            access_token: "tok".into(),
            account: AuthAccount {
                id: "a1".into(),
                label: "u".into(),
            },
            scopes: vec!["read".into()],
        };
        assert!(validate_session(&valid));

        let empty_token = AuthSession {
            access_token: "".into(),
            ..valid.clone()
        };
        assert!(!validate_session(&empty_token));

        let empty_scopes = AuthSession {
            scopes: vec![],
            ..valid.clone()
        };
        assert!(!validate_session(&empty_scopes));
    }

    #[test]
    fn find_best_session_prefers_fewer_scopes() {
        let broad = AuthSession {
            id: "s1".into(),
            access_token: "t1".into(),
            account: AuthAccount {
                id: "a1".into(),
                label: "u".into(),
            },
            scopes: vec!["repo".into(), "user".into(), "admin".into()],
        };
        let narrow = AuthSession {
            id: "s2".into(),
            access_token: "t2".into(),
            account: AuthAccount {
                id: "a1".into(),
                label: "u".into(),
            },
            scopes: vec!["repo".into(), "user".into()],
        };
        let required: Vec<String> = vec!["repo".into(), "user".into()];
        let sessions = [broad, narrow];
        let best = find_best_session(&sessions, &required);
        assert_eq!(best.unwrap().id, "s2");
    }

    #[test]
    fn find_best_session_returns_none_when_no_match() {
        let session = AuthSession {
            id: "s1".into(),
            access_token: "t1".into(),
            account: AuthAccount {
                id: "a1".into(),
                label: "u".into(),
            },
            scopes: vec!["repo".into()],
        };
        let required: Vec<String> = vec!["admin".into()];
        let sessions = [session];
        assert!(find_best_session(&sessions, &required).is_none());
    }

    #[test]
    fn session_store_add_and_get() {
        let mut store = SessionStore::new();
        let session = AuthSession {
            id: "s1".into(),
            access_token: "tok".into(),
            account: AuthAccount {
                id: "a1".into(),
                label: "u".into(),
            },
            scopes: vec!["repo".into(), "user".into()],
        };
        store.add_session("github", session);
        let found = store.get_sessions("github", &["repo".into()]);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id, "s1");

        let none = store.get_sessions("github", &["admin".into()]);
        assert!(none.is_empty());
    }

    #[test]
    fn session_store_remove_and_clear() {
        let mut store = SessionStore::new();
        let mk = |id: &str| AuthSession {
            id: id.into(),
            access_token: "tok".into(),
            account: AuthAccount {
                id: "a1".into(),
                label: "u".into(),
            },
            scopes: vec!["repo".into()],
        };
        store.add_session("github", mk("s1"));
        store.add_session("github", mk("s2"));
        assert!(store.remove_session("github", "s1"));
        assert!(!store.remove_session("github", "s1"));
        assert_eq!(store.get_sessions("github", &[]).len(), 1);

        store.clear_sessions("github");
        assert!(store.get_sessions("github", &[]).is_empty());
    }

    #[test]
    fn session_store_unknown_provider() {
        let store = SessionStore::new();
        assert!(store.get_sessions("unknown", &[]).is_empty());
    }

    #[test]
    fn session_store_remove_from_unknown_provider() {
        let mut store = SessionStore::new();
        assert!(!store.remove_session("unknown", "s1"));
    }
}
