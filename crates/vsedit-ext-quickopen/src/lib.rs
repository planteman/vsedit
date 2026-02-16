//! Ext API: Quick open.
//!
//! RPC bridge between the extension host and the main thread for QuickPick/InputBox.

use serde::{Deserialize, Serialize};

/// Proxy identifier for this extension API namespace.
pub const PROXY_ID: &str = "ext_quickopen";

// ── RPC Messages ──

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum QuickOpenMessage {
    ShowQuickPick {
        items: Vec<QuickPickItem>,
        options: QuickPickOptions,
    },
    ShowInputBox {
        options: InputBoxOptions,
    },
    Hide,
    SetItems {
        items: Vec<QuickPickItem>,
    },
    ItemSelected {
        index: usize,
    },
}

// ── Core Types ──

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QuickPickItem {
    pub label: String,
    pub description: Option<String>,
    pub detail: Option<String>,
    pub picked: bool,
    pub always_show: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QuickPickOptions {
    pub placeholder: Option<String>,
    pub can_pick_many: bool,
    pub match_on_description: bool,
    pub match_on_detail: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InputBoxOptions {
    pub prompt: Option<String>,
    pub placeholder: Option<String>,
    pub value: Option<String>,
    pub password: bool,
}

// ── Separators & Entries ──

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QuickPickSeparator {
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum QuickPickEntry {
    Item(QuickPickItem),
    Separator(QuickPickSeparator),
}

// ── Input Validation ──

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum InputValidationSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InputBoxValidation {
    pub message: String,
    pub severity: InputValidationSeverity,
}

/// Validate an input value against optional length and simple pattern constraints.
///
/// The `pattern` is matched as a literal substring (no regex).
pub fn validate_input(
    value: &str,
    min_length: Option<usize>,
    max_length: Option<usize>,
    pattern: Option<&str>,
) -> Option<InputBoxValidation> {
    if let Some(min) = min_length {
        if value.len() < min {
            return Some(InputBoxValidation {
                message: format!("Input must be at least {} characters", min),
                severity: InputValidationSeverity::Error,
            });
        }
    }
    if let Some(max) = max_length {
        if value.len() > max {
            return Some(InputBoxValidation {
                message: format!("Input must be at most {} characters", max),
                severity: InputValidationSeverity::Error,
            });
        }
    }
    if let Some(pat) = pattern {
        if !value.contains(pat) {
            return Some(InputBoxValidation {
                message: format!("Input must contain \"{}\"", pat),
                severity: InputValidationSeverity::Warning,
            });
        }
    }
    None
}

// ── Item Utilities ──

/// Filter items by case-insensitive substring match on label and description.
pub fn filter_items<'a>(items: &'a [QuickPickItem], query: &str) -> Vec<&'a QuickPickItem> {
    let q = query.to_lowercase();
    items
        .iter()
        .filter(|item| {
            let label_match = item.label.to_lowercase().contains(&q);
            let desc_match = item
                .description
                .as_deref()
                .map(|d| d.to_lowercase().contains(&q))
                .unwrap_or(false);
            label_match || desc_match
        })
        .collect()
}

/// Return all items where `picked` is `true`.
pub fn get_picked_items(items: &[QuickPickItem]) -> Vec<&QuickPickItem> {
    items.iter().filter(|i| i.picked).collect()
}

/// Set the `picked` flag on every item.
pub fn set_all_picked(items: &mut [QuickPickItem], picked: bool) {
    for item in items.iter_mut() {
        item.picked = picked;
    }
}

/// Sort items alphabetically by label.
pub fn sort_items_alphabetically(items: &mut [QuickPickItem]) {
    items.sort_by(|a, b| a.label.to_lowercase().cmp(&b.label.to_lowercase()));
}

/// Sort items so that recently-used labels appear first (order preserved),
/// followed by the remaining items in their original order.
pub fn sort_items_by_recently_used(items: &mut [QuickPickItem], recent: &[String]) {
    items.sort_by(|a, b| {
        let a_idx = recent.iter().position(|r| r == &a.label);
        let b_idx = recent.iter().position(|r| r == &b.label);
        match (a_idx, b_idx) {
            (Some(ai), Some(bi)) => ai.cmp(&bi),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        }
    });
}

// ── QuickPickSession ──

/// Tracks an active quick-pick session with query state and selection.
pub struct QuickPickSession {
    items: Vec<QuickPickItem>,
    query: String,
    selected_indices: Vec<usize>,
}

impl QuickPickSession {
    pub fn new(items: Vec<QuickPickItem>) -> Self {
        Self {
            items,
            query: String::new(),
            selected_indices: Vec::new(),
        }
    }

    pub fn update_query(&mut self, query: &str) {
        self.query = query.to_string();
        self.selected_indices.clear();
    }

    pub fn get_filtered(&self) -> Vec<&QuickPickItem> {
        if self.query.is_empty() {
            self.items.iter().collect()
        } else {
            filter_items(&self.items, &self.query)
        }
    }

    pub fn select_index(&mut self, index: usize) {
        if !self.selected_indices.contains(&index) {
            self.selected_indices.push(index);
        }
    }

    pub fn selected_indices(&self) -> &[usize] {
        &self.selected_indices
    }

    pub fn query(&self) -> &str {
        &self.query
    }
}

// ── Bridge ──

pub struct QuickOpenBridge {
    is_visible: bool,
    current_items: Vec<QuickPickItem>,
}

impl QuickOpenBridge {
    pub fn new() -> Self {
        Self {
            is_visible: false,
            current_items: Vec::new(),
        }
    }

    pub fn show(&mut self, items: Vec<QuickPickItem>) {
        self.current_items = items;
        self.is_visible = true;
    }

    pub fn hide(&mut self) {
        self.is_visible = false;
        self.current_items.clear();
    }

    pub fn is_visible(&self) -> bool {
        self.is_visible
    }

    pub fn item_count(&self) -> usize {
        self.current_items.len()
    }

    pub fn select_item(&self, index: usize) -> Option<&QuickPickItem> {
        self.current_items.get(index)
    }

    pub fn handle_message(&mut self, msg: &QuickOpenMessage) -> serde_json::Value {
        match msg {
            QuickOpenMessage::ShowQuickPick { items, .. } => {
                self.show(items.clone());
                serde_json::json!({"shown": true, "count": items.len()})
            }
            QuickOpenMessage::ShowInputBox { options } => {
                self.is_visible = true;
                serde_json::json!({"shown": true, "prompt": options.prompt})
            }
            QuickOpenMessage::Hide => {
                self.hide();
                serde_json::json!({"hidden": true})
            }
            QuickOpenMessage::SetItems { items } => {
                self.current_items = items.clone();
                serde_json::json!({"updated": items.len()})
            }
            QuickOpenMessage::ItemSelected { index } => {
                let label = self.select_item(*index).map(|i| i.label.clone());
                serde_json::json!({"selected": label})
            }
        }
    }
}

impl Default for QuickOpenBridge {
    fn default() -> Self {
        Self::new()
    }
}

/// Initialize the quickopen extension API bridge.
pub fn register() {
    // Registration will connect RPC handlers when extension host starts
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_item(label: &str) -> QuickPickItem {
        QuickPickItem {
            label: label.into(),
            description: None,
            detail: None,
            picked: false,
            always_show: false,
        }
    }

    #[test]
    fn proxy_id() {
        assert!(!PROXY_ID.is_empty());
    }

    #[test]
    fn message_roundtrip() {
        let msg = QuickOpenMessage::ShowInputBox {
            options: InputBoxOptions {
                prompt: Some("Enter name".into()),
                placeholder: None,
                value: None,
                password: false,
            },
        };
        let json = serde_json::to_string(&msg).unwrap();
        let back: QuickOpenMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(msg, back);
    }

    #[test]
    fn quick_pick_item_serialization() {
        let item = test_item("Open File");
        let json = serde_json::to_string(&item).unwrap();
        let back: QuickPickItem = serde_json::from_str(&json).unwrap();
        assert_eq!(item, back);
    }

    #[test]
    fn bridge_show_and_hide() {
        let mut bridge = QuickOpenBridge::new();
        bridge.show(vec![test_item("a"), test_item("b")]);
        assert!(bridge.is_visible());
        bridge.hide();
        assert!(!bridge.is_visible());
    }

    #[test]
    fn bridge_select_item() {
        let mut bridge = QuickOpenBridge::new();
        bridge.show(vec![test_item("first"), test_item("second")]);
        assert_eq!(bridge.select_item(0).unwrap().label, "first");
        assert_eq!(bridge.select_item(1).unwrap().label, "second");
        assert!(bridge.select_item(5).is_none());
    }

    #[test]
    fn bridge_handle_hide() {
        let mut bridge = QuickOpenBridge::new();
        bridge.show(vec![test_item("a")]);
        bridge.handle_message(&QuickOpenMessage::Hide);
        assert!(!bridge.is_visible());
    }

    #[test]
    fn bridge_item_count() {
        let mut bridge = QuickOpenBridge::new();
        assert_eq!(bridge.item_count(), 0);
        bridge.show(vec![test_item("a"), test_item("b"), test_item("c")]);
        assert_eq!(bridge.item_count(), 3);
        bridge.hide();
        assert_eq!(bridge.item_count(), 0);
    }

    #[test]
    fn filter_items_case_insensitive() {
        let items = vec![
            test_item("Open File"),
            QuickPickItem {
                label: "Save".into(),
                description: Some("Save current file".into()),
                detail: None,
                picked: false,
                always_show: false,
            },
            test_item("Close Editor"),
        ];
        let result = filter_items(&items, "file");
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].label, "Open File");
        assert_eq!(result[1].label, "Save");
    }

    #[test]
    fn filter_items_empty_query_returns_all() {
        let items = vec![test_item("a"), test_item("b")];
        assert_eq!(filter_items(&items, "").len(), 2);
    }

    #[test]
    fn get_and_set_picked() {
        let mut items = vec![test_item("a"), test_item("b"), test_item("c")];
        assert!(get_picked_items(&items).is_empty());
        set_all_picked(&mut items, true);
        assert_eq!(get_picked_items(&items).len(), 3);
        items[1].picked = false;
        let picked = get_picked_items(&items);
        assert_eq!(picked.len(), 2);
        assert_eq!(picked[0].label, "a");
        assert_eq!(picked[1].label, "c");
    }

    #[test]
    fn sort_alphabetically() {
        let mut items = vec![test_item("Zebra"), test_item("apple"), test_item("Mango")];
        sort_items_alphabetically(&mut items);
        assert_eq!(items[0].label, "apple");
        assert_eq!(items[1].label, "Mango");
        assert_eq!(items[2].label, "Zebra");
    }

    #[test]
    fn sort_by_recently_used() {
        let mut items = vec![test_item("c"), test_item("a"), test_item("b")];
        let recent = vec!["b".to_string(), "a".to_string()];
        sort_items_by_recently_used(&mut items, &recent);
        assert_eq!(items[0].label, "b");
        assert_eq!(items[1].label, "a");
        assert_eq!(items[2].label, "c");
    }

    #[test]
    fn quick_pick_separator_and_entry() {
        let entry_item = QuickPickEntry::Item(test_item("Open"));
        let entry_sep = QuickPickEntry::Separator(QuickPickSeparator {
            label: "Recent".into(),
        });
        let json_item = serde_json::to_string(&entry_item).unwrap();
        let json_sep = serde_json::to_string(&entry_sep).unwrap();
        assert_eq!(
            serde_json::from_str::<QuickPickEntry>(&json_item).unwrap(),
            entry_item
        );
        assert_eq!(
            serde_json::from_str::<QuickPickEntry>(&json_sep).unwrap(),
            entry_sep
        );
    }

    #[test]
    fn validate_input_min_length() {
        let result = validate_input("ab", Some(3), None, None);
        assert!(result.is_some());
        assert_eq!(result.unwrap().severity, InputValidationSeverity::Error);
        assert!(validate_input("abc", Some(3), None, None).is_none());
    }

    #[test]
    fn validate_input_max_length() {
        let result = validate_input("toolong", None, Some(4), None);
        assert!(result.is_some());
        assert!(validate_input("ok", None, Some(4), None).is_none());
    }

    #[test]
    fn validate_input_pattern() {
        let result = validate_input("hello", None, None, Some("world"));
        assert!(result.is_some());
        assert_eq!(result.unwrap().severity, InputValidationSeverity::Warning);
        assert!(validate_input("hello world", None, None, Some("world")).is_none());
    }

    #[test]
    fn quick_pick_session_workflow() {
        let items = vec![
            test_item("Open File"),
            test_item("Close Tab"),
            test_item("Open Terminal"),
        ];
        let mut session = QuickPickSession::new(items);
        assert_eq!(session.get_filtered().len(), 3);
        assert_eq!(session.query(), "");

        session.update_query("open");
        assert_eq!(session.get_filtered().len(), 2);

        session.select_index(0);
        session.select_index(1);
        session.select_index(0); // duplicate ignored
        assert_eq!(session.selected_indices().len(), 2);

        session.update_query("terminal");
        assert_eq!(session.get_filtered().len(), 1);
        assert!(session.selected_indices().is_empty());
    }
}
