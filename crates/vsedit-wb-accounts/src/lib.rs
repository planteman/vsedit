//! Account/login management.

use std::collections::{BTreeSet, HashMap};
use std::fmt;

/// Information about an authenticated account.
#[derive(Debug, Clone, PartialEq)]
pub struct AccountInfo {
    pub id: String,
    pub label: String,
    pub provider_id: String,
    pub email: Option<String>,
}

impl AccountInfo {
    /// Returns the display name (label) of the account.
    pub fn display_name(&self) -> &str {
        &self.label
    }
}

impl fmt::Display for AccountInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.label, self.provider_id)
    }
}

/// An active authentication session.
#[derive(Debug, Clone)]
pub struct AuthSession {
    pub id: String,
    pub account: AccountInfo,
    pub scopes: Vec<String>,
    pub access_token: String,
}

impl AuthSession {
    /// Returns `true` if this session includes the given scope.
    pub fn has_scope(&self, scope: &str) -> bool {
        self.scopes.iter().any(|s| s == scope)
    }

    /// Returns `true` if this session includes all of the given scopes.
    pub fn has_all_scopes(&self, scopes: &[&str]) -> bool {
        scopes.iter().all(|scope| self.has_scope(scope))
    }
}

impl fmt::Display for AuthSession {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} - {}", self.account, self.scopes.join(", "))
    }
}

/// Service for managing authentication sessions.
pub struct AccountsService {
    sessions: Vec<AuthSession>,
}

impl AccountsService {
    pub fn new() -> Self {
        Self {
            sessions: Vec::new(),
        }
    }

    pub fn add_session(&mut self, session: AuthSession) {
        self.sessions.push(session);
    }

    pub fn remove_session(&mut self, id: &str) -> bool {
        let len = self.sessions.len();
        self.sessions.retain(|s| s.id != id);
        self.sessions.len() != len
    }

    pub fn get_sessions(&self, provider_id: &str) -> Vec<&AuthSession> {
        self.sessions
            .iter()
            .filter(|s| s.account.provider_id == provider_id)
            .collect()
    }

    pub fn get_session_by_id(&self, id: &str) -> Option<&AuthSession> {
        self.sessions.iter().find(|s| s.id == id)
    }

    pub fn is_authenticated(&self, provider_id: &str) -> bool {
        self.sessions
            .iter()
            .any(|s| s.account.provider_id == provider_id)
    }

    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }

    /// Returns a slice of all sessions.
    pub fn get_all_sessions(&self) -> &[AuthSession] {
        &self.sessions
    }

    /// Finds the first session for `provider_id` that has all of the given scopes.
    pub fn find_session_with_scopes(
        &self,
        provider_id: &str,
        scopes: &[&str],
    ) -> Option<&AuthSession> {
        self.sessions.iter().find(|s| {
            s.account.provider_id == provider_id && s.has_all_scopes(scopes)
        })
    }

    /// Returns a deduplicated list of provider IDs across all sessions.
    pub fn get_unique_providers(&self) -> Vec<&str> {
        let mut providers: Vec<&str> =
            self.sessions.iter().map(|s| s.account.provider_id.as_str()).collect();
        providers.sort_unstable();
        providers.dedup();
        providers
    }

    /// Removes all sessions for the given provider, returning the number removed.
    pub fn remove_sessions_for_provider(&mut self, provider_id: &str) -> usize {
        let before = self.sessions.len();
        self.sessions.retain(|s| s.account.provider_id != provider_id);
        before - self.sessions.len()
    }

    /// Updates the access token for the session with the given id.
    /// Returns `true` if the session was found and updated.
    pub fn update_token(&mut self, session_id: &str, new_token: String) -> bool {
        if let Some(session) = self.sessions.iter_mut().find(|s| s.id == session_id) {
            session.access_token = new_token;
            true
        } else {
            false
        }
    }

    /// Returns `true` if a session with the given id exists.
    pub fn is_session_valid(&self, id: &str) -> bool {
        self.sessions.iter().any(|s| s.id == id)
    }
}

impl Default for AccountsService {
    fn default() -> Self {
        Self::new()
    }
}

/// Errors related to account operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccountError {
    SessionNotFound(String),
    ProviderNotFound(String),
    DuplicateSession(String),
    InvalidToken,
    MissingScope(String),
}

impl fmt::Display for AccountError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SessionNotFound(id) => write!(f, "session not found: {id}"),
            Self::ProviderNotFound(id) => write!(f, "provider not found: {id}"),
            Self::DuplicateSession(id) => write!(f, "duplicate session: {id}"),
            Self::InvalidToken => write!(f, "invalid or empty token"),
            Self::MissingScope(scope) => write!(f, "missing required scope: {scope}"),
        }
    }
}

impl std::error::Error for AccountError {}

impl AccountInfo {
    /// Create a new AccountInfo with all fields.
    pub fn new(
        id: impl Into<String>,
        label: impl Into<String>,
        provider_id: impl Into<String>,
        email: Option<String>,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            provider_id: provider_id.into(),
            email,
        }
    }

    /// Check if an email is set.
    pub fn has_email(&self) -> bool {
        self.email.is_some()
    }

    /// Return the email, falling back to a default string.
    pub fn email_or_default(&self) -> &str {
        self.email.as_deref().unwrap_or("(no email)")
    }
}

impl AuthSession {
    /// Create a new auth session.
    pub fn new(
        id: impl Into<String>,
        account: AccountInfo,
        scopes: Vec<String>,
        access_token: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            account,
            scopes,
            access_token: access_token.into(),
        }
    }

    /// Returns the number of scopes this session has.
    pub fn scope_count(&self) -> usize {
        self.scopes.len()
    }

    /// Check if the access token is non-empty.
    pub fn has_valid_token(&self) -> bool {
        !self.access_token.is_empty()
    }

    /// Return scopes as a comma-separated string.
    pub fn scopes_display(&self) -> String {
        self.scopes.join(", ")
    }
}

impl PartialEq for AuthSession {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl AccountsService {
    /// Add a session, returning an error if a session with the same ID exists.
    pub fn try_add_session(&mut self, session: AuthSession) -> Result<(), AccountError> {
        if self.sessions.iter().any(|s| s.id == session.id) {
            return Err(AccountError::DuplicateSession(session.id));
        }
        self.sessions.push(session);
        Ok(())
    }

    /// Remove a session by ID, returning an error if not found.
    pub fn try_remove_session(&mut self, id: &str) -> Result<AuthSession, AccountError> {
        let pos = self
            .sessions
            .iter()
            .position(|s| s.id == id)
            .ok_or_else(|| AccountError::SessionNotFound(id.to_string()))?;
        Ok(self.sessions.remove(pos))
    }

    /// Get all session IDs.
    pub fn session_ids(&self) -> Vec<&str> {
        self.sessions.iter().map(|s| s.id.as_str()).collect()
    }

    /// Find sessions by account email.
    pub fn find_by_email(&self, email: &str) -> Vec<&AuthSession> {
        self.sessions
            .iter()
            .filter(|s| s.account.email.as_deref() == Some(email))
            .collect()
    }

    /// Check if any session has a specific scope across all providers.
    pub fn any_session_has_scope(&self, scope: &str) -> bool {
        self.sessions.iter().any(|s| s.has_scope(scope))
    }

    /// Return the total number of unique scopes across all sessions.
    pub fn total_unique_scopes(&self) -> usize {
        let mut all_scopes: Vec<&str> = self
            .sessions
            .iter()
            .flat_map(|s| s.scopes.iter().map(|sc| sc.as_str()))
            .collect();
        all_scopes.sort_unstable();
        all_scopes.dedup();
        all_scopes.len()
    }

    /// Validate that a session has a valid (non-empty) token.
    pub fn validate_session_token(&self, id: &str) -> Result<(), AccountError> {
        let session = self
            .sessions
            .iter()
            .find(|s| s.id == id)
            .ok_or_else(|| AccountError::SessionNotFound(id.to_string()))?;
        if session.access_token.is_empty() {
            return Err(AccountError::InvalidToken);
        }
        Ok(())
    }
}

impl fmt::Display for AccountsService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "AccountsService({} sessions, {} providers)",
            self.sessions.len(),
            self.get_unique_providers().len()
        )
    }
}

/// Accumulated statistics for wb-accounts operations.
#[derive(Debug, Clone, PartialEq)]
pub struct WbAccountsStats {
    total_operations: u64,
    successful_operations: u64,
    failed_operations: u64,
    last_operation_ns: u64,
    max_operation_ns: u64,
    min_operation_ns: u64,
    total_time_ns: u64,
}

impl WbAccountsStats {
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
    pub fn merge(&mut self, other: &WbAccountsStats) {
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

impl Default for WbAccountsStats {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for WbAccountsStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "WbAccountsStats(total={}, ok={}, err={}, avg_ns={})",
            self.total_operations,
            self.successful_operations,
            self.failed_operations,
            self.average_time_ns()
        )
    }
}

/// Validation utilities for wb-accounts.
#[derive(Debug, Clone)]
pub struct WbAccountsValidator {
    max_name_length: usize,
    allowed_chars: Option<Vec<char>>,
    forbidden_prefixes: Vec<String>,
}

impl WbAccountsValidator {
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

impl Default for WbAccountsValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// AccountSession management with token refresh
// ---------------------------------------------------------------------------

/// Represents a session with an expiration timestamp and refresh capability.
#[derive(Debug, Clone)]
pub struct AccountSession {
    pub session: AuthSession,
    /// Unix timestamp (seconds) when the token expires. `None` means no expiry.
    pub expires_at: Option<u64>,
    /// The refresh token, if available.
    pub refresh_token: Option<String>,
}

impl AccountSession {
    /// Create a new session with optional expiry and refresh token.
    pub fn new(session: AuthSession, expires_at: Option<u64>, refresh_token: Option<String>) -> Self {
        Self {
            session,
            expires_at,
            refresh_token,
        }
    }

    /// Check if the session's access token has expired, given the current time.
    pub fn is_expired(&self, now: u64) -> bool {
        match self.expires_at {
            Some(exp) => now >= exp,
            None => false,
        }
    }

    /// Check if the session is nearing expiry (within `buffer_secs` seconds).
    pub fn needs_refresh(&self, now: u64, buffer_secs: u64) -> bool {
        match self.expires_at {
            Some(exp) => now + buffer_secs >= exp,
            None => false,
        }
    }

    /// Whether this session has a refresh token available.
    pub fn can_refresh(&self) -> bool {
        self.refresh_token.is_some()
    }

    /// Simulate refreshing the token: update the access token, expiry, and optionally a new refresh token.
    pub fn refresh(&mut self, new_access_token: String, new_expires_at: Option<u64>, new_refresh_token: Option<String>) {
        self.session.access_token = new_access_token;
        self.expires_at = new_expires_at;
        if let Some(rt) = new_refresh_token {
            self.refresh_token = Some(rt);
        }
    }

    /// Time remaining until expiry (in seconds). Returns 0 if already expired or no expiry set.
    pub fn time_remaining(&self, now: u64) -> u64 {
        match self.expires_at {
            Some(exp) if exp > now => exp - now,
            _ => 0,
        }
    }

    /// Returns the account display name for convenience.
    pub fn display_name(&self) -> &str {
        self.session.account.display_name()
    }
}

impl fmt::Display for AccountSession {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "AccountSession({}", self.session.account)?;
        if let Some(exp) = self.expires_at {
            write!(f, ", expires={exp}")?;
        }
        write!(f, ")")
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AccountsSummary {
    pub total_sessions: usize,
    pub providers_count: usize,
    pub sessions_per_provider: Vec<(String, usize)>,
    pub total_scopes: usize,
    pub sessions_with_email: usize,
}

impl AccountsSummary {
    pub fn has_provider(&self, provider_id: &str) -> bool {
        self.sessions_per_provider.iter().any(|(p, _)| p == provider_id)
    }

    pub fn count_for_provider(&self, provider_id: &str) -> usize {
        self.sessions_per_provider
            .iter()
            .find(|(p, _)| p == provider_id)
            .map(|(_, c)| *c)
            .unwrap_or(0)
    }

    pub fn average_sessions_per_provider(&self) -> f64 {
        if self.providers_count == 0 {
            return 0.0;
        }
        self.total_sessions as f64 / self.providers_count as f64
    }

    pub fn max_sessions_provider(&self) -> Option<(&str, usize)> {
        self.sessions_per_provider
            .iter()
            .max_by_key(|(_, c)| *c)
            .map(|(p, c)| (p.as_str(), *c))
    }

    pub fn min_sessions_provider(&self) -> Option<(&str, usize)> {
        self.sessions_per_provider
            .iter()
            .min_by_key(|(_, c)| *c)
            .map(|(p, c)| (p.as_str(), *c))
    }
}

impl fmt::Display for AccountsSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "AccountsSummary(sessions={}, providers={}, scopes={}, with_email={})",
            self.total_sessions, self.providers_count, self.total_scopes, self.sessions_with_email,
        )
    }
}

impl AccountInfo {
    pub fn matches_filter(&self, query: &str) -> bool {
        let q = query.to_lowercase();
        self.id.to_lowercase().contains(&q)
            || self.label.to_lowercase().contains(&q)
            || self.provider_id.to_lowercase().contains(&q)
            || self
                .email
                .as_ref()
                .map(|e| e.to_lowercase().contains(&q))
                .unwrap_or(false)
    }
}

impl AccountsService {
    pub fn providers(&self) -> Vec<String> {
        let mut providers: Vec<String> = self
            .sessions
            .iter()
            .map(|s| s.account.provider_id.clone())
            .collect();
        providers.sort_unstable();
        providers.dedup();
        providers
    }

    pub fn summary(&self) -> AccountsSummary {
        let mut counts: HashMap<String, usize> = HashMap::new();
        let mut total_scopes = 0;
        let mut sessions_with_email = 0;
        for session in &self.sessions {
            *counts.entry(session.account.provider_id.clone()).or_insert(0) += 1;
            total_scopes += session.scopes.len();
            if session.account.has_email() {
                sessions_with_email += 1;
            }
        }
        let mut sessions_per_provider: Vec<(String, usize)> = counts.into_iter().collect();
        sessions_per_provider.sort_by(|a, b| a.0.cmp(&b.0));
        AccountsSummary {
            total_sessions: self.sessions.len(),
            providers_count: sessions_per_provider.len(),
            sessions_per_provider,
            total_scopes,
            sessions_with_email,
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &AuthSession> {
        self.sessions.iter()
    }

    pub fn find_by_label(&self, label: &str) -> Vec<&AuthSession> {
        self.sessions
            .iter()
            .filter(|s| s.account.label == label)
            .collect()
    }

    pub fn find_by_filter(&self, query: &str) -> Vec<&AuthSession> {
        self.sessions
            .iter()
            .filter(|s| s.account.matches_filter(query))
            .collect()
    }
}

impl<'a> IntoIterator for &'a AccountsService {
    type Item = &'a AuthSession;
    type IntoIter = std::slice::Iter<'a, AuthSession>;

    fn into_iter(self) -> Self::IntoIter {
        self.sessions.iter()
    }
}

// ---------------------------------------------------------------------------
// ScopeSet – set operations over scope strings
// ---------------------------------------------------------------------------

/// A set of OAuth/API scopes with set-theoretic operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopeSet {
    inner: BTreeSet<String>,
}

impl ScopeSet {
    /// Create an empty scope set.
    pub fn new() -> Self {
        Self {
            inner: BTreeSet::new(),
        }
    }

    /// Create a scope set from a slice of scope strings.
    pub fn from_scopes(scopes: &[&str]) -> Self {
        Self {
            inner: scopes.iter().map(|s| s.to_string()).collect(),
        }
    }

    /// Insert a scope. Returns `true` if the scope was newly inserted.
    pub fn insert(&mut self, scope: impl Into<String>) -> bool {
        self.inner.insert(scope.into())
    }

    /// Returns `true` if the set contains the given scope.
    pub fn contains(&self, scope: &str) -> bool {
        self.inner.contains(scope)
    }

    /// Number of scopes in the set.
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Returns `true` if the set is empty.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Returns the union of this set with `other`.
    pub fn union(&self, other: &ScopeSet) -> ScopeSet {
        ScopeSet {
            inner: self.inner.union(&other.inner).cloned().collect(),
        }
    }

    /// Returns the intersection of this set with `other`.
    pub fn intersection(&self, other: &ScopeSet) -> ScopeSet {
        ScopeSet {
            inner: self.inner.intersection(&other.inner).cloned().collect(),
        }
    }

    /// Returns scopes in `self` that are not in `other`.
    pub fn difference(&self, other: &ScopeSet) -> ScopeSet {
        ScopeSet {
            inner: self.inner.difference(&other.inner).cloned().collect(),
        }
    }

    /// Returns `true` if every scope in `self` is also in `other`.
    pub fn is_subset(&self, other: &ScopeSet) -> bool {
        self.inner.is_subset(&other.inner)
    }

    /// Iterate over scopes in sorted order.
    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.inner.iter().map(|s| s.as_str())
    }
}

impl Default for ScopeSet {
    fn default() -> Self {
        Self::new()
    }
}

impl From<&[String]> for ScopeSet {
    fn from(scopes: &[String]) -> Self {
        Self {
            inner: scopes.iter().cloned().collect(),
        }
    }
}

impl From<Vec<String>> for ScopeSet {
    fn from(scopes: Vec<String>) -> Self {
        Self {
            inner: scopes.into_iter().collect(),
        }
    }
}

impl fmt::Display for ScopeSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let scopes: Vec<&str> = self.iter().collect();
        write!(f, "{{{}}}", scopes.join(", "))
    }
}

// ---------------------------------------------------------------------------
// SessionValidator – validates sessions against configurable rules
// ---------------------------------------------------------------------------

/// Validates [`AuthSession`] instances against configurable rules.
#[derive(Debug, Clone)]
pub struct SessionValidator {
    required_scopes: ScopeSet,
    allowed_providers: Option<Vec<String>>,
    require_email: bool,
    require_non_empty_token: bool,
}

impl SessionValidator {
    /// Create a validator with default settings (only checks for non-empty token).
    pub fn new() -> Self {
        Self {
            required_scopes: ScopeSet::new(),
            allowed_providers: None,
            require_email: false,
            require_non_empty_token: true,
        }
    }

    /// Require that sessions include all of the given scopes.
    pub fn require_scopes(mut self, scopes: &[&str]) -> Self {
        self.required_scopes = ScopeSet::from_scopes(scopes);
        self
    }

    /// Restrict sessions to a set of allowed provider IDs.
    pub fn allowed_providers(mut self, providers: &[&str]) -> Self {
        self.allowed_providers = Some(providers.iter().map(|s| s.to_string()).collect());
        self
    }

    /// Require that the account has an email address.
    pub fn require_email(mut self) -> Self {
        self.require_email = true;
        self
    }

    /// Validate a session, returning a list of validation errors (empty = valid).
    pub fn validate(&self, session: &AuthSession) -> Vec<AccountError> {
        let mut errors = Vec::new();
        if self.require_non_empty_token && session.access_token.is_empty() {
            errors.push(AccountError::InvalidToken);
        }
        for scope in self.required_scopes.iter() {
            if !session.has_scope(scope) {
                errors.push(AccountError::MissingScope(scope.to_string()));
            }
        }
        if let Some(ref allowed) = self.allowed_providers {
            if !allowed.iter().any(|p| p == &session.account.provider_id) {
                errors.push(AccountError::ProviderNotFound(
                    session.account.provider_id.clone(),
                ));
            }
        }
        if self.require_email && session.account.email.is_none() {
            errors.push(AccountError::SessionNotFound(
                "account email is required".to_string(),
            ));
        }
        errors
    }

    /// Returns `true` if the session passes all validation rules.
    pub fn is_valid(&self, session: &AuthSession) -> bool {
        self.validate(session).is_empty()
    }
}

impl Default for SessionValidator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// AccountMatcher – match accounts by pattern
// ---------------------------------------------------------------------------

/// Matches [`AccountInfo`] instances by various patterns.
#[derive(Debug, Clone)]
pub struct AccountMatcher {
    email_domain: Option<String>,
    provider: Option<String>,
    label_substring: Option<String>,
}

impl AccountMatcher {
    /// Create a matcher with no filters (matches everything).
    pub fn new() -> Self {
        Self {
            email_domain: None,
            provider: None,
            label_substring: None,
        }
    }

    /// Only match accounts whose email ends with `@domain`.
    pub fn email_domain(mut self, domain: impl Into<String>) -> Self {
        self.email_domain = Some(domain.into());
        self
    }

    /// Only match accounts with the given provider ID.
    pub fn provider(mut self, provider: impl Into<String>) -> Self {
        self.provider = Some(provider.into());
        self
    }

    /// Only match accounts whose label contains the given substring (case-insensitive).
    pub fn label_contains(mut self, substring: impl Into<String>) -> Self {
        self.label_substring = Some(substring.into());
        self
    }

    /// Returns `true` if the account matches all configured filters.
    pub fn matches(&self, account: &AccountInfo) -> bool {
        if let Some(ref domain) = self.email_domain {
            let suffix = format!("@{domain}");
            match &account.email {
                Some(email) => {
                    if !email.to_lowercase().ends_with(&suffix.to_lowercase()) {
                        return false;
                    }
                }
                None => return false,
            }
        }
        if let Some(ref provider) = self.provider {
            if account.provider_id != *provider {
                return false;
            }
        }
        if let Some(ref sub) = self.label_substring {
            if !account.label.to_lowercase().contains(&sub.to_lowercase()) {
                return false;
            }
        }
        true
    }

    /// Filter a slice of sessions, returning those whose account matches.
    pub fn filter_sessions<'a>(&self, sessions: &'a [AuthSession]) -> Vec<&'a AuthSession> {
        sessions
            .iter()
            .filter(|s| self.matches(&s.account))
            .collect()
    }
}

impl Default for AccountMatcher {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// SessionSummary – aggregate statistics from a collection of sessions
// ---------------------------------------------------------------------------

/// Aggregate statistics computed from a slice of [`AuthSession`] instances.
#[derive(Debug, Clone, PartialEq)]
pub struct SessionSummary {
    pub counts_by_provider: Vec<(String, usize)>,
    pub total_scopes: usize,
    pub unique_emails: Vec<String>,
    pub total_sessions: usize,
}

impl SessionSummary {
    /// Build a summary from a slice of sessions.
    pub fn from_sessions(sessions: &[AuthSession]) -> Self {
        let mut provider_counts: HashMap<&str, usize> = HashMap::new();
        let mut scope_set: BTreeSet<&str> = BTreeSet::new();
        let mut email_set: BTreeSet<&str> = BTreeSet::new();

        for session in sessions {
            *provider_counts
                .entry(session.account.provider_id.as_str())
                .or_insert(0) += 1;
            for scope in &session.scopes {
                scope_set.insert(scope.as_str());
            }
            if let Some(ref email) = session.account.email {
                email_set.insert(email.as_str());
            }
        }

        let mut counts_by_provider: Vec<(String, usize)> = provider_counts
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect();
        counts_by_provider.sort_by(|a, b| a.0.cmp(&b.0));

        SessionSummary {
            counts_by_provider,
            total_scopes: scope_set.len(),
            unique_emails: email_set.into_iter().map(|s| s.to_string()).collect(),
            total_sessions: sessions.len(),
        }
    }

    /// Number of distinct providers.
    pub fn provider_count(&self) -> usize {
        self.counts_by_provider.len()
    }

    /// Number of unique email addresses.
    pub fn unique_email_count(&self) -> usize {
        self.unique_emails.len()
    }
}

impl fmt::Display for SessionSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "SessionSummary(sessions={}, providers={}, scopes={}, emails={})",
            self.total_sessions,
            self.provider_count(),
            self.total_scopes,
            self.unique_email_count(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_session(id: &str, provider: &str) -> AuthSession {
        AuthSession {
            id: id.to_string(),
            account: AccountInfo {
                id: format!("acct-{id}"),
                label: format!("User {id}"),
                provider_id: provider.to_string(),
                email: None,
            },
            scopes: vec!["read".to_string()],
            access_token: format!("token-{id}"),
        }
    }

    #[test]
    fn add_and_query_sessions() {
        let mut svc = AccountsService::new();
        svc.add_session(make_session("s1", "github"));
        svc.add_session(make_session("s2", "github"));
        svc.add_session(make_session("s3", "azure"));
        assert_eq!(svc.session_count(), 3);
        assert_eq!(svc.get_sessions("github").len(), 2);
        assert_eq!(svc.get_sessions("azure").len(), 1);
    }

    #[test]
    fn remove_session() {
        let mut svc = AccountsService::new();
        svc.add_session(make_session("s1", "github"));
        assert!(svc.remove_session("s1"));
        assert!(!svc.remove_session("s1"));
        assert_eq!(svc.session_count(), 0);
    }

    #[test]
    fn is_authenticated() {
        let mut svc = AccountsService::new();
        assert!(!svc.is_authenticated("github"));
        svc.add_session(make_session("s1", "github"));
        assert!(svc.is_authenticated("github"));
        assert!(!svc.is_authenticated("azure"));
    }

    #[test]
    fn get_session_by_id() {
        let mut svc = AccountsService::new();
        svc.add_session(make_session("s1", "github"));
        assert!(svc.get_session_by_id("s1").is_some());
        assert!(svc.get_session_by_id("missing").is_none());
    }

    #[test]
    fn display_name_returns_label() {
        let info = AccountInfo {
            id: "a1".to_string(),
            label: "Alice".to_string(),
            provider_id: "github".to_string(),
            email: None,
        };
        assert_eq!(info.display_name(), "Alice");
    }

    #[test]
    fn account_info_display() {
        let info = AccountInfo {
            id: "a1".to_string(),
            label: "Alice".to_string(),
            provider_id: "github".to_string(),
            email: None,
        };
        assert_eq!(format!("{info}"), "Alice (github)");
    }

    #[test]
    fn auth_session_display() {
        let session = AuthSession {
            id: "s1".to_string(),
            account: AccountInfo {
                id: "a1".to_string(),
                label: "Alice".to_string(),
                provider_id: "github".to_string(),
                email: None,
            },
            scopes: vec!["read".to_string(), "write".to_string()],
            access_token: "tok".to_string(),
        };
        assert_eq!(format!("{session}"), "Alice (github) - read, write");
    }

    #[test]
    fn has_scope_and_has_all_scopes() {
        let session = AuthSession {
            id: "s1".to_string(),
            account: AccountInfo {
                id: "a1".to_string(),
                label: "Alice".to_string(),
                provider_id: "github".to_string(),
                email: None,
            },
            scopes: vec!["read".to_string(), "write".to_string()],
            access_token: "tok".to_string(),
        };
        assert!(session.has_scope("read"));
        assert!(session.has_scope("write"));
        assert!(!session.has_scope("admin"));
        assert!(session.has_all_scopes(&["read", "write"]));
        assert!(!session.has_all_scopes(&["read", "admin"]));
        assert!(session.has_all_scopes(&[]));
    }

    #[test]
    fn get_all_sessions() {
        let mut svc = AccountsService::new();
        assert!(svc.get_all_sessions().is_empty());
        svc.add_session(make_session("s1", "github"));
        svc.add_session(make_session("s2", "azure"));
        assert_eq!(svc.get_all_sessions().len(), 2);
    }

    fn make_session_with_scopes(id: &str, provider: &str, scopes: &[&str]) -> AuthSession {
        AuthSession {
            id: id.to_string(),
            account: AccountInfo {
                id: format!("acct-{id}"),
                label: format!("User {id}"),
                provider_id: provider.to_string(),
                email: None,
            },
            scopes: scopes.iter().map(|s| s.to_string()).collect(),
            access_token: format!("token-{id}"),
        }
    }

    #[test]
    fn find_session_with_scopes() {
        let mut svc = AccountsService::new();
        svc.add_session(make_session_with_scopes("s1", "github", &["read"]));
        svc.add_session(make_session_with_scopes("s2", "github", &["read", "write"]));
        svc.add_session(make_session_with_scopes("s3", "azure", &["read"]));

        let found = svc.find_session_with_scopes("github", &["read", "write"]);
        assert_eq!(found.unwrap().id, "s2");

        assert!(svc.find_session_with_scopes("github", &["admin"]).is_none());
        assert!(svc.find_session_with_scopes("azure", &["write"]).is_none());
    }

    #[test]
    fn get_unique_providers() {
        let mut svc = AccountsService::new();
        svc.add_session(make_session("s1", "github"));
        svc.add_session(make_session("s2", "github"));
        svc.add_session(make_session("s3", "azure"));
        let providers = svc.get_unique_providers();
        assert_eq!(providers.len(), 2);
        assert!(providers.contains(&"github"));
        assert!(providers.contains(&"azure"));
    }

    #[test]
    fn remove_sessions_for_provider() {
        let mut svc = AccountsService::new();
        svc.add_session(make_session("s1", "github"));
        svc.add_session(make_session("s2", "github"));
        svc.add_session(make_session("s3", "azure"));
        assert_eq!(svc.remove_sessions_for_provider("github"), 2);
        assert_eq!(svc.session_count(), 1);
        assert_eq!(svc.remove_sessions_for_provider("missing"), 0);
    }

    #[test]
    fn update_token() {
        let mut svc = AccountsService::new();
        svc.add_session(make_session("s1", "github"));
        assert!(svc.update_token("s1", "new-token".to_string()));
        assert_eq!(svc.get_session_by_id("s1").unwrap().access_token, "new-token");
        assert!(!svc.update_token("missing", "x".to_string()));
    }

    #[test]
    fn is_session_valid() {
        let mut svc = AccountsService::new();
        svc.add_session(make_session("s1", "github"));
        assert!(svc.is_session_valid("s1"));
        assert!(!svc.is_session_valid("missing"));
    }

    #[test]
    fn account_info_new_constructor() {
        let info = AccountInfo::new("a1", "Alice", "github", Some("alice@example.com".to_string()));
        assert_eq!(info.id, "a1");
        assert_eq!(info.label, "Alice");
        assert_eq!(info.provider_id, "github");
        assert!(info.has_email());
        assert_eq!(info.email_or_default(), "alice@example.com");
    }

    #[test]
    fn account_info_no_email() {
        let info = AccountInfo::new("a1", "Bob", "github", None);
        assert!(!info.has_email());
        assert_eq!(info.email_or_default(), "(no email)");
    }

    #[test]
    fn auth_session_new_constructor() {
        let account = AccountInfo::new("a1", "Alice", "github", None);
        let session = AuthSession::new("s1", account, vec!["read".to_string()], "tok123");
        assert_eq!(session.id, "s1");
        assert_eq!(session.scope_count(), 1);
        assert!(session.has_valid_token());
        assert_eq!(session.scopes_display(), "read");
    }

    #[test]
    fn auth_session_empty_token() {
        let account = AccountInfo::new("a1", "Alice", "github", None);
        let session = AuthSession::new("s1", account, vec![], "");
        assert!(!session.has_valid_token());
    }

    #[test]
    fn auth_session_partial_eq_by_id() {
        let a1 = AccountInfo::new("a1", "Alice", "github", None);
        let a2 = AccountInfo::new("a2", "Bob", "azure", None);
        let s1 = AuthSession::new("s1", a1, vec![], "tok1");
        let s2 = AuthSession::new("s1", a2, vec![], "tok2");
        assert_eq!(s1, s2); // same id
    }

    #[test]
    fn try_add_duplicate_session() {
        let mut svc = AccountsService::new();
        svc.add_session(make_session("s1", "github"));
        let result = svc.try_add_session(make_session("s1", "azure"));
        assert_eq!(result, Err(AccountError::DuplicateSession("s1".to_string())));
    }

    #[test]
    fn try_add_session_success() {
        let mut svc = AccountsService::new();
        assert!(svc.try_add_session(make_session("s1", "github")).is_ok());
        assert_eq!(svc.session_count(), 1);
    }

    #[test]
    fn try_remove_session_success() {
        let mut svc = AccountsService::new();
        svc.add_session(make_session("s1", "github"));
        let removed = svc.try_remove_session("s1").unwrap();
        assert_eq!(removed.id, "s1");
        assert_eq!(svc.session_count(), 0);
    }

    #[test]
    fn try_remove_session_not_found() {
        let mut svc = AccountsService::new();
        assert_eq!(
            svc.try_remove_session("missing"),
            Err(AccountError::SessionNotFound("missing".to_string()))
        );
    }

    #[test]
    fn session_ids_list() {
        let mut svc = AccountsService::new();
        svc.add_session(make_session("s1", "github"));
        svc.add_session(make_session("s2", "azure"));
        let ids = svc.session_ids();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&"s1"));
        assert!(ids.contains(&"s2"));
    }

    #[test]
    fn find_by_email_match() {
        let mut svc = AccountsService::new();
        let mut session = make_session("s1", "github");
        session.account.email = Some("alice@example.com".to_string());
        svc.add_session(session);
        svc.add_session(make_session("s2", "github"));
        let found = svc.find_by_email("alice@example.com");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id, "s1");
    }

    #[test]
    fn any_session_has_scope_check() {
        let mut svc = AccountsService::new();
        svc.add_session(make_session("s1", "github")); // has "read" scope
        assert!(svc.any_session_has_scope("read"));
        assert!(!svc.any_session_has_scope("admin"));
    }

    #[test]
    fn total_unique_scopes_count() {
        let mut svc = AccountsService::new();
        svc.add_session(make_session_with_scopes("s1", "github", &["read", "write"]));
        svc.add_session(make_session_with_scopes("s2", "azure", &["read", "admin"]));
        assert_eq!(svc.total_unique_scopes(), 3); // read, write, admin
    }

    #[test]
    fn validate_session_token_ok() {
        let mut svc = AccountsService::new();
        svc.add_session(make_session("s1", "github"));
        assert!(svc.validate_session_token("s1").is_ok());
    }

    #[test]
    fn validate_session_token_not_found() {
        let svc = AccountsService::new();
        assert_eq!(
            svc.validate_session_token("missing"),
            Err(AccountError::SessionNotFound("missing".to_string()))
        );
    }

    #[test]
    fn validate_session_token_empty() {
        let mut svc = AccountsService::new();
        let account = AccountInfo::new("a1", "User", "github", None);
        let session = AuthSession::new("s1", account, vec![], "");
        svc.add_session(session);
        assert_eq!(svc.validate_session_token("s1"), Err(AccountError::InvalidToken));
    }

    #[test]
    fn accounts_service_display() {
        let mut svc = AccountsService::new();
        svc.add_session(make_session("s1", "github"));
        svc.add_session(make_session("s2", "azure"));
        let s = format!("{svc}");
        assert!(s.contains("2 sessions"));
        assert!(s.contains("2 providers"));
    }

    #[test]
    fn account_error_display_messages() {
        assert_eq!(
            AccountError::SessionNotFound("x".to_string()).to_string(),
            "session not found: x"
        );
        assert_eq!(
            AccountError::ProviderNotFound("y".to_string()).to_string(),
            "provider not found: y"
        );
        assert_eq!(
            AccountError::DuplicateSession("z".to_string()).to_string(),
            "duplicate session: z"
        );
        assert_eq!(AccountError::InvalidToken.to_string(), "invalid or empty token");
        assert_eq!(
            AccountError::MissingScope("admin".to_string()).to_string(),
            "missing required scope: admin"
        );
    }

    #[test]
    fn account_error_is_std_error() {
        let err: Box<dyn std::error::Error> = Box::new(AccountError::InvalidToken);
        assert_eq!(err.to_string(), "invalid or empty token");
    }

    #[test]
    fn wb_accounts_stats_new_defaults() {
        let stats = WbAccountsStats::new();
        assert_eq!(stats.total(), 0);
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);
        assert_eq!(stats.average_time_ns(), 0);
        assert_eq!(stats.min_time_ns(), None);
        assert_eq!(stats.max_time_ns(), None);
    }

    #[test]
    fn wb_accounts_stats_record_success() {
        let mut stats = WbAccountsStats::new();
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
    fn wb_accounts_stats_record_failure() {
        let mut stats = WbAccountsStats::new();
        stats.record_success(100);
        stats.record_failure(300);
        assert_eq!(stats.total(), 2);
        assert_eq!(stats.failed_operations, 1);
        assert!((stats.success_rate() - 0.5).abs() < f64::EPSILON);
        assert!((stats.failure_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn wb_accounts_stats_reset() {
        let mut stats = WbAccountsStats::new();
        stats.record_success(500);
        stats.record_failure(100);
        stats.reset();
        assert_eq!(stats.total(), 0);
        assert_eq!(stats.average_time_ns(), 0);
    }

    #[test]
    fn wb_accounts_stats_merge() {
        let mut a = WbAccountsStats::new();
        a.record_success(100);
        a.record_success(200);
        let mut b = WbAccountsStats::new();
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
    fn wb_accounts_stats_display() {
        let mut stats = WbAccountsStats::new();
        stats.record_success(100);
        let s = format!("{stats}");
        assert!(s.contains("total=1"));
        assert!(s.contains("ok=1"));
        assert!(s.contains("err=0"));
    }

    #[test]
    fn wb_accounts_stats_default() {
        let stats = WbAccountsStats::default();
        assert_eq!(stats.total(), 0);
    }

    #[test]
    fn wb_accounts_validator_accepts_valid_name() {
        let v = WbAccountsValidator::new();
        assert!(v.validate_name("hello_world").is_ok());
    }

    #[test]
    fn wb_accounts_validator_rejects_empty() {
        let v = WbAccountsValidator::new();
        assert!(v.validate_name("").is_err());
    }

    #[test]
    fn wb_accounts_validator_rejects_too_long() {
        let v = WbAccountsValidator::new().max_length(5);
        assert!(v.validate_name("toolong").is_err());
        assert!(v.validate_name("ok").is_ok());
    }

    #[test]
    fn wb_accounts_validator_forbidden_prefix() {
        let v = WbAccountsValidator::new().forbid_prefix("__");
        assert!(v.validate_name("__internal").is_err());
        assert!(v.validate_name("public").is_ok());
    }

    #[test]
    fn wb_accounts_validator_allowed_chars() {
        let v = WbAccountsValidator::new().allowed_chars(&['a', 'b', 'c']);
        assert!(v.validate_name("abc").is_ok());
        assert!(v.validate_name("abcd").is_err());
    }

    #[test]
    fn wb_accounts_validator_range() {
        let v = WbAccountsValidator::new();
        assert!(v.validate_range(5, 0, 10).is_ok());
        assert!(v.validate_range(-1, 0, 10).is_err());
        assert!(v.validate_range(11, 0, 10).is_err());
    }

    #[test]
    fn wb_accounts_sanitize_removes_control() {
        let result = WbAccountsValidator::sanitize("hello\x00world\x07");
        assert_eq!(result, "helloworld");
    }

    #[test]
    fn wb_accounts_truncate_short_string() {
        assert_eq!(WbAccountsValidator::truncate("hi", 10), "hi");
    }

    #[test]
    fn wb_accounts_truncate_long_string() {
        let result = WbAccountsValidator::truncate("hello world", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.ends_with("…"));
    }

    #[test]
    fn wb_accounts_is_ascii_printable() {
        assert!(WbAccountsValidator::is_ascii_printable("Hello World 123"));
        assert!(!WbAccountsValidator::is_ascii_printable("Hello\x00World"));
    }

    fn make_test_account_session(expires_at: Option<u64>) -> AccountSession {
        let account = AccountInfo {
            id: "acc1".to_string(),
            label: "Test User".to_string(),
            provider_id: "github".to_string(),
            email: Some("test@test.com".to_string()),
        };
        let session = AuthSession {
            id: "sess1".to_string(),
            account,
            scopes: vec!["repo".to_string()],
            access_token: "token123".to_string(),
        };
        AccountSession::new(session, expires_at, Some("refresh_abc".to_string()))
    }

    #[test]
    fn account_session_not_expired() {
        let sess = make_test_account_session(Some(2000));
        assert!(!sess.is_expired(1000));
    }

    #[test]
    fn account_session_expired() {
        let sess = make_test_account_session(Some(1000));
        assert!(sess.is_expired(1500));
    }

    #[test]
    fn account_session_no_expiry() {
        let sess = make_test_account_session(None);
        assert!(!sess.is_expired(99999));
    }

    #[test]
    fn account_session_needs_refresh() {
        let sess = make_test_account_session(Some(1100));
        assert!(sess.needs_refresh(1000, 200)); // 1000 + 200 >= 1100
        assert!(!sess.needs_refresh(800, 200)); // 800 + 200 < 1100
    }

    #[test]
    fn account_session_refresh() {
        let mut sess = make_test_account_session(Some(1000));
        sess.refresh("new_token".to_string(), Some(2000), Some("new_refresh".to_string()));
        assert_eq!(sess.session.access_token, "new_token");
        assert_eq!(sess.expires_at, Some(2000));
        assert_eq!(sess.refresh_token.as_deref(), Some("new_refresh"));
    }

    #[test]
    fn account_session_time_remaining() {
        let sess = make_test_account_session(Some(2000));
        assert_eq!(sess.time_remaining(1500), 500);
        assert_eq!(sess.time_remaining(2500), 0);
    }

    #[test]
    fn account_session_display_name() {
        let sess = make_test_account_session(Some(1000));
        assert_eq!(sess.display_name(), "Test User");
    }

    #[test]
    fn account_info_matches_filter_by_label() {
        let info = AccountInfo::new("a1", "Alice Smith", "github", None);
        assert!(info.matches_filter("alice"));
        assert!(info.matches_filter("SMITH"));
        assert!(!info.matches_filter("bob"));
    }

    #[test]
    fn account_info_matches_filter_by_provider_and_email() {
        let info = AccountInfo::new("a1", "Alice", "github", Some("alice@corp.com".to_string()));
        assert!(info.matches_filter("github"));
        assert!(info.matches_filter("corp.com"));
        assert!(info.matches_filter("a1"));
        assert!(!info.matches_filter("azure"));
    }

    #[test]
    fn accounts_service_providers_owned() {
        let mut svc = AccountsService::new();
        svc.add_session(make_session("s1", "github"));
        svc.add_session(make_session("s2", "azure"));
        svc.add_session(make_session("s3", "github"));
        let providers = svc.providers();
        assert_eq!(providers, vec!["azure".to_string(), "github".to_string()]);
    }

    #[test]
    fn accounts_service_summary() {
        let mut svc = AccountsService::new();
        let mut s1 = make_session("s1", "github");
        s1.account.email = Some("alice@example.com".to_string());
        svc.add_session(s1);
        svc.add_session(make_session("s2", "github"));
        svc.add_session(make_session("s3", "azure"));
        let summary = svc.summary();
        assert_eq!(summary.total_sessions, 3);
        assert_eq!(summary.providers_count, 2);
        assert_eq!(summary.sessions_with_email, 1);
        assert_eq!(summary.count_for_provider("github"), 2);
        assert_eq!(summary.count_for_provider("azure"), 1);
        assert_eq!(summary.count_for_provider("missing"), 0);
        assert!(summary.has_provider("github"));
        assert!(!summary.has_provider("bitbucket"));
        assert!((summary.average_sessions_per_provider() - 1.5).abs() < f64::EPSILON);
        let (max_p, max_c) = summary.max_sessions_provider().unwrap();
        assert_eq!(max_p, "github");
        assert_eq!(max_c, 2);
        let (min_p, min_c) = summary.min_sessions_provider().unwrap();
        assert_eq!(min_p, "azure");
        assert_eq!(min_c, 1);
        let display = format!("{summary}");
        assert!(display.contains("sessions=3"));
    }

    #[test]
    fn accounts_service_iter_and_into_iter() {
        let mut svc = AccountsService::new();
        svc.add_session(make_session("s1", "github"));
        svc.add_session(make_session("s2", "azure"));
        let ids_iter: Vec<&str> = svc.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids_iter.len(), 2);
        let ids_into: Vec<&str> = (&svc).into_iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids_into, ids_iter);
    }

    #[test]
    fn accounts_service_find_by_label_and_filter() {
        let mut svc = AccountsService::new();
        svc.add_session(make_session("s1", "github"));
        svc.add_session(make_session("s2", "azure"));
        let found = svc.find_by_label("User s1");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id, "s1");
        assert!(svc.find_by_label("Nobody").is_empty());
        let filtered = svc.find_by_filter("azure");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, "s2");
        let all = svc.find_by_filter("User");
        assert_eq!(all.len(), 2);
    }

    // -----------------------------------------------------------------------
    // ScopeSet tests
    // -----------------------------------------------------------------------

    #[test]
    fn scope_set_union_intersection_difference() {
        let a = ScopeSet::from_scopes(&["read", "write"]);
        let b = ScopeSet::from_scopes(&["write", "admin"]);

        let union = a.union(&b);
        assert_eq!(union.len(), 3);
        assert!(union.contains("read"));
        assert!(union.contains("write"));
        assert!(union.contains("admin"));

        let inter = a.intersection(&b);
        assert_eq!(inter.len(), 1);
        assert!(inter.contains("write"));

        let diff = a.difference(&b);
        assert_eq!(diff.len(), 1);
        assert!(diff.contains("read"));
        assert!(!diff.contains("write"));

        assert!(!a.is_subset(&b));
        let subset = ScopeSet::from_scopes(&["write"]);
        assert!(subset.is_subset(&a));
    }

    #[test]
    fn scope_set_display_and_from() {
        let scopes = vec!["beta".to_string(), "alpha".to_string()];
        let set = ScopeSet::from(scopes);
        // BTreeSet sorts, so alpha < beta
        assert_eq!(format!("{set}"), "{alpha, beta}");

        let slice_scopes = vec!["x".to_string(), "y".to_string()];
        let set2 = ScopeSet::from(slice_scopes.as_slice());
        assert!(set2.contains("x"));
        assert!(set2.contains("y"));
    }

    // -----------------------------------------------------------------------
    // SessionValidator tests
    // -----------------------------------------------------------------------

    #[test]
    fn session_validator_valid_session() {
        let account = AccountInfo::new("a1", "Alice", "github", Some("a@b.com".to_string()));
        let session = AuthSession::new("s1", account, vec!["read".to_string()], "tok");

        let validator = SessionValidator::new()
            .require_scopes(&["read"])
            .allowed_providers(&["github", "azure"])
            .require_email();

        assert!(validator.is_valid(&session));
        assert!(validator.validate(&session).is_empty());
    }

    #[test]
    fn session_validator_multiple_errors() {
        let account = AccountInfo::new("a1", "Alice", "bitbucket", None);
        let session = AuthSession::new("s1", account, vec![], "");

        let validator = SessionValidator::new()
            .require_scopes(&["admin"])
            .allowed_providers(&["github"])
            .require_email();

        let errors = validator.validate(&session);
        assert!(!validator.is_valid(&session));
        // Should have: InvalidToken, MissingScope("admin"), ProviderNotFound, missing email
        assert!(errors.iter().any(|e| matches!(e, AccountError::InvalidToken)));
        assert!(errors.iter().any(|e| matches!(e, AccountError::MissingScope(_))));
        assert!(errors.iter().any(|e| matches!(e, AccountError::ProviderNotFound(_))));
        assert_eq!(errors.len(), 4);
    }

    // -----------------------------------------------------------------------
    // AccountMatcher tests
    // -----------------------------------------------------------------------

    #[test]
    fn account_matcher_filters() {
        let sessions = vec![
            AuthSession::new(
                "s1",
                AccountInfo::new("a1", "Alice Smith", "github", Some("alice@corp.com".to_string())),
                vec!["read".to_string()],
                "tok1",
            ),
            AuthSession::new(
                "s2",
                AccountInfo::new("a2", "Bob Jones", "azure", Some("bob@example.com".to_string())),
                vec!["write".to_string()],
                "tok2",
            ),
            AuthSession::new(
                "s3",
                AccountInfo::new("a3", "Carol Smith", "github", None),
                vec!["read".to_string()],
                "tok3",
            ),
        ];

        // Match by email domain
        let by_domain = AccountMatcher::new().email_domain("corp.com");
        let matched = by_domain.filter_sessions(&sessions);
        assert_eq!(matched.len(), 1);
        assert_eq!(matched[0].id, "s1");

        // Match by provider
        let by_provider = AccountMatcher::new().provider("azure");
        let matched = by_provider.filter_sessions(&sessions);
        assert_eq!(matched.len(), 1);
        assert_eq!(matched[0].id, "s2");

        // Match by label substring
        let by_label = AccountMatcher::new().label_contains("smith");
        let matched = by_label.filter_sessions(&sessions);
        assert_eq!(matched.len(), 2);

        // Combined filters
        let combined = AccountMatcher::new()
            .provider("github")
            .label_contains("alice");
        let matched = combined.filter_sessions(&sessions);
        assert_eq!(matched.len(), 1);
        assert_eq!(matched[0].id, "s1");

        // No email means domain filter excludes
        let no_email = AccountMatcher::new().email_domain("any.com");
        assert!(!no_email.matches(&sessions[2].account));
    }

    // -----------------------------------------------------------------------
    // SessionSummary tests
    // -----------------------------------------------------------------------

    #[test]
    fn session_summary_aggregates() {
        let sessions = vec![
            AuthSession::new(
                "s1",
                AccountInfo::new("a1", "Alice", "github", Some("alice@x.com".to_string())),
                vec!["read".to_string(), "write".to_string()],
                "tok1",
            ),
            AuthSession::new(
                "s2",
                AccountInfo::new("a2", "Bob", "azure", Some("bob@y.com".to_string())),
                vec!["read".to_string(), "admin".to_string()],
                "tok2",
            ),
            AuthSession::new(
                "s3",
                AccountInfo::new("a3", "Carol", "github", Some("alice@x.com".to_string())),
                vec!["read".to_string()],
                "tok3",
            ),
        ];

        let summary = SessionSummary::from_sessions(&sessions);
        assert_eq!(summary.total_sessions, 3);
        assert_eq!(summary.provider_count(), 2);
        assert_eq!(summary.total_scopes, 3); // read, write, admin
        assert_eq!(summary.unique_email_count(), 2); // alice@x.com, bob@y.com

        let display = format!("{summary}");
        assert!(display.contains("sessions=3"));
        assert!(display.contains("providers=2"));
        assert!(display.contains("scopes=3"));
        assert!(display.contains("emails=2"));
    }
}
