//! OAuth provider integration.

/// The lifecycle status of a registered authentication provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthProviderStatus {
    Registered,
    Active,
    Disabled,
}

/// A registered authentication provider.
#[derive(Debug, Clone)]
pub struct AuthProvider {
    pub id: String,
    pub label: String,
    pub supports_multiple_accounts: bool,
    pub status: AuthProviderStatus,
}

impl std::fmt::Display for AuthProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.label, self.id)
    }
}

/// A session created through an authentication provider.
#[derive(Debug, Clone)]
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
}

impl Default for AuthenticationService {
    fn default() -> Self {
        Self::new()
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
}
