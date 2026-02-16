//! OAuth provider integration.

use std::collections::HashMap;
use std::fmt;

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

/// Errors that may occur when working with the authentication service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthError {
    /// The provider id was empty or contained invalid characters.
    InvalidProviderId(String),
    /// A provider with the given id is already registered.
    ProviderAlreadyRegistered(String),
    /// No provider with the given id exists.
    ProviderNotFound(String),
    /// The provider is not in the `Active` state.
    ProviderNotActive(String),
    /// The requested session does not exist.
    SessionNotFound(String),
    /// The provider does not support multiple accounts and one already exists.
    MultipleAccountsNotSupported(String),
}

impl fmt::Display for AuthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidProviderId(id) => write!(f, "invalid provider id: '{id}'"),
            Self::ProviderAlreadyRegistered(id) => {
                write!(f, "provider '{id}' is already registered")
            }
            Self::ProviderNotFound(id) => write!(f, "provider '{id}' not found"),
            Self::ProviderNotActive(id) => write!(f, "provider '{id}' is not active"),
            Self::SessionNotFound(id) => write!(f, "session '{id}' not found"),
            Self::MultipleAccountsNotSupported(id) => {
                write!(f, "provider '{id}' does not support multiple accounts")
            }
        }
    }
}

impl std::error::Error for AuthError {}

// ---------------------------------------------------------------------------
// AuthProviderStatus
// ---------------------------------------------------------------------------

/// The lifecycle status of a registered authentication provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AuthProviderStatus {
    Registered,
    Active,
    Disabled,
}

impl AuthProviderStatus {
    /// Returns `true` when the provider is considered usable.
    pub fn is_usable(&self) -> bool {
        matches!(self, Self::Registered | Self::Active)
    }
}

impl fmt::Display for AuthProviderStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::Registered => "Registered",
            Self::Active => "Active",
            Self::Disabled => "Disabled",
        };
        f.write_str(label)
    }
}

// ---------------------------------------------------------------------------
// AuthProvider + builder
// ---------------------------------------------------------------------------

/// A registered authentication provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthProvider {
    pub id: String,
    pub label: String,
    pub supports_multiple_accounts: bool,
    pub status: AuthProviderStatus,
}

impl AuthProvider {
    /// Validate that the provider id is non-empty and ASCII-alphanumeric
    /// (plus hyphens / underscores).
    pub fn validate_id(id: &str) -> Result<(), AuthError> {
        if id.is_empty()
            || !id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return Err(AuthError::InvalidProviderId(id.to_string()));
        }
        Ok(())
    }
}

/// Fluent builder for [`AuthProvider`].
#[derive(Debug, Clone)]
pub struct AuthProviderBuilder {
    id: String,
    label: String,
    supports_multiple_accounts: bool,
    status: AuthProviderStatus,
}

impl AuthProviderBuilder {
    pub fn new(id: impl Into<String>) -> Self {
        let id = id.into();
        Self {
            label: id.clone(),
            id,
            supports_multiple_accounts: false,
            status: AuthProviderStatus::Registered,
        }
    }

    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }

    pub fn supports_multiple_accounts(mut self, val: bool) -> Self {
        self.supports_multiple_accounts = val;
        self
    }

    pub fn status(mut self, status: AuthProviderStatus) -> Self {
        self.status = status;
        self
    }

    /// Build the [`AuthProvider`], validating the id.
    pub fn build(self) -> Result<AuthProvider, AuthError> {
        AuthProvider::validate_id(&self.id)?;
        Ok(AuthProvider {
            id: self.id,
            label: self.label,
            supports_multiple_accounts: self.supports_multiple_accounts,
            status: self.status,
        })
    }
}

impl std::fmt::Display for AuthProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.label, self.id)
    }
}

/// A session created through an authentication provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthenticationSession {
    pub id: String,
    pub provider_id: String,
    pub account_label: String,
    pub scopes: Vec<String>,
}

impl AuthenticationSession {
    /// Returns `true` if the session includes the given scope.
    pub fn has_scope(&self, scope: &str) -> bool {
        self.scopes.iter().any(|s| s == scope)
    }

    /// Returns `true` if the session includes **all** of the given scopes.
    pub fn has_all_scopes(&self, scopes: &[&str]) -> bool {
        scopes.iter().all(|s| self.has_scope(s))
    }

    /// Returns the number of scopes granted to this session.
    pub fn scope_count(&self) -> usize {
        self.scopes.len()
    }
}

impl std::fmt::Display for AuthenticationSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Session {} (provider={}, account={})",
            self.id, self.provider_id, self.account_label
        )
    }
}

/// Service for managing authentication providers and sessions.
pub struct AuthenticationService {
    providers: Vec<AuthProvider>,
    sessions: Vec<AuthenticationSession>,
    next_session_id: u64,
}

impl AuthenticationService {
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
            sessions: Vec::new(),
            next_session_id: 1,
        }
    }

    pub fn register_provider(&mut self, provider: AuthProvider) {
        self.providers.push(provider);
    }

    pub fn get_provider(&self, id: &str) -> Option<&AuthProvider> {
        self.providers.iter().find(|p| p.id == id)
    }

    /// Creates a new session for the given provider, returning the session id.
    pub fn create_session(&mut self, provider_id: &str, scopes: Vec<String>) -> String {
        let id = format!("session-{}", self.next_session_id);
        self.next_session_id += 1;
        self.sessions.push(AuthenticationSession {
            id: id.clone(),
            provider_id: provider_id.to_string(),
            account_label: format!("account@{provider_id}"),
            scopes,
        });
        id
    }

    pub fn get_session(&self, id: &str) -> Option<&AuthenticationSession> {
        self.sessions.iter().find(|s| s.id == id)
    }

    pub fn remove_session(&mut self, id: &str) -> bool {
        let len = self.sessions.len();
        self.sessions.retain(|s| s.id != id);
        self.sessions.len() != len
    }

    pub fn get_sessions_for_provider(&self, provider_id: &str) -> Vec<&AuthenticationSession> {
        self.sessions
            .iter()
            .filter(|s| s.provider_id == provider_id)
            .collect()
    }

    /// Removes a provider by id, returning `true` if it existed.
    pub fn unregister_provider(&mut self, id: &str) -> bool {
        let len = self.providers.len();
        self.providers.retain(|p| p.id != id);
        self.providers.len() != len
    }

    /// Returns the number of registered providers.
    pub fn provider_count(&self) -> usize {
        self.providers.len()
    }

    /// Returns a slice of all registered providers.
    pub fn get_all_providers(&self) -> &[AuthProvider] {
        &self.providers
    }

    /// Returns a slice of all sessions.
    pub fn get_all_sessions(&self) -> &[AuthenticationSession] {
        &self.sessions
    }

    /// Returns `true` if there is a session for `provider_id` that contains
    /// all of the requested `scopes`.
    pub fn has_session_with_scopes(&self, provider_id: &str, scopes: &[&str]) -> bool {
        self.sessions.iter().any(|s| {
            s.provider_id == provider_id && scopes.iter().all(|scope| s.has_scope(scope))
        })
    }

    /// Returns the total number of sessions.
    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }

    // -- checked / validated variants ------------------------------------------

    /// Register a provider, returning an error if the id is invalid or already
    /// registered.
    pub fn register_provider_checked(
        &mut self,
        provider: AuthProvider,
    ) -> Result<(), AuthError> {
        AuthProvider::validate_id(&provider.id)?;
        if self.get_provider(&provider.id).is_some() {
            return Err(AuthError::ProviderAlreadyRegistered(provider.id));
        }
        self.providers.push(provider);
        Ok(())
    }

    /// Create a session, validating that the provider exists, is active, and
    /// (if it does not support multiple accounts) that no session already
    /// exists for it.
    pub fn create_session_checked(
        &mut self,
        provider_id: &str,
        scopes: Vec<String>,
    ) -> Result<String, AuthError> {
        let provider = self
            .get_provider(provider_id)
            .ok_or_else(|| AuthError::ProviderNotFound(provider_id.to_string()))?;

        if !provider.status.is_usable() {
            return Err(AuthError::ProviderNotActive(provider_id.to_string()));
        }

        if !provider.supports_multiple_accounts
            && !self.get_sessions_for_provider(provider_id).is_empty()
        {
            return Err(AuthError::MultipleAccountsNotSupported(
                provider_id.to_string(),
            ));
        }

        Ok(self.create_session(provider_id, scopes))
    }

    /// Remove a session, returning an error if it does not exist.
    pub fn remove_session_checked(&mut self, id: &str) -> Result<(), AuthError> {
        if self.remove_session(id) {
            Ok(())
        } else {
            Err(AuthError::SessionNotFound(id.to_string()))
        }
    }

    /// Set the status of a provider by id.
    pub fn set_provider_status(
        &mut self,
        id: &str,
        status: AuthProviderStatus,
    ) -> Result<(), AuthError> {
        let provider = self
            .providers
            .iter_mut()
            .find(|p| p.id == id)
            .ok_or_else(|| AuthError::ProviderNotFound(id.to_string()))?;
        provider.status = status;
        Ok(())
    }

    /// Remove all sessions belonging to a given provider, returning the count
    /// of removed sessions.
    pub fn remove_sessions_for_provider(&mut self, provider_id: &str) -> usize {
        let before = self.sessions.len();
        self.sessions.retain(|s| s.provider_id != provider_id);
        before - self.sessions.len()
    }
}

impl Default for AuthenticationService {
    fn default() -> Self {
        Self::new()
    }
}

/// Accumulated statistics for wb-auth operations.
#[derive(Debug, Clone, PartialEq)]
pub struct WbAuthStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl WbAuthStats {
    /// Create a new empty statistics tracker.
    pub fn new() -> Self {
        Self {
            total_operations: 0,
            successful_operations: 0,
            failed_operations: 0,
            last_operation_ns: 0,
            max_operation_ns: 0,
            min_operation_ns: u64::MAX,
            total_time_ns: 0,
        }
    }

    /// Record a successful operation with its duration in nanoseconds.
    pub fn record_success(&mut self, duration_ns: u64) {
        self.total_operations += 1;
        self.successful_operations += 1;
        self.last_operation_ns = duration_ns;
        self.total_time_ns = self.total_time_ns.saturating_add(duration_ns);
        if duration_ns > self.max_operation_ns {
            self.max_operation_ns = duration_ns;
        }
        if duration_ns < self.min_operation_ns {
            self.min_operation_ns = duration_ns;
        }
    }

    /// Record a failed operation with its duration in nanoseconds.
    pub fn record_failure(&mut self, duration_ns: u64) {
        self.total_operations += 1;
        self.failed_operations += 1;
        self.last_operation_ns = duration_ns;
        self.total_time_ns = self.total_time_ns.saturating_add(duration_ns);
        if duration_ns > self.max_operation_ns {
            self.max_operation_ns = duration_ns;
        }
        if duration_ns < self.min_operation_ns {
            self.min_operation_ns = duration_ns;
        }
    }

    /// Return the average operation time in nanoseconds, or 0 if no operations recorded.
    pub fn average_time_ns(&self) -> u64 {
        if self.total_operations == 0 {
            return 0;
        }
        self.total_time_ns / self.total_operations
    }

    /// Return the success rate as a fraction in [0.0, 1.0].
    pub fn success_rate(&self) -> f64 {
        if self.total_operations == 0 {
            return 1.0;
        }
        self.successful_operations as f64 / self.total_operations as f64
    }

    /// Return the failure rate as a fraction in [0.0, 1.0].
    pub fn failure_rate(&self) -> f64 {
        1.0 - self.success_rate()
    }

    /// Return total number of recorded operations.
    pub fn total(&self) -> u64 {
        self.total_operations
    }

    /// Return the minimum operation time, or `None` if no operations recorded.
    pub fn min_time_ns(&self) -> Option<u64> {
        if self.total_operations == 0 {
            None
        } else {
            Some(self.min_operation_ns)
        }
    }

    /// Return the maximum operation time, or `None` if no operations recorded.
    pub fn max_time_ns(&self) -> Option<u64> {
        if self.total_operations == 0 {
            None
        } else {
            Some(self.max_operation_ns)
        }
    }

    /// Reset all counters to zero.
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Merge another stats instance into this one.
    pub fn merge(&mut self, other: &WbAuthStats) {
        self.total_operations += other.total_operations;
        self.successful_operations += other.successful_operations;
        self.failed_operations += other.failed_operations;
        self.total_time_ns = self.total_time_ns.saturating_add(other.total_time_ns);
        if other.max_operation_ns > self.max_operation_ns {
            self.max_operation_ns = other.max_operation_ns;
        }
        if other.total_operations > 0 && other.min_operation_ns < self.min_operation_ns {
            self.min_operation_ns = other.min_operation_ns;
        }
    }
}

impl Default for WbAuthStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for WbAuthStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "WbAuthStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for wb-auth.
#[derive(Debug, Clone)]
pub struct WbAuthValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl WbAuthValidator {
    /// Create a new validator with default settings.
    pub fn new() -> Self {
        Self {
            max_name_length: 256,
            allowed_chars: None,
            forbidden_prefixes: Vec::new(),
        }
    }

    /// Set the maximum allowed name length.
    pub fn max_length(mut self, max: usize) -> Self {
        self.max_name_length = max;
        self
    }

    /// Restrict names to only the given characters.
    pub fn allowed_chars(mut self, chars: &[char]) -> Self {
        self.allowed_chars = Some(chars.to_vec());
        self
    }

    /// Add a forbidden prefix.
    pub fn forbid_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.forbidden_prefixes.push(prefix.into());
        self
    }

    /// Validate a name, returning an error description on failure.
    pub fn validate_name(&self, name: &str) -> Result<(), String> {
        if name.is_empty() {
            return Err("name must not be empty".to_string());
        }
        if name.len() > self.max_name_length {
            return Err(format!(
                "name length {} exceeds maximum {}",
                name.len(),
                self.max_name_length
            ));
        }
        if let Some(ref allowed) = self.allowed_chars {
            for ch in name.chars() {
                if !allowed.contains(&ch) {
                    return Err(format!("character '{}' is not allowed", ch));
                }
            }
        }
        for prefix in &self.forbidden_prefixes {
            if name.starts_with(prefix.as_str()) {
                return Err(format!("name must not start with '{}'", prefix));
            }
        }
        Ok(())
    }

    /// Validate that a numeric value is within the given range.
    pub fn validate_range(&self, value: i64, min: i64, max: i64) -> Result<(), String> {
        if value < min || value > max {
            return Err(format!("value {} is outside range [{}..{}]", value, min, max));
        }
        Ok(())
    }

    /// Check whether a string contains only ASCII printable characters.
    pub fn is_ascii_printable(s: &str) -> bool {
        s.chars().all(|c| c.is_ascii_graphic() || c == ' ')
    }

    /// Sanitize a string by removing control characters.
    pub fn sanitize(s: &str) -> String {
        s.chars().filter(|c| !c.is_control()).collect()
    }

    /// Truncate a string to a maximum number of characters, appending an ellipsis if needed.
    pub fn truncate(s: &str, max_chars: usize) -> String {
        if s.chars().count() <= max_chars {
            return s.to_string();
        }
        let truncated: String = s.chars().take(max_chars.saturating_sub(1)).collect();
        format!("{}…", truncated)
    }
}

impl Default for WbAuthValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// AuthSession
// ---------------------------------------------------------------------------

/// An authentication session with token and expiration management.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthSession {
    pub session_id: String,
    pub provider_id: String,
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: Option<u64>,
    pub scopes: Vec<String>,
}

impl AuthSession {
    /// Create a new auth session.
    pub fn new(
        session_id: impl Into<String>,
        provider_id: impl Into<String>,
        access_token: impl Into<String>,
    ) -> Self {
        Self {
            session_id: session_id.into(),
            provider_id: provider_id.into(),
            access_token: access_token.into(),
            refresh_token: None,
            expires_at: None,
            scopes: Vec::new(),
        }
    }

    /// Set the refresh token.
    pub fn with_refresh_token(mut self, token: impl Into<String>) -> Self {
        self.refresh_token = Some(token.into());
        self
    }

    /// Set the expiration time (unix timestamp in seconds).
    pub fn with_expires_at(mut self, ts: u64) -> Self {
        self.expires_at = Some(ts);
        self
    }

    /// Add scopes to this session.
    pub fn with_scopes(mut self, scopes: Vec<String>) -> Self {
        self.scopes = scopes;
        self
    }

    /// Check if the session is expired at the given current time.
    pub fn is_expired(&self, current_time: u64) -> bool {
        match self.expires_at {
            Some(exp) => current_time >= exp,
            None => false,
        }
    }

    /// Check if a specific scope is granted.
    pub fn has_scope(&self, scope: &str) -> bool {
        self.scopes.iter().any(|s| s == scope)
    }

    /// Time remaining in seconds before expiration, or None if no expiration.
    pub fn time_remaining(&self, current_time: u64) -> Option<u64> {
        self.expires_at.map(|exp| exp.saturating_sub(current_time))
    }
}

impl fmt::Display for AuthSession {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "AuthSession({}, provider={})",
            self.session_id, self.provider_id
        )
    }
}

// ---------------------------------------------------------------------------
// AuthProviderRegistry
// ---------------------------------------------------------------------------

/// Registry that manages authentication providers and their sessions.
pub struct AuthProviderRegistry {
    providers: Vec<AuthProvider>,
    sessions: Vec<AuthSession>,
}

impl AuthProviderRegistry {
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
            sessions: Vec::new(),
        }
    }

    /// Register a new provider.
    pub fn register(&mut self, provider: AuthProvider) -> Result<(), AuthError> {
        if self.providers.iter().any(|p| p.id == provider.id) {
            return Err(AuthError::ProviderAlreadyRegistered(provider.id));
        }
        self.providers.push(provider);
        Ok(())
    }

    /// Find a provider by id.
    pub fn get_provider(&self, id: &str) -> Option<&AuthProvider> {
        self.providers.iter().find(|p| p.id == id)
    }

    /// Add a session.
    pub fn add_session(&mut self, session: AuthSession) {
        self.sessions.push(session);
    }

    /// Get all sessions for a provider.
    pub fn sessions_for_provider(&self, provider_id: &str) -> Vec<&AuthSession> {
        self.sessions
            .iter()
            .filter(|s| s.provider_id == provider_id)
            .collect()
    }

    /// Remove expired sessions given the current time.
    pub fn remove_expired(&mut self, current_time: u64) -> usize {
        let before = self.sessions.len();
        self.sessions.retain(|s| !s.is_expired(current_time));
        before - self.sessions.len()
    }

    /// Number of registered providers.
    pub fn provider_count(&self) -> usize {
        self.providers.len()
    }

    /// Number of active sessions.
    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }
}

impl Default for AuthProviderRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// AuthAuditLog
// ---------------------------------------------------------------------------

/// The kind of authentication event recorded in the audit log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthEventKind {
    /// A new session was created.
    SessionCreated,
    /// A session was removed / logged out.
    SessionRemoved,
    /// A token was refreshed.
    TokenRefreshed,
    /// An authentication attempt was denied.
    AuthDenied,
    /// A provider was registered.
    ProviderRegistered,
    /// A provider was unregistered.
    ProviderUnregistered,
}

impl fmt::Display for AuthEventKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::SessionCreated => "SessionCreated",
            Self::SessionRemoved => "SessionRemoved",
            Self::TokenRefreshed => "TokenRefreshed",
            Self::AuthDenied => "AuthDenied",
            Self::ProviderRegistered => "ProviderRegistered",
            Self::ProviderUnregistered => "ProviderUnregistered",
        };
        f.write_str(label)
    }
}

/// A single entry in the authentication audit log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthAuditEntry {
    pub timestamp: u64,
    pub kind: AuthEventKind,
    pub provider_id: String,
    pub session_id: Option<String>,
    pub detail: Option<String>,
}

/// Append-only audit log for authentication events.
#[derive(Debug, Clone, Default)]
pub struct AuthAuditLog {
    entries: Vec<AuthAuditEntry>,
}

impl AuthAuditLog {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Record an event in the audit log.
    pub fn record(
        &mut self,
        timestamp: u64,
        kind: AuthEventKind,
        provider_id: impl Into<String>,
        session_id: Option<String>,
        detail: Option<String>,
    ) {
        self.entries.push(AuthAuditEntry {
            timestamp,
            kind,
            provider_id: provider_id.into(),
            session_id,
            detail,
        });
    }

    /// Return all entries.
    pub fn entries(&self) -> &[AuthAuditEntry] {
        &self.entries
    }

    /// Return entries filtered by event kind.
    pub fn entries_by_kind(&self, kind: &AuthEventKind) -> Vec<&AuthAuditEntry> {
        self.entries.iter().filter(|e| &e.kind == kind).collect()
    }

    /// Return entries for a specific provider.
    pub fn entries_for_provider(&self, provider_id: &str) -> Vec<&AuthAuditEntry> {
        self.entries
            .iter()
            .filter(|e| e.provider_id == provider_id)
            .collect()
    }

    /// Return the total number of recorded events.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` if the log is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

// ---------------------------------------------------------------------------
// PermissionChecker
// ---------------------------------------------------------------------------

/// A simple permission checker that maps required scopes to actions.
#[derive(Debug, Clone)]
pub struct PermissionChecker {
    rules: Vec<(String, Vec<String>)>,
}

impl PermissionChecker {
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    /// Define the scopes required to perform `action`.
    pub fn add_rule(&mut self, action: impl Into<String>, required_scopes: Vec<String>) {
        self.rules.push((action.into(), required_scopes));
    }

    /// Check whether a session is allowed to perform the given action.
    pub fn is_allowed(&self, session: &AuthSession, action: &str) -> bool {
        match self.rules.iter().find(|(a, _)| a == action) {
            Some((_, required)) => required.iter().all(|s| session.has_scope(s)),
            // No rule means the action is unrestricted.
            None => true,
        }
    }

    /// Return all actions that the given session is permitted to perform.
    pub fn allowed_actions(&self, session: &AuthSession) -> Vec<&str> {
        self.rules
            .iter()
            .filter(|(_, required)| required.iter().all(|s| session.has_scope(s)))
            .map(|(action, _)| action.as_str())
            .collect()
    }

    /// Return the number of rules defined.
    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }
}

impl Default for PermissionChecker {
    fn default() -> Self {
        Self::new()
    }
}

/// Attempt to refresh an auth token. Returns a new session with updated token.
pub fn auth_token_refresh(
    session: &AuthSession,
    new_token: &str,
    new_expires_at: Option<u64>,
) -> Result<AuthSession, AuthError> {
    if session.refresh_token.is_none() {
        return Err(AuthError::SessionNotFound(format!(
            "no refresh token for session {}",
            session.session_id
        )));
    }
    Ok(AuthSession {
        session_id: session.session_id.clone(),
        provider_id: session.provider_id.clone(),
        access_token: new_token.to_string(),
        refresh_token: session.refresh_token.clone(),
        expires_at: new_expires_at,
        scopes: session.scopes.clone(),
    })
}


// ---------------------------------------------------------------------------
// TokenValidator - validates auth tokens
// ---------------------------------------------------------------------------

/// Validates authentication tokens.
#[derive(Debug, Clone)]
pub struct TokenValidator {
    pub token: String,
    pub issued_at: u64,
    pub expires_at: u64,
    pub scopes: Vec<String>,
}

impl TokenValidator {
    /// Create a new token validator.
    pub fn new(token: impl Into<String>, issued_at: u64, expires_at: u64) -> Self {
        Self {
            token: token.into(),
            issued_at,
            expires_at,
            scopes: Vec::new(),
        }
    }

    /// Add scopes to this token.
    pub fn with_scopes(mut self, scopes: Vec<String>) -> Self {
        self.scopes = scopes;
        self
    }

    /// Validate that the token is well-formed and not expired.
    pub fn validate(&self, current_time: u64) -> Result<(), String> {
        if self.token.is_empty() {
            return Err("token is empty".to_string());
        }
        if self.issued_at > self.expires_at {
            return Err("issued_at is after expires_at".to_string());
        }
        if self.is_expired(current_time) {
            return Err(format!(
                "token expired {} seconds ago",
                current_time.saturating_sub(self.expires_at)
            ));
        }
        Ok(())
    }

    /// Check if the token is expired at the given time.
    pub fn is_expired(&self, current_time: u64) -> bool {
        current_time >= self.expires_at
    }

    /// Remaining seconds until expiration, or 0 if already expired.
    pub fn remaining_seconds(&self, current_time: u64) -> u64 {
        self.expires_at.saturating_sub(current_time)
    }

    /// Duration for which this token is valid (total lifetime).
    pub fn lifetime(&self) -> u64 {
        self.expires_at.saturating_sub(self.issued_at)
    }

    /// Check if a specific scope is present.
    pub fn has_scope(&self, scope: &str) -> bool {
        self.scopes.iter().any(|s| s == scope)
    }
}

impl fmt::Display for TokenValidator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Token(lifetime={}s, scopes={})",
            self.lifetime(),
            self.scopes.len()
        )
    }
}

// ---------------------------------------------------------------------------
// AuthFlowState - state machine for authentication flows
// ---------------------------------------------------------------------------

/// Represents the state of an authentication flow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthFlowState {
    /// No authentication flow in progress.
    Idle,
    /// Waiting for the user to authenticate in a browser.
    AwaitingUserAuth,
    /// Exchanging an authorization code for tokens.
    ExchangingCode,
    /// Successfully authenticated.
    Authenticated,
    /// Authentication failed with an error message.
    Failed(String),
}

impl AuthFlowState {
    /// Returns true if this state is terminal (Authenticated or Failed).
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Authenticated | Self::Failed(_))
    }

    /// Returns true if the flow is in progress.
    pub fn is_in_progress(&self) -> bool {
        matches!(self, Self::AwaitingUserAuth | Self::ExchangingCode)
    }

    /// Returns true if the flow has not started.
    pub fn is_idle(&self) -> bool {
        matches!(self, Self::Idle)
    }

    /// Returns true if authentication succeeded.
    pub fn is_authenticated(&self) -> bool {
        matches!(self, Self::Authenticated)
    }

    /// Returns the error message if the flow failed.
    pub fn error_message(&self) -> Option<&str> {
        match self {
            Self::Failed(msg) => Some(msg),
            _ => None,
        }
    }
}

impl fmt::Display for AuthFlowState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Idle => write!(f, "Idle"),
            Self::AwaitingUserAuth => write!(f, "AwaitingUserAuth"),
            Self::ExchangingCode => write!(f, "ExchangingCode"),
            Self::Authenticated => write!(f, "Authenticated"),
            Self::Failed(msg) => write!(f, "Failed({msg})"),
        }
    }
}

// ---------------------------------------------------------------------------
// AuthFlowTracker - tracks multiple auth flows
// ---------------------------------------------------------------------------

/// Tracks authentication flows by provider.
#[derive(Debug, Clone, Default)]
pub struct AuthFlowTracker {
    flows: HashMap<String, AuthFlowState>,
}

impl AuthFlowTracker {
    /// Create a new empty flow tracker.
    pub fn new() -> Self {
        Self {
            flows: HashMap::new(),
        }
    }

    /// Set the state of a flow for the given provider.
    pub fn set_state(&mut self, provider_id: impl Into<String>, state: AuthFlowState) {
        self.flows.insert(provider_id.into(), state);
    }

    /// Get the current state for a provider.
    pub fn get_state(&self, provider_id: &str) -> Option<&AuthFlowState> {
        self.flows.get(provider_id)
    }

    /// Returns all providers with active (in-progress) flows.
    pub fn active_flows(&self) -> Vec<&str> {
        self.flows
            .iter()
            .filter(|(_, s)| s.is_in_progress())
            .map(|(id, _)| id.as_str())
            .collect()
    }

    /// Returns all providers that have completed authentication.
    pub fn authenticated_providers(&self) -> Vec<&str> {
        self.flows
            .iter()
            .filter(|(_, s)| s.is_authenticated())
            .map(|(id, _)| id.as_str())
            .collect()
    }

    /// Number of tracked flows.
    pub fn len(&self) -> usize {
        self.flows.len()
    }

    /// Returns true if no flows are being tracked.
    pub fn is_empty(&self) -> bool {
        self.flows.is_empty()
    }

    /// Remove all completed (terminal) flows.
    pub fn clear_completed(&mut self) {
        self.flows.retain(|_, s| !s.is_terminal());
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    fn github_provider() -> AuthProvider {
        AuthProvider {
            id: "github".to_string(),
            label: "GitHub".to_string(),
            supports_multiple_accounts: false,
            status: AuthProviderStatus::Registered,
        }
    }

    fn azure_provider() -> AuthProvider {
        AuthProvider {
            id: "azure".to_string(),
            label: "Azure AD".to_string(),
            supports_multiple_accounts: true,
            status: AuthProviderStatus::Active,
        }
    }

    #[test]
    fn register_and_get_provider() {
        let mut svc = AuthenticationService::new();
        svc.register_provider(github_provider());
        assert!(svc.get_provider("github").is_some());
        assert!(svc.get_provider("azure").is_none());
    }

    #[test]
    fn create_and_query_sessions() {
        let mut svc = AuthenticationService::new();
        svc.register_provider(github_provider());
        let id = svc.create_session("github", vec!["repo".to_string()]);
        assert!(svc.get_session(&id).is_some());
        assert_eq!(svc.get_sessions_for_provider("github").len(), 1);
        assert_eq!(svc.get_sessions_for_provider("azure").len(), 0);
    }

    #[test]
    fn remove_session() {
        let mut svc = AuthenticationService::new();
        let id = svc.create_session("github", vec![]);
        assert!(svc.remove_session(&id));
        assert!(!svc.remove_session(&id));
        assert!(svc.get_session(&id).is_none());
    }

    #[test]
    fn unregister_provider() {
        let mut svc = AuthenticationService::new();
        svc.register_provider(github_provider());
        assert!(svc.unregister_provider("github"));
        assert!(!svc.unregister_provider("github"));
        assert!(svc.get_provider("github").is_none());
    }

    #[test]
    fn provider_and_session_counts() {
        let mut svc = AuthenticationService::new();
        assert_eq!(svc.provider_count(), 0);
        assert_eq!(svc.session_count(), 0);

        svc.register_provider(github_provider());
        svc.register_provider(azure_provider());
        assert_eq!(svc.provider_count(), 2);

        svc.create_session("github", vec!["repo".to_string()]);
        svc.create_session("azure", vec!["openid".to_string()]);
        svc.create_session("github", vec!["gist".to_string()]);
        assert_eq!(svc.session_count(), 3);
    }

    #[test]
    fn get_all_providers_and_sessions() {
        let mut svc = AuthenticationService::new();
        svc.register_provider(github_provider());
        svc.register_provider(azure_provider());
        svc.create_session("github", vec!["repo".to_string()]);

        let providers = svc.get_all_providers();
        assert_eq!(providers.len(), 2);
        assert_eq!(providers[0].id, "github");
        assert_eq!(providers[1].id, "azure");

        let sessions = svc.get_all_sessions();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].provider_id, "github");
    }

    #[test]
    fn has_session_with_scopes() {
        let mut svc = AuthenticationService::new();
        svc.register_provider(github_provider());
        svc.create_session("github", vec!["repo".to_string(), "user".to_string()]);

        assert!(svc.has_session_with_scopes("github", &["repo"]));
        assert!(svc.has_session_with_scopes("github", &["repo", "user"]));
        assert!(!svc.has_session_with_scopes("github", &["repo", "admin"]));
        assert!(!svc.has_session_with_scopes("azure", &["repo"]));
    }

    #[test]
    fn session_has_scope() {
        let mut svc = AuthenticationService::new();
        let id = svc.create_session("github", vec!["repo".to_string(), "gist".to_string()]);
        let session = svc.get_session(&id).unwrap();
        assert!(session.has_scope("repo"));
        assert!(session.has_scope("gist"));
        assert!(!session.has_scope("admin"));
    }

    #[test]
    fn display_impls() {
        let provider = github_provider();
        assert_eq!(format!("{provider}"), "GitHub (github)");

        let session = AuthenticationSession {
            id: "s1".to_string(),
            provider_id: "github".to_string(),
            account_label: "user@github".to_string(),
            scopes: vec![],
        };
        assert_eq!(
            format!("{session}"),
            "Session s1 (provider=github, account=user@github)"
        );
    }

    #[test]
    fn auth_provider_status_equality() {
        assert_eq!(AuthProviderStatus::Registered, AuthProviderStatus::Registered);
        assert_ne!(AuthProviderStatus::Active, AuthProviderStatus::Disabled);

        let provider = azure_provider();
        assert_eq!(provider.status, AuthProviderStatus::Active);
    }

    // -- new tests -------------------------------------------------------------

    #[test]
    fn auth_error_display() {
        let err = AuthError::ProviderNotFound("okta".into());
        assert_eq!(err.to_string(), "provider 'okta' not found");

        let err2 = AuthError::InvalidProviderId("".into());
        assert_eq!(err2.to_string(), "invalid provider id: ''");
    }

    #[test]
    fn builder_valid() {
        let provider = AuthProviderBuilder::new("gitlab")
            .label("GitLab")
            .supports_multiple_accounts(true)
            .status(AuthProviderStatus::Active)
            .build()
            .unwrap();
        assert_eq!(provider.id, "gitlab");
        assert_eq!(provider.label, "GitLab");
        assert!(provider.supports_multiple_accounts);
        assert_eq!(provider.status, AuthProviderStatus::Active);
    }

    #[test]
    fn builder_invalid_id() {
        let result = AuthProviderBuilder::new("").build();
        assert_eq!(result, Err(AuthError::InvalidProviderId("".into())));

        let result2 = AuthProviderBuilder::new("has spaces").build();
        assert!(result2.is_err());
    }

    #[test]
    fn register_provider_checked_duplicate() {
        let mut svc = AuthenticationService::new();
        svc.register_provider_checked(github_provider()).unwrap();
        let err = svc.register_provider_checked(github_provider()).unwrap_err();
        assert_eq!(err, AuthError::ProviderAlreadyRegistered("github".into()));
    }

    #[test]
    fn create_session_checked_unknown_provider() {
        let mut svc = AuthenticationService::new();
        let err = svc
            .create_session_checked("nope", vec![])
            .unwrap_err();
        assert_eq!(err, AuthError::ProviderNotFound("nope".into()));
    }

    #[test]
    fn create_session_checked_disabled_provider() {
        let mut svc = AuthenticationService::new();
        let mut p = github_provider();
        p.status = AuthProviderStatus::Disabled;
        svc.register_provider(p);
        let err = svc
            .create_session_checked("github", vec![])
            .unwrap_err();
        assert_eq!(err, AuthError::ProviderNotActive("github".into()));
    }

    #[test]
    fn create_session_checked_multiple_accounts_rejected() {
        let mut svc = AuthenticationService::new();
        svc.register_provider(github_provider()); // supports_multiple_accounts = false
        svc.create_session_checked("github", vec!["repo".into()])
            .unwrap();
        let err = svc
            .create_session_checked("github", vec!["gist".into()])
            .unwrap_err();
        assert_eq!(
            err,
            AuthError::MultipleAccountsNotSupported("github".into())
        );
    }

    #[test]
    fn create_session_checked_multiple_accounts_allowed() {
        let mut svc = AuthenticationService::new();
        svc.register_provider(azure_provider()); // supports_multiple_accounts = true
        svc.create_session_checked("azure", vec!["openid".into()])
            .unwrap();
        svc.create_session_checked("azure", vec!["profile".into()])
            .unwrap();
        assert_eq!(svc.get_sessions_for_provider("azure").len(), 2);
    }

    #[test]
    fn remove_session_checked_not_found() {
        let mut svc = AuthenticationService::new();
        let err = svc.remove_session_checked("no-such").unwrap_err();
        assert_eq!(err, AuthError::SessionNotFound("no-such".into()));
    }

    #[test]
    fn set_provider_status_and_is_usable() {
        assert!(AuthProviderStatus::Registered.is_usable());
        assert!(AuthProviderStatus::Active.is_usable());
        assert!(!AuthProviderStatus::Disabled.is_usable());

        let mut svc = AuthenticationService::new();
        svc.register_provider(github_provider());
        svc.set_provider_status("github", AuthProviderStatus::Disabled)
            .unwrap();
        assert_eq!(
            svc.get_provider("github").unwrap().status,
            AuthProviderStatus::Disabled
        );
    }

    #[test]
    fn remove_sessions_for_provider() {
        let mut svc = AuthenticationService::new();
        svc.create_session("github", vec!["repo".into()]);
        svc.create_session("github", vec!["gist".into()]);
        svc.create_session("azure", vec!["openid".into()]);
        assert_eq!(svc.remove_sessions_for_provider("github"), 2);
        assert_eq!(svc.session_count(), 1);
    }

    #[test]
    fn session_has_all_scopes() {
        let session = AuthenticationSession {
            id: "s1".into(),
            provider_id: "gh".into(),
            account_label: "me".into(),
            scopes: vec!["repo".into(), "user".into(), "gist".into()],
        };
        assert!(session.has_all_scopes(&["repo", "gist"]));
        assert!(!session.has_all_scopes(&["repo", "admin"]));
        assert!(session.has_all_scopes(&[]));
        assert_eq!(session.scope_count(), 3);
    }

    #[test]
    fn auth_provider_status_display() {
        assert_eq!(AuthProviderStatus::Registered.to_string(), "Registered");
        assert_eq!(AuthProviderStatus::Active.to_string(), "Active");
        assert_eq!(AuthProviderStatus::Disabled.to_string(), "Disabled");
    }

    #[test]
    fn wb_auth_stats_new_defaults() {
        let stats = WbAuthStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn wb_auth_stats_record_success() {
        let mut stats = WbAuthStats::new();
        stats.record_success(100);
        stats.record_success(200);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.successful_operations, 2);
        assert_eq!(stats.failed_operations, 0);
        assert_eq!(stats.average_time_ns(), 150);
        assert_eq!(stats.min_time_ns(), Some(100));
        assert_eq!(stats.max_time_ns(), Some(200));
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn wb_auth_stats_record_failure() {
        let mut stats = WbAuthStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn wb_auth_stats_reset() {
        let mut stats = WbAuthStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn wb_auth_stats_merge() {
        let mut a = WbAuthStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = WbAuthStats::new();
        b.record_failure(50);
        b.record_success(400);
        a.merge(&b);
        assert_eq!(a.total(), 4);
        assert_eq!(a.successful_operations, 3);
        assert_eq!(a.failed_operations, 1);
        assert_eq!(a.min_time_ns(), Some(50));
        assert_eq!(a.max_time_ns(), Some(400));
    }

    #[test]
    fn wb_auth_stats_display() {
        let mut stats = WbAuthStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn wb_auth_stats_default() {
        let stats = WbAuthStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn wb_auth_validator_accepts_valid_name() {
        let v = WbAuthValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn wb_auth_validator_rejects_empty() {
        let v = WbAuthValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn wb_auth_validator_rejects_too_long() {
        let v = WbAuthValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn wb_auth_validator_forbidden_prefix() {
        let v = WbAuthValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn wb_auth_validator_allowed_chars() {
        let v = WbAuthValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn wb_auth_validator_range() {
        let v = WbAuthValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn wb_auth_sanitize_removes_control() {
        let result = WbAuthValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn wb_auth_truncate_short_string() {
        assert_eq!(WbAuthValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn wb_auth_truncate_long_string() {
        let result = WbAuthValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn wb_auth_is_ascii_printable() {
        assert!(WbAuthValidator::is_ascii_printable("Hello World 123"));
        assert!(!WbAuthValidator::is_ascii_printable("Hello\x00World"));
    }

    #[test]
    fn auth_session_basic() {
        let session = AuthSession::new("s1", "github", "token123")
            .with_expires_at(1000)
            .with_scopes(vec!["read".into(), "write".into()]);
        assert!(!session.is_expired(500));
        assert!(session.is_expired(1000));
        assert!(session.has_scope("read"));
        assert!(!session.has_scope("admin"));
    }

    #[test]
    fn auth_session_time_remaining() {
        let session = AuthSession::new("s1", "github", "tok").with_expires_at(1000);
        assert_eq!(session.time_remaining(800), Some(200));
        assert_eq!(session.time_remaining(1200), Some(0));
    }

    #[test]
    fn auth_session_no_expiry() {
        let session = AuthSession::new("s1", "github", "tok");
        assert!(!session.is_expired(9999));
        assert_eq!(session.time_remaining(100), None);
    }

    #[test]
    fn auth_session_display() {
        let session = AuthSession::new("s1", "github", "tok");
        assert!(session.to_string().contains("s1"));
        assert!(session.to_string().contains("github"));
    }

    #[test]
    fn auth_provider_registry_register() {
        let mut reg = AuthProviderRegistry::new();
        let provider = AuthProviderBuilder::new("github")
            .label("GitHub")
            .build()
            .unwrap();
        assert!(reg.register(provider.clone()).is_ok());
        assert!(reg.register(provider).is_err()); // duplicate
        assert_eq!(reg.provider_count(), 1);
    }

    #[test]
    fn auth_provider_registry_sessions() {
        let mut reg = AuthProviderRegistry::new();
        reg.add_session(AuthSession::new("s1", "github", "tok1"));
        reg.add_session(AuthSession::new("s2", "azure", "tok2"));
        reg.add_session(AuthSession::new("s3", "github", "tok3"));
        let gh = reg.sessions_for_provider("github");
        assert_eq!(gh.len(), 2);
    }

    #[test]
    fn auth_provider_registry_remove_expired() {
        let mut reg = AuthProviderRegistry::new();
        reg.add_session(AuthSession::new("s1", "gh", "t1").with_expires_at(100));
        reg.add_session(AuthSession::new("s2", "gh", "t2").with_expires_at(200));
        let removed = reg.remove_expired(150);
        assert_eq!(removed, 1);
        assert_eq!(reg.session_count(), 1);
    }

    #[test]
    fn auth_token_refresh_works() {
        let session =
            AuthSession::new("s1", "github", "old_token").with_refresh_token("refresh_tok");
        let refreshed = auth_token_refresh(&session, "new_token", Some(2000)).unwrap();
        assert_eq!(refreshed.access_token, "new_token");
        assert_eq!(refreshed.expires_at, Some(2000));
        assert_eq!(refreshed.session_id, "s1");
    }

    // -- AuthAuditLog tests ---------------------------------------------------

    #[test]
    fn audit_log_record_and_query() {
        let mut log = AuthAuditLog::new();
        assert!(log.is_empty());

        log.record(100, AuthEventKind::SessionCreated, "github", Some("s1".into()), None);
        log.record(200, AuthEventKind::TokenRefreshed, "github", Some("s1".into()), None);
        log.record(300, AuthEventKind::AuthDenied, "azure", None, Some("bad creds".into()));

        assert_eq!(log.len(), 3);
        assert!(!log.is_empty());
        assert_eq!(log.entries_by_kind(&AuthEventKind::AuthDenied).len(), 1);
        assert_eq!(log.entries_for_provider("github").len(), 2);
        assert_eq!(log.entries_for_provider("azure").len(), 1);
    }

    #[test]
    fn audit_log_clear() {
        let mut log = AuthAuditLog::new();
        log.record(1, AuthEventKind::ProviderRegistered, "gh", None, None);
        log.record(2, AuthEventKind::ProviderUnregistered, "gh", None, None);
        assert_eq!(log.len(), 2);
        log.clear();
        assert!(log.is_empty());
    }

    #[test]
    fn audit_event_kind_display() {
        assert_eq!(AuthEventKind::SessionCreated.to_string(), "SessionCreated");
        assert_eq!(AuthEventKind::AuthDenied.to_string(), "AuthDenied");
        assert_eq!(AuthEventKind::TokenRefreshed.to_string(), "TokenRefreshed");
        assert_eq!(AuthEventKind::ProviderRegistered.to_string(), "ProviderRegistered");
    }

    // -- PermissionChecker tests ----------------------------------------------

    #[test]
    fn permission_checker_allows_when_scopes_match() {
        let mut checker = PermissionChecker::new();
        checker.add_rule("read_repo", vec!["repo".into()]);
        checker.add_rule("admin", vec!["repo".into(), "admin".into()]);

        let session = AuthSession::new("s1", "gh", "tok")
            .with_scopes(vec!["repo".into(), "admin".into()]);
        assert!(checker.is_allowed(&session, "read_repo"));
        assert!(checker.is_allowed(&session, "admin"));
        assert_eq!(checker.allowed_actions(&session).len(), 2);
    }

    #[test]
    fn permission_checker_denies_missing_scope() {
        let mut checker = PermissionChecker::new();
        checker.add_rule("admin", vec!["admin".into()]);

        let session = AuthSession::new("s1", "gh", "tok")
            .with_scopes(vec!["repo".into()]);
        assert!(!checker.is_allowed(&session, "admin"));
        assert!(checker.allowed_actions(&session).is_empty());
        assert_eq!(checker.rule_count(), 1);
    }

    #[test]
    fn permission_checker_unknown_action_is_unrestricted() {
        let checker = PermissionChecker::new();
        let session = AuthSession::new("s1", "gh", "tok");
        assert!(checker.is_allowed(&session, "anything"));
    }

    #[test]
    fn token_validator_valid() {
        let tv = TokenValidator::new("tok123", 1000, 2000)
            .with_scopes(vec!["read".into(), "write".into()]);
        assert!(tv.validate(1500).is_ok());
        assert!(!tv.is_expired(1500));
        assert_eq!(tv.remaining_seconds(1500), 500);
        assert_eq!(tv.lifetime(), 1000);
        assert!(tv.has_scope("read"));
        assert!(!tv.has_scope("admin"));
    }

    #[test]
    fn token_validator_expired() {
        let tv = TokenValidator::new("tok", 100, 200);
        assert!(tv.is_expired(300));
        assert_eq!(tv.remaining_seconds(300), 0);
        let err = tv.validate(300).unwrap_err();
        assert!(err.contains("expired"));
    }

    #[test]
    fn token_validator_empty_token() {
        let tv = TokenValidator::new("", 100, 200);
        let err = tv.validate(150).unwrap_err();
        assert!(err.contains("empty"));
    }

    #[test]
    fn auth_flow_state_transitions() {
        let idle = AuthFlowState::Idle;
        assert!(idle.is_idle());
        assert!(!idle.is_terminal());

        let awaiting = AuthFlowState::AwaitingUserAuth;
        assert!(awaiting.is_in_progress());

        let auth = AuthFlowState::Authenticated;
        assert!(auth.is_terminal());
        assert!(auth.is_authenticated());

        let failed = AuthFlowState::Failed("timeout".into());
        assert!(failed.is_terminal());
        assert_eq!(failed.error_message(), Some("timeout"));
        let s = format!("{failed}");
        assert!(s.contains("timeout"));
    }

    #[test]
    fn auth_flow_tracker_operations() {
        let mut tracker = AuthFlowTracker::new();
        assert!(tracker.is_empty());
        tracker.set_state("github", AuthFlowState::AwaitingUserAuth);
        tracker.set_state("gitlab", AuthFlowState::Authenticated);
        assert_eq!(tracker.len(), 2);
        assert_eq!(tracker.active_flows(), vec!["github"]);
        assert_eq!(tracker.authenticated_providers(), vec!["gitlab"]);
        tracker.clear_completed();
        assert_eq!(tracker.len(), 1);
        assert!(tracker.get_state("github").is_some());
    }

    #[test]
    fn auth_flow_tracker_clear_completed() {
        let mut tracker = AuthFlowTracker::new();
        tracker.set_state("a", AuthFlowState::Failed("err".into()));
        tracker.set_state("b", AuthFlowState::ExchangingCode);
        tracker.clear_completed();
        assert_eq!(tracker.len(), 1);
        assert!(tracker.get_state("b").unwrap().is_in_progress());
    }
}
