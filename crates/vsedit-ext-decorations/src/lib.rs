//! Ext API: Decorations.
//!
//! RPC bridge between the extension host and the main thread for editor decorations.

use serde::{Deserialize, Serialize};

/// Proxy identifier for this extension API namespace.
pub const PROXY_ID: &str = "ext_decorations";

// ── RPC Messages ──

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum DecorationMessage {
    RegisterType {
        key: String,
        options: DecorationRenderOptions,
    },
    UnregisterType {
        key: String,
    },
    SetDecorations {
        key: String,
        uri: String,
        ranges: Vec<DecorationOptions>,
    },
}

// ── Core Types ──

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DecorationRenderOptions {
    pub background_color: Option<String>,
    pub border: Option<String>,
    pub color: Option<String>,
    pub font_style: Option<String>,
    pub font_weight: Option<String>,
    pub is_whole_line: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DecorationType {
    pub key: String,
    pub options: DecorationRenderOptions,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DecorationOptions {
    pub start_line: u32,
    pub start_character: u32,
    pub end_line: u32,
    pub end_character: u32,
    pub hover_message: Option<String>,
}

// ── Bridge ──

pub struct DecorationBridge {
    types: Vec<DecorationType>,
    applied: Vec<(String, String, Vec<DecorationOptions>)>,
}

impl DecorationBridge {
    pub fn new() -> Self {
        Self {
            types: Vec::new(),
            applied: Vec::new(),
        }
    }

    pub fn register_type(&mut self, key: &str, options: DecorationRenderOptions) {
        if !self.types.iter().any(|t| t.key == key) {
            self.types.push(DecorationType {
                key: key.to_string(),
                options,
            });
        }
    }

    pub fn unregister_type(&mut self, key: &str) {
        self.types.retain(|t| t.key != key);
        self.applied.retain(|(k, _, _)| k != key);
    }

    pub fn set_decorations(&mut self, key: &str, uri: &str, ranges: Vec<DecorationOptions>) {
        self.applied.retain(|(k, u, _)| !(k == key && u == uri));
        if !ranges.is_empty() {
            self.applied.push((key.to_string(), uri.to_string(), ranges));
        }
    }

    pub fn has_type(&self, key: &str) -> bool {
        self.types.iter().any(|t| t.key == key)
    }

    pub fn handle_message(&mut self, msg: &DecorationMessage) -> serde_json::Value {
        match msg {
            DecorationMessage::RegisterType { key, options } => {
                self.register_type(key, options.clone());
                serde_json::json!({"registered": true})
            }
            DecorationMessage::UnregisterType { key } => {
                self.unregister_type(key);
                serde_json::json!({"unregistered": true})
            }
            DecorationMessage::SetDecorations { key, uri, ranges } => {
                self.set_decorations(key, uri, ranges.clone());
                serde_json::json!({"set": ranges.len()})
            }
        }
    }
}

impl Default for DecorationBridge {
    fn default() -> Self {
        Self::new()
    }
}

/// Initialize the decorations extension API bridge.
pub fn register() {
    // Registration will connect RPC handlers when extension host starts
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
        let msg = DecorationMessage::RegisterType {
            key: "highlight".into(),
            options: DecorationRenderOptions {
                background_color: Some("yellow".into()),
                border: None,
                color: None,
                font_style: None,
                font_weight: None,
                is_whole_line: false,
            },
        };
        let json = serde_json::to_string(&msg).unwrap();
        let back: DecorationMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, back);
    }

    #[test]
    fn decoration_options_serialization() {
        let opt = DecorationOptions {
            start_line: 1,
            start_character: 0,
            end_line: 1,
            end_character: 10,
            hover_message: Some("error here".into()),
        };
        let json = serde_json::to_string(&opt).unwrap();
        let back: DecorationOptions = serde_json::from_str(&json).unwrap();
        assert_eq!(opt, back);
    }

    #[test]
    fn bridge_register_and_unregister() {
        let mut bridge = DecorationBridge::new();
        bridge.register_type(
            "err",
            DecorationRenderOptions {
                background_color: Some("red".into()),
                border: None,
                color: None,
                font_style: None,
                font_weight: None,
                is_whole_line: true,
            },
        );
        assert!(bridge.has_type("err"));
        bridge.unregister_type("err");
        assert!(!bridge.has_type("err"));
    }

    #[test]
    fn bridge_set_clears_old() {
        let mut bridge = DecorationBridge::new();
        let opts = vec![DecorationOptions {
            start_line: 1,
            start_character: 0,
            end_line: 1,
            end_character: 5,
            hover_message: None,
        }];
        bridge.set_decorations("k", "file:///a", opts.clone());
        bridge.set_decorations("k", "file:///a", vec![]);
        assert!(bridge.applied.is_empty());
    }

    #[test]
    fn bridge_unregister_cleans_applied() {
        let mut bridge = DecorationBridge::new();
        let render = DecorationRenderOptions {
            background_color: None,
            border: None,
            color: None,
            font_style: None,
            font_weight: None,
            is_whole_line: false,
        };
        bridge.register_type("k", render);
        bridge.set_decorations(
            "k",
            "file:///a",
            vec![DecorationOptions {
                start_line: 1,
                start_character: 0,
                end_line: 1,
                end_character: 5,
                hover_message: None,
            }],
        );
        bridge.unregister_type("k");
        assert!(bridge.applied.is_empty());
    }
}
