//! Ext API: Webview.
//!
//! RPC bridge between the extension host and the main thread for webview panels.

use std::collections::HashMap;
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

// ── HTML to Terminal Text ──

/// Converts simple HTML to plain text suitable for terminal display.
///
/// Handles: tag stripping, `<br>` → newline, `<p>` → double newline,
/// `<b>` content → UPPERCASE, and basic entity decoding.
pub fn html_to_terminal_text(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut chars = html.chars().peekable();
    let mut in_bold = false;

    while let Some(ch) = chars.next() {
        if ch == '&' {
            // Decode HTML entities.
            let mut entity = String::new();
            for ec in chars.by_ref() {
                if ec == ';' {
                    break;
                }
                entity.push(ec);
            }
            let decoded = match entity.as_str() {
                "amp" => "&",
                "lt" => "<",
                "gt" => ">",
                "quot" => "\"",
                _ => "",
            };
            if in_bold {
                out.push_str(&decoded.to_uppercase());
            } else {
                out.push_str(decoded);
            }
        } else if ch == '<' {
            // Read the tag name.
            let mut tag = String::new();
            for tc in chars.by_ref() {
                if tc == '>' {
                    break;
                }
                tag.push(tc);
            }
            let tag_lower = tag.trim().to_lowercase();
            if tag_lower == "br" || tag_lower == "br/" || tag_lower == "br /" {
                out.push('\n');
            } else if tag_lower == "p" {
                out.push_str("\n\n");
            } else if tag_lower == "b" {
                in_bold = true;
            } else if tag_lower == "/b" {
                in_bold = false;
            }
            // All other tags are silently stripped.
        } else if in_bold {
            for upper in ch.to_uppercase() {
                out.push(upper);
            }
        } else {
            out.push(ch);
        }
    }
    out
}

// ── Webview Message Bus ──

/// A publish/subscribe message bus for extension↔webview communication.
///
/// Subscribers register on named channels. Published messages are held as
/// pending until drained by a specific webview handle.
pub struct WebviewMessageBus {
    subscribers: Vec<(String, u64)>,
    pending: Vec<(String, serde_json::Value)>,
}

impl WebviewMessageBus {
    pub fn new() -> Self {
        Self {
            subscribers: Vec::new(),
            pending: Vec::new(),
        }
    }

    pub fn subscribe(&mut self, channel: &str, handle: u64) {
        let entry = (channel.to_string(), handle);
        if !self.subscribers.contains(&entry) {
            self.subscribers.push(entry);
        }
    }

    pub fn unsubscribe(&mut self, channel: &str, handle: u64) {
        self.subscribers.retain(|(c, h)| !(c == channel && *h == handle));
    }

    pub fn publish(&mut self, channel: &str, message: serde_json::Value) {
        self.pending.push((channel.to_string(), message));
    }

    /// Drain all pending messages whose channel the given handle is subscribed to.
    pub fn drain_for_handle(&mut self, handle: u64) -> Vec<(String, serde_json::Value)> {
        let subscribed_channels: Vec<String> = self
            .subscribers
            .iter()
            .filter(|(_, h)| *h == handle)
            .map(|(c, _)| c.clone())
            .collect();

        let mut delivered = Vec::new();
        let mut remaining = Vec::new();

        for (ch, msg) in self.pending.drain(..) {
            if subscribed_channels.contains(&ch) {
                delivered.push((ch, msg));
            } else {
                remaining.push((ch, msg));
            }
        }
        self.pending = remaining;
        delivered
    }

    pub fn subscriber_count(&self, channel: &str) -> usize {
        self.subscribers.iter().filter(|(c, _)| c == channel).count()
    }

    pub fn channel_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .subscribers
            .iter()
            .map(|(c, _)| c.clone())
            .collect();
        names.sort();
        names.dedup();
        names
    }
}

impl Default for WebviewMessageBus {
    fn default() -> Self {
        Self::new()
    }
}

// ── Webview Persistence ──

/// Stores serializable state for webview panels so it can be saved/restored.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WebviewPersistenceStore {
    states: Vec<(u64, serde_json::Value)>,
}

impl WebviewPersistenceStore {
    pub fn new() -> Self {
        Self { states: Vec::new() }
    }

    /// Save (or overwrite) the state for the given handle.
    pub fn save_state(&mut self, handle: u64, state: serde_json::Value) {
        if let Some(entry) = self.states.iter_mut().find(|(h, _)| *h == handle) {
            entry.1 = state;
        } else {
            self.states.push((handle, state));
        }
    }

    pub fn load_state(&self, handle: u64) -> Option<&serde_json::Value> {
        self.states.iter().find(|(h, _)| *h == handle).map(|(_, v)| v)
    }

    /// Remove the state for a handle. Returns `true` if it existed.
    pub fn remove_state(&mut self, handle: u64) -> bool {
        let before = self.states.len();
        self.states.retain(|(h, _)| *h != handle);
        self.states.len() < before
    }

    pub fn handles(&self) -> Vec<u64> {
        self.states.iter().map(|(h, _)| *h).collect()
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("WebviewPersistenceStore is always serializable")
    }

    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

impl Default for WebviewPersistenceStore {
    fn default() -> Self {
        Self::new()
    }
}

// ── Webview Theme Adapter ──

/// Known webview colour themes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WebviewThemeKind {
    Light,
    Dark,
    HighContrast,
}

/// Maps a colour theme to CSS custom-property values that a webview can inject.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WebviewThemeAdapter {
    pub kind: WebviewThemeKind,
    pub foreground: String,
    pub background: String,
    pub accent: String,
    pub font_family: String,
}

impl WebviewThemeAdapter {
    /// Create an adapter with sensible defaults for the given theme kind.
    pub fn from_kind(kind: WebviewThemeKind) -> Self {
        match kind {
            WebviewThemeKind::Light => Self {
                kind,
                foreground: "#1e1e1e".into(),
                background: "#ffffff".into(),
                accent: "#0066b8".into(),
                font_family: "system-ui, sans-serif".into(),
            },
            WebviewThemeKind::Dark => Self {
                kind,
                foreground: "#cccccc".into(),
                background: "#1e1e1e".into(),
                accent: "#569cd6".into(),
                font_family: "system-ui, sans-serif".into(),
            },
            WebviewThemeKind::HighContrast => Self {
                kind,
                foreground: "#ffffff".into(),
                background: "#000000".into(),
                accent: "#ffff00".into(),
                font_family: "system-ui, sans-serif".into(),
            },
        }
    }

    /// Render a `<style>` block containing CSS custom properties for the theme.
    pub fn to_css_variables(&self) -> String {
        format!(
            ":root {{\n  --vscode-foreground: {};\n  --vscode-background: {};\n  --vscode-accent: {};\n  --vscode-font-family: {};\n}}",
            self.foreground, self.background, self.accent, self.font_family
        )
    }

    /// Wrap existing HTML content with theme CSS injected into a `<style>` tag.
    pub fn wrap_html(&self, body_html: &str) -> String {
        format!(
            "<style>{}</style>\n{}",
            self.to_css_variables(),
            body_html
        )
    }
}

impl Default for WebviewThemeAdapter {
    fn default() -> Self {
        Self::from_kind(WebviewThemeKind::Dark)
    }
}

// ── Webview Resource Loader ──

/// Entry in the resource loader mapping a short alias to a resolved URI.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResourceEntry {
    pub alias: String,
    pub uri: String,
}

/// Manages a set of named resource mappings so extensions can reference assets
/// by alias instead of full URIs.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct WebviewResourceLoader {
    entries: Vec<ResourceEntry>,
}

impl WebviewResourceLoader {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a resource alias. Returns an error if the alias already exists.
    pub fn register(
        &mut self,
        alias: impl Into<String>,
        uri: impl Into<String>,
    ) -> Result<(), WebviewError> {
        let alias = alias.into();
        let uri = uri.into();
        if alias.is_empty() {
            return Err(WebviewError::InvalidContent("alias must not be empty".into()));
        }
        if self.entries.iter().any(|e| e.alias == alias) {
            return Err(WebviewError::DuplicateHandle(0));
        }
        if !validate_webview_uri(&uri) {
            return Err(WebviewError::InvalidResourceRoot(uri));
        }
        self.entries.push(ResourceEntry { alias, uri });
        Ok(())
    }

    /// Resolve an alias to its URI, returning `None` if unregistered.
    pub fn resolve(&self, alias: &str) -> Option<&str> {
        self.entries
            .iter()
            .find(|e| e.alias == alias)
            .map(|e| e.uri.as_str())
    }

    /// Remove a resource entry by alias. Returns `true` if it existed.
    pub fn unregister(&mut self, alias: &str) -> bool {
        let before = self.entries.len();
        self.entries.retain(|e| e.alias != alias);
        self.entries.len() < before
    }

    /// Return all registered aliases.
    pub fn aliases(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.alias.as_str()).collect()
    }

    /// Generate `<link>` / `<script>` tags for all registered resources based
    /// on file extension heuristics.
    pub fn to_html_tags(&self) -> String {
        let mut out = String::new();
        for entry in &self.entries {
            if entry.uri.ends_with(".css") {
                out.push_str(&format!(
                    "<link rel=\"stylesheet\" href=\"{}\">\n",
                    entry.uri
                ));
            } else if entry.uri.ends_with(".js") {
                out.push_str(&format!(
                    "<script src=\"{}\"></script>\n",
                    entry.uri
                ));
            }
        }
        out
    }

    /// Return the total number of registered resources.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Return `true` when no resources are registered.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

// ── Content Security Policy Builder ──

/// Fluent builder for constructing Content-Security-Policy header values.
///
/// Each directive is accumulated independently, then serialised into a single
/// semicolon-separated header string via [`CspBuilder::build`].
#[derive(Debug, Clone, Default)]
pub struct CspBuilder {
    directives: HashMap<String, Vec<String>>,
}

impl CspBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add one or more source expressions to a directive (e.g. `"script-src"`, `"'self'"`).
    pub fn add(mut self, directive: &str, source: &str) -> Self {
        self.directives
            .entry(directive.to_string())
            .or_default()
            .push(source.to_string());
        self
    }

    /// Shorthand: set `default-src` to `'none'` (strictest baseline).
    pub fn default_none(self) -> Self {
        self.add("default-src", "'none'")
    }

    /// Shorthand: set `default-src` to `'self'`.
    pub fn default_self(self) -> Self {
        self.add("default-src", "'self'")
    }

    /// Allow inline scripts guarded by a nonce.
    pub fn script_nonce(self, nonce: &str) -> Self {
        self.add("script-src", &format!("'nonce-{nonce}'"))
    }

    /// Allow styles from `'self'` plus an optional nonce.
    pub fn style_self_with_nonce(self, nonce: Option<&str>) -> Self {
        let s = match nonce {
            Some(n) => self.add("style-src", "'self'").add("style-src", &format!("'nonce-{n}'")),
            None => self.add("style-src", "'self'"),
        };
        s
    }

    /// Allow images from `https:` and `data:` schemes (common for webview panels).
    pub fn img_https_data(self) -> Self {
        self.add("img-src", "https:").add("img-src", "data:")
    }

    /// Serialize the accumulated directives into a CSP header value.
    ///
    /// Directives are sorted alphabetically for deterministic output.
    pub fn build(&self) -> String {
        let mut keys: Vec<&String> = self.directives.keys().collect();
        keys.sort();
        keys.iter()
            .map(|k| {
                let sources = self.directives[*k].join(" ");
                format!("{k} {sources}")
            })
            .collect::<Vec<_>>()
            .join("; ")
    }
}

// ── Webview Panel Layout Tracking ──

/// Describes the editor column a webview panel occupies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ViewColumn {
    One,
    Two,
    Three,
    /// Panel is in the side-bar area rather than an editor column.
    Sidebar,
}

/// Tracks the layout state of a single webview panel.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PanelLayoutEntry {
    pub handle: u64,
    pub title: String,
    pub column: ViewColumn,
    pub active: bool,
    pub visible: bool,
}

/// Registry that tracks the layout positions and visibility of all webview panels.
#[derive(Debug, Clone, Default)]
pub struct PanelLayoutTracker {
    panels: Vec<PanelLayoutEntry>,
}

impl PanelLayoutTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a new panel in the given column.
    pub fn add_panel(&mut self, handle: u64, title: impl Into<String>, column: ViewColumn) {
        if self.panels.iter().any(|p| p.handle == handle) {
            return;
        }
        self.panels.push(PanelLayoutEntry {
            handle,
            title: title.into(),
            column,
            active: false,
            visible: true,
        });
    }

    /// Remove a panel by handle. Returns `true` if it existed.
    pub fn remove_panel(&mut self, handle: u64) -> bool {
        let before = self.panels.len();
        self.panels.retain(|p| p.handle != handle);
        self.panels.len() < before
    }

    /// Move a panel to a different column.
    pub fn move_panel(&mut self, handle: u64, column: ViewColumn) -> bool {
        if let Some(p) = self.panels.iter_mut().find(|p| p.handle == handle) {
            p.column = column;
            true
        } else {
            false
        }
    }

    /// Set exactly one panel as active within its column, deactivating others
    /// in the same column.
    pub fn set_active(&mut self, handle: u64) -> bool {
        let col = match self.panels.iter().find(|p| p.handle == handle) {
            Some(p) => p.column,
            None => return false,
        };
        for p in &mut self.panels {
            if p.column == col {
                p.active = p.handle == handle;
            }
        }
        true
    }

    /// Return the currently active panel in the given column, if any.
    pub fn active_in_column(&self, column: ViewColumn) -> Option<&PanelLayoutEntry> {
        self.panels.iter().find(|p| p.column == column && p.active)
    }

    /// List all panels in a given column, ordered by insertion.
    pub fn panels_in_column(&self, column: ViewColumn) -> Vec<&PanelLayoutEntry> {
        self.panels.iter().filter(|p| p.column == column).collect()
    }

    /// Toggle the visibility of a panel.
    pub fn set_visible(&mut self, handle: u64, visible: bool) -> bool {
        if let Some(p) = self.panels.iter_mut().find(|p| p.handle == handle) {
            p.visible = visible;
            true
        } else {
            false
        }
    }

    /// Return all visible panels across every column.
    pub fn visible_panels(&self) -> Vec<&PanelLayoutEntry> {
        self.panels.iter().filter(|p| p.visible).collect()
    }

    pub fn panel_count(&self) -> usize {
        self.panels.len()
    }

    pub fn get_panel(&self, handle: u64) -> Option<&PanelLayoutEntry> {
        self.panels.iter().find(|p| p.handle == handle)
    }
}

// ── Webview Lifecycle Management ──

/// Lifecycle states a webview panel can be in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LifecycleState {
    /// Webview has been created but not yet rendered.
    Created,
    /// Webview has completed initial render and is ready.
    Ready,
    /// Webview is the focused/active panel.
    Active,
    /// Webview is backgrounded or hidden to save resources.
    Suspended,
    /// Webview has been disposed and cannot be reused.
    Disposed,
}

impl LifecycleState {
    /// Returns the set of states that are valid transitions from `self`.
    pub fn valid_transitions(self) -> &'static [LifecycleState] {
        match self {
            Self::Created => &[Self::Ready, Self::Disposed],
            Self::Ready => &[Self::Active, Self::Suspended, Self::Disposed],
            Self::Active => &[Self::Ready, Self::Suspended, Self::Disposed],
            Self::Suspended => &[Self::Ready, Self::Active, Self::Disposed],
            Self::Disposed => &[],
        }
    }

    /// Returns `true` if transitioning from `self` to `target` is valid.
    pub fn can_transition_to(self, target: LifecycleState) -> bool {
        self.valid_transitions().contains(&target)
    }
}

impl fmt::Display for LifecycleState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Created => "created",
            Self::Ready => "ready",
            Self::Active => "active",
            Self::Suspended => "suspended",
            Self::Disposed => "disposed",
        };
        f.write_str(s)
    }
}

/// Tracks the lifecycle state of multiple webview panels and enforces valid
/// state transitions.
#[derive(Debug, Clone, Default)]
pub struct WebviewLifecycleManager {
    states: HashMap<u64, LifecycleState>,
}

impl WebviewLifecycleManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a new webview in the `Created` state.
    pub fn register(&mut self, handle: u64) -> Result<(), WebviewError> {
        if self.states.contains_key(&handle) {
            return Err(WebviewError::DuplicateHandle(handle));
        }
        self.states.insert(handle, LifecycleState::Created);
        Ok(())
    }

    /// Transition a webview to a new lifecycle state.
    pub fn transition(
        &mut self,
        handle: u64,
        target: LifecycleState,
    ) -> Result<LifecycleState, WebviewError> {
        let current = self
            .states
            .get(&handle)
            .copied()
            .ok_or(WebviewError::NotFound(handle))?;
        if !current.can_transition_to(target) {
            return Err(WebviewError::InvalidContent(format!(
                "invalid transition from {} to {}",
                current, target
            )));
        }
        self.states.insert(handle, target);
        Ok(target)
    }

    /// Get the current state of a webview.
    pub fn state_of(&self, handle: u64) -> Option<LifecycleState> {
        self.states.get(&handle).copied()
    }

    /// Return all handles currently in the given state.
    pub fn handles_in_state(&self, state: LifecycleState) -> Vec<u64> {
        self.states
            .iter()
            .filter(|&(_, s)| *s == state)
            .map(|(&h, _)| h)
            .collect()
    }

    /// Dispose all webviews that are not already disposed.
    /// Returns the number of webviews that were transitioned.
    pub fn dispose_all(&mut self) -> usize {
        let mut count = 0;
        for (_, state) in self.states.iter_mut() {
            if *state != LifecycleState::Disposed {
                *state = LifecycleState::Disposed;
                count += 1;
            }
        }
        count
    }

    /// Return the total number of tracked webviews (including disposed).
    pub fn tracked_count(&self) -> usize {
        self.states.len()
    }
}

// ── HTML Content Sanitizer ──

/// Tags considered dangerous for webview rendering.
const DANGEROUS_TAGS: &[&str] = &[
    "script", "iframe", "object", "embed", "applet", "form", "input",
    "textarea", "button", "select",
];

/// Sanitize HTML by removing dangerous tags and their contents.
///
/// This performs a simple tag-level scan — it is not a full HTML parser.
/// Tags listed in [`DANGEROUS_TAGS`] are removed along with everything
/// between their opening and closing tags.
pub fn sanitize_html(html: &str) -> String {
    let mut result = html.to_string();
    for tag in DANGEROUS_TAGS {
        // Remove paired tags and their content: <script>...</script>
        loop {
            let open = format!("<{}", tag);
            let close = format!("</{}>", tag);
            let start = result.to_lowercase().find(&open);
            let end = result.to_lowercase().find(&close);
            match (start, end) {
                (Some(s), Some(e)) if s <= e => {
                    result.replace_range(s..e + close.len(), "");
                }
                _ => break,
            }
        }
        // Remove self-closing variants: <script/>, <script />
        loop {
            let lower = result.to_lowercase();
            if let Some(s) = lower.find(&format!("<{}", tag)) {
                if let Some(e) = result[s..].find('>') {
                    let tag_content = &result[s..s + e + 1];
                    if tag_content.contains('/') || DANGEROUS_TAGS.contains(&tag) {
                        result.replace_range(s..s + e + 1, "");
                        continue;
                    }
                }
            }
            break;
        }
    }
    result
}

/// Returns `true` if the HTML string contains any dangerous tags.
pub fn html_has_dangerous_content(html: &str) -> bool {
    let lower = html.to_lowercase();
    DANGEROUS_TAGS
        .iter()
        .any(|tag| lower.contains(&format!("<{}", tag)))
}

// ── Webview Message Protocol ──

/// A typed message envelope for extension↔webview communication with
/// sequence numbering for request/response correlation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MessageEnvelope {
    /// Monotonically increasing sequence number.
    pub seq: u64,
    /// If this is a response, the sequence number of the original request.
    pub request_seq: Option<u64>,
    /// The webview handle this message is associated with.
    pub handle: u64,
    /// Message channel/topic.
    pub channel: String,
    /// The message payload.
    pub payload: serde_json::Value,
}

/// Generates sequenced message envelopes for a single webview handle.
#[derive(Debug, Clone)]
pub struct MessageEnvelopeFactory {
    handle: u64,
    next_seq: u64,
}

impl MessageEnvelopeFactory {
    pub fn new(handle: u64) -> Self {
        Self {
            handle,
            next_seq: 1,
        }
    }

    /// Create a new request envelope.
    pub fn request(&mut self, channel: impl Into<String>, payload: serde_json::Value) -> MessageEnvelope {
        let seq = self.next_seq;
        self.next_seq += 1;
        MessageEnvelope {
            seq,
            request_seq: None,
            handle: self.handle,
            channel: channel.into(),
            payload,
        }
    }

    /// Create a response envelope correlated to a request.
    pub fn response(
        &mut self,
        request_seq: u64,
        channel: impl Into<String>,
        payload: serde_json::Value,
    ) -> MessageEnvelope {
        let seq = self.next_seq;
        self.next_seq += 1;
        MessageEnvelope {
            seq,
            request_seq: Some(request_seq),
            handle: self.handle,
            channel: channel.into(),
            payload,
        }
    }

    /// Return the next sequence number that will be assigned.
    pub fn peek_next_seq(&self) -> u64 {
        self.next_seq
    }
}

// ── Extended Bridge: Bulk Operations & Search ──

impl WebviewBridge {
    /// Dispose multiple webviews at once. Returns the count of actually removed webviews.
    pub fn bulk_dispose(&mut self, handles: &[u64]) -> usize {
        let before = self.webviews.len();
        self.webviews.retain(|w| !handles.contains(&w.handle));
        self.messages.retain(|(h, _)| !handles.contains(h));
        before - self.webviews.len()
    }

    /// Find all webview handles whose HTML content contains the given substring.
    pub fn find_by_html_contains(&self, needle: &str) -> Vec<u64> {
        self.webviews
            .iter()
            .filter(|w| w.html.contains(needle))
            .map(|w| w.handle)
            .collect()
    }

    /// Return a summary map of handle → html length for diagnostics.
    pub fn content_size_map(&self) -> HashMap<u64, usize> {
        self.webviews
            .iter()
            .map(|w| (w.handle, w.html.len()))
            .collect()
    }

    /// Clone the options from one webview to another.
    pub fn clone_options(
        &mut self,
        source: u64,
        target: u64,
    ) -> Result<(), WebviewError> {
        let opts = self
            .webviews
            .iter()
            .find(|w| w.handle == source)
            .map(|w| w.options.clone())
            .ok_or(WebviewError::NotFound(source))?;
        let tw = self
            .webviews
            .iter_mut()
            .find(|w| w.handle == target)
            .ok_or(WebviewError::NotFound(target))?;
        tw.options = opts;
        Ok(())
    }
}


// -- Webview Content Sanitizer --

/// Sanitizes HTML content for safe rendering in webview panels.
#[derive(Debug, Clone)]
pub struct WebviewContentSanitizer {
    allowed_tags: Vec<String>,
    allowed_attrs: Vec<String>,
    strip_scripts: bool,
    strip_event_handlers: bool,
    max_content_length: usize,
}

/// Result of a sanitization pass.
#[derive(Debug, Clone, PartialEq)]
pub struct SanitizeResult {
    pub output: String,
    pub elements_removed: usize,
    pub attrs_removed: usize,
    pub was_truncated: bool,
}

impl Default for WebviewContentSanitizer {
    fn default() -> Self {
        Self {
            allowed_tags: ["div","span","p","h1","h2","h3","a","img","ul","ol","li","table","tr","td","th","br","hr","em","strong","code","pre"].iter().map(|s| s.to_string()).collect(),
            allowed_attrs: ["class","id","href","src","alt","title","style"].iter().map(|s| s.to_string()).collect(),
            strip_scripts: true,
            strip_event_handlers: true,
            max_content_length: 1_000_000,
        }
    }
}

impl WebviewContentSanitizer {
    pub fn new() -> Self { Self::default() }

    pub fn with_max_length(mut self, max: usize) -> Self {
        self.max_content_length = max;
        self
    }

    pub fn allow_tag(mut self, tag: &str) -> Self {
        if !self.allowed_tags.iter().any(|t| t == tag) {
            self.allowed_tags.push(tag.to_string());
        }
        self
    }

    pub fn is_tag_allowed(&self, tag: &str) -> bool {
        self.allowed_tags.iter().any(|t| t.eq_ignore_ascii_case(tag))
    }

    pub fn is_attr_allowed(&self, attr: &str) -> bool {
        if self.strip_event_handlers && attr.starts_with("on") { return false; }
        self.allowed_attrs.iter().any(|a| a.eq_ignore_ascii_case(attr))
    }

    pub fn sanitize(&self, html: &str) -> SanitizeResult {
        let mut output = html.to_string();
        let mut elements_removed = 0usize;
        let mut attrs_removed = 0usize;
        let was_truncated = output.len() > self.max_content_length;
        if was_truncated { output.truncate(self.max_content_length); }
        if self.strip_scripts {
            while let Some(start) = output.to_lowercase().find("<script") {
                if let Some(end) = output[start..].to_lowercase().find("</script>") {
                    output = format!("{}{}", &output[..start], &output[start + end + 9..]);
                    elements_removed += 1;
                } else {
                    output = output[..start].to_string();
                    elements_removed += 1;
                    break;
                }
            }
        }
        if self.strip_event_handlers {
            for pat in &["onclick=", "onload=", "onerror=", "onmouseover="] {
                while output.to_lowercase().contains(pat) {
                    if let Some(pos) = output.to_lowercase().find(pat) {
                        let rest = &output[pos..];
                        let end = rest.find(|c: char| c == ' ' || c == '>' || c == '/').unwrap_or(rest.len());
                        output = format!("{}{}", &output[..pos], &output[pos + end..]);
                        attrs_removed += 1;
                    }
                }
            }
        }
        SanitizeResult { output, elements_removed, attrs_removed, was_truncated }
    }
}

// -- Webview Resource Mapper --

/// Maps webview URIs to local filesystem paths.
#[derive(Debug, Clone)]
pub struct WebviewResourceMapper {
    mappings: HashMap<String, String>,
    scheme_prefix: String,
}

impl WebviewResourceMapper {
    pub fn new(scheme_prefix: &str) -> Self {
        Self { mappings: HashMap::new(), scheme_prefix: scheme_prefix.to_string() }
    }
    pub fn add_mapping(&mut self, webview_uri: &str, local_path: &str) {
        self.mappings.insert(webview_uri.to_string(), local_path.to_string());
    }
    pub fn resolve(&self, webview_uri: &str) -> Option<&str> {
        self.mappings.get(webview_uri).map(|s| s.as_str())
    }
    pub fn to_webview_uri(&self, local_path: &str) -> String {
        format!("{}://{}", self.scheme_prefix, local_path)
    }
    pub fn mapping_count(&self) -> usize { self.mappings.len() }
    pub fn clear_mappings(&mut self) { self.mappings.clear(); }
    pub fn has_mapping(&self, uri: &str) -> bool { self.mappings.contains_key(uri) }

    pub fn guess_mime(path: &str) -> &'static str {
        let lower = path.to_lowercase();
        if lower.ends_with(".html") || lower.ends_with(".htm") { "text/html" }
        else if lower.ends_with(".css") { "text/css" }
        else if lower.ends_with(".js") { "application/javascript" }
        else if lower.ends_with(".json") { "application/json" }
        else if lower.ends_with(".png") { "image/png" }
        else if lower.ends_with(".svg") { "image/svg+xml" }
        else { "application/octet-stream" }
    }
}


// -- Webview CSP Directive Builder --

/// Builds Content-Security-Policy directives for webview panels.
#[derive(Debug, Clone)]
pub struct WebviewCspDirectiveBuilder {
    directives: Vec<(String, Vec<String>)>,
}

impl WebviewCspDirectiveBuilder {
    pub fn new() -> Self {
        Self { directives: Vec::new() }
    }

    pub fn add_directive(&mut self, name: &str, values: &[&str]) -> &mut Self {
        self.directives.push((
            name.to_string(),
            values.iter().map(|v| v.to_string()).collect(),
        ));
        self
    }

    pub fn default_src(&mut self, sources: &[&str]) -> &mut Self {
        self.add_directive("default-src", sources)
    }

    pub fn script_src(&mut self, sources: &[&str]) -> &mut Self {
        self.add_directive("script-src", sources)
    }

    pub fn style_src(&mut self, sources: &[&str]) -> &mut Self {
        self.add_directive("style-src", sources)
    }

    pub fn img_src(&mut self, sources: &[&str]) -> &mut Self {
        self.add_directive("img-src", sources)
    }

    pub fn font_src(&mut self, sources: &[&str]) -> &mut Self {
        self.add_directive("font-src", sources)
    }

    pub fn connect_src(&mut self, sources: &[&str]) -> &mut Self {
        self.add_directive("connect-src", sources)
    }

    pub fn build(&self) -> String {
        self.directives
            .iter()
            .map(|(name, values)| format!("{} {}", name, values.join(" ")))
            .collect::<Vec<_>>()
            .join("; ")
    }

    pub fn directive_count(&self) -> usize {
        self.directives.len()
    }

    pub fn has_directive(&self, name: &str) -> bool {
        self.directives.iter().any(|(n, _)| n == name)
    }

    /// Create a restrictive CSP suitable for untrusted content.
    pub fn restrictive() -> Self {
        let mut builder = Self::new();
        builder
            .default_src(&["'none'"])
            .script_src(&["'none'"])
            .style_src(&["'unsafe-inline'"])
            .img_src(&["data:", "https:"])
            .font_src(&["data:"]);
        builder
    }

    /// Create a permissive CSP for trusted extensions.
    pub fn permissive() -> Self {
        let mut builder = Self::new();
        builder
            .default_src(&["'self'", "https:"])
            .script_src(&["'self'", "'unsafe-inline'", "'unsafe-eval'"])
            .style_src(&["'self'", "'unsafe-inline'"])
            .img_src(&["'self'", "data:", "https:"])
            .font_src(&["'self'", "data:", "https:"]);
        builder
    }
}

impl Default for WebviewCspDirectiveBuilder {
    fn default() -> Self { Self::new() }
}


// ---------------------------------------------------------------------------
// ext_webview – Extension protocol helpers
// ---------------------------------------------------------------------------

/// Activation event kinds for extension lifecycle management.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum XExtWebviewActivationKind {
    /// Activate on a specific language.
    Language(String),
    /// Activate on a command.
    Command(String),
    /// Activate on a workspace-contains glob.
    WorkspaceContains(String),
    /// Activate on a custom URI scheme.
    UriScheme(String),
    /// Activate on startup.
    Star,
}

impl XExtWebviewActivationKind {
    /// Parse an activation event string like `"onLanguage:rust"`.
    pub fn parse(raw: &str) -> Option<Self> {
        if raw == "*" {
            return Some(Self::Star);
        }
        let (kind, value) = raw.split_once(':')?;
        match kind {
            "onLanguage" => Some(Self::Language(value.to_string())),
            "onCommand" => Some(Self::Command(value.to_string())),
            "workspaceContains" => Some(Self::WorkspaceContains(value.to_string())),
            "onUri" => Some(Self::UriScheme(value.to_string())),
            _ => None,
        }
    }

    /// Returns true if this activation kind targets a specific language.
    pub fn is_language(&self) -> bool {
        matches!(self, Self::Language(_))
    }
}

/// Message envelope for extension host RPC.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XExtWebviewRpcEnvelope {
    pub seq: u64,
    pub method: String,
    pub payload: String,
}

impl XExtWebviewRpcEnvelope {
    /// Create a new RPC envelope.
    pub fn new(seq: u64, method: impl Into<String>, payload: impl Into<String>) -> Self {
        Self { seq, method: method.into(), payload: payload.into() }
    }

    /// Returns true when the envelope carries a response (method starts with `$/`).
    pub fn is_response(&self) -> bool {
        self.method.starts_with("$/")
    }

    /// Compute a simple checksum of the payload (sum of bytes mod 2^32).
    pub fn payload_checksum(&self) -> u32 {
        self.payload.bytes().fold(0u32, |acc, b| acc.wrapping_add(b as u32))
    }
}

/// Batch multiple RPC envelopes and return their sequence numbers.
pub fn x_ext_webview_collect_sequences(envelopes: &[XExtWebviewRpcEnvelope]) -> Vec<u64> {
    envelopes.iter().map(|e| e.seq).collect()
}

/// Filter envelopes by method prefix.
pub fn x_ext_webview_filter_by_method<'a>(
    envelopes: &'a [XExtWebviewRpcEnvelope],
    method_prefix: &str,
) -> Vec<&'a XExtWebviewRpcEnvelope> {
    envelopes.iter().filter(|e| e.method.starts_with(method_prefix)).collect()
}

/// Deduplicate envelopes by sequence number, keeping the first occurrence.
pub fn x_ext_webview_dedup_by_seq(envelopes: Vec<XExtWebviewRpcEnvelope>) -> Vec<XExtWebviewRpcEnvelope> {
    let mut seen = std::collections::HashSet::new();
    envelopes.into_iter().filter(|e| seen.insert(e.seq)).collect()
}

/// Simple capability negotiation: given requested and available feature sets,
/// return the intersection.
pub fn x_ext_webview_negotiate_capabilities(
    requested: &[&str],
    available: &[&str],
) -> Vec<String> {
    requested.iter()
        .filter(|r| available.contains(r))
        .map(|s| s.to_string())
        .collect()
}

/// Version tuple for extension API compatibility checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct XExtWebviewApiVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl XExtWebviewApiVersion {
    pub fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self { major, minor, patch }
    }
    /// Check if this version satisfies a minimum requirement.
    pub fn satisfies(&self, min: &Self) -> bool {
        (self.major, self.minor, self.patch) >= (min.major, min.minor, min.patch)
    }
}

impl std::fmt::Display for XExtWebviewApiVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}


/// Configuration manager for ext_webview functionality.
pub struct ExtWebviewConfig {
    options: HashMap<String, String>,
    enabled: bool,
    version: u32,
}

impl ExtWebviewConfig {
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

    pub fn merge(&mut self, other: &ExtWebviewConfig) {
        for (k, v) in &other.options {
            self.options.insert(k.clone(), v.clone());
        }
    }
}

/// Rate tracker for ext_webview operations.
pub struct ExtWebviewRateTracker {
    window_ms: u64,
    timestamps: Vec<u64>,
}

impl ExtWebviewRateTracker {
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

/// Validation result collector for ext_webview.
pub struct ExtWebviewValidator {
    errors: Vec<String>,
    warnings: Vec<String>,
}

impl ExtWebviewValidator {
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

    pub fn merge(&mut self, other: &ExtWebviewValidator) {
        self.errors.extend(other.errors.iter().cloned());
        self.warnings.extend(other.warnings.iter().cloned());
    }

    pub fn first_error(&self) -> Option<&str> {
        self.errors.first().map(|s| s.as_str())
    }
}


// ── zq extended utilities ──

/// A lightweight tagged-value store for zq operations.
#[derive(Debug, Clone, PartialEq)]
pub struct ZqStore {
    entries: Vec<(String, String)>,
    capacity: usize,
}

impl ZqStore {
    /// Create a new store with the given capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: Vec::new(),
            capacity,
        }
    }

    /// Insert a key-value pair, evicting the oldest if at capacity.
    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<String>) -> bool {
        let key = key.into();
        let value = value.into();
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push((key, value));
        true
    }

    /// Look up a value by key.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries
            .iter()
            .rev()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    /// Remove all entries matching the given key, returning how many were removed.
    pub fn remove(&mut self, key: &str) -> usize {
        let before = self.entries.len();
        self.entries.retain(|(k, _)| k != key);
        before - self.entries.len()
    }

    /// Return the number of stored entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check whether the store is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Collect all keys in insertion order.
    pub fn keys(&self) -> Vec<&str> {
        self.entries.iter().map(|(k, _)| k.as_str()).collect()
    }

    /// Collect all values in insertion order.
    pub fn values(&self) -> Vec<&str> {
        self.entries.iter().map(|(_, v)| v.as_str()).collect()
    }

    /// Drain entries whose key starts with the given prefix.
    pub fn drain_prefix(&mut self, pfx: &str) -> Vec<(String, String)> {
        let mut drained = Vec::new();
        let mut i = 0;
        while i < self.entries.len() {
            if self.entries[i].0.starts_with(pfx) {
                drained.push(self.entries.remove(i));
            } else {
                i += 1;
            }
        }
        drained
    }

    /// Retain only entries satisfying the predicate.
    pub fn retain<F: Fn(&str, &str) -> bool>(&mut self, f: F) {
        self.entries.retain(|(k, v)| f(k, v));
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Return remaining capacity.
    pub fn remaining(&self) -> usize {
        self.capacity.saturating_sub(self.entries.len())
    }

    /// Merge another store into this one, respecting capacity.
    pub fn merge(&mut self, other: &ZqStore) {
        for (k, v) in &other.entries {
            if self.entries.len() >= self.capacity {
                break;
            }
            self.entries.push((k.clone(), v.clone()));
        }
    }
}

/// Format a byte count as a human-readable string for zq display.
pub fn zq_format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Truncate a string to `max_len` characters, appending an ellipsis if needed.
pub fn zq_truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let mut result = s[..max_len.saturating_sub(3)].to_string();
        result.push_str("...");
        result
    }
}


// ---------------------------------------------------------------------------
// xa_ extended helpers for ext_webview
// ---------------------------------------------------------------------------

/// A bounded ring-buffer that stores `xa_` metric samples.
pub struct XaExtWebviewRingBuf {
    buf: Vec<f64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XaExtWebviewRingBuf {
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
pub struct XaExtWebviewCounter {
    counts: std::collections::HashMap<String, u64>,
}

impl XaExtWebviewCounter {
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

impl Default for XaExtWebviewCounter {
    fn default() -> Self {
        Self::new()
    }
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 77
// ---------------------------------------------------------------------------

/// Generic object pool `Xc77Pool<T>`.
pub struct Xc77Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc77Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc77PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc77Pool<T> {
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
    pub fn stats(&self) -> Xc77PoolStats {
        Xc77PoolStats {
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

impl<T> Default for Xc77Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc77Scheduler`.
pub struct Xc77Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc77Scheduler {
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

impl Default for Xc77Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_77 hash for the given byte slice.
pub fn xc_77_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_77 convention.
pub fn xc_77_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe18 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe18Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe18PipelineError {
    pub stage: Xe18Stage,
    pub message: String,
}

impl std::fmt::Display for Xe18PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe18Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe18Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe18PipelineError>>>,
    stage_names: Vec<Xe18Stage>,
}

impl Xe18Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe18PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe18Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe18PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe18Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe18PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe18Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe18PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe18Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe18PipelineError> {
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

    pub fn compose(mut self, other: Xe18Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe18CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe18CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe18Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe18CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe18CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe18Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe18CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_18_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe18CacheEntry {
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

    fn xe_18_evict_expired(&mut self) {
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

    pub fn stats(&self) -> &Xe18CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_18_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe18PipelineError> {
    Ok(data)
}

pub fn xe_18_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe18PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_18_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe18PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_18_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe18PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_18_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe18PipelineError> {
    Err(Xe18PipelineError {
        stage: Xe18Stage::Parse,
        message: "intentional failure".to_string(),
    })
}


// ---------------------------------------------------------------------------
// xf_ data structures (Trie + BloomFilter) — unique instance #88
// ---------------------------------------------------------------------------

/// A node in the prefix tree `Xf88Trie`.
#[derive(Debug, Clone, Default)]
pub struct Xf88TrieNode {
    children: std::collections::HashMap<char, Xf88TrieNode>,
    is_end: bool,
}

/// Prefix tree with insert, search, starts_with, remove, word_count,
/// longest_prefix, all_words, and autocomplete.
#[derive(Debug, Clone, Default)]
pub struct Xf88Trie {
    root: Xf88TrieNode,
    count: usize,
}

impl Xf88Trie {
    /// Create an empty trie.
    pub fn xf_new() -> Self {
        Self { root: Xf88TrieNode::default(), count: 0 }
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

    fn xf_remove_recursive(node: &mut Xf88TrieNode, word: &str, depth: usize) -> bool {
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

    fn xf_collect(node: &Xf88TrieNode, buf: &mut String, out: &mut Vec<String>) {
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
pub struct Xf88BloomFilter {
    bits: Vec<bool>,
    num_hashes: usize,
    len: usize,
    item_count: usize,
}

impl Xf88BloomFilter {
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


/// A probabilistic sorted list using a skip-list structure (variant 76).
pub struct Xh76SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh76SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 118 as u64,
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

/// A compact bit set supporting boolean operations (variant 76).
pub struct Xh76BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh76BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 76).
pub struct Xi76Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi76Deque<T> {
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
pub struct Xi76Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi76Interval {
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

/// A simple interval tree (variant 76).
pub struct Xi76IntervalTree {
    xi_intervals: Vec<Xi76Interval>,
}

impl Xi76IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi76Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi76Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi76Interval) -> Vec<&Xi76Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi76Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi76Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi76Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi76Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi76Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi76Interval> = Vec::new();
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

    // ── html_to_terminal_text tests ──

    #[test]
    fn html_to_text_strips_tags() {
        assert_eq!(html_to_terminal_text("<div>hello</div>"), "hello");
        assert_eq!(html_to_terminal_text("<span>a</span><span>b</span>"), "ab");
    }

    #[test]
    fn html_to_text_br_and_p() {
        assert_eq!(html_to_terminal_text("a<br>b"), "a\nb");
        assert_eq!(html_to_terminal_text("a<br/>b"), "a\nb");
        assert_eq!(html_to_terminal_text("a<br />b"), "a\nb");
        assert_eq!(html_to_terminal_text("x<p>y"), "x\n\ny");
    }

    #[test]
    fn html_to_text_bold_uppercases() {
        assert_eq!(html_to_terminal_text("say <b>hello</b> world"), "say HELLO world");
    }

    #[test]
    fn html_to_text_decodes_entities() {
        assert_eq!(html_to_terminal_text("a &amp; b"), "a & b");
        assert_eq!(html_to_terminal_text("&lt;tag&gt;"), "<tag>");
        assert_eq!(html_to_terminal_text("&quot;hi&quot;"), "\"hi\"");
    }

    // ── WebviewMessageBus tests ──

    #[test]
    fn message_bus_subscribe_and_publish() {
        let mut bus = WebviewMessageBus::new();
        bus.subscribe("events", 1);
        bus.subscribe("events", 2);
        bus.publish("events", serde_json::json!("hello"));

        assert_eq!(bus.subscriber_count("events"), 2);
        let msgs = bus.drain_for_handle(1);
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].0, "events");
        assert_eq!(msgs[0].1, serde_json::json!("hello"));
    }

    #[test]
    fn message_bus_unsubscribe() {
        let mut bus = WebviewMessageBus::new();
        bus.subscribe("ch", 1);
        bus.subscribe("ch", 2);
        assert_eq!(bus.subscriber_count("ch"), 2);

        bus.unsubscribe("ch", 1);
        assert_eq!(bus.subscriber_count("ch"), 1);

        bus.publish("ch", serde_json::json!(42));
        let msgs = bus.drain_for_handle(1);
        assert!(msgs.is_empty());
    }

    #[test]
    fn message_bus_channel_names() {
        let mut bus = WebviewMessageBus::new();
        bus.subscribe("beta", 1);
        bus.subscribe("alpha", 2);
        bus.subscribe("beta", 3);
        let names = bus.channel_names();
        assert_eq!(names, vec!["alpha", "beta"]);
    }

    #[test]
    fn message_bus_drain_leaves_other_channels() {
        let mut bus = WebviewMessageBus::new();
        bus.subscribe("a", 1);
        bus.subscribe("b", 2);
        bus.publish("a", serde_json::json!("for-a"));
        bus.publish("b", serde_json::json!("for-b"));

        let msgs = bus.drain_for_handle(1);
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].1, serde_json::json!("for-a"));

        // "for-b" should still be pending
        let msgs2 = bus.drain_for_handle(2);
        assert_eq!(msgs2.len(), 1);
        assert_eq!(msgs2[0].1, serde_json::json!("for-b"));
    }

    // ── WebviewPersistenceStore tests ──

    #[test]
    fn persistence_save_load_remove() {
        let mut store = WebviewPersistenceStore::new();
        store.save_state(1, serde_json::json!({"scroll": 100}));
        store.save_state(2, serde_json::json!({"scroll": 200}));

        assert_eq!(store.load_state(1), Some(&serde_json::json!({"scroll": 100})));
        assert_eq!(store.load_state(3), None);
        assert!(store.remove_state(1));
        assert!(!store.remove_state(1));
        assert_eq!(store.handles(), vec![2]);
    }

    #[test]
    fn persistence_overwrite_state() {
        let mut store = WebviewPersistenceStore::new();
        store.save_state(1, serde_json::json!("old"));
        store.save_state(1, serde_json::json!("new"));
        assert_eq!(store.load_state(1), Some(&serde_json::json!("new")));
        assert_eq!(store.handles().len(), 1);
    }

    #[test]
    fn persistence_json_roundtrip() {
        let mut store = WebviewPersistenceStore::new();
        store.save_state(5, serde_json::json!({"theme": "dark"}));
        store.save_state(10, serde_json::json!([1, 2, 3]));

        let json = store.to_json();
        let restored = WebviewPersistenceStore::from_json(&json).unwrap();
        assert_eq!(store, restored);
    }

    // ── WebviewThemeAdapter tests ──

    #[test]
    fn theme_adapter_light_defaults() {
        let adapter = WebviewThemeAdapter::from_kind(WebviewThemeKind::Light);
        assert_eq!(adapter.kind, WebviewThemeKind::Light);
        assert_eq!(adapter.background, "#ffffff");
        assert_eq!(adapter.foreground, "#1e1e1e");
    }

    #[test]
    fn theme_adapter_css_variables() {
        let adapter = WebviewThemeAdapter::from_kind(WebviewThemeKind::Dark);
        let css = adapter.to_css_variables();
        assert!(css.contains("--vscode-foreground: #cccccc"));
        assert!(css.contains("--vscode-background: #1e1e1e"));
        assert!(css.contains("--vscode-accent: #569cd6"));
        assert!(css.starts_with(":root {"));
    }

    #[test]
    fn theme_adapter_wrap_html() {
        let adapter = WebviewThemeAdapter::from_kind(WebviewThemeKind::HighContrast);
        let wrapped = adapter.wrap_html("<p>content</p>");
        assert!(wrapped.contains("<style>"));
        assert!(wrapped.contains("--vscode-foreground: #ffffff"));
        assert!(wrapped.ends_with("<p>content</p>"));
    }

    #[test]
    fn theme_adapter_serde_roundtrip() {
        let adapter = WebviewThemeAdapter::from_kind(WebviewThemeKind::Dark);
        let json = serde_json::to_string(&adapter).unwrap();
        let back: WebviewThemeAdapter = serde_json::from_str(&json).unwrap();
        assert_eq!(adapter, back);
    }

    #[test]
    fn theme_kind_serde() {
        let kind = WebviewThemeKind::HighContrast;
        let json = serde_json::to_string(&kind).unwrap();
        assert_eq!(json, "\"highContrast\"");
        let back: WebviewThemeKind = serde_json::from_str(&json).unwrap();
        assert_eq!(back, WebviewThemeKind::HighContrast);
    }

    // ── WebviewResourceLoader tests ──

    #[test]
    fn resource_loader_register_and_resolve() {
        let mut loader = WebviewResourceLoader::new();
        loader.register("styles", "https://cdn.example.com/app.css").unwrap();
        loader.register("script", "https://cdn.example.com/app.js").unwrap();
        assert_eq!(loader.len(), 2);
        assert_eq!(loader.resolve("styles"), Some("https://cdn.example.com/app.css"));
        assert_eq!(loader.resolve("missing"), None);
    }

    #[test]
    fn resource_loader_rejects_duplicate_alias() {
        let mut loader = WebviewResourceLoader::new();
        loader.register("a", "https://x.com/a.css").unwrap();
        assert!(loader.register("a", "https://x.com/b.css").is_err());
    }

    #[test]
    fn resource_loader_rejects_invalid_uri() {
        let mut loader = WebviewResourceLoader::new();
        let res = loader.register("bad", "ftp://evil.com/file");
        assert!(matches!(res, Err(WebviewError::InvalidResourceRoot(_))));
    }

    #[test]
    fn resource_loader_unregister() {
        let mut loader = WebviewResourceLoader::new();
        loader.register("x", "https://cdn.example.com/x.js").unwrap();
        assert!(loader.unregister("x"));
        assert!(!loader.unregister("x"));
        assert!(loader.is_empty());
    }

    #[test]
    fn resource_loader_html_tags() {
        let mut loader = WebviewResourceLoader::new();
        loader.register("style", "https://cdn.example.com/app.css").unwrap();
        loader.register("main", "https://cdn.example.com/main.js").unwrap();
        let tags = loader.to_html_tags();
        assert!(tags.contains("<link rel=\"stylesheet\" href=\"https://cdn.example.com/app.css\">"));
        assert!(tags.contains("<script src=\"https://cdn.example.com/main.js\"></script>"));
    }

    // ── CspBuilder tests ──

    #[test]
    fn csp_builder_default_none() {
        let csp = CspBuilder::new().default_none().build();
        assert_eq!(csp, "default-src 'none'");
    }

    #[test]
    fn csp_builder_complex_policy() {
        let csp = CspBuilder::new()
            .default_self()
            .script_nonce("abc123")
            .style_self_with_nonce(Some("xyz"))
            .img_https_data()
            .add("font-src", "https:")
            .build();
        assert!(csp.contains("default-src 'self'"));
        assert!(csp.contains("script-src 'nonce-abc123'"));
        assert!(csp.contains("style-src 'self' 'nonce-xyz'"));
        assert!(csp.contains("img-src https: data:"));
        assert!(csp.contains("font-src https:"));
    }

    #[test]
    fn csp_builder_deterministic_order() {
        let csp1 = CspBuilder::new()
            .add("script-src", "'self'")
            .add("default-src", "'none'")
            .build();
        let csp2 = CspBuilder::new()
            .add("default-src", "'none'")
            .add("script-src", "'self'")
            .build();
        assert_eq!(csp1, csp2);
        // default-src should come before script-src alphabetically
        let idx_default = csp1.find("default-src").unwrap();
        let idx_script = csp1.find("script-src").unwrap();
        assert!(idx_default < idx_script);
    }

    // ── PanelLayoutTracker tests ──

    #[test]
    fn panel_layout_add_and_query() {
        let mut tracker = PanelLayoutTracker::new();
        tracker.add_panel(1, "Preview", ViewColumn::One);
        tracker.add_panel(2, "Terminal", ViewColumn::Two);
        assert_eq!(tracker.panel_count(), 2);

        let panels = tracker.panels_in_column(ViewColumn::One);
        assert_eq!(panels.len(), 1);
        assert_eq!(panels[0].title, "Preview");
        assert_eq!(panels[0].column, ViewColumn::One);
    }

    #[test]
    fn panel_layout_set_active_deactivates_siblings() {
        let mut tracker = PanelLayoutTracker::new();
        tracker.add_panel(1, "A", ViewColumn::One);
        tracker.add_panel(2, "B", ViewColumn::One);
        tracker.add_panel(3, "C", ViewColumn::Two);

        assert!(tracker.set_active(1));
        assert!(tracker.get_panel(1).unwrap().active);
        assert!(!tracker.get_panel(2).unwrap().active);

        // Activating panel 2 in the same column deactivates panel 1
        assert!(tracker.set_active(2));
        assert!(!tracker.get_panel(1).unwrap().active);
        assert!(tracker.get_panel(2).unwrap().active);

        // Panel 3 in column Two is unaffected
        assert!(!tracker.get_panel(3).unwrap().active);
        assert_eq!(tracker.active_in_column(ViewColumn::Two), None);
    }

    #[test]
    fn panel_layout_move_and_visibility() {
        let mut tracker = PanelLayoutTracker::new();
        tracker.add_panel(1, "Panel", ViewColumn::One);

        assert!(tracker.move_panel(1, ViewColumn::Sidebar));
        assert_eq!(tracker.get_panel(1).unwrap().column, ViewColumn::Sidebar);
        assert!(tracker.panels_in_column(ViewColumn::One).is_empty());

        assert!(tracker.set_visible(1, false));
        assert!(tracker.visible_panels().is_empty());

        assert!(tracker.set_visible(1, true));
        assert_eq!(tracker.visible_panels().len(), 1);
    }

    #[test]
    fn panel_layout_remove() {
        let mut tracker = PanelLayoutTracker::new();
        tracker.add_panel(1, "X", ViewColumn::One);
        assert!(tracker.remove_panel(1));
        assert!(!tracker.remove_panel(1));
        assert_eq!(tracker.panel_count(), 0);
    }

    #[test]
    fn panel_layout_duplicate_add_ignored() {
        let mut tracker = PanelLayoutTracker::new();
        tracker.add_panel(1, "First", ViewColumn::One);
        tracker.add_panel(1, "Second", ViewColumn::Two);
        assert_eq!(tracker.panel_count(), 1);
        assert_eq!(tracker.get_panel(1).unwrap().title, "First");
    }

    #[test]
    fn view_column_serde_roundtrip() {
        let col = ViewColumn::Sidebar;
        let json = serde_json::to_string(&col).unwrap();
        assert_eq!(json, "\"sidebar\"");
        let back: ViewColumn = serde_json::from_str(&json).unwrap();
        assert_eq!(back, ViewColumn::Sidebar);
    }

    // ── Lifecycle Management tests ──

    #[test]
    fn lifecycle_valid_transitions() {
        let mut mgr = WebviewLifecycleManager::new();
        mgr.register(1).unwrap();
        assert_eq!(mgr.state_of(1), Some(LifecycleState::Created));

        // Created → Ready
        mgr.transition(1, LifecycleState::Ready).unwrap();
        assert_eq!(mgr.state_of(1), Some(LifecycleState::Ready));

        // Ready → Active
        mgr.transition(1, LifecycleState::Active).unwrap();
        assert_eq!(mgr.state_of(1), Some(LifecycleState::Active));

        // Active → Suspended
        mgr.transition(1, LifecycleState::Suspended).unwrap();
        assert_eq!(mgr.state_of(1), Some(LifecycleState::Suspended));

        // Suspended → Disposed
        mgr.transition(1, LifecycleState::Disposed).unwrap();
        assert_eq!(mgr.state_of(1), Some(LifecycleState::Disposed));
    }

    #[test]
    fn lifecycle_invalid_transition_rejected() {
        let mut mgr = WebviewLifecycleManager::new();
        mgr.register(1).unwrap();

        // Created → Active is not valid (must go through Ready first)
        let result = mgr.transition(1, LifecycleState::Active);
        assert!(matches!(result, Err(WebviewError::InvalidContent(_))));
        assert_eq!(mgr.state_of(1), Some(LifecycleState::Created));

        // Disposed is a terminal state
        mgr.transition(1, LifecycleState::Disposed).unwrap();
        let result = mgr.transition(1, LifecycleState::Ready);
        assert!(matches!(result, Err(WebviewError::InvalidContent(_))));
    }

    #[test]
    fn lifecycle_dispose_all() {
        let mut mgr = WebviewLifecycleManager::new();
        mgr.register(1).unwrap();
        mgr.register(2).unwrap();
        mgr.transition(1, LifecycleState::Ready).unwrap();
        mgr.transition(2, LifecycleState::Ready).unwrap();
        mgr.transition(2, LifecycleState::Active).unwrap();

        let count = mgr.dispose_all();
        assert_eq!(count, 2);
        assert_eq!(mgr.state_of(1), Some(LifecycleState::Disposed));
        assert_eq!(mgr.state_of(2), Some(LifecycleState::Disposed));

        // Calling again disposes none
        assert_eq!(mgr.dispose_all(), 0);
    }

    #[test]
    fn lifecycle_handles_in_state() {
        let mut mgr = WebviewLifecycleManager::new();
        mgr.register(10).unwrap();
        mgr.register(20).unwrap();
        mgr.register(30).unwrap();
        mgr.transition(10, LifecycleState::Ready).unwrap();
        mgr.transition(20, LifecycleState::Ready).unwrap();

        let mut created = mgr.handles_in_state(LifecycleState::Created);
        created.sort();
        assert_eq!(created, vec![30]);

        let mut ready = mgr.handles_in_state(LifecycleState::Ready);
        ready.sort();
        assert_eq!(ready, vec![10, 20]);
    }

    #[test]
    fn lifecycle_state_display() {
        assert_eq!(LifecycleState::Created.to_string(), "created");
        assert_eq!(LifecycleState::Disposed.to_string(), "disposed");
        assert_eq!(LifecycleState::Active.to_string(), "active");
    }

    #[test]
    fn lifecycle_state_serde_roundtrip() {
        let state = LifecycleState::Suspended;
        let json = serde_json::to_string(&state).unwrap();
        assert_eq!(json, "\"suspended\"");
        let back: LifecycleState = serde_json::from_str(&json).unwrap();
        assert_eq!(back, LifecycleState::Suspended);
    }

    // ── HTML Sanitizer tests ──

    #[test]
    fn sanitize_html_removes_script_tags() {
        let input = "<p>Hello</p><script>alert('xss')</script><p>World</p>";
        let output = sanitize_html(input);
        assert_eq!(output, "<p>Hello</p><p>World</p>");
        assert!(!html_has_dangerous_content(&output));
    }

    #[test]
    fn sanitize_html_removes_iframe_and_object() {
        let input = "<div><iframe src='evil.com'></iframe><object data='x'></object>safe</div>";
        let output = sanitize_html(input);
        assert!(!output.contains("<iframe"));
        assert!(!output.contains("<object"));
        assert!(output.contains("safe"));
    }

    #[test]
    fn sanitize_html_preserves_safe_content() {
        let input = "<h1>Title</h1><p>Paragraph with <b>bold</b> text.</p>";
        let output = sanitize_html(input);
        assert_eq!(output, input);
    }

    #[test]
    fn html_has_dangerous_content_detects_tags() {
        assert!(html_has_dangerous_content("<script>x</script>"));
        assert!(html_has_dangerous_content("<IFRAME src='x'>"));
        assert!(html_has_dangerous_content("<embed type='x'>"));
        assert!(!html_has_dangerous_content("<p>safe</p>"));
        assert!(!html_has_dangerous_content("plain text"));
    }

    // ── Message Protocol tests ──

    #[test]
    fn message_envelope_roundtrip() {
        let mut factory = MessageEnvelopeFactory::new(42);
        let req = factory.request("getData", serde_json::json!({"key": "value"}));
        assert_eq!(req.seq, 1);
        assert_eq!(req.request_seq, None);
        assert_eq!(req.handle, 42);
        assert_eq!(req.channel, "getData");

        let json = serde_json::to_string(&req).unwrap();
        let back: MessageEnvelope = serde_json::from_str(&json).unwrap();
        assert_eq!(req, back);
    }

    #[test]
    fn message_envelope_response_correlation() {
        let mut factory = MessageEnvelopeFactory::new(1);
        let req = factory.request("query", serde_json::json!(null));
        let resp = factory.response(req.seq, "query", serde_json::json!({"result": 42}));

        assert_eq!(resp.seq, 2);
        assert_eq!(resp.request_seq, Some(1));
        assert_eq!(resp.channel, "query");
    }

    #[test]
    fn message_envelope_factory_seq_increments() {
        let mut factory = MessageEnvelopeFactory::new(1);
        assert_eq!(factory.peek_next_seq(), 1);
        factory.request("a", serde_json::json!(null));
        assert_eq!(factory.peek_next_seq(), 2);
        factory.request("b", serde_json::json!(null));
        assert_eq!(factory.peek_next_seq(), 3);
    }

    // ── Bulk Operations & Search tests ──

    #[test]
    fn bulk_dispose_removes_multiple() {
        let mut bridge = WebviewBridge::new();
        bridge.create_webview(1);
        bridge.create_webview(2);
        bridge.create_webview(3);
        bridge.handle_message(&WebviewMessage::PostMessage {
            handle: 1,
            message: serde_json::json!("msg"),
        });

        let removed = bridge.bulk_dispose(&[1, 3]);
        assert_eq!(removed, 2);
        assert_eq!(bridge.webview_count(), 1);
        assert!(bridge.get_webview(2).is_some());
        // Messages for disposed handles are also cleaned up
        assert_eq!(bridge.total_pending_messages(), 0);
    }

    #[test]
    fn find_by_html_contains_matches() {
        let mut bridge = WebviewBridge::new();
        bridge.create_webview(1);
        bridge.create_webview(2);
        bridge.create_webview(3);
        bridge.set_html_checked(1, "<p>alpha beta</p>".into()).unwrap();
        bridge.set_html_checked(2, "<p>gamma</p>".into()).unwrap();
        bridge.set_html_checked(3, "<p>alpha delta</p>".into()).unwrap();

        let mut found = bridge.find_by_html_contains("alpha");
        found.sort();
        assert_eq!(found, vec![1, 3]);
        assert!(bridge.find_by_html_contains("missing").is_empty());
    }

    #[test]
    fn content_size_map_reports_lengths() {
        let mut bridge = WebviewBridge::new();
        bridge.create_webview(1);
        bridge.create_webview(2);
        bridge.set_html_checked(1, "<p>hi</p>".into()).unwrap();

        let map = bridge.content_size_map();
        assert_eq!(map[&1], 9); // "<p>hi</p>" is 9 chars
        assert_eq!(map[&2], 0); // empty html
    }

    #[test]
    fn clone_options_copies_between_webviews() {
        let mut bridge = WebviewBridge::new();
        let opts = WebviewOptionsBuilder::new()
            .enable_scripts(true)
            .enable_forms(true)
            .add_resource_root("/ext/media")
            .build()
            .unwrap();
        bridge.create_webview_checked(1, opts).unwrap();
        bridge.create_webview(2);

        bridge.clone_options(1, 2).unwrap();
        let w2 = bridge.get_webview(2).unwrap();
        assert!(w2.options.enable_scripts);
        assert!(w2.options.enable_forms);
        assert_eq!(w2.options.local_resource_roots, vec!["/ext/media"]);
    }

    #[test]
    fn clone_options_errors_on_missing_handle() {
        let mut bridge = WebviewBridge::new();
        bridge.create_webview(1);
        assert_eq!(bridge.clone_options(99, 1), Err(WebviewError::NotFound(99)));
        assert_eq!(bridge.clone_options(1, 99), Err(WebviewError::NotFound(99)));
    }

    // -- Content Sanitizer Tests --
    #[test]
    fn test_sanitizer_default_tags() {
        let san = WebviewContentSanitizer::new();
        assert!(san.is_tag_allowed("div"));
        assert!(!san.is_tag_allowed("script"));
    }
    #[test]
    fn test_sanitizer_custom_tag() {
        let san = WebviewContentSanitizer::new().allow_tag("my-elem");
        assert!(san.is_tag_allowed("my-elem"));
    }
    #[test]
    fn test_sanitizer_strip_scripts() {
        let san = WebviewContentSanitizer::new();
        let r = san.sanitize("<p>Hi</p><script>alert(1)</script><p>Safe</p>");
        assert!(!r.output.to_lowercase().contains("<script"));
        assert_eq!(r.elements_removed, 1);
    }
    #[test]
    fn test_sanitizer_strip_events() {
        let san = WebviewContentSanitizer::new();
        let r = san.sanitize(r#"<div onclick="bad()">x</div>"#);
        assert!(!r.output.contains("onclick"));
    }
    #[test]
    fn test_sanitizer_truncation() {
        let san = WebviewContentSanitizer::new().with_max_length(5);
        let r = san.sanitize("Hello World");
        assert!(r.was_truncated);
    }
    #[test]
    fn test_sanitizer_safe_pass() {
        let san = WebviewContentSanitizer::new();
        let r = san.sanitize("<div>Hello</div>");
        assert_eq!(r.elements_removed, 0);
    }
    #[test]
    fn test_sanitizer_attr_check() {
        let san = WebviewContentSanitizer::new();
        assert!(!san.is_attr_allowed("onclick"));
        assert!(san.is_attr_allowed("class"));
    }
    // -- Resource Mapper Tests --
    #[test]
    fn test_mapper_resolve() {
        let mut m = WebviewResourceMapper::new("vscode-resource");
        m.add_mapping("res://s.css", "/ws/s.css");
        assert_eq!(m.resolve("res://s.css"), Some("/ws/s.css"));
    }
    #[test]
    fn test_mapper_uri() {
        let m = WebviewResourceMapper::new("vscode-resource");
        assert!(m.to_webview_uri("/ws/a.js").starts_with("vscode-resource://"));
    }
    #[test]
    fn test_mapper_mime() {
        assert_eq!(WebviewResourceMapper::guess_mime("s.css"), "text/css");
        assert_eq!(WebviewResourceMapper::guess_mime("a.js"), "application/javascript");
    }
    #[test]
    fn test_mapper_clear() {
        let mut m = WebviewResourceMapper::new("r");
        m.add_mapping("a", "b");
        m.clear_mappings();
        assert_eq!(m.mapping_count(), 0);
    }
    #[test]
    fn test_mapper_has() {
        let mut m = WebviewResourceMapper::new("r");
        m.add_mapping("a", "b");
        assert!(m.has_mapping("a"));
        assert!(!m.has_mapping("c"));
    }


    // -- CSP Directive Builder Tests --

    #[test]
    fn test_csp_build_empty() {
        let b = WebviewCspDirectiveBuilder::new();
        assert_eq!(b.build(), "");
    }

    #[test]
    fn test_csp_single_directive() {
        let mut b = WebviewCspDirectiveBuilder::new();
        b.default_src(&["'self'"]);
        assert_eq!(b.build(), "default-src 'self'");
    }

    #[test]
    fn test_csp_multiple_directives() {
        let mut b = WebviewCspDirectiveBuilder::new();
        b.default_src(&["'none'"]).script_src(&["'self'"]);
        let csp = b.build();
        assert!(csp.contains("default-src 'none'"));
        assert!(csp.contains("script-src 'self'"));
    }

    #[test]
    fn test_csp_restrictive() {
        let b = WebviewCspDirectiveBuilder::restrictive();
        let csp = b.build();
        assert!(csp.contains("script-src 'none'"));
        assert!(csp.contains("default-src 'none'"));
    }

    #[test]
    fn test_csp_permissive() {
        let b = WebviewCspDirectiveBuilder::permissive();
        let csp = b.build();
        assert!(csp.contains("'unsafe-eval'"));
    }

    #[test]
    fn test_csp_has_directive() {
        let mut b = WebviewCspDirectiveBuilder::new();
        b.img_src(&["data:"]);
        assert!(b.has_directive("img-src"));
        assert!(!b.has_directive("script-src"));
    }

    #[test]
    fn test_csp_directive_count() {
        let mut b = WebviewCspDirectiveBuilder::new();
        b.default_src(&["'self'"]).style_src(&["'unsafe-inline'"]);
        assert_eq!(b.directive_count(), 2);
    }


    // -- ext_webview additional tests -------------------------------------------

    #[test]
    fn x_ext_webview_activation_parse_language() {
        let ak = XExtWebviewActivationKind::parse("onLanguage:rust").unwrap();
        assert_eq!(ak, XExtWebviewActivationKind::Language("rust".into()));
        assert!(ak.is_language());
    }

    #[test]
    fn x_ext_webview_activation_parse_command() {
        let ak = XExtWebviewActivationKind::parse("onCommand:editor.action.format").unwrap();
        assert_eq!(ak, XExtWebviewActivationKind::Command("editor.action.format".into()));
        assert!(!ak.is_language());
    }

    #[test]
    fn x_ext_webview_activation_parse_star() {
        assert_eq!(XExtWebviewActivationKind::parse("*"), Some(XExtWebviewActivationKind::Star));
    }

    #[test]
    fn x_ext_webview_activation_parse_unknown() {
        assert!(XExtWebviewActivationKind::parse("badKind:thing").is_none());
    }

    #[test]
    fn x_ext_webview_activation_parse_workspace() {
        let ak = XExtWebviewActivationKind::parse("workspaceContains:**/Cargo.toml").unwrap();
        assert_eq!(ak, XExtWebviewActivationKind::WorkspaceContains("**/" .to_owned() + "Cargo.toml"));
    }

    #[test]
    fn x_ext_webview_rpc_envelope_basic() {
        let env = XExtWebviewRpcEnvelope::new(1, "textDocument/didOpen", "{}" );
        assert_eq!(env.seq, 1);
        assert!(!env.is_response());
    }

    #[test]
    fn x_ext_webview_rpc_envelope_response() {
        let env = XExtWebviewRpcEnvelope::new(2, "$/cancelRequest", "");
        assert!(env.is_response());
    }

    #[test]
    fn x_ext_webview_rpc_payload_checksum() {
        let env = XExtWebviewRpcEnvelope::new(1, "m", "AB");
        assert_eq!(env.payload_checksum(), 65 + 66);
    }

    #[test]
    fn x_ext_webview_collect_sequences_works() {
        let envs = vec![
            XExtWebviewRpcEnvelope::new(10, "a", ""),
            XExtWebviewRpcEnvelope::new(20, "b", ""),
        ];
        assert_eq!(x_ext_webview_collect_sequences(&envs), vec![10, 20]);
    }

    #[test]
    fn x_ext_webview_filter_by_method_works() {
        let envs = vec![
            XExtWebviewRpcEnvelope::new(1, "textDocument/open", ""),
            XExtWebviewRpcEnvelope::new(2, "workspace/config", ""),
            XExtWebviewRpcEnvelope::new(3, "textDocument/close", ""),
        ];
        let filtered = x_ext_webview_filter_by_method(&envs, "textDocument/");
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn x_ext_webview_dedup_by_seq_works() {
        let envs = vec![
            XExtWebviewRpcEnvelope::new(1, "a", "first"),
            XExtWebviewRpcEnvelope::new(1, "a", "second"),
            XExtWebviewRpcEnvelope::new(2, "b", "third"),
        ];
        let deduped = x_ext_webview_dedup_by_seq(envs);
        assert_eq!(deduped.len(), 2);
        assert_eq!(deduped[0].payload, "first");
    }

    #[test]
    fn x_ext_webview_negotiate_capabilities_basic() {
        let result = x_ext_webview_negotiate_capabilities(
            &["hover", "completion", "rename"],
            &["hover", "rename", "format"],
        );
        assert_eq!(result, vec!["hover", "rename"]);
    }

    #[test]
    fn x_ext_webview_api_version_satisfies() {
        let v1 = XExtWebviewApiVersion::new(1, 80, 0);
        let min = XExtWebviewApiVersion::new(1, 70, 0);
        assert!(v1.satisfies(&min));
        assert!(!min.satisfies(&v1));
    }

    #[test]
    fn x_ext_webview_api_version_display() {
        let v = XExtWebviewApiVersion::new(2, 3, 4);
        assert_eq!(v.to_string(), "2.3.4");
    }

    #[test]
    fn x_ext_webview_api_version_ord() {
        let v1 = XExtWebviewApiVersion::new(1, 0, 0);
        let v2 = XExtWebviewApiVersion::new(1, 1, 0);
        assert!(v1 < v2);
    }


    #[test]
    fn ext_webview_config_new() {
        let cfg = ExtWebviewConfig::new();
        assert!(cfg.is_enabled());
        assert_eq!(cfg.version(), 1);
        assert_eq!(cfg.option_count(), 0);
    }

    #[test]
    fn ext_webview_config_set_get() {
        let mut cfg = ExtWebviewConfig::new();
        cfg.set_option("key", "value");
        assert_eq!(cfg.get_option("key"), Some("value"));
        assert!(cfg.has_option("key"));
    }

    #[test]
    fn ext_webview_config_remove() {
        let mut cfg = ExtWebviewConfig::new();
        cfg.set_option("a", "1");
        assert_eq!(cfg.remove_option("a"), Some("1".into()));
        assert!(!cfg.has_option("a"));
    }

    #[test]
    fn ext_webview_config_keys_sorted() {
        let mut cfg = ExtWebviewConfig::new();
        cfg.set_option("z", "1");
        cfg.set_option("a", "2");
        assert_eq!(cfg.option_keys(), vec!["a", "z"]);
    }

    #[test]
    fn ext_webview_config_bump_version() {
        let mut cfg = ExtWebviewConfig::new();
        cfg.bump_version();
        cfg.bump_version();
        assert_eq!(cfg.version(), 3);
    }

    #[test]
    fn ext_webview_config_clear() {
        let mut cfg = ExtWebviewConfig::new();
        cfg.set_option("x", "y");
        cfg.bump_version();
        cfg.clear();
        assert_eq!(cfg.option_count(), 0);
        assert_eq!(cfg.version(), 1);
    }

    #[test]
    fn ext_webview_config_merge() {
        let mut cfg1 = ExtWebviewConfig::new();
        cfg1.set_option("a", "1");
        let mut cfg2 = ExtWebviewConfig::new();
        cfg2.set_option("b", "2");
        cfg1.merge(&cfg2);
        assert_eq!(cfg1.option_count(), 2);
    }

    #[test]
    fn ext_webview_config_disable() {
        let mut cfg = ExtWebviewConfig::new();
        cfg.set_enabled(false);
        assert!(!cfg.is_enabled());
    }

    #[test]
    fn ext_webview_rate_tracker_empty() {
        let rt = ExtWebviewRateTracker::new(1000);
        assert_eq!(rt.count(), 0);
        assert_eq!(rt.rate_per_second(), 0.0);
    }

    #[test]
    fn ext_webview_rate_tracker_record() {
        let mut rt = ExtWebviewRateTracker::new(1000);
        rt.record(100);
        rt.record(200);
        rt.record(300);
        assert_eq!(rt.count(), 3);
    }

    #[test]
    fn ext_webview_rate_tracker_prune() {
        let mut rt = ExtWebviewRateTracker::new(100);
        rt.record(10);
        rt.record(200);
        assert_eq!(rt.count(), 1);
    }

    #[test]
    fn ext_webview_validator_valid() {
        let v = ExtWebviewValidator::new();
        assert!(v.is_valid());
        assert_eq!(v.error_count(), 0);
    }

    #[test]
    fn ext_webview_validator_errors() {
        let mut v = ExtWebviewValidator::new();
        v.add_error("bad input");
        v.add_warning("slow");
        assert!(!v.is_valid());
        assert_eq!(v.error_count(), 1);
        assert_eq!(v.warning_count(), 1);
        assert_eq!(v.first_error(), Some("bad input"));
    }

    #[test]
    fn ext_webview_validator_clear() {
        let mut v = ExtWebviewValidator::new();
        v.add_error("err");
        v.clear();
        assert!(v.is_valid());
    }

    #[test]
    fn ext_webview_validator_merge() {
        let mut v1 = ExtWebviewValidator::new();
        v1.add_error("e1");
        let mut v2 = ExtWebviewValidator::new();
        v2.add_error("e2");
        v2.add_warning("w1");
        v1.merge(&v2);
        assert_eq!(v1.error_count(), 2);
        assert_eq!(v1.warning_count(), 1);
    }

    #[test]
    fn ext_webview_rate_tracker_clear() {
        let mut rt = ExtWebviewRateTracker::new(1000);
        rt.record(100);
        rt.clear();
        assert_eq!(rt.count(), 0);
    }


    #[test]
    fn zq_store_new_empty() {
        let store = super::ZqStore::new(8);
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);
        assert_eq!(store.remaining(), 8);
    }

    #[test]
    fn zq_store_insert_and_get() {
        let mut store = super::ZqStore::new(8);
        assert!(store.insert("color", "red"));
        assert_eq!(store.get("color"), Some("red"));
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_eviction() {
        let mut store = super::ZqStore::new(2);
        store.insert("a", "1");
        store.insert("b", "2");
        store.insert("c", "3");
        assert_eq!(store.len(), 2);
        assert!(store.get("a").is_none());
        assert_eq!(store.get("b"), Some("2"));
        assert_eq!(store.get("c"), Some("3"));
    }

    #[test]
    fn zq_store_remove() {
        let mut store = super::ZqStore::new(8);
        store.insert("x", "10");
        store.insert("x", "20");
        store.insert("y", "30");
        let removed = store.remove("x");
        assert_eq!(removed, 2);
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_keys_values() {
        let mut store = super::ZqStore::new(8);
        store.insert("k1", "v1");
        store.insert("k2", "v2");
        assert_eq!(store.keys(), vec!["k1", "k2"]);
        assert_eq!(store.values(), vec!["v1", "v2"]);
    }

    #[test]
    fn zq_store_drain_prefix() {
        let mut store = super::ZqStore::new(8);
        store.insert("pre_a", "1");
        store.insert("pre_b", "2");
        store.insert("other", "3");
        let drained = store.drain_prefix("pre_");
        assert_eq!(drained.len(), 2);
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn zq_store_retain() {
        let mut store = super::ZqStore::new(8);
        store.insert("a", "keep");
        store.insert("b", "drop");
        store.insert("c", "keep");
        store.retain(|_k, v| v == "keep");
        assert_eq!(store.len(), 2);
    }

    #[test]
    fn zq_store_clear() {
        let mut store = super::ZqStore::new(8);
        store.insert("a", "1");
        store.insert("b", "2");
        store.clear();
        assert!(store.is_empty());
        assert_eq!(store.remaining(), 8);
    }

    #[test]
    fn zq_store_merge() {
        let mut s1 = super::ZqStore::new(3);
        s1.insert("a", "1");
        let mut s2 = super::ZqStore::new(8);
        s2.insert("b", "2");
        s2.insert("c", "3");
        s2.insert("d", "4");
        s1.merge(&s2);
        assert_eq!(s1.len(), 3);
        assert!(s1.get("d").is_none());
    }

    #[test]
    fn zq_format_bytes_units() {
        assert_eq!(super::zq_format_bytes(500), "500 B");
        assert_eq!(super::zq_format_bytes(2048), "2.00 KB");
        assert_eq!(super::zq_format_bytes(5 * 1024 * 1024), "5.00 MB");
        assert_eq!(super::zq_format_bytes(3 * 1024 * 1024 * 1024), "3.00 GB");
    }

    #[test]
    fn zq_truncate_short() {
        assert_eq!(super::zq_truncate("hi", 10), "hi");
    }

    #[test]
    fn zq_truncate_long() {
        let long = "abcdefghijklmnop";
        let t = super::zq_truncate(long, 10);
        assert!(t.ends_with("..."));
        assert!(t.len() <= 10);
    }


    // xa_ extended tests for ext_webview
    #[test]
    fn xa_ext_webview_ring_new() {
        let rb = super::XaExtWebviewRingBuf::new(4);
        assert_eq!(rb.len(), 0);
        assert!(rb.is_empty());
    }

    #[test]
    fn xa_ext_webview_ring_push_len() {
        let mut rb = super::XaExtWebviewRingBuf::new(3);
        rb.push(1.0);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xa_ext_webview_ring_wrap() {
        let mut rb = super::XaExtWebviewRingBuf::new(2);
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.len(), 2);
        let v = rb.drain_to_vec();
        assert_eq!(v, vec![2.0, 3.0]);
    }

    #[test]
    fn xa_ext_webview_ring_mean_empty() {
        let rb = super::XaExtWebviewRingBuf::new(5);
        assert!(rb.mean().is_none());
    }

    #[test]
    fn xa_ext_webview_ring_mean_values() {
        let mut rb = super::XaExtWebviewRingBuf::new(4);
        rb.push(2.0);
        rb.push(4.0);
        let m = rb.mean().unwrap();
        assert!((m - 3.0).abs() < 1e-9);
    }

    #[test]
    fn xa_ext_webview_ring_min_max() {
        let mut rb = super::XaExtWebviewRingBuf::new(5);
        rb.push(7.0);
        rb.push(2.0);
        rb.push(9.0);
        assert_eq!(rb.min_val().unwrap(), 2.0);
        assert_eq!(rb.max_val().unwrap(), 9.0);
    }

    #[test]
    fn xa_ext_webview_ring_iter() {
        let mut rb = super::XaExtWebviewRingBuf::new(3);
        rb.push(10.0);
        rb.push(20.0);
        let collected: Vec<f64> = rb.iter().collect();
        assert_eq!(collected, vec![10.0, 20.0]);
    }

    #[test]
    fn xa_ext_webview_counter_new() {
        let c = super::XaExtWebviewCounter::new();
        assert_eq!(c.get("x"), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_ext_webview_counter_inc() {
        let mut c = super::XaExtWebviewCounter::new();
        c.inc("a");
        c.inc("a");
        c.inc("b");
        assert_eq!(c.get("a"), 2);
        assert_eq!(c.get("b"), 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn xa_ext_webview_counter_inc_by() {
        let mut c = super::XaExtWebviewCounter::new();
        c.inc_by("k", 10);
        c.inc_by("k", 5);
        assert_eq!(c.get("k"), 15);
    }

    #[test]
    fn xa_ext_webview_counter_reset() {
        let mut c = super::XaExtWebviewCounter::new();
        c.inc("a");
        c.inc("b");
        c.reset();
        assert_eq!(c.get("a"), 0);
        assert_eq!(c.get("b"), 0);
        assert_eq!(c.num_keys(), 2);
    }

    #[test]
    fn xa_ext_webview_counter_clear() {
        let mut c = super::XaExtWebviewCounter::new();
        c.inc("a");
        c.clear();
        assert_eq!(c.num_keys(), 0);
        assert_eq!(c.total(), 0);
    }

    #[test]
    fn xa_ext_webview_counter_default() {
        let c = super::XaExtWebviewCounter::default();
        assert_eq!(c.total(), 0);
        assert_eq!(c.num_keys(), 0);
    }


    // ---- xc_ pool / scheduler tests – block 77 ----

    #[test]
    fn xc_77_pool_new_empty() {
        let pool: super::Xc77Pool<i32> = super::Xc77Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_77_pool_release_acquire() {
        let mut pool = super::Xc77Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_77_pool_acquire_empty() {
        let mut pool: super::Xc77Pool<i32> = super::Xc77Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_77_pool_full() {
        let mut pool = super::Xc77Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_77_pool_drain() {
        let mut pool = super::Xc77Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_77_pool_stats() {
        let mut pool = super::Xc77Pool::new(8);
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
    fn xc_77_pool_clear() {
        let mut pool = super::Xc77Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_77_pool_shrink() {
        let mut pool = super::Xc77Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_77_pool_default() {
        let pool: super::Xc77Pool<String> = super::Xc77Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_77_pool_extend() {
        let mut pool = super::Xc77Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_77_pool_retain() {
        let mut pool = super::Xc77Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_77_scheduler_round_robin() {
        let mut sched = super::Xc77Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_77_scheduler_empty() {
        let mut sched = super::Xc77Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_77_scheduler_reset() {
        let mut sched = super::Xc77Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_77_scheduler_add_remove() {
        let mut sched = super::Xc77Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_77_scheduler_targets() {
        let sched = super::Xc77Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_77_hash_empty() {
        assert_eq!(super::xc_77_hash(b""), 5381);
    }

    #[test]
    fn xc_77_hash_data() {
        let h = super::xc_77_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_77_hash(b"hello"), h);
    }

    #[test]
    fn xc_77_reverse_str() {
        assert_eq!(super::xc_77_reverse("abc"), "cba");
        assert_eq!(super::xc_77_reverse(""), "");
    }


    #[test]
    fn xe_18_pipeline_empty() {
        let p = super::Xe18Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_18_pipeline_parse_stage() {
        let p = super::Xe18Pipeline::new()
            .add_parse(super::xe_18_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_18_pipeline_transform_double() {
        let p = super::Xe18Pipeline::new()
            .add_transform(super::xe_18_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_18_pipeline_validate_reverse() {
        let p = super::Xe18Pipeline::new()
            .add_validate(super::xe_18_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_18_pipeline_emit_filter() {
        let p = super::Xe18Pipeline::new()
            .add_emit(super::xe_18_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_18_pipeline_multi_stage() {
        let p = super::Xe18Pipeline::new()
            .add_parse(super::xe_18_pipeline_identity)
            .add_transform(super::xe_18_pipeline_double)
            .add_validate(super::xe_18_pipeline_reverse)
            .add_emit(super::xe_18_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_18_pipeline_error_propagation() {
        let p = super::Xe18Pipeline::new()
            .add_parse(super::xe_18_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe18Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_18_pipeline_compose() {
        let p1 = super::Xe18Pipeline::new()
            .add_parse(super::xe_18_pipeline_identity);
        let p2 = super::Xe18Pipeline::new()
            .add_transform(super::xe_18_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_18_pipeline_error_display() {
        let e = super::Xe18PipelineError {
            stage: super::Xe18Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_18_cache_put_get() {
        let mut c = super::Xe18Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_18_cache_miss() {
        let mut c: super::Xe18Cache<&str, i32> = super::Xe18Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_18_cache_ttl_expiry() {
        let mut c = super::Xe18Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_18_cache_evict() {
        let mut c = super::Xe18Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_18_cache_capacity() {
        let mut c = super::Xe18Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_18_cache_stats() {
        let mut c = super::Xe18Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_18_cache_clear() {
        let mut c = super::Xe18Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // -- xf_ trie + bloom tests for instance #88 --

    #[test]
    fn xf88_trie_insert_search() {
        let mut t = Xf88Trie::xf_new();
        t.xf_insert("apple");
        t.xf_insert("app");
        assert!(t.xf_search("apple"));
        assert!(t.xf_search("app"));
        assert!(!t.xf_search("ap"));
    }

    #[test]
    fn xf88_trie_starts_with() {
        let mut t = Xf88Trie::xf_new();
        t.xf_insert("banana");
        assert!(t.xf_starts_with("ban"));
        assert!(!t.xf_starts_with("can"));
    }

    #[test]
    fn xf88_trie_remove() {
        let mut t = Xf88Trie::xf_new();
        t.xf_insert("hello");
        assert!(t.xf_remove("hello"));
        assert!(!t.xf_search("hello"));
        assert!(!t.xf_remove("hello"));
    }

    #[test]
    fn xf88_trie_word_count() {
        let mut t = Xf88Trie::xf_new();
        assert_eq!(t.xf_word_count(), 0);
        t.xf_insert("a");
        t.xf_insert("b");
        t.xf_insert("a");
        assert_eq!(t.xf_word_count(), 2);
    }

    #[test]
    fn xf88_trie_longest_prefix() {
        let mut t = Xf88Trie::xf_new();
        t.xf_insert("ab");
        t.xf_insert("abc");
        t.xf_insert("abcde");
        assert_eq!(t.xf_longest_prefix("abcdef"), Some("abcde".to_string()));
        assert_eq!(t.xf_longest_prefix("x"), None);
    }

    #[test]
    fn xf88_trie_all_words() {
        let mut t = Xf88Trie::xf_new();
        t.xf_insert("cat");
        t.xf_insert("car");
        t.xf_insert("card");
        let mut words = t.xf_all_words();
        words.sort();
        assert_eq!(words, vec!["car", "card", "cat"]);
    }

    #[test]
    fn xf88_trie_autocomplete() {
        let mut t = Xf88Trie::xf_new();
        t.xf_insert("dog");
        t.xf_insert("dot");
        t.xf_insert("dove");
        let mut results = t.xf_autocomplete("do");
        results.sort();
        assert_eq!(results, vec!["dog", "dot", "dove"]);
    }

    #[test]
    fn xf88_trie_empty_search() {
        let t = Xf88Trie::xf_new();
        assert!(!t.xf_search("anything"));
        assert_eq!(t.xf_all_words().len(), 0);
    }

    #[test]
    fn xf88_bloom_add_contains() {
        let mut bf = Xf88BloomFilter::xf_new(1024, 3);
        bf.xf_add("hello");
        bf.xf_add("world");
        assert!(bf.xf_might_contain("hello"));
        assert!(bf.xf_might_contain("world"));
    }

    #[test]
    fn xf88_bloom_probably_absent() {
        let bf = Xf88BloomFilter::xf_new(1024, 3);
        assert!(!bf.xf_might_contain("never_added"));
    }

    #[test]
    fn xf88_bloom_false_positive_rate() {
        let mut bf = Xf88BloomFilter::xf_new(1024, 3);
        let rate_empty = bf.xf_false_positive_rate();
        assert!((rate_empty - 0.0).abs() < f64::EPSILON);
        bf.xf_add("item");
        let rate = bf.xf_false_positive_rate();
        assert!(rate < 1.0);
    }

    #[test]
    fn xf88_bloom_clear() {
        let mut bf = Xf88BloomFilter::xf_new(512, 2);
        bf.xf_add("data");
        bf.xf_clear();
        assert!(!bf.xf_might_contain("data"));
    }

    #[test]
    fn xf88_bloom_union() {
        let mut a = Xf88BloomFilter::xf_new(512, 2);
        let mut b = Xf88BloomFilter::xf_new(512, 2);
        a.xf_add("alpha");
        b.xf_add("beta");
        let u = a.xf_union(&b).unwrap();
        assert!(u.xf_might_contain("alpha"));
        assert!(u.xf_might_contain("beta"));
    }

    #[test]
    fn xf88_bloom_intersection_estimate() {
        let mut a = Xf88BloomFilter::xf_new(512, 2);
        let mut b = Xf88BloomFilter::xf_new(512, 2);
        a.xf_add("shared");
        b.xf_add("shared");
        let est = a.xf_intersection_estimate(&b);
        assert!(est > 0.0);
    }

    #[test]
    fn xf88_bloom_union_size_mismatch() {
        let a = Xf88BloomFilter::xf_new(256, 2);
        let b = Xf88BloomFilter::xf_new(512, 2);
        assert!(a.xf_union(&b).is_none());
    }


    #[test]
    fn xh76_skip_insert_contains() {
        let mut sl = super::Xh76SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh76_skip_remove() {
        let mut sl = super::Xh76SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh76_skip_len() {
        let mut sl = super::Xh76SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh76_skip_range_query() {
        let mut sl = super::Xh76SkipList::xh_new(4);
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
    fn xh76_skip_floor_ceiling() {
        let mut sl = super::Xh76SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh76_skip_rank() {
        let mut sl = super::Xh76SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh76_skip_empty() {
        let sl = super::Xh76SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh76_skip_duplicates() {
        let mut sl = super::Xh76SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh76_bitset_set_test() {
        let mut bs = super::Xh76BitSet::xh_new(256);
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
    fn xh76_bitset_clear_count() {
        let mut bs = super::Xh76BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh76_bitset_and_or_xor() {
        let mut a = super::Xh76BitSet::xh_new(128);
        let mut b = super::Xh76BitSet::xh_new(128);
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
    fn xh76_bitset_iter_ones() {
        let mut bs = super::Xh76BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh76_bitset_first_last() {
        let mut bs = super::Xh76BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh76_bitset_empty() {
        let bs = super::Xh76BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi76_deque_push_pop_back() {
        let mut dq = super::Xi76Deque::xi_new(4);
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
    fn xi76_deque_push_pop_front() {
        let mut dq = super::Xi76Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi76_deque_mixed_ops() {
        let mut dq = super::Xi76Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi76_deque_get_and_split() {
        let mut dq = super::Xi76Deque::xi_new(8);
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
    fn xi76_deque_rotate_left() {
        let mut dq = super::Xi76Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi76_deque_rotate_right() {
        let mut dq = super::Xi76Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi76_deque_grow() {
        let mut dq = super::Xi76Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi76_deque_empty() {
        let dq = super::Xi76Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi76_interval_tree_insert_query() {
        let mut tree = super::Xi76IntervalTree::xi_new();
        tree.xi_insert(super::Xi76Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi76Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi76Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi76_interval_tree_overlap() {
        let mut tree = super::Xi76IntervalTree::xi_new();
        tree.xi_insert(super::Xi76Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi76Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi76Interval::xi_new(12, 20));
        let q = super::Xi76Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi76_interval_tree_remove() {
        let mut tree = super::Xi76IntervalTree::xi_new();
        tree.xi_insert(super::Xi76Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi76Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi76_interval_tree_gaps() {
        let mut tree = super::Xi76IntervalTree::xi_new();
        tree.xi_insert(super::Xi76Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi76Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi76Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi76Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi76Interval::xi_new(8, 10));
    }

    #[test]
    fn xi76_interval_tree_merge() {
        let mut tree = super::Xi76IntervalTree::xi_new();
        tree.xi_insert(super::Xi76Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi76Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi76Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi76Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi76Interval::xi_new(10, 15));
    }

    #[test]
    fn xi76_interval_tree_all() {
        let mut tree = super::Xi76IntervalTree::xi_new();
        tree.xi_insert(super::Xi76Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi76Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi76_interval_tree_empty() {
        let tree = super::Xi76IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi76_interval_tree_contains_point() {
        let iv = super::Xi76Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }

}
