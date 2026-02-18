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

// ---------------------------------------------------------------------------
// AuthSession builder
// ---------------------------------------------------------------------------

/// Fluent builder for constructing [`AuthSession`] instances.
#[derive(Debug, Clone)]
pub struct AuthSessionBuilder {
    id: Option<String>,
    access_token: Option<String>,
    account_id: Option<String>,
    account_label: Option<String>,
    scopes: Vec<String>,
}

impl AuthSessionBuilder {
    pub fn new() -> Self {
        Self {
            id: None,
            access_token: None,
            account_id: None,
            account_label: None,
            scopes: Vec::new(),
        }
    }

    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    pub fn access_token(mut self, token: impl Into<String>) -> Self {
        self.access_token = Some(token.into());
        self
    }

    pub fn account(mut self, id: impl Into<String>, label: impl Into<String>) -> Self {
        self.account_id = Some(id.into());
        self.account_label = Some(label.into());
        self
    }

    pub fn scope(mut self, scope: impl Into<String>) -> Self {
        self.scopes.push(scope.into());
        self
    }

    pub fn scopes(mut self, scopes: &[&str]) -> Self {
        self.scopes.extend(scopes.iter().map(|s| s.to_string()));
        self
    }

    /// Build the session. Returns `Err` if required fields are missing.
    pub fn build(self) -> Result<AuthSession, AuthError> {
        let id = self.id.ok_or(AuthError::NetworkError("missing session id".into()))?;
        let access_token = self
            .access_token
            .ok_or(AuthError::NetworkError("missing access token".into()))?;
        let account_id = self
            .account_id
            .ok_or(AuthError::NetworkError("missing account id".into()))?;
        let account_label = self
            .account_label
            .ok_or(AuthError::NetworkError("missing account label".into()))?;
        Ok(AuthSession {
            id,
            access_token,
            account: AuthAccount {
                id: account_id,
                label: account_label,
            },
            scopes: self.scopes,
        })
    }
}

impl Default for AuthSessionBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Scope diff utility
// ---------------------------------------------------------------------------

/// Describes the difference between two scope sets.
#[derive(Debug, Clone, PartialEq)]
pub struct ScopeDiff {
    /// Scopes present in `new` but not in `old`.
    pub added: Vec<String>,
    /// Scopes present in `old` but not in `new`.
    pub removed: Vec<String>,
    /// Scopes present in both.
    pub unchanged: Vec<String>,
}

/// Compute the difference between two scope slices.
pub fn diff_scopes(old: &[String], new: &[String]) -> ScopeDiff {
    let mut added: Vec<String> = new.iter().filter(|s| !old.contains(s)).cloned().collect();
    let mut removed: Vec<String> = old.iter().filter(|s| !new.contains(s)).cloned().collect();
    let mut unchanged: Vec<String> = old.iter().filter(|s| new.contains(s)).cloned().collect();
    added.sort();
    removed.sort();
    unchanged.sort();
    ScopeDiff {
        added,
        removed,
        unchanged,
    }
}

impl ScopeDiff {
    /// Returns `true` if scopes are identical (nothing added or removed).
    pub fn is_unchanged(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty()
    }
}

// ---------------------------------------------------------------------------
// SessionStore: find_by_account, replace_session
// ---------------------------------------------------------------------------

impl SessionStore {
    /// Find all sessions belonging to a specific account across all providers.
    pub fn find_by_account(&self, account_id: &str) -> Vec<(&str, &AuthSession)> {
        self.sessions
            .iter()
            .flat_map(|(pid, sessions)| {
                sessions
                    .iter()
                    .filter(|s| s.account.id == account_id)
                    .map(move |s| (pid.as_str(), s))
            })
            .collect()
    }

    /// Replace a session in-place by its id, returning the old session if found.
    pub fn replace_session(
        &mut self,
        provider_id: &str,
        session_id: &str,
        replacement: AuthSession,
    ) -> Option<AuthSession> {
        if let Some(sessions) = self.sessions.get_mut(provider_id) {
            for slot in sessions.iter_mut() {
                if slot.id == session_id {
                    let old = std::mem::replace(slot, replacement);
                    return Some(old);
                }
            }
        }
        None
    }
}

// ---------------------------------------------------------------------------
// AuthBridge: handle_message_result with AuthError
// ---------------------------------------------------------------------------

impl AuthBridge {
    /// Process a message, returning `Err` when the target provider is not registered.
    pub fn handle_message_checked(
        &mut self,
        msg: &AuthMessage,
    ) -> Result<serde_json::Value, AuthError> {
        match msg {
            AuthMessage::RegisterProvider { provider_id, .. } => {
                self.register_provider(provider_id);
                Ok(serde_json::json!({"registered": true}))
            }
            AuthMessage::UnregisterProvider { provider_id } => {
                if !self.has_provider(provider_id) {
                    return Err(AuthError::ProviderNotFound);
                }
                self.unregister_provider(provider_id);
                Ok(serde_json::json!({"unregistered": true}))
            }
            AuthMessage::GetSessions { provider_id, .. } => {
                if !self.has_provider(provider_id) {
                    return Err(AuthError::ProviderNotFound);
                }
                Ok(serde_json::json!({"provider": provider_id, "sessions": []}))
            }
            AuthMessage::SessionsChanged { provider_id } => {
                if !self.has_provider(provider_id) {
                    return Err(AuthError::ProviderNotFound);
                }
                Ok(serde_json::json!({"provider": provider_id, "changed": true}))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// AuthSession: scope helpers
// ---------------------------------------------------------------------------

impl AuthSession {
    /// Returns `true` if the session has a specific scope.
    pub fn has_scope(&self, scope: &str) -> bool {
        self.scopes.iter().any(|s| s == scope)
    }

    /// Returns a new session with an updated access token.
    pub fn with_token(&self, new_token: impl Into<String>) -> Self {
        Self {
            access_token: new_token.into(),
            ..self.clone()
        }
    }
}

// ---------------------------------------------------------------------------
// AuthTokenRefreshStore: manages token refresh with expiry
// ---------------------------------------------------------------------------

/// Manages token storage with expiry tracking for refresh workflows.
pub struct AuthTokenRefreshStore {
    tokens: HashMap<String, StoredToken>,
}

struct StoredToken {
    value: String,
    expires_at_ms: u64,
}

impl AuthTokenRefreshStore {
    pub fn new() -> Self {
        Self {
            tokens: HashMap::new(),
        }
    }

    /// Store a token for a provider with its expiry timestamp.
    pub fn store_token(&mut self, provider: &str, token: &str, expires_at_ms: u64) {
        self.tokens.insert(
            provider.to_string(),
            StoredToken {
                value: token.to_string(),
                expires_at_ms,
            },
        );
    }

    /// Retrieve the token for a provider, if it exists.
    pub fn get_token(&self, provider: &str) -> Option<&str> {
        self.tokens.get(provider).map(|t| t.value.as_str())
    }

    /// Returns `true` if the token for the provider has expired.
    pub fn is_expired(&self, provider: &str, current_time_ms: u64) -> bool {
        self.tokens
            .get(provider)
            .map(|t| current_time_ms >= t.expires_at_ms)
            .unwrap_or(false)
    }

    /// Returns `true` if the token is within `buffer_ms` of expiry.
    pub fn needs_refresh(&self, provider: &str, current_time_ms: u64, buffer_ms: u64) -> bool {
        self.tokens
            .get(provider)
            .map(|t| current_time_ms + buffer_ms >= t.expires_at_ms)
            .unwrap_or(false)
    }

    /// Remove a stored token.
    pub fn remove_token(&mut self, provider: &str) {
        self.tokens.remove(provider);
    }

    /// Returns the number of stored tokens.
    pub fn token_count(&self) -> usize {
        self.tokens.len()
    }
}

impl Default for AuthTokenRefreshStore {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// AuthScopeValidator: validates requested scopes against available
// ---------------------------------------------------------------------------

/// Result of validating requested scopes against available scopes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScopeValidationResult {
    pub valid: bool,
    pub granted: Vec<String>,
    pub denied: Vec<String>,
}

/// Validates requested authentication scopes against a set of available scopes.
pub struct AuthScopeValidator {
    available: Vec<String>,
}

impl AuthScopeValidator {
    pub fn new() -> Self {
        Self {
            available: Vec::new(),
        }
    }

    /// Register a scope as available.
    pub fn register_available_scope(&mut self, scope: &str) {
        if !self.available.iter().any(|s| s == scope) {
            self.available.push(scope.to_string());
        }
    }

    /// Validate a set of requested scopes, splitting into granted and denied.
    pub fn validate_scopes(&self, requested: &[&str]) -> ScopeValidationResult {
        let mut granted = Vec::new();
        let mut denied = Vec::new();

        for &scope in requested {
            if self.available.iter().any(|s| s == scope) {
                granted.push(scope.to_string());
            } else {
                denied.push(scope.to_string());
            }
        }

        ScopeValidationResult {
            valid: denied.is_empty(),
            granted,
            denied,
        }
    }

    /// Returns `true` if a specific scope is available.
    pub fn is_scope_available(&self, scope: &str) -> bool {
        self.available.iter().any(|s| s == scope)
    }

    /// Returns all available scopes.
    pub fn available_scopes(&self) -> Vec<&str> {
        self.available.iter().map(|s| s.as_str()).collect()
    }
}

impl Default for AuthScopeValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// AuthSessionDeduplicator: prevents duplicate auth sessions
// ---------------------------------------------------------------------------

/// Prevents duplicate authentication sessions per provider/account pair.
pub struct AuthSessionDeduplicator {
    registered: Vec<(String, String)>,
}

impl AuthSessionDeduplicator {
    pub fn new() -> Self {
        Self {
            registered: Vec::new(),
        }
    }

    /// Try to register a session. Returns `false` if already registered.
    pub fn try_register(&mut self, provider: &str, account: &str) -> bool {
        if self.is_registered(provider, account) {
            return false;
        }
        self.registered
            .push((provider.to_string(), account.to_string()));
        true
    }

    /// Remove a registration.
    pub fn unregister(&mut self, provider: &str, account: &str) {
        self.registered.retain(|(p, a)| p != provider || a != account);
    }

    /// Check if a provider/account pair is registered.
    pub fn is_registered(&self, provider: &str, account: &str) -> bool {
        self.registered.iter().any(|(p, a)| p == provider && a == account)
    }

    /// Return all registered sessions as `(provider, account)` pairs.
    pub fn sessions(&self) -> Vec<(&str, &str)> {
        self.registered
            .iter()
            .map(|(p, a)| (p.as_str(), a.as_str()))
            .collect()
    }

    /// Returns the number of registered sessions.
    pub fn session_count(&self) -> usize {
        self.registered.len()
    }
}

impl Default for AuthSessionDeduplicator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// AuthProviderCapabilities: discovers provider capabilities
// ---------------------------------------------------------------------------

/// Describes the capabilities of an authentication provider.
pub struct AuthProviderCapabilities {
    provider_id: String,
    multi_account: bool,
    logout: bool,
    token_refresh: bool,
}

impl AuthProviderCapabilities {
    pub fn new(provider_id: &str) -> Self {
        Self {
            provider_id: provider_id.to_string(),
            multi_account: false,
            logout: false,
            token_refresh: false,
        }
    }

    pub fn set_supports_multi_account(&mut self, v: bool) {
        self.multi_account = v;
    }

    pub fn set_supports_logout(&mut self, v: bool) {
        self.logout = v;
    }

    pub fn set_supports_token_refresh(&mut self, v: bool) {
        self.token_refresh = v;
    }

    pub fn supports_multi_account(&self) -> bool {
        self.multi_account
    }

    pub fn supports_logout(&self) -> bool {
        self.logout
    }

    pub fn supports_token_refresh(&self) -> bool {
        self.token_refresh
    }

    pub fn provider_id(&self) -> &str {
        &self.provider_id
    }
}

impl fmt::Display for AuthProviderCapabilities {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut caps = Vec::new();
        if self.multi_account {
            caps.push("multi-account");
        }
        if self.logout {
            caps.push("logout");
        }
        if self.token_refresh {
            caps.push("token-refresh");
        }
        if caps.is_empty() {
            write!(f, "{}: (none)", self.provider_id)
        } else {
            write!(f, "{}: {}", self.provider_id, caps.join(", "))
        }
    }
}


// ─── AuthBuf Ring Buffer ──────────────────────────────────────

/// A fixed-capacity ring buffer for auth events.
#[derive(Debug, Clone)]
pub struct AuthBufRingBuffer<T> {
    buf: Vec<Option<T>>,
    head: usize,
    len: usize,
}

impl<T: Clone> AuthBufRingBuffer<T> {
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "capacity must be > 0");
        Self { buf: vec![None; capacity], head: 0, len: 0 }
    }

    pub fn push(&mut self, item: T) {
        let cap = self.buf.len();
        let idx = (self.head + self.len) % cap;
        self.buf[idx] = Some(item);
        if self.len == cap { self.head = (self.head + 1) % cap; }
        else { self.len += 1; }
    }

    pub fn len(&self) -> usize { self.len }
    pub fn is_empty(&self) -> bool { self.len == 0 }
    pub fn is_full(&self) -> bool { self.len == self.buf.len() }
    pub fn capacity(&self) -> usize { self.buf.len() }

    pub fn get(&self, index: usize) -> Option<&T> {
        if index >= self.len { return None; }
        self.buf[(self.head + index) % self.buf.len()].as_ref()
    }

    pub fn iter(&self) -> Vec<&T> {
        let cap = self.buf.len();
        (0..self.len).filter_map(|i| self.buf[(self.head + i) % cap].as_ref()).collect()
    }

    pub fn clear(&mut self) {
        for slot in &mut self.buf { *slot = None; }
        self.head = 0;
        self.len = 0;
    }

    pub fn to_vec(&self) -> Vec<T> { self.iter().into_iter().cloned().collect() }

    pub fn newest(&self) -> Option<&T> {
        if self.len == 0 { return None; }
        self.buf[(self.head + self.len - 1) % self.buf.len()].as_ref()
    }

    pub fn oldest(&self) -> Option<&T> {
        if self.len == 0 { return None; }
        self.buf[self.head].as_ref()
    }
}

impl<T: Clone + fmt::Display> fmt::Display for AuthBufRingBuffer<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "AuthBufRingBuffer(len={}, cap={})", self.len, self.capacity())
    }
}

// ─── AuthC LRU Cache ───────────────────────────────────────

/// A simple LRU cache for auth tokens.
#[derive(Debug)]
pub struct AuthCLruCache<V> {
    entries: Vec<(String, V)>,
    capacity: usize,
    hits: u64,
    misses: u64,
}

impl<V: Clone> AuthCLruCache<V> {
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0);
        Self { entries: Vec::with_capacity(capacity), capacity, hits: 0, misses: 0 }
    }

    pub fn insert(&mut self, key: impl Into<String>, value: V) -> Option<(String, V)> {
        let key = key.into();
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == &key) {
            self.entries.remove(pos);
            self.entries.insert(0, (key, value));
            return None;
        }
        let evicted = if self.entries.len() >= self.capacity {
            Some(self.entries.pop().unwrap())
        } else { None };
        self.entries.insert(0, (key, value));
        evicted
    }

    pub fn get(&mut self, key: &str) -> Option<&V> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            self.hits += 1;
            let entry = self.entries.remove(pos);
            self.entries.insert(0, entry);
            Some(&self.entries[0].1)
        } else {
            self.misses += 1;
            None
        }
    }

    pub fn peek(&self, key: &str) -> Option<&V> {
        self.entries.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    pub fn remove(&mut self, key: &str) -> Option<V> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            Some(self.entries.remove(pos).1)
        } else { None }
    }

    pub fn len(&self) -> usize { self.entries.len() }
    pub fn is_empty(&self) -> bool { self.entries.is_empty() }

    pub fn hit_ratio(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 { 0.0 } else { self.hits as f64 / total as f64 }
    }

    pub fn hits(&self) -> u64 { self.hits }
    pub fn misses(&self) -> u64 { self.misses }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.hits = 0;
        self.misses = 0;
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    pub fn contains(&self, key: &str) -> bool {
        self.entries.iter().any(|(k, _)| k == key)
    }
}

impl<V: Clone + fmt::Display> fmt::Display for AuthCLruCache<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "AuthCLruCache(size={}, cap={}, hits={}, misses={})",
            self.len(), self.capacity, self.hits, self.misses)
    }
}


/// Configuration manager for ext_auth functionality.
pub struct ExtAuthConfig {
    options: HashMap<String, String>,
    enabled: bool,
    version: u32,
}

impl ExtAuthConfig {
    pub fn new() -> Self {
        Self { options: HashMap::new(), enabled: true, version: 1 }
    }

    pub fn set_option(&mut self, key: &str, value: &str) {
        self.options.insert(key.to_string(), value.to_string());
    }

    pub fn get_option(&self, key: &str) -> Option<&str> {
        self.options.get(key).map(|s| s.as_str())
    }

    pub fn remove_option(&mut self, key: &str) -> Option<String> {
        self.options.remove(key)
    }

    pub fn option_count(&self) -> usize { self.options.len() }

    pub fn is_enabled(&self) -> bool { self.enabled }

    pub fn set_enabled(&mut self, enabled: bool) { self.enabled = enabled; }

    pub fn version(&self) -> u32 { self.version }

    pub fn bump_version(&mut self) { self.version += 1; }

    pub fn has_option(&self, key: &str) -> bool { self.options.contains_key(key) }

    pub fn option_keys(&self) -> Vec<String> {
        let mut keys: Vec<_> = self.options.keys().cloned().collect();
        keys.sort();
        keys
    }

    pub fn clear(&mut self) {
        self.options.clear();
        self.version = 1;
    }

    pub fn merge(&mut self, other: &ExtAuthConfig) {
        for (k, v) in &other.options {
            self.options.insert(k.clone(), v.clone());
        }
    }
}

/// Rate tracker for ext_auth operations.
pub struct ExtAuthRateTracker {
    window_ms: u64,
    timestamps: Vec<u64>,
}

impl ExtAuthRateTracker {
    pub fn new(window_ms: u64) -> Self {
        Self { window_ms, timestamps: Vec::new() }
    }

    pub fn record(&mut self, ts: u64) {
        self.timestamps.push(ts);
        self.prune(ts);
    }

    fn prune(&mut self, now: u64) {
        let cutoff = now.saturating_sub(self.window_ms);
        self.timestamps.retain(|&t| t >= cutoff);
    }

    pub fn count(&self) -> usize { self.timestamps.len() }

    pub fn rate_per_second(&self) -> f64 {
        if self.timestamps.len() < 2 { return 0.0; }
        let span = self.timestamps.last().unwrap() - self.timestamps.first().unwrap();
        if span == 0 { return 0.0; }
        (self.timestamps.len() as f64 / span as f64) * 1000.0
    }

    pub fn clear(&mut self) { self.timestamps.clear(); }

    pub fn window_ms(&self) -> u64 { self.window_ms }
}

/// Validation result collector for ext_auth.
pub struct ExtAuthValidator {
    errors: Vec<String>,
    warnings: Vec<String>,
}

impl ExtAuthValidator {
    pub fn new() -> Self {
        Self { errors: Vec::new(), warnings: Vec::new() }
    }

    pub fn add_error(&mut self, msg: &str) {
        self.errors.push(msg.to_string());
    }

    pub fn add_warning(&mut self, msg: &str) {
        self.warnings.push(msg.to_string());
    }

    pub fn is_valid(&self) -> bool { self.errors.is_empty() }

    pub fn error_count(&self) -> usize { self.errors.len() }

    pub fn warning_count(&self) -> usize { self.warnings.len() }

    pub fn errors(&self) -> &[String] { &self.errors }

    pub fn warnings(&self) -> &[String] { &self.warnings }

    pub fn clear(&mut self) {
        self.errors.clear();
        self.warnings.clear();
    }

    pub fn merge(&mut self, other: &ExtAuthValidator) {
        self.errors.extend(other.errors.iter().cloned());
        self.warnings.extend(other.warnings.iter().cloned());
    }

    pub fn first_error(&self) -> Option<&str> {
        self.errors.first().map(|s| s.as_str())
    }
}


// ---------------------------------------------------------------------------
// Authentication provider for extensions — extended utilities (yi)
// ---------------------------------------------------------------------------

/// Metric accumulator for ext_auth operations.
#[derive(Debug, Clone)]
pub struct YiMetrics {
    samples: Vec<f64>,
    label: String,
}

impl YiMetrics {
    pub fn new(label: &str) -> Self {
        Self { samples: Vec::new(), label: label.to_string() }
    }

    pub fn record(&mut self, value: f64) {
        self.samples.push(value);
    }

    pub fn mean(&self) -> f64 {
        if self.samples.is_empty() { return 0.0; }
        self.samples.iter().sum::<f64>() / self.samples.len() as f64
    }

    pub fn max_val(&self) -> f64 {
        self.samples.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
    }

    pub fn min_val(&self) -> f64 {
        self.samples.iter().cloned().fold(f64::INFINITY, f64::min)
    }

    pub fn count(&self) -> usize {
        self.samples.len()
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn reset(&mut self) {
        self.samples.clear();
    }

    pub fn variance(&self) -> f64 {
        if self.samples.len() < 2 { return 0.0; }
        let m = self.mean();
        let sq: f64 = self.samples.iter().map(|v| (v - m).powi(2)).sum();
        sq / (self.samples.len() as f64 - 1.0)
    }

    pub fn std_dev(&self) -> f64 {
        self.variance().sqrt()
    }

    pub fn percentile(&self, p: f64) -> f64 {
        if self.samples.is_empty() { return 0.0; }
        let mut sorted = self.samples.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let idx = ((p / 100.0) * (sorted.len() as f64 - 1.0)).round() as usize;
        sorted[idx.min(sorted.len() - 1)]
    }

    pub fn sum(&self) -> f64 {
        self.samples.iter().sum()
    }

    pub fn merge(&mut self, other: &Self) {
        self.samples.extend_from_slice(&other.samples);
    }
}

/// Sliding-window rate counter for ext_auth.
#[derive(Debug, Clone)]
pub struct YiRateWindow {
    timestamps: Vec<u64>,
    window_ms: u64,
}

impl YiRateWindow {
    pub fn new(window_ms: u64) -> Self {
        Self { timestamps: Vec::new(), window_ms }
    }

    pub fn tick(&mut self, now_ms: u64) {
        self.timestamps.push(now_ms);
        self.prune(now_ms);
    }

    fn prune(&mut self, now_ms: u64) {
        let cutoff = now_ms.saturating_sub(self.window_ms);
        self.timestamps.retain(|&t| t >= cutoff);
    }

    pub fn rate(&mut self, now_ms: u64) -> usize {
        self.prune(now_ms);
        self.timestamps.len()
    }

    pub fn clear(&mut self) {
        self.timestamps.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.timestamps.is_empty()
    }

    pub fn window_ms(&self) -> u64 {
        self.window_ms
    }
}

/// A small LRU-style cache for ext_auth lookups.
#[derive(Debug, Clone)]
pub struct YiLruCache {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl YiLruCache {
    pub fn new(capacity: usize) -> Self {
        Self { entries: Vec::new(), capacity }
    }

    pub fn get(&mut self, key: &str) -> Option<String> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            let entry = self.entries.remove(pos);
            let val = entry.1.clone();
            self.entries.push(entry);
            Some(val)
        } else {
            None
        }
    }

    pub fn put(&mut self, key: String, value: String) {
        self.entries.retain(|(k, _)| k != &key);
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value));
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.entries.iter().any(|(k, _)| k == key)
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    pub fn remove(&mut self, key: &str) -> Option<String> {
        if let Some(pos) = self.entries.iter().position(|(k, _)| k == key) {
            Some(self.entries.remove(pos).1)
        } else {
            None
        }
    }
}


// ---------------------------------------------------------------------------
// xa_ extended helpers for ext_auth
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaExtAuthRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaExtAuthRingBuf {
    /// Create a new ring buffer with the given capacity.
    pub fn new(cap: usize) -> Self {
        assert!(cap > 0, "capacity must be > 0");
        Self {
            buf: vec![0.0; cap],
            cap,
            head: 0,
            len: 0,
        }
    }

    /// Push a value into the ring buffer.
    pub fn push(&mut self, v: f64) {
        let idx = (self.head + self.len) % self.cap;
        self.buf[idx] = v;
        if self.len == self.cap {
            self.head = (self.head + 1) % self.cap;
        } else {
            self.len += 1;
        }
    }

    /// Return the number of items currently stored.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Return the arithmetic mean, or `None` if empty.
    pub fn mean(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        let sum: f64 = (0..self.len)
            .map(|i| self.buf[(self.head + i) % self.cap])
            .sum();
        Some(sum / self.len as f64)
    }

    /// Return the minimum value, or `None` if empty.
    pub fn min_val(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        Some(
            (0..self.len)
                .map(|i| self.buf[(self.head + i) % self.cap])
                .fold(f64::INFINITY, f64::min),
        )
    }

    /// Return the maximum value, or `None` if empty.
    pub fn max_val(&self) -> Option<f64> {
        if self.len == 0 {
            return None;
        }
        Some(
            (0..self.len)
                .map(|i| self.buf[(self.head + i) % self.cap])
                .fold(f64::NEG_INFINITY, f64::max),
        )
    }

    /// Drain all elements as a `Vec` in insertion order.
    pub fn drain_to_vec(&mut self) -> Vec<f64> {
        let v: Vec<f64> = (0..self.len)
            .map(|i| self.buf[(self.head + i) % self.cap])
            .collect();
        self.head = 0;
        self.len = 0;
        v
    }

    /// Iterate over elements in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = f64> + '_ {
        (0..self.len).map(move |i| self.buf[(self.head + i) % self.cap])
    }
}

/// Simple string-keyed counter map used by `xa_` utilities.
pub struct XaExtAuthCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaExtAuthCounter {
    /// Create an empty counter.
    pub fn new() -> Self {
        Self {
            counts: std::collections::HashMap::new(),
        }
    }

    /// Increment key by one.
    pub fn inc(&mut self, key: &str) {
        *self.counts.entry(key.to_owned()).or_insert(0) += 1;
    }

    /// Increment key by an arbitrary delta.
    pub fn inc_by(&mut self, key: &str, delta: u64) {
        *self.counts.entry(key.to_owned()).or_insert(0) += delta;
    }

    /// Get the current count (0 if absent).
    pub fn get(&self, key: &str) -> u64 {
        self.counts.get(key).copied().unwrap_or(0)
    }

    /// Return the total across all keys.
    pub fn total(&self) -> u64 {
        self.counts.values().sum()
    }

    /// Return the number of distinct keys.
    pub fn num_keys(&self) -> usize {
        self.counts.len()
    }

    /// Reset all counts to zero (keeps keys).
    pub fn reset(&mut self) {
        for v in self.counts.values_mut() {
            *v = 0;
        }
    }

    /// Remove all keys.
    pub fn clear(&mut self) {
        self.counts.clear();
    }
}

impl Default for XaExtAuthCounter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 50
// ---------------------------------------------------------------------------

/// Generic object pool `Xc50Pool<T>`.
pub struct Xc50Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc50Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc50PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc50Pool<T> {
    /// Create a pool with the given maximum capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            items: Vec::with_capacity(capacity),
            capacity,
            acquired: 0,
        }
    }

    /// Try to acquire an item from the pool.
    pub fn acquire(&mut self) -> Option<T> {
        if let Some(item) = self.items.pop() {
            self.acquired += 1;
            Some(item)
        } else {
            None
        }
    }

    /// Release an item back into the pool.
    pub fn release(&mut self, item: T) {
        if self.items.len() < self.capacity {
            self.items.push(item);
            if self.acquired > 0 {
                self.acquired -= 1;
            }
        }
    }

    /// Number of items currently stored in the pool.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Maximum capacity of the pool.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Number of items available for acquisition.
    pub fn available(&self) -> usize {
        self.items.len()
    }

    /// Drain all items from the pool.
    pub fn drain(&mut self) -> Vec<T> {
        self.acquired = 0;
        self.items.drain(..).collect()
    }

    /// Whether the pool is at capacity.
    pub fn is_full(&self) -> bool {
        self.items.len() >= self.capacity
    }

    /// Whether the pool is empty.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Return a statistics snapshot.
    pub fn stats(&self) -> Xc50PoolStats {
        Xc50PoolStats {
            capacity: self.capacity,
            len: self.items.len(),
            acquired: self.acquired,
            available: self.items.len(),
        }
    }

    /// Remove all items and reset counters.
    pub fn clear(&mut self) {
        self.items.clear();
        self.acquired = 0;
    }

    /// Shrink internal storage to fit current length.
    pub fn shrink_to_fit(&mut self) {
        self.items.shrink_to_fit();
    }

    /// Extend pool with an iterator of items (up to remaining capacity).
    pub fn extend_from<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        for item in iter {
            if self.items.len() >= self.capacity {
                break;
            }
            self.items.push(item);
        }
    }

    /// Retain only items matching a predicate.
    pub fn retain<F: FnMut(&T) -> bool>(&mut self, f: F) {
        self.items.retain(f);
    }
}

impl<T> Default for Xc50Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc50Scheduler`.
pub struct Xc50Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc50Scheduler {
    /// Create a scheduler with the given targets.
    pub fn new(targets: Vec<String>) -> Self {
        Self {
            targets,
            index: 0,
            dispatched: 0,
        }
    }

    /// Get the next target in round-robin order.
    pub fn next(&mut self) -> Option<&str> {
        if self.targets.is_empty() {
            return None;
        }
        let target = &self.targets[self.index % self.targets.len()];
        self.index += 1;
        self.dispatched += 1;
        Some(target)
    }

    /// Number of targets.
    pub fn len(&self) -> usize {
        self.targets.len()
    }

    /// Whether there are no targets.
    pub fn is_empty(&self) -> bool {
        self.targets.is_empty()
    }

    /// Total number of dispatches so far.
    pub fn dispatched(&self) -> usize {
        self.dispatched
    }

    /// Current index position.
    pub fn position(&self) -> usize {
        if self.targets.is_empty() {
            0
        } else {
            self.index % self.targets.len()
        }
    }

    /// Reset the scheduler to the beginning.
    pub fn reset(&mut self) {
        self.index = 0;
        self.dispatched = 0;
    }

    /// Add a target.
    pub fn add_target(&mut self, target: String) {
        self.targets.push(target);
    }

    /// Remove a target by name (first occurrence).
    pub fn remove_target(&mut self, name: &str) -> bool {
        if let Some(pos) = self.targets.iter().position(|t| t == name) {
            self.targets.remove(pos);
            if !self.targets.is_empty() {
                self.index %= self.targets.len();
            } else {
                self.index = 0;
            }
            true
        } else {
            false
        }
    }

    /// Get all targets.
    pub fn targets(&self) -> &[String] {
        &self.targets
    }
}

impl Default for Xc50Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_50 hash for the given byte slice.
pub fn xc_50_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_50 convention.
pub fn xc_50_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// --- xd_86 deepening: state machine + event bus ---

/// States for the Xd86 state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Xd86State {
    Idle,
    Running,
    Paused,
    Done,
}

impl std::fmt::Display for Xd86State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Idle => write!(f, "Idle"),
            Self::Running => write!(f, "Running"),
            Self::Paused => write!(f, "Paused"),
            Self::Done => write!(f, "Done"),
        }
    }
}

/// Transition record for history tracking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xd86Transition {
    pub from: Xd86State,
    pub to: Xd86State,
    pub step: usize,
}

/// State machine with history tracking and serialization.
pub struct Xd86StateMachine {
    current: Xd86State,
    history: Vec<Xd86Transition>,
    step_counter: usize,
}

impl Xd86StateMachine {
    pub fn new() -> Self {
        Self {
            current: Xd86State::Idle,
            history: Vec::new(),
            step_counter: 0,
        }
    }

    pub fn current_state(&self) -> Xd86State {
        self.current
    }

    pub fn history(&self) -> &[Xd86Transition] {
        &self.history
    }

    pub fn step_count(&self) -> usize {
        self.step_counter
    }

    /// Attempt a state transition. Returns Ok(new_state) or Err with reason.
    pub fn transition(&mut self, target: Xd86State) -> Result<Xd86State, String> {
        let allowed = match (self.current, target) {
            (Xd86State::Idle, Xd86State::Running) => true,
            (Xd86State::Running, Xd86State::Paused) => true,
            (Xd86State::Running, Xd86State::Done) => true,
            (Xd86State::Paused, Xd86State::Running) => true,
            (Xd86State::Paused, Xd86State::Done) => true,
            (Xd86State::Done, Xd86State::Idle) => true,
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "xd_86: invalid transition {} -> {}",
                self.current, target
            ));
        }
        let t = Xd86Transition {
            from: self.current,
            to: target,
            step: self.step_counter,
        };
        self.step_counter += 1;
        self.current = target;
        self.history.push(t);
        Ok(self.current)
    }

    /// Serialize state machine to a simple string representation.
    pub fn serialize(&self) -> String {
        let hist: Vec<String> = self
            .history
            .iter()
            .map(|t| format!("{}->{}@{}", t.from, t.to, t.step))
            .collect();
        format!(
            "Xd86SM[current={},steps={},history=[{}]]",
            self.current,
            self.step_counter,
            hist.join(";")
        )
    }

    /// Deserialize from the serialized string, recovering current state.
    pub fn deserialize_current(s: &str) -> Option<Xd86State> {
        let prefix = "Xd86SM[current=";
        if !s.starts_with(prefix) {
            return None;
        }
        let rest = &s[prefix.len()..];
        let end = rest.find(',')?;
        match &rest[..end] {
            "Idle" => Some(Xd86State::Idle),
            "Running" => Some(Xd86State::Running),
            "Paused" => Some(Xd86State::Paused),
            "Done" => Some(Xd86State::Done),
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.current = Xd86State::Idle;
        self.history.clear();
        self.step_counter = 0;
    }
}

/// Typed events for the Xd86 event bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Xd86Event {
    Started(String),
    Stopped(String),
    Error(String),
    Custom(String, String),
}

impl Xd86Event {
    pub fn kind(&self) -> &str {
        match self {
            Self::Started(_) => "started",
            Self::Stopped(_) => "stopped",
            Self::Error(_) => "error",
            Self::Custom(k, _) => k.as_str(),
        }
    }

    pub fn payload(&self) -> &str {
        match self {
            Self::Started(p) | Self::Stopped(p) | Self::Error(p) => p.as_str(),
            Self::Custom(_, p) => p.as_str(),
        }
    }
}

type Xd86HandlerFn = Box<dyn Fn(&Xd86Event) + Send + Sync>;

/// Event bus with subscribe/publish/unsubscribe and filtering.
pub struct Xd86EventBus {
    handlers: Vec<(usize, Option<String>, Xd86HandlerFn)>,
    next_id: usize,
    published: Vec<Xd86Event>,
}

impl Xd86EventBus {
    pub fn new() -> Self {
        Self {
            handlers: Vec::new(),
            next_id: 0,
            published: Vec::new(),
        }
    }

    /// Subscribe to all events. Returns a subscription id.
    pub fn subscribe<F>(&mut self, handler: F) -> usize
    where
        F: Fn(&Xd86Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers.push((id, None, Box::new(handler)));
        id
    }

    /// Subscribe only to events matching a specific kind filter.
    pub fn subscribe_filtered<F>(&mut self, kind_filter: &str, handler: F) -> usize
    where
        F: Fn(&Xd86Event) + Send + Sync + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        self.handlers
            .push((id, Some(kind_filter.to_string()), Box::new(handler)));
        id
    }

    /// Unsubscribe by subscription id.
    pub fn unsubscribe(&mut self, sub_id: usize) -> bool {
        let before = self.handlers.len();
        self.handlers.retain(|(id, _, _)| *id != sub_id);
        self.handlers.len() < before
    }

    /// Publish an event to all matching subscribers.
    pub fn publish(&mut self, event: Xd86Event) {
        for (_, filter, handler) in &self.handlers {
            let matched = match filter {
                None => true,
                Some(f) => event.kind() == f.as_str(),
            };
            if matched {
                handler(&event);
            }
        }
        self.published.push(event);
    }

    pub fn published_events(&self) -> &[Xd86Event] {
        &self.published
    }

    pub fn subscriber_count(&self) -> usize {
        self.handlers.len()
    }

    pub fn clear_history(&mut self) {
        self.published.clear();
    }
}


// ---------------------------------------------------------------------------
// xf_ data structures (Trie + BloomFilter) — unique instance #107
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf107Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf107TrieNode {
    children: std::collections::HashMap<char, Xf107TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf107Trie {
    root: Xf107TrieNode,
    count: usize,
}

impl Xf107Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf107TrieNode::default(), count: 0 }
    }

    /// Insert a word into the trie.
    pub fn xf_insert(&mut self, word: &str) {
        let mut node = &mut self.root;
        for ch in word.chars() {
            node = node.children.entry(ch).or_default();
        }
        if !node.is_end {
            node.is_end = true;
            self.count += 1;
        }
    }

    /// Return `true` if the exact word exists in the trie.
    pub fn xf_search(&self, word: &str) -> bool {
        let mut node = &self.root;
        for ch in word.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return false,
            }
        }
        node.is_end
    }

    /// Return `true` if any word in the trie starts with `prefix`.
    pub fn xf_starts_with(&self, prefix: &str) -> bool {
        let mut node = &self.root;
        for ch in prefix.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return false,
            }
        }
        true
    }

    /// Remove a word. Returns `true` if it was present.
    pub fn xf_remove(&mut self, word: &str) -> bool {
        if Self::xf_remove_recursive(&mut self.root, word, 0) {
            self.count -= 1;
            true
        } else {
            false
        }
    }

    fn xf_remove_recursive(node: &mut Xf107TrieNode, word: &str, depth: usize) -> bool {
        let chars: Vec<char> = word.chars().collect();
        if depth == chars.len() {
            if !node.is_end {
                return false;
            }
            node.is_end = false;
            return node.children.is_empty();
        }
        let ch = chars[depth];
        let should_delete = {
            if let Some(child) = node.children.get_mut(&ch) {
                Self::xf_remove_recursive(child, word, depth + 1)
            } else {
                return false;
            }
        };
        if should_delete {
            node.children.remove(&ch);
            return !node.is_end && node.children.is_empty();
        }
        false
    }

    /// Number of distinct words stored.
    pub fn xf_word_count(&self) -> usize {
        self.count
    }

    /// Return the longest prefix of `query` that exists as a word in the trie.
    pub fn xf_longest_prefix(&self, query: &str) -> Option<String> {
        let mut node = &self.root;
        let mut last_match: Option<usize> = None;
        for (i, ch) in query.chars().enumerate() {
            match node.children.get(&ch) {
                Some(n) => {
                    node = n;
                    if node.is_end {
                        last_match = Some(i + 1);
                    }
                }
                None => break,
            }
        }
        last_match.map(|end| query.chars().take(end).collect())
    }

    /// Collect every word in the trie.
    pub fn xf_all_words(&self) -> Vec<String> {
        let mut results = Vec::new();
        let mut buffer = String::new();
        Self::xf_collect(&self.root, &mut buffer, &mut results);
        results
    }

    fn xf_collect(node: &Xf107TrieNode, buf: &mut String, out: &mut Vec<String>) {
        if node.is_end {
            out.push(buf.clone());
        }
        let mut keys: Vec<char> = node.children.keys().copied().collect();
        keys.sort();
        for ch in keys {
            buf.push(ch);
            Self::xf_collect(&node.children[&ch], buf, out);
            buf.pop();
        }
    }

    /// Return all words that start with the given prefix.
    pub fn xf_autocomplete(&self, prefix: &str) -> Vec<String> {
        let mut node = &self.root;
        for ch in prefix.chars() {
            match node.children.get(&ch) {
                Some(n) => node = n,
                None => return Vec::new(),
            }
        }
        let mut results = Vec::new();
        let mut buf = prefix.to_string();
        Self::xf_collect(node, &mut buf, &mut results);
        results
    }
}

// ---------------------------------------------------------------------------

/// Simple Bloom filter using two hash functions.
#[derive(Debug, Clone)]
pub struct Xf107BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf107BloomFilter {
    /// Create a Bloom filter with `size` bits and `num_hashes` hash functions.
    pub fn xf_new(size: usize, num_hashes: usize) -> Self {
        Self { bits: vec![false; size], num_hashes, len: size, item_count: 0 }
    }

    fn xf_hashes(&self, item: &str) -> Vec<usize> {
        let mut h1: u64 = 0;
        let mut h2: u64 = 0;
        for (i, b) in item.bytes().enumerate() {
            h1 = h1.wrapping_mul(31).wrapping_add(b as u64);
            h2 = h2.wrapping_mul(37).wrapping_add((b as u64).wrapping_add(i as u64));
        }
        (0..self.num_hashes)
            .map(|i| (h1.wrapping_add((i as u64).wrapping_mul(h2))) as usize % self.len)
            .collect()
    }

    /// Add an item to the filter.
    pub fn xf_add(&mut self, item: &str) {
        for idx in self.xf_hashes(item) {
            self.bits[idx] = true;
        }
        self.item_count += 1;
    }

    /// Check if an item might be in the filter.
    pub fn xf_might_contain(&self, item: &str) -> bool {
        self.xf_hashes(item).iter().all(|&idx| self.bits[idx])
    }

    /// Estimated false-positive rate.
    pub fn xf_false_positive_rate(&self) -> f64 {
        let set_bits = self.bits.iter().filter(|&&b| b).count() as f64;
        let ratio = set_bits / self.len as f64;
        ratio.powi(self.num_hashes as i32)
    }

    /// Clear all bits.
    pub fn xf_clear(&mut self) {
        for b in self.bits.iter_mut() {
            *b = false;
        }
        self.item_count = 0;
    }

    /// Bitwise OR union of two filters (must be same size).
    pub fn xf_union(&self, other: &Self) -> Option<Self> {
        if self.len != other.len || self.num_hashes != other.num_hashes {
            return None;
        }
        let bits = self.bits.iter().zip(&other.bits).map(|(&a, &b)| a || b).collect();
        Some(Self { bits, num_hashes: self.num_hashes, len: self.len, item_count: self.item_count + other.item_count })
    }

    /// Estimate intersection size using inclusion-exclusion on bit counts.
    pub fn xf_intersection_estimate(&self, other: &Self) -> f64 {
        if self.len != other.len {
            return 0.0;
        }
        let both = self.bits.iter().zip(&other.bits).filter(|(a, b)| **a && **b).count();
        both as f64
    }
}


/// A probabilistic sorted list using a skip-list structure (variant 49).
pub struct Xh49SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh49SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 91 as u64,
        }
    }

    fn xh_random_level(&mut self) -> usize {
        self.xh_seed ^= self.xh_seed << 13;
        self.xh_seed ^= self.xh_seed >> 7;
        self.xh_seed ^= self.xh_seed << 17;
        let mut lvl = 1;
        while lvl < self.xh_max_level && (self.xh_seed & 1) == 0 {
            lvl += 1;
            self.xh_seed ^= self.xh_seed.wrapping_mul(6364136223846793005);
        }
        lvl
    }

    /// Insert a value into the skip list.
    pub fn xh_insert(&mut self, value: i64) {
        let pos = self.xh_data.len();
        self.xh_data.push(value);
        let lvl = self.xh_random_level();
        for i in 0..lvl {
            self.xh_levels[i].push((value, pos));
            self.xh_levels[i].sort_by_key(|&(v, _)| v);
        }
        self.xh_len += 1;
    }

    /// Check whether the skip list contains the given value.
    pub fn xh_contains(&self, value: i64) -> bool {
        if self.xh_levels.is_empty() {
            return false;
        }
        self.xh_levels[0].binary_search_by_key(&value, |&(v, _)| v).is_ok()
    }

    /// Remove one occurrence of `value`. Returns `true` if found.
    pub fn xh_remove(&mut self, value: i64) -> bool {
        let mut found = false;
        for level in &mut self.xh_levels {
            if let Ok(idx) = level.binary_search_by_key(&value, |&(v, _)| v) {
                level.remove(idx);
                found = true;
            }
        }
        if found {
            self.xh_len -= 1;
        }
        found
    }

    /// Return the number of elements.
    pub fn xh_len(&self) -> usize {
        self.xh_len
    }

    /// Collect values in `[lo, hi]` inclusive.
    pub fn xh_range_query(&self, lo: i64, hi: i64) -> Vec<i64> {
        if self.xh_levels.is_empty() {
            return Vec::new();
        }
        self.xh_levels[0]
            .iter()
            .filter(|&&(v, _)| v >= lo && v <= hi)
            .map(|&(v, _)| v)
            .collect()
    }

    /// Greatest value <= `value`, if any.
    pub fn xh_floor(&self, value: i64) -> Option<i64> {
        if self.xh_levels.is_empty() {
            return None;
        }
        self.xh_levels[0]
            .iter()
            .rev()
            .find(|&&(v, _)| v <= value)
            .map(|&(v, _)| v)
    }

    /// Smallest value >= `value`, if any.
    pub fn xh_ceiling(&self, value: i64) -> Option<i64> {
        if self.xh_levels.is_empty() {
            return None;
        }
        self.xh_levels[0]
            .iter()
            .find(|&&(v, _)| v >= value)
            .map(|&(v, _)| v)
    }

    /// Number of elements strictly less than `value`.
    pub fn xh_rank(&self, value: i64) -> usize {
        if self.xh_levels.is_empty() {
            return 0;
        }
        self.xh_levels[0]
            .iter()
            .take_while(|&&(v, _)| v < value)
            .count()
    }
}

/// A compact bit set supporting boolean operations (variant 49).
pub struct Xh49BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh49BitSet {
    /// Create a bit set that can hold `nbits` bits.
    pub fn xh_new(nbits: usize) -> Self {
        let nwords = (nbits + 63) / 64;
        Self {
            xh_words: vec![0u64; nwords],
            xh_nbits: nbits,
        }
    }

    /// Set bit at `index`.
    pub fn xh_set(&mut self, index: usize) {
        if index < self.xh_nbits {
            self.xh_words[index / 64] |= 1u64 << (index % 64);
        }
    }

    /// Clear bit at `index`.
    pub fn xh_clear(&mut self, index: usize) {
        if index < self.xh_nbits {
            self.xh_words[index / 64] &= !(1u64 << (index % 64));
        }
    }

    /// Test whether bit at `index` is set.
    pub fn xh_test(&self, index: usize) -> bool {
        if index >= self.xh_nbits {
            return false;
        }
        (self.xh_words[index / 64] >> (index % 64)) & 1 == 1
    }

    /// Count the number of set bits.
    pub fn xh_count(&self) -> usize {
        self.xh_words.iter().map(|w| w.count_ones() as usize).sum()
    }

    /// Bitwise AND with another bit set, returning a new one.
    pub fn xh_and(&self, other: &Self) -> Self {
        let len = self.xh_words.len().min(other.xh_words.len());
        let mut result = Self::xh_new(self.xh_nbits.min(other.xh_nbits));
        for i in 0..len {
            result.xh_words[i] = self.xh_words[i] & other.xh_words[i];
        }
        result
    }

    /// Bitwise OR with another bit set, returning a new one.
    pub fn xh_or(&self, other: &Self) -> Self {
        let len = self.xh_words.len().max(other.xh_words.len());
        let mut result = Self::xh_new(self.xh_nbits.max(other.xh_nbits));
        for i in 0..len {
            let a = if i < self.xh_words.len() { self.xh_words[i] } else { 0 };
            let b = if i < other.xh_words.len() { other.xh_words[i] } else { 0 };
            result.xh_words[i] = a | b;
        }
        result
    }

    /// Bitwise XOR with another bit set, returning a new one.
    pub fn xh_xor(&self, other: &Self) -> Self {
        let len = self.xh_words.len().max(other.xh_words.len());
        let mut result = Self::xh_new(self.xh_nbits.max(other.xh_nbits));
        for i in 0..len {
            let a = if i < self.xh_words.len() { self.xh_words[i] } else { 0 };
            let b = if i < other.xh_words.len() { other.xh_words[i] } else { 0 };
            result.xh_words[i] = a ^ b;
        }
        result
    }

    /// Iterate over the indices of all set bits.
    pub fn xh_iter_ones(&self) -> Vec<usize> {
        let mut result = Vec::new();
        for (wi, &word) in self.xh_words.iter().enumerate() {
            let mut w = word;
            while w != 0 {
                let bit = w.trailing_zeros() as usize;
                result.push(wi * 64 + bit);
                w &= w - 1;
            }
        }
        result
    }

    /// Index of the first set bit, if any.
    pub fn xh_first_set(&self) -> Option<usize> {
        for (wi, &word) in self.xh_words.iter().enumerate() {
            if word != 0 {
                return Some(wi * 64 + word.trailing_zeros() as usize);
            }
        }
        None
    }

    /// Index of the last set bit, if any.
    pub fn xh_last_set(&self) -> Option<usize> {
        for (wi, &word) in self.xh_words.iter().enumerate().rev() {
            if word != 0 {
                return Some(wi * 64 + (63 - word.leading_zeros() as usize));
            }
        }
        None
    }
}


/// A double-ended queue backed by a ring buffer (variant 49).
pub struct Xi49Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi49Deque<T> {
    /// Create a new deque with the given capacity.
    pub fn xi_new(capacity: usize) -> Self {
        let cap = capacity.max(4);
        Self {
            xi_buf: (0..cap).map(|_| None).collect(),
            xi_head: 0,
            xi_tail: 0,
            xi_len: 0,
        }
    }

    /// Return the number of elements.
    pub fn xi_len(&self) -> usize {
        self.xi_len
    }

    /// Return the capacity.
    pub fn xi_capacity(&self) -> usize {
        self.xi_buf.len()
    }

    /// Return true if empty.
    pub fn xi_is_empty(&self) -> bool {
        self.xi_len == 0
    }

    fn xi_grow(&mut self) {
        let old_cap = self.xi_buf.len();
        let new_cap = old_cap * 2;
        let mut new_buf: Vec<Option<T>> = (0..new_cap).map(|_| None).collect();
        for i in 0..self.xi_len {
            let idx = (self.xi_head + i) % old_cap;
            new_buf[i] = self.xi_buf[idx].take();
        }
        self.xi_buf = new_buf;
        self.xi_head = 0;
        self.xi_tail = self.xi_len;
    }

    /// Push an element to the back.
    pub fn xi_push_back(&mut self, val: T) {
        if self.xi_len == self.xi_buf.len() {
            self.xi_grow();
        }
        self.xi_buf[self.xi_tail] = Some(val);
        self.xi_tail = (self.xi_tail + 1) % self.xi_buf.len();
        self.xi_len += 1;
    }

    /// Push an element to the front.
    pub fn xi_push_front(&mut self, val: T) {
        if self.xi_len == self.xi_buf.len() {
            self.xi_grow();
        }
        self.xi_head = if self.xi_head == 0 {
            self.xi_buf.len() - 1
        } else {
            self.xi_head - 1
        };
        self.xi_buf[self.xi_head] = Some(val);
        self.xi_len += 1;
    }

    /// Pop an element from the back.
    pub fn xi_pop_back(&mut self) -> Option<T> {
        if self.xi_len == 0 {
            return None;
        }
        self.xi_tail = if self.xi_tail == 0 {
            self.xi_buf.len() - 1
        } else {
            self.xi_tail - 1
        };
        self.xi_len -= 1;
        self.xi_buf[self.xi_tail].take()
    }

    /// Pop an element from the front.
    pub fn xi_pop_front(&mut self) -> Option<T> {
        if self.xi_len == 0 {
            return None;
        }
        let val = self.xi_buf[self.xi_head].take();
        self.xi_head = (self.xi_head + 1) % self.xi_buf.len();
        self.xi_len -= 1;
        val
    }

    /// Get element at index.
    pub fn xi_get(&self, index: usize) -> Option<&T> {
        if index >= self.xi_len {
            return None;
        }
        let real = (self.xi_head + index) % self.xi_buf.len();
        self.xi_buf[real].as_ref()
    }

    /// Rotate elements left by k positions.
    pub fn xi_rotate_left(&mut self, k: usize) {
        if self.xi_len <= 1 {
            return;
        }
        let k = k % self.xi_len;
        for _ in 0..k {
            if let Some(v) = self.xi_pop_front() {
                self.xi_push_back(v);
            }
        }
    }

    /// Rotate elements right by k positions.
    pub fn xi_rotate_right(&mut self, k: usize) {
        if self.xi_len <= 1 {
            return;
        }
        let k = k % self.xi_len;
        for _ in 0..k {
            if let Some(v) = self.xi_pop_back() {
                self.xi_push_front(v);
            }
        }
    }

    /// Collect elements into a vector.
    pub fn xi_iter(&self) -> Vec<T> {
        let mut out = Vec::with_capacity(self.xi_len);
        for i in 0..self.xi_len {
            let idx = (self.xi_head + i) % self.xi_buf.len();
            if let Some(ref v) = self.xi_buf[idx] {
                out.push(v.clone());
            }
        }
        out
    }

    /// Split at index, returning (left, right) vectors.
    pub fn xi_split_at(&self, mid: usize) -> (Vec<T>, Vec<T>) {
        let all = self.xi_iter();
        let mid = mid.min(all.len());
        let left = all[..mid].to_vec();
        let right = all[mid..].to_vec();
        (left, right)
    }
}

/// An interval represented as [low, high).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xi49Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi49Interval {
    /// Create a new interval.
    pub fn xi_new(low: i64, high: i64) -> Self {
        Self { xi_low: low, xi_high: high }
    }

    /// Check whether this interval overlaps with another.
    pub fn xi_overlaps(&self, other: &Self) -> bool {
        self.xi_low < other.xi_high && other.xi_low < self.xi_high
    }

    /// Check whether this interval contains a point.
    pub fn xi_contains_point(&self, p: i64) -> bool {
        p >= self.xi_low && p < self.xi_high
    }
}

/// A simple interval tree (variant 49).
pub struct Xi49IntervalTree {
    xi_intervals: Vec<Xi49Interval>,
}

impl Xi49IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi49Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi49Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi49Interval) -> Vec<&Xi49Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_overlaps(query)).collect()
    }

    /// Remove the first interval matching [low, high).
    pub fn xi_remove(&mut self, low: i64, high: i64) -> bool {
        if let Some(pos) = self.xi_intervals.iter().position(|iv| iv.xi_low == low && iv.xi_high == high) {
            self.xi_intervals.remove(pos);
            true
        } else {
            false
        }
    }

    /// Return all intervals.
    pub fn xi_all_intervals(&self) -> &[Xi49Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi49Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi49Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi49Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi49Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi49Interval> = Vec::new();
        for iv in &self.xi_intervals {
            if let Some(last) = merged.last_mut() {
                if iv.xi_low <= last.xi_high {
                    last.xi_high = last.xi_high.max(iv.xi_high);
                } else {
                    merged.push(iv.clone());
                }
            } else {
                merged.push(iv.clone());
            }
        }
        merged
    }
}


// --- xj_ Union-Find and B-Tree (crate index 49) ---

/// Disjoint set / union-find for crate 49.
pub struct Xj49UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
    size: Vec<usize>,
    count: usize,
}

impl Xj49UnionFind {
    /// Create an empty union-find.
    pub fn xj_new() -> Self {
        Self { parent: Vec::new(), rank: Vec::new(), size: Vec::new(), count: 0 }
    }

    /// Add a new singleton set and return its id.
    pub fn xj_make_set(&mut self) -> usize {
        let id = self.parent.len();
        self.parent.push(id);
        self.rank.push(0);
        self.size.push(1);
        self.count += 1;
        id
    }

    /// Find representative with path compression.
    pub fn xj_find(&mut self, mut x: usize) -> usize {
        while self.parent[x] != x {
            self.parent[x] = self.parent[self.parent[x]];
            x = self.parent[x];
        }
        x
    }

    /// Union two sets by rank. Returns true if they were separate.
    pub fn xj_union(&mut self, a: usize, b: usize) -> bool {
        let ra = self.xj_find(a);
        let rb = self.xj_find(b);
        if ra == rb { return false; }
        let (small, big) = if self.rank[ra] < self.rank[rb] { (ra, rb) } else { (rb, ra) };
        self.parent[small] = big;
        self.size[big] += self.size[small];
        if self.rank[big] == self.rank[small] { self.rank[big] += 1; }
        self.count -= 1;
        true
    }

    /// Check whether a and b are in the same component.
    pub fn xj_connected(&mut self, a: usize, b: usize) -> bool {
        self.xj_find(a) == self.xj_find(b)
    }

    /// Number of disjoint components.
    pub fn xj_component_count(&self) -> usize {
        self.count
    }

    /// Size of the component containing x.
    pub fn xj_component_size(&mut self, x: usize) -> usize {
        let r = self.xj_find(x);
        self.size[r]
    }

    /// Size of the largest component (0 if empty).
    pub fn xj_largest_component(&self) -> usize {
        self.size.iter().enumerate()
            .filter(|(i, _)| self.parent[*i] == *i)
            .map(|(_, s)| *s)
            .max()
            .unwrap_or(0)
    }
}

const XJ49_BTREE_ORDER: usize = 4;

/// Simple B-tree map for crate 49.
pub struct Xj49BTree<K: Ord + Clone, V: Clone> {
    root: Option<Box<Xj49BTreeNode<K, V>>>,
    len: usize,
}

struct Xj49BTreeNode<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    children: Vec<Box<Xj49BTreeNode<K, V>>>,
}

impl<K: Ord + Clone, V: Clone> Xj49BTreeNode<K, V> {
    fn xj_new_leaf() -> Self {
        Self { keys: Vec::new(), values: Vec::new(), children: Vec::new() }
    }

    fn xj_is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    fn xj_is_full(&self) -> bool {
        self.keys.len() >= 2 * XJ49_BTREE_ORDER - 1
    }

    fn xj_search(&self, key: &K) -> Option<&V> {
        let mut idx = 0;
        while idx < self.keys.len() && *key > self.keys[idx] { idx += 1; }
        if idx < self.keys.len() && self.keys[idx] == *key {
            return Some(&self.values[idx]);
        }
        if self.xj_is_leaf() { return None; }
        self.children[idx].xj_search(key)
    }

    fn xj_split_child(&mut self, i: usize) {
        let mid = XJ49_BTREE_ORDER - 1;
        let mut child = &mut self.children[i];
        let mut new_node = Xj49BTreeNode::xj_new_leaf();
        new_node.keys = child.keys.split_off(mid + 1);
        new_node.values = child.values.split_off(mid + 1);
        if !child.xj_is_leaf() {
            new_node.children = child.children.split_off(mid + 1);
        }
        let up_key = child.keys.pop().unwrap();
        let up_val = child.values.pop().unwrap();
        self.keys.insert(i, up_key);
        self.values.insert(i, up_val);
        self.children.insert(i + 1, Box::new(new_node));
    }

    fn xj_insert_non_full(&mut self, key: K, value: V) -> Option<V> {
        let mut idx = self.keys.len();
        while idx > 0 && key < self.keys[idx - 1] { idx -= 1; }
        if idx < self.keys.len() && self.keys[idx] == key {
            let old = std::mem::replace(&mut self.values[idx], value);
            return Some(old);
        }
        if self.xj_is_leaf() {
            self.keys.insert(idx, key);
            self.values.insert(idx, value);
            return None;
        }
        if self.children[idx].xj_is_full() {
            self.xj_split_child(idx);
            if key > self.keys[idx] { idx += 1; }
            else if key == self.keys[idx] {
                let old = std::mem::replace(&mut self.values[idx], value);
                return Some(old);
            }
        }
        self.children[idx].xj_insert_non_full(key, value)
    }

    fn xj_collect_keys(&self, out: &mut Vec<K>) {
        for i in 0..self.keys.len() {
            if !self.xj_is_leaf() { self.children[i].xj_collect_keys(out); }
            out.push(self.keys[i].clone());
        }
        if !self.xj_is_leaf() { self.children[self.keys.len()].xj_collect_keys(out); }
    }

    fn xj_collect_values(&self, out: &mut Vec<V>) {
        for i in 0..self.keys.len() {
            if !self.xj_is_leaf() { self.children[i].xj_collect_values(out); }
            out.push(self.values[i].clone());
        }
        if !self.xj_is_leaf() { self.children[self.keys.len()].xj_collect_values(out); }
    }

    fn xj_collect_range(&self, lo: &K, hi: &K, out: &mut Vec<(K, V)>) {
        let mut i = 0;
        while i < self.keys.len() {
            if !self.xj_is_leaf() && self.keys[i] >= *lo {
                self.children[i].xj_collect_range(lo, hi, out);
            }
            if self.keys[i] >= *lo && self.keys[i] <= *hi {
                out.push((self.keys[i].clone(), self.values[i].clone()));
            }
            i += 1;
        }
        if !self.xj_is_leaf() && (i == 0 || self.keys[i - 1] <= *hi) {
            self.children[i].xj_collect_range(lo, hi, out);
        }
    }

    fn xj_min_key(&self) -> Option<&K> {
        if self.xj_is_leaf() { self.keys.first() }
        else { self.children[0].xj_min_key().or(self.keys.first()) }
    }

    fn xj_max_key(&self) -> Option<&K> {
        if self.xj_is_leaf() { self.keys.last() }
        else { self.children.last().unwrap().xj_max_key().or(self.keys.last()) }
    }

    fn xj_remove(&mut self, key: &K) -> Option<V> {
        let mut idx = 0;
        while idx < self.keys.len() && *key > self.keys[idx] { idx += 1; }
        if idx < self.keys.len() && self.keys[idx] == *key {
            if self.xj_is_leaf() {
                self.keys.remove(idx);
                return Some(self.values.remove(idx));
            }
            let pred_val = self.children[idx].xj_remove_max();
            let old_val = std::mem::replace(&mut self.values[idx], pred_val.1);
            self.keys[idx] = pred_val.0;
            return Some(old_val);
        }
        if self.xj_is_leaf() { return None; }
        self.children.get_mut(idx).and_then(|c| c.xj_remove(key))
    }

    fn xj_remove_max(&mut self) -> (K, V) {
        if self.xj_is_leaf() {
            let k = self.keys.pop().unwrap();
            let v = self.values.pop().unwrap();
            (k, v)
        } else {
            self.children.last_mut().unwrap().xj_remove_max()
        }
    }
}

impl<K: Ord + Clone, V: Clone> Xj49BTree<K, V> {
    /// Create an empty B-tree map.
    pub fn xj_new() -> Self {
        Self { root: None, len: 0 }
    }

    /// Insert a key-value pair. Returns previous value if key existed.
    pub fn xj_insert(&mut self, key: K, value: V) -> Option<V> {
        if self.root.is_none() {
            let mut node = Xj49BTreeNode::xj_new_leaf();
            node.keys.push(key);
            node.values.push(value);
            self.root = Some(Box::new(node));
            self.len = 1;
            return None;
        }
        let root = self.root.as_mut().unwrap();
        if root.xj_is_full() {
            let mut new_root = Xj49BTreeNode::xj_new_leaf();
            new_root.children.push(self.root.take().unwrap());
            new_root.xj_split_child(0);
            let old = new_root.xj_insert_non_full(key, value);
            self.root = Some(Box::new(new_root));
            if old.is_none() { self.len += 1; }
            old
        } else {
            let old = root.xj_insert_non_full(key, value);
            if old.is_none() { self.len += 1; }
            old
        }
    }

    /// Get a reference to the value for the given key.
    pub fn xj_get(&self, key: &K) -> Option<&V> {
        self.root.as_ref().and_then(|r| r.xj_search(key))
    }

    /// Remove a key and return its value.
    pub fn xj_remove(&mut self, key: &K) -> Option<V> {
        let result = self.root.as_mut().and_then(|r| r.xj_remove(key));
        if result.is_some() { self.len -= 1; }
        result
    }

    /// Check if a key is present.
    pub fn xj_contains_key(&self, key: &K) -> bool {
        self.xj_get(key).is_some()
    }

    /// Number of entries.
    pub fn xj_len(&self) -> usize {
        self.len
    }

    /// Collect all keys in sorted order.
    pub fn xj_keys(&self) -> Vec<K> {
        let mut out = Vec::new();
        if let Some(r) = &self.root { r.xj_collect_keys(&mut out); }
        out
    }

    /// Collect all values in key-sorted order.
    pub fn xj_values(&self) -> Vec<V> {
        let mut out = Vec::new();
        if let Some(r) = &self.root { r.xj_collect_values(&mut out); }
        out
    }

    /// Collect entries in [lo, hi] range.
    pub fn xj_range(&self, lo: &K, hi: &K) -> Vec<(K, V)> {
        let mut out = Vec::new();
        if let Some(r) = &self.root { r.xj_collect_range(lo, hi, &mut out); }
        out
    }

    /// Smallest key, if any.
    pub fn xj_min_key(&self) -> Option<&K> {
        self.root.as_ref().and_then(|r| r.xj_min_key())
    }

    /// Largest key, if any.
    pub fn xj_max_key(&self) -> Option<&K> {
        self.root.as_ref().and_then(|r| r.xj_max_key())
    }
}


// --- xk_49 segment tree and disjoint intervals ---

/// Segment tree for range queries over `i64` values.
pub struct Xk49SegmentTree {
    xk_n: usize,
    xk_tree: Vec<i64>,
    xk_min_tree: Vec<i64>,
    xk_max_tree: Vec<i64>,
}

impl Xk49SegmentTree {
    /// Build a segment tree from the given slice.
    pub fn xk_build(data: &[i64]) -> Self {
        let n = data.len();
        let tree = vec![0i64; 4 * n.max(1)];
        let min_tree = vec![i64::MAX; 4 * n.max(1)];
        let max_tree = vec![i64::MIN; 4 * n.max(1)];
        let mut st = Self { xk_n: n, xk_tree: tree, xk_min_tree: min_tree, xk_max_tree: max_tree };
        if n > 0 {
            st.xk_build_rec(data, 1, 0, n - 1);
        }
        st
    }

    fn xk_build_rec(&mut self, data: &[i64], node: usize, start: usize, end: usize) {
        if start == end {
            self.xk_tree[node] = data[start];
            self.xk_min_tree[node] = data[start];
            self.xk_max_tree[node] = data[start];
        } else {
            let mid = (start + end) / 2;
            self.xk_build_rec(data, 2 * node, start, mid);
            self.xk_build_rec(data, 2 * node + 1, mid + 1, end);
            self.xk_tree[node] = self.xk_tree[2 * node] + self.xk_tree[2 * node + 1];
            self.xk_min_tree[node] = self.xk_min_tree[2 * node].min(self.xk_min_tree[2 * node + 1]);
            self.xk_max_tree[node] = self.xk_max_tree[2 * node].max(self.xk_max_tree[2 * node + 1]);
        }
    }

    /// Query the sum of elements in the range `[l, r]` (inclusive).
    pub fn xk_query(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return 0; }
        self.xk_query_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_query_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return 0; }
        if l <= start && end <= r { return self.xk_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_query_rec(2 * node, start, mid, l, r)
            + self.xk_query_rec(2 * node + 1, mid + 1, end, l, r)
    }

    /// Update the value at index `idx` to `val`.
    pub fn xk_update(&mut self, idx: usize, val: i64) {
        if idx >= self.xk_n { return; }
        self.xk_update_rec(1, 0, self.xk_n - 1, idx, val);
    }

    fn xk_update_rec(&mut self, node: usize, start: usize, end: usize, idx: usize, val: i64) {
        if start == end {
            self.xk_tree[node] = val;
            self.xk_min_tree[node] = val;
            self.xk_max_tree[node] = val;
        } else {
            let mid = (start + end) / 2;
            if idx <= mid {
                self.xk_update_rec(2 * node, start, mid, idx, val);
            } else {
                self.xk_update_rec(2 * node + 1, mid + 1, end, idx, val);
            }
            self.xk_tree[node] = self.xk_tree[2 * node] + self.xk_tree[2 * node + 1];
            self.xk_min_tree[node] = self.xk_min_tree[2 * node].min(self.xk_min_tree[2 * node + 1]);
            self.xk_max_tree[node] = self.xk_max_tree[2 * node].max(self.xk_max_tree[2 * node + 1]);
        }
    }

    /// Return the minimum value in the range `[l, r]` (inclusive).
    pub fn xk_range_min(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return i64::MAX; }
        self.xk_min_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_min_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return i64::MAX; }
        if l <= start && end <= r { return self.xk_min_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_min_rec(2 * node, start, mid, l, r)
            .min(self.xk_min_rec(2 * node + 1, mid + 1, end, l, r))
    }

    /// Return the maximum value in the range `[l, r]` (inclusive).
    pub fn xk_range_max(&self, l: usize, r: usize) -> i64 {
        if l > r || r >= self.xk_n { return i64::MIN; }
        self.xk_max_rec(1, 0, self.xk_n - 1, l, r)
    }

    fn xk_max_rec(&self, node: usize, start: usize, end: usize, l: usize, r: usize) -> i64 {
        if r < start || end < l { return i64::MIN; }
        if l <= start && end <= r { return self.xk_max_tree[node]; }
        let mid = (start + end) / 2;
        self.xk_max_rec(2 * node, start, mid, l, r)
            .max(self.xk_max_rec(2 * node + 1, mid + 1, end, l, r))
    }

    /// Return the number of elements.
    pub fn xk_len(&self) -> usize {
        self.xk_n
    }
}

/// A set of non-overlapping intervals over `i64`.
pub struct Xk49DisjointIntervals {
    xk_intervals: Vec<(i64, i64)>,
}

impl Xk49DisjointIntervals {
    /// Create an empty interval set.
    pub fn xk_new() -> Self {
        Self { xk_intervals: Vec::new() }
    }

    /// Add interval `[lo, hi]` and merge any overlaps.
    pub fn xk_add_interval(&mut self, lo: i64, hi: i64) {
        if lo > hi { return; }
        let mut new_lo = lo;
        let mut new_hi = hi;
        let mut merged = Vec::new();
        for &(a, b) in &self.xk_intervals {
            if b < new_lo - 1 || a > new_hi + 1 {
                merged.push((a, b));
            } else {
                new_lo = new_lo.min(a);
                new_hi = new_hi.max(b);
            }
        }
        merged.push((new_lo, new_hi));
        merged.sort();
        self.xk_intervals = merged;
    }

    /// Remove interval `[lo, hi]` from the set.
    pub fn xk_remove_interval(&mut self, lo: i64, hi: i64) {
        if lo > hi { return; }
        let mut result = Vec::new();
        for &(a, b) in &self.xk_intervals {
            if b < lo || a > hi {
                result.push((a, b));
            } else {
                if a < lo { result.push((a, lo - 1)); }
                if b > hi { result.push((hi + 1, b)); }
            }
        }
        self.xk_intervals = result;
    }

    /// Check if a point is contained in any interval.
    pub fn xk_contains_point(&self, p: i64) -> bool {
        self.xk_intervals.iter().any(|&(a, b)| a <= p && p <= b)
    }

    /// Return the total length covered by all intervals.
    pub fn xk_covered_length(&self) -> i64 {
        self.xk_intervals.iter().map(|&(a, b)| b - a + 1).sum()
    }

    /// Return the gaps between intervals as a vec of `(start, end)`.
    pub fn xk_gaps(&self) -> Vec<(i64, i64)> {
        let mut gaps = Vec::new();
        for w in self.xk_intervals.windows(2) {
            gaps.push((w[0].1 + 1, w[1].0 - 1));
        }
        gaps
    }

    /// Merge adjacent intervals that are exactly contiguous.
    pub fn xk_merge_adjacent(&mut self) {
        if self.xk_intervals.len() < 2 { return; }
        let mut merged = vec![self.xk_intervals[0]];
        for &(a, b) in &self.xk_intervals[1..] {
            let last = merged.last_mut().unwrap();
            if a <= last.1 + 1 {
                last.1 = last.1.max(b);
            } else {
                merged.push((a, b));
            }
        }
        self.xk_intervals = merged;
    }

    /// Return the number of disjoint intervals.
    pub fn xk_interval_count(&self) -> usize {
        self.xk_intervals.len()
    }
}


/// Rope data structure for efficient large text manipulation (xl_49).
#[derive(Debug, Clone)]
pub struct Xl49Rope {
    xl_chunks: Vec<String>,
    xl_total_len: usize,
}

impl Xl49Rope {
    /// Create a new empty rope.
    pub fn xl_new() -> Self {
        Self {
            xl_chunks: Vec::new(),
            xl_total_len: 0,
        }
    }

    /// Create a rope from a string.
    pub fn xl_from_str(s: &str) -> Self {
        let mut rope = Self::xl_new();
        if !s.is_empty() {
            let chunk_size = 64;
            let mut start = 0;
            while start < s.len() {
                let end = (start + chunk_size).min(s.len());
                let boundary = if end < s.len() {
                    let mut b = end;
                    while b > start && !s.is_char_boundary(b) {
                        b -= 1;
                    }
                    if b == start { end } else { b }
                } else {
                    end
                };
                rope.xl_chunks.push(s[start..boundary].to_string());
                rope.xl_total_len += boundary - start;
                start = boundary;
            }
        }
        rope
    }

    /// Insert text at a character offset.
    pub fn xl_insert_at(&mut self, pos: usize, text: &str) {
        if text.is_empty() {
            return;
        }
        let flat = self.xl_to_string();
        let byte_pos = flat.char_indices()
            .nth(pos)
            .map(|(i, _)| i)
            .unwrap_or(flat.len());
        let mut new_str = String::with_capacity(flat.len() + text.len());
        new_str.push_str(&flat[..byte_pos]);
        new_str.push_str(text);
        new_str.push_str(&flat[byte_pos..]);
        *self = Self::xl_from_str(&new_str);
    }

    /// Delete a range of characters [start, end).
    pub fn xl_delete_range(&mut self, start: usize, end: usize) {
        if start >= end {
            return;
        }
        let flat = self.xl_to_string();
        let indices: Vec<usize> = flat.char_indices().map(|(i, _)| i).collect();
        let byte_start = if start < indices.len() { indices[start] } else { flat.len() };
        let byte_end = if end < indices.len() { indices[end] } else { flat.len() };
        let mut new_str = String::with_capacity(flat.len() - (byte_end - byte_start));
        new_str.push_str(&flat[..byte_start]);
        new_str.push_str(&flat[byte_end..]);
        *self = Self::xl_from_str(&new_str);
    }

    /// Get the character at a given index.
    pub fn xl_char_at(&self, index: usize) -> Option<char> {
        self.xl_to_string().chars().nth(index)
    }

    /// Total length in bytes.
    pub fn xl_len(&self) -> usize {
        self.xl_total_len
    }

    /// Check if empty.
    pub fn xl_is_empty(&self) -> bool {
        self.xl_total_len == 0
    }

    /// Extract a substring by byte range.
    pub fn xl_slice(&self, start: usize, end: usize) -> String {
        let flat = self.xl_to_string();
        let clamped_end = end.min(flat.len());
        let clamped_start = start.min(clamped_end);
        flat[clamped_start..clamped_end].to_string()
    }

    /// Split the rope at a byte position into two ropes.
    pub fn xl_split(self, at: usize) -> (Self, Self) {
        let flat = self.xl_to_string();
        let split_at = at.min(flat.len());
        (Self::xl_from_str(&flat[..split_at]), Self::xl_from_str(&flat[split_at..]))
    }

    /// Concatenate another rope onto this one.
    pub fn xl_concat(&mut self, other: &Self) {
        for chunk in &other.xl_chunks {
            self.xl_total_len += chunk.len();
            self.xl_chunks.push(chunk.clone());
        }
    }

    /// Count lines (number of '\n' characters + 1).
    pub fn xl_line_count(&self) -> usize {
        let flat = self.xl_to_string();
        if flat.is_empty() {
            return 0;
        }
        flat.chars().filter(|&c| c == '\n').count() + 1
    }

    /// Get a specific line by zero-based index.
    pub fn xl_line_at(&self, index: usize) -> Option<String> {
        let flat = self.xl_to_string();
        flat.split('\n').nth(index).map(|s| s.to_string())
    }

    /// Flatten to a single String.
    pub fn xl_to_string(&self) -> String {
        let mut out = String::with_capacity(self.xl_total_len);
        for chunk in &self.xl_chunks {
            out.push_str(chunk);
        }
        out
    }

    /// Number of chunks in internal storage.
    pub fn xl_chunk_count(&self) -> usize {
        self.xl_chunks.len()
    }
}

/// Suffix array for efficient string searching (xl_49).
#[derive(Debug, Clone)]
pub struct Xl49SuffixArray {
    xl_text: String,
    xl_sa: Vec<usize>,
}

impl Xl49SuffixArray {
    /// Build a suffix array from the given text.
    pub fn xl_build(text: &str) -> Self {
        let n = text.len();
        let mut sa: Vec<usize> = (0..n).collect();
        let bytes = text.as_bytes();
        sa.sort_by(|&a, &b| bytes[a..].cmp(&bytes[b..]));
        Self {
            xl_text: text.to_string(),
            xl_sa: sa,
        }
    }

    /// Search for a pattern; returns the first matching position or None.
    pub fn xl_search(&self, pattern: &str) -> Option<usize> {
        let pat = pattern.as_bytes();
        let text = self.xl_text.as_bytes();
        let mut lo: usize = 0;
        let mut hi: usize = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let suffix_start = self.xl_sa[mid];
            let suffix_end = (suffix_start + pat.len()).min(text.len());
            if &text[suffix_start..suffix_end] < pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        if lo < self.xl_sa.len() {
            let suffix_start = self.xl_sa[lo];
            let suffix_end = (suffix_start + pat.len()).min(text.len());
            if &text[suffix_start..suffix_end] == pat {
                return Some(self.xl_sa[lo]);
            }
        }
        None
    }

    /// Count occurrences of a pattern.
    pub fn xl_count_occurrences(&self, pattern: &str) -> usize {
        self.xl_all_positions(pattern).len()
    }

    /// Find the longest repeated substring.
    pub fn xl_longest_repeated(&self) -> String {
        if self.xl_sa.len() < 2 {
            return String::new();
        }
        let text = self.xl_text.as_bytes();
        let mut best_len = 0;
        let mut best_start = 0;
        for i in 1..self.xl_sa.len() {
            let a = self.xl_sa[i - 1];
            let b = self.xl_sa[i];
            let mut common = 0;
            while a + common < text.len() && b + common < text.len() && text[a + common] == text[b + common] {
                common += 1;
            }
            if common > best_len {
                best_len = common;
                best_start = a;
            }
        }
        self.xl_text[best_start..best_start + best_len].to_string()
    }

    /// Return all positions where the pattern occurs.
    pub fn xl_all_positions(&self, pattern: &str) -> Vec<usize> {
        let pat = pattern.as_bytes();
        let text = self.xl_text.as_bytes();
        let mut results = Vec::new();
        if pat.is_empty() || text.is_empty() {
            return results;
        }
        // Find lower bound
        let mut lo: usize = 0;
        let mut hi: usize = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let s = self.xl_sa[mid];
            let e = (s + pat.len()).min(text.len());
            if &text[s..e] < pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        let start = lo;
        // Find upper bound
        hi = self.xl_sa.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let s = self.xl_sa[mid];
            let e = (s + pat.len()).min(text.len());
            if &text[s..e] <= pat {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        for idx in start..lo {
            results.push(self.xl_sa[idx]);
        }
        results.sort();
        results
    }

    /// Length of the underlying text.
    pub fn xl_len(&self) -> usize {
        self.xl_text.len()
    }

    /// Whether the text is empty.
    pub fn xl_is_empty(&self) -> bool {
        self.xl_text.is_empty()
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

    // ── AuthSessionBuilder tests ──

    #[test]
    fn session_builder_builds_valid_session() {
        let session = AuthSessionBuilder::new()
            .id("s1")
            .access_token("tok123")
            .account("acc1", "Alice")
            .scope("repo")
            .scope("user")
            .build()
            .unwrap();
        assert_eq!(session.id, "s1");
        assert_eq!(session.access_token, "tok123");
        assert_eq!(session.account.id, "acc1");
        assert_eq!(session.account.label, "Alice");
        assert_eq!(session.scopes, vec!["repo", "user"]);
    }

    #[test]
    fn session_builder_missing_id_fails() {
        let result = AuthSessionBuilder::new()
            .access_token("tok")
            .account("a", "A")
            .build();
        assert!(result.is_err());
    }

    #[test]
    fn session_builder_missing_token_fails() {
        let result = AuthSessionBuilder::new()
            .id("s1")
            .account("a", "A")
            .build();
        assert!(result.is_err());
    }

    #[test]
    fn session_builder_missing_account_fails() {
        let result = AuthSessionBuilder::new()
            .id("s1")
            .access_token("tok")
            .build();
        assert!(result.is_err());
    }

    #[test]
    fn session_builder_scopes_batch() {
        let session = AuthSessionBuilder::new()
            .id("s1")
            .access_token("tok")
            .account("a", "A")
            .scopes(&["read", "write", "admin"])
            .build()
            .unwrap();
        assert_eq!(session.scopes.len(), 3);
        assert!(session.scopes.contains(&"admin".to_string()));
    }

    // ── ScopeDiff tests ──

    #[test]
    fn diff_scopes_detects_added_removed() {
        let old: Vec<String> = vec!["read".into(), "write".into()];
        let new: Vec<String> = vec!["write".into(), "admin".into()];
        let diff = diff_scopes(&old, &new);
        assert_eq!(diff.added, vec!["admin"]);
        assert_eq!(diff.removed, vec!["read"]);
        assert_eq!(diff.unchanged, vec!["write"]);
        assert!(!diff.is_unchanged());
    }

    #[test]
    fn diff_scopes_identical_is_unchanged() {
        let scopes: Vec<String> = vec!["a".into(), "b".into()];
        let diff = diff_scopes(&scopes, &scopes);
        assert!(diff.is_unchanged());
        assert!(diff.added.is_empty());
        assert!(diff.removed.is_empty());
    }

    #[test]
    fn diff_scopes_empty_to_full() {
        let diff = diff_scopes(&[], &["x".into(), "y".into()]);
        assert_eq!(diff.added, vec!["x", "y"]);
        assert!(diff.removed.is_empty());
    }

    // ── SessionStore: find_by_account, replace_session tests ──

    #[test]
    fn session_store_find_by_account() {
        let mut store = SessionStore::new();
        store.add_session("gh", AuthSession {
            id: "s1".into(), access_token: "t".into(),
            account: AuthAccount { id: "acc1".into(), label: "Alice".into() },
            scopes: vec!["repo".into()],
        });
        store.add_session("gl", AuthSession {
            id: "s2".into(), access_token: "t".into(),
            account: AuthAccount { id: "acc1".into(), label: "Alice".into() },
            scopes: vec!["read".into()],
        });
        store.add_session("gh", AuthSession {
            id: "s3".into(), access_token: "t".into(),
            account: AuthAccount { id: "acc2".into(), label: "Bob".into() },
            scopes: vec![],
        });
        let results = store.find_by_account("acc1");
        assert_eq!(results.len(), 2);
        assert!(store.find_by_account("unknown").is_empty());
    }

    #[test]
    fn session_store_replace_session() {
        let mut store = SessionStore::new();
        let original = AuthSession {
            id: "s1".into(), access_token: "old-tok".into(),
            account: AuthAccount { id: "a".into(), label: "A".into() },
            scopes: vec!["read".into()],
        };
        store.add_session("gh", original.clone());
        let replacement = AuthSession {
            id: "s1".into(), access_token: "new-tok".into(),
            account: AuthAccount { id: "a".into(), label: "A".into() },
            scopes: vec!["read".into(), "write".into()],
        };
        let old = store.replace_session("gh", "s1", replacement).unwrap();
        assert_eq!(old.access_token, "old-tok");
        let sessions = store.get_sessions("gh", &[]);
        assert_eq!(sessions[0].access_token, "new-tok");
        assert_eq!(sessions[0].scopes.len(), 2);
    }

    #[test]
    fn session_store_replace_missing_returns_none() {
        let mut store = SessionStore::new();
        let session = AuthSession {
            id: "s1".into(), access_token: "t".into(),
            account: AuthAccount { id: "a".into(), label: "A".into() },
            scopes: vec![],
        };
        assert!(store.replace_session("gh", "s1", session).is_none());
    }

    // ── AuthBridge: handle_message_checked tests ──

    #[test]
    fn bridge_checked_rejects_unknown_provider() {
        let mut bridge = AuthBridge::new();
        let msg = AuthMessage::GetSessions {
            provider_id: "unknown".into(),
            scopes: vec![],
        };
        let result = bridge.handle_message_checked(&msg);
        assert_eq!(result, Err(AuthError::ProviderNotFound));
    }

    #[test]
    fn bridge_checked_register_then_get() {
        let mut bridge = AuthBridge::new();
        let reg = AuthMessage::RegisterProvider {
            provider_id: "gh".into(),
            label: "GitHub".into(),
        };
        assert!(bridge.handle_message_checked(&reg).is_ok());
        let get = AuthMessage::GetSessions {
            provider_id: "gh".into(),
            scopes: vec!["repo".into()],
        };
        let result = bridge.handle_message_checked(&get).unwrap();
        assert_eq!(result["provider"], "gh");
    }

    #[test]
    fn bridge_checked_unregister_unknown_fails() {
        let mut bridge = AuthBridge::new();
        let msg = AuthMessage::UnregisterProvider {
            provider_id: "nope".into(),
        };
        assert_eq!(
            bridge.handle_message_checked(&msg),
            Err(AuthError::ProviderNotFound)
        );
    }

    // ── AuthSession scope helper tests ──

    #[test]
    fn session_has_scope() {
        let session = AuthSession {
            id: "s1".into(), access_token: "t".into(),
            account: AuthAccount { id: "a".into(), label: "A".into() },
            scopes: vec!["read".into(), "write".into()],
        };
        assert!(session.has_scope("read"));
        assert!(session.has_scope("write"));
        assert!(!session.has_scope("admin"));
    }

    #[test]
    fn session_with_token_creates_copy() {
        let session = AuthSession {
            id: "s1".into(), access_token: "old".into(),
            account: AuthAccount { id: "a".into(), label: "A".into() },
            scopes: vec!["read".into()],
        };
        let refreshed = session.with_token("new-tok");
        assert_eq!(refreshed.access_token, "new-tok");
        assert_eq!(refreshed.id, "s1");
        assert_eq!(refreshed.scopes, vec!["read"]);
        // original unchanged
        assert_eq!(session.access_token, "old");
    }

    // ── AuthTokenRefreshStore tests ──

    #[test]
    fn token_refresh_store_lifecycle() {
        let mut store = AuthTokenRefreshStore::new();
        assert_eq!(store.token_count(), 0);
        store.store_token("github", "ghp_abc", 5000);
        assert_eq!(store.token_count(), 1);
        assert_eq!(store.get_token("github"), Some("ghp_abc"));
        assert!(!store.is_expired("github", 4999));
        assert!(store.is_expired("github", 5000));
        assert!(store.is_expired("github", 6000));
        store.remove_token("github");
        assert_eq!(store.token_count(), 0);
        assert!(store.get_token("github").is_none());
    }

    #[test]
    fn token_refresh_store_needs_refresh() {
        let mut store = AuthTokenRefreshStore::new();
        store.store_token("gh", "tok", 10000);
        assert!(!store.needs_refresh("gh", 8000, 1000));
        assert!(store.needs_refresh("gh", 9000, 1000));
        assert!(store.needs_refresh("gh", 9500, 1000));
        // unknown provider does not need refresh
        assert!(!store.needs_refresh("unknown", 9000, 1000));
    }

    #[test]
    fn token_refresh_store_multiple_providers() {
        let mut store = AuthTokenRefreshStore::new();
        store.store_token("gh", "tok1", 5000);
        store.store_token("gl", "tok2", 8000);
        assert_eq!(store.token_count(), 2);
        assert_eq!(store.get_token("gh"), Some("tok1"));
        assert_eq!(store.get_token("gl"), Some("tok2"));
        assert!(store.is_expired("gh", 5000));
        assert!(!store.is_expired("gl", 5000));
    }

    #[test]
    fn token_refresh_store_overwrite() {
        let mut store = AuthTokenRefreshStore::new();
        store.store_token("gh", "old", 1000);
        store.store_token("gh", "new", 2000);
        assert_eq!(store.token_count(), 1);
        assert_eq!(store.get_token("gh"), Some("new"));
        assert!(!store.is_expired("gh", 1500));
    }

    // ── AuthScopeValidator tests ──

    #[test]
    fn scope_validator_all_granted() {
        let mut v = AuthScopeValidator::new();
        v.register_available_scope("read");
        v.register_available_scope("write");
        let result = v.validate_scopes(&["read", "write"]);
        assert!(result.valid);
        assert_eq!(result.granted, vec!["read", "write"]);
        assert!(result.denied.is_empty());
    }

    #[test]
    fn scope_validator_partial_denied() {
        let mut v = AuthScopeValidator::new();
        v.register_available_scope("read");
        let result = v.validate_scopes(&["read", "admin"]);
        assert!(!result.valid);
        assert_eq!(result.granted, vec!["read"]);
        assert_eq!(result.denied, vec!["admin"]);
    }

    #[test]
    fn scope_validator_available_and_dedup() {
        let mut v = AuthScopeValidator::new();
        v.register_available_scope("read");
        v.register_available_scope("read"); // duplicate ignored
        assert!(v.is_scope_available("read"));
        assert!(!v.is_scope_available("write"));
        assert_eq!(v.available_scopes(), vec!["read"]);
    }

    // ── AuthSessionDeduplicator tests ──

    #[test]
    fn deduplicator_prevents_duplicates() {
        let mut d = AuthSessionDeduplicator::new();
        assert!(d.try_register("gh", "alice"));
        assert!(!d.try_register("gh", "alice"));
        assert_eq!(d.session_count(), 1);
        assert!(d.is_registered("gh", "alice"));
    }

    #[test]
    fn deduplicator_different_accounts() {
        let mut d = AuthSessionDeduplicator::new();
        assert!(d.try_register("gh", "alice"));
        assert!(d.try_register("gh", "bob"));
        assert!(d.try_register("gl", "alice"));
        assert_eq!(d.session_count(), 3);
        let sessions = d.sessions();
        assert_eq!(sessions.len(), 3);
    }

    #[test]
    fn deduplicator_unregister() {
        let mut d = AuthSessionDeduplicator::new();
        d.try_register("gh", "alice");
        d.try_register("gh", "bob");
        d.unregister("gh", "alice");
        assert!(!d.is_registered("gh", "alice"));
        assert!(d.is_registered("gh", "bob"));
        assert_eq!(d.session_count(), 1);
        // re-register succeeds after unregister
        assert!(d.try_register("gh", "alice"));
    }

    // ── AuthProviderCapabilities tests ──

    #[test]
    fn provider_capabilities_defaults() {
        let cap = AuthProviderCapabilities::new("github");
        assert_eq!(cap.provider_id(), "github");
        assert!(!cap.supports_multi_account());
        assert!(!cap.supports_logout());
        assert!(!cap.supports_token_refresh());
    }

    #[test]
    fn provider_capabilities_setters() {
        let mut cap = AuthProviderCapabilities::new("gh");
        cap.set_supports_multi_account(true);
        cap.set_supports_logout(true);
        cap.set_supports_token_refresh(true);
        assert!(cap.supports_multi_account());
        assert!(cap.supports_logout());
        assert!(cap.supports_token_refresh());
    }

    #[test]
    fn provider_capabilities_display_none() {
        let cap = AuthProviderCapabilities::new("gh");
        assert_eq!(format!("{cap}"), "gh: (none)");
    }

    #[test]
    fn provider_capabilities_display_all() {
        let mut cap = AuthProviderCapabilities::new("gh");
        cap.set_supports_multi_account(true);
        cap.set_supports_logout(true);
        cap.set_supports_token_refresh(true);
        let s = format!("{cap}");
        assert!(s.contains("multi-account"));
        assert!(s.contains("logout"));
        assert!(s.contains("token-refresh"));
    }

    #[test]
    fn scope_validation_result_roundtrip() {
        let result = ScopeValidationResult {
            valid: false,
            granted: vec!["read".into()],
            denied: vec!["admin".into()],
        };
        let json = serde_json::to_string(&result).unwrap();
        let back: ScopeValidationResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result, back);
    }

    #[test]
    fn token_refresh_and_session_with_token() {
        let session = AuthSession {
            id: "s1".into(), access_token: "old".into(),
            account: AuthAccount { id: "a".into(), label: "A".into() },
            scopes: vec!["read".into()],
        };
        let refreshed = session.with_token("new-tok");
        assert_eq!(refreshed.access_token, "new-tok");
        assert_eq!(refreshed.id, "s1");
        assert_eq!(refreshed.scopes, vec!["read"]);
        // original unchanged
        assert_eq!(session.access_token, "old");
    }

    #[test]
    fn authbuf_ringbuf_push_get() {
        let mut rb = AuthBufRingBuffer::new(3);
        rb.push(10); rb.push(20); rb.push(30);
        assert_eq!(rb.get(0), Some(&10));
        assert_eq!(rb.get(2), Some(&30));
        assert_eq!(rb.len(), 3);
    }

    #[test]
    fn authbuf_ringbuf_overflow() {
        let mut rb = AuthBufRingBuffer::<i32>::new(2);
        rb.push(1); rb.push(2); rb.push(3);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(&2));
        assert_eq!(rb.get(1), Some(&3));
    }

    #[test]
    fn authbuf_ringbuf_clear() {
        let mut rb = AuthBufRingBuffer::new(5);
        rb.push("a".to_string()); rb.push("b".to_string());
        rb.clear();
        assert!(rb.is_empty());
    }

    #[test]
    fn authbuf_ringbuf_newest_oldest() {
        let mut rb = AuthBufRingBuffer::new(4);
        rb.push(100); rb.push(200); rb.push(300);
        assert_eq!(rb.oldest(), Some(&100));
        assert_eq!(rb.newest(), Some(&300));
    }

    #[test]
    fn authbuf_ringbuf_to_vec() {
        let mut rb = AuthBufRingBuffer::new(3);
        rb.push(1); rb.push(2);
        assert_eq!(rb.to_vec(), vec![1, 2]);
    }

    #[test]
    fn authbuf_ringbuf_is_full() {
        let mut rb = AuthBufRingBuffer::new(2);
        assert!(!rb.is_full());
        rb.push(1); rb.push(2);
        assert!(rb.is_full());
    }

    #[test]
    fn authc_lru_insert_get() {
        let mut c = AuthCLruCache::new(3);
        c.insert("a", 1); c.insert("b", 2); c.insert("c", 3);
        assert_eq!(c.get("a"), Some(&1));
        assert_eq!(c.get("b"), Some(&2));
        assert_eq!(c.len(), 3);
    }

    #[test]
    fn authc_lru_eviction() {
        let mut c = AuthCLruCache::new(2);
        c.insert("a", 1); c.insert("b", 2);
        let ev = c.insert("c", 3);
        assert!(ev.is_some());
        assert_eq!(ev.unwrap().0, "a");
        assert!(!c.contains("a"));
    }

    #[test]
    fn authc_lru_hit_ratio() {
        let mut c = AuthCLruCache::new(5);
        c.insert("x", 10);
        c.get("x"); c.get("y");
        assert!(c.hit_ratio() > 0.4 && c.hit_ratio() < 0.6);
    }

    #[test]
    fn authc_lru_clear() {
        let mut c = AuthCLruCache::new(3);
        c.insert("a", 1); c.insert("b", 2);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.hits(), 0);
    }

    #[test]
    fn authc_lru_remove() {
        let mut c = AuthCLruCache::new(3);
        c.insert("a", 100);
        assert_eq!(c.remove("a"), Some(100));
        assert!(!c.contains("a"));
    }

    #[test]
    fn authc_lru_peek() {
        let mut c = AuthCLruCache::new(3);
        c.insert("x", 42);
        assert_eq!(c.peek("x"), Some(&42));
        assert_eq!(c.misses(), 0);
    }


    #[test]
    fn ext_auth_config_new() {
        let cfg = ExtAuthConfig::new();
        assert!(cfg.is_enabled());
        assert_eq!(cfg.version(), 1);
        assert_eq!(cfg.option_count(), 0);
    }

    #[test]
    fn ext_auth_config_set_get() {
        let mut cfg = ExtAuthConfig::new();
        cfg.set_option("key", "value");
        assert_eq!(cfg.get_option("key"), Some("value"));
        assert!(cfg.has_option("key"));
    }

    #[test]
    fn ext_auth_config_remove() {
        let mut cfg = ExtAuthConfig::new();
        cfg.set_option("a", "1");
        assert_eq!(cfg.remove_option("a"), Some("1".into()));
        assert!(!cfg.has_option("a"));
    }

    #[test]
    fn ext_auth_config_keys_sorted() {
        let mut cfg = ExtAuthConfig::new();
        cfg.set_option("z", "1");
        cfg.set_option("a", "2");
        assert_eq!(cfg.option_keys(), vec!["a", "z"]);
    }

    #[test]
    fn ext_auth_config_bump_version() {
        let mut cfg = ExtAuthConfig::new();
        cfg.bump_version();
        cfg.bump_version();
        assert_eq!(cfg.version(), 3);
    }

    #[test]
    fn ext_auth_config_clear() {
        let mut cfg = ExtAuthConfig::new();
        cfg.set_option("x", "y");
        cfg.bump_version();
        cfg.clear();
        assert_eq!(cfg.option_count(), 0);
        assert_eq!(cfg.version(), 1);
    }

    #[test]
    fn ext_auth_config_merge() {
        let mut cfg1 = ExtAuthConfig::new();
        cfg1.set_option("a", "1");
        let mut cfg2 = ExtAuthConfig::new();
        cfg2.set_option("b", "2");
        cfg1.merge(&cfg2);
        assert_eq!(cfg1.option_count(), 2);
    }

    #[test]
    fn ext_auth_config_disable() {
        let mut cfg = ExtAuthConfig::new();
        cfg.set_enabled(false);
        assert!(!cfg.is_enabled());
    }

    #[test]
    fn ext_auth_rate_tracker_empty() {
        let rt = ExtAuthRateTracker::new(1000);
        assert_eq!(rt.count(), 0);
        assert_eq!(rt.rate_per_second(), 0.0);
    }

    #[test]
    fn ext_auth_rate_tracker_record() {
        let mut rt = ExtAuthRateTracker::new(1000);
        rt.record(100);
        rt.record(200);
        rt.record(300);
        assert_eq!(rt.count(), 3);
    }

    #[test]
    fn ext_auth_rate_tracker_prune() {
        let mut rt = ExtAuthRateTracker::new(100);
        rt.record(10);
        rt.record(200);
        assert_eq!(rt.count(), 1);
    }

    #[test]
    fn ext_auth_validator_valid() {
        let v = ExtAuthValidator::new();
        assert!(v.is_valid());
        assert_eq!(v.error_count(), 0);
    }

    #[test]
    fn ext_auth_validator_errors() {
        let mut v = ExtAuthValidator::new();
        v.add_error("bad input");
        v.add_warning("slow");
        assert!(!v.is_valid());
        assert_eq!(v.error_count(), 1);
        assert_eq!(v.warning_count(), 1);
        assert_eq!(v.first_error(), Some("bad input"));
    }

    #[test]
    fn ext_auth_validator_clear() {
        let mut v = ExtAuthValidator::new();
        v.add_error("err");
        v.clear();
        assert!(v.is_valid());
    }

    #[test]
    fn ext_auth_validator_merge() {
        let mut v1 = ExtAuthValidator::new();
        v1.add_error("e1");
        let mut v2 = ExtAuthValidator::new();
        v2.add_error("e2");
        v2.add_warning("w1");
        v1.merge(&v2);
        assert_eq!(v1.error_count(), 2);
        assert_eq!(v1.warning_count(), 1);
    }

    #[test]
    fn ext_auth_rate_tracker_clear() {
        let mut rt = ExtAuthRateTracker::new(1000);
        rt.record(100);
        rt.clear();
        assert_eq!(rt.count(), 0);
    }


    #[test]
    fn yi_metrics_empty() {
        let m = YiMetrics::new("ext_auth");
        assert_eq!(m.count(), 0);
        assert!((m.mean() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn yi_metrics_record_and_mean() {
        let mut m = YiMetrics::new("ext_auth");
        m.record(10.0);
        m.record(20.0);
        m.record(30.0);
        assert_eq!(m.count(), 3);
        assert!((m.mean() - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn yi_metrics_min_max() {
        let mut m = YiMetrics::new("test");
        m.record(5.0);
        m.record(15.0);
        m.record(10.0);
        assert!((m.min_val() - 5.0).abs() < f64::EPSILON);
        assert!((m.max_val() - 15.0).abs() < f64::EPSILON);
    }

    #[test]
    fn yi_metrics_variance_and_std() {
        let mut m = YiMetrics::new("v");
        m.record(2.0);
        m.record(4.0);
        m.record(4.0);
        m.record(4.0);
        m.record(5.0);
        m.record(5.0);
        m.record(7.0);
        m.record(9.0);
        assert!(m.variance() > 0.0);
        assert!(m.std_dev() > 0.0);
    }

    #[test]
    fn yi_metrics_percentile() {
        let mut m = YiMetrics::new("p");
        for i in 1..=100 {
            m.record(i as f64);
        }
        let p50 = m.percentile(50.0);
        assert!(p50 >= 49.0 && p50 <= 51.0);
    }

    #[test]
    fn yi_metrics_merge() {
        let mut a = YiMetrics::new("a");
        a.record(1.0);
        let mut b = YiMetrics::new("b");
        b.record(2.0);
        b.record(3.0);
        a.merge(&b);
        assert_eq!(a.count(), 3);
    }

    #[test]
    fn yi_metrics_reset() {
        let mut m = YiMetrics::new("r");
        m.record(42.0);
        m.reset();
        assert_eq!(m.count(), 0);
    }

    #[test]
    fn yi_rate_window_empty() {
        let rw = YiRateWindow::new(1000);
        assert!(rw.is_empty());
        assert_eq!(rw.window_ms(), 1000);
    }

    #[test]
    fn yi_rate_window_tick_and_rate() {
        let mut rw = YiRateWindow::new(1000);
        rw.tick(100);
        rw.tick(200);
        rw.tick(300);
        assert_eq!(rw.rate(500), 3);
        assert_eq!(rw.rate(1500), 0);
    }

    #[test]
    fn yi_lru_cache_basic() {
        let mut c = YiLruCache::new(2);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        assert_eq!(c.get("a"), Some("1".to_string()));
        c.put("c".into(), "3".into());
        assert_eq!(c.get("b"), None);
    }

    #[test]
    fn yi_lru_cache_contains_and_keys() {
        let mut c = YiLruCache::new(3);
        c.put("x".into(), "10".into());
        c.put("y".into(), "20".into());
        assert!(c.contains_key("x"));
        assert!(!c.contains_key("z"));
        assert_eq!(c.keys().len(), 2);
    }

    #[test]
    fn yi_lru_cache_remove() {
        let mut c = YiLruCache::new(3);
        c.put("k".into(), "v".into());
        assert_eq!(c.remove("k"), Some("v".to_string()));
        assert!(c.is_empty());
        assert_eq!(c.remove("k"), None);
    }

    #[test]
    fn yi_metrics_sum() {
        let mut m = YiMetrics::new("s");
        m.record(1.0);
        m.record(2.0);
        m.record(3.0);
        assert!((m.sum() - 6.0).abs() < f64::EPSILON);
    }

    #[test]
    fn yi_metrics_label() {
        let m = YiMetrics::new("my_label");
        assert_eq!(m.label(), "my_label");
    }

    #[test]
    fn yi_lru_cache_clear() {
        let mut c = YiLruCache::new(5);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // xa_ extended tests for ext_auth
    #[test]
    fn xa_ext_auth_ring_new() {
        let rb = super::XaExtAuthRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_ext_auth_ring_push_len() {
        let mut rb = super::XaExtAuthRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_ext_auth_ring_wrap() {
        let mut rb = super::XaExtAuthRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_ext_auth_ring_mean_empty() {
        let rb = super::XaExtAuthRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_ext_auth_ring_mean_values() {
        let mut rb = super::XaExtAuthRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_ext_auth_ring_min_max() {
        let mut rb = super::XaExtAuthRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_ext_auth_ring_iter() {
        let mut rb = super::XaExtAuthRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_ext_auth_counter_new() {
        let c = super::XaExtAuthCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_ext_auth_counter_inc() {
        let mut c = super::XaExtAuthCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_ext_auth_counter_inc_by() {
        let mut c = super::XaExtAuthCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_ext_auth_counter_reset() {
        let mut c = super::XaExtAuthCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_ext_auth_counter_clear() {
        let mut c = super::XaExtAuthCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_ext_auth_counter_default() {
        let c = super::XaExtAuthCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }


    // ---- xc_ pool / scheduler tests – block 50 ----

    #[test]
    fn xc_50_pool_new_empty() {
        let pool: super::Xc50Pool<i32> = super::Xc50Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_50_pool_release_acquire() {
        let mut pool = super::Xc50Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_50_pool_acquire_empty() {
        let mut pool: super::Xc50Pool<i32> = super::Xc50Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_50_pool_full() {
        let mut pool = super::Xc50Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_50_pool_drain() {
        let mut pool = super::Xc50Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_50_pool_stats() {
        let mut pool = super::Xc50Pool::new(8);
        pool.release(1);
        pool.release(2);
        let _ = pool.acquire();
        let s = pool.stats();
        assert_eq!(s.capacity, 8);
        assert_eq!(s.len, 1);
        assert_eq!(s.acquired, 1);
        assert_eq!(s.available, 1);
    }

    #[test]
    fn xc_50_pool_clear() {
        let mut pool = super::Xc50Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_50_pool_shrink() {
        let mut pool = super::Xc50Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_50_pool_default() {
        let pool: super::Xc50Pool<String> = super::Xc50Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_50_pool_extend() {
        let mut pool = super::Xc50Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_50_pool_retain() {
        let mut pool = super::Xc50Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_50_scheduler_round_robin() {
        let mut sched = super::Xc50Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_50_scheduler_empty() {
        let mut sched = super::Xc50Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_50_scheduler_reset() {
        let mut sched = super::Xc50Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_50_scheduler_add_remove() {
        let mut sched = super::Xc50Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_50_scheduler_targets() {
        let sched = super::Xc50Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_50_hash_empty() {
        assert_eq!(super::xc_50_hash(b""), 5381);
    }

    #[test]
    fn xc_50_hash_data() {
        let h = super::xc_50_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_50_hash(b"hello"), h);
    }

    #[test]
    fn xc_50_reverse_str() {
        assert_eq!(super::xc_50_reverse("abc"), "cba");
        assert_eq!(super::xc_50_reverse(""), "");
    }


    // --- xd_86 deepening tests ---

    #[test]
    fn xd_86_sm_initial_state() {
        let sm = Xd86StateMachine::new();
        assert_eq!(sm.current_state(), Xd86State::Idle);
        assert!(sm.history().is_empty());
        assert_eq!(sm.step_count(), 0);
    }

    #[test]
    fn xd_86_sm_valid_idle_to_running() {
        let mut sm = Xd86StateMachine::new();
        assert!(sm.transition(Xd86State::Running).is_ok());
        assert_eq!(sm.current_state(), Xd86State::Running);
    }

    #[test]
    fn xd_86_sm_valid_running_to_paused() {
        let mut sm = Xd86StateMachine::new();
        sm.transition(Xd86State::Running).unwrap();
        assert!(sm.transition(Xd86State::Paused).is_ok());
        assert_eq!(sm.current_state(), Xd86State::Paused);
    }

    #[test]
    fn xd_86_sm_valid_running_to_done() {
        let mut sm = Xd86StateMachine::new();
        sm.transition(Xd86State::Running).unwrap();
        assert!(sm.transition(Xd86State::Done).is_ok());
        assert_eq!(sm.current_state(), Xd86State::Done);
    }

    #[test]
    fn xd_86_sm_valid_paused_to_running() {
        let mut sm = Xd86StateMachine::new();
        sm.transition(Xd86State::Running).unwrap();
        sm.transition(Xd86State::Paused).unwrap();
        assert!(sm.transition(Xd86State::Running).is_ok());
    }

    #[test]
    fn xd_86_sm_valid_done_to_idle() {
        let mut sm = Xd86StateMachine::new();
        sm.transition(Xd86State::Running).unwrap();
        sm.transition(Xd86State::Done).unwrap();
        assert!(sm.transition(Xd86State::Idle).is_ok());
        assert_eq!(sm.current_state(), Xd86State::Idle);
    }

    #[test]
    fn xd_86_sm_invalid_idle_to_done() {
        let mut sm = Xd86StateMachine::new();
        assert!(sm.transition(Xd86State::Done).is_err());
    }

    #[test]
    fn xd_86_sm_invalid_idle_to_paused() {
        let mut sm = Xd86StateMachine::new();
        assert!(sm.transition(Xd86State::Paused).is_err());
    }

    #[test]
    fn xd_86_sm_history_tracking() {
        let mut sm = Xd86StateMachine::new();
        sm.transition(Xd86State::Running).unwrap();
        sm.transition(Xd86State::Paused).unwrap();
        sm.transition(Xd86State::Done).unwrap();
        assert_eq!(sm.history().len(), 3);
        assert_eq!(sm.history()[0].from, Xd86State::Idle);
        assert_eq!(sm.history()[0].to, Xd86State::Running);
        assert_eq!(sm.history()[1].from, Xd86State::Running);
        assert_eq!(sm.history()[2].to, Xd86State::Done);
    }

    #[test]
    fn xd_86_sm_serialize_deserialize() {
        let mut sm = Xd86StateMachine::new();
        sm.transition(Xd86State::Running).unwrap();
        let s = sm.serialize();
        assert!(s.contains("current=Running"));
        let recovered = Xd86StateMachine::deserialize_current(&s);
        assert_eq!(recovered, Some(Xd86State::Running));
    }

    #[test]
    fn xd_86_sm_deserialize_invalid() {
        assert_eq!(Xd86StateMachine::deserialize_current("garbage"), None);
    }

    #[test]
    fn xd_86_sm_reset() {
        let mut sm = Xd86StateMachine::new();
        sm.transition(Xd86State::Running).unwrap();
        sm.reset();
        assert_eq!(sm.current_state(), Xd86State::Idle);
        assert!(sm.history().is_empty());
    }

    #[test]
    fn xd_86_bus_publish_and_receive() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd86EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe(move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd86Event::Started("go".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(bus.published_events().len(), 1);
    }

    #[test]
    fn xd_86_bus_filtered_subscribe() {
        use std::sync::{Arc, atomic::{AtomicUsize, Ordering}};
        let mut bus = Xd86EventBus::new();
        let count = Arc::new(AtomicUsize::new(0));
        let c = count.clone();
        bus.subscribe_filtered("error", move |_| { c.fetch_add(1, Ordering::SeqCst); });
        bus.publish(Xd86Event::Started("a".into()));
        assert_eq!(count.load(Ordering::SeqCst), 0);
        bus.publish(Xd86Event::Error("fail".into()));
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn xd_86_bus_unsubscribe() {
        let mut bus = Xd86EventBus::new();
        let id = bus.subscribe(|_| {});
        assert_eq!(bus.subscriber_count(), 1);
        assert!(bus.unsubscribe(id));
        assert_eq!(bus.subscriber_count(), 0);
        assert!(!bus.unsubscribe(id));
    }

    #[test]
    fn xd_86_event_kind_and_payload() {
        let e = Xd86Event::Custom("mytype".into(), "mydata".into());
        assert_eq!(e.kind(), "mytype");
        assert_eq!(e.payload(), "mydata");
        let e2 = Xd86Event::Started("hello".into());
        assert_eq!(e2.kind(), "started");
        assert_eq!(e2.payload(), "hello");
    }

    #[test]
    fn xd_86_bus_clear_history() {
        let mut bus = Xd86EventBus::new();
        bus.publish(Xd86Event::Stopped("x".into()));
        assert_eq!(bus.published_events().len(), 1);
        bus.clear_history();
        assert!(bus.published_events().is_empty());
    }

    #[test]
    fn xd_86_sm_step_counter_increments() {
        let mut sm = Xd86StateMachine::new();
        sm.transition(Xd86State::Running).unwrap();
        assert_eq!(sm.step_count(), 1);
        sm.transition(Xd86State::Paused).unwrap();
        assert_eq!(sm.step_count(), 2);
    }


    // -- xf_ trie + bloom tests for instance #107 --

    #[test]
    fn xf107_trie_insert_search() {
        let mut t = Xf107Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf107_trie_starts_with() {
        let mut t = Xf107Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf107_trie_remove() {
        let mut t = Xf107Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf107_trie_word_count() {
        let mut t = Xf107Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf107_trie_longest_prefix() {
        let mut t = Xf107Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf107_trie_all_words() {
        let mut t = Xf107Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf107_trie_autocomplete() {
        let mut t = Xf107Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf107_trie_empty_search() {
        let t = Xf107Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf107_bloom_add_contains() {
        let mut bf = Xf107BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf107_bloom_probably_absent() {
        let bf = Xf107BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf107_bloom_false_positive_rate() {
        let mut bf = Xf107BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf107_bloom_clear() {
        let mut bf = Xf107BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf107_bloom_union() {
        let mut a = Xf107BloomFilter::xf_new(512, 2);
        let mut b = Xf107BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf107_bloom_intersection_estimate() {
        let mut a = Xf107BloomFilter::xf_new(512, 2);
        let mut b = Xf107BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf107_bloom_union_size_mismatch() {
        let a = Xf107BloomFilter::xf_new(256, 2);
        let b = Xf107BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }


    #[test]
    fn xh49_skip_insert_contains() {
        let mut sl = super::Xh49SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh49_skip_remove() {
        let mut sl = super::Xh49SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh49_skip_len() {
        let mut sl = super::Xh49SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh49_skip_range_query() {
        let mut sl = super::Xh49SkipList::xh_new(4);
        for v in [3, 7, 1, 9, 5] {
            sl.xh_insert(v);
        }
        let r = sl.xh_range_query(3, 7);
        assert!(r.contains(&3));
        assert!(r.contains(&5));
        assert!(r.contains(&7));
        assert!(!r.contains(&1));
        assert!(!r.contains(&9));
    }

    #[test]
    fn xh49_skip_floor_ceiling() {
        let mut sl = super::Xh49SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh49_skip_rank() {
        let mut sl = super::Xh49SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh49_skip_empty() {
        let sl = super::Xh49SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh49_skip_duplicates() {
        let mut sl = super::Xh49SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh49_bitset_set_test() {
        let mut bs = super::Xh49BitSet::xh_new(256);
        bs.xh_set(0);
        bs.xh_set(63);
        bs.xh_set(64);
        bs.xh_set(255);
        assert!(bs.xh_test(0));
        assert!(bs.xh_test(63));
        assert!(bs.xh_test(64));
        assert!(bs.xh_test(255));
        assert!(!bs.xh_test(1));
    }

    #[test]
    fn xh49_bitset_clear_count() {
        let mut bs = super::Xh49BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh49_bitset_and_or_xor() {
        let mut a = super::Xh49BitSet::xh_new(128);
        let mut b = super::Xh49BitSet::xh_new(128);
        a.xh_set(1);
        a.xh_set(2);
        b.xh_set(2);
        b.xh_set(3);
        let and_r = a.xh_and(&b);
        assert!(and_r.xh_test(2));
        assert!(!and_r.xh_test(1));
        let or_r = a.xh_or(&b);
        assert!(or_r.xh_test(1));
        assert!(or_r.xh_test(2));
        assert!(or_r.xh_test(3));
        let xor_r = a.xh_xor(&b);
        assert!(xor_r.xh_test(1));
        assert!(!xor_r.xh_test(2));
        assert!(xor_r.xh_test(3));
    }

    #[test]
    fn xh49_bitset_iter_ones() {
        let mut bs = super::Xh49BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh49_bitset_first_last() {
        let mut bs = super::Xh49BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh49_bitset_empty() {
        let bs = super::Xh49BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi49_deque_push_pop_back() {
        let mut dq = super::Xi49Deque::xi_new(4);
        dq.xi_push_back(10);
        dq.xi_push_back(20);
        dq.xi_push_back(30);
        assert_eq!(dq.xi_len(), 3);
        assert_eq!(dq.xi_pop_back(), Some(30));
        assert_eq!(dq.xi_pop_back(), Some(20));
        assert_eq!(dq.xi_pop_back(), Some(10));
        assert_eq!(dq.xi_pop_back(), None);
    }

    #[test]
    fn xi49_deque_push_pop_front() {
        let mut dq = super::Xi49Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi49_deque_mixed_ops() {
        let mut dq = super::Xi49Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi49_deque_get_and_split() {
        let mut dq = super::Xi49Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_get(0), Some(&0));
        assert_eq!(dq.xi_get(4), Some(&4));
        assert_eq!(dq.xi_get(5), None);
        let (left, right) = dq.xi_split_at(3);
        assert_eq!(left, vec![0, 1, 2]);
        assert_eq!(right, vec![3, 4]);
    }

    #[test]
    fn xi49_deque_rotate_left() {
        let mut dq = super::Xi49Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi49_deque_rotate_right() {
        let mut dq = super::Xi49Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi49_deque_grow() {
        let mut dq = super::Xi49Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi49_deque_empty() {
        let dq = super::Xi49Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi49_interval_tree_insert_query() {
        let mut tree = super::Xi49IntervalTree::xi_new();
        tree.xi_insert(super::Xi49Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi49Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi49Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi49_interval_tree_overlap() {
        let mut tree = super::Xi49IntervalTree::xi_new();
        tree.xi_insert(super::Xi49Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi49Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi49Interval::xi_new(12, 20));
        let q = super::Xi49Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi49_interval_tree_remove() {
        let mut tree = super::Xi49IntervalTree::xi_new();
        tree.xi_insert(super::Xi49Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi49Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi49_interval_tree_gaps() {
        let mut tree = super::Xi49IntervalTree::xi_new();
        tree.xi_insert(super::Xi49Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi49Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi49Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi49Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi49Interval::xi_new(8, 10));
    }

    #[test]
    fn xi49_interval_tree_merge() {
        let mut tree = super::Xi49IntervalTree::xi_new();
        tree.xi_insert(super::Xi49Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi49Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi49Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi49Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi49Interval::xi_new(10, 15));
    }

    #[test]
    fn xi49_interval_tree_all() {
        let mut tree = super::Xi49IntervalTree::xi_new();
        tree.xi_insert(super::Xi49Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi49Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi49_interval_tree_empty() {
        let tree = super::Xi49IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi49_interval_tree_contains_point() {
        let iv = super::Xi49Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }


    // --- xj_ tests for union-find and btree (crate index 49) ---

    #[test]
    fn xj_49_uf_make_and_find() {
        let mut uf = super::Xj49UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_find(a), a);
        assert_eq!(uf.xj_find(b), b);
        assert_ne!(uf.xj_find(a), uf.xj_find(b));
    }

    #[test]
    fn xj_49_uf_union_connected() {
        let mut uf = super::Xj49UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert!(!uf.xj_connected(a, b));
        assert!(uf.xj_union(a, b));
        assert!(uf.xj_connected(a, b));
        assert!(!uf.xj_union(a, b));
    }

    #[test]
    fn xj_49_uf_component_count() {
        let mut uf = super::Xj49UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let c = uf.xj_make_set();
        assert_eq!(uf.xj_component_count(), 3);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_count(), 2);
        uf.xj_union(b, c);
        assert_eq!(uf.xj_component_count(), 1);
    }

    #[test]
    fn xj_49_uf_component_size() {
        let mut uf = super::Xj49UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        assert_eq!(uf.xj_component_size(a), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_component_size(a), 2);
        assert_eq!(uf.xj_component_size(b), 2);
    }

    #[test]
    fn xj_49_uf_largest_component() {
        let mut uf = super::Xj49UnionFind::xj_new();
        assert_eq!(uf.xj_largest_component(), 0);
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let _c = uf.xj_make_set();
        assert_eq!(uf.xj_largest_component(), 1);
        uf.xj_union(a, b);
        assert_eq!(uf.xj_largest_component(), 2);
    }

    #[test]
    fn xj_49_uf_many_elements() {
        let mut uf = super::Xj49UnionFind::xj_new();
        let ids: Vec<usize> = (0..10).map(|_| uf.xj_make_set()).collect();
        for i in 1..10 { uf.xj_union(ids[0], ids[i]); }
        assert_eq!(uf.xj_component_count(), 1);
        assert_eq!(uf.xj_component_size(ids[5]), 10);
    }

    #[test]
    fn xj_49_uf_separate_components() {
        let mut uf = super::Xj49UnionFind::xj_new();
        let a = uf.xj_make_set();
        let b = uf.xj_make_set();
        let c = uf.xj_make_set();
        let d = uf.xj_make_set();
        uf.xj_union(a, b);
        uf.xj_union(c, d);
        assert!(uf.xj_connected(a, b));
        assert!(uf.xj_connected(c, d));
        assert!(!uf.xj_connected(a, c));
    }

    #[test]
    fn xj_49_uf_path_compression() {
        let mut uf = super::Xj49UnionFind::xj_new();
        let ids: Vec<usize> = (0..5).map(|_| uf.xj_make_set()).collect();
        for i in 1..5 { uf.xj_union(ids[i - 1], ids[i]); }
        let root = uf.xj_find(ids[4]);
        assert_eq!(uf.xj_find(ids[0]), root);
    }

    #[test]
    fn xj_49_bt_insert_get() {
        let mut bt = super::Xj49BTree::<i32, String>::xj_new();
        assert!(bt.xj_insert(5, "five".into()).is_none());
        assert!(bt.xj_insert(3, "three".into()).is_none());
        assert_eq!(bt.xj_get(&5), Some(&"five".into()));
        assert_eq!(bt.xj_get(&3), Some(&"three".into()));
        assert_eq!(bt.xj_get(&99), None);
    }

    #[test]
    fn xj_49_bt_contains_len() {
        let mut bt = super::Xj49BTree::<i32, i32>::xj_new();
        for i in 0..10 { bt.xj_insert(i, i * 10); }
        assert_eq!(bt.xj_len(), 10);
        assert!(bt.xj_contains_key(&7));
        assert!(!bt.xj_contains_key(&42));
    }

    #[test]
    fn xj_49_bt_replace() {
        let mut bt = super::Xj49BTree::<i32, &str>::xj_new();
        bt.xj_insert(1, "a");
        bt.xj_insert(2, "b");
    }

    #[test]
    fn xj_49_bt_remove() {
        let mut bt = super::Xj49BTree::<i32, i32>::xj_new();
        for i in 0..8 { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_remove(&3), Some(3));
        assert!(!bt.xj_contains_key(&3));
        assert_eq!(bt.xj_len(), 7);
        assert_eq!(bt.xj_remove(&3), None);
    }

    #[test]
    fn xj_49_bt_keys_values() {
        let mut bt = super::Xj49BTree::<i32, i32>::xj_new();
        for i in [5, 1, 9, 3, 7] { bt.xj_insert(i, i * 2); }
        assert_eq!(bt.xj_keys(), vec![1, 3, 5, 7, 9]);
        assert_eq!(bt.xj_values(), vec![2, 6, 10, 14, 18]);
    }

    #[test]
    fn xj_49_bt_range() {
        let mut bt = super::Xj49BTree::<i32, i32>::xj_new();
        for i in 0..20 { bt.xj_insert(i, i); }
        let r = bt.xj_range(&5, &10);
        let rk: Vec<i32> = r.iter().map(|(k, _)| *k).collect();
        assert_eq!(rk, vec![5, 6, 7, 8, 9, 10]);
    }

    #[test]
    fn xj_49_bt_min_max() {
        let mut bt = super::Xj49BTree::<i32, i32>::xj_new();
        assert_eq!(bt.xj_min_key(), None);
        assert_eq!(bt.xj_max_key(), None);
        for i in [20, 5, 15, 1, 30] { bt.xj_insert(i, i); }
        assert_eq!(bt.xj_min_key(), Some(&1));
        assert_eq!(bt.xj_max_key(), Some(&30));
    }

    #[test]
    fn xj_49_bt_many_inserts() {
        let mut bt = super::Xj49BTree::<i32, i32>::xj_new();
        for i in 0..100 { bt.xj_insert(i, i * 3); }
        assert_eq!(bt.xj_len(), 100);
        for i in 0..100 { assert_eq!(bt.xj_get(&i), Some(&(i * 3))); }
        assert_eq!(bt.xj_min_key(), Some(&0));
        assert_eq!(bt.xj_max_key(), Some(&99));
    }


    // --- xk_49 segment tree tests ---

    #[test]
    fn xk_49_st_build_query() {
        let data = vec![1, 3, 5, 7, 9, 11];
        let st = super::Xk49SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 5), 36);
        assert_eq!(st.xk_query(1, 3), 15);
    }

    #[test]
    fn xk_49_st_update() {
        let data = vec![2, 4, 6, 8];
        let mut st = super::Xk49SegmentTree::xk_build(&data);
        st.xk_update(2, 10);
        assert_eq!(st.xk_query(0, 3), 24);
        assert_eq!(st.xk_query(2, 2), 10);
    }

    #[test]
    fn xk_49_st_range_min() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk49SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_min(0, 4), 1);
        assert_eq!(st.xk_range_min(0, 2), 2);
    }

    #[test]
    fn xk_49_st_range_max() {
        let data = vec![5, 2, 8, 1, 9];
        let st = super::Xk49SegmentTree::xk_build(&data);
        assert_eq!(st.xk_range_max(0, 4), 9);
        assert_eq!(st.xk_range_max(1, 3), 8);
    }

    #[test]
    fn xk_49_st_len() {
        let data = vec![10, 20, 30];
        let st = super::Xk49SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 3);
    }

    #[test]
    fn xk_49_st_single_element() {
        let data = vec![42];
        let st = super::Xk49SegmentTree::xk_build(&data);
        assert_eq!(st.xk_query(0, 0), 42);
        assert_eq!(st.xk_range_min(0, 0), 42);
        assert_eq!(st.xk_range_max(0, 0), 42);
    }

    #[test]
    fn xk_49_st_update_and_min_max() {
        let data = vec![3, 1, 4, 1, 5];
        let mut st = super::Xk49SegmentTree::xk_build(&data);
        st.xk_update(1, 10);
        assert_eq!(st.xk_range_max(0, 4), 10);
        assert_eq!(st.xk_range_min(0, 4), 1);
    }

    #[test]
    fn xk_49_st_empty() {
        let data: Vec<i64> = vec![];
        let st = super::Xk49SegmentTree::xk_build(&data);
        assert_eq!(st.xk_len(), 0);
        assert_eq!(st.xk_query(0, 0), 0);
    }

    // --- xk_49 disjoint intervals tests ---

    #[test]
    fn xk_49_di_add_and_count() {
        let mut di = super::Xk49DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(10, 15);
        assert_eq!(di.xk_interval_count(), 2);
    }

    #[test]
    fn xk_49_di_merge_overlap() {
        let mut di = super::Xk49DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(3, 8);
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 8);
    }

    #[test]
    fn xk_49_di_contains() {
        let mut di = super::Xk49DisjointIntervals::xk_new();
        di.xk_add_interval(10, 20);
        assert!(di.xk_contains_point(15));
        assert!(!di.xk_contains_point(9));
        assert!(!di.xk_contains_point(21));
    }

    #[test]
    fn xk_49_di_remove() {
        let mut di = super::Xk49DisjointIntervals::xk_new();
        di.xk_add_interval(1, 10);
        di.xk_remove_interval(4, 6);
        assert_eq!(di.xk_interval_count(), 2);
        assert!(!di.xk_contains_point(5));
        assert!(di.xk_contains_point(3));
        assert!(di.xk_contains_point(7));
    }

    #[test]
    fn xk_49_di_covered_length() {
        let mut di = super::Xk49DisjointIntervals::xk_new();
        di.xk_add_interval(0, 4);
        di.xk_add_interval(10, 14);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_49_di_gaps() {
        let mut di = super::Xk49DisjointIntervals::xk_new();
        di.xk_add_interval(1, 3);
        di.xk_add_interval(7, 9);
        let gaps = di.xk_gaps();
        assert_eq!(gaps, vec![(4, 6)]);
    }

    #[test]
    fn xk_49_di_merge_adjacent() {
        let mut di = super::Xk49DisjointIntervals::xk_new();
        di.xk_add_interval(1, 5);
        di.xk_add_interval(6, 10);
        di.xk_merge_adjacent();
        assert_eq!(di.xk_interval_count(), 1);
        assert_eq!(di.xk_covered_length(), 10);
    }

    #[test]
    fn xk_49_di_empty() {
        let di = super::Xk49DisjointIntervals::xk_new();
        assert_eq!(di.xk_interval_count(), 0);
        assert_eq!(di.xk_covered_length(), 0);
        assert!(!di.xk_contains_point(0));
    }


    #[test]
    fn xl_49_rope_new_empty() {
        let rope = super::Xl49Rope::xl_new();
        assert_eq!(rope.xl_len(), 0);
        assert!(rope.xl_is_empty());
    }

    #[test]
    fn xl_49_rope_from_str() {
        let rope = super::Xl49Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_len(), 11);
        assert_eq!(rope.xl_to_string(), "hello world");
    }

    #[test]
    fn xl_49_rope_insert_at() {
        let mut rope = super::Xl49Rope::xl_from_str("helo");
        rope.xl_insert_at(2, "l");
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_49_rope_delete_range() {
        let mut rope = super::Xl49Rope::xl_from_str("hello world");
        rope.xl_delete_range(5, 11);
        assert_eq!(rope.xl_to_string(), "hello");
    }

    #[test]
    fn xl_49_rope_char_at() {
        let rope = super::Xl49Rope::xl_from_str("abcdef");
        assert_eq!(rope.xl_char_at(0), Some('a'));
        assert_eq!(rope.xl_char_at(5), Some('f'));
        assert_eq!(rope.xl_char_at(6), None);
    }

    #[test]
    fn xl_49_rope_split_concat() {
        let rope = super::Xl49Rope::xl_from_str("hello world");
        let (left, right) = rope.xl_split(5);
        assert_eq!(left.xl_to_string(), "hello");
        assert_eq!(right.xl_to_string(), " world");
    }

    #[test]
    fn xl_49_rope_line_count() {
        let rope = super::Xl49Rope::xl_from_str("line1\nline2\nline3");
        assert_eq!(rope.xl_line_count(), 3);
    }

    #[test]
    fn xl_49_rope_line_at() {
        let rope = super::Xl49Rope::xl_from_str("aaa\nbbb\nccc");
        assert_eq!(rope.xl_line_at(0), Some("aaa".to_string()));
        assert_eq!(rope.xl_line_at(2), Some("ccc".to_string()));
        assert_eq!(rope.xl_line_at(3), None);
    }

    #[test]
    fn xl_49_sa_build_and_search() {
        let sa = super::Xl49SuffixArray::xl_build("banana");
        assert!(sa.xl_search("ana").is_some());
        assert!(sa.xl_search("xyz").is_none());
    }

    #[test]
    fn xl_49_sa_count() {
        let sa = super::Xl49SuffixArray::xl_build("banana");
        assert_eq!(sa.xl_count_occurrences("ana"), 2);
        assert_eq!(sa.xl_count_occurrences("ban"), 1);
        assert_eq!(sa.xl_count_occurrences("xyz"), 0);
    }

    #[test]
    fn xl_49_sa_longest_repeated() {
        let sa = super::Xl49SuffixArray::xl_build("banana");
        let lr = sa.xl_longest_repeated();
        assert_eq!(lr, "ana");
    }

    #[test]
    fn xl_49_sa_all_positions() {
        let sa = super::Xl49SuffixArray::xl_build("abcabc");
        let pos = sa.xl_all_positions("abc");
        assert_eq!(pos, vec![0, 3]);
    }

    #[test]
    fn xl_49_sa_len() {
        let sa = super::Xl49SuffixArray::xl_build("test");
        assert_eq!(sa.xl_len(), 4);
        assert!(!sa.xl_is_empty());
    }

    #[test]
    fn xl_49_sa_empty() {
        let sa = super::Xl49SuffixArray::xl_build("");
        assert_eq!(sa.xl_len(), 0);
        assert!(sa.xl_is_empty());
        assert_eq!(sa.xl_count_occurrences("x"), 0);
    }

    #[test]
    fn xl_49_rope_slice() {
        let rope = super::Xl49Rope::xl_from_str("hello world");
        assert_eq!(rope.xl_slice(0, 5), "hello");
    }

    #[test]
    fn xl_49_sa_search_start() {
        let sa = super::Xl49SuffixArray::xl_build("hello world");
        let pos = sa.xl_search("hello");
        assert_eq!(pos, Some(0));
    }
}