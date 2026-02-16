//! Account/login management.

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
}
