//! OAuth provider integration.

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
}
