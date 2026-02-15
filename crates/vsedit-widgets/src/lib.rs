//! Base widget system for vsedit TUI.
use vsedit_tui::{Frame, Rect};

/// Trait that all vsedit widgets implement.
pub trait Widget {
    fn render(&self, frame: &mut Frame, area: Rect);
}

/// A widget that can receive focus.
pub trait Focusable: Widget {
    fn is_focused(&self) -> bool;
    fn set_focused(&mut self, focused: bool);
}

/// Focus direction for navigation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusDirection {
    Next,
    Previous,
    Up,
    Down,
    Left,
    Right,
}

/// Focus manager that tracks which widget has focus.
pub struct FocusManager {
    focused_id: Option<String>,
    focus_order: Vec<String>,
}

impl FocusManager {
    pub fn new() -> Self {
        Self {
            focused_id: None,
            focus_order: Vec::new(),
        }
    }

    pub fn register(&mut self, id: impl Into<String>) {
        let id = id.into();
        if !self.focus_order.contains(&id) {
            self.focus_order.push(id);
        }
    }

    pub fn set_focus(&mut self, id: &str) {
        if self.focus_order.iter().any(|s| s == id) {
            self.focused_id = Some(id.to_string());
        }
    }

    pub fn focused(&self) -> Option<&str> {
        self.focused_id.as_deref()
    }

    pub fn move_focus(&mut self, direction: FocusDirection) {
        match direction {
            FocusDirection::Next | FocusDirection::Down | FocusDirection::Right => {
                self.focus_next();
            }
            FocusDirection::Previous | FocusDirection::Up | FocusDirection::Left => {
                self.focus_previous();
            }
        }
    }

    pub fn focus_next(&mut self) {
        if self.focus_order.is_empty() {
            return;
        }
        let next = match &self.focused_id {
            Some(current) => {
                let idx = self
                    .focus_order
                    .iter()
                    .position(|s| s == current)
                    .unwrap_or(0);
                (idx + 1) % self.focus_order.len()
            }
            None => 0,
        };
        self.focused_id = Some(self.focus_order[next].clone());
    }

    pub fn focus_previous(&mut self) {
        if self.focus_order.is_empty() {
            return;
        }
        let prev = match &self.focused_id {
            Some(current) => {
                let idx = self
                    .focus_order
                    .iter()
                    .position(|s| s == current)
                    .unwrap_or(0);
                if idx == 0 {
                    self.focus_order.len() - 1
                } else {
                    idx - 1
                }
            }
            None => self.focus_order.len() - 1,
        };
        self.focused_id = Some(self.focus_order[prev].clone());
    }
}

impl Default for FocusManager {
    fn default() -> Self {
        Self::new()
    }
}

/// A bordered panel widget.
pub struct Panel {
    pub title: String,
    pub border_style: vsedit_styles::Style,
}

impl Panel {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            border_style: vsedit_styles::Style::default()
                .fg(vsedit_styles::ThemeDefaults::border()),
        }
    }

    pub fn with_style(mut self, style: vsedit_styles::Style) -> Self {
        self.border_style = style;
        self
    }
}

impl Widget for Panel {
    fn render(&self, frame: &mut Frame, area: Rect) {
        use ratatui::widgets::{Block, Borders};

        let block = Block::default()
            .title(self.title.as_str())
            .borders(Borders::ALL)
            .border_style(self.border_style);
        frame.render_widget(block, area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn focus_manager_empty() {
        let fm = FocusManager::new();
        assert!(fm.focused().is_none());
    }

    #[test]
    fn focus_manager_register_and_set() {
        let mut fm = FocusManager::new();
        fm.register("editor");
        fm.register("sidebar");
        fm.set_focus("editor");
        assert_eq!(fm.focused(), Some("editor"));
    }

    #[test]
    fn focus_manager_cycle_next() {
        let mut fm = FocusManager::new();
        fm.register("a");
        fm.register("b");
        fm.register("c");
        fm.focus_next();
        assert_eq!(fm.focused(), Some("a"));
        fm.focus_next();
        assert_eq!(fm.focused(), Some("b"));
        fm.focus_next();
        assert_eq!(fm.focused(), Some("c"));
        fm.focus_next();
        assert_eq!(fm.focused(), Some("a"));
    }

    #[test]
    fn focus_manager_cycle_previous() {
        let mut fm = FocusManager::new();
        fm.register("a");
        fm.register("b");
        fm.register("c");
        fm.focus_previous();
        assert_eq!(fm.focused(), Some("c"));
        fm.focus_previous();
        assert_eq!(fm.focused(), Some("b"));
        fm.focus_previous();
        assert_eq!(fm.focused(), Some("a"));
        fm.focus_previous();
        assert_eq!(fm.focused(), Some("c"));
    }

    #[test]
    fn focus_manager_move_direction() {
        let mut fm = FocusManager::new();
        fm.register("x");
        fm.register("y");
        fm.move_focus(FocusDirection::Next);
        assert_eq!(fm.focused(), Some("x"));
        fm.move_focus(FocusDirection::Next);
        assert_eq!(fm.focused(), Some("y"));
        fm.move_focus(FocusDirection::Previous);
        assert_eq!(fm.focused(), Some("x"));
    }

    #[test]
    fn focus_manager_no_duplicate_register() {
        let mut fm = FocusManager::new();
        fm.register("a");
        fm.register("a");
        assert_eq!(fm.focus_order.len(), 1);
    }

    #[test]
    fn panel_construction() {
        let panel = Panel::new("Test Panel");
        assert_eq!(panel.title, "Test Panel");
    }

    #[test]
    fn panel_with_style() {
        let style = vsedit_styles::Style::default().fg(vsedit_styles::Color::Red);
        let panel = Panel::new("Styled").with_style(style);
        assert_eq!(panel.border_style, style);
    }
}
