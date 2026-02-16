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

// ── Token Refresh Policy ──

/// Policy governing when authentication tokens should be refreshed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenRefreshPolicy {
    /// Never automatically refresh tokens.
    Never,
    /// Refresh tokens only when they have expired.
    OnExpiry,
    /// Proactively refresh tokens before they expire.
    Proactive,
}

impl fmt::Display for TokenRefreshPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TokenRefreshPolicy::Never => write!(f, "never"),
            TokenRefreshPolicy::OnExpiry => write!(f, "on_expiry"),
            TokenRefreshPolicy::Proactive => write!(f, "proactive"),
        }
    }
}

// ── Session Validator ──

/// Utilities for validating authentication sessions and tokens.
pub struct SessionValidator;

impl SessionValidator {
    /// Returns `true` if `token` is non-empty and contains no whitespace.
    pub fn validate_token_format(token: &str) -> bool {
        !token.is_empty() && !token.chars().any(|c| c.is_whitespace())
    }

    /// Returns `true` if no scope string is empty and there are no duplicates.
    pub fn validate_scopes(scopes: &[String]) -> bool {
        if scopes.iter().any(|s| s.is_empty()) {
            return false;
        }
        let mut seen = std::collections::HashSet::new();
        scopes.iter().all(|s| seen.insert(s))
    }

    /// Returns `true` if the session's access token appears in `expiry_map`
    /// and its recorded expiry time is at or before `current_time`.
    pub fn is_session_expired(
        session: &AuthSession,
        expiry_map: &HashMap<String, u64>,
        current_time: u64,
    ) -> bool {
        match expiry_map.get(&session.id) {
            Some(&expiry) => current_time >= expiry,
            None => false,
        }
    }
}

// ── Audit Log ──

/// Types of authentication events recorded by [`AuthAuditLog`].
#[derive(Debug, Clone, PartialEq)]
pub enum AuthAuditEvent {
    SessionCreated {
        provider_id: String,
        session_id: String,
    },
    SessionRemoved {
        provider_id: String,
        session_id: String,
    },
    ProviderRegistered {
        provider_id: String,
    },
    ProviderUnregistered {
        provider_id: String,
    },
}

/// A timestamped audit-log entry.
#[derive(Debug, Clone)]
pub struct AuditEntry {
    pub event: AuthAuditEvent,
    pub timestamp: u64,
}

/// Append-only log for authentication-related events.
#[derive(Debug, Default)]
pub struct AuthAuditLog {
    entries: Vec<AuditEntry>,
}

impl AuthAuditLog {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Record an event at the given timestamp.
    pub fn record(&mut self, event: AuthAuditEvent, timestamp: u64) {
        self.entries.push(AuditEntry { event, timestamp });
    }

    /// Returns entries whose event references `provider_id`.
    pub fn get_events_for_provider(&self, provider_id: &str) -> Vec<&AuditEntry> {
        self.entries
            .iter()
            .filter(|e| match &e.event {
                AuthAuditEvent::SessionCreated { provider_id: p, .. }
                | AuthAuditEvent::SessionRemoved { provider_id: p, .. }
                | AuthAuditEvent::ProviderRegistered { provider_id: p }
                | AuthAuditEvent::ProviderUnregistered { provider_id: p } => p == provider_id,
            })
            .collect()
    }

    /// Returns the most recent `n` entries (newest last).
    pub fn recent(&self, n: usize) -> &[AuditEntry] {
        let len = self.entries.len();
        if n >= len {
            &self.entries
        } else {
            &self.entries[len - n..]
        }
    }
}

// ── Additional Helper Functions ──

/// Returns the sorted, deduplicated union of two scope slices.
pub fn merge_scopes(a: &[String], b: &[String]) -> Vec<String> {
    let mut merged: Vec<String> = a.iter().chain(b.iter()).cloned().collect();
    merged.sort();
    merged.dedup();
    merged
}

/// Groups sessions by their account id.
pub fn group_sessions_by_account<'a>(
    sessions: &'a [AuthSession],
) -> HashMap<String, Vec<&'a AuthSession>> {
    let mut map: HashMap<String, Vec<&'a AuthSession>> = HashMap::new();
    for session in sessions {
        map.entry(session.account.id.clone())
            .or_default()
            .push(session);
    }
    map
}

/// Validates whether an authentication session has expired.
#[derive(Debug, Clone)]
pub struct AuthSessionValidator {
    /// TTL in seconds.
    pub ttl_seconds: u64,
}

impl AuthSessionValidator {
    pub fn new(ttl_seconds: u64) -> Self {
        Self { ttl_seconds }
    }

    /// Check if a session is expired given a creation timestamp and the current time.
    pub fn is_expired(&self, created_at: u64, now: u64) -> bool {
        if now < created_at {
            return false;
        }
        now - created_at > self.ttl_seconds
    }

    /// Filter out expired sessions from a list.
    pub fn filter_valid<'a>(
        &self,
        sessions: &'a [(AuthSession, u64)],
        now: u64,
    ) -> Vec<&'a AuthSession> {
        sessions
            .iter()
            .filter(|(_, created_at)| !self.is_expired(*created_at, now))
            .map(|(session, _)| session)
            .collect()
    }

    /// Remaining time in seconds before a session expires. Returns 0 if already expired.
    pub fn remaining(&self, created_at: u64, now: u64) -> u64 {
        if self.is_expired(created_at, now) {
            return 0;
        }
        self.ttl_seconds - (now - created_at)
    }
}

/// Tracks the state of a token refresh operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefreshState {
    Idle,
    Pending,
    Succeeded,
    Failed(String),
}

/// Manages token refresh state for a session.
#[derive(Debug, Clone)]
pub struct AuthTokenRefresher {
    pub session_id: String,
    pub state: RefreshState,
    pub attempt_count: u32,
    pub max_attempts: u32,
    pub last_attempt_at: Option<u64>,
}

impl AuthTokenRefresher {
    pub fn new(session_id: impl Into<String>, max_attempts: u32) -> Self {
        Self {
            session_id: session_id.into(),
            state: RefreshState::Idle,
            attempt_count: 0,
            max_attempts,
            last_attempt_at: None,
        }
    }

    /// Start a refresh attempt. Returns false if max attempts exceeded.
    pub fn begin_refresh(&mut self, timestamp: u64) -> bool {
        if self.attempt_count >= self.max_attempts {
            self.state = RefreshState::Failed("max attempts exceeded".into());
            return false;
        }
        self.state = RefreshState::Pending;
        self.attempt_count += 1;
        self.last_attempt_at = Some(timestamp);
        true
    }

    /// Mark the refresh as succeeded.
    pub fn succeed(&mut self) {
        self.state = RefreshState::Succeeded;
    }

    /// Mark the refresh as failed.
    pub fn fail(&mut self, reason: impl Into<String>) {
        self.state = RefreshState::Failed(reason.into());
    }

    /// Whether more refresh attempts can be made.
    pub fn can_retry(&self) -> bool {
        self.attempt_count < self.max_attempts
    }

    /// Reset the refresher to initial state.
    pub fn reset(&mut self) {
        self.state = RefreshState::Idle;
        self.attempt_count = 0;
        self.last_attempt_at = None;
    }
}

impl fmt::Display for AuthTokenRefresher {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Refresher({}, {:?}, {}/{})",
            self.session_id, self.state, self.attempt_count, self.max_attempts,
        )
    }
}

/// Checks if a session's scopes include the required permissions.
pub struct AuthPermissionChecker;

impl AuthPermissionChecker {
    /// Check if all required scopes are present in the session's scopes.
    pub fn has_permissions(session: &AuthSession, required: &[&str]) -> bool {
        required
            .iter()
            .all(|req| session.scopes.iter().any(|s| s == req))
    }

    /// Return which required scopes are missing from the session.
    pub fn missing_permissions<'a>(
        session: &AuthSession,
        required: &[&'a str],
    ) -> Vec<&'a str> {
        required
            .iter()
            .filter(|req| !session.scopes.iter().any(|s| s == **req))
            .copied()
            .collect()
    }

    /// Check if a session has at least one of the given scopes.
    pub fn has_any(session: &AuthSession, scopes: &[&str]) -> bool {
        scopes
            .iter()
            .any(|req| session.scopes.iter().any(|s| s == req))
    }
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

    // ── New Tests (added items) ──

    #[test]
    fn token_refresh_policy_display() {
        assert_eq!(TokenRefreshPolicy::Never.to_string(), "never");
        assert_eq!(TokenRefreshPolicy::OnExpiry.to_string(), "on_expiry");
        assert_eq!(TokenRefreshPolicy::Proactive.to_string(), "proactive");
    }

    #[test]
    fn validate_token_format() {
        assert!(SessionValidator::validate_token_format("ghp_abc123"));
        assert!(!SessionValidator::validate_token_format(""));
        assert!(!SessionValidator::validate_token_format("has space"));
        assert!(!SessionValidator::validate_token_format("has\ttab"));
        assert!(!SessionValidator::validate_token_format("has\nnewline"));
    }

    #[test]
    fn validate_scopes_rejects_empty_and_duplicates() {
        assert!(SessionValidator::validate_scopes(&[
            "repo".into(),
            "user".into()
        ]));
        assert!(SessionValidator::validate_scopes(&[]));
        assert!(!SessionValidator::validate_scopes(&[
            "repo".into(),
            "".into()
        ]));
        assert!(!SessionValidator::validate_scopes(&[
            "repo".into(),
            "repo".into()
        ]));
    }

    #[test]
    fn is_session_expired_checks_expiry_map() {
        let session = AuthSession {
            id: "s1".into(),
            access_token: "tok".into(),
            account: AuthAccount {
                id: "a1".into(),
                label: "u".into(),
            },
            scopes: vec!["read".into()],
        };
        let mut expiry_map = HashMap::new();
        expiry_map.insert("s1".into(), 1000);

        assert!(!SessionValidator::is_session_expired(&session, &expiry_map, 999));
        assert!(SessionValidator::is_session_expired(&session, &expiry_map, 1000));
        assert!(SessionValidator::is_session_expired(&session, &expiry_map, 1500));
        // Missing from map → not expired
        let session2 = AuthSession {
            id: "s2".into(),
            ..session.clone()
        };
        assert!(!SessionValidator::is_session_expired(&session2, &expiry_map, 9999));
    }

    #[test]
    fn audit_log_record_and_query() {
        let mut log = AuthAuditLog::new();
        log.record(
            AuthAuditEvent::ProviderRegistered {
                provider_id: "github".into(),
            },
            100,
        );
        log.record(
            AuthAuditEvent::SessionCreated {
                provider_id: "github".into(),
                session_id: "s1".into(),
            },
            200,
        );
        log.record(
            AuthAuditEvent::ProviderRegistered {
                provider_id: "gitlab".into(),
            },
            300,
        );

        let gh_events = log.get_events_for_provider("github");
        assert_eq!(gh_events.len(), 2);
        let gl_events = log.get_events_for_provider("gitlab");
        assert_eq!(gl_events.len(), 1);
        assert!(log.get_events_for_provider("unknown").is_empty());
    }

    #[test]
    fn audit_log_recent() {
        let mut log = AuthAuditLog::new();
        for i in 0..5 {
            log.record(
                AuthAuditEvent::ProviderRegistered {
                    provider_id: format!("p{}", i),
                },
                i as u64,
            );
        }
        let last3 = log.recent(3);
        assert_eq!(last3.len(), 3);
        assert_eq!(last3[0].timestamp, 2);
        assert_eq!(last3[2].timestamp, 4);

        // Requesting more than available returns all
        assert_eq!(log.recent(100).len(), 5);
    }

    #[test]
    fn merge_scopes_union_sorted_dedup() {
        let a: Vec<String> = vec!["repo".into(), "user".into(), "admin".into()];
        let b: Vec<String> = vec!["user".into(), "read:org".into()];
        let merged = merge_scopes(&a, &b);
        assert_eq!(merged, vec!["admin", "read:org", "repo", "user"]);

        assert!(merge_scopes(&[], &[]).is_empty());
        assert_eq!(
            merge_scopes(&["x".into()], &[]),
            vec!["x".to_string()]
        );
    }

    #[test]
    fn group_sessions_by_account_groups_correctly() {
        let sessions = vec![
            AuthSession {
                id: "s1".into(),
                access_token: "t1".into(),
                account: AuthAccount {
                    id: "a1".into(),
                    label: "Alice".into(),
                },
                scopes: vec!["repo".into()],
            },
            AuthSession {
                id: "s2".into(),
                access_token: "t2".into(),
                account: AuthAccount {
                    id: "a2".into(),
                    label: "Bob".into(),
                },
                scopes: vec!["repo".into()],
            },
            AuthSession {
                id: "s3".into(),
                access_token: "t3".into(),
                account: AuthAccount {
                    id: "a1".into(),
                    label: "Alice".into(),
                },
                scopes: vec!["user".into()],
            },
        ];
        let grouped = group_sessions_by_account(&sessions);
        assert_eq!(grouped.len(), 2);
        assert_eq!(grouped["a1"].len(), 2);
        assert_eq!(grouped["a2"].len(), 1);
        assert_eq!(grouped["a2"][0].id, "s2");
    }

    #[test]
    fn session_validator_expired() {
        let validator = AuthSessionValidator::new(3600);
        assert!(!validator.is_expired(1000, 2000));
        assert!(validator.is_expired(1000, 5000));
        assert_eq!(validator.remaining(1000, 2000), 2600);
        assert_eq!(validator.remaining(1000, 5000), 0);
    }

    #[test]
    fn session_validator_filter() {
        let validator = AuthSessionValidator::new(100);
        let sessions = vec![
            (AuthSession {
                id: "s1".into(),
                access_token: "t1".into(),
                account: AuthAccount { id: "a1".into(), label: "A".into() },
                scopes: vec![],
            }, 50),
            (AuthSession {
                id: "s2".into(),
                access_token: "t2".into(),
                account: AuthAccount { id: "a2".into(), label: "B".into() },
                scopes: vec![],
            }, 200),
        ];
        let valid = validator.filter_valid(&sessions, 200);
        assert_eq!(valid.len(), 1);
        assert_eq!(valid[0].id, "s2");
    }

    #[test]
    fn token_refresher_lifecycle() {
        let mut refresher = AuthTokenRefresher::new("s1", 3);
        assert!(refresher.can_retry());
        assert!(refresher.begin_refresh(100));
        assert_eq!(refresher.state, RefreshState::Pending);
        refresher.fail("network error");
        assert!(matches!(refresher.state, RefreshState::Failed(_)));
        assert!(refresher.begin_refresh(200));
        refresher.succeed();
        assert_eq!(refresher.state, RefreshState::Succeeded);
        assert_eq!(refresher.attempt_count, 2);
    }

    #[test]
    fn token_refresher_max_attempts() {
        let mut refresher = AuthTokenRefresher::new("s1", 1);
        assert!(refresher.begin_refresh(100));
        refresher.fail("err");
        assert!(!refresher.begin_refresh(200));
        assert!(!refresher.can_retry());
    }

    #[test]
    fn permission_checker_has_all() {
        let session = AuthSession {
            id: "s1".into(),
            access_token: "t".into(),
            account: AuthAccount { id: "a".into(), label: "A".into() },
            scopes: vec!["repo".into(), "user".into(), "read:org".into()],
        };
        assert!(AuthPermissionChecker::has_permissions(&session, &["repo", "user"]));
        assert!(!AuthPermissionChecker::has_permissions(&session, &["repo", "admin"]));
    }

    #[test]
    fn permission_checker_missing() {
        let session = AuthSession {
            id: "s1".into(),
            access_token: "t".into(),
            account: AuthAccount { id: "a".into(), label: "A".into() },
            scopes: vec!["repo".into()],
        };
        let missing = AuthPermissionChecker::missing_permissions(&session, &["repo", "admin", "user"]);
        assert_eq!(missing, vec!["admin", "user"]);
    }

    #[test]
    fn permission_checker_has_any() {
        let session = AuthSession {
            id: "s1".into(),
            access_token: "t".into(),
            account: AuthAccount { id: "a".into(), label: "A".into() },
            scopes: vec!["user".into()],
        };
        assert!(AuthPermissionChecker::has_any(&session, &["repo", "user"]));
        assert!(!AuthPermissionChecker::has_any(&session, &["admin", "write"]));
    }
}
