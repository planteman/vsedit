//! Main-thread RPC message handlers for vscode.* API stubs.
//!
//! When an extension running in the JS shim calls a `vscode.*` API, the shim
//! sends an RPC request such as `mainThread/showMessage` to the Rust side.
//! [`MainThreadHandlers`] dispatches these requests to stub handlers that
//! return valid JSON responses so extensions do not crash.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use serde_json::{json, Value};
use tracing::{debug, info};

type HandlerFn = Box<dyn Fn(Value) -> Value + Send + Sync>;

/// Dispatches incoming mainThread/* RPC method calls to registered handlers.
pub struct MainThreadHandlers {
    handlers: HashMap<String, HandlerFn>,
    next_channel_id: Arc<AtomicU64>,
}

impl MainThreadHandlers {
    /// Create an empty handler registry.
    pub fn new() -> Self {
        Self {
            handlers: HashMap::new(),
            next_channel_id: Arc::new(AtomicU64::new(1)),
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

        self.register("mainThread/registerCompletionProvider", |_params| {
            info!("mainThread/registerCompletionProvider (stub)");
            Value::Null
        });

        self.register("mainThread/registerHoverProvider", |_params| {
            info!("mainThread/registerHoverProvider (stub)");
            Value::Null
        });

        self.register("mainThread/registerDefinitionProvider", |_params| {
            info!("mainThread/registerDefinitionProvider (stub)");
            Value::Null
        });

        self.register("mainThread/registerCodeActionsProvider", |_params| {
            info!("mainThread/registerCodeActionsProvider (stub)");
            Value::Null
        });

        self.register("mainThread/registerFormattingProvider", |_params| {
            info!("mainThread/registerFormattingProvider (stub)");
            Value::Null
        });

        self.register("mainThread/registerRangeFormattingProvider", |_params| {
            info!("mainThread/registerRangeFormattingProvider (stub)");
            Value::Null
        });

        self.register("mainThread/unregisterProvider", |_params| {
            info!("mainThread/unregisterProvider (stub)");
            Value::Null
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
}
