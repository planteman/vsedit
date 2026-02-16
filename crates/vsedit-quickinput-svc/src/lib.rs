//! Quick input model service.

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// An item in a quick pick list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuickPickItem {
    pub label: String,
    pub description: Option<String>,
    pub detail: Option<String>,
    pub picked: bool,
    pub always_show: bool,
}

/// Options controlling quick pick behaviour.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuickPickOptions {
    pub title: Option<String>,
    pub placeholder: Option<String>,
    pub can_pick_many: bool,
    pub match_on_description: bool,
}

impl Default for QuickPickOptions {
    fn default() -> Self {
        Self {
            title: None,
            placeholder: None,
            can_pick_many: false,
            match_on_description: false,
        }
    }
}

/// The result of a quick pick interaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuickPickResult {
    pub items: Vec<QuickPickItem>,
    pub cancelled: bool,
}

/// Validation result for an input box value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputBoxValidation {
    Ok,
    Error(String),
    Warning(String),
}

// ---------------------------------------------------------------------------
// Trait
// ---------------------------------------------------------------------------

/// Service interface for quick input UI.
pub trait QuickInputService {
    fn show_quick_pick(
        &self,
        items: Vec<QuickPickItem>,
        options: QuickPickOptions,
    ) -> QuickPickResult;

    fn show_input_box(&self, prompt: &str, value: &str) -> Option<String>;
}

// ---------------------------------------------------------------------------
// Filtering helper
// ---------------------------------------------------------------------------

/// Returns the indices of items whose label contains `query` (case-insensitive).
pub fn filter_quick_pick_items(items: &[QuickPickItem], query: &str) -> Vec<usize> {
    if query.is_empty() {
        return (0..items.len()).collect();
    }
    let query_lower = query.to_lowercase();
    items
        .iter()
        .enumerate()
        .filter(|(_, item)| item.label.to_lowercase().contains(&query_lower))
        .map(|(i, _)| i)
        .collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn item(label: &str) -> QuickPickItem {
        QuickPickItem {
            label: label.into(),
            description: None,
            detail: None,
            picked: false,
            always_show: false,
        }
    }

    #[test]
    fn filter_matches_substring() {
        let items = vec![item("Open File"), item("Close File"), item("Run Task")];
        let result = filter_quick_pick_items(&items, "file");
        assert_eq!(result, vec![0, 1]);
    }

    #[test]
    fn filter_empty_query_returns_all() {
        let items = vec![item("A"), item("B")];
        let result = filter_quick_pick_items(&items, "");
        assert_eq!(result, vec![0, 1]);
    }

    #[test]
    fn filter_no_match_returns_empty() {
        let items = vec![item("Open File")];
        let result = filter_quick_pick_items(&items, "zzz");
        assert!(result.is_empty());
    }

    #[test]
    fn filter_case_insensitive() {
        let items = vec![item("FooBar")];
        let result = filter_quick_pick_items(&items, "FOOB");
        assert_eq!(result, vec![0]);
    }

    #[test]
    fn quick_pick_result_cancelled() {
        let r = QuickPickResult {
            items: vec![],
            cancelled: true,
        };
        assert!(r.cancelled);
    }
}
