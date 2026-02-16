//! Ext API: Webview.
//!
//! RPC bridge between the extension host and the main thread for webview panels.

use std::fmt;
use serde::{Deserialize, Serialize};

/// Proxy identifier for this extension API namespace.
pub const PROXY_ID: &str = "ext_webview";

// ── RPC Messages ──

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum WebviewMessage {
    SetHtml {
        handle: u64,
        html: String,
    },
    SetOptions {
        handle: u64,
        options: WebviewOptions,
    },
    PostMessage {
        handle: u64,
        message: serde_json::Value,
    },
    OnDidReceiveMessage {
        handle: u64,
        message: serde_json::Value,
    },
    Dispose {
        handle: u64,
    },
}

// ── Core Types ──

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WebviewContent {
    pub handle: u64,
    pub html: String,
    pub options: WebviewOptions,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WebviewOptions {
    pub enable_scripts: bool,
    pub enable_forms: bool,
    pub local_resource_roots: Vec<String>,
}

// ── Bridge ──

pub struct WebviewBridge {
    webviews: Vec<WebviewContent>,
    messages: Vec<(u64, serde_json::Value)>,
}

impl WebviewBridge {
    pub fn new() -> Self {
        Self {
            webviews: Vec::new(),
            messages: Vec::new(),
        }
    }

    pub fn create_webview(&mut self, handle: u64) {
        if !self.webviews.iter().any(|w| w.handle == handle) {
            self.webviews.push(WebviewContent {
                handle,
                html: String::new(),
                options: WebviewOptions {
                    enable_scripts: false,
                    enable_forms: false,
                    local_resource_roots: Vec::new(),
                },
            });
        }
    }

    pub fn get_webview(&self, handle: u64) -> Option<&WebviewContent> {
        self.webviews.iter().find(|w| w.handle == handle)
    }

    pub fn dispose_webview(&mut self, handle: u64) -> bool {
        let before = self.webviews.len();
        self.webviews.retain(|w| w.handle != handle);
        self.webviews.len() < before
    }

    pub fn handle_message(&mut self, msg: &WebviewMessage) -> serde_json::Value {
        match msg {
            WebviewMessage::SetHtml { handle, html } => {
                if let Some(w) = self.webviews.iter_mut().find(|w| w.handle == *handle) {
                    w.html = html.clone();
                    serde_json::json!({"updated": true})
                } else {
                    serde_json::json!({"error": "not found"})
                }
            }
            WebviewMessage::SetOptions { handle, options } => {
                if let Some(w) = self.webviews.iter_mut().find(|w| w.handle == *handle) {
                    w.options = options.clone();
                    serde_json::json!({"updated": true})
                } else {
                    serde_json::json!({"error": "not found"})
                }
            }
            WebviewMessage::PostMessage { handle, message } => {
                self.messages.push((*handle, message.clone()));
                serde_json::json!({"posted": true})
            }
            WebviewMessage::OnDidReceiveMessage { handle, message } => {
                serde_json::json!({"handle": handle, "message": message})
            }
            WebviewMessage::Dispose { handle } => {
                let ok = self.dispose_webview(*handle);
                serde_json::json!({"disposed": ok})
            }
        }
    }
}

impl Default for WebviewBridge {
    fn default() -> Self {
        Self::new()
    }
}

// ── Error Handling ──

/// Errors that can occur during webview operations.
#[derive(Debug, Clone, PartialEq)]
pub enum WebviewError {
    /// The referenced webview handle does not exist.
    NotFound(u64),
    /// A webview with this handle already exists.
    DuplicateHandle(u64),
    /// The provided HTML content is invalid or empty.
    InvalidContent(String),
    /// A local resource root path is invalid.
    InvalidResourceRoot(String),
    /// The message payload exceeds the maximum allowed size.
    PayloadTooLarge { size: usize, max: usize },
}

impl std::fmt::Display for WebviewError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound(h) => write!(f, "webview handle {h} not found"),
            Self::DuplicateHandle(h) => write!(f, "webview handle {h} already exists"),
            Self::InvalidContent(reason) => write!(f, "invalid content: {reason}"),
            Self::InvalidResourceRoot(path) => write!(f, "invalid resource root: {path}"),
            Self::PayloadTooLarge { size, max } => {
                write!(f, "payload size {size} exceeds maximum {max}")
            }
        }
    }
}

impl std::error::Error for WebviewError {}

// ── Display for WebviewBridge ──

impl std::fmt::Debug for WebviewBridge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WebviewBridge")
            .field("webview_count", &self.webviews.len())
            .field("pending_messages", &self.messages.len())
            .finish()
    }
}

impl std::fmt::Display for WebviewBridge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "WebviewBridge({} webviews, {} pending messages)",
            self.webviews.len(),
            self.messages.len()
        )
    }
}

// ── Builder for WebviewOptions ──

/// Builder for constructing [`WebviewOptions`] with validation.
#[derive(Clone, Debug, Default)]
pub struct WebviewOptionsBuilder {
    enable_scripts: bool,
    enable_forms: bool,
    local_resource_roots: Vec<String>,
}

impl WebviewOptionsBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn enable_scripts(mut self, enabled: bool) -> Self {
        self.enable_scripts = enabled;
        self
    }

    pub fn enable_forms(mut self, enabled: bool) -> Self {
        self.enable_forms = enabled;
        self
    }

    pub fn add_resource_root(mut self, root: impl Into<String>) -> Self {
        self.local_resource_roots.push(root.into());
        self
    }

    /// Validate and build the options.
    pub fn build(self) -> Result<WebviewOptions, WebviewError> {
        for root in &self.local_resource_roots {
            if root.is_empty() || root.contains("..") {
                return Err(WebviewError::InvalidResourceRoot(root.clone()));
            }
        }
        Ok(WebviewOptions {
            enable_scripts: self.enable_scripts,
            enable_forms: self.enable_forms,
            local_resource_roots: self.local_resource_roots,
        })
    }
}

// ── Extended Bridge Methods ──

/// Maximum message payload size in bytes (1 MiB).
pub const MAX_PAYLOAD_SIZE: usize = 1024 * 1024;

impl WebviewBridge {
    /// Create a webview with validated options, returning an error on duplicates.
    pub fn create_webview_checked(
        &mut self,
        handle: u64,
        options: WebviewOptions,
    ) -> Result<(), WebviewError> {
        if self.webviews.iter().any(|w| w.handle == handle) {
            return Err(WebviewError::DuplicateHandle(handle));
        }
        self.webviews.push(WebviewContent {
            handle,
            html: String::new(),
            options,
        });
        Ok(())
    }

    /// Set HTML content with validation.
    pub fn set_html_checked(
        &mut self,
        handle: u64,
        html: String,
    ) -> Result<(), WebviewError> {
        if html.is_empty() {
            return Err(WebviewError::InvalidContent("html must not be empty".into()));
        }
        let wv = self
            .webviews
            .iter_mut()
            .find(|w| w.handle == handle)
            .ok_or(WebviewError::NotFound(handle))?;
        wv.html = html;
        Ok(())
    }

    /// Post a message with payload size validation.
    pub fn post_message_checked(
        &mut self,
        handle: u64,
        message: serde_json::Value,
    ) -> Result<(), WebviewError> {
        let payload = serde_json::to_string(&message).unwrap_or_default();
        if payload.len() > MAX_PAYLOAD_SIZE {
            return Err(WebviewError::PayloadTooLarge {
                size: payload.len(),
                max: MAX_PAYLOAD_SIZE,
            });
        }
        if !self.webviews.iter().any(|w| w.handle == handle) {
            return Err(WebviewError::NotFound(handle));
        }
        self.messages.push((handle, message));
        Ok(())
    }

    /// Return all handles currently registered.
    pub fn handles(&self) -> Vec<u64> {
        self.webviews.iter().map(|w| w.handle).collect()
    }

    /// Return the number of registered webviews.
    pub fn webview_count(&self) -> usize {
        self.webviews.len()
    }

    /// Drain and return all pending messages for a given handle.
    pub fn drain_messages(&mut self, handle: u64) -> Vec<serde_json::Value> {
        let mut drained = Vec::new();
        let mut remaining = Vec::new();
        for (h, msg) in self.messages.drain(..) {
            if h == handle {
                drained.push(msg);
            } else {
                remaining.push((h, msg));
            }
        }
        self.messages = remaining;
        drained
    }

    /// Update the options on an existing webview.
    pub fn set_options_checked(
        &mut self,
        handle: u64,
        options: WebviewOptions,
    ) -> Result<(), WebviewError> {
        let wv = self
            .webviews
            .iter_mut()
            .find(|w| w.handle == handle)
            .ok_or(WebviewError::NotFound(handle))?;
        wv.options = options;
        Ok(())
    }

    /// Check whether scripting is enabled for the given webview.
    pub fn is_scripting_enabled(&self, handle: u64) -> Result<bool, WebviewError> {
        self.webviews
            .iter()
            .find(|w| w.handle == handle)
            .map(|w| w.options.enable_scripts)
            .ok_or(WebviewError::NotFound(handle))
    }
}

// ── Display for WebviewContent ──

impl std::fmt::Display for WebviewContent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Webview(handle={}, html_len={}, scripts={})",
            self.handle,
            self.html.len(),
            self.options.enable_scripts
        )
    }
}

/// Initialize the webview extension API bridge.
pub fn register() {
    // Registration will connect RPC handlers when extension host starts
}

// ── Resource Policy ──

/// Controls which external resources a webview is permitted to load.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WebviewResourcePolicy {
    /// Allow all resource loads without restriction.
    Allow,
    /// Deny all external resource loads.
    Deny,
    /// Only allow resources from the same origin as the webview content.
    SameOrigin,
}

/// Security configuration for a webview panel, combining a resource policy
/// with explicit origin allowlisting and feature toggles.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WebviewSecurityConfig {
    /// The resource loading policy applied to the webview.
    pub policy: WebviewResourcePolicy,
    /// Origins that are always permitted regardless of the policy.
    pub allowed_origins: Vec<String>,
    /// Whether JavaScript execution is enabled.
    pub enable_scripts: bool,
    /// Whether HTML form submission is enabled.
    pub enable_forms: bool,
}

impl WebviewSecurityConfig {
    /// Create a restrictive default configuration.
    pub fn restrictive() -> Self {
        Self {
            policy: WebviewResourcePolicy::Deny,
            allowed_origins: Vec::new(),
            enable_scripts: false,
            enable_forms: false,
        }
    }

    /// Check whether a given origin is explicitly allowed.
    pub fn is_origin_allowed(&self, origin: &str) -> bool {
        match self.policy {
            WebviewResourcePolicy::Allow => true,
            WebviewResourcePolicy::Deny => {
                self.allowed_origins.iter().any(|o| o == origin)
            }
            WebviewResourcePolicy::SameOrigin => {
                self.allowed_origins.iter().any(|o| o == origin)
            }
        }
    }
}

impl Default for WebviewSecurityConfig {
    fn default() -> Self {
        Self::restrictive()
    }
}

// ── URI Validation ──

/// Accepted URI schemes for webview resource references.
const VALID_SCHEMES: &[&str] = &["https://", "http://", "vscode-resource://", "file:///"];

/// Validate that a URI uses an accepted scheme for webview resources.
///
/// Returns `true` when the URI starts with one of the recognised schemes
/// and contains at least one character after the scheme prefix.
pub fn validate_webview_uri(uri: &str) -> bool {
    VALID_SCHEMES
        .iter()
        .any(|scheme| uri.starts_with(scheme) && uri.len() > scheme.len())
}

// ── Message Router ──

/// A simple message router that dispatches incoming JSON messages to
/// registered handler functions based on a `"type"` field in the payload.
pub struct WebviewMessageRouter {
    handlers: std::collections::HashMap<String, Box<dyn Fn(&serde_json::Value) -> serde_json::Value>>,
}

impl WebviewMessageRouter {
    pub fn new() -> Self {
        Self {
            handlers: std::collections::HashMap::new(),
        }
    }

    /// Register a handler for messages whose `"type"` field equals `msg_type`.
    pub fn register_handler<F>(&mut self, msg_type: impl Into<String>, handler: F)
    where
        F: Fn(&serde_json::Value) -> serde_json::Value + 'static,
    {
        self.handlers.insert(msg_type.into(), Box::new(handler));
    }

    /// Route a JSON message to the appropriate handler.
    ///
    /// The message must contain a `"type"` field whose value matches a
    /// registered handler key. Returns `None` if no handler matches or if
    /// the `"type"` field is absent.
    pub fn route_message(&self, message: &serde_json::Value) -> Option<serde_json::Value> {
        let msg_type = message.get("type")?.as_str()?;
        let handler = self.handlers.get(msg_type)?;
        Some(handler(message))
    }

    /// Return the number of currently registered handlers.
    pub fn handler_count(&self) -> usize {
        self.handlers.len()
    }
}

impl Default for WebviewMessageRouter {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for WebviewMessageRouter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WebviewMessageRouter")
            .field("handler_count", &self.handlers.len())
            .finish()
    }
}

// ── Resource URI Resolution ──

/// Resolve a local file path to a webview-safe resource URI.
///
/// Converts an absolute file path into a `vscode-resource://` URI,
/// rejecting paths containing `..` traversal sequences.
pub fn resolve_resource_uri(extension_root: &str, relative_path: &str) -> Result<String, WebviewError> {
    if relative_path.contains("..") {
        return Err(WebviewError::InvalidResourceRoot(relative_path.to_string()));
    }
    if relative_path.is_empty() {
        return Err(WebviewError::InvalidContent("path must not be empty".into()));
    }
    let sep = if extension_root.ends_with('/') { "" } else { "/" };
    Ok(format!("vscode-resource://{}{}{}", extension_root, sep, relative_path))
}

// ── CSP Header Generation ──

/// Generate a Content-Security-Policy header value for a webview.
///
/// The generated policy restricts resources based on the provided
/// security configuration and an optional nonce for inline scripts.
pub fn generate_csp_header(config: &WebviewSecurityConfig, nonce: Option<&str>) -> String {
    let mut directives = Vec::new();

    let default_src = match config.policy {
        WebviewResourcePolicy::Allow => "default-src *".to_string(),
        WebviewResourcePolicy::Deny => "default-src 'none'".to_string(),
        WebviewResourcePolicy::SameOrigin => "default-src 'self'".to_string(),
    };
    directives.push(default_src);

    if config.enable_scripts {
        let script_src = match nonce {
            Some(n) => format!("script-src 'nonce-{}'", n),
            None => "script-src 'self'".to_string(),
        };
        directives.push(script_src);
    }

    if !config.allowed_origins.is_empty() {
        let origins = config.allowed_origins.join(" ");
        directives.push(format!("connect-src {}", origins));
    }

    directives.join("; ")
}

// ── Webview State Serialization ──

/// Serializable snapshot of a webview's state for persistence.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WebviewStateSnapshot {
    pub handle: u64,
    pub html: String,
    pub options: WebviewOptions,
    pub custom_state: serde_json::Value,
}

impl WebviewBridge {
    /// Serialize the state of a webview into a snapshot.
    pub fn snapshot_webview(
        &self,
        handle: u64,
        custom_state: serde_json::Value,
    ) -> Result<WebviewStateSnapshot, WebviewError> {
        let wv = self
            .webviews
            .iter()
            .find(|w| w.handle == handle)
            .ok_or(WebviewError::NotFound(handle))?;
        Ok(WebviewStateSnapshot {
            handle: wv.handle,
            html: wv.html.clone(),
            options: wv.options.clone(),
            custom_state,
        })
    }

    /// Restore a webview from a snapshot, creating it if it doesn't exist.
    pub fn restore_from_snapshot(&mut self, snapshot: &WebviewStateSnapshot) -> Result<(), WebviewError> {
        if let Some(wv) = self.webviews.iter_mut().find(|w| w.handle == snapshot.handle) {
            wv.html = snapshot.html.clone();
            wv.options = snapshot.options.clone();
        } else {
            self.webviews.push(WebviewContent {
                handle: snapshot.handle,
                html: snapshot.html.clone(),
                options: snapshot.options.clone(),
            });
        }
        Ok(())
    }

    /// Return the total number of pending messages across all webviews.
    pub fn total_pending_messages(&self) -> usize {
        self.messages.len()
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
        let msg = WebviewMessage::SetHtml {
            handle: 1,
            html: "<h1>Hello</h1>".into(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let back: WebviewMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, back);
    }

    #[test]
    fn webview_content_serialization() {
        let wv = WebviewContent {
            handle: 1,
            html: "<p>test</p>".into(),
            options: WebviewOptions {
                enable_scripts: true,
                enable_forms: false,
                local_resource_roots: vec!["file:///ext".into()],
            },
        };
        let json = serde_json::to_string(&wv).unwrap();
        let back: WebviewContent = serde_json::from_str(&json).unwrap();
        assert_eq!(wv, back);
    }

    #[test]
    fn bridge_create_and_set_html() {
        let mut bridge = WebviewBridge::new();
        bridge.create_webview(1);
        bridge.handle_message(&WebviewMessage::SetHtml {
            handle: 1,
            html: "<div>hi</div>".into(),
        });
        assert_eq!(bridge.get_webview(1).unwrap().html, "<div>hi</div>");
    }

    #[test]
    fn bridge_dispose() {
        let mut bridge = WebviewBridge::new();
        bridge.create_webview(1);
        assert!(bridge.dispose_webview(1));
        assert!(bridge.get_webview(1).is_none());
    }

    #[test]
    fn bridge_post_message_tracked() {
        let mut bridge = WebviewBridge::new();
        bridge.create_webview(1);
        bridge.handle_message(&WebviewMessage::PostMessage {
            handle: 1,
            message: serde_json::json!({"cmd": "update"}),
        });
        assert_eq!(bridge.messages.len(), 1);
    }

    #[test]
    fn error_display_not_found() {
        let err = WebviewError::NotFound(42);
        assert_eq!(err.to_string(), "webview handle 42 not found");
    }

    #[test]
    fn error_display_payload_too_large() {
        let err = WebviewError::PayloadTooLarge {
            size: 2_000_000,
            max: MAX_PAYLOAD_SIZE,
        };
        assert!(err.to_string().contains("2000000"));
        assert!(err.to_string().contains("exceeds maximum"));
    }

    #[test]
    fn create_webview_checked_duplicate() {
        let mut bridge = WebviewBridge::new();
        let opts = WebviewOptions {
            enable_scripts: false,
            enable_forms: false,
            local_resource_roots: Vec::new(),
        };
        assert!(bridge.create_webview_checked(1, opts.clone()).is_ok());
        assert_eq!(
            bridge.create_webview_checked(1, opts),
            Err(WebviewError::DuplicateHandle(1))
        );
    }

    #[test]
    fn set_html_checked_empty_rejected() {
        let mut bridge = WebviewBridge::new();
        bridge.create_webview(1);
        let result = bridge.set_html_checked(1, String::new());
        assert!(matches!(result, Err(WebviewError::InvalidContent(_))));
    }

    #[test]
    fn set_html_checked_not_found() {
        let mut bridge = WebviewBridge::new();
        let result = bridge.set_html_checked(99, "<p>hi</p>".into());
        assert_eq!(result, Err(WebviewError::NotFound(99)));
    }

    #[test]
    fn options_builder_success() {
        let opts = WebviewOptionsBuilder::new()
            .enable_scripts(true)
            .enable_forms(false)
            .add_resource_root("file:///workspace")
            .build()
            .unwrap();
        assert!(opts.enable_scripts);
        assert!(!opts.enable_forms);
        assert_eq!(opts.local_resource_roots, vec!["file:///workspace"]);
    }

    #[test]
    fn options_builder_rejects_traversal() {
        let result = WebviewOptionsBuilder::new()
            .add_resource_root("file:///../../etc/passwd")
            .build();
        assert!(matches!(result, Err(WebviewError::InvalidResourceRoot(_))));
    }

    #[test]
    fn drain_messages_isolates_handle() {
        let mut bridge = WebviewBridge::new();
        bridge.create_webview(1);
        bridge.create_webview(2);
        bridge.handle_message(&WebviewMessage::PostMessage {
            handle: 1,
            message: serde_json::json!("a"),
        });
        bridge.handle_message(&WebviewMessage::PostMessage {
            handle: 2,
            message: serde_json::json!("b"),
        });
        bridge.handle_message(&WebviewMessage::PostMessage {
            handle: 1,
            message: serde_json::json!("c"),
        });
        let msgs = bridge.drain_messages(1);
        assert_eq!(msgs.len(), 2);
        assert_eq!(bridge.messages.len(), 1);
    }

    #[test]
    fn handles_returns_all() {
        let mut bridge = WebviewBridge::new();
        bridge.create_webview(10);
        bridge.create_webview(20);
        bridge.create_webview(30);
        let mut handles = bridge.handles();
        handles.sort();
        assert_eq!(handles, vec![10, 20, 30]);
    }

    #[test]
    fn is_scripting_enabled_reflects_options() {
        let mut bridge = WebviewBridge::new();
        let opts = WebviewOptionsBuilder::new()
            .enable_scripts(true)
            .build()
            .unwrap();
        bridge.create_webview_checked(5, opts).unwrap();
        assert!(bridge.is_scripting_enabled(5).unwrap());
        assert_eq!(
            bridge.is_scripting_enabled(99),
            Err(WebviewError::NotFound(99))
        );
    }

    #[test]
    fn display_impls() {
        let bridge = WebviewBridge::new();
        let display = format!("{bridge}");
        assert!(display.contains("0 webviews"));
        let debug = format!("{bridge:?}");
        assert!(debug.contains("WebviewBridge"));

        let content = WebviewContent {
            handle: 7,
            html: "<b>hi</b>".into(),
            options: WebviewOptions {
                enable_scripts: true,
                enable_forms: false,
                local_resource_roots: Vec::new(),
            },
        };
        let display = format!("{content}");
        assert!(display.contains("handle=7"));
        assert!(display.contains("scripts=true"));
    }

    // ── New tests ──

    #[test]
    fn resource_policy_serde_roundtrip() {
        let policy = WebviewResourcePolicy::SameOrigin;
        let json = serde_json::to_string(&policy).unwrap();
        assert_eq!(json, "\"sameOrigin\"");
        let back: WebviewResourcePolicy = serde_json::from_str(&json).unwrap();
        assert_eq!(back, WebviewResourcePolicy::SameOrigin);
    }

    #[test]
    fn security_config_restrictive_defaults() {
        let config = WebviewSecurityConfig::restrictive();
        assert_eq!(config.policy, WebviewResourcePolicy::Deny);
        assert!(config.allowed_origins.is_empty());
        assert!(!config.enable_scripts);
        assert!(!config.enable_forms);
    }

    #[test]
    fn security_config_origin_allowed() {
        let mut config = WebviewSecurityConfig::restrictive();
        config.allowed_origins.push("https://example.com".into());
        assert!(config.is_origin_allowed("https://example.com"));
        assert!(!config.is_origin_allowed("https://evil.com"));

        let allow_all = WebviewSecurityConfig {
            policy: WebviewResourcePolicy::Allow,
            ..config
        };
        assert!(allow_all.is_origin_allowed("https://anything.com"));
    }

    #[test]
    fn validate_webview_uri_accepts_valid() {
        assert!(validate_webview_uri("https://example.com/resource"));
        assert!(validate_webview_uri("vscode-resource://ext/file.js"));
        assert!(validate_webview_uri("file:///workspace/style.css"));
        assert!(validate_webview_uri("http://localhost:3000/api"));
    }

    #[test]
    fn validate_webview_uri_rejects_invalid() {
        assert!(!validate_webview_uri("ftp://files.example.com/a"));
        assert!(!validate_webview_uri("javascript:alert(1)"));
        assert!(!validate_webview_uri("data:text/html,<h1>bad</h1>"));
        assert!(!validate_webview_uri(""));
        // scheme-only with no path should fail
        assert!(!validate_webview_uri("https://"));
    }

    #[test]
    fn message_router_routes_by_type() {
        let mut router = WebviewMessageRouter::new();
        router.register_handler("greet", |msg| {
            let name = msg.get("name").and_then(|v| v.as_str()).unwrap_or("world");
            serde_json::json!({"greeting": format!("hello {name}")})
        });
        assert_eq!(router.handler_count(), 1);

        let result = router
            .route_message(&serde_json::json!({"type": "greet", "name": "Alice"}))
            .unwrap();
        assert_eq!(result, serde_json::json!({"greeting": "hello Alice"}));

        // Unknown type returns None.
        assert!(router
            .route_message(&serde_json::json!({"type": "unknown"}))
            .is_none());

        // Missing type field returns None.
        assert!(router
            .route_message(&serde_json::json!({"data": 42}))
            .is_none());
    }

    #[test]
    fn security_config_serde_roundtrip() {
        let config = WebviewSecurityConfig {
            policy: WebviewResourcePolicy::SameOrigin,
            allowed_origins: vec!["https://trusted.dev".into()],
            enable_scripts: true,
            enable_forms: false,
        };
        let json = serde_json::to_string(&config).unwrap();
        let back: WebviewSecurityConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config, back);
    }

    #[test]
    fn resolve_resource_uri_success() {
        let uri = resolve_resource_uri("/ext/my-extension", "media/style.css").unwrap();
        assert_eq!(uri, "vscode-resource:///ext/my-extension/media/style.css");
    }

    #[test]
    fn resolve_resource_uri_rejects_traversal() {
        let result = resolve_resource_uri("/ext", "../etc/passwd");
        assert!(matches!(result, Err(WebviewError::InvalidResourceRoot(_))));
    }

    #[test]
    fn resolve_resource_uri_rejects_empty() {
        let result = resolve_resource_uri("/ext", "");
        assert!(matches!(result, Err(WebviewError::InvalidContent(_))));
    }

    #[test]
    fn generate_csp_header_deny() {
        let config = WebviewSecurityConfig::restrictive();
        let csp = generate_csp_header(&config, None);
        assert!(csp.contains("default-src 'none'"));
        assert!(!csp.contains("script-src"));
    }

    #[test]
    fn generate_csp_header_with_scripts_and_nonce() {
        let config = WebviewSecurityConfig {
            policy: WebviewResourcePolicy::SameOrigin,
            allowed_origins: vec!["https://api.example.com".into()],
            enable_scripts: true,
            enable_forms: false,
        };
        let csp = generate_csp_header(&config, Some("abc123"));
        assert!(csp.contains("default-src 'self'"));
        assert!(csp.contains("script-src 'nonce-abc123'"));
        assert!(csp.contains("connect-src https://api.example.com"));
    }

    #[test]
    fn webview_state_snapshot_roundtrip() {
        let mut bridge = WebviewBridge::new();
        let opts = WebviewOptionsBuilder::new().enable_scripts(true).build().unwrap();
        bridge.create_webview_checked(1, opts).unwrap();
        bridge.set_html_checked(1, "<p>hello</p>".into()).unwrap();

        let snapshot = bridge.snapshot_webview(1, serde_json::json!({"key": "val"})).unwrap();
        assert_eq!(snapshot.handle, 1);
        assert_eq!(snapshot.html, "<p>hello</p>");

        let json = serde_json::to_string(&snapshot).unwrap();
        let back: WebviewStateSnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(snapshot, back);
    }

    #[test]
    fn restore_from_snapshot_creates_webview() {
        let mut bridge = WebviewBridge::new();
        let snapshot = WebviewStateSnapshot {
            handle: 42,
            html: "<div>restored</div>".into(),
            options: WebviewOptions {
                enable_scripts: false,
                enable_forms: false,
                local_resource_roots: Vec::new(),
            },
            custom_state: serde_json::json!(null),
        };
        bridge.restore_from_snapshot(&snapshot).unwrap();
        assert_eq!(bridge.webview_count(), 1);
        assert_eq!(bridge.get_webview(42).unwrap().html, "<div>restored</div>");
    }

    #[test]
    fn total_pending_messages_count() {
        let mut bridge = WebviewBridge::new();
        bridge.create_webview(1);
        assert_eq!(bridge.total_pending_messages(), 0);
        bridge.handle_message(&WebviewMessage::PostMessage {
            handle: 1,
            message: serde_json::json!("a"),
        });
        bridge.handle_message(&WebviewMessage::PostMessage {
            handle: 1,
            message: serde_json::json!("b"),
        });
        assert_eq!(bridge.total_pending_messages(), 2);
    }
}
