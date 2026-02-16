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
}
