//! Main-thread RPC message handlers for vscode.* API stubs.
//!
//! When an extension running in the JS shim calls a `vscode.*` API, the shim
//! sends an RPC request such as `mainThread/showMessage` to the Rust side.
//! [`MainThreadHandlers`] dispatches these requests to stub handlers that
//! return valid JSON responses so extensions do not crash.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use serde_json::{json, Value};
use tracing::{debug, info};

type HandlerFn = Box<dyn Fn(Value) -> Value + Send + Sync>;

/// Kind of language feature provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProviderKind {
    Completion,
    Hover,
    Definition,
    CodeActions,
    Formatting,
    RangeFormatting,
    References,
    Rename,
    SignatureHelp,
    DocumentHighlight,
    DocumentLink,
    DocumentColor,
    FoldingRange,
    CodeLens,
    SemanticTokens,
    OnTypeFormatting,
    SelectionRange,
    DocumentSymbol,
    WorkspaceSymbol,
    TypeDefinition,
    Implementation,
    Declaration,
    InlayHint,
    LinkedEditing,
    CallHierarchy,
    TypeHierarchy,
}

/// A registered language feature provider from an extension.
#[derive(Debug, Clone)]
pub struct RegisteredProvider {
    pub handle: u64,
    pub kind: ProviderKind,
    pub extension_id: String,
    pub document_selector: Value,
}

/// Tracks all registered language feature providers from extensions.
#[derive(Debug, Clone, Default)]
pub struct ProviderRegistry {
    providers: Vec<RegisteredProvider>,
    next_handle: u64,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
            next_handle: 1,
        }
    }

    /// Register a provider and return its handle.
    pub fn register(&mut self, kind: ProviderKind, extension_id: &str, selector: Value) -> u64 {
        let handle = self.next_handle;
        self.next_handle += 1;
        self.providers.push(RegisteredProvider {
            handle,
            kind,
            extension_id: extension_id.to_string(),
            document_selector: selector,
        });
        handle
    }

    /// Unregister a provider by handle.
    pub fn unregister(&mut self, handle: u64) -> bool {
        let len = self.providers.len();
        self.providers.retain(|p| p.handle != handle);
        self.providers.len() < len
    }

    /// Get all providers of a specific kind.
    pub fn providers_for(&self, kind: ProviderKind) -> Vec<&RegisteredProvider> {
        self.providers.iter().filter(|p| p.kind == kind).collect()
    }

    /// Get all registered providers.
    pub fn all_providers(&self) -> &[RegisteredProvider] {
        &self.providers
    }

    /// Check if any provider of a kind is registered.
    pub fn has_provider(&self, kind: ProviderKind) -> bool {
        self.providers.iter().any(|p| p.kind == kind)
    }

    /// Count of all registered providers.
    pub fn count(&self) -> usize {
        self.providers.len()
    }

    /// Get a provider by handle.
    pub fn get(&self, handle: u64) -> Option<&RegisteredProvider> {
        self.providers.iter().find(|p| p.handle == handle)
    }
}

/// Pending document events to send to the extension host.
#[derive(Debug, Clone)]
pub enum DocumentEvent {
    /// A document was opened.
    DidOpen {
        uri: String,
        language_id: String,
        version: u64,
        content: String,
    },
    /// A document's content changed.
    DidChange {
        uri: String,
        version: u64,
        changes: Vec<DocumentChange>,
    },
    /// A document was saved.
    DidSave {
        uri: String,
    },
    /// A document was closed.
    DidClose {
        uri: String,
    },
}

/// A single text change within a document.
#[derive(Debug, Clone)]
pub struct DocumentChange {
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
    pub text: String,
}

impl DocumentEvent {
    /// Serialize this event to a JSON-RPC notification payload.
    pub fn to_rpc_notification(&self) -> (String, Value) {
        match self {
            DocumentEvent::DidOpen {
                uri,
                language_id,
                version,
                content,
            } => (
                "ext/didOpenTextDocument".to_string(),
                json!({
                    "uri": uri,
                    "languageId": language_id,
                    "version": version,
                    "text": content,
                }),
            ),
            DocumentEvent::DidChange {
                uri,
                version,
                changes,
            } => {
                let change_list: Vec<Value> = changes
                    .iter()
                    .map(|c| {
                        json!({
                            "range": {
                                "start": { "line": c.start_line, "character": c.start_col },
                                "end": { "line": c.end_line, "character": c.end_col },
                            },
                            "text": c.text,
                        })
                    })
                    .collect();
                (
                    "ext/didChangeTextDocument".to_string(),
                    json!({
                        "uri": uri,
                        "version": version,
                        "changes": change_list,
                    }),
                )
            }
            DocumentEvent::DidSave { uri } => (
                "ext/didSaveTextDocument".to_string(),
                json!({ "uri": uri }),
            ),
            DocumentEvent::DidClose { uri } => (
                "ext/didCloseTextDocument".to_string(),
                json!({ "uri": uri }),
            ),
        }
    }
}

/// Dispatches incoming mainThread/* RPC method calls to registered handlers.
pub struct MainThreadHandlers {
    handlers: HashMap<String, HandlerFn>,
    next_channel_id: Arc<AtomicU64>,
    provider_registry: Arc<Mutex<ProviderRegistry>>,
}

impl MainThreadHandlers {
    /// Create an empty handler registry.
    pub fn new() -> Self {
        Self {
            handlers: HashMap::new(),
            next_channel_id: Arc::new(AtomicU64::new(1)),
            provider_registry: Arc::new(Mutex::new(ProviderRegistry::new())),
        }
    }

    /// Register a handler for a given RPC method name (e.g. `"mainThread/showMessage"`).
    pub fn register(
        &mut self,
        method: &str,
        handler: impl Fn(Value) -> Value + Send + Sync + 'static,
    ) {
        self.handlers.insert(method.to_string(), Box::new(handler));
    }

    /// Dispatch an incoming method call, returning `None` if no handler is
    /// registered for that method.
    pub fn handle(&self, method: &str, params: Value) -> Option<Value> {
        self.handlers.get(method).map(|h| h(params))
    }

    /// Return the list of all registered method names.
    pub fn registered_methods(&self) -> Vec<&str> {
        self.handlers.keys().map(|s| s.as_str()).collect()
    }

    /// Get a reference to the provider registry.
    pub fn provider_registry(&self) -> Arc<Mutex<ProviderRegistry>> {
        Arc::clone(&self.provider_registry)
    }

    /// Register the default set of mainThread/* stub handlers that the JS
    /// extension host shim expects.
    pub fn register_defaults(&mut self) {
        // -- Window / messages --

        self.register("mainThread/showMessage", |params| {
            let severity = params.get("severity").and_then(|v| v.as_str()).unwrap_or("info");
            let message = params.get("message").and_then(|v| v.as_str()).unwrap_or("");
            info!(severity, message, "mainThread/showMessage");
            Value::Null
        });

        self.register("mainThread/showQuickPick", |params| {
            let items = params.get("items").and_then(|v| v.as_array());
            info!("mainThread/showQuickPick (stub: returning first item)");
            match items.and_then(|arr| arr.first()) {
                Some(item) => item.clone(),
                None => Value::Null,
            }
        });

        self.register("mainThread/showInputBox", |_params| {
            info!("mainThread/showInputBox (stub: returning empty string)");
            json!("")
        });

        self.register("mainThread/showOpenDialog", |_params| {
            info!("mainThread/showOpenDialog (stub)");
            Value::Null
        });

        self.register("mainThread/showSaveDialog", |_params| {
            info!("mainThread/showSaveDialog (stub)");
            Value::Null
        });

        // -- Clipboard --

        self.register("mainThread/clipboardRead", |_params| {
            info!("mainThread/clipboardRead (stub: returning empty)");
            json!("")
        });

        self.register("mainThread/clipboardWrite", |params| {
            let text = params.get("text").and_then(|v| v.as_str()).unwrap_or("");
            info!(text, "mainThread/clipboardWrite (stub)");
            Value::Null
        });

        // -- Documents --

        self.register("mainThread/openTextDocument", |params| {
            let uri = params.get("uri").and_then(|v| v.as_str()).unwrap_or("untitled:Untitled-1");
            info!(uri, "mainThread/openTextDocument (stub)");
            json!({ "uri": uri, "languageId": "plaintext", "version": 1, "lineCount": 0 })
        });

        self.register("mainThread/saveDocument", |params| {
            let uri = params.get("uri").and_then(|v| v.as_str()).unwrap_or("");
            info!(uri, "mainThread/saveDocument (stub)");
            json!(true)
        });

        self.register("mainThread/showTextDocument", |params| {
            let uri = params.get("uri").and_then(|v| v.as_str()).unwrap_or("");
            info!(uri, "mainThread/showTextDocument (stub)");
            Value::Null
        });

        // -- Commands --

        self.register("mainThread/registerCommand", |params| {
            let id = params.get("id").and_then(|v| v.as_str()).unwrap_or("");
            info!(id, "mainThread/registerCommand (stub)");
            Value::Null
        });

        self.register("mainThread/unregisterCommand", |params| {
            let id = params.get("id").and_then(|v| v.as_str()).unwrap_or("");
            info!(id, "mainThread/unregisterCommand (stub)");
            Value::Null
        });

        self.register("mainThread/executeCommand", |params| {
            let id = params.get("id").and_then(|v| v.as_str()).unwrap_or("");
            info!(id, "mainThread/executeCommand (stub)");
            Value::Null
        });

        self.register("mainThread/getCommands", |_params| {
            info!("mainThread/getCommands (stub)");
            json!([])
        });

        // -- Configuration --

        self.register("mainThread/getConfiguration", |params| {
            let section = params.get("section").and_then(|v| v.as_str()).unwrap_or("");
            info!(section, "mainThread/getConfiguration");
            let config_path = dirs::config_dir()
                .unwrap_or_default()
                .join("vsedit")
                .join("settings.json");
            if config_path.exists() {
                if let Ok(content) = std::fs::read_to_string(&config_path) {
                    if let Ok(val) = serde_json::from_str::<Value>(&content) {
                        if section.is_empty() {
                            return val;
                        }
                        if let Some(sub) = val.get(section) {
                            return sub.clone();
                        }
                    }
                }
            }
            json!({})
        });

        self.register("mainThread/updateConfiguration", |_params| {
            info!("mainThread/updateConfiguration (stub)");
            Value::Null
        });

        // -- Output channels --

        let next_id = Arc::clone(&self.next_channel_id);
        self.register("mainThread/createOutputChannel", move |params| {
            let name = params.get("name").and_then(|v| v.as_str()).unwrap_or("output");
            let id = next_id.fetch_add(1, Ordering::Relaxed);
            info!(name, id, "mainThread/createOutputChannel (stub)");
            json!({ "id": id, "name": name })
        });

        self.register("mainThread/appendOutputChannel", |params| {
            let name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let value = params.get("value").and_then(|v| v.as_str()).unwrap_or("");
            info!(name, value, "mainThread/appendOutputChannel (stub)");
            Value::Null
        });

        // -- Content providers --

        self.register("mainThread/registerContentProvider", |params| {
            let scheme = params.get("scheme").and_then(|v| v.as_str()).unwrap_or("");
            info!(scheme, "mainThread/registerContentProvider (stub)");
            Value::Null
        });

        self.register("mainThread/unregisterContentProvider", |params| {
            let scheme = params.get("scheme").and_then(|v| v.as_str()).unwrap_or("");
            info!(scheme, "mainThread/unregisterContentProvider (stub)");
            Value::Null
        });

        // -- Status bar --

        self.register("mainThread/setStatusBarMessage", |params| {
            let text = params.get("text").and_then(|v| v.as_str()).unwrap_or("");
            info!(text, "mainThread/setStatusBarMessage (stub)");
            Value::Null
        });

        self.register("mainThread/clearStatusBarMessage", |_params| {
            info!("mainThread/clearStatusBarMessage (stub)");
            Value::Null
        });

        self.register("mainThread/statusBarShow", |_params| {
            info!("mainThread/statusBarShow (stub)");
            Value::Null
        });

        self.register("mainThread/statusBarHide", |_params| {
            info!("mainThread/statusBarHide (stub)");
            Value::Null
        });

        // -- Diagnostics --

        self.register("mainThread/setDiagnostics", |_params| {
            info!("mainThread/setDiagnostics (stub)");
            Value::Null
        });

        // -- Terminal --

        self.register("mainThread/createTerminal", |_params| {
            info!("mainThread/createTerminal (stub)");
            Value::Null
        });

        self.register("mainThread/terminalSendText", |_params| {
            info!("mainThread/terminalSendText (stub)");
            Value::Null
        });

        // -- Tree view --

        self.register("mainThread/registerTreeView", |params| {
            let view_id = params.get("viewId").and_then(|v| v.as_str()).unwrap_or("");
            info!(view_id, "mainThread/registerTreeView (stub)");
            Value::Null
        });

        // -- Progress --

        self.register("mainThread/progressReport", |_params| {
            info!("mainThread/progressReport (stub)");
            Value::Null
        });

        // -- Decorations --

        self.register("mainThread/registerDecorationType", |_params| {
            info!("mainThread/registerDecorationType (stub)");
            Value::Null
        });

        self.register("mainThread/removeDecorationType", |_params| {
            info!("mainThread/removeDecorationType (stub)");
            Value::Null
        });

        // -- Webview --

        self.register("mainThread/registerWebviewView", |_params| {
            info!("mainThread/registerWebviewView (stub)");
            Value::Null
        });

        // -- Workspace edits --

        self.register("mainThread/applyWorkspaceEdit", |_params| {
            info!("mainThread/applyWorkspaceEdit (stub)");
            json!(true)
        });

        // -- File search --

        self.register("mainThread/findFiles", |_params| {
            info!("mainThread/findFiles (stub)");
            json!([])
        });

        // -- File watchers --

        self.register("mainThread/watchFiles", |_params| {
            info!("mainThread/watchFiles (stub)");
            Value::Null
        });

        self.register("mainThread/unwatchFiles", |_params| {
            info!("mainThread/unwatchFiles (stub)");
            Value::Null
        });

        // -- File system --

        self.register("mainThread/fsReadFile", |params| {
            if let Some(path) = params.get("path").and_then(|p| p.as_str()) {
                match std::fs::read_to_string(path) {
                    Ok(content) => return json!({ "content": content }),
                    Err(e) => return json!({ "error": e.to_string() }),
                }
            }
            json!({ "error": "missing path" })
        });

        self.register("mainThread/fsWriteFile", |params| {
            let path = match params.get("path").and_then(|p| p.as_str()) {
                Some(p) => p,
                None => return json!({ "error": "missing path" }),
            };
            let content = params.get("content").and_then(|c| c.as_str()).unwrap_or("");
            match std::fs::write(path, content) {
                Ok(()) => Value::Null,
                Err(e) => json!({ "error": e.to_string() }),
            }
        });

        self.register("mainThread/fsStat", |params| {
            let path = match params.get("path").and_then(|p| p.as_str()) {
                Some(p) => p,
                None => return json!({ "error": "missing path" }),
            };
            match std::fs::metadata(path) {
                Ok(meta) => {
                    let file_type = if meta.is_dir() { "directory" } else { "file" };
                    let mtime = meta
                        .modified()
                        .ok()
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| d.as_millis() as u64)
                        .unwrap_or(0);
                    json!({ "type": file_type, "size": meta.len(), "mtime": mtime })
                }
                Err(e) => json!({ "error": e.to_string() }),
            }
        });

        self.register("mainThread/fsReadDir", |params| {
            let path = match params.get("path").and_then(|p| p.as_str()) {
                Some(p) => p,
                None => return json!({ "error": "missing path" }),
            };
            match std::fs::read_dir(path) {
                Ok(entries) => {
                    let items: Vec<Value> = entries
                        .filter_map(|e| e.ok())
                        .map(|e| {
                            let ft = if e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                                "directory"
                            } else {
                                "file"
                            };
                            json!({ "name": e.file_name().to_string_lossy(), "type": ft })
                        })
                        .collect();
                    json!(items)
                }
                Err(e) => json!({ "error": e.to_string() }),
            }
        });

        self.register("mainThread/fsCreateDir", |params| {
            let path = match params.get("path").and_then(|p| p.as_str()) {
                Some(p) => p,
                None => return json!({ "error": "missing path" }),
            };
            match std::fs::create_dir_all(path) {
                Ok(()) => Value::Null,
                Err(e) => json!({ "error": e.to_string() }),
            }
        });

        self.register("mainThread/fsDelete", |params| {
            let path = match params.get("path").and_then(|p| p.as_str()) {
                Some(p) => p,
                None => return json!({ "error": "missing path" }),
            };
            let result = if std::path::Path::new(path).is_dir() {
                std::fs::remove_dir_all(path)
            } else {
                std::fs::remove_file(path)
            };
            match result {
                Ok(()) => Value::Null,
                Err(e) => json!({ "error": e.to_string() }),
            }
        });

        self.register("mainThread/fsRename", |params| {
            let old = match params.get("oldPath").and_then(|p| p.as_str()) {
                Some(p) => p,
                None => return json!({ "error": "missing oldPath" }),
            };
            let new_path = match params.get("newPath").and_then(|p| p.as_str()) {
                Some(p) => p,
                None => return json!({ "error": "missing newPath" }),
            };
            match std::fs::rename(old, new_path) {
                Ok(()) => Value::Null,
                Err(e) => json!({ "error": e.to_string() }),
            }
        });

        self.register("mainThread/fsCopy", |params| {
            let src = match params.get("source").and_then(|p| p.as_str()) {
                Some(p) => p,
                None => return json!({ "error": "missing source" }),
            };
            let dest = match params.get("destination").and_then(|p| p.as_str()) {
                Some(p) => p,
                None => return json!({ "error": "missing destination" }),
            };
            match std::fs::copy(src, dest) {
                Ok(_) => Value::Null,
                Err(e) => json!({ "error": e.to_string() }),
            }
        });

        // -- Language feature providers --

        let reg = Arc::clone(&self.provider_registry);
        self.register("mainThread/registerCompletionProvider", move |params| {
            let ext_id = params.get("extensionId").and_then(|e| e.as_str()).unwrap_or("unknown");
            let selector = params.get("selector").cloned().unwrap_or(Value::Null);
            let handle = reg.lock().unwrap().register(ProviderKind::Completion, ext_id, selector);
            info!(handle, ext_id, "Registered completion provider");
            json!({ "handle": handle })
        });

        let reg = Arc::clone(&self.provider_registry);
        self.register("mainThread/registerHoverProvider", move |params| {
            let ext_id = params.get("extensionId").and_then(|e| e.as_str()).unwrap_or("unknown");
            let selector = params.get("selector").cloned().unwrap_or(Value::Null);
            let handle = reg.lock().unwrap().register(ProviderKind::Hover, ext_id, selector);
            info!(handle, ext_id, "Registered hover provider");
            json!({ "handle": handle })
        });

        let reg = Arc::clone(&self.provider_registry);
        self.register("mainThread/registerDefinitionProvider", move |params| {
            let ext_id = params.get("extensionId").and_then(|e| e.as_str()).unwrap_or("unknown");
            let selector = params.get("selector").cloned().unwrap_or(Value::Null);
            let handle = reg.lock().unwrap().register(ProviderKind::Definition, ext_id, selector);
            info!(handle, ext_id, "Registered definition provider");
            json!({ "handle": handle })
        });

        let reg = Arc::clone(&self.provider_registry);
        self.register("mainThread/registerCodeActionsProvider", move |params| {
            let ext_id = params.get("extensionId").and_then(|e| e.as_str()).unwrap_or("unknown");
            let selector = params.get("selector").cloned().unwrap_or(Value::Null);
            let handle = reg.lock().unwrap().register(ProviderKind::CodeActions, ext_id, selector);
            info!(handle, ext_id, "Registered code actions provider");
            json!({ "handle": handle })
        });

        let reg = Arc::clone(&self.provider_registry);
        self.register("mainThread/registerFormattingProvider", move |params| {
            let ext_id = params.get("extensionId").and_then(|e| e.as_str()).unwrap_or("unknown");
            let selector = params.get("selector").cloned().unwrap_or(Value::Null);
            let handle = reg.lock().unwrap().register(ProviderKind::Formatting, ext_id, selector);
            info!(handle, ext_id, "Registered formatting provider");
            json!({ "handle": handle })
        });

        let reg = Arc::clone(&self.provider_registry);
        self.register("mainThread/registerRangeFormattingProvider", move |params| {
            let ext_id = params.get("extensionId").and_then(|e| e.as_str()).unwrap_or("unknown");
            let selector = params.get("selector").cloned().unwrap_or(Value::Null);
            let handle = reg.lock().unwrap().register(ProviderKind::RangeFormatting, ext_id, selector);
            info!(handle, ext_id, "Registered range formatting provider");
            json!({ "handle": handle })
        });

        let reg = Arc::clone(&self.provider_registry);
        self.register("mainThread/unregisterProvider", move |params| {
            if let Some(handle) = params.get("handle").and_then(|h| h.as_u64()) {
                let removed = reg.lock().unwrap().unregister(handle);
                info!(handle, removed, "Unregistered provider");
            }
            Value::Null
        });

        // Additional provider types that extensions commonly register
        let reg = Arc::clone(&self.provider_registry);
        self.register("mainThread/registerReferencesProvider", move |params| {
            let ext_id = params.get("extensionId").and_then(|e| e.as_str()).unwrap_or("unknown");
            let selector = params.get("selector").cloned().unwrap_or(Value::Null);
            let handle = reg.lock().unwrap().register(ProviderKind::References, ext_id, selector);
            json!({ "handle": handle })
        });

        let reg = Arc::clone(&self.provider_registry);
        self.register("mainThread/registerRenameProvider", move |params| {
            let ext_id = params.get("extensionId").and_then(|e| e.as_str()).unwrap_or("unknown");
            let selector = params.get("selector").cloned().unwrap_or(Value::Null);
            let handle = reg.lock().unwrap().register(ProviderKind::Rename, ext_id, selector);
            json!({ "handle": handle })
        });

        let reg = Arc::clone(&self.provider_registry);
        self.register("mainThread/registerSignatureHelpProvider", move |params| {
            let ext_id = params.get("extensionId").and_then(|e| e.as_str()).unwrap_or("unknown");
            let selector = params.get("selector").cloned().unwrap_or(Value::Null);
            let handle = reg.lock().unwrap().register(ProviderKind::SignatureHelp, ext_id, selector);
            json!({ "handle": handle })
        });

        let reg = Arc::clone(&self.provider_registry);
        self.register("mainThread/registerDocumentHighlightProvider", move |params| {
            let ext_id = params.get("extensionId").and_then(|e| e.as_str()).unwrap_or("unknown");
            let selector = params.get("selector").cloned().unwrap_or(Value::Null);
            let handle = reg.lock().unwrap().register(ProviderKind::DocumentHighlight, ext_id, selector);
            json!({ "handle": handle })
        });

        let reg = Arc::clone(&self.provider_registry);
        self.register("mainThread/registerDocumentLinkProvider", move |params| {
            let ext_id = params.get("extensionId").and_then(|e| e.as_str()).unwrap_or("unknown");
            let selector = params.get("selector").cloned().unwrap_or(Value::Null);
            let handle = reg.lock().unwrap().register(ProviderKind::DocumentLink, ext_id, selector);
            json!({ "handle": handle })
        });

        let reg = Arc::clone(&self.provider_registry);
        self.register("mainThread/registerColorProvider", move |params| {
            let ext_id = params.get("extensionId").and_then(|e| e.as_str()).unwrap_or("unknown");
            let selector = params.get("selector").cloned().unwrap_or(Value::Null);
            let handle = reg.lock().unwrap().register(ProviderKind::DocumentColor, ext_id, selector);
            json!({ "handle": handle })
        });

        let reg = Arc::clone(&self.provider_registry);
        self.register("mainThread/registerFoldingRangeProvider", move |params| {
            let ext_id = params.get("extensionId").and_then(|e| e.as_str()).unwrap_or("unknown");
            let selector = params.get("selector").cloned().unwrap_or(Value::Null);
            let handle = reg.lock().unwrap().register(ProviderKind::FoldingRange, ext_id, selector);
            json!({ "handle": handle })
        });

        let reg = Arc::clone(&self.provider_registry);
        self.register("mainThread/registerCodeLensProvider", move |params| {
            let ext_id = params.get("extensionId").and_then(|e| e.as_str()).unwrap_or("unknown");
            let selector = params.get("selector").cloned().unwrap_or(Value::Null);
            let handle = reg.lock().unwrap().register(ProviderKind::CodeLens, ext_id, selector);
            json!({ "handle": handle })
        });

        let reg = Arc::clone(&self.provider_registry);
        self.register("mainThread/registerDocumentSymbolProvider", move |params| {
            let ext_id = params.get("extensionId").and_then(|e| e.as_str()).unwrap_or("unknown");
            let selector = params.get("selector").cloned().unwrap_or(Value::Null);
            let handle = reg.lock().unwrap().register(ProviderKind::DocumentSymbol, ext_id, selector);
            json!({ "handle": handle })
        });

        let reg = Arc::clone(&self.provider_registry);
        self.register("mainThread/registerWorkspaceSymbolProvider", move |params| {
            let ext_id = params.get("extensionId").and_then(|e| e.as_str()).unwrap_or("unknown");
            let handle = reg.lock().unwrap().register(ProviderKind::WorkspaceSymbol, ext_id, Value::Null);
            json!({ "handle": handle })
        });

        let reg = Arc::clone(&self.provider_registry);
        self.register("mainThread/registerTypeDefinitionProvider", move |params| {
            let ext_id = params.get("extensionId").and_then(|e| e.as_str()).unwrap_or("unknown");
            let selector = params.get("selector").cloned().unwrap_or(Value::Null);
            let handle = reg.lock().unwrap().register(ProviderKind::TypeDefinition, ext_id, selector);
            json!({ "handle": handle })
        });

        let reg = Arc::clone(&self.provider_registry);
        self.register("mainThread/registerImplementationProvider", move |params| {
            let ext_id = params.get("extensionId").and_then(|e| e.as_str()).unwrap_or("unknown");
            let selector = params.get("selector").cloned().unwrap_or(Value::Null);
            let handle = reg.lock().unwrap().register(ProviderKind::Implementation, ext_id, selector);
            json!({ "handle": handle })
        });

        let reg = Arc::clone(&self.provider_registry);
        self.register("mainThread/registerInlayHintProvider", move |params| {
            let ext_id = params.get("extensionId").and_then(|e| e.as_str()).unwrap_or("unknown");
            let selector = params.get("selector").cloned().unwrap_or(Value::Null);
            let handle = reg.lock().unwrap().register(ProviderKind::InlayHint, ext_id, selector);
            json!({ "handle": handle })
        });

        // -- Output channel lifecycle --

        self.register("mainThread/outputAppend", |_params| {
            info!("mainThread/outputAppend (stub)");
            Value::Null
        });

        self.register("mainThread/outputReplace", |_params| {
            info!("mainThread/outputReplace (stub)");
            Value::Null
        });

        self.register("mainThread/outputClear", |_params| {
            info!("mainThread/outputClear (stub)");
            Value::Null
        });

        self.register("mainThread/outputShow", |_params| {
            info!("mainThread/outputShow (stub)");
            Value::Null
        });

        self.register("mainThread/outputHide", |_params| {
            info!("mainThread/outputHide (stub)");
            Value::Null
        });

        self.register("mainThread/outputDispose", |_params| {
            info!("mainThread/outputDispose (stub)");
            Value::Null
        });

        // -- Language --

        self.register("mainThread/setLanguage", |params| {
            if let Some(lang) = params.get("languageId").and_then(|l| l.as_str()) {
                info!(lang, "mainThread/setLanguage");
            }
            Value::Null
        });

        self.register("mainThread/getLanguages", |_params| {
            json!([
                "rust", "python", "javascript", "typescript", "json", "yaml", "toml",
                "markdown", "html", "css", "c", "cpp", "java", "go", "ruby", "php",
                "shell", "sql", "xml", "plaintext"
            ])
        });

        // -- External --

        self.register("mainThread/openExternal", |params| {
            if let Some(uri) = params.get("uri").and_then(|u| u.as_str()) {
                info!(uri, "mainThread/openExternal");
                #[cfg(target_os = "linux")]
                {
                    let _ = std::process::Command::new("xdg-open").arg(uri).spawn();
                }
                #[cfg(target_os = "macos")]
                {
                    let _ = std::process::Command::new("open").arg(uri).spawn();
                }
            }
            json!(true)
        });

        // -- Secrets --

        self.register("mainThread/secretGet", |_params| Value::Null);

        self.register("mainThread/secretStore", |params| {
            debug!("mainThread/secretStore: {:?}", params.get("key"));
            json!(true)
        });

        self.register("mainThread/secretDelete", |_params| json!(true));

        // -- Memento --

        self.register("mainThread/mementoUpdate", |_params| json!(true));

        // -- Source control --

        self.register("mainThread/registerSourceControl", |params| {
            if let Some(id) = params.get("id").and_then(|i| i.as_str()) {
                info!(id, "mainThread/registerSourceControl");
            }
            json!({ "handle": 1 })
        });

        self.register("mainThread/unregisterSourceControl", |_params| Value::Null);
    }
}

impl Default for MainThreadHandlers {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn handlers_with_defaults() -> MainThreadHandlers {
        let mut h = MainThreadHandlers::new();
        h.register_defaults();
        h
    }

    // ── Registration / dispatch basics ──

    #[test]
    fn new_has_no_handlers() {
        let h = MainThreadHandlers::new();
        assert!(h.registered_methods().is_empty());
    }

    #[test]
    fn register_custom_handler() {
        let mut h = MainThreadHandlers::new();
        h.register("test/method", |_| json!("ok"));
        assert!(h.registered_methods().contains(&"test/method"));
        assert_eq!(h.handle("test/method", json!({})), Some(json!("ok")));
    }

    #[test]
    fn unknown_method_returns_none() {
        let h = handlers_with_defaults();
        assert_eq!(h.handle("mainThread/nonExistent", json!({})), None);
    }

    #[test]
    fn defaults_registers_many_handlers() {
        let h = handlers_with_defaults();
        assert!(h.registered_methods().len() >= 15);
    }

    // ── showMessage ──

    #[test]
    fn show_info_message_returns_null() {
        let h = handlers_with_defaults();
        let result = h.handle(
            "mainThread/showMessage",
            json!({"severity": "info", "message": "Hello!", "items": []}),
        );
        assert_eq!(result, Some(Value::Null));
    }

    #[test]
    fn show_warning_message_returns_null() {
        let h = handlers_with_defaults();
        let result = h.handle(
            "mainThread/showMessage",
            json!({"severity": "warning", "message": "Watch out"}),
        );
        assert_eq!(result, Some(Value::Null));
    }

    #[test]
    fn show_error_message_returns_null() {
        let h = handlers_with_defaults();
        let result = h.handle(
            "mainThread/showMessage",
            json!({"severity": "error", "message": "Oops"}),
        );
        assert_eq!(result, Some(Value::Null));
    }

    // ── showQuickPick ──

    #[test]
    fn quick_pick_returns_first_item() {
        let h = handlers_with_defaults();
        let result = h.handle(
            "mainThread/showQuickPick",
            json!({"items": ["alpha", "beta"]}),
        );
        assert_eq!(result, Some(json!("alpha")));
    }

    #[test]
    fn quick_pick_empty_items_returns_null() {
        let h = handlers_with_defaults();
        let result = h.handle("mainThread/showQuickPick", json!({"items": []}));
        assert_eq!(result, Some(Value::Null));
    }

    // ── showInputBox ──

    #[test]
    fn input_box_returns_empty_string() {
        let h = handlers_with_defaults();
        let result = h.handle("mainThread/showInputBox", json!({}));
        assert_eq!(result, Some(json!("")));
    }

    // ── Clipboard ──

    #[test]
    fn clipboard_read_returns_empty() {
        let h = handlers_with_defaults();
        let result = h.handle("mainThread/clipboardRead", json!({}));
        assert_eq!(result, Some(json!("")));
    }

    #[test]
    fn clipboard_write_returns_null() {
        let h = handlers_with_defaults();
        let result = h.handle(
            "mainThread/clipboardWrite",
            json!({"text": "hello clipboard"}),
        );
        assert_eq!(result, Some(Value::Null));
    }

    // ── Documents ──

    #[test]
    fn open_text_document_returns_doc_stub() {
        let h = handlers_with_defaults();
        let result = h
            .handle(
                "mainThread/openTextDocument",
                json!({"uri": "file:///test.txt"}),
            )
            .unwrap();
        assert_eq!(result["uri"], "file:///test.txt");
        assert_eq!(result["languageId"], "plaintext");
    }

    // ── Commands ──

    #[test]
    fn register_command_returns_null() {
        let h = handlers_with_defaults();
        let result = h.handle(
            "mainThread/registerCommand",
            json!({"id": "myext.doStuff"}),
        );
        assert_eq!(result, Some(Value::Null));
    }

    #[test]
    fn execute_command_returns_null() {
        let h = handlers_with_defaults();
        let result = h.handle(
            "mainThread/executeCommand",
            json!({"id": "workbench.action.files.save", "args": []}),
        );
        assert_eq!(result, Some(Value::Null));
    }

    // ── Configuration ──

    #[test]
    fn get_configuration_returns_empty_object() {
        let h = handlers_with_defaults();
        let result = h.handle(
            "mainThread/getConfiguration",
            json!({"section": "editor"}),
        );
        assert_eq!(result, Some(json!({})));
    }

    // ── Output channels ──

    #[test]
    fn create_output_channel_returns_id() {
        let h = handlers_with_defaults();
        let result = h
            .handle(
                "mainThread/createOutputChannel",
                json!({"name": "MyChannel"}),
            )
            .unwrap();
        assert!(result.get("id").is_some());
        assert_eq!(result["name"], "MyChannel");
    }

    #[test]
    fn append_output_channel_returns_null() {
        let h = handlers_with_defaults();
        let result = h.handle(
            "mainThread/appendOutputChannel",
            json!({"name": "MyChannel", "value": "hello"}),
        );
        assert_eq!(result, Some(Value::Null));
    }

    // ── Content provider ──

    #[test]
    fn register_content_provider_returns_null() {
        let h = handlers_with_defaults();
        let result = h.handle(
            "mainThread/registerContentProvider",
            json!({"scheme": "git"}),
        );
        assert_eq!(result, Some(Value::Null));
    }

    // ── Status bar ──

    #[test]
    fn set_status_bar_message_returns_null() {
        let h = handlers_with_defaults();
        let result = h.handle(
            "mainThread/setStatusBarMessage",
            json!({"text": "Building…"}),
        );
        assert_eq!(result, Some(Value::Null));
    }

    // ── Handler params are forwarded ──

    #[test]
    fn handler_receives_params() {
        let mut h = MainThreadHandlers::new();
        h.register("test/echo", |p| p);
        let input = json!({"a": 1, "b": [2, 3]});
        assert_eq!(h.handle("test/echo", input.clone()), Some(input));
    }

    // ── Channel ID increments ──

    #[test]
    fn output_channel_ids_increment() {
        let h = handlers_with_defaults();
        let r1 = h
            .handle(
                "mainThread/createOutputChannel",
                json!({"name": "ch1"}),
            )
            .unwrap();
        let r2 = h
            .handle(
                "mainThread/createOutputChannel",
                json!({"name": "ch2"}),
            )
            .unwrap();
        let id1 = r1["id"].as_u64().unwrap();
        let id2 = r2["id"].as_u64().unwrap();
        assert_eq!(id2, id1 + 1);
    }

    // ── Default trait ──

    #[test]
    fn default_creates_empty() {
        let h = MainThreadHandlers::default();
        assert!(h.registered_methods().is_empty());
    }

    // ── Provider Registry ──

    #[test]
    fn provider_registry_register_and_get() {
        let mut reg = ProviderRegistry::new();
        let h = reg.register(ProviderKind::Completion, "test-ext", json!({"language": "rust"}));
        assert_eq!(h, 1);
        assert!(reg.get(h).is_some());
        assert_eq!(reg.get(h).unwrap().extension_id, "test-ext");
    }

    #[test]
    fn provider_registry_unregister() {
        let mut reg = ProviderRegistry::new();
        let h = reg.register(ProviderKind::Hover, "ext1", Value::Null);
        assert_eq!(reg.count(), 1);
        assert!(reg.unregister(h));
        assert_eq!(reg.count(), 0);
        assert!(!reg.unregister(999));
    }

    #[test]
    fn provider_registry_providers_for() {
        let mut reg = ProviderRegistry::new();
        reg.register(ProviderKind::Completion, "ext1", Value::Null);
        reg.register(ProviderKind::Hover, "ext2", Value::Null);
        reg.register(ProviderKind::Completion, "ext3", Value::Null);
        assert_eq!(reg.providers_for(ProviderKind::Completion).len(), 2);
        assert_eq!(reg.providers_for(ProviderKind::Hover).len(), 1);
        assert_eq!(reg.providers_for(ProviderKind::Definition).len(), 0);
    }

    #[test]
    fn provider_registry_has_provider() {
        let mut reg = ProviderRegistry::new();
        assert!(!reg.has_provider(ProviderKind::Completion));
        reg.register(ProviderKind::Completion, "ext1", Value::Null);
        assert!(reg.has_provider(ProviderKind::Completion));
    }

    #[test]
    fn provider_registry_handles_increment() {
        let mut reg = ProviderRegistry::new();
        let h1 = reg.register(ProviderKind::Completion, "ext1", Value::Null);
        let h2 = reg.register(ProviderKind::Hover, "ext2", Value::Null);
        assert_eq!(h2, h1 + 1);
    }

    #[test]
    fn register_completion_provider_returns_handle() {
        let h = handlers_with_defaults();
        let result = h.handle(
            "mainThread/registerCompletionProvider",
            json!({"extensionId": "test.ext", "selector": {"language": "rust"}}),
        ).unwrap();
        assert!(result.get("handle").and_then(|h| h.as_u64()).is_some());
    }

    #[test]
    fn register_hover_provider_returns_handle() {
        let h = handlers_with_defaults();
        let result = h.handle(
            "mainThread/registerHoverProvider",
            json!({"extensionId": "hover.ext"}),
        ).unwrap();
        assert!(result.get("handle").is_some());
    }

    #[test]
    fn unregister_provider_removes_it() {
        let h = handlers_with_defaults();
        let result = h.handle(
            "mainThread/registerCompletionProvider",
            json!({"extensionId": "test.ext"}),
        ).unwrap();
        let handle = result["handle"].as_u64().unwrap();
        let reg = h.provider_registry();
        assert_eq!(reg.lock().unwrap().count(), 1);
        h.handle("mainThread/unregisterProvider", json!({"handle": handle}));
        assert_eq!(reg.lock().unwrap().count(), 0);
    }

    #[test]
    fn multiple_provider_types_tracked() {
        let h = handlers_with_defaults();
        h.handle("mainThread/registerCompletionProvider", json!({"extensionId": "ext1"}));
        h.handle("mainThread/registerHoverProvider", json!({"extensionId": "ext2"}));
        h.handle("mainThread/registerDefinitionProvider", json!({"extensionId": "ext3"}));
        h.handle("mainThread/registerCodeActionsProvider", json!({"extensionId": "ext4"}));
        let reg = h.provider_registry();
        let locked = reg.lock().unwrap();
        assert_eq!(locked.count(), 4);
        assert!(locked.has_provider(ProviderKind::Completion));
        assert!(locked.has_provider(ProviderKind::Hover));
        assert!(locked.has_provider(ProviderKind::Definition));
        assert!(locked.has_provider(ProviderKind::CodeActions));
    }

    // ── Document Events ──

    #[test]
    fn document_event_did_open_rpc() {
        let event = DocumentEvent::DidOpen {
            uri: "file:///test.rs".to_string(),
            language_id: "rust".to_string(),
            version: 1,
            content: "fn main() {}".to_string(),
        };
        let (method, params) = event.to_rpc_notification();
        assert_eq!(method, "ext/didOpenTextDocument");
        assert_eq!(params["uri"], "file:///test.rs");
        assert_eq!(params["languageId"], "rust");
        assert_eq!(params["version"], 1);
    }

    #[test]
    fn document_event_did_change_rpc() {
        let event = DocumentEvent::DidChange {
            uri: "file:///test.rs".to_string(),
            version: 2,
            changes: vec![DocumentChange {
                start_line: 0,
                start_col: 0,
                end_line: 0,
                end_col: 5,
                text: "hello".to_string(),
            }],
        };
        let (method, params) = event.to_rpc_notification();
        assert_eq!(method, "ext/didChangeTextDocument");
        assert_eq!(params["version"], 2);
        assert!(params["changes"].is_array());
    }

    #[test]
    fn document_event_did_save_rpc() {
        let event = DocumentEvent::DidSave {
            uri: "file:///test.rs".to_string(),
        };
        let (method, params) = event.to_rpc_notification();
        assert_eq!(method, "ext/didSaveTextDocument");
        assert_eq!(params["uri"], "file:///test.rs");
    }

    #[test]
    fn document_event_did_close_rpc() {
        let event = DocumentEvent::DidClose {
            uri: "file:///test.rs".to_string(),
        };
        let (method, params) = event.to_rpc_notification();
        assert_eq!(method, "ext/didCloseTextDocument");
        assert_eq!(params["uri"], "file:///test.rs");
    }

    // ── New Handlers ──

    #[test]
    fn set_language_handler() {
        let h = handlers_with_defaults();
        let result = h.handle("mainThread/setLanguage", json!({"languageId": "python"}));
        assert_eq!(result, Some(Value::Null));
    }

    #[test]
    fn get_languages_returns_list() {
        let h = handlers_with_defaults();
        let result = h.handle("mainThread/getLanguages", json!({})).unwrap();
        let langs = result.as_array().unwrap();
        assert!(langs.len() >= 10);
        assert!(langs.contains(&json!("rust")));
        assert!(langs.contains(&json!("python")));
    }

    #[test]
    fn secret_get_returns_null() {
        let h = handlers_with_defaults();
        let result = h.handle("mainThread/secretGet", json!({"key": "test"}));
        assert_eq!(result, Some(Value::Null));
    }

    #[test]
    fn secret_store_returns_true() {
        let h = handlers_with_defaults();
        let result = h.handle("mainThread/secretStore", json!({"key": "k", "value": "v"}));
        assert_eq!(result, Some(json!(true)));
    }

    #[test]
    fn register_source_control_returns_handle() {
        let h = handlers_with_defaults();
        let result = h.handle("mainThread/registerSourceControl", json!({"id": "git"})).unwrap();
        assert!(result.get("handle").is_some());
    }

    #[test]
    fn fs_read_file_nonexistent() {
        let h = handlers_with_defaults();
        let result = h.handle("mainThread/fsReadFile", json!({"path": "/nonexistent/path/xyz"})).unwrap();
        assert!(result.get("error").is_some());
    }

    #[test]
    fn fs_stat_nonexistent() {
        let h = handlers_with_defaults();
        let result = h.handle("mainThread/fsStat", json!({"path": "/nonexistent/path/xyz"})).unwrap();
        assert!(result.get("error").is_some());
    }

    #[test]
    fn register_additional_providers() {
        let h = handlers_with_defaults();
        for method in &[
            "mainThread/registerReferencesProvider",
            "mainThread/registerRenameProvider",
            "mainThread/registerSignatureHelpProvider",
            "mainThread/registerDocumentHighlightProvider",
            "mainThread/registerDocumentLinkProvider",
            "mainThread/registerColorProvider",
            "mainThread/registerFoldingRangeProvider",
            "mainThread/registerCodeLensProvider",
            "mainThread/registerDocumentSymbolProvider",
            "mainThread/registerWorkspaceSymbolProvider",
            "mainThread/registerTypeDefinitionProvider",
            "mainThread/registerImplementationProvider",
            "mainThread/registerInlayHintProvider",
        ] {
            let result = h.handle(method, json!({"extensionId": "test"})).unwrap();
            assert!(result.get("handle").is_some(), "Expected handle from {}", method);
        }
        let reg = h.provider_registry();
        assert_eq!(reg.lock().unwrap().count(), 13);
    }
}
