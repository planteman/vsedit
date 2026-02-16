//! Global suggest configuration.

#[derive(Debug, Clone, PartialEq)]
pub enum SuggestWidgetState {
    Hidden,
    Loading,
    Visible,
    Details,
}

#[derive(Debug, Clone, PartialEq)]
pub enum InsertMode {
    Insert,
    Replace,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SortMode {
    InlineFirst,
    SnippetsFirst,
    None,
}

#[derive(Debug, Clone)]
pub struct SuggestConfig {
    pub insert_mode: InsertMode,
    pub filter_graceful: bool,
    pub snippets_prevent_quick_suggestions: bool,
    pub local_sorting: SortMode,
    pub show_icons: bool,
    pub max_visible_suggestions: u32,
    pub status_bar_visible: bool,
}

impl Default for SuggestConfig {
    fn default() -> Self {
        Self {
            insert_mode: InsertMode::Insert,
            filter_graceful: true,
            snippets_prevent_quick_suggestions: false,
            local_sorting: SortMode::InlineFirst,
            show_icons: true,
            max_visible_suggestions: 12,
            status_bar_visible: true,
        }
    }
}

/// Widget for suggest/autocomplete functionality.
pub struct SuggestWidget {
    state: SuggestWidgetState,
    selected_index: Option<usize>,
    item_count: usize,
}

impl SuggestWidget {
    pub fn new() -> Self {
        Self {
            state: SuggestWidgetState::Hidden,
            selected_index: None,
            item_count: 0,
        }
    }

    pub fn show(&mut self, count: usize) {
        self.item_count = count;
        self.selected_index = if count > 0 { Some(0) } else { None };
        self.state = SuggestWidgetState::Visible;
    }

    pub fn hide(&mut self) {
        self.state = SuggestWidgetState::Hidden;
        self.selected_index = None;
        self.item_count = 0;
    }

    pub fn select(&mut self, index: usize) {
        if index < self.item_count {
            self.selected_index = Some(index);
        }
    }

    pub fn select_next(&mut self) {
        if let Some(idx) = self.selected_index {
            if idx + 1 < self.item_count {
                self.selected_index = Some(idx + 1);
            }
        }
    }

    pub fn select_previous(&mut self) {
        if let Some(idx) = self.selected_index {
            if idx > 0 {
                self.selected_index = Some(idx - 1);
            }
        }
    }

    pub fn get_state(&self) -> &SuggestWidgetState {
        &self.state
    }

    pub fn is_visible(&self) -> bool {
        matches!(self.state, SuggestWidgetState::Visible | SuggestWidgetState::Details)
    }
}

impl Default for SuggestWidget {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn show_and_hide() {
        let mut w = SuggestWidget::new();
        assert!(!w.is_visible());
        assert_eq!(*w.get_state(), SuggestWidgetState::Hidden);
        w.show(5);
        assert!(w.is_visible());
        assert_eq!(w.selected_index, Some(0));
        w.hide();
        assert!(!w.is_visible());
    }

    #[test]
    fn navigation() {
        let mut w = SuggestWidget::new();
        w.show(3);
        assert_eq!(w.selected_index, Some(0));
        w.select_next();
        assert_eq!(w.selected_index, Some(1));
        w.select_next();
        assert_eq!(w.selected_index, Some(2));
        w.select_next(); // should not go past end
        assert_eq!(w.selected_index, Some(2));
        w.select_previous();
        assert_eq!(w.selected_index, Some(1));
    }

    #[test]
    fn select_index() {
        let mut w = SuggestWidget::new();
        w.show(5);
        w.select(3);
        assert_eq!(w.selected_index, Some(3));
        w.select(10); // out of range, no change
        assert_eq!(w.selected_index, Some(3));
    }

    #[test]
    fn default_config() {
        let cfg = SuggestConfig::default();
        assert_eq!(cfg.insert_mode, InsertMode::Insert);
        assert!(cfg.filter_graceful);
        assert_eq!(cfg.max_visible_suggestions, 12);
    }
}
