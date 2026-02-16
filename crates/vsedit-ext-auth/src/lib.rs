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

// ---------------------------------------------------------------------------
// Auth provider capabilities
// ---------------------------------------------------------------------------

/// Capabilities that an authentication provider may support.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AuthProviderCapability {
    /// Provider supports silent/background token refresh.
    SilentRefresh,
    /// Provider supports multi-account sign-in.
    MultiAccount,
    /// Provider can supply scoped tokens.
    ScopedTokens,
    /// Provider supports device-code flow (for headless).
    DeviceCodeFlow,
    /// Provider supports PKCE authorization code flow.
    Pkce,
    /// Provider can revoke tokens.
    TokenRevocation,
}

impl fmt::Display for AuthProviderCapability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SilentRefresh => write!(f, "silent-refresh"),
            Self::MultiAccount => write!(f, "multi-account"),
            Self::ScopedTokens => write!(f, "scoped-tokens"),
            Self::DeviceCodeFlow => write!(f, "device-code-flow"),
            Self::Pkce => write!(f, "pkce"),
            Self::TokenRevocation => write!(f, "token-revocation"),
        }
    }
}

/// A set of capabilities advertised by an auth provider.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AuthCapabilitySet {
    capabilities: Vec<AuthProviderCapability>,
}

impl AuthCapabilitySet {
    pub fn new() -> Self {
        Self { capabilities: Vec::new() }
    }

    pub fn add(&mut self, cap: AuthProviderCapability) {
        if !self.capabilities.contains(&cap) {
            self.capabilities.push(cap);
        }
    }

    pub fn has(&self, cap: AuthProviderCapability) -> bool {
        self.capabilities.contains(&cap)
    }

    pub fn has_all(&self, caps: &[AuthProviderCapability]) -> bool {
        caps.iter().all(|c| self.has(*c))
    }

    pub fn has_any(&self, caps: &[AuthProviderCapability]) -> bool {
        caps.iter().any(|c| self.has(*c))
    }

    pub fn len(&self) -> usize {
        self.capabilities.len()
    }

    pub fn is_empty(&self) -> bool {
        self.capabilities.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &AuthProviderCapability> {
        self.capabilities.iter()
    }

    /// Check if this provider is suitable for headless environments.
    pub fn supports_headless(&self) -> bool {
        self.has(AuthProviderCapability::DeviceCodeFlow)
    }

    /// Check if this provider supports secure token lifecycle.
    pub fn supports_secure_lifecycle(&self) -> bool {
        self.has(AuthProviderCapability::SilentRefresh)
            && self.has(AuthProviderCapability::TokenRevocation)
    }
}

impl fmt::Display for AuthCapabilitySet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let names: Vec<String> = self.capabilities.iter().map(|c| format!("{c}")).collect();
        write!(f, "[{}]", names.join(", "))
    }
}

// ---------------------------------------------------------------------------
// Additional AuthBridge methods
// ---------------------------------------------------------------------------

impl AuthBridge {
    /// Remove all registered providers.
    pub fn clear_all(&mut self) {
        self.providers.clear();
    }
}

// ---------------------------------------------------------------------------
// Additional SessionStore methods
// ---------------------------------------------------------------------------

impl SessionStore {
    /// Returns the total number of sessions across all providers.
    pub fn session_count(&self) -> usize {
        self.sessions.values().map(Vec::len).sum()
    }

    /// Returns the provider ids that have at least one session.
    pub fn all_provider_ids(&self) -> Vec<&str> {
        self.sessions.keys().map(String::as_str).collect()
    }
}

// ---------------------------------------------------------------------------
// Additional AuthSession methods
// ---------------------------------------------------------------------------

impl AuthSession {
    /// Returns `true` if the access token is non-empty.
    pub fn is_token_present(&self) -> bool {
        !self.access_token.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Additional AuthAuditLog methods
// ---------------------------------------------------------------------------

impl AuthAuditLog {
    /// Returns the total number of recorded events.
    pub fn event_count(&self) -> usize {
        self.entries.len()
    }

    /// Remove all entries from the log.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

// ---------------------------------------------------------------------------
// Additional AuthTokenRefresher methods
// ---------------------------------------------------------------------------

impl AuthTokenRefresher {
    /// Returns the number of remaining refresh attempts before the maximum is
    /// reached.
    pub fn attempts_remaining(&self) -> u32 {
        self.max_attempts.saturating_sub(self.attempt_count)
    }
}

// ---------------------------------------------------------------------------
// Display for AuthSession
// ---------------------------------------------------------------------------

impl fmt::Display for AuthSession {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Session({}, account={}, scopes=[{}])",
            self.id,
            self.account.label,
            self.scopes.join(", ")
        )
    }
}

// ---------------------------------------------------------------------------
// Token validation
// ---------------------------------------------------------------------------

/// Validates the format of a bearer token string.
///
/// A valid token must be non-empty, contain only ASCII alphanumeric characters,
/// hyphens, underscores, or dots, and must not exceed the given max length.
pub fn validate_token(token: &str, max_length: usize) -> Result<(), AuthError> {
    if token.is_empty() {
        return Err(AuthError::SessionExpired);
    }
    if token.len() > max_length {
        return Err(AuthError::NetworkError(format!(
            "token exceeds max length of {max_length}"
        )));
    }
    if !token.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.') {
        return Err(AuthError::NetworkError("token contains invalid characters".into()));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Auth scope parsing
// ---------------------------------------------------------------------------

/// Parse a space-delimited scope string into individual scopes.
///
/// Empty segments are ignored and results are deduplicated.
pub fn parse_scopes(scope_string: &str) -> Vec<String> {
    let mut scopes: Vec<String> = scope_string
        .split_whitespace()
        .map(|s| s.to_string())
        .collect();
    scopes.sort();
    scopes.dedup();
    scopes
}

/// Checks whether `requested` scopes are a subset of `granted` scopes.
pub fn scopes_satisfied(granted: &[String], requested: &[String]) -> bool {
    requested.iter().all(|r| granted.contains(r))
}

// ---------------------------------------------------------------------------
// Session tracker — in-memory session registry with timestamps
// ---------------------------------------------------------------------------

/// Tracks active sessions with creation timestamps.
#[derive(Debug, Clone, Default)]
pub struct SessionTracker {
    sessions: Vec<(AuthSession, u64)>,
}

impl SessionTracker {
    pub fn new() -> Self {
        Self { sessions: Vec::new() }
    }

    /// Register a session with its creation timestamp.
    pub fn add(&mut self, session: AuthSession, created_at: u64) {
        self.sessions.push((session, created_at));
    }

    /// Remove a session by id. Returns `true` if found.
    pub fn remove(&mut self, session_id: &str) -> bool {
        let before = self.sessions.len();
        self.sessions.retain(|(s, _)| s.id != session_id);
        self.sessions.len() < before
    }

    /// Return sessions that are still valid given the TTL and current time.
    pub fn active_sessions(&self, ttl_seconds: u64, now: u64) -> Vec<&AuthSession> {
        self.sessions
            .iter()
            .filter(|(_, created_at)| {
                if now < *created_at {
                    true
                } else {
                    now - created_at <= ttl_seconds
                }
            })
            .map(|(s, _)| s)
            .collect()
    }

    /// Return sessions that have expired.
    pub fn expired_sessions(&self, ttl_seconds: u64, now: u64) -> Vec<&AuthSession> {
        self.sessions
            .iter()
            .filter(|(_, created_at)| now >= *created_at && now - created_at > ttl_seconds)
            .map(|(s, _)| s)
            .collect()
    }

    /// Total number of tracked sessions.
    pub fn len(&self) -> usize {
        self.sessions.len()
    }

    /// Whether the tracker has no sessions.
    pub fn is_empty(&self) -> bool {
        self.sessions.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Credential rotation scheduling
// ---------------------------------------------------------------------------

/// Determines when credentials should be rotated.
#[derive(Debug, Clone)]
pub struct RotationSchedule {
    /// Rotation interval in seconds.
    pub interval_seconds: u64,
    /// Last rotation timestamp.
    pub last_rotation: Option<u64>,
}

impl RotationSchedule {
    pub fn new(interval_seconds: u64) -> Self {
        Self {
            interval_seconds,
            last_rotation: None,
        }
    }

    /// Record a rotation at the given timestamp.
    pub fn record_rotation(&mut self, timestamp: u64) {
        self.last_rotation = Some(timestamp);
    }

    /// Whether rotation is due given the current time.
    pub fn is_due(&self, now: u64) -> bool {
        match self.last_rotation {
            None => true,
            Some(last) => now >= last && now - last >= self.interval_seconds,
        }
    }

    /// Seconds until next rotation. Returns 0 if already due.
    pub fn seconds_until_due(&self, now: u64) -> u64 {
        match self.last_rotation {
            None => 0,
            Some(last) => {
                let elapsed = now.saturating_sub(last);
                self.interval_seconds.saturating_sub(elapsed)
            }
        }
    }
}

/// Count total sessions across all providers in a `SessionStore`.
pub fn total_session_count(store: &SessionStore) -> usize {
    store.all_provider_ids().iter().map(|pid| {
        store.get_sessions(pid, &[]).len()
    }).sum()
}

/// Find sessions that have all of the requested scopes.
pub fn find_sessions_with_scopes<'a>(
    store: &'a SessionStore,
    provider_id: &str,
    required: &[String],
) -> Vec<&'a AuthSession> {
    store
        .get_sessions(provider_id, &[])
        .into_iter()
        .filter(|s| scopes_match(required, &s.scopes))
        .collect()
}

/// Deduplicate scopes in a session's scope list (preserving order).
pub fn deduplicate_scopes(scopes: &[String]) -> Vec<String> {
    let mut seen = Vec::new();
    for s in scopes {
        if !seen.contains(s) {
            seen.push(s.clone());
        }
    }
    seen
}

/// Check whether a token string looks like a JWT (three dot-separated base64 parts).
pub fn is_jwt_like(token: &str) -> bool {
    let parts: Vec<&str> = token.split('.').collect();
    parts.len() == 3 && parts.iter().all(|p| !p.is_empty())
}

/// Summarize a `SessionTracker` into a human-readable string.
pub fn session_tracker_summary(tracker: &SessionTracker, ttl: u64, now: u64) -> String {
    let active = tracker.active_sessions(ttl, now).len();
    let expired = tracker.expired_sessions(ttl, now).len();
    format!(
        "{} active, {} expired, {} total",
        active,
        expired,
        tracker.len()
    )
}

/// Return provider IDs that have at least one session in the store.
pub fn providers_with_sessions(store: &SessionStore) -> Vec<String> {
    store
        .all_provider_ids()
        .into_iter()
        .filter(|pid| !store.get_sessions(pid, &[]).is_empty())
        .map(|s| s.to_string())
        .collect()
}

/// Validate that all sessions in a tracker are still alive given a TTL.
pub fn all_sessions_active(tracker: &SessionTracker, ttl: u64, now: u64) -> bool {
    tracker.expired_sessions(ttl, now).is_empty()
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

    // -- AuthProviderCapability --

    #[test]
    fn capability_display() {
        assert_eq!(format!("{}", AuthProviderCapability::SilentRefresh), "silent-refresh");
        assert_eq!(format!("{}", AuthProviderCapability::Pkce), "pkce");
    }

    #[test]
    fn capability_set_add_dedup() {
        let mut set = AuthCapabilitySet::new();
        set.add(AuthProviderCapability::SilentRefresh);
        set.add(AuthProviderCapability::SilentRefresh);
        assert_eq!(set.len(), 1);
    }

    #[test]
    fn capability_set_has_all() {
        let mut set = AuthCapabilitySet::new();
        set.add(AuthProviderCapability::SilentRefresh);
        set.add(AuthProviderCapability::Pkce);
        assert!(set.has_all(&[AuthProviderCapability::SilentRefresh, AuthProviderCapability::Pkce]));
        assert!(!set.has_all(&[AuthProviderCapability::SilentRefresh, AuthProviderCapability::MultiAccount]));
    }

    #[test]
    fn capability_set_has_any() {
        let mut set = AuthCapabilitySet::new();
        set.add(AuthProviderCapability::DeviceCodeFlow);
        assert!(set.has_any(&[AuthProviderCapability::DeviceCodeFlow, AuthProviderCapability::Pkce]));
        assert!(!set.has_any(&[AuthProviderCapability::Pkce, AuthProviderCapability::MultiAccount]));
    }

    #[test]
    fn capability_set_headless() {
        let mut set = AuthCapabilitySet::new();
        assert!(!set.supports_headless());
        set.add(AuthProviderCapability::DeviceCodeFlow);
        assert!(set.supports_headless());
    }

    #[test]
    fn capability_set_secure_lifecycle() {
        let mut set = AuthCapabilitySet::new();
        set.add(AuthProviderCapability::SilentRefresh);
        assert!(!set.supports_secure_lifecycle());
        set.add(AuthProviderCapability::TokenRevocation);
        assert!(set.supports_secure_lifecycle());
    }

    #[test]
    fn capability_set_display() {
        let mut set = AuthCapabilitySet::new();
        set.add(AuthProviderCapability::SilentRefresh);
        set.add(AuthProviderCapability::Pkce);
        let s = format!("{set}");
        assert!(s.contains("silent-refresh"));
        assert!(s.contains("pkce"));
    }

    #[test]
    fn auth_bridge_clear_all() {
        let mut bridge = AuthBridge::new();
        bridge.register_provider("github");
        bridge.register_provider("gitlab");
        assert_eq!(bridge.provider_count(), 2);
        bridge.clear_all();
        assert_eq!(bridge.provider_count(), 0);
    }

    #[test]
    fn session_store_session_count() {
        let mut store = SessionStore::new();
        assert_eq!(store.session_count(), 0);
        store.add_session("gh", AuthSession {
            id: "s1".into(), access_token: "t".into(),
            account: AuthAccount { id: "a".into(), label: "u".into() },
            scopes: vec!["read".into()],
        });
        store.add_session("gl", AuthSession {
            id: "s2".into(), access_token: "t".into(),
            account: AuthAccount { id: "a".into(), label: "u".into() },
            scopes: vec!["write".into()],
        });
        assert_eq!(store.session_count(), 2);
    }

    #[test]
    fn session_store_all_provider_ids() {
        let mut store = SessionStore::new();
        store.add_session("gh", AuthSession {
            id: "s1".into(), access_token: "t".into(),
            account: AuthAccount { id: "a".into(), label: "u".into() },
            scopes: vec!["read".into()],
        });
        let ids = store.all_provider_ids();
        assert_eq!(ids.len(), 1);
        assert!(ids.contains(&"gh"));
    }

    #[test]
    fn auth_session_is_token_present() {
        let session = AuthSession {
            id: "s1".into(), access_token: "tok".into(),
            account: AuthAccount { id: "a".into(), label: "u".into() },
            scopes: vec!["read".into()],
        };
        assert!(session.is_token_present());
        let empty = AuthSession {
            id: "s2".into(), access_token: "".into(),
            account: AuthAccount { id: "a".into(), label: "u".into() },
            scopes: vec![],
        };
        assert!(!empty.is_token_present());
    }

    #[test]
    fn audit_log_event_count_and_clear() {
        let mut log = AuthAuditLog::new();
        assert_eq!(log.event_count(), 0);
        log.record(AuthAuditEvent::ProviderRegistered { provider_id: "gh".into() }, 100);
        log.record(AuthAuditEvent::SessionCreated { provider_id: "gh".into(), session_id: "s1".into() }, 200);
        assert_eq!(log.event_count(), 2);
        log.clear();
        assert_eq!(log.event_count(), 0);
    }

    #[test]
    fn auth_session_display() {
        let session = AuthSession {
            id: "s1".into(), access_token: "tok".into(),
            account: AuthAccount { id: "a".into(), label: "alice".into() },
            scopes: vec!["read".into(), "write".into()],
        };
        let s = format!("{session}");
        assert!(s.contains("alice"));
        assert!(s.contains("read, write"));
    }

    #[test]
    fn auth_token_refresher_attempts_remaining() {
        let mut r = AuthTokenRefresher::new("s1", 3);
        assert_eq!(r.attempts_remaining(), 3);
        r.begin_refresh(100);
        assert_eq!(r.attempts_remaining(), 2);
        r.begin_refresh(200);
        r.begin_refresh(300);
        assert_eq!(r.attempts_remaining(), 0);
    }

    #[test]
    fn validate_token_valid() {
        assert!(validate_token("abc-123_def.ghi", 100).is_ok());
    }

    #[test]
    fn validate_token_empty_is_expired() {
        assert_eq!(validate_token("", 100), Err(AuthError::SessionExpired));
    }

    #[test]
    fn validate_token_too_long() {
        let long = "a".repeat(200);
        assert!(validate_token(&long, 50).is_err());
    }

    #[test]
    fn validate_token_invalid_chars() {
        assert!(validate_token("abc def", 100).is_err());
        assert!(validate_token("abc!@#", 100).is_err());
    }

    #[test]
    fn parse_scopes_deduplicates_and_sorts() {
        let scopes = parse_scopes("write read write admin read");
        assert_eq!(scopes, vec!["admin", "read", "write"]);
    }

    #[test]
    fn scopes_satisfied_checks_subset() {
        let granted = vec!["read".into(), "write".into(), "admin".into()];
        assert!(scopes_satisfied(&granted, &["read".into(), "write".into()]));
        assert!(!scopes_satisfied(&granted, &["read".into(), "delete".into()]));
    }

    #[test]
    fn session_tracker_active_and_expired() {
        let mut tracker = SessionTracker::new();
        let session = AuthSession {
            id: "s1".into(),
            access_token: "tok".into(),
            account: AuthAccount { id: "a1".into(), label: "Alice".into() },
            scopes: vec![],
        };
        tracker.add(session, 100);
        assert_eq!(tracker.active_sessions(3600, 200).len(), 1);
        assert_eq!(tracker.expired_sessions(3600, 200).len(), 0);
        assert_eq!(tracker.active_sessions(50, 200).len(), 0);
        assert_eq!(tracker.expired_sessions(50, 200).len(), 1);
    }

    #[test]
    fn rotation_schedule_is_due() {
        let mut sched = RotationSchedule::new(3600);
        assert!(sched.is_due(0)); // never rotated
        sched.record_rotation(1000);
        assert!(!sched.is_due(2000));
        assert!(sched.is_due(5000));
        assert_eq!(sched.seconds_until_due(2000), 2600);
        assert_eq!(sched.seconds_until_due(5000), 0);
    }

    #[test]
    fn total_session_count_across_providers() {
        let mut store = SessionStore::new();
        store.add_session("gh", AuthSession {
            id: "s1".into(), access_token: "t".into(), account: AuthAccount { label: "a".into(), id: "1".into() }, scopes: vec![]
        });
        store.add_session("gh", AuthSession {
            id: "s2".into(), access_token: "t".into(), account: AuthAccount { label: "b".into(), id: "2".into() }, scopes: vec![]
        });
        store.add_session("ms", AuthSession {
            id: "s3".into(), access_token: "t".into(), account: AuthAccount { label: "c".into(), id: "3".into() }, scopes: vec![]
        });
        assert_eq!(total_session_count(&store), 3);
    }

    #[test]
    fn total_session_count_empty_store() {
        assert_eq!(total_session_count(&SessionStore::new()), 0);
    }

    #[test]
    fn deduplicate_scopes_removes_dups() {
        let scopes = vec!["read".into(), "write".into(), "read".into()];
        let deduped = deduplicate_scopes(&scopes);
        assert_eq!(deduped, vec!["read", "write"]);
    }

    #[test]
    fn deduplicate_scopes_empty() {
        assert!(deduplicate_scopes(&[]).is_empty());
    }

    #[test]
    fn is_jwt_like_valid() {
        assert!(is_jwt_like("header.payload.signature"));
        assert!(!is_jwt_like("not-a-jwt"));
        assert!(!is_jwt_like("a.b."));
        assert!(!is_jwt_like("a..c"));
    }

    #[test]
    fn session_tracker_summary_format() {
        let mut tracker = SessionTracker::new();
        tracker.add(AuthSession {
            id: "s1".into(), access_token: "t".into(),
            account: AuthAccount { label: "a".into(), id: "1".into() },
            scopes: vec![]
        }, 100);
        let s = session_tracker_summary(&tracker, 3600, 200);
        assert!(s.contains("1 active"));
        assert!(s.contains("0 expired"));
    }

    #[test]
    fn all_sessions_active_checks() {
        let mut tracker = SessionTracker::new();
        tracker.add(AuthSession {
            id: "s1".into(), access_token: "t".into(),
            account: AuthAccount { label: "a".into(), id: "1".into() },
            scopes: vec![]
        }, 100);
        assert!(all_sessions_active(&tracker, 3600, 200));
        assert!(!all_sessions_active(&tracker, 10, 200));
    }

    #[test]
    fn providers_with_sessions_filters() {
        let mut store = SessionStore::new();
        store.add_session("gh", AuthSession {
            id: "s1".into(), access_token: "t".into(),
            account: AuthAccount { label: "a".into(), id: "1".into() },
            scopes: vec![]
        });
        let pids = providers_with_sessions(&store);
        assert_eq!(pids, vec!["gh"]);
    }
}
