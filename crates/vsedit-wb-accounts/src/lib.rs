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

// ---------------------------------------------------------------------------
// AccountMergerService – merge two AccountInfo records
// ---------------------------------------------------------------------------

/// Service for merging duplicate accounts.
pub struct AccountMergerService {
    merge_log: Vec<(String, String)>,
}

impl AccountMergerService {
    pub fn new() -> Self {
        Self {
            merge_log: Vec::new(),
        }
    }

    /// Returns `true` if both `from_id` and `to_id` exist and are different.
    pub fn can_merge(accounts: &[AccountInfo], from_id: &str, to_id: &str) -> bool {
        if from_id == to_id {
            return false;
        }
        let has_from = accounts.iter().any(|a| a.id == from_id);
        let has_to = accounts.iter().any(|a| a.id == to_id);
        has_from && has_to
    }

    /// Merge the account `from_id` into `to_id`.
    ///
    /// Copies email from `from` to `to` when `to` is missing it, then removes `from`.
    pub fn merge(
        &mut self,
        accounts: &mut Vec<AccountInfo>,
        from_id: &str,
        to_id: &str,
    ) -> Result<(), String> {
        if from_id == to_id {
            return Err("cannot merge an account into itself".to_string());
        }
        let from_idx = accounts
            .iter()
            .position(|a| a.id == from_id)
            .ok_or_else(|| format!("source account '{from_id}' not found"))?;
        let to_idx = accounts
            .iter()
            .position(|a| a.id == to_id)
            .ok_or_else(|| format!("target account '{to_id}' not found"))?;

        let from_email = accounts[from_idx].email.clone();

        if accounts[to_idx].email.is_none() {
            accounts[to_idx].email = from_email;
        }

        accounts.remove(from_idx);
        self.merge_log
            .push((from_id.to_string(), to_id.to_string()));
        Ok(())
    }

    pub fn merge_count(&self) -> usize {
        self.merge_log.len()
    }

    pub fn merge_history(&self) -> &[(String, String)] {
        &self.merge_log
    }
}

impl Default for AccountMergerService {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// AccountSessionRefresher – retry-aware token refresh helper
// ---------------------------------------------------------------------------

/// Result of a refresh attempt.
#[derive(Debug, Clone, PartialEq)]
pub enum RefreshResult {
    Success,
    Retry,
    Exhausted,
}

/// Tracks retry state for session token refreshes.
pub struct AccountSessionRefresher {
    retry_count: u32,
    max_retries: u32,
    last_attempt_ms: Option<u64>,
}

impl AccountSessionRefresher {
    pub fn new(max_retries: u32) -> Self {
        Self {
            retry_count: 0,
            max_retries,
            last_attempt_ms: None,
        }
    }

    /// Attempt a refresh. Returns `Success` while under the limit, `Exhausted` once the
    /// maximum number of retries has been reached.
    pub fn attempt_refresh(&mut self, _session_id: &str, now_ms: u64) -> RefreshResult {
        self.last_attempt_ms = Some(now_ms);
        if self.retry_count < self.max_retries {
            self.retry_count += 1;
            RefreshResult::Success
        } else {
            RefreshResult::Exhausted
        }
    }

    pub fn reset(&mut self) {
        self.retry_count = 0;
        self.last_attempt_ms = None;
    }

    pub fn attempts(&self) -> u32 {
        self.retry_count
    }

    /// Returns `true` if the session has expired relative to `now_ms`.
    pub fn needs_refresh(session: &AccountSession, now_ms: u64) -> bool {
        match session.expires_at {
            Some(exp) => now_ms >= exp,
            None => false,
        }
    }
}

// ---------------------------------------------------------------------------
// AccountProviderPriority – ordering providers by priority
// ---------------------------------------------------------------------------

/// Maps provider IDs to numeric priorities (lower = higher priority).
pub struct AccountProviderPriority {
    priorities: HashMap<String, u32>,
}

impl AccountProviderPriority {
    pub fn new() -> Self {
        Self {
            priorities: HashMap::new(),
        }
    }

    pub fn set_priority(&mut self, provider_id: impl Into<String>, priority: u32) {
        self.priorities.insert(provider_id.into(), priority);
    }

    /// Returns the priority for a provider, defaulting to 100.
    pub fn get_priority(&self, provider_id: &str) -> u32 {
        self.priorities.get(provider_id).copied().unwrap_or(100)
    }

    /// Returns provider IDs sorted by ascending priority.
    pub fn sorted_providers(&self) -> Vec<String> {
        let mut entries: Vec<_> = self.priorities.iter().collect();
        entries.sort_by_key(|(_, p)| **p);
        entries.into_iter().map(|(id, _)| id.clone()).collect()
    }

    /// Returns the provider with the lowest (highest-priority) value.
    pub fn highest_priority(&self) -> Option<String> {
        self.priorities
            .iter()
            .min_by_key(|(_, p)| **p)
            .map(|(id, _)| id.clone())
    }

    pub fn remove(&mut self, provider_id: &str) {
        self.priorities.remove(provider_id);
    }

    pub fn count(&self) -> usize {
        self.priorities.len()
    }
}

impl Default for AccountProviderPriority {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// AccountActivityLogger – activity log with summaries
// ---------------------------------------------------------------------------

/// A single logged activity.
#[derive(Debug, Clone)]
pub struct AccountActivity {
    pub account_id: String,
    pub action: String,
    pub timestamp: u64,
}

impl fmt::Display for AccountActivity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{}] {} @ {}",
            self.account_id, self.action, self.timestamp
        )
    }
}

/// Logger that records account activities and provides query helpers.
pub struct AccountActivityLogger {
    log: Vec<AccountActivity>,
}

impl AccountActivityLogger {
    pub fn new() -> Self {
        Self { log: Vec::new() }
    }

    pub fn log_activity(
        &mut self,
        account_id: impl Into<String>,
        action: impl Into<String>,
        timestamp: u64,
    ) {
        self.log.push(AccountActivity {
            account_id: account_id.into(),
            action: action.into(),
            timestamp,
        });
    }

    pub fn activities_for(&self, account_id: &str) -> Vec<&AccountActivity> {
        self.log
            .iter()
            .filter(|a| a.account_id == account_id)
            .collect()
    }

    /// Returns a slice of the last `n` activities (or all if fewer exist).
    pub fn recent(&self, n: usize) -> &[AccountActivity] {
        let start = self.log.len().saturating_sub(n);
        &self.log[start..]
    }

    pub fn count(&self) -> usize {
        self.log.len()
    }

    pub fn clear(&mut self) {
        self.log.clear();
    }

    /// Returns a count of each distinct action string.
    pub fn actions_summary(&self) -> HashMap<String, usize> {
        let mut map = HashMap::new();
        for entry in &self.log {
            *map.entry(entry.action.clone()).or_insert(0) += 1;
        }
        map
    }
}

impl Default for AccountActivityLogger {
    fn default() -> Self {
        Self::new()
    }
}


// === Account Activity Tracker ===

/// Account Activity Tracker implementation.
#[derive(Debug, Clone)]
pub struct AccountActivityTracker {
    entries: Vec<String>,
    index: HashMap<String, usize>,
    enabled: bool,
    capacity: usize,
    stats: AccountActivityTrackerStats,
}

/// Statistics for AccountActivityTracker.
#[derive(Debug, Clone, Default)]
pub struct AccountActivityTrackerStats {
    pub total_operations: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub last_operation_ms: u64,
}

impl AccountActivityTrackerStats {
    pub fn hit_rate(&self) -> f64 {
        let total = self.cache_hits + self.cache_misses;
        if total == 0 {
            return 0.0;
        }
        self.cache_hits as f64 / total as f64
    }

    pub fn reset(&mut self) {
        self.total_operations = 0;
        self.cache_hits = 0;
        self.cache_misses = 0;
        self.last_operation_ms = 0;
    }
}

impl AccountActivityTracker {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            index: HashMap::new(),
            enabled: true,
            capacity: 1024,
            stats: AccountActivityTrackerStats::default(),
        }
    }

    pub fn with_capacity(mut self, cap: usize) -> Self {
        self.capacity = cap;
        self
    }

    pub fn add(&mut self, entry: impl Into<String>) -> bool {
        let entry = entry.into();
        if self.entries.len() >= self.capacity {
            return false;
        }
        if self.index.contains_key(&entry) {
            self.stats.cache_hits += 1;
            return false;
        }
        let idx = self.entries.len();
        self.index.insert(entry.clone(), idx);
        self.entries.push(entry);
        self.stats.total_operations += 1;
        self.stats.cache_misses += 1;
        true
    }

    pub fn remove(&mut self, entry: &str) -> bool {
        if let Some(idx) = self.index.remove(entry) {
            self.entries.remove(idx);
            // Rebuild index after removal
            self.index.clear();
            for (i, e) in self.entries.iter().enumerate() {
                self.index.insert(e.clone(), i);
            }
            self.stats.total_operations += 1;
            true
        } else {
            false
        }
    }

    pub fn contains(&self, entry: &str) -> bool {
        self.index.contains_key(entry)
    }

    pub fn get(&self, index: usize) -> Option<&str> {
        self.entries.get(index).map(|s| s.as_str())
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.index.clear();
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn stats(&self) -> &AccountActivityTrackerStats {
        &self.stats
    }

    pub fn search(&self, query: &str) -> Vec<&str> {
        self.entries.iter()
            .filter(|e| e.contains(query))
            .map(|s| s.as_str())
            .collect()
    }

    pub fn sorted_entries(&self) -> Vec<&str> {
        let mut sorted: Vec<&str> = self.entries.iter().map(|s| s.as_str()).collect();
        sorted.sort();
        sorted
    }

    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|s| s.as_str())
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn remaining_capacity(&self) -> usize {
        self.capacity.saturating_sub(self.entries.len())
    }
}

impl Default for AccountActivityTracker {
    fn default() -> Self {
        Self::new()
    }
}

// === Account Permission Matrix ===

/// Priority level for AccountPermissionMatrix items.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AccountPermissionMatrixPriority {
    Low,
    Normal,
    High,
    Critical,
}

impl AccountPermissionMatrixPriority {
    pub fn as_weight(&self) -> u32 {
        match self {
            Self::Low => 1,
            Self::Normal => 5,
            Self::High => 10,
            Self::Critical => 100,
        }
    }
}

impl fmt::Display for AccountPermissionMatrixPriority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Normal => write!(f, "normal"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// Account Permission Matrix implementation.
#[derive(Debug, Clone)]
pub struct AccountPermissionMatrix {
    items: Vec<AccountPermissionMatrixItem>,
    max_items: usize,
    default_priority: AccountPermissionMatrixPriority,
}

/// A single item in AccountPermissionMatrix.
#[derive(Debug, Clone)]
pub struct AccountPermissionMatrixItem {
    pub id: String,
    pub label: String,
    pub priority: AccountPermissionMatrixPriority,
    pub timestamp: u64,
    pub metadata: HashMap<String, String>,
}

impl AccountPermissionMatrixItem {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            priority: AccountPermissionMatrixPriority::Normal,
            timestamp: 0,
            metadata: HashMap::new(),
        }
    }

    pub fn with_priority(mut self, priority: AccountPermissionMatrixPriority) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_timestamp(mut self, ts: u64) -> Self {
        self.timestamp = ts;
        self
    }

    pub fn set_meta(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.metadata.insert(key.into(), value.into());
    }

    pub fn get_meta(&self, key: &str) -> Option<&str> {
        self.metadata.get(key).map(|s| s.as_str())
    }
}

impl AccountPermissionMatrix {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            max_items: 500,
            default_priority: AccountPermissionMatrixPriority::Normal,
        }
    }

    pub fn with_max_items(mut self, max: usize) -> Self {
        self.max_items = max;
        self
    }

    pub fn add(&mut self, item: AccountPermissionMatrixItem) -> bool {
        if self.items.len() >= self.max_items {
            return false;
        }
        self.items.push(item);
        true
    }

    pub fn remove_by_id(&mut self, id: &str) -> Option<AccountPermissionMatrixItem> {
        if let Some(idx) = self.items.iter().position(|i| i.id == id) {
            Some(self.items.remove(idx))
        } else {
            None
        }
    }

    pub fn find_by_id(&self, id: &str) -> Option<&AccountPermissionMatrixItem> {
        self.items.iter().find(|i| i.id == id)
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }

    pub fn by_priority(&self, priority: AccountPermissionMatrixPriority) -> Vec<&AccountPermissionMatrixItem> {
        self.items.iter().filter(|i| i.priority == priority).collect()
    }

    pub fn sorted_by_priority(&self) -> Vec<&AccountPermissionMatrixItem> {
        let mut sorted: Vec<&AccountPermissionMatrixItem> = self.items.iter().collect();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
        sorted
    }

    pub fn sorted_by_timestamp(&self) -> Vec<&AccountPermissionMatrixItem> {
        let mut sorted: Vec<&AccountPermissionMatrixItem> = self.items.iter().collect();
        sorted.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
        sorted
    }

    pub fn search(&self, query: &str) -> Vec<&AccountPermissionMatrixItem> {
        let q = query.to_lowercase();
        self.items.iter()
            .filter(|i| i.label.to_lowercase().contains(&q) || i.id.to_lowercase().contains(&q))
            .collect()
    }

    pub fn total_weight(&self) -> u32 {
        self.items.iter().map(|i| i.priority.as_weight()).sum()
    }

    pub fn set_default_priority(&mut self, p: AccountPermissionMatrixPriority) {
        self.default_priority = p;
    }

    pub fn default_priority(&self) -> AccountPermissionMatrixPriority {
        self.default_priority
    }

    pub fn max_items(&self) -> usize {
        self.max_items
    }

    pub fn remaining_capacity(&self) -> usize {
        self.max_items.saturating_sub(self.items.len())
    }

    pub fn iter(&self) -> impl Iterator<Item = &AccountPermissionMatrixItem> {
        self.items.iter()
    }
}

impl Default for AccountPermissionMatrix {
    fn default() -> Self {
        Self::new()
    }
}


/// Workbench account configuration manager.
#[derive(Debug, Clone)]
pub struct WbAccountsConfig {
    entries: Vec<WbAccountsEntry>,
    enabled: bool,
    max_entries: usize,
}

/// A single workbench account entry.
#[derive(Debug, Clone, PartialEq)]
pub struct WbAccountsEntry {
    pub id: String,
    pub label: String,
    pub priority: i32,
    pub active: bool,
    pub metadata: Vec<(String, String)>,
}

impl WbAccountsEntry {
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

impl WbAccountsConfig {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            enabled: true,
            max_entries,
        }
    }

    pub fn add(&mut self, entry: WbAccountsEntry) -> bool {
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

    pub fn get(&self, id: &str) -> Option<&WbAccountsEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut WbAccountsEntry> {
        self.entries.iter_mut().find(|e| e.id == id)
    }

    pub fn active_entries(&self) -> Vec<&WbAccountsEntry> {
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

    pub fn top_n(&self, n: usize) -> Vec<&WbAccountsEntry> {
        self.entries.iter().take(n).collect()
    }

    pub fn find_by_label(&self, label: &str) -> Option<&WbAccountsEntry> {
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

    pub fn drain_inactive(&mut self) -> Vec<WbAccountsEntry> {
        let (inactive, active): (Vec<_>, Vec<_>) =
            self.entries.drain(..).partition(|e| !e.active);
        self.entries = active;
        inactive
    }
}


// ---------------------------------------------------------------------------
// Account management UI — extended utilities (xm)
// ---------------------------------------------------------------------------

/// Metric accumulator for wb_acct operations.
#[derive(Debug, Clone)]
pub struct XmMetrics {
    samples: Vec<f64>,
    label: String,
}

impl XmMetrics {
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

/// Sliding-window rate counter for wb_acct.
#[derive(Debug, Clone)]
pub struct XmRateWindow {
    timestamps: Vec<u64>,
    window_ms: u64,
}

impl XmRateWindow {
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

/// A small LRU-style cache for wb_acct lookups.
#[derive(Debug, Clone)]
pub struct XmLruCache {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl XmLruCache {
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
// xb_ utilities – batch 31
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer31 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer31 {
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
pub fn xb_fnv1a_31(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_31<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
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
pub fn xb_rle_decode_31<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_31(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_31(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 198
// ---------------------------------------------------------------------------

/// Generic object pool `Xc198Pool<T>`.
pub struct Xc198Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc198Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc198PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc198Pool<T> {
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
    pub fn stats(&self) -> Xc198PoolStats {
        Xc198PoolStats {
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

impl<T> Default for Xc198Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc198Scheduler`.
pub struct Xc198Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc198Scheduler {
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

impl Default for Xc198Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_198 hash for the given byte slice.
pub fn xc_198_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_198 convention.
pub fn xc_198_reverse(s: &str) -> String {
    s.chars().rev().collect()
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

    // -----------------------------------------------------------------------
    // AccountMergerService tests
    // -----------------------------------------------------------------------

    fn make_account(id: &str, email: Option<&str>) -> AccountInfo {
        AccountInfo {
            id: id.to_string(),
            label: format!("User {id}"),
            provider_id: "github".to_string(),
            email: email.map(|e| e.to_string()),
        }
    }

    #[test]
    fn merger_basic_merge() {
        let mut merger = AccountMergerService::new();
        let mut accounts = vec![
            make_account("a1", Some("a1@x.com")),
            make_account("a2", None),
        ];
        assert!(merger.merge(&mut accounts, "a1", "a2").is_ok());
        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].email.as_deref(), Some("a1@x.com"));
        assert_eq!(merger.merge_count(), 1);
    }

    #[test]
    fn merger_does_not_overwrite_existing_email() {
        let mut merger = AccountMergerService::new();
        let mut accounts = vec![
            make_account("a1", Some("old@x.com")),
            make_account("a2", Some("keep@x.com")),
        ];
        assert!(merger.merge(&mut accounts, "a1", "a2").is_ok());
        assert_eq!(accounts[0].email.as_deref(), Some("keep@x.com"));
    }

    #[test]
    fn merger_rejects_same_id() {
        let mut merger = AccountMergerService::new();
        let mut accounts = vec![make_account("a1", None)];
        assert!(merger.merge(&mut accounts, "a1", "a1").is_err());
    }

    #[test]
    fn merger_can_merge_checks() {
        let accounts = vec![make_account("a1", None), make_account("a2", None)];
        assert!(AccountMergerService::can_merge(&accounts, "a1", "a2"));
        assert!(!AccountMergerService::can_merge(&accounts, "a1", "a1"));
        assert!(!AccountMergerService::can_merge(&accounts, "a1", "missing"));
    }

    #[test]
    fn merger_history() {
        let mut merger = AccountMergerService::new();
        let mut accounts = vec![
            make_account("a1", None),
            make_account("a2", None),
            make_account("a3", None),
        ];
        merger.merge(&mut accounts, "a1", "a2").unwrap();
        merger.merge(&mut accounts, "a3", "a2").unwrap();
        assert_eq!(merger.merge_history().len(), 2);
        assert_eq!(merger.merge_history()[0], ("a1".to_string(), "a2".to_string()));
    }

    // -----------------------------------------------------------------------
    // AccountSessionRefresher tests
    // -----------------------------------------------------------------------

    #[test]
    fn refresher_success_then_exhausted() {
        let mut r = AccountSessionRefresher::new(2);
        assert_eq!(r.attempt_refresh("s1", 100), RefreshResult::Success);
        assert_eq!(r.attempt_refresh("s1", 200), RefreshResult::Success);
        assert_eq!(r.attempt_refresh("s1", 300), RefreshResult::Exhausted);
        assert_eq!(r.attempts(), 2);
    }

    #[test]
    fn refresher_reset() {
        let mut r = AccountSessionRefresher::new(1);
        r.attempt_refresh("s1", 10);
        r.reset();
        assert_eq!(r.attempts(), 0);
        assert_eq!(r.attempt_refresh("s1", 20), RefreshResult::Success);
    }

    #[test]
    fn refresher_needs_refresh() {
        let sess = make_test_account_session(Some(1000));
        assert!(AccountSessionRefresher::needs_refresh(&sess, 1000));
        assert!(!AccountSessionRefresher::needs_refresh(&sess, 999));
        let no_exp = make_test_account_session(None);
        assert!(!AccountSessionRefresher::needs_refresh(&no_exp, 99999));
    }

    // -----------------------------------------------------------------------
    // AccountProviderPriority tests
    // -----------------------------------------------------------------------

    #[test]
    fn provider_priority_sorted() {
        let mut pp = AccountProviderPriority::new();
        pp.set_priority("azure", 30);
        pp.set_priority("github", 10);
        pp.set_priority("gitlab", 20);
        let sorted = pp.sorted_providers();
        assert_eq!(sorted, vec!["github", "gitlab", "azure"]);
        assert_eq!(pp.highest_priority().as_deref(), Some("github"));
    }

    #[test]
    fn provider_priority_default_and_remove() {
        let mut pp = AccountProviderPriority::new();
        assert_eq!(pp.get_priority("unknown"), 100);
        pp.set_priority("github", 5);
        assert_eq!(pp.count(), 1);
        pp.remove("github");
        assert_eq!(pp.count(), 0);
    }

    // -----------------------------------------------------------------------
    // AccountActivityLogger tests
    // -----------------------------------------------------------------------

    #[test]
    fn activity_logger_log_and_query() {
        let mut logger = AccountActivityLogger::new();
        logger.log_activity("a1", "login", 100);
        logger.log_activity("a2", "login", 200);
        logger.log_activity("a1", "logout", 300);
        assert_eq!(logger.count(), 3);
        assert_eq!(logger.activities_for("a1").len(), 2);
        assert_eq!(logger.activities_for("a2").len(), 1);
    }

    #[test]
    fn activity_logger_recent_and_clear() {
        let mut logger = AccountActivityLogger::new();
        for i in 0..5 {
            logger.log_activity("a1", "action", i);
        }
        assert_eq!(logger.recent(3).len(), 3);
        assert_eq!(logger.recent(10).len(), 5);
        logger.clear();
        assert_eq!(logger.count(), 0);
    }

    #[test]
    fn activity_logger_actions_summary() {
        let mut logger = AccountActivityLogger::new();
        logger.log_activity("a1", "login", 1);
        logger.log_activity("a2", "login", 2);
        logger.log_activity("a1", "logout", 3);
        let summary = logger.actions_summary();
        assert_eq!(summary.get("login"), Some(&2));
        assert_eq!(summary.get("logout"), Some(&1));
    }

    #[test]
    fn activity_display() {
        let a = AccountActivity {
            account_id: "a1".to_string(),
            action: "login".to_string(),
            timestamp: 42,
        };
        let s = format!("{a}");
        assert!(s.contains("a1"));
        assert!(s.contains("login"));
        assert!(s.contains("42"));
    }

    #[test]
    fn accountActivityTracker_new() {
        let s = AccountActivityTracker::new();
        assert!(s.is_empty());
        assert_eq!(s.len(), 0);
    }

    #[test]
    fn accountActivityTracker_add_contains() {
        let mut s = AccountActivityTracker::new();
        assert!(s.add("item1"));
        assert!(s.contains("item1"));
        assert!(!s.contains("item2"));
    }

    #[test]
    fn accountActivityTracker_add_duplicate() {
        let mut s = AccountActivityTracker::new();
        assert!(s.add("dup"));
        assert!(!s.add("dup"));
        assert_eq!(s.len(), 1);
    }

    #[test]
    fn accountActivityTracker_remove() {
        let mut s = AccountActivityTracker::new();
        s.add("rem");
        assert!(s.remove("rem"));
        assert!(!s.contains("rem"));
    }

    #[test]
    fn accountActivityTracker_capacity() {
        let s = AccountActivityTracker::new().with_capacity(5);
        assert_eq!(s.capacity(), 5);
        assert_eq!(s.remaining_capacity(), 5);
    }

    #[test]
    fn accountActivityTracker_search() {
        let mut s = AccountActivityTracker::new();
        s.add("hello_world");
        s.add("hello_rust");
        s.add("goodbye");
        let results = s.search("hello");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn accountActivityTracker_stats() {
        let mut s = AccountActivityTracker::new();
        s.add("a");
        s.add("a"); // duplicate = cache hit
        assert_eq!(s.stats().cache_hits, 1);
        assert_eq!(s.stats().cache_misses, 1);
    }

    #[test]
    fn accountPermissionMatrix_new() {
        let m = AccountPermissionMatrix::new();
        assert!(m.is_empty());
        assert_eq!(m.len(), 0);
    }

    #[test]
    fn accountPermissionMatrix_add_find() {
        let mut m = AccountPermissionMatrix::new();
        m.add(AccountPermissionMatrixItem::new("id1", "Label 1"));
        assert!(m.find_by_id("id1").is_some());
        assert!(m.find_by_id("id2").is_none());
    }

    #[test]
    fn accountPermissionMatrix_priority_filter() {
        let mut m = AccountPermissionMatrix::new();
        m.add(AccountPermissionMatrixItem::new("a", "A").with_priority(AccountPermissionMatrixPriority::High));
        m.add(AccountPermissionMatrixItem::new("b", "B").with_priority(AccountPermissionMatrixPriority::Low));
        m.add(AccountPermissionMatrixItem::new("c", "C").with_priority(AccountPermissionMatrixPriority::High));
        assert_eq!(m.by_priority(AccountPermissionMatrixPriority::High).len(), 2);
    }

    #[test]
    fn accountPermissionMatrix_remove() {
        let mut m = AccountPermissionMatrix::new();
        m.add(AccountPermissionMatrixItem::new("r1", "Remove me"));
        assert!(m.remove_by_id("r1").is_some());
        assert!(m.is_empty());
    }

    #[test]
    fn accountPermissionMatrix_search() {
        let mut m = AccountPermissionMatrix::new();
        m.add(AccountPermissionMatrixItem::new("id1", "Hello World"));
        m.add(AccountPermissionMatrixItem::new("id2", "Goodbye"));
        let results = m.search("hello");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn accountPermissionMatrix_total_weight() {
        let mut m = AccountPermissionMatrix::new();
        m.add(AccountPermissionMatrixItem::new("a", "A").with_priority(AccountPermissionMatrixPriority::Critical));
        m.add(AccountPermissionMatrixItem::new("b", "B").with_priority(AccountPermissionMatrixPriority::Low));
        assert_eq!(m.total_weight(), 101);
    }

    #[test]
    fn accountPermissionMatrix_capacity_limit() {
        let mut m = AccountPermissionMatrix::new().with_max_items(2);
        m.add(AccountPermissionMatrixItem::new("1", "one"));
        m.add(AccountPermissionMatrixItem::new("2", "two"));
        assert!(!m.add(AccountPermissionMatrixItem::new("3", "three")));
        assert_eq!(m.len(), 2);
    }

    #[test]
    fn accountPermissionMatrix_sorted_by_priority() {
        let mut m = AccountPermissionMatrix::new();
        m.add(AccountPermissionMatrixItem::new("lo", "Low").with_priority(AccountPermissionMatrixPriority::Low));
        m.add(AccountPermissionMatrixItem::new("hi", "High").with_priority(AccountPermissionMatrixPriority::Critical));
        let sorted = m.sorted_by_priority();
        assert_eq!(sorted[0].id, "hi");
    }

    #[test]
    fn accountPermissionMatrix_item_metadata() {
        let mut item = AccountPermissionMatrixItem::new("m1", "Meta");
        item.set_meta("key", "value");
        assert_eq!(item.get_meta("key"), Some("value"));
        assert_eq!(item.get_meta("missing"), None);
    }

    #[test]
    fn accountActivityTracker_enabled_toggle() {
        let mut s = AccountActivityTracker::new();
        assert!(s.is_enabled());
        s.set_enabled(false);
        assert!(!s.is_enabled());
    }

    #[test]
    fn accountPermissionMatrix_priority_display() {
        assert_eq!(format!("{}", AccountPermissionMatrixPriority::High), "high");
        assert_eq!(format!("{}", AccountPermissionMatrixPriority::Low), "low");
    }


    #[test]
    fn wb_accounts_entry_creation() {
        let e = WbAccountsEntry::new("e1", "Entry 1");
        assert_eq!(e.id, "e1");
        assert_eq!(e.label, "Entry 1");
        assert!(e.active);
        assert_eq!(e.priority, 0);
    }

    #[test]
    fn wb_accounts_entry_with_priority() {
        let e = WbAccountsEntry::new("e1", "E").with_priority(5);
        assert_eq!(e.priority, 5);
    }

    #[test]
    fn wb_accounts_entry_metadata() {
        let e = WbAccountsEntry::new("e1", "E").with_meta("key", "val");
        assert_eq!(e.get_meta("key"), Some("val"));
        assert_eq!(e.get_meta("missing"), None);
        assert!(e.has_meta("key"));
        assert_eq!(e.meta_count(), 1);
    }

    #[test]
    fn wb_accounts_entry_remove_meta() {
        let mut e = WbAccountsEntry::new("e1", "E").with_meta("k", "v");
        assert!(e.remove_meta("k"));
        assert!(!e.remove_meta("k"));
    }

    #[test]
    fn wb_accounts_entry_activate_deactivate() {
        let mut e = WbAccountsEntry::new("e1", "E");
        e.deactivate();
        assert!(!e.active);
        e.activate();
        assert!(e.active);
    }

    #[test]
    fn wb_accounts_config_add_sorted() {
        let mut c = WbAccountsConfig::new(10);
        c.add(WbAccountsEntry::new("lo", "Lo").with_priority(1));
        c.add(WbAccountsEntry::new("hi", "Hi").with_priority(10));
        assert_eq!(c.ids()[0], "hi");
    }

    #[test]
    fn wb_accounts_config_capacity() {
        let mut c = WbAccountsConfig::new(1);
        assert!(c.add(WbAccountsEntry::new("a", "A")));
        assert!(!c.add(WbAccountsEntry::new("b", "B")));
        assert!(c.is_full());
    }

    #[test]
    fn wb_accounts_config_remove() {
        let mut c = WbAccountsConfig::new(10);
        c.add(WbAccountsEntry::new("a", "A"));
        assert!(c.remove("a"));
        assert!(!c.remove("a"));
        assert!(c.is_empty());
    }

    #[test]
    fn wb_accounts_config_get() {
        let mut c = WbAccountsConfig::new(10);
        c.add(WbAccountsEntry::new("x", "X"));
        assert!(c.get("x").is_some());
        assert!(c.get("y").is_none());
    }

    #[test]
    fn wb_accounts_config_active_entries() {
        let mut c = WbAccountsConfig::new(10);
        c.add(WbAccountsEntry::new("a", "A"));
        c.add(WbAccountsEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        assert_eq!(c.active_entries().len(), 1);
        assert_eq!(c.count_active(), 1);
    }

    #[test]
    fn wb_accounts_config_enable_disable() {
        let mut c = WbAccountsConfig::new(10);
        c.disable();
        assert!(!c.is_enabled());
        c.enable();
        assert!(c.is_enabled());
    }

    #[test]
    fn wb_accounts_config_clear() {
        let mut c = WbAccountsConfig::new(10);
        c.add(WbAccountsEntry::new("a", "A"));
        c.clear();
        assert!(c.is_empty());
    }

    #[test]
    fn wb_accounts_config_find_by_label() {
        let mut c = WbAccountsConfig::new(10);
        c.add(WbAccountsEntry::new("a", "Alpha"));
        assert_eq!(c.find_by_label("Alpha").unwrap().id, "a");
        assert!(c.find_by_label("missing").is_none());
    }

    #[test]
    fn wb_accounts_config_top_n() {
        let mut c = WbAccountsConfig::new(10);
        c.add(WbAccountsEntry::new("a", "A").with_priority(1));
        c.add(WbAccountsEntry::new("b", "B").with_priority(2));
        c.add(WbAccountsEntry::new("c", "C").with_priority(3));
        assert_eq!(c.top_n(2).len(), 2);
    }

    #[test]
    fn wb_accounts_config_deactivate_activate_all() {
        let mut c = WbAccountsConfig::new(10);
        c.add(WbAccountsEntry::new("a", "A"));
        c.add(WbAccountsEntry::new("b", "B"));
        c.deactivate_all();
        assert_eq!(c.count_active(), 0);
        c.activate_all();
        assert_eq!(c.count_active(), 2);
    }

    #[test]
    fn wb_accounts_config_highest_priority() {
        let mut c = WbAccountsConfig::new(10);
        assert!(c.highest_priority().is_none());
        c.add(WbAccountsEntry::new("a", "A").with_priority(7));
        assert_eq!(c.highest_priority(), Some(7));
    }

    #[test]
    fn wb_accounts_config_contains() {
        let mut c = WbAccountsConfig::new(10);
        c.add(WbAccountsEntry::new("a", "A"));
        assert!(c.contains("a"));
        assert!(!c.contains("b"));
    }

    #[test]
    fn wb_accounts_config_labels() {
        let mut c = WbAccountsConfig::new(10);
        c.add(WbAccountsEntry::new("a", "Alpha"));
        c.add(WbAccountsEntry::new("b", "Beta"));
        let labels = c.labels();
        assert!(labels.contains(&"Alpha"));
        assert!(labels.contains(&"Beta"));
    }

    #[test]
    fn wb_accounts_config_drain_inactive() {
        let mut c = WbAccountsConfig::new(10);
        c.add(WbAccountsEntry::new("a", "A"));
        c.add(WbAccountsEntry::new("b", "B"));
        c.get_mut("a").unwrap().deactivate();
        let drained = c.drain_inactive();
        assert_eq!(drained.len(), 1);
        assert_eq!(drained[0].id, "a");
        assert_eq!(c.len(), 1);
    }


    #[test]
    fn xm_metrics_empty() {
        let m = XmMetrics::new("wb_acct");
        assert_eq!(m.count(), 0);
        assert!((m.mean() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_metrics_record_and_mean() {
        let mut m = XmMetrics::new("wb_acct");
        m.record(10.0);
        m.record(20.0);
        m.record(30.0);
        assert_eq!(m.count(), 3);
        assert!((m.mean() - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_metrics_min_max() {
        let mut m = XmMetrics::new("test");
        m.record(5.0);
        m.record(15.0);
        m.record(10.0);
        assert!((m.min_val() - 5.0).abs() < f64::EPSILON);
        assert!((m.max_val() - 15.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_metrics_variance_and_std() {
        let mut m = XmMetrics::new("v");
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
    fn xm_metrics_percentile() {
        let mut m = XmMetrics::new("p");
        for i in 1..=100 {
            m.record(i as f64);
        }
        let p50 = m.percentile(50.0);
        assert!(p50 >= 49.0 && p50 <= 51.0);
    }

    #[test]
    fn xm_metrics_merge() {
        let mut a = XmMetrics::new("a");
        a.record(1.0);
        let mut b = XmMetrics::new("b");
        b.record(2.0);
        b.record(3.0);
        a.merge(&b);
        assert_eq!(a.count(), 3);
    }

    #[test]
    fn xm_metrics_reset() {
        let mut m = XmMetrics::new("r");
        m.record(42.0);
        m.reset();
        assert_eq!(m.count(), 0);
    }

    #[test]
    fn xm_rate_window_empty() {
        let rw = XmRateWindow::new(1000);
        assert!(rw.is_empty());
        assert_eq!(rw.window_ms(), 1000);
    }

    #[test]
    fn xm_rate_window_tick_and_rate() {
        let mut rw = XmRateWindow::new(1000);
        rw.tick(100);
        rw.tick(200);
        rw.tick(300);
        assert_eq!(rw.rate(500), 3);
        assert_eq!(rw.rate(1500), 0);
    }

    #[test]
    fn xm_lru_cache_basic() {
        let mut c = XmLruCache::new(2);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        assert_eq!(c.get("a"), Some("1".to_string()));
        c.put("c".into(), "3".into());
        assert_eq!(c.get("b"), None);
    }

    #[test]
    fn xm_lru_cache_contains_and_keys() {
        let mut c = XmLruCache::new(3);
        c.put("x".into(), "10".into());
        c.put("y".into(), "20".into());
        assert!(c.contains_key("x"));
        assert!(!c.contains_key("z"));
        assert_eq!(c.keys().len(), 2);
    }

    #[test]
    fn xm_lru_cache_remove() {
        let mut c = XmLruCache::new(3);
        c.put("k".into(), "v".into());
        assert_eq!(c.remove("k"), Some("v".to_string()));
        assert!(c.is_empty());
        assert_eq!(c.remove("k"), None);
    }

    #[test]
    fn xm_metrics_sum() {
        let mut m = XmMetrics::new("s");
        m.record(1.0);
        m.record(2.0);
        m.record(3.0);
        assert!((m.sum() - 6.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xm_metrics_label() {
        let m = XmMetrics::new("my_label");
        assert_eq!(m.label(), "my_label");
    }

    #[test]
    fn xm_lru_cache_clear() {
        let mut c = XmLruCache::new(5);
        c.put("a".into(), "1".into());
        c.put("b".into(), "2".into());
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    #[test]
    fn xb_ring_buffer_31_push_and_len() {
        let mut rb = super::XbRingBuffer31::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_31_overwrite() {
        let mut rb = super::XbRingBuffer31::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_31_get_out_of_bounds() {
        let rb = super::XbRingBuffer31::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_31_drain_all() {
        let mut rb = super::XbRingBuffer31::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_31_peek_front_back() {
        let mut rb = super::XbRingBuffer31::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_31_clear() {
        let mut rb = super::XbRingBuffer31::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_31_capacity() {
        let rb = super::XbRingBuffer31::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_31_basic() {
        let h = super::xb_fnv1a_31(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_31(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_31_different_inputs() {
        let h1 = super::xb_fnv1a_31(b"abc");
        let h2 = super::xb_fnv1a_31(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_31_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_31(&data);
        let dec = super::xb_rle_decode_31(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_31_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_31(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_31(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_31_values() {
        assert!((super::xb_clamp_31(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_31(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_31(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_31_values() {
        assert!((super::xb_lerp_31(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_31(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_31(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_31_wrap_around_twice() {
        let mut rb = super::XbRingBuffer31::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
    }


    // ---- xc_ pool / scheduler tests – block 198 ----

    #[test]
    fn xc_198_pool_new_empty() {
        let pool: super::Xc198Pool<i32> = super::Xc198Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_198_pool_release_acquire() {
        let mut pool = super::Xc198Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_198_pool_acquire_empty() {
        let mut pool: super::Xc198Pool<i32> = super::Xc198Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_198_pool_full() {
        let mut pool = super::Xc198Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_198_pool_drain() {
        let mut pool = super::Xc198Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_198_pool_stats() {
        let mut pool = super::Xc198Pool::new(8);
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
    fn xc_198_pool_clear() {
        let mut pool = super::Xc198Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_198_pool_shrink() {
        let mut pool = super::Xc198Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_198_pool_default() {
        let pool: super::Xc198Pool<String> = super::Xc198Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_198_pool_extend() {
        let mut pool = super::Xc198Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_198_pool_retain() {
        let mut pool = super::Xc198Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_198_scheduler_round_robin() {
        let mut sched = super::Xc198Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_198_scheduler_empty() {
        let mut sched = super::Xc198Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_198_scheduler_reset() {
        let mut sched = super::Xc198Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_198_scheduler_add_remove() {
        let mut sched = super::Xc198Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_198_scheduler_targets() {
        let sched = super::Xc198Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_198_hash_empty() {
        assert_eq!(super::xc_198_hash(b""), 5381);
    }

    #[test]
    fn xc_198_hash_data() {
        let h = super::xc_198_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_198_hash(b"hello"), h);
    }

    #[test]
    fn xc_198_reverse_str() {
        assert_eq!(super::xc_198_reverse("abc"), "cba");
        assert_eq!(super::xc_198_reverse(""), "");
    }

}
