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
}
