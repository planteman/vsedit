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

// ---------------------------------------------------------------------------
// Session scope operations
// ---------------------------------------------------------------------------

impl AuthenticationSession {
    pub fn scopes_display(&self) -> String { self.scopes.join(", ") }

    pub fn missing_scopes(&self, required: &[&str]) -> Vec<String> {
        required.iter().filter(|s| !self.has_scope(s)).map(|s| s.to_string()).collect()
    }

    pub fn has_any_scope(&self, scopes: &[&str]) -> bool {
        scopes.iter().any(|s| self.has_scope(s))
    }
}

impl AuthenticationService {
    pub fn providers_with_sessions(&self) -> Vec<String> {
        let mut ids: Vec<String> = self.sessions.iter().map(|s| s.provider_id.clone()).collect();
        ids.sort(); ids.dedup(); ids
    }

    pub fn total_unique_scopes(&self) -> usize {
        let mut scopes: Vec<&str> = self.sessions.iter().flat_map(|s| s.scopes.iter().map(String::as_str)).collect();
        scopes.sort(); scopes.dedup(); scopes.len()
    }

    pub fn sessions_with_scope(&self, scope: &str) -> Vec<&AuthenticationSession> {
        self.sessions.iter().filter(|s| s.has_scope(scope)).collect()
    }

    pub fn clear_provider_sessions(&mut self, provider_id: &str) -> usize {
        let before = self.sessions.len();
        self.sessions.retain(|s| s.provider_id != provider_id);
        before - self.sessions.len()
    }

    pub fn summary(&self) -> String {
        format!("{} provider(s), {} session(s)", self.providers.len(), self.sessions.len())
    }
}

pub fn active_providers(providers: &[AuthProvider]) -> Vec<&AuthProvider> {
    providers.iter().filter(|p| p.status == AuthProviderStatus::Active).collect()
}

pub fn multi_account_providers(providers: &[AuthProvider]) -> Vec<&AuthProvider> {
    providers.iter().filter(|p| p.supports_multiple_accounts).collect()
}

// ---------------------------------------------------------------------------
// AuthSessionManager – token refresh
// ---------------------------------------------------------------------------

/// Manages authentication sessions with token refresh tracking.
pub struct AuthSessionManager {
    sessions: HashMap<String, ManagedSession>,
}

/// A session with refresh metadata.
#[derive(Debug, Clone)]
pub struct ManagedSession {
    pub session_id: String,
    pub provider_id: String,
    pub access_token: String,
    pub expires_at_ms: Option<u64>,
    pub refresh_count: u32,
}

impl AuthSessionManager {
    /// Create a new session manager.
    pub fn new() -> Self {
        Self { sessions: HashMap::new() }
    }

    /// Store a session.
    pub fn store(&mut self, session: ManagedSession) {
        self.sessions.insert(session.session_id.clone(), session);
    }

    /// Check if a session's token has expired given the current time.
    pub fn is_expired(&self, session_id: &str, now_ms: u64) -> bool {
        self.sessions.get(session_id)
            .and_then(|s| s.expires_at_ms)
            .map(|exp| now_ms >= exp)
            .unwrap_or(false)
    }

    /// Refresh a session's token. Returns the old token.
    pub fn refresh_token(&mut self, session_id: &str, new_token: String, new_expires_at: Option<u64>) -> Option<String> {
        if let Some(session) = self.sessions.get_mut(session_id) {
            let old = std::mem::replace(&mut session.access_token, new_token);
            session.expires_at_ms = new_expires_at;
            session.refresh_count += 1;
            Some(old)
        } else {
            None
        }
    }

    /// Get the number of times a session has been refreshed.
    pub fn refresh_count(&self, session_id: &str) -> u32 {
        self.sessions.get(session_id).map(|s| s.refresh_count).unwrap_or(0)
    }

    /// List sessions that will expire before the given deadline.
    pub fn expiring_before(&self, deadline_ms: u64) -> Vec<&ManagedSession> {
        self.sessions.values()
            .filter(|s| s.expires_at_ms.map(|e| e < deadline_ms).unwrap_or(false))
            .collect()
    }

    /// Total number of managed sessions.
    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }
}

// ---------------------------------------------------------------------------
// OAuthFlowHandler – browser-based auth flow
// ---------------------------------------------------------------------------

/// State of an OAuth flow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OAuthFlowState {
    /// Waiting for user to open the browser.
    Pending,
    /// Browser opened, waiting for callback.
    AwaitingCallback,
    /// Flow completed successfully.
    Completed,
    /// Flow failed or was cancelled.
    Failed,
}

/// Handles an OAuth browser-based authentication flow.
pub struct OAuthFlowHandler {
    pub client_id: String,
    pub redirect_uri: String,
    pub scopes: Vec<String>,
    state: OAuthFlowState,
    authorization_code: Option<String>,
}

impl OAuthFlowHandler {
    /// Create a new OAuth flow handler.
    pub fn new(client_id: impl Into<String>, redirect_uri: impl Into<String>, scopes: Vec<String>) -> Self {
        Self {
            client_id: client_id.into(),
            redirect_uri: redirect_uri.into(),
            scopes,
            state: OAuthFlowState::Pending,
            authorization_code: None,
        }
    }

    /// Build the authorization URL.
    pub fn authorization_url(&self) -> String {
        let scopes = self.scopes.join(" ");
        format!(
            "https://auth.example.com/authorize?client_id={}&redirect_uri={}&scope={}&response_type=code",
            self.client_id, self.redirect_uri, scopes
        )
    }

    /// Mark the flow as awaiting callback (browser opened).
    pub fn mark_browser_opened(&mut self) {
        self.state = OAuthFlowState::AwaitingCallback;
    }

    /// Handle callback with authorization code.
    pub fn handle_callback(&mut self, code: String) {
        self.authorization_code = Some(code);
        self.state = OAuthFlowState::Completed;
    }

    /// Mark the flow as failed.
    pub fn fail(&mut self) {
        self.state = OAuthFlowState::Failed;
    }

    /// Current flow state.
    pub fn state(&self) -> OAuthFlowState {
        self.state
    }

    /// Get the authorization code (only after completion).
    pub fn authorization_code(&self) -> Option<&str> {
        self.authorization_code.as_deref()
    }
}

// ---------------------------------------------------------------------------
// AuthProviderChain – fallback auth
// ---------------------------------------------------------------------------

/// Chains multiple auth providers for fallback authentication.
pub struct AuthProviderChain {
    provider_ids: Vec<String>,
}

impl AuthProviderChain {
    /// Create a chain with providers in priority order.
    pub fn new(provider_ids: Vec<String>) -> Self {
        Self { provider_ids }
    }

    /// Find the first provider that has an active session in the service.
    pub fn first_available<'a>(&self, service: &'a AuthenticationService) -> Option<&'a AuthProvider> {
        for pid in &self.provider_ids {
            if let Some(provider) = service.get_provider(pid) {
                if provider.status.is_usable() {
                    return Some(provider);
                }
            }
        }
        None
    }

    /// Get the number of providers in the chain.
    pub fn len(&self) -> usize {
        self.provider_ids.len()
    }

    /// Whether the chain is empty.
    pub fn is_empty(&self) -> bool {
        self.provider_ids.is_empty()
    }

    /// Get provider IDs.
    pub fn provider_ids(&self) -> &[String] {
        &self.provider_ids
    }
}

// ---------------------------------------------------------------------------
// CredentialEncryptionWrapper – simple XOR-based obfuscation
// ---------------------------------------------------------------------------

/// Wraps credential strings with simple XOR obfuscation for in-memory storage.
///
/// NOTE: This is NOT cryptographic encryption, just obfuscation to prevent
/// accidental exposure in memory dumps.
pub struct CredentialEncryptionWrapper {
    key: u8,
}

impl CredentialEncryptionWrapper {
    /// Create a wrapper with the given XOR key.
    pub fn new(key: u8) -> Self {
        Self { key }
    }

    /// Obfuscate a credential string.
    pub fn encrypt(&self, plaintext: &str) -> Vec<u8> {
        plaintext.as_bytes().iter().map(|b| b ^ self.key).collect()
    }

    /// De-obfuscate a credential string.
    pub fn decrypt(&self, ciphertext: &[u8]) -> String {
        let bytes: Vec<u8> = ciphertext.iter().map(|b| b ^ self.key).collect();
        String::from_utf8_lossy(&bytes).to_string()
    }
}


// ---------------------------------------------------------------------------
// AuthSessionListView — display a list of auth sessions
// ---------------------------------------------------------------------------

/// Display formatting for a list of authentication sessions.
#[derive(Debug, Clone)]
pub struct AuthSessionListView {
    sessions: Vec<AuthenticationSession>,
    show_scopes: bool,
    show_provider: bool,
}

impl AuthSessionListView {
    pub fn new(sessions: Vec<AuthenticationSession>) -> Self {
        Self {
            sessions,
            show_scopes: true,
            show_provider: true,
        }
    }

    /// Set whether scopes are shown.
    pub fn with_scopes(mut self, show: bool) -> Self {
        self.show_scopes = show;
        self
    }

    /// Set whether the provider id is shown.
    pub fn with_provider(mut self, show: bool) -> Self {
        self.show_provider = show;
        self
    }

    /// Number of sessions in the list.
    pub fn len(&self) -> usize {
        self.sessions.len()
    }

    /// Whether the list is empty.
    pub fn is_empty(&self) -> bool {
        self.sessions.is_empty()
    }

    /// Render a single session to a display string.
    pub fn render_session(&self, session: &AuthenticationSession) -> String {
        let mut parts = Vec::new();
        parts.push(format!("id={}", session.id));
        if self.show_provider {
            parts.push(format!("provider={}", session.provider_id));
        }
        parts.push(format!("account={}", session.account_label));
        if self.show_scopes && !session.scopes.is_empty() {
            parts.push(format!("scopes=[{}]", session.scopes.join(", ")));
        }
        parts.join(" | ")
    }

    /// Render all sessions to display strings.
    pub fn render_all(&self) -> Vec<String> {
        self.sessions.iter().map(|s| self.render_session(s)).collect()
    }

    /// Filter sessions that include a specific scope.
    pub fn filter_by_scope(&self, scope: &str) -> Vec<&AuthenticationSession> {
        self.sessions.iter().filter(|s| s.has_scope(scope)).collect()
    }

    /// Filter sessions by provider id.
    pub fn filter_by_provider(&self, provider_id: &str) -> Vec<&AuthenticationSession> {
        self.sessions.iter().filter(|s| s.provider_id == provider_id).collect()
    }

    /// Get a session by id.
    pub fn get(&self, id: &str) -> Option<&AuthenticationSession> {
        self.sessions.iter().find(|s| s.id == id)
    }

    /// Return unique provider ids across all sessions.
    pub fn provider_ids(&self) -> Vec<&str> {
        let mut ids: Vec<&str> = self.sessions.iter().map(|s| s.provider_id.as_str()).collect();
        ids.sort();
        ids.dedup();
        ids
    }
}

impl fmt::Display for AuthSessionListView {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Sessions ({}):", self.sessions.len())?;
        for line in self.render_all() {
            writeln!(f, "  {}", line)?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// AuthTokenInspector — inspect token claims
// ---------------------------------------------------------------------------

/// A single claim inside a token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenClaim {
    pub key: String,
    pub value: String,
}

impl TokenClaim {
    pub fn new(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self { key: key.into(), value: value.into() }
    }
}

impl fmt::Display for TokenClaim {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}={}", self.key, self.value)
    }
}

/// Inspects a token and extracts claims.
///
/// This is a simplified mock inspector — real JWT parsing would use a
/// dedicated library. Here we parse a `key=value;key=value` format.
#[derive(Debug, Clone)]
pub struct AuthTokenInspector {
    claims: Vec<TokenClaim>,
    raw: String,
}

impl AuthTokenInspector {
    /// Parse a simplified token string of the form `key=val;key=val`.
    pub fn parse(token: &str) -> Self {
        let claims: Vec<TokenClaim> = token
            .split(';')
            .filter_map(|part| {
                let mut kv = part.splitn(2, '=');
                match (kv.next(), kv.next()) {
                    (Some(k), Some(v)) if !k.trim().is_empty() => {
                        Some(TokenClaim::new(k.trim(), v.trim()))
                    }
                    _ => None,
                }
            })
            .collect();
        Self { claims, raw: token.to_string() }
    }

    /// The raw token string.
    pub fn raw(&self) -> &str {
        &self.raw
    }

    /// All parsed claims.
    pub fn claims(&self) -> &[TokenClaim] {
        &self.claims
    }

    /// Get the value of a specific claim.
    pub fn get_claim(&self, key: &str) -> Option<&str> {
        self.claims.iter().find(|c| c.key == key).map(|c| c.value.as_str())
    }

    /// Whether a claim with the given key exists.
    pub fn has_claim(&self, key: &str) -> bool {
        self.claims.iter().any(|c| c.key == key)
    }

    /// Number of claims.
    pub fn claim_count(&self) -> usize {
        self.claims.len()
    }

    /// Render all claims to a human-readable string.
    pub fn render_claims(&self) -> String {
        self.claims
            .iter()
            .map(|c| c.to_string())
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// Get the "sub" (subject) claim if present.
    pub fn subject(&self) -> Option<&str> {
        self.get_claim("sub")
    }

    /// Get the "iss" (issuer) claim if present.
    pub fn issuer(&self) -> Option<&str> {
        self.get_claim("iss")
    }

    /// Get the "exp" (expiry) claim as a u64 timestamp if present and parsable.
    pub fn expiry_timestamp(&self) -> Option<u64> {
        self.get_claim("exp").and_then(|v| v.parse().ok())
    }
}

// ---------------------------------------------------------------------------
// AuthScopeDisplay — display scopes in a readable format
// ---------------------------------------------------------------------------

/// A parsed scope with optional resource and permission parts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedScope {
    pub full: String,
    pub resource: String,
    pub permission: Option<String>,
}

impl ParsedScope {
    /// Parse a scope string. Format: `resource:permission` or just `resource`.
    pub fn parse(scope: &str) -> Self {
        let parts: Vec<&str> = scope.splitn(2, ':').collect();
        Self {
            full: scope.to_string(),
            resource: parts[0].to_string(),
            permission: parts.get(1).map(|s| s.to_string()),
        }
    }

    /// Whether this scope grants write permission (permission contains "write").
    pub fn is_write(&self) -> bool {
        self.permission.as_deref().map_or(false, |p| p.contains("write"))
    }

    /// Whether this scope grants read permission.
    pub fn is_read(&self) -> bool {
        self.permission.as_deref().map_or(false, |p| p.contains("read"))
    }

    /// Whether this scope is a wildcard (resource is "*" or permission is "*").
    pub fn is_wildcard(&self) -> bool {
        self.resource == "*" || self.permission.as_deref() == Some("*")
    }
}

impl fmt::Display for ParsedScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.full)
    }
}

/// Display helper for a collection of scopes.
#[derive(Debug, Clone)]
pub struct AuthScopeDisplay {
    scopes: Vec<ParsedScope>,
}

impl AuthScopeDisplay {
    /// Parse a list of scope strings.
    pub fn new(scopes: &[String]) -> Self {
        Self {
            scopes: scopes.iter().map(|s| ParsedScope::parse(s)).collect(),
        }
    }

    /// All parsed scopes.
    pub fn scopes(&self) -> &[ParsedScope] {
        &self.scopes
    }

    /// Number of scopes.
    pub fn len(&self) -> usize {
        self.scopes.len()
    }

    /// Whether the scope list is empty.
    pub fn is_empty(&self) -> bool {
        self.scopes.is_empty()
    }

    /// Unique resources across all scopes.
    pub fn resources(&self) -> Vec<&str> {
        let mut res: Vec<&str> = self.scopes.iter().map(|s| s.resource.as_str()).collect();
        res.sort();
        res.dedup();
        res
    }

    /// Scopes that grant write access.
    pub fn write_scopes(&self) -> Vec<&ParsedScope> {
        self.scopes.iter().filter(|s| s.is_write()).collect()
    }

    /// Scopes that are wildcards.
    pub fn wildcard_scopes(&self) -> Vec<&ParsedScope> {
        self.scopes.iter().filter(|s| s.is_wildcard()).collect()
    }

    /// Render as a comma-separated list of full scopes.
    pub fn render(&self) -> String {
        self.scopes.iter().map(|s| s.full.as_str()).collect::<Vec<_>>().join(", ")
    }
}

impl fmt::Display for AuthScopeDisplay {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}]", self.render())
    }
}

// ---------------------------------------------------------------------------
// AuthExpiryWarning — warn when tokens are about to expire
// ---------------------------------------------------------------------------

/// The urgency level of an expiry warning.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ExpiryUrgency {
    /// More than 1 hour remaining.
    Ok,
    /// Less than 1 hour remaining.
    Warning,
    /// Less than 10 minutes remaining.
    Critical,
    /// Already expired.
    Expired,
}

impl fmt::Display for ExpiryUrgency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ok => write!(f, "ok"),
            Self::Warning => write!(f, "warning"),
            Self::Critical => write!(f, "critical"),
            Self::Expired => write!(f, "expired"),
        }
    }
}

/// Tracks token expiry and generates warnings.
#[derive(Debug, Clone)]
pub struct AuthExpiryWarning {
    pub session_id: String,
    pub expiry_epoch_secs: u64,
}

impl AuthExpiryWarning {
    pub fn new(session_id: impl Into<String>, expiry_epoch_secs: u64) -> Self {
        Self {
            session_id: session_id.into(),
            expiry_epoch_secs,
        }
    }

    /// Compute the urgency given the current time as epoch seconds.
    pub fn urgency(&self, now_epoch_secs: u64) -> ExpiryUrgency {
        if now_epoch_secs >= self.expiry_epoch_secs {
            ExpiryUrgency::Expired
        } else {
            let remaining = self.expiry_epoch_secs - now_epoch_secs;
            if remaining < 600 {
                ExpiryUrgency::Critical
            } else if remaining < 3600 {
                ExpiryUrgency::Warning
            } else {
                ExpiryUrgency::Ok
            }
        }
    }

    /// Remaining seconds until expiry (0 if already expired).
    pub fn remaining_secs(&self, now_epoch_secs: u64) -> u64 {
        self.expiry_epoch_secs.saturating_sub(now_epoch_secs)
    }

    /// Human-readable remaining time.
    pub fn remaining_display(&self, now_epoch_secs: u64) -> String {
        let secs = self.remaining_secs(now_epoch_secs);
        if secs == 0 {
            return "expired".to_string();
        }
        let hours = secs / 3600;
        let minutes = (secs % 3600) / 60;
        if hours > 0 {
            format!("{}h {}m", hours, minutes)
        } else {
            format!("{}m", minutes)
        }
    }

    /// Whether the token has expired.
    pub fn is_expired(&self, now_epoch_secs: u64) -> bool {
        now_epoch_secs >= self.expiry_epoch_secs
    }

    /// Whether a warning should be shown.
    pub fn should_warn(&self, now_epoch_secs: u64) -> bool {
        matches!(self.urgency(now_epoch_secs), ExpiryUrgency::Warning | ExpiryUrgency::Critical)
    }
}

impl fmt::Display for AuthExpiryWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "session {} expires at {}", self.session_id, self.expiry_epoch_secs)
    }
}


/// Workbench auth configuration manager.
#[derive(Debug, Clone)]
pub struct WbAuthConfig {
    entries: Vec<WbAuthEntry>,
    enabled: bool,
    max_entries: usize,
}

/// A single workbench auth entry.
#[derive(Debug, Clone, PartialEq)]
pub struct WbAuthEntry {
    pub id: String,
    pub label: String,
    pub priority: i32,
    pub active: bool,
    pub metadata: Vec<(String, String)>,
}

impl WbAuthEntry {
    pub fn new(id: &str, label: &str) -> Self {
        Self {
            id: id.to_string(),
            label: label.to_string(),
            priority: 0,
            active: true,
            metadata: Vec::new(),
        }
    }

    pub fn with_priority(mut self, p: i32) -> Self {
        self.priority = p;
        self
    }

    pub fn with_meta(mut self, key: &str, val: &str) -> Self {
        self.metadata.push((key.to_string(), val.to_string()));
        self
    }

    pub fn get_meta(&self, key: &str) -> Option<&str> {
        self.metadata.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }

    pub fn deactivate(&mut self) {
        self.active = false;
    }

    pub fn activate(&mut self) {
        self.active = true;
    }

    pub fn has_meta(&self, key: &str) -> bool {
        self.metadata.iter().any(|(k, _)| k == key)
    }

    pub fn meta_count(&self) -> usize {
        self.metadata.len()
    }

    pub fn remove_meta(&mut self, key: &str) -> bool {
        let len = self.metadata.len();
        self.metadata.retain(|(k, _)| k != key);
        self.metadata.len() < len
    }
}

impl WbAuthConfig {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            enabled: true,
            max_entries,
        }
    }

    pub fn add(&mut self, entry: WbAuthEntry) -> bool {
        if self.entries.len() >= self.max_entries {
            return false;
        }
        self.entries.push(entry);
        self.entries.sort_by(|a, b| b.priority.cmp(&a.priority));
        true
    }

    pub fn remove(&mut self, id: &str) -> bool {
        let len = self.entries.len();
        self.entries.retain(|e| e.id != id);
        self.entries.len() < len
    }

    pub fn get(&self, id: &str) -> Option<&WbAuthEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut WbAuthEntry> {
        self.entries.iter_mut().find(|e| e.id == id)
    }

    pub fn active_entries(&self) -> Vec<&WbAuthEntry> {
        self.entries.iter().filter(|e| e.active).collect()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn is_full(&self) -> bool {
        self.entries.len() >= self.max_entries
    }

    pub fn enable(&mut self) {
        self.enabled = true;
    }

    pub fn disable(&mut self) {
        self.enabled = false;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn ids(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.id.as_str()).collect()
    }

    pub fn top_n(&self, n: usize) -> Vec<&WbAuthEntry> {
        self.entries.iter().take(n).collect()
    }

    pub fn find_by_label(&self, label: &str) -> Option<&WbAuthEntry> {
        self.entries.iter().find(|e| e.label == label)
    }

    pub fn deactivate_all(&mut self) {
        for e in &mut self.entries {
            e.active = false;
        }
    }

    pub fn activate_all(&mut self) {
        for e in &mut self.entries {
            e.active = true;
        }
    }

    pub fn count_active(&self) -> usize {
        self.entries.iter().filter(|e| e.active).count()
    }

    pub fn highest_priority(&self) -> Option<i32> {
        self.entries.first().map(|e| e.priority)
    }

    pub fn contains(&self, id: &str) -> bool {
        self.entries.iter().any(|e| e.id == id)
    }

    pub fn labels(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.label.as_str()).collect()
    }

    pub fn reorder_by_label(&mut self) {
        self.entries.sort_by(|a, b| a.label.cmp(&b.label));
    }

    pub fn drain_inactive(&mut self) -> Vec<WbAuthEntry> {
        let (inactive, active): (Vec<_>, Vec<_>) =
            self.entries.drain(..).partition(|e| !e.active);
        self.entries = active;
        inactive
    }
}


// ---------------------------------------------------------------------------
// Workbench authentication flow — extended utilities (xo)
// ---------------------------------------------------------------------------

/// Metric accumulator for wb_auth operations.
#[derive(Debug, Clone)]
pub struct XoMetrics {
    samples: Vec<f64>,
    label: String,
}

impl XoMetrics {
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

/// Sliding-window rate counter for wb_auth.
#[derive(Debug, Clone)]
pub struct XoRateWindow {
    timestamps: Vec<u64>,
    window_ms: u64,
}

impl XoRateWindow {
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

/// A small LRU-style cache for wb_auth lookups.
#[derive(Debug, Clone)]
pub struct XoLruCache {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl XoLruCache {
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
// xb_ utilities – batch 34
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer34 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer34 {
    /// Create a new ring buffer with the given capacity.
    pub fn new(cap: usize) -> Self {
        Self {
            buf: vec![0i64; cap],
            cap,
            head: 0,
            len: 0,
        }
    }

    /// Push a value into the buffer, overwriting the oldest if full.
    pub fn push(&mut self, val: i64) {
        let pos = (self.head + self.len) % self.cap;
        self.buf[pos] = val;
        if self.len == self.cap {
            self.head = (self.head + 1) % self.cap;
        } else {
            self.len += 1;
        }
    }

    /// Return the number of elements currently stored.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Return whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Get element at logical index (0 = oldest).
    pub fn get(&self, index: usize) -> Option<i64> {
        if index >= self.len {
            return None;
        }
        Some(self.buf[(self.head + index) % self.cap])
    }

    /// Drain all elements oldest-first.
    pub fn drain_all(&mut self) -> Vec<i64> {
        let mut out = Vec::with_capacity(self.len);
        for i in 0..self.len {
            out.push(self.buf[(self.head + i) % self.cap]);
        }
        self.head = 0;
        self.len = 0;
        out
    }

    /// Peek at the oldest element.
    pub fn peek_front(&self) -> Option<i64> {
        self.get(0)
    }

    /// Peek at the newest element.
    pub fn peek_back(&self) -> Option<i64> {
        if self.len == 0 {
            None
        } else {
            self.get(self.len - 1)
        }
    }

    /// Clear the buffer.
    pub fn clear(&mut self) {
        self.head = 0;
        self.len = 0;
    }

    /// Return capacity.
    pub fn capacity(&self) -> usize {
        self.cap
    }
}

/// Compute a simple FNV-1a 64-bit hash over bytes.
pub fn xb_fnv1a_34(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_34<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
    let mut result = Vec::new();
    let mut i = 0;
    while i < items.len() {
        let val = &items[i];
        let mut count = 1;
        while i + count < items.len() && items[i + count] == *val {
            count += 1;
        }
        result.push((val.clone(), count));
        i += count;
    }
    result
}

/// Decode an RLE-encoded sequence.
pub fn xb_rle_decode_34<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_34(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_34(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 201
// ---------------------------------------------------------------------------

/// Generic object pool `Xc201Pool<T>`.
pub struct Xc201Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc201Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc201PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc201Pool<T> {
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
    pub fn stats(&self) -> Xc201PoolStats {
        Xc201PoolStats {
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

impl<T> Default for Xc201Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc201Scheduler`.
pub struct Xc201Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc201Scheduler {
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

impl Default for Xc201Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_201 hash for the given byte slice.
pub fn xc_201_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_201 convention.
pub fn xc_201_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe46 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe46Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe46PipelineError {
    pub stage: Xe46Stage,
    pub message: String,
}

impl std::fmt::Display for Xe46PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe46Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe46Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe46PipelineError>>>,
    stage_names: Vec<Xe46Stage>,
}

impl Xe46Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe46PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe46Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe46PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe46Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe46PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe46Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe46PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe46Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe46PipelineError> {
        let mut data = input;
        for (i, stage_fn) in self.stages.iter().enumerate() {
            data = stage_fn(data).map_err(|mut e| {
                e.stage = self.stage_names[i].clone();
                e
            })?;
        }
        Ok(data)
    }

    pub fn stage_count(&self) -> usize {
        self.stages.len()
    }

    pub fn compose(mut self, other: Xe46Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe46CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe46CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe46Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe46CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe46CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe46Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe46CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_46_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe46CacheEntry {
            value,
            inserted_at: self.current_time,
            ttl,
        });
    }

    pub fn get(&mut self, key: &K) -> Option<V> {
        let now = self.current_time;
        if let Some(entry) = self.entries.get(key) {
            if now - entry.inserted_at < entry.ttl {
                self.stats.hits += 1;
                return Some(entry.value.clone());
            } else {
                self.stats.misses += 1;
                let key_clone = key.clone();
                self.entries.remove(&key_clone);
                return None;
            }
        }
        self.stats.misses += 1;
        None
    }

    pub fn evict(&mut self, key: &K) -> bool {
        if self.entries.remove(key).is_some() {
            self.stats.evictions += 1;
            true
        } else {
            false
        }
    }

    fn xe_46_evict_expired(&mut self) {
        let now = self.current_time;
        let expired: Vec<K> = self.entries.iter()
            .filter(|(_, e)| now - e.inserted_at >= e.ttl)
            .map(|(k, _)| k.clone())
            .collect();
        for k in &expired {
            self.entries.remove(k);
            self.stats.evictions += 1;
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn stats(&self) -> &Xe46CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_46_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe46PipelineError> {
    Ok(data)
}

pub fn xe_46_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe46PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_46_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe46PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_46_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe46PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_46_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe46PipelineError> {
    Err(Xe46PipelineError {
        stage: Xe46Stage::Parse,
        message: "intentional failure".to_string(),
    })
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

    #[test]
    fn session_scopes_display() {
        let session = AuthenticationSession { id: "s1".into(), provider_id: "github".into(), account_label: "user".into(), scopes: vec!["read".into(), "write".into()] };
        assert_eq!(session.scopes_display(), "read, write");
    }

    #[test]
    fn session_missing_scopes() {
        let session = AuthenticationSession { id: "s1".into(), provider_id: "github".into(), account_label: "user".into(), scopes: vec!["read".into(), "write".into()] };
        assert_eq!(session.missing_scopes(&["read", "admin"]), vec!["admin"]);
    }

    #[test]
    fn session_has_any_scope() {
        let session = AuthenticationSession { id: "s1".into(), provider_id: "github".into(), account_label: "user".into(), scopes: vec!["read".into()] };
        assert!(session.has_any_scope(&["write", "read"]));
        assert!(!session.has_any_scope(&["write", "admin"]));
    }

    #[test]
    fn providers_with_sessions_lists() {
        let mut svc = AuthenticationService::new();
        svc.register_provider(github_provider());
        svc.register_provider(azure_provider());
        svc.create_session("github", vec!["read".into()]);
        svc.create_session("azure", vec!["email".into()]);
        let ids = svc.providers_with_sessions();
        assert_eq!(ids, vec!["azure", "github"]);
    }

    #[test]
    fn total_unique_scopes_counts() {
        let mut svc = AuthenticationService::new();
        svc.register_provider(github_provider());
        svc.create_session("github", vec!["read".into(), "write".into()]);
        svc.create_session("github", vec!["read".into(), "admin".into()]);
        assert_eq!(svc.total_unique_scopes(), 3);
    }

    #[test]
    fn clear_provider_sessions_removes() {
        let mut svc = AuthenticationService::new();
        svc.register_provider(github_provider());
        svc.create_session("github", vec!["read".into()]);
        svc.create_session("github", vec!["write".into()]);
        assert_eq!(svc.clear_provider_sessions("github"), 2);
        assert_eq!(svc.session_count(), 0);
    }

    #[test]
    fn service_summary_format() {
        let mut svc = AuthenticationService::new();
        svc.register_provider(github_provider());
        svc.create_session("github", vec!["read".into()]);
        assert!(svc.summary().contains("1 provider(s)"));
    }

    #[test]
    fn active_providers_filters() {
        let providers = vec![github_provider(), azure_provider()];
        let active = active_providers(&providers);
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].id, "azure");
    }

    #[test]
    fn multi_account_providers_filters() {
        let providers = vec![github_provider(), azure_provider()];
        let multi = multi_account_providers(&providers);
        assert_eq!(multi.len(), 1);
        assert_eq!(multi[0].id, "azure");
    }

    // -- AuthSessionManager tests --

    #[test]
    fn session_manager_store_and_expire() {
        let mut mgr = AuthSessionManager::new();
        mgr.store(ManagedSession {
            session_id: "s1".into(),
            provider_id: "github".into(),
            access_token: "tok123".into(),
            expires_at_ms: Some(5000),
            refresh_count: 0,
        });
        assert!(!mgr.is_expired("s1", 4000));
        assert!(mgr.is_expired("s1", 5000));
        assert_eq!(mgr.session_count(), 1);
    }

    #[test]
    fn session_manager_refresh() {
        let mut mgr = AuthSessionManager::new();
        mgr.store(ManagedSession {
            session_id: "s1".into(),
            provider_id: "gh".into(),
            access_token: "old".into(),
            expires_at_ms: Some(1000),
            refresh_count: 0,
        });
        let old = mgr.refresh_token("s1", "new".into(), Some(2000));
        assert_eq!(old, Some("old".into()));
        assert_eq!(mgr.refresh_count("s1"), 1);
        assert!(!mgr.is_expired("s1", 1500));
    }

    #[test]
    fn session_manager_expiring_before() {
        let mut mgr = AuthSessionManager::new();
        mgr.store(ManagedSession {
            session_id: "a".into(), provider_id: "gh".into(),
            access_token: "t".into(), expires_at_ms: Some(100), refresh_count: 0,
        });
        mgr.store(ManagedSession {
            session_id: "b".into(), provider_id: "gh".into(),
            access_token: "t".into(), expires_at_ms: Some(500), refresh_count: 0,
        });
        assert_eq!(mgr.expiring_before(200).len(), 1);
        assert_eq!(mgr.expiring_before(600).len(), 2);
    }

    // -- OAuthFlowHandler tests --

    #[test]
    fn oauth_flow_lifecycle() {
        let mut flow = OAuthFlowHandler::new("client1", "http://localhost:8080", vec!["read".into()]);
        assert_eq!(flow.state(), OAuthFlowState::Pending);
        assert!(flow.authorization_url().contains("client1"));
        flow.mark_browser_opened();
        assert_eq!(flow.state(), OAuthFlowState::AwaitingCallback);
        flow.handle_callback("code123".into());
        assert_eq!(flow.state(), OAuthFlowState::Completed);
        assert_eq!(flow.authorization_code(), Some("code123"));
    }

    #[test]
    fn oauth_flow_fail() {
        let mut flow = OAuthFlowHandler::new("c", "http://localhost", vec![]);
        flow.fail();
        assert_eq!(flow.state(), OAuthFlowState::Failed);
        assert_eq!(flow.authorization_code(), None);
    }

    // -- AuthProviderChain tests --

    #[test]
    fn auth_chain_finds_first_available() {
        let mut svc = AuthenticationService::new();
        svc.register_provider(AuthProvider {
            id: "gh".into(), label: "GitHub".into(),
            supports_multiple_accounts: false, status: AuthProviderStatus::Active,
        });
        let chain = AuthProviderChain::new(vec!["missing".into(), "gh".into()]);
        let found = chain.first_available(&svc);
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, "gh");
    }

    #[test]
    fn auth_chain_empty() {
        let chain = AuthProviderChain::new(vec![]);
        assert!(chain.is_empty());
        assert_eq!(chain.len(), 0);
    }

    // -- CredentialEncryptionWrapper tests --

    #[test]
    fn credential_roundtrip() {
        let wrapper = CredentialEncryptionWrapper::new(0xAB);
        let encrypted = wrapper.encrypt("my_secret_token");
        let decrypted = wrapper.decrypt(&encrypted);
        assert_eq!(decrypted, "my_secret_token");
        // Encrypted should differ from plaintext
        assert_ne!(encrypted, b"my_secret_token");
    }

    #[test]
    fn credential_different_keys() {
        let w1 = CredentialEncryptionWrapper::new(0x11);
        let w2 = CredentialEncryptionWrapper::new(0x22);
        let e1 = w1.encrypt("test");
        let e2 = w2.encrypt("test");
        assert_ne!(e1, e2);
    }

    // --- AuthSessionListView tests ------------------------------------------

    fn make_session(id: &str, provider: &str, scopes: &[&str]) -> AuthenticationSession {
        AuthenticationSession {
            id: id.to_string(),
            provider_id: provider.to_string(),
            account_label: format!("user@{provider}"),
            scopes: scopes.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn session_list_view_basic() {
        let sessions = vec![
            make_session("s1", "github", &["repo", "user"]),
            make_session("s2", "azure", &["mail"]),
        ];
        let view = AuthSessionListView::new(sessions);
        assert_eq!(view.len(), 2);
        assert!(!view.is_empty());
    }

    #[test]
    fn session_list_view_render() {
        let sessions = vec![make_session("s1", "github", &["repo"])];
        let view = AuthSessionListView::new(sessions);
        let rendered = view.render_all();
        assert_eq!(rendered.len(), 1);
        assert!(rendered[0].contains("github"));
        assert!(rendered[0].contains("repo"));
    }

    #[test]
    fn session_list_view_hide_scopes() {
        let sessions = vec![make_session("s1", "gh", &["a"])];
        let view = AuthSessionListView::new(sessions).with_scopes(false);
        let rendered = view.render_all();
        assert!(!rendered[0].contains("scopes"));
    }

    #[test]
    fn session_list_view_filter_by_scope() {
        let sessions = vec![
            make_session("s1", "gh", &["repo", "user"]),
            make_session("s2", "gh", &["user"]),
        ];
        let view = AuthSessionListView::new(sessions);
        let filtered = view.filter_by_scope("repo");
        assert_eq!(filtered.len(), 1);
    }

    #[test]
    fn session_list_view_filter_by_provider() {
        let sessions = vec![
            make_session("s1", "github", &[]),
            make_session("s2", "azure", &[]),
        ];
        let view = AuthSessionListView::new(sessions);
        assert_eq!(view.filter_by_provider("azure").len(), 1);
    }

    #[test]
    fn session_list_view_provider_ids() {
        let sessions = vec![
            make_session("s1", "github", &[]),
            make_session("s2", "azure", &[]),
            make_session("s3", "github", &[]),
        ];
        let view = AuthSessionListView::new(sessions);
        let ids = view.provider_ids();
        assert_eq!(ids.len(), 2);
    }

    #[test]
    fn session_list_view_display() {
        let sessions = vec![make_session("s1", "gh", &[])];
        let view = AuthSessionListView::new(sessions);
        let display = format!("{}", view);
        assert!(display.contains("Sessions (1)"));
    }

    // --- AuthTokenInspector tests -------------------------------------------

    #[test]
    fn token_inspector_parse() {
        let inspector = AuthTokenInspector::parse("sub=user123;iss=github;exp=1700000000");
        assert_eq!(inspector.claim_count(), 3);
        assert_eq!(inspector.subject(), Some("user123"));
        assert_eq!(inspector.issuer(), Some("github"));
        assert_eq!(inspector.expiry_timestamp(), Some(1700000000));
    }

    #[test]
    fn token_inspector_get_claim() {
        let inspector = AuthTokenInspector::parse("key=value;other=data");
        assert_eq!(inspector.get_claim("key"), Some("value"));
        assert!(inspector.has_claim("other"));
        assert!(!inspector.has_claim("missing"));
    }

    #[test]
    fn token_inspector_render() {
        let inspector = AuthTokenInspector::parse("a=1;b=2");
        let rendered = inspector.render_claims();
        assert!(rendered.contains("a=1"));
        assert!(rendered.contains("b=2"));
    }

    #[test]
    fn token_inspector_empty() {
        let inspector = AuthTokenInspector::parse("");
        assert_eq!(inspector.claim_count(), 0);
        assert_eq!(inspector.subject(), None);
    }

    #[test]
    fn token_claim_display() {
        let claim = TokenClaim::new("sub", "user1");
        assert_eq!(claim.to_string(), "sub=user1");
    }

    // --- AuthScopeDisplay tests ---------------------------------------------

    #[test]
    fn scope_display_parse() {
        let scopes = vec!["repo:read".into(), "user:write".into(), "admin".into()];
        let display = AuthScopeDisplay::new(&scopes);
        assert_eq!(display.len(), 3);
    }

    #[test]
    fn parsed_scope_permissions() {
        let read = ParsedScope::parse("repo:read");
        assert!(read.is_read());
        assert!(!read.is_write());
        let write = ParsedScope::parse("repo:write");
        assert!(write.is_write());
    }

    #[test]
    fn parsed_scope_wildcard() {
        let wc = ParsedScope::parse("*:read");
        assert!(wc.is_wildcard());
        let normal = ParsedScope::parse("repo:read");
        assert!(!normal.is_wildcard());
    }

    #[test]
    fn scope_display_resources() {
        let scopes = vec!["repo:read".into(), "repo:write".into(), "user:read".into()];
        let display = AuthScopeDisplay::new(&scopes);
        let res = display.resources();
        assert_eq!(res.len(), 2);
    }

    #[test]
    fn scope_display_write_scopes() {
        let scopes = vec!["repo:write".into(), "user:read".into()];
        let display = AuthScopeDisplay::new(&scopes);
        assert_eq!(display.write_scopes().len(), 1);
    }

    #[test]
    fn scope_display_render() {
        let scopes = vec!["a".into(), "b".into()];
        let display = AuthScopeDisplay::new(&scopes);
        assert_eq!(display.render(), "a, b");
    }

    // --- AuthExpiryWarning tests --------------------------------------------

    #[test]
    fn expiry_warning_ok() {
        let w = AuthExpiryWarning::new("s1", 10000);
        assert_eq!(w.urgency(0), ExpiryUrgency::Ok);
        assert!(!w.is_expired(0));
        assert!(!w.should_warn(0));
    }

    #[test]
    fn expiry_warning_warning() {
        let w = AuthExpiryWarning::new("s1", 1800);
        assert_eq!(w.urgency(0), ExpiryUrgency::Warning);
        assert!(w.should_warn(0));
    }

    #[test]
    fn expiry_warning_critical() {
        let w = AuthExpiryWarning::new("s1", 300);
        assert_eq!(w.urgency(0), ExpiryUrgency::Critical);
        assert!(w.should_warn(0));
    }

    #[test]
    fn expiry_warning_expired() {
        let w = AuthExpiryWarning::new("s1", 100);
        assert_eq!(w.urgency(200), ExpiryUrgency::Expired);
        assert!(w.is_expired(200));
    }

    #[test]
    fn expiry_remaining_display() {
        let w = AuthExpiryWarning::new("s1", 7200);
        assert!(w.remaining_display(0).contains("2h"));
        let w2 = AuthExpiryWarning::new("s2", 0);
        assert_eq!(w2.remaining_display(100), "expired");
    }

    #[test]
    fn expiry_urgency_ordering() {
        assert!(ExpiryUrgency::Ok < ExpiryUrgency::Warning);
        assert!(ExpiryUrgency::Warning < ExpiryUrgency::Critical);
        assert!(ExpiryUrgency::Critical < ExpiryUrgency::Expired);
    }

    #[test]
    fn expiry_urgency_display() {
        assert_eq!(ExpiryUrgency::Ok.to_string(), "ok");
        assert_eq!(ExpiryUrgency::Expired.to_string(), "expired");
    }

    #[test]
    fn expiry_warning_display() {
        let w = AuthExpiryWarning::new("s1", 1000);
        let s = w.to_string();
        assert!(s.contains("s1"));
        assert!(s.contains("1000"));
    }


    #[test]
    fn wb_auth_entry_creation() {
        let e = WbAuthEntry::new("e1", "Entry 1");
        assert_eq!(e.id, "e1");
        assert_eq!(e.label, "Entry 1");
        assert!(e.active);
        assert_eq!(e.priority, 0);
    }

    #[test]
    fn wb_auth_entry_with_priority() {
        let e = WbAuthEntry::new("e1", "E").with_priority(5);
        assert_eq!(e.priority, 5);
    }

    #[test]
    fn wb_auth_entry_metadata() {
        let e = WbAuthEntry::new("e1", "E").with_meta("key", "val");
        assert_eq!(e.get_meta("key"), Some("val"));
        assert_eq!(e.get_meta("missing"), None);
        assert!(e.has_meta("key"));
        assert_eq!(e.meta_count(), 1);
    }

    #[test]
    fn wb_auth_entry_remove_meta() {
        let mut e = WbAuthEntry::new("e1", "E").with_meta("k", "v");
        assert!(e.remove_meta("k"));
        assert!(!e.remove_meta("k"));
    }

    #[test]
    fn wb_auth_entry_activate_deactivate() {
        let mut e = WbAuthEntry::new("e1", "E");
        e.deactivate();
        assert!(!e.active);
        e.activate();
        assert!(e.active);
    }

    #[test]
    fn wb_auth_config_add_sorted() {
        let mut c = WbAuthConfig::new(10);
        c.add(WbAuthEntry::new("lo", "Lo").with_priority(1));
        c.add(WbAuthEntry::new("hi", "Hi").with_priority(10));
        assert_eq!(c.ids()[0], "hi");
    }

    #[test]
    fn wb_auth_config_capacity() {
        let mut c = WbAuthConfig::new(1);
        assert!(c.add(WbAuthEntry::new("a", "A")));
        assert!(!c.add(WbAuthEntry::new("b", "B")));
        assert!(c.is_full());
    }

    #[test]
    fn wb_auth_config_remove() {
        let mut c = WbAuthConfig::new(10);
        c.add(WbAuthEntry::new("a", "A"));
        assert!(c.remove("a"));
        assert!(!c.remove("a"));
        assert!(c.is_empty());
    }

    #[test]
    fn wb_auth_config_get() {
        let mut c = WbAuthConfig::new(10);
        c.add(WbAuthEntry::new("x", "X"));
        assert!(c.get("x").is_some());
        assert!(c.get("y").is_none());
    }

    #[test]
    fn wb_auth_config_active_entries() {
        let mut c = WbAuthConfig::new(10);
        c.add(WbAuthEntry::new("a", "A"));
        c.add(WbAuthEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        assert_eq!(c.active_entries().len(), 1);
        assert_eq!(c.count_active(), 1);
    }

    #[test]
    fn wb_auth_config_enable_disable() {
        let mut c = WbAuthConfig::new(10);
        c.disable();
        assert!(!c.is_enabled());
        c.enable();
        assert!(c.is_enabled());
    }

    #[test]
    fn wb_auth_config_clear() {
        let mut c = WbAuthConfig::new(10);
        c.add(WbAuthEntry::new("a", "A"));
        c.clear();
        assert!(c.is_empty());
    }

    #[test]
    fn wb_auth_config_find_by_label() {
        let mut c = WbAuthConfig::new(10);
        c.add(WbAuthEntry::new("a", "Alpha"));
        assert_eq!(c.find_by_label("Alpha").unwrap().id, "a");
        assert!(c.find_by_label("missing").is_none());
    }

    #[test]
    fn wb_auth_config_top_n() {
        let mut c = WbAuthConfig::new(10);
        c.add(WbAuthEntry::new("a", "A").with_priority(1));
        c.add(WbAuthEntry::new("b", "B").with_priority(2));
        c.add(WbAuthEntry::new("c", "C").with_priority(3));
        assert_eq!(c.top_n(2).len(), 2);
    }

    #[test]
    fn wb_auth_config_deactivate_activate_all() {
        let mut c = WbAuthConfig::new(10);
        c.add(WbAuthEntry::new("a", "A"));
        c.add(WbAuthEntry::new("b", "B"));
        c.deactivate_all();
        assert_eq!(c.count_active(), 0);
        c.activate_all();
        assert_eq!(c.count_active(), 2);
    }

    #[test]
    fn wb_auth_config_highest_priority() {
        let mut c = WbAuthConfig::new(10);
        assert!(c.highest_priority().is_none());
        c.add(WbAuthEntry::new("a", "A").with_priority(7));
        assert_eq!(c.highest_priority(), Some(7));
    }

    #[test]
    fn wb_auth_config_contains() {
        let mut c = WbAuthConfig::new(10);
        c.add(WbAuthEntry::new("a", "A"));
        assert!(c.contains("a"));
        assert!(!c.contains("b"));
    }

    #[test]
    fn wb_auth_config_labels() {
        let mut c = WbAuthConfig::new(10);
        c.add(WbAuthEntry::new("a", "Alpha"));
        c.add(WbAuthEntry::new("b", "Beta"));
        let labels = c.labels();
        assert!(labels.contains(&"Alpha"));
        assert!(labels.contains(&"Beta"));
    }

    #[test]
    fn wb_auth_config_drain_inactive() {
        let mut c = WbAuthConfig::new(10);
        c.add(WbAuthEntry::new("a", "A"));
        c.add(WbAuthEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        let drained = c.drain_inactive();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].id, "a");
        assert_eq!(c.len(), 1);
    }


    #[test]
    fn xo_metrics_empty() {
        let m = XoMetrics::new("wb_auth");
        assert_eq!(m.count(), 0);
        assert!((m.mean() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xo_metrics_record_and_mean() {
        let mut m = XoMetrics::new("wb_auth");
        m.record(10.0);
        m.record(20.0);
        m.record(30.0);
        assert_eq!(m.count(), 3);
        assert!((m.mean() - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xo_metrics_min_max() {
        let mut m = XoMetrics::new("test");
        m.record(5.0);
        m.record(15.0);
        m.record(10.0);
        assert!((m.min_val() - 5.0).abs() < f64::EPSILON);
        assert!((m.max_val() - 15.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xo_metrics_variance_and_std() {
        let mut m = XoMetrics::new("v");
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
    fn xo_metrics_percentile() {
        let mut m = XoMetrics::new("p");
        for i in 1..=100 {
            m.record(i as f64);
        }
        let p50 = m.percentile(50.0);
        assert!(p50 >= 49.0 && p50 <= 51.0);
    }

    #[test]
    fn xo_metrics_merge() {
        let mut a = XoMetrics::new("a");
        a.record(1.0);
        let mut b = XoMetrics::new("b");
        b.record(2.0);
        b.record(3.0);
        a.merge(&b);
        assert_eq!(a.count(), 3);
    }

    #[test]
    fn xo_metrics_reset() {
        let mut m = XoMetrics::new("r");
        m.record(42.0);
        m.reset();
        assert_eq!(m.count(), 0);
    }

    #[test]
    fn xo_rate_window_empty() {
        let rw = XoRateWindow::new(1000);
        assert!(rw.is_empty());
        assert_eq!(rw.window_ms(), 1000);
    }

    #[test]
    fn xo_rate_window_tick_and_rate() {
        let mut rw = XoRateWindow::new(1000);
        rw.tick(100);
        rw.tick(200);
        rw.tick(300);
        assert_eq!(rw.rate(500), 3);
        assert_eq!(rw.rate(1500), 0);
    }

    #[test]
    fn xo_lru_cache_basic() {
        let mut c = XoLruCache::new(2);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        assert_eq!(c.get("a"), Some("1".to_string()));
        c.put("c".into(), "3".into());
        assert_eq!(c.get("b"), None);
    }

    #[test]
    fn xo_lru_cache_contains_and_keys() {
        let mut c = XoLruCache::new(3);
        c.put("x".into(), "10".into());
        c.put("y".into(), "20".into());
        assert!(c.contains_key("x"));
        assert!(!c.contains_key("z"));
        assert_eq!(c.keys().len(), 2);
    }

    #[test]
    fn xo_lru_cache_remove() {
        let mut c = XoLruCache::new(3);
        c.put("k".into(), "v".into());
        assert_eq!(c.remove("k"), Some("v".to_string()));
        assert!(c.is_empty());
        assert_eq!(c.remove("k"), None);
    }

    #[test]
    fn xo_metrics_sum() {
        let mut m = XoMetrics::new("s");
        m.record(1.0);
        m.record(2.0);
        m.record(3.0);
        assert!((m.sum() - 6.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xo_metrics_label() {
        let m = XoMetrics::new("my_label");
        assert_eq!(m.label(), "my_label");
    }

    #[test]
    fn xo_lru_cache_clear() {
        let mut c = XoLruCache::new(5);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    #[test]
    fn xb_ring_buffer_34_push_and_len() {
        let mut rb = super::XbRingBuffer34::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_34_overwrite() {
        let mut rb = super::XbRingBuffer34::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_34_get_out_of_bounds() {
        let rb = super::XbRingBuffer34::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_34_drain_all() {
        let mut rb = super::XbRingBuffer34::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_34_peek_front_back() {
        let mut rb = super::XbRingBuffer34::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_34_clear() {
        let mut rb = super::XbRingBuffer34::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_34_capacity() {
        let rb = super::XbRingBuffer34::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_34_basic() {
        let h = super::xb_fnv1a_34(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_34(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_34_different_inputs() {
        let h1 = super::xb_fnv1a_34(b"abc");
        let h2 = super::xb_fnv1a_34(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_34_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_34(&data);
        let dec = super::xb_rle_decode_34(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_34_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_34(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_34(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_34_values() {
        assert!((super::xb_clamp_34(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_34(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_34(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_34_values() {
        assert!((super::xb_lerp_34(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_34(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_34(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_34_wrap_around_twice() {
        let mut rb = super::XbRingBuffer34::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
    }


    // ---- xc_ pool / scheduler tests – block 201 ----

    #[test]
    fn xc_201_pool_new_empty() {
        let pool: super::Xc201Pool<i32> = super::Xc201Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_201_pool_release_acquire() {
        let mut pool = super::Xc201Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_201_pool_acquire_empty() {
        let mut pool: super::Xc201Pool<i32> = super::Xc201Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_201_pool_full() {
        let mut pool = super::Xc201Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_201_pool_drain() {
        let mut pool = super::Xc201Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_201_pool_stats() {
        let mut pool = super::Xc201Pool::new(8);
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
    fn xc_201_pool_clear() {
        let mut pool = super::Xc201Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_201_pool_shrink() {
        let mut pool = super::Xc201Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_201_pool_default() {
        let pool: super::Xc201Pool<String> = super::Xc201Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_201_pool_extend() {
        let mut pool = super::Xc201Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_201_pool_retain() {
        let mut pool = super::Xc201Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_201_scheduler_round_robin() {
        let mut sched = super::Xc201Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_201_scheduler_empty() {
        let mut sched = super::Xc201Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_201_scheduler_reset() {
        let mut sched = super::Xc201Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_201_scheduler_add_remove() {
        let mut sched = super::Xc201Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_201_scheduler_targets() {
        let sched = super::Xc201Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_201_hash_empty() {
        assert_eq!(super::xc_201_hash(b""), 5381);
    }

    #[test]
    fn xc_201_hash_data() {
        let h = super::xc_201_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_201_hash(b"hello"), h);
    }

    #[test]
    fn xc_201_reverse_str() {
        assert_eq!(super::xc_201_reverse("abc"), "cba");
        assert_eq!(super::xc_201_reverse(""), "");
    }


    #[test]
    fn xe_46_pipeline_empty() {
        let p = super::Xe46Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_46_pipeline_parse_stage() {
        let p = super::Xe46Pipeline::new()
            .add_parse(super::xe_46_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_46_pipeline_transform_double() {
        let p = super::Xe46Pipeline::new()
            .add_transform(super::xe_46_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_46_pipeline_validate_reverse() {
        let p = super::Xe46Pipeline::new()
            .add_validate(super::xe_46_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_46_pipeline_emit_filter() {
        let p = super::Xe46Pipeline::new()
            .add_emit(super::xe_46_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_46_pipeline_multi_stage() {
        let p = super::Xe46Pipeline::new()
            .add_parse(super::xe_46_pipeline_identity)
            .add_transform(super::xe_46_pipeline_double)
            .add_validate(super::xe_46_pipeline_reverse)
            .add_emit(super::xe_46_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_46_pipeline_error_propagation() {
        let p = super::Xe46Pipeline::new()
            .add_parse(super::xe_46_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe46Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_46_pipeline_compose() {
        let p1 = super::Xe46Pipeline::new()
            .add_parse(super::xe_46_pipeline_identity);
        let p2 = super::Xe46Pipeline::new()
            .add_transform(super::xe_46_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_46_pipeline_error_display() {
        let e = super::Xe46PipelineError {
            stage: super::Xe46Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_46_cache_put_get() {
        let mut c = super::Xe46Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_46_cache_miss() {
        let mut c: super::Xe46Cache<&str, i32> = super::Xe46Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_46_cache_ttl_expiry() {
        let mut c = super::Xe46Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_46_cache_evict() {
        let mut c = super::Xe46Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_46_cache_capacity() {
        let mut c = super::Xe46Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_46_cache_stats() {
        let mut c = super::Xe46Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_46_cache_clear() {
        let mut c = super::Xe46Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }

}
