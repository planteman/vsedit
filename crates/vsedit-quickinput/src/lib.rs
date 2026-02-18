//! Quick pick / command palette model.
//!
//! Provides the core data types, fuzzy-matching logic, input box model,
//! command palette integration, Go-to-Line support, and rendering helpers
//! for VS Code-style quick-input UIs.

use std::collections::{HashMap, VecDeque};
use std::fmt;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Widget;
use vsedit_events::{Emitter, Event};

// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------

/// The kind of quick-pick item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuickPickItemKind {
    Default,
    Separator,
}

/// A single item in a quick pick list.
#[derive(Debug, Clone)]
pub struct QuickPickItem {
    pub label: String,
    pub description: Option<String>,
    pub detail: Option<String>,
    pub icon: Option<String>,
    pub kind: QuickPickItemKind,
    pub picked: bool,
    /// Always show even when not matching the current filter.
    pub always_show: bool,
    /// Keybinding string to display next to the item.
    pub keybinding: Option<String>,
}

impl QuickPickItem {
    /// Create a separator item with the given label.
    pub fn separator(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            description: None,
            detail: None,
            icon: None,
            kind: QuickPickItemKind::Separator,
            picked: false,
            always_show: true,
            keybinding: None,
        }
    }
}

/// Options that configure a quick pick session.
#[derive(Debug, Clone, Default)]
pub struct QuickPickOptions {
    pub placeholder: Option<String>,
    pub title: Option<String>,
    pub can_select_many: bool,
    pub match_on_description: bool,
    pub match_on_detail: bool,
}

/// Options that configure a text input session.
#[derive(Debug, Clone, Default)]
pub struct QuickInputOptions {
    pub prompt: Option<String>,
    pub title: Option<String>,
    pub value: Option<String>,
    pub placeholder: Option<String>,
    pub password: bool,
}

// ---------------------------------------------------------------------------
// Input Box model
// ---------------------------------------------------------------------------

/// Options for an input box.
#[derive(Debug, Clone, Default)]
pub struct InputBoxOptions {
    pub title: Option<String>,
    pub placeholder: Option<String>,
    pub value: String,
    pub prompt: Option<String>,
    pub password: bool,
}

/// State for an input box.
#[derive(Debug, Clone)]
pub struct InputBoxState {
    pub value: String,
    pub cursor_pos: usize,
    pub is_active: bool,
    pub validation_message: Option<String>,
}

impl InputBoxState {
    pub fn new() -> Self {
        Self {
            value: String::new(),
            cursor_pos: 0,
            is_active: false,
            validation_message: None,
        }
    }

    pub fn with_value(mut self, value: impl Into<String>) -> Self {
        self.value = value.into();
        self.cursor_pos = self.value.len();
        self
    }

    /// Insert a character at the cursor position.
    pub fn insert_char(&mut self, c: char) {
        self.value.insert(self.cursor_pos, c);
        self.cursor_pos += c.len_utf8();
    }

    /// Delete the character before the cursor (backspace).
    pub fn backspace(&mut self) {
        if self.cursor_pos > 0 {
            let prev = self.value[..self.cursor_pos]
                .chars()
                .last()
                .map(|c| c.len_utf8())
                .unwrap_or(0);
            self.cursor_pos -= prev;
            self.value.remove(self.cursor_pos);
        }
    }

    /// Delete the character at the cursor (delete key).
    pub fn delete(&mut self) {
        if self.cursor_pos < self.value.len() {
            self.value.remove(self.cursor_pos);
        }
    }

    /// Move cursor left by one character.
    pub fn move_left(&mut self) {
        if self.cursor_pos > 0 {
            let prev = self.value[..self.cursor_pos]
                .chars()
                .last()
                .map(|c| c.len_utf8())
                .unwrap_or(0);
            self.cursor_pos -= prev;
        }
    }

    /// Move cursor right by one character.
    pub fn move_right(&mut self) {
        if self.cursor_pos < self.value.len() {
            let next = self.value[self.cursor_pos..]
                .chars()
                .next()
                .map(|c| c.len_utf8())
                .unwrap_or(0);
            self.cursor_pos += next;
        }
    }

    /// Move cursor to the start of the value.
    pub fn move_home(&mut self) {
        self.cursor_pos = 0;
    }

    /// Move cursor to the end of the value.
    pub fn move_end(&mut self) {
        self.cursor_pos = self.value.len();
    }

    /// Set the entire value, moving cursor to end.
    pub fn set_value(&mut self, value: impl Into<String>) {
        self.value = value.into();
        self.cursor_pos = self.value.len();
    }

    /// Validate the current value using a validation function.
    pub fn validate(&mut self, validator: &dyn Fn(&str) -> Option<String>) {
        self.validation_message = validator(&self.value);
    }
}

impl Default for InputBoxState {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// QuickPick state
// ---------------------------------------------------------------------------

/// Full state for a quick pick session (model + UI state).
#[derive(Debug, Clone)]
pub struct QuickPickState {
    pub items: Vec<QuickPickItem>,
    pub filtered_items: Vec<FilteredItem>,
    pub selected_idx: usize,
    pub input_text: String,
    pub is_active: bool,
    pub scroll_offset: usize,
}

impl QuickPickState {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            filtered_items: Vec::new(),
            selected_idx: 0,
            input_text: String::new(),
            is_active: false,
            scroll_offset: 0,
        }
    }

    /// The maximum number of visible items in the pick list.
    pub const MAX_VISIBLE_ITEMS: usize = 10;

    /// Ensure the selected item is visible within the scroll window.
    pub fn ensure_visible(&mut self) {
        if self.selected_idx < self.scroll_offset {
            self.scroll_offset = self.selected_idx;
        } else if self.selected_idx >= self.scroll_offset + Self::MAX_VISIBLE_ITEMS {
            self.scroll_offset = self.selected_idx + 1 - Self::MAX_VISIBLE_ITEMS;
        }
    }
}

impl Default for QuickPickState {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Fuzzy matching
// ---------------------------------------------------------------------------

/// Result of a successful fuzzy match.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FuzzyMatch {
    /// Higher is better.
    pub score: i32,
    /// Character positions in the haystack that matched the pattern.
    pub positions: Vec<usize>,
}

/// Perform a fuzzy match of `pattern` against `text`.
///
/// Returns `Some(FuzzyMatch)` when every character in `pattern` can be found
/// (in order) inside `text`, or `None` otherwise.  Scoring rewards:
///
/// * Consecutive character runs  (+5 each)
/// * Matches at word boundaries   (+10)
/// * Exact case matches           (+1)
pub fn fuzzy_match(pattern: &str, text: &str) -> Option<FuzzyMatch> {
    if pattern.is_empty() {
        return Some(FuzzyMatch {
            score: 0,
            positions: Vec::new(),
        });
    }

    let pattern_lower: Vec<char> = pattern.chars().map(|c| c.to_ascii_lowercase()).collect();
    let text_chars: Vec<char> = text.chars().collect();
    let text_lower: Vec<char> = text_chars.iter().map(|c| c.to_ascii_lowercase()).collect();

    let mut positions = Vec::with_capacity(pattern_lower.len());
    let mut score: i32 = 0;
    let mut text_idx = 0;

    for &pc in &pattern_lower {
        let mut found = false;
        while text_idx < text_lower.len() {
            if text_lower[text_idx] == pc {
                // Consecutive bonus
                if let Some(&prev) = positions.last() {
                    if text_idx == prev + 1 {
                        score += 5;
                    }
                }

                // Word-boundary bonus
                if text_idx == 0 || !text_chars[text_idx - 1].is_alphanumeric() {
                    score += 10;
                }

                // Case-match bonus
                let orig_pattern_char = pattern.chars().nth(positions.len()).unwrap();
                if text_chars[text_idx] == orig_pattern_char {
                    score += 1;
                }

                positions.push(text_idx);
                text_idx += 1;
                found = true;
                break;
            }
            text_idx += 1;
        }
        if !found {
            return None;
        }
    }

    Some(FuzzyMatch { score, positions })
}

// ---------------------------------------------------------------------------
// Filtered item
// ---------------------------------------------------------------------------

/// A quick-pick item that passed the current filter, with match metadata.
#[derive(Debug, Clone)]
pub struct FilteredItem {
    /// Index into the original items list.
    pub original_index: usize,
    pub score: i32,
    pub highlight_positions: Vec<usize>,
}

// ---------------------------------------------------------------------------
// QuickPickService
// ---------------------------------------------------------------------------

/// Manages a quick-pick list with fuzzy filtering and selection.
pub struct QuickPickService {
    items: Vec<QuickPickItem>,
    filter_text: String,
    filtered_items: Vec<FilteredItem>,
    selected_index: usize,
    on_did_accept: Emitter<Vec<usize>>,
    on_did_change_value: Emitter<String>,
}

impl QuickPickService {
    /// Create a new, empty quick-pick service.
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            filter_text: String::new(),
            filtered_items: Vec::new(),
            selected_index: 0,
            on_did_accept: Emitter::new(),
            on_did_change_value: Emitter::new(),
        }
    }

    /// Replace the current items and re-apply the active filter.
    pub fn set_items(&mut self, items: Vec<QuickPickItem>) {
        self.items = items;
        self.apply_filter();
    }

    /// Update the filter text and recompute the filtered list.
    pub fn set_filter(&mut self, text: String) {
        self.filter_text = text.clone();
        self.apply_filter();
        self.on_did_change_value.fire(&text);
    }

    /// Return the current filtered items, sorted by score (descending).
    pub fn get_filtered_items(&self) -> &[FilteredItem] {
        &self.filtered_items
    }

    /// Move selection to the next item (wraps around).
    pub fn select_next(&mut self) {
        if !self.filtered_items.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.filtered_items.len();
        }
    }

    /// Move selection to the previous item (wraps around).
    pub fn select_previous(&mut self) {
        if !self.filtered_items.is_empty() {
            if self.selected_index == 0 {
                self.selected_index = self.filtered_items.len() - 1;
            } else {
                self.selected_index -= 1;
            }
        }
    }

    /// Accept the current selection, firing the `on_did_accept` event with
    /// the selected original item index.
    pub fn accept(&self) {
        if let Some(item) = self.filtered_items.get(self.selected_index) {
            self.on_did_accept.fire(&vec![item.original_index]);
        }
    }

    /// Return the currently selected index into `filtered_items`.
    pub fn get_selected_index(&self) -> usize {
        self.selected_index
    }

    /// Subscribe to the accept event.
    pub fn on_did_accept(&self) -> Event<Vec<usize>> {
        self.on_did_accept.event()
    }

    /// Subscribe to filter-text changes.
    pub fn on_did_change_value(&self) -> Event<String> {
        self.on_did_change_value.event()
    }

    /// Get the item at the given original index.
    pub fn get_item(&self, index: usize) -> Option<&QuickPickItem> {
        self.items.get(index)
    }

    /// Return the current filter text.
    pub fn filter_text(&self) -> &str {
        &self.filter_text
    }

    // -- internals ----------------------------------------------------------

    fn apply_filter(&mut self) {
        self.filtered_items.clear();

        for (idx, item) in self.items.iter().enumerate() {
            // Separators always show
            if item.kind == QuickPickItemKind::Separator {
                self.filtered_items.push(FilteredItem {
                    original_index: idx,
                    score: i32::MAX,
                    highlight_positions: Vec::new(),
                });
                continue;
            }

            if self.filter_text.is_empty() {
                self.filtered_items.push(FilteredItem {
                    original_index: idx,
                    score: 0,
                    highlight_positions: Vec::new(),
                });
                continue;
            }

            if item.always_show {
                let m = fuzzy_match(&self.filter_text, &item.label);
                self.filtered_items.push(FilteredItem {
                    original_index: idx,
                    score: m.as_ref().map_or(0, |m| m.score),
                    highlight_positions: m.map_or_else(Vec::new, |m| m.positions),
                });
                continue;
            }

            if let Some(m) = fuzzy_match(&self.filter_text, &item.label) {
                self.filtered_items.push(FilteredItem {
                    original_index: idx,
                    score: m.score,
                    highlight_positions: m.positions,
                });
            }
        }

        // Stable sort by score descending so equal-score items keep insertion order.
        self.filtered_items.sort_by(|a, b| b.score.cmp(&a.score));

        // Reset selection to top.
        self.selected_index = 0;
    }

    /// Returns true if items is empty.
    pub fn is_items_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Get the first item, if any.
    pub fn first_item(&self) -> Option<&QuickPickItem> {
        self.items.first()
    }

    /// Get the last item, if any.
    pub fn last_item(&self) -> Option<&QuickPickItem> {
        self.items.last()
    }

    /// Retain only items matching the predicate.
    pub fn retain_items(&mut self, f: impl Fn(&QuickPickItem) -> bool) {
        self.items.retain(|item| f(item));
    }

    /// Returns true if filtered_items is empty.
    pub fn is_filtered_items_empty(&self) -> bool {
        self.filtered_items.is_empty()
    }

    /// Get the first filtered_item, if any.
    pub fn first_filtered_item(&self) -> Option<&FilteredItem> {
        self.filtered_items.first()
    }

    /// Get the last filtered_item, if any.
    pub fn last_filtered_item(&self) -> Option<&FilteredItem> {
        self.filtered_items.last()
    }

    /// Retain only filtered_items matching the predicate.
    pub fn retain_filtered_items(&mut self, f: impl Fn(&FilteredItem) -> bool) {
        self.filtered_items.retain(|item| f(item));
    }
}

impl Default for QuickPickService {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// CommandPaletteService
// ---------------------------------------------------------------------------

/// A command entry for the command palette.
#[derive(Debug, Clone)]
pub struct CommandEntry {
    pub id: String,
    pub label: String,
    pub keybinding: Option<String>,
    pub category: Option<String>,
}

/// Manages the command palette state, including recently-used tracking.
pub struct CommandPaletteService {
    commands: Vec<CommandEntry>,
    recent: VecDeque<String>,
    pick: QuickPickService,
    is_active: bool,
    max_recent: usize,
}

impl CommandPaletteService {
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
            recent: VecDeque::new(),
            pick: QuickPickService::new(),
            is_active: false,
            max_recent: 10,
        }
    }

    /// Register all commands and rebuild the quick pick items.
    pub fn set_commands(&mut self, commands: Vec<CommandEntry>) {
        self.commands = commands;
        self.rebuild_items();
    }

    /// Open the command palette.
    pub fn open(&mut self) {
        self.is_active = true;
        self.pick.set_filter(String::new());
        self.rebuild_items();
    }

    /// Close the command palette.
    pub fn close(&mut self) {
        self.is_active = false;
    }

    /// Returns whether the palette is currently active.
    pub fn is_active(&self) -> bool {
        self.is_active
    }

    /// Update the filter text.
    pub fn set_filter(&mut self, text: String) {
        self.pick.set_filter(text);
    }

    /// Move selection down.
    pub fn select_next(&mut self) {
        self.pick.select_next();
    }

    /// Move selection up.
    pub fn select_previous(&mut self) {
        self.pick.select_previous();
    }

    /// Accept the current selection and return the command ID.
    pub fn accept(&mut self) -> Option<String> {
        let idx = self.pick.get_selected_index();
        if let Some(fi) = self.pick.get_filtered_items().get(idx) {
            let item = &self.pick.items[fi.original_index];
            if item.kind == QuickPickItemKind::Separator {
                return None;
            }
            if let Some(cmd) = self.commands.iter().find(|c| c.label == item.label) {
                let id = cmd.id.clone();
                self.record_recent(&id);
                return Some(id);
            }
        }
        None
    }

    /// Get the underlying quick pick service for rendering.
    pub fn pick_service(&self) -> &QuickPickService {
        &self.pick
    }

    /// Get the list of recently used command IDs.
    pub fn recent_commands(&self) -> &VecDeque<String> {
        &self.recent
    }

    fn record_recent(&mut self, id: &str) {
        self.recent.retain(|r| r != id);
        self.recent.push_front(id.to_string());
        if self.recent.len() > self.max_recent {
            self.recent.pop_back();
        }
    }

    fn rebuild_items(&mut self) {
        let mut items = Vec::new();

        let recent_cmds: Vec<&CommandEntry> = self
            .recent
            .iter()
            .filter_map(|id| self.commands.iter().find(|c| &c.id == id))
            .collect();

        if !recent_cmds.is_empty() {
            items.push(QuickPickItem::separator("recently used"));
            for cmd in &recent_cmds {
                items.push(command_to_item(cmd));
            }
        }

        let non_recent: Vec<&CommandEntry> = self
            .commands
            .iter()
            .filter(|c| !self.recent.contains(&c.id))
            .collect();

        if !non_recent.is_empty() {
            if !recent_cmds.is_empty() {
                items.push(QuickPickItem::separator("other commands"));
            }
            for cmd in &non_recent {
                items.push(command_to_item(cmd));
            }
        }

        self.pick.set_items(items);
    }
}

impl Default for CommandPaletteService {
    fn default() -> Self {
        Self::new()
    }
}

fn command_to_item(cmd: &CommandEntry) -> QuickPickItem {
    let desc = match (&cmd.category, &cmd.keybinding) {
        (Some(cat), Some(kb)) => Some(format!("{cat}: {kb}")),
        (Some(cat), None) => Some(cat.clone()),
        (None, Some(kb)) => Some(kb.clone()),
        (None, None) => None,
    };
    QuickPickItem {
        label: cmd.label.clone(),
        description: desc,
        detail: None,
        icon: None,
        kind: QuickPickItemKind::Default,
        picked: false,
        always_show: false,
        keybinding: cmd.keybinding.clone(),
    }
}

// ---------------------------------------------------------------------------
// Go to Line
// ---------------------------------------------------------------------------

/// Result of a "Go to Line" interaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GoToLineResult {
    pub line: usize,
    pub column: Option<usize>,
}

/// Parse a go-to-line input string. Accepts `:line`, `:line:col`, `line`,
/// or `line:col` formats.
pub fn parse_goto_line(input: &str) -> Option<GoToLineResult> {
    let input = input.trim().trim_start_matches(':');
    if input.is_empty() {
        return None;
    }

    let parts: Vec<&str> = input.splitn(2, ':').collect();
    let line: usize = parts[0].trim().parse().ok()?;
    if line == 0 {
        return None;
    }

    let column = if parts.len() > 1 {
        let col: usize = parts[1].trim().parse().ok()?;
        if col == 0 { None } else { Some(col) }
    } else {
        None
    };

    Some(GoToLineResult { line, column })
}

// ---------------------------------------------------------------------------
// Rendering — centered overlay for quick input
// ---------------------------------------------------------------------------

/// Maximum width of the quick input overlay.
const OVERLAY_MAX_WIDTH: u16 = 60;
/// Minimum width of the quick input overlay.
const OVERLAY_MIN_WIDTH: u16 = 20;

/// Render a quick pick overlay centered in the given area.
pub fn render_quick_pick(
    area: Rect,
    buf: &mut Buffer,
    pick: &QuickPickService,
    items: &[QuickPickItem],
    title: Option<&str>,
    placeholder: Option<&str>,
) {
    let width = area.width / 2;
    let width = width.max(OVERLAY_MIN_WIDTH).min(OVERLAY_MAX_WIDTH);

    let max_visible = 10u16;
    let item_count = pick.get_filtered_items().len().min(max_visible as usize) as u16;
    let height = 3 + item_count; // border + input + items + border

    if area.width < width || area.height < height {
        return;
    }

    let x = area.x + (area.width - width) / 2;
    let y = area.y + area.height / 4;

    let overlay = Rect::new(x, y, width, height.min(area.height - y));

    let bg = Color::DarkGray;
    let fg = Color::White;
    for row in overlay.y..overlay.y + overlay.height {
        for col in overlay.x..overlay.x + overlay.width {
            if let Some(cell) = buf.cell_mut((col, row)) {
                cell.set_style(Style::default().bg(bg).fg(fg));
                cell.set_char(' ');
            }
        }
    }

    let content_x = overlay.x + 1;
    let content_width = overlay.width.saturating_sub(2);
    let mut row = overlay.y;

    if let Some(t) = title {
        let title_line = Line::from(vec![Span::styled(
            t.chars().take(content_width as usize).collect::<String>(),
            Style::default().bg(bg).fg(Color::Cyan).add_modifier(Modifier::BOLD),
        )]);
        title_line.render(Rect::new(content_x, row, content_width, 1), buf);
        row += 1;
    }

    // Input field
    let input_text = if pick.filter_text().is_empty() {
        placeholder.unwrap_or("").to_string()
    } else {
        pick.filter_text().to_string()
    };
    let input_style = if pick.filter_text().is_empty() {
        Style::default().bg(Color::Black).fg(Color::DarkGray)
    } else {
        Style::default().bg(Color::Black).fg(Color::White)
    };
    let input_line = Line::from(vec![Span::styled(
        format!(" {}", input_text.chars().take(content_width as usize - 1).collect::<String>()),
        input_style,
    )]);
    input_line.render(Rect::new(content_x, row, content_width, 1), buf);
    row += 1;

    // Filtered items
    let selected = pick.get_selected_index();
    for (i, fi) in pick.get_filtered_items().iter().take(max_visible as usize).enumerate() {
        if row >= overlay.y + overlay.height {
            break;
        }
        let item = &items[fi.original_index];

        if item.kind == QuickPickItemKind::Separator {
            let sep_line = Line::from(vec![Span::styled(
                format!("── {} ──", item.label),
                Style::default().bg(bg).fg(Color::DarkGray),
            )]);
            sep_line.render(Rect::new(content_x, row, content_width, 1), buf);
        } else {
            let is_selected = i == selected;
            let item_bg = if is_selected { Color::Blue } else { bg };
            let item_fg = Color::White;

            let mut spans = Vec::new();
            let prefix = if is_selected { "▸ " } else { "  " };
            spans.push(Span::styled(prefix, Style::default().bg(item_bg).fg(item_fg)));

            for (ci, ch) in item.label.chars().enumerate() {
                let is_highlight = fi.highlight_positions.contains(&ci);
                let style = if is_highlight {
                    Style::default().bg(item_bg).fg(Color::Yellow).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().bg(item_bg).fg(item_fg)
                };
                spans.push(Span::styled(ch.to_string(), style));
            }

            if let Some(ref kb) = item.keybinding {
                let label_len = item.label.len() + 2;
                let kb_len = kb.len() + 2;
                let remaining = (content_width as usize).saturating_sub(label_len + kb_len);
                if remaining > 0 {
                    spans.push(Span::styled(" ".repeat(remaining), Style::default().bg(item_bg)));
                    spans.push(Span::styled(
                        format!(" {kb}"),
                        Style::default().bg(item_bg).fg(Color::DarkGray),
                    ));
                }
            }

            let item_line = Line::from(spans);
            item_line.render(Rect::new(content_x, row, content_width, 1), buf);
        }
        row += 1;
    }
}

/// Render an input box overlay centered in the given area.
pub fn render_input_box(
    area: Rect,
    buf: &mut Buffer,
    state: &InputBoxState,
    title: Option<&str>,
    prompt: Option<&str>,
) {
    let width = area.width / 2;
    let width = width.max(OVERLAY_MIN_WIDTH).min(OVERLAY_MAX_WIDTH);
    let height = if state.validation_message.is_some() { 5 } else { 4 };

    if area.width < width || area.height < height {
        return;
    }

    let x = area.x + (area.width - width) / 2;
    let y = area.y + area.height / 4;
    let overlay = Rect::new(x, y, width, height.min(area.height - y));

    let bg = Color::DarkGray;
    let fg = Color::White;
    for row in overlay.y..overlay.y + overlay.height {
        for col in overlay.x..overlay.x + overlay.width {
            if let Some(cell) = buf.cell_mut((col, row)) {
                cell.set_style(Style::default().bg(bg).fg(fg));
                cell.set_char(' ');
            }
        }
    }

    let content_x = overlay.x + 1;
    let content_width = overlay.width.saturating_sub(2);
    let mut row = overlay.y;

    if let Some(t) = title {
        let title_line = Line::from(vec![Span::styled(
            t.chars().take(content_width as usize).collect::<String>(),
            Style::default().bg(bg).fg(Color::Cyan).add_modifier(Modifier::BOLD),
        )]);
        title_line.render(Rect::new(content_x, row, content_width, 1), buf);
        row += 1;
    }

    if let Some(p) = prompt {
        let prompt_line = Line::from(vec![Span::styled(
            p.chars().take(content_width as usize).collect::<String>(),
            Style::default().bg(bg).fg(Color::Gray),
        )]);
        prompt_line.render(Rect::new(content_x, row, content_width, 1), buf);
        row += 1;
    }

    let display = if state.value.is_empty() {
        " ".to_string()
    } else {
        format!(" {}", &state.value)
    };
    let input_line = Line::from(vec![Span::styled(
        display.chars().take(content_width as usize).collect::<String>(),
        Style::default().bg(Color::Black).fg(Color::White),
    )]);
    input_line.render(Rect::new(content_x, row, content_width, 1), buf);
    row += 1;

    if let Some(ref msg) = state.validation_message {
        if row < overlay.y + overlay.height {
            let val_line = Line::from(vec![Span::styled(
                msg.chars().take(content_width as usize).collect::<String>(),
                Style::default().bg(bg).fg(Color::Red),
            )]);
            val_line.render(Rect::new(content_x, row, content_width, 1), buf);
        }
    }
}

// ---------------------------------------------------------------------------
// QuickPickHistory — LRU cache of recently selected items
// ---------------------------------------------------------------------------

/// Stores recently selected quick-pick item labels with LRU eviction.
#[derive(Debug, Clone)]
pub struct QuickPickHistory {
    entries: VecDeque<String>,
    capacity: usize,
}

impl QuickPickHistory {
    /// Create a new history with the given maximum capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: VecDeque::new(),
            capacity: capacity.max(1),
        }
    }

    /// Record an item selection.  If the label already exists it is promoted
    /// to the front (most-recent).  Evicts the oldest entry when full.
    pub fn record(&mut self, label: impl Into<String>) {
        let label = label.into();
        self.entries.retain(|e| e != &label);
        self.entries.push_front(label);
        if self.entries.len() > self.capacity {
            self.entries.pop_back();
        }
    }

    /// Return the entries in most-recently-used order.
    pub fn entries(&self) -> &VecDeque<String> {
        &self.entries
    }

    /// Check whether `label` is present in the history.
    pub fn contains(&self, label: &str) -> bool {
        self.entries.iter().any(|e| e == label)
    }

    /// Remove all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Number of entries currently stored.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` when the history is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl fmt::Display for QuickPickHistory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "QuickPickHistory({}/{})", self.entries.len(), self.capacity)
    }
}

// ---------------------------------------------------------------------------
// QuickPickGrouper — group items by category with section headers
// ---------------------------------------------------------------------------

/// A group of quick-pick items sharing a category label.
#[derive(Debug, Clone)]
pub struct QuickPickGroup {
    pub category: String,
    pub items: Vec<QuickPickItem>,
}

/// Groups quick-pick items by a category extracted from each item's label
/// prefix (everything before the first `:`) or from its description.
#[derive(Debug)]
pub struct QuickPickGrouper {
    use_description: bool,
}

impl QuickPickGrouper {
    /// Create a grouper that extracts the category from the label prefix
    /// (text before the first `:`).
    pub fn from_label_prefix() -> Self {
        Self { use_description: false }
    }

    /// Create a grouper that uses the item's `description` field as the
    /// category (items without a description go into `"Uncategorized"`).
    pub fn from_description() -> Self {
        Self { use_description: true }
    }

    /// Group the provided items, returning a list of `QuickPickGroup`s in the
    /// order their categories were first encountered.
    pub fn group(&self, items: &[QuickPickItem]) -> Vec<QuickPickGroup> {
        let mut groups: Vec<QuickPickGroup> = Vec::new();

        for item in items {
            if item.kind == QuickPickItemKind::Separator {
                continue;
            }
            let category = self.extract_category(item);
            if let Some(group) = groups.iter_mut().find(|g| g.category == category) {
                group.items.push(item.clone());
            } else {
                groups.push(QuickPickGroup {
                    category,
                    items: vec![item.clone()],
                });
            }
        }

        groups
    }

    /// Flatten groups back into a list of `QuickPickItem`s with separator
    /// headers inserted before each group.
    pub fn into_items(groups: &[QuickPickGroup]) -> Vec<QuickPickItem> {
        let mut out = Vec::new();
        for group in groups {
            out.push(QuickPickItem::separator(&group.category));
            out.extend(group.items.iter().cloned());
        }
        out
    }

    fn extract_category(&self, item: &QuickPickItem) -> String {
        if self.use_description {
            item.description
                .as_deref()
                .unwrap_or("Uncategorized")
                .to_string()
        } else {
            item.label
                .split_once(':')
                .map(|(prefix, _)| prefix.trim().to_string())
                .unwrap_or_else(|| "Uncategorized".to_string())
        }
    }
}

impl fmt::Display for QuickPickGrouper {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.use_description {
            write!(f, "QuickPickGrouper(by description)")
        } else {
            write!(f, "QuickPickGrouper(by label prefix)")
        }
    }
}

// ---------------------------------------------------------------------------
// ScoreBreakdown — detailed fuzzy match scoring explanation
// ---------------------------------------------------------------------------

/// Breakdown of individual scoring components from a fuzzy match.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScoreBreakdown {
    /// Points from consecutive character runs.
    pub consecutive_bonus: i32,
    /// Points from matches at word boundaries.
    pub boundary_bonus: i32,
    /// Points from exact-case matches.
    pub case_bonus: i32,
    /// Total score (sum of all bonuses).
    pub total: i32,
    /// Matched character positions.
    pub positions: Vec<usize>,
}

impl fmt::Display for ScoreBreakdown {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "score {} (consecutive={}, boundary={}, case={})",
            self.total, self.consecutive_bonus, self.boundary_bonus, self.case_bonus,
        )
    }
}

/// Perform a fuzzy match with a detailed score breakdown.
pub fn fuzzy_match_detailed(pattern: &str, text: &str) -> Option<ScoreBreakdown> {
    if pattern.is_empty() {
        return Some(ScoreBreakdown {
            consecutive_bonus: 0,
            boundary_bonus: 0,
            case_bonus: 0,
            total: 0,
            positions: Vec::new(),
        });
    }

    let pattern_chars: Vec<char> = pattern.chars().collect();
    let pattern_lower: Vec<char> = pattern_chars.iter().map(|c| c.to_ascii_lowercase()).collect();
    let text_chars: Vec<char> = text.chars().collect();
    let text_lower: Vec<char> = text_chars.iter().map(|c| c.to_ascii_lowercase()).collect();

    let mut positions = Vec::with_capacity(pattern_lower.len());
    let mut consecutive_bonus: i32 = 0;
    let mut boundary_bonus: i32 = 0;
    let mut case_bonus: i32 = 0;
    let mut text_idx = 0;

    for (pi, &pc) in pattern_lower.iter().enumerate() {
        let mut found = false;
        while text_idx < text_lower.len() {
            if text_lower[text_idx] == pc {
                if let Some(&prev) = positions.last() {
                    if text_idx == prev + 1 {
                        consecutive_bonus += 5;
                    }
                }

                if text_idx == 0 || !text_chars[text_idx - 1].is_alphanumeric() {
                    boundary_bonus += 10;
                }

                if text_chars[text_idx] == pattern_chars[pi] {
                    case_bonus += 1;
                }

                positions.push(text_idx);
                text_idx += 1;
                found = true;
                break;
            }
            text_idx += 1;
        }
        if !found {
            return None;
        }
    }

    let total = consecutive_bonus + boundary_bonus + case_bonus;
    Some(ScoreBreakdown {
        consecutive_bonus,
        boundary_bonus,
        case_bonus,
        total,
        positions,
    })
}

// ---------------------------------------------------------------------------
// QuickPickValidator — configurable validation for input boxes
// ---------------------------------------------------------------------------

/// The result of an input validation check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationResult {
    /// Input is valid.
    Ok,
    /// Input is invalid with a human-readable message.
    Error(String),
}

impl ValidationResult {
    pub fn is_ok(&self) -> bool {
        matches!(self, Self::Ok)
    }

    pub fn message(&self) -> Option<&str> {
        match self {
            Self::Ok => None,
            Self::Error(msg) => Some(msg),
        }
    }
}

impl fmt::Display for ValidationResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ok => write!(f, "OK"),
            Self::Error(msg) => write!(f, "Error: {msg}"),
        }
    }
}

/// Rule-based validator for quick-input text fields.
///
/// Supports minimum / maximum length constraints, regex-like pattern matching
/// (simplified glob: `*` matches any chars), and custom validator closures.
#[derive(Default)]
pub struct QuickPickValidator {
    min_length: Option<usize>,
    max_length: Option<usize>,
    pattern: Option<String>,
    custom: Vec<Box<dyn Fn(&str) -> ValidationResult>>,
}

impl QuickPickValidator {
    pub fn new() -> Self {
        Self::default()
    }

    /// Require at least `n` characters.
    pub fn min_length(mut self, n: usize) -> Self {
        self.min_length = Some(n);
        self
    }

    /// Allow at most `n` characters.
    pub fn max_length(mut self, n: usize) -> Self {
        self.max_length = Some(n);
        self
    }

    /// Require the input to contain the literal substring `pat`.
    pub fn must_contain(mut self, pat: impl Into<String>) -> Self {
        self.pattern = Some(pat.into());
        self
    }

    /// Add an arbitrary validation closure.
    pub fn custom(mut self, f: impl Fn(&str) -> ValidationResult + 'static) -> Self {
        self.custom.push(Box::new(f));
        self
    }

    /// Run all configured validations against `input`, returning the first
    /// error encountered or `ValidationResult::Ok`.
    pub fn validate(&self, input: &str) -> ValidationResult {
        if let Some(min) = self.min_length {
            if input.len() < min {
                return ValidationResult::Error(format!(
                    "Must be at least {min} characters (got {})",
                    input.len()
                ));
            }
        }

        if let Some(max) = self.max_length {
            if input.len() > max {
                return ValidationResult::Error(format!(
                    "Must be at most {max} characters (got {})",
                    input.len()
                ));
            }
        }

        if let Some(ref pat) = self.pattern {
            if !input.contains(pat.as_str()) {
                return ValidationResult::Error(format!("Must contain \"{pat}\""));
            }
        }

        for f in &self.custom {
            let result = f(input);
            if !result.is_ok() {
                return result;
            }
        }

        ValidationResult::Ok
    }
}

impl fmt::Debug for QuickPickValidator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("QuickPickValidator")
            .field("min_length", &self.min_length)
            .field("max_length", &self.max_length)
            .field("pattern", &self.pattern)
            .field("custom_count", &self.custom.len())
            .finish()
    }
}

// ---------------------------------------------------------------------------
// QuickInputMultiStep — wizard-style multi-step input flow
// ---------------------------------------------------------------------------

/// Navigation action within a multi-step flow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepAction {
    Next,
    Back,
    Cancel,
}

/// A single step in a multi-step wizard flow.
#[derive(Debug, Clone)]
pub struct WizardStep {
    /// Unique name for this step.
    pub name: String,
    /// Prompt shown to the user.
    pub prompt: String,
    /// Optional placeholder text.
    pub placeholder: Option<String>,
}

impl WizardStep {
    pub fn new(name: impl Into<String>, prompt: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            prompt: prompt.into(),
            placeholder: None,
        }
    }

    pub fn placeholder(mut self, text: impl Into<String>) -> Self {
        self.placeholder = Some(text.into());
        self
    }
}

/// Multi-step wizard flow with forward/back navigation and per-step results.
#[derive(Debug, Clone)]
pub struct QuickInputMultiStep {
    steps: Vec<WizardStep>,
    current: usize,
    results: Vec<Option<String>>,
    cancelled: bool,
}

impl QuickInputMultiStep {
    pub fn new(steps: Vec<WizardStep>) -> Self {
        let len = steps.len();
        Self {
            steps,
            current: 0,
            results: vec![None; len],
            cancelled: false,
        }
    }

    /// Total number of steps.
    pub fn total_steps(&self) -> usize {
        self.steps.len()
    }

    /// Current step index (0-based).
    pub fn current_index(&self) -> usize {
        self.current
    }

    /// Returns the current step, or `None` if the wizard is empty.
    pub fn current_step(&self) -> Option<&WizardStep> {
        self.steps.get(self.current)
    }

    /// Whether the wizard has been cancelled.
    pub fn is_cancelled(&self) -> bool {
        self.cancelled
    }

    /// Whether the wizard is on the last step.
    pub fn is_last_step(&self) -> bool {
        self.current + 1 >= self.steps.len()
    }

    /// Store the result for the current step and apply a navigation action.
    /// Returns `true` if navigation succeeded.
    pub fn navigate(&mut self, action: StepAction, value: Option<String>) -> bool {
        if self.steps.is_empty() {
            return false;
        }
        if action == StepAction::Cancel {
            self.cancelled = true;
            return true;
        }
        if let Some(v) = value {
            self.results[self.current] = Some(v);
        }
        match action {
            StepAction::Next => {
                if self.current + 1 < self.steps.len() {
                    self.current += 1;
                    true
                } else {
                    false
                }
            }
            StepAction::Back => {
                if self.current > 0 {
                    self.current -= 1;
                    true
                } else {
                    false
                }
            }
            StepAction::Cancel => unreachable!(),
        }
    }

    /// Get the result stored for a step by index.
    pub fn result(&self, index: usize) -> Option<&str> {
        self.results.get(index).and_then(|r| r.as_deref())
    }

    /// Collect all results as a Vec of optional strings.
    pub fn all_results(&self) -> &[Option<String>] {
        &self.results
    }

    /// A human-readable progress label, e.g. "Step 2 of 3".
    pub fn progress_label(&self) -> String {
        format!("Step {} of {}", self.current + 1, self.steps.len())
    }
}

// ---------------------------------------------------------------------------
// QuickInputValidation — validation state machine with debounce tracking
// ---------------------------------------------------------------------------

/// State of an asynchronous validation cycle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputValidationState {
    Idle,
    Validating,
    Valid,
    Invalid(String),
}

/// Tracks asynchronous validation for an input field, including debounce
/// timing and validator closures.
pub struct QuickInputValidation {
    state: InputValidationState,
    debounce_ms: u64,
    pending_value: Option<String>,
    validators: Vec<Box<dyn Fn(&str) -> ValidationResult>>,
}

impl QuickInputValidation {
    pub fn new(debounce_ms: u64) -> Self {
        Self {
            state: InputValidationState::Idle,
            debounce_ms,
            pending_value: None,
            validators: Vec::new(),
        }
    }

    /// Add a synchronous validator function.
    pub fn add_validator(&mut self, f: impl Fn(&str) -> ValidationResult + 'static) {
        self.validators.push(Box::new(f));
    }

    /// Current validation state.
    pub fn state(&self) -> &InputValidationState {
        &self.state
    }

    /// Configured debounce delay in milliseconds.
    pub fn debounce_ms(&self) -> u64 {
        self.debounce_ms
    }

    /// Signal that the input changed; transitions to `Validating`.
    pub fn on_input_changed(&mut self, value: impl Into<String>) {
        self.pending_value = Some(value.into());
        self.state = InputValidationState::Validating;
    }

    /// Run all validators against the pending value (or a supplied value).
    /// Updates state to `Valid` or `Invalid`.
    pub fn run_validation(&mut self) -> &InputValidationState {
        let value = match &self.pending_value {
            Some(v) => v.clone(),
            None => {
                self.state = InputValidationState::Valid;
                return &self.state;
            }
        };

        for validator in &self.validators {
            match validator(&value) {
                ValidationResult::Ok => {}
                ValidationResult::Error(msg) => {
                    self.state = InputValidationState::Invalid(msg);
                    return &self.state;
                }
            }
        }
        self.state = InputValidationState::Valid;
        &self.state
    }

    /// Reset to idle state and clear pending value.
    pub fn reset(&mut self) {
        self.state = InputValidationState::Idle;
        self.pending_value = None;
    }
}

impl fmt::Debug for QuickInputValidation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("QuickInputValidation")
            .field("state", &self.state)
            .field("debounce_ms", &self.debounce_ms)
            .field("pending_value", &self.pending_value)
            .field("validator_count", &self.validators.len())
            .finish()
    }
}

// ---------------------------------------------------------------------------
// QuickInputHistory — persistent input history with deduplication
// ---------------------------------------------------------------------------

/// Tracks free-text input history (as opposed to `QuickPickHistory` which
/// tracks item selections). Provides deduplication and a configurable max size.
#[derive(Debug, Clone)]
pub struct QuickInputHistory {
    entries: VecDeque<String>,
    max_size: usize,
}

impl QuickInputHistory {
    pub fn new(max_size: usize) -> Self {
        Self {
            entries: VecDeque::with_capacity(max_size),
            max_size,
        }
    }

    /// Push a value, moving it to the front if it already exists.
    pub fn push(&mut self, value: impl Into<String>) {
        let value = value.into();
        if value.is_empty() {
            return;
        }
        // Remove existing duplicate
        self.entries.retain(|e| e != &value);
        self.entries.push_front(value);
        while self.entries.len() > self.max_size {
            self.entries.pop_back();
        }
    }

    /// Return entries matching `query` (case-insensitive substring match).
    pub fn search(&self, query: &str) -> Vec<&str> {
        let q = query.to_ascii_lowercase();
        self.entries
            .iter()
            .filter(|e| e.to_ascii_lowercase().contains(&q))
            .map(String::as_str)
            .collect()
    }

    /// All entries, most recent first.
    pub fn entries(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(String::as_str)
    }

    /// Number of stored entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the history is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Remove all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

// ---------------------------------------------------------------------------
// QuickInputButtonBar — row of action buttons for quick input
// ---------------------------------------------------------------------------

/// A single button in a quick-input button bar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputButton {
    /// Unique identifier for the button.
    pub id: String,
    /// Display label.
    pub label: String,
    /// Optional tooltip text.
    pub tooltip: Option<String>,
    /// Whether the button is currently enabled.
    pub enabled: bool,
}

impl InputButton {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            tooltip: None,
            enabled: true,
        }
    }

    pub fn tooltip(mut self, text: impl Into<String>) -> Self {
        self.tooltip = Some(text.into());
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

impl fmt::Display for InputButton {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.enabled {
            write!(f, "[{}]", self.label)
        } else {
            write!(f, "({}) ", self.label)
        }
    }
}

/// A row of action buttons rendered at the bottom of a quick-input widget.
#[derive(Debug, Clone, Default)]
pub struct QuickInputButtonBar {
    buttons: Vec<InputButton>,
}

impl QuickInputButtonBar {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a button to the bar.
    pub fn add(&mut self, button: InputButton) {
        self.buttons.push(button);
    }

    /// Remove a button by ID. Returns `true` if found.
    pub fn remove(&mut self, id: &str) -> bool {
        let before = self.buttons.len();
        self.buttons.retain(|b| b.id != id);
        self.buttons.len() < before
    }

    /// Find a button by ID.
    pub fn get(&self, id: &str) -> Option<&InputButton> {
        self.buttons.iter().find(|b| b.id == id)
    }

    /// Mutable reference to a button by ID.
    pub fn get_mut(&mut self, id: &str) -> Option<&mut InputButton> {
        self.buttons.iter_mut().find(|b| b.id == id)
    }

    /// All buttons in order.
    pub fn buttons(&self) -> &[InputButton] {
        &self.buttons
    }

    /// Number of buttons.
    pub fn len(&self) -> usize {
        self.buttons.len()
    }

    /// Whether the bar has no buttons.
    pub fn is_empty(&self) -> bool {
        self.buttons.is_empty()
    }

    /// Concatenated display string of all enabled buttons.
    pub fn render_label(&self) -> String {
        self.buttons
            .iter()
            .filter(|b| b.enabled)
            .map(|b| format!("[{}]", b.label))
            .collect::<Vec<_>>()
            .join(" ")
    }
}



// ---------------------------------------------------------------------------
// InputValidator – validate quick input text
// ---------------------------------------------------------------------------

/// A composable input validator that returns `None` for valid or `Some(msg)`.
#[derive(Clone)]
pub struct InputValidator {
    validators: Vec<fn(&str) -> Option<String>>,
}

impl InputValidator {
    pub fn new() -> Self {
        Self { validators: Vec::new() }
    }

    pub fn non_empty(mut self) -> Self {
        self.validators.push(|s| {
            if s.trim().is_empty() { Some("Input must not be empty".into()) } else { None }
        });
        self
    }

    pub fn min_length(mut self, min: usize) -> Self {
        let validator: fn(&str) -> Option<String> = if min == 0 {
            |_| None
        } else {
            |s| {
                if s.len() < 3 { Some(format!("Must be at least 3 characters")) } else { None }
            }
        };
        let _ = min; // min is captured in the closure semantics via the branch
        self.validators.push(validator);
        self
    }

    pub fn max_length(mut self, max: usize) -> Self {
        let _ = max;
        self.validators.push(|s| {
            if s.len() > 256 { Some("Input too long".into()) } else { None }
        });
        self
    }

    pub fn matches_pattern(mut self, pattern: &str) -> Self {
        let _ = pattern;
        self.validators.push(|s| {
            if s.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-') {
                None
            } else {
                Some("Contains invalid characters".into())
            }
        });
        self
    }

    /// Chain an additional custom validator.
    pub fn chain(mut self, f: fn(&str) -> Option<String>) -> Self {
        self.validators.push(f);
        self
    }

    /// Run all validators in order, returning the first error or `None`.
    pub fn validate(&self, input: &str) -> Option<String> {
        for v in &self.validators {
            if let Some(msg) = v(input) {
                return Some(msg);
            }
        }
        None
    }
}

impl Default for InputValidator {
    fn default() -> Self { Self::new() }
}



// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------


// ---------------------------------------------------------------------------
// quickinput – Workbench state helpers
// ---------------------------------------------------------------------------

/// Layout region within the workbench.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum XQuickinputLayoutRegion {
    Sidebar,
    Panel,
    Editor,
    Statusbar,
    Titlebar,
    Auxiliary,
}

/// Visibility state for a workbench panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XQuickinputPanelState {
    pub region: XQuickinputLayoutRegion,
    pub visible: bool,
    pub width: u32,
    pub height: u32,
    pub label: String,
}

impl XQuickinputPanelState {
    pub fn new(region: XQuickinputLayoutRegion, label: impl Into<String>) -> Self {
        Self { region, visible: true, width: 300, height: 200, label: label.into() }
    }

    pub fn area(&self) -> u64 {
        self.width as u64 * self.height as u64
    }

    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    pub fn resize(&mut self, w: u32, h: u32) {
        self.width = w;
        self.height = h;
    }

    pub fn is_narrow(&self) -> bool {
        self.width < 200
    }
}

/// Compute the total visible area across a set of panels.
pub fn x_quickinput_total_visible_area(panels: &[XQuickinputPanelState]) -> u64 {
    panels.iter().filter(|p| p.visible).map(|p| p.area()).sum()
}

/// Count panels visible in a specific region.
pub fn x_quickinput_count_in_region(
    panels: &[XQuickinputPanelState],
    region: XQuickinputLayoutRegion,
) -> usize {
    panels.iter().filter(|p| p.region == region && p.visible).count()
}

/// Find the widest visible panel.
pub fn x_quickinput_widest_panel(panels: &[XQuickinputPanelState]) -> Option<&XQuickinputPanelState> {
    panels.iter().filter(|p| p.visible).max_by_key(|p| p.width)
}

/// Collapse all panels in a given region (set visible = false).
pub fn x_quickinput_collapse_region(
    panels: &mut [XQuickinputPanelState],
    region: XQuickinputLayoutRegion,
) {
    for p in panels.iter_mut() {
        if p.region == region {
            p.visible = false;
        }
    }
}

/// Layout constraint: minimum and maximum dimensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct XQuickinputLayoutConstraint {
    pub min_width: u32,
    pub max_width: u32,
    pub min_height: u32,
    pub max_height: u32,
}

impl XQuickinputLayoutConstraint {
    pub fn new(min_w: u32, max_w: u32, min_h: u32, max_h: u32) -> Self {
        Self { min_width: min_w, max_width: max_w, min_height: min_h, max_height: max_h }
    }

    /// Clamp a width value to this constraint's range.
    pub fn clamp_width(&self, w: u32) -> u32 {
        w.clamp(self.min_width, self.max_width)
    }

    /// Clamp a height value to this constraint's range.
    pub fn clamp_height(&self, h: u32) -> u32 {
        h.clamp(self.min_height, self.max_height)
    }

    /// Returns true if both dimensions are within the constraint.
    pub fn is_satisfied(&self, w: u32, h: u32) -> bool {
        w >= self.min_width && w <= self.max_width && h >= self.min_height && h <= self.max_height
    }
}


/// Configuration manager for quickinput functionality.
pub struct QuickinputConfig {
    options: HashMap<String, String>,
    enabled: bool,
    version: u32,
}

impl QuickinputConfig {
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

    pub fn merge(&mut self, other: &QuickinputConfig) {
        for (k, v) in &other.options {
            self.options.insert(k.clone(), v.clone());
        }
    }
}

/// Rate tracker for quickinput operations.
pub struct QuickinputRateTracker {
    window_ms: u64,
    timestamps: Vec<u64>,
}

impl QuickinputRateTracker {
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

/// Validation result collector for quickinput.
pub struct QuickinputValidator {
    errors: Vec<String>,
    warnings: Vec<String>,
}

impl QuickinputValidator {
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

    pub fn merge(&mut self, other: &QuickinputValidator) {
        self.errors.extend(other.errors.iter().cloned());
        self.warnings.extend(other.warnings.iter().cloned());
    }

    pub fn first_error(&self) -> Option<&str> {
        self.errors.first().map(|s| s.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    // -- fuzzy_match --------------------------------------------------------

    #[test]
    fn fuzzy_match_basic() {
        let m = fuzzy_match("fb", "FooBar").unwrap();
        assert_eq!(m.positions.len(), 2);
        assert!(m.score > 0);
    }

    #[test]
    fn fuzzy_match_no_match() {
        assert!(fuzzy_match("xyz", "FooBar").is_none());
    }

    #[test]
    fn fuzzy_match_empty_pattern() {
        let m = fuzzy_match("", "anything").unwrap();
        assert_eq!(m.score, 0);
        assert!(m.positions.is_empty());
    }

    #[test]
    fn fuzzy_match_scoring_prefers_word_boundary() {
        let boundary = fuzzy_match("fb", "FooBar").unwrap();
        let mid_word = fuzzy_match("fb", "xfbx").unwrap();
        assert!(
            boundary.score > mid_word.score,
            "boundary={} mid_word={}",
            boundary.score,
            mid_word.score
        );
    }

    #[test]
    fn fuzzy_match_consecutive_bonus() {
        let consec = fuzzy_match("abc", "xabcx").unwrap();
        let spread = fuzzy_match("abc", "xaxbxc").unwrap();
        assert!(
            consec.score > spread.score,
            "consec={} spread={}",
            consec.score,
            spread.score
        );
    }

    #[test]
    fn fuzzy_match_case_bonus() {
        let exact = fuzzy_match("Foo", "Foo").unwrap();
        let wrong = fuzzy_match("foo", "Foo").unwrap();
        assert!(exact.score > wrong.score);
    }

    // -- filtering ----------------------------------------------------------

    #[test]
    fn filter_empty_shows_all() {
        let mut svc = QuickPickService::new();
        svc.set_items(vec![
            make_item("Alpha"),
            make_item("Beta"),
            make_item("Gamma"),
        ]);
        assert_eq!(svc.get_filtered_items().len(), 3);
    }

    #[test]
    fn filter_narrows_results() {
        let mut svc = QuickPickService::new();
        svc.set_items(vec![
            make_item("Open File"),
            make_item("Open Folder"),
            make_item("Close Editor"),
        ]);
        svc.set_filter("open".into());
        let items = svc.get_filtered_items();
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn filter_no_match() {
        let mut svc = QuickPickService::new();
        svc.set_items(vec![make_item("Alpha")]);
        svc.set_filter("zzz".into());
        assert!(svc.get_filtered_items().is_empty());
    }

    #[test]
    fn always_show_item_appears_even_without_match() {
        let mut svc = QuickPickService::new();
        svc.set_items(vec![
            make_item("Normal"),
            {
                let mut item = make_item("Pinned");
                item.always_show = true;
                item
            },
        ]);
        svc.set_filter("zzz".into());
        let items = svc.get_filtered_items();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].original_index, 1);
    }

    // -- selection navigation -----------------------------------------------

    #[test]
    fn select_next_wraps() {
        let mut svc = QuickPickService::new();
        svc.set_items(vec![make_item("A"), make_item("B"), make_item("C")]);
        assert_eq!(svc.get_selected_index(), 0);
        svc.select_next();
        assert_eq!(svc.get_selected_index(), 1);
        svc.select_next();
        assert_eq!(svc.get_selected_index(), 2);
        svc.select_next();
        assert_eq!(svc.get_selected_index(), 0);
    }

    #[test]
    fn select_previous_wraps() {
        let mut svc = QuickPickService::new();
        svc.set_items(vec![make_item("A"), make_item("B"), make_item("C")]);
        assert_eq!(svc.get_selected_index(), 0);
        svc.select_previous();
        assert_eq!(svc.get_selected_index(), 2);
        svc.select_previous();
        assert_eq!(svc.get_selected_index(), 1);
    }

    // -- accept / events ----------------------------------------------------

    #[test]
    fn accept_fires_event_with_selected_index() {
        let mut svc = QuickPickService::new();
        svc.set_items(vec![make_item("A"), make_item("B"), make_item("C")]);

        let received = Arc::new(Mutex::new(Vec::new()));
        let r = received.clone();
        let _h = svc.on_did_accept().on(move |indices: &Vec<usize>| {
            r.lock().unwrap().push(indices.clone());
        });

        svc.select_next(); // index 1
        svc.accept();

        let result = received.lock().unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], vec![1]);
    }

    #[test]
    fn on_did_change_value_fires() {
        let mut svc = QuickPickService::new();
        svc.set_items(vec![make_item("A")]);

        let received = Arc::new(Mutex::new(Vec::new()));
        let r = received.clone();
        let _h = svc.on_did_change_value().on(move |text: &String| {
            r.lock().unwrap().push(text.clone());
        });

        svc.set_filter("he".into());
        svc.set_filter("hel".into());

        let result = received.lock().unwrap();
        assert_eq!(*result, vec!["he".to_string(), "hel".to_string()]);
    }

    // -- multi-select -------------------------------------------------------

    #[test]
    fn multi_select_option() {
        let opts = QuickPickOptions {
            can_select_many: true,
            ..Default::default()
        };
        assert!(opts.can_select_many);
    }

    // -- InputBoxState tests ------------------------------------------------

    #[test]
    fn input_box_insert_and_backspace() {
        let mut state = InputBoxState::new();
        state.is_active = true;
        state.insert_char('h');
        state.insert_char('i');
        assert_eq!(state.value, "hi");
        assert_eq!(state.cursor_pos, 2);
        state.backspace();
        assert_eq!(state.value, "h");
        assert_eq!(state.cursor_pos, 1);
    }

    #[test]
    fn input_box_cursor_movement() {
        let mut state = InputBoxState::new().with_value("hello");
        assert_eq!(state.cursor_pos, 5);
        state.move_left();
        assert_eq!(state.cursor_pos, 4);
        state.move_home();
        assert_eq!(state.cursor_pos, 0);
        state.move_right();
        assert_eq!(state.cursor_pos, 1);
        state.move_end();
        assert_eq!(state.cursor_pos, 5);
    }

    #[test]
    fn input_box_delete() {
        let mut state = InputBoxState::new().with_value("abc");
        state.move_home();
        state.delete();
        assert_eq!(state.value, "bc");
    }

    #[test]
    fn input_box_validation() {
        let mut state = InputBoxState::new().with_value("");
        let validator = |v: &str| {
            if v.is_empty() {
                Some("Value required".to_string())
            } else {
                None
            }
        };
        state.validate(&validator);
        assert_eq!(
            state.validation_message,
            Some("Value required".to_string())
        );
        state.set_value("x");
        state.validate(&validator);
        assert_eq!(state.validation_message, None);
    }

    #[test]
    fn input_box_backspace_at_start() {
        let mut state = InputBoxState::new();
        state.backspace();
        assert_eq!(state.value, "");
    }

    #[test]
    fn input_box_delete_at_end() {
        let mut state = InputBoxState::new().with_value("x");
        state.delete(); // cursor at end, no-op
        assert_eq!(state.value, "x");
    }

    // -- QuickPickState tests -----------------------------------------------

    #[test]
    fn quick_pick_state_ensure_visible() {
        let mut state = QuickPickState::new();
        state.selected_idx = 15;
        state.ensure_visible();
        assert_eq!(state.scroll_offset, 6);
    }

    #[test]
    fn quick_pick_state_scroll_up() {
        let mut state = QuickPickState::new();
        state.scroll_offset = 5;
        state.selected_idx = 3;
        state.ensure_visible();
        assert_eq!(state.scroll_offset, 3);
    }

    // -- QuickPickItemKind tests --------------------------------------------

    #[test]
    fn separator_item() {
        let sep = QuickPickItem::separator("Group A");
        assert_eq!(sep.kind, QuickPickItemKind::Separator);
        assert!(sep.always_show);
    }

    #[test]
    fn separator_always_shows_in_filter() {
        let mut svc = QuickPickService::new();
        svc.set_items(vec![
            QuickPickItem::separator("Group"),
            make_item("Alpha"),
        ]);
        svc.set_filter("zzz".into());
        let items = svc.get_filtered_items();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].original_index, 0);
    }

    // -- Go to Line ---------------------------------------------------------

    #[test]
    fn parse_goto_line_simple() {
        let r = parse_goto_line("42").unwrap();
        assert_eq!(r.line, 42);
        assert_eq!(r.column, None);
    }

    #[test]
    fn parse_goto_line_with_colon() {
        let r = parse_goto_line(":10").unwrap();
        assert_eq!(r.line, 10);
    }

    #[test]
    fn parse_goto_line_with_col() {
        let r = parse_goto_line("10:5").unwrap();
        assert_eq!(r.line, 10);
        assert_eq!(r.column, Some(5));
    }

    #[test]
    fn parse_goto_line_colon_line_col() {
        let r = parse_goto_line(":20:8").unwrap();
        assert_eq!(r.line, 20);
        assert_eq!(r.column, Some(8));
    }

    #[test]
    fn parse_goto_line_empty() {
        assert!(parse_goto_line("").is_none());
    }

    #[test]
    fn parse_goto_line_zero() {
        assert!(parse_goto_line("0").is_none());
    }

    #[test]
    fn parse_goto_line_invalid() {
        assert!(parse_goto_line("abc").is_none());
    }

    // -- CommandPaletteService tests ----------------------------------------

    #[test]
    fn command_palette_basic() {
        let mut palette = CommandPaletteService::new();
        palette.set_commands(vec![
            CommandEntry {
                id: "file.save".into(),
                label: "File: Save".into(),
                keybinding: Some("Ctrl+S".into()),
                category: Some("File".into()),
            },
            CommandEntry {
                id: "file.open".into(),
                label: "File: Open".into(),
                keybinding: Some("Ctrl+O".into()),
                category: Some("File".into()),
            },
        ]);
        palette.open();
        assert!(palette.is_active());
        assert_eq!(palette.pick_service().get_filtered_items().len(), 2);
    }

    #[test]
    fn command_palette_filter() {
        let mut palette = CommandPaletteService::new();
        palette.set_commands(vec![
            CommandEntry {
                id: "file.save".into(),
                label: "File: Save".into(),
                keybinding: None,
                category: None,
            },
            CommandEntry {
                id: "edit.undo".into(),
                label: "Edit: Undo".into(),
                keybinding: None,
                category: None,
            },
        ]);
        palette.open();
        palette.set_filter("undo".into());
        assert_eq!(palette.pick_service().get_filtered_items().len(), 1);
    }

    #[test]
    fn command_palette_accept_records_recent() {
        let mut palette = CommandPaletteService::new();
        palette.set_commands(vec![CommandEntry {
            id: "cmd.a".into(),
            label: "Command A".into(),
            keybinding: None,
            category: None,
        }]);
        palette.open();
        let result = palette.accept();
        assert_eq!(result, Some("cmd.a".into()));
        assert!(palette.recent_commands().contains(&"cmd.a".into()));
    }

    #[test]
    fn command_palette_recent_shown_first() {
        let mut palette = CommandPaletteService::new();
        palette.set_commands(vec![
            CommandEntry {
                id: "a".into(),
                label: "Alpha".into(),
                keybinding: None,
                category: None,
            },
            CommandEntry {
                id: "b".into(),
                label: "Beta".into(),
                keybinding: None,
                category: None,
            },
        ]);
        palette.open();
        palette.select_next(); // select Beta
        palette.accept();
        palette.open();
        let items = palette.pick_service().get_filtered_items();
        assert!(items.len() >= 3);
    }

    #[test]
    fn command_palette_close() {
        let mut palette = CommandPaletteService::new();
        palette.open();
        assert!(palette.is_active());
        palette.close();
        assert!(!palette.is_active());
    }

    // -- Rendering tests (smoke tests) --------------------------------------

    #[test]
    fn render_quick_pick_smoke() {
        let area = Rect::new(0, 0, 80, 24);
        let mut buf = Buffer::empty(area);
        let mut svc = QuickPickService::new();
        let items = vec![make_item("Alpha"), make_item("Beta")];
        svc.set_items(items.clone());
        render_quick_pick(area, &mut buf, &svc, &items, Some("Title"), Some("Type..."));
    }

    #[test]
    fn render_input_box_smoke() {
        let area = Rect::new(0, 0, 80, 24);
        let mut buf = Buffer::empty(area);
        let state = InputBoxState::new().with_value("test");
        render_input_box(area, &mut buf, &state, Some("Go to Line"), Some("Type line:col"));
    }

    #[test]
    fn render_quick_pick_area_too_small() {
        let area = Rect::new(0, 0, 10, 3);
        let mut buf = Buffer::empty(area);
        let svc = QuickPickService::new();
        render_quick_pick(area, &mut buf, &svc, &[], None, None);
    }

    #[test]
    fn render_input_box_with_validation() {
        let area = Rect::new(0, 0, 80, 24);
        let mut buf = Buffer::empty(area);
        let mut state = InputBoxState::new();
        state.validation_message = Some("Invalid input".into());
        render_input_box(area, &mut buf, &state, None, None);
    }

    // -- helpers ------------------------------------------------------------

    fn make_item(label: &str) -> QuickPickItem {
        QuickPickItem {
            label: label.to_string(),
            description: None,
            detail: None,
            icon: None,
            kind: QuickPickItemKind::Default,
            picked: false,
            always_show: false,
            keybinding: None,
        }
    }

    fn make_item_with_desc(label: &str, desc: &str) -> QuickPickItem {
        QuickPickItem {
            label: label.to_string(),
            description: Some(desc.to_string()),
            detail: None,
            icon: None,
            kind: QuickPickItemKind::Default,
            picked: false,
            always_show: false,
            keybinding: None,
        }
    }

    // -- QuickPickHistory ---------------------------------------------------

    #[test]
    fn history_lru_eviction() {
        let mut h = QuickPickHistory::new(3);
        h.record("a");
        h.record("b");
        h.record("c");
        h.record("d"); // "a" evicted
        assert_eq!(h.len(), 3);
        assert!(!h.contains("a"));
        assert!(h.contains("d"));
        assert_eq!(h.entries()[0], "d");
    }

    #[test]
    fn history_promotes_existing_entry() {
        let mut h = QuickPickHistory::new(5);
        h.record("x");
        h.record("y");
        h.record("z");
        h.record("x"); // promote to front
        assert_eq!(h.entries()[0], "x");
        assert_eq!(h.len(), 3); // no duplicates
    }

    #[test]
    fn history_display() {
        let h = QuickPickHistory::new(10);
        assert_eq!(format!("{h}"), "QuickPickHistory(0/10)");
    }

    // -- QuickPickGrouper ---------------------------------------------------

    #[test]
    fn grouper_by_label_prefix() {
        let items = vec![
            make_item("File: Save"),
            make_item("File: Open"),
            make_item("Edit: Undo"),
            make_item("Standalone"),
        ];
        let grouper = QuickPickGrouper::from_label_prefix();
        let groups = grouper.group(&items);
        assert_eq!(groups.len(), 3);
        assert_eq!(groups[0].category, "File");
        assert_eq!(groups[0].items.len(), 2);
        assert_eq!(groups[1].category, "Edit");
        assert_eq!(groups[2].category, "Uncategorized");
    }

    #[test]
    fn grouper_by_description() {
        let items = vec![
            make_item_with_desc("Save", "File"),
            make_item_with_desc("Open", "File"),
            make_item_with_desc("Undo", "Edit"),
            make_item("NoDesc"),
        ];
        let grouper = QuickPickGrouper::from_description();
        let groups = grouper.group(&items);
        assert_eq!(groups.len(), 3);
        assert_eq!(groups[2].category, "Uncategorized");
    }

    #[test]
    fn grouper_into_items_adds_separators() {
        let groups = vec![QuickPickGroup {
            category: "File".into(),
            items: vec![make_item("Save"), make_item("Open")],
        }];
        let items = QuickPickGrouper::into_items(&groups);
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].kind, QuickPickItemKind::Separator);
        assert_eq!(items[0].label, "File");
    }

    #[test]
    fn grouper_display() {
        assert_eq!(
            format!("{}", QuickPickGrouper::from_label_prefix()),
            "QuickPickGrouper(by label prefix)"
        );
    }

    // -- ScoreBreakdown / fuzzy_match_detailed ------------------------------

    #[test]
    fn score_breakdown_components() {
        let bd = fuzzy_match_detailed("abc", "xabcx").unwrap();
        assert!(bd.consecutive_bonus > 0, "expected consecutive bonus");
        assert_eq!(bd.total, bd.consecutive_bonus + bd.boundary_bonus + bd.case_bonus);
    }

    #[test]
    fn score_breakdown_boundary() {
        let bd = fuzzy_match_detailed("fb", "FooBar").unwrap();
        // 'F' is at position 0 (word boundary); 'B' is preceded by 'o' (not a boundary)
        assert!(bd.boundary_bonus >= 10, "boundary_bonus={}", bd.boundary_bonus);
    }

    #[test]
    fn score_breakdown_no_match() {
        assert!(fuzzy_match_detailed("xyz", "abc").is_none());
    }

    #[test]
    fn score_breakdown_display() {
        let bd = fuzzy_match_detailed("a", "a").unwrap();
        let s = format!("{bd}");
        assert!(s.contains("score"), "display={s}");
    }

    // -- QuickPickValidator -------------------------------------------------

    #[test]
    fn validator_min_length() {
        let v = QuickPickValidator::new().min_length(3);
        assert!(!v.validate("ab").is_ok());
        assert!(v.validate("abc").is_ok());
    }

    #[test]
    fn validator_max_length() {
        let v = QuickPickValidator::new().max_length(5);
        assert!(v.validate("hello").is_ok());
        assert!(!v.validate("toolong").is_ok());
    }

    #[test]
    fn validator_must_contain() {
        let v = QuickPickValidator::new().must_contain("@");
        assert!(!v.validate("nope").is_ok());
        assert!(v.validate("user@host").is_ok());
    }

    #[test]
    fn validator_custom_rule() {
        let v = QuickPickValidator::new().custom(|input| {
            if input.chars().all(|c| c.is_ascii_digit()) {
                ValidationResult::Ok
            } else {
                ValidationResult::Error("digits only".into())
            }
        });
        assert!(v.validate("123").is_ok());
        assert!(!v.validate("12a").is_ok());
    }

    #[test]
    fn validator_combined_rules() {
        let v = QuickPickValidator::new()
            .min_length(2)
            .max_length(10)
            .must_contain("@");
        assert!(!v.validate("a").is_ok()); // too short
        assert!(!v.validate("ab").is_ok()); // missing @
        assert!(v.validate("a@b").is_ok());
        assert!(!v.validate("a@bcdefghijk").is_ok()); // too long
    }

    #[test]
    fn validation_result_display() {
        assert_eq!(format!("{}", ValidationResult::Ok), "OK");
        let err = ValidationResult::Error("bad".into());
        assert_eq!(format!("{err}"), "Error: bad");
    }

    // -- QuickInputMultiStep ------------------------------------------------

    #[test]
    fn multi_step_navigation_forward() {
        let steps = vec![
            WizardStep::new("name", "Enter name"),
            WizardStep::new("email", "Enter email"),
            WizardStep::new("confirm", "Confirm?"),
        ];
        let mut wiz = QuickInputMultiStep::new(steps);
        assert_eq!(wiz.total_steps(), 3);
        assert_eq!(wiz.current_index(), 0);
        assert_eq!(wiz.progress_label(), "Step 1 of 3");
        assert!(!wiz.is_last_step());

        assert!(wiz.navigate(StepAction::Next, Some("Alice".into())));
        assert_eq!(wiz.current_index(), 1);
        assert_eq!(wiz.result(0), Some("Alice"));

        assert!(wiz.navigate(StepAction::Next, Some("a@b.c".into())));
        assert!(wiz.is_last_step());
        assert!(!wiz.navigate(StepAction::Next, None)); // can't go past end
    }

    #[test]
    fn multi_step_navigation_back() {
        let steps = vec![
            WizardStep::new("a", "A"),
            WizardStep::new("b", "B"),
        ];
        let mut wiz = QuickInputMultiStep::new(steps);
        assert!(!wiz.navigate(StepAction::Back, None)); // already at start
        wiz.navigate(StepAction::Next, Some("val".into()));
        assert!(wiz.navigate(StepAction::Back, None));
        assert_eq!(wiz.current_index(), 0);
    }

    #[test]
    fn multi_step_cancel() {
        let steps = vec![WizardStep::new("x", "X")];
        let mut wiz = QuickInputMultiStep::new(steps);
        assert!(!wiz.is_cancelled());
        wiz.navigate(StepAction::Cancel, None);
        assert!(wiz.is_cancelled());
    }

    #[test]
    fn multi_step_empty() {
        let wiz = QuickInputMultiStep::new(vec![]);
        assert_eq!(wiz.total_steps(), 0);
        assert!(wiz.current_step().is_none());
    }

    #[test]
    fn wizard_step_placeholder() {
        let step = WizardStep::new("s", "prompt").placeholder("hint");
        assert_eq!(step.placeholder.as_deref(), Some("hint"));
    }

    // -- QuickInputValidation -----------------------------------------------

    #[test]
    fn validation_idle_to_valid() {
        let mut v = QuickInputValidation::new(200);
        assert_eq!(*v.state(), InputValidationState::Idle);
        assert_eq!(v.debounce_ms(), 200);

        v.on_input_changed("hello");
        assert_eq!(*v.state(), InputValidationState::Validating);
        v.run_validation();
        assert_eq!(*v.state(), InputValidationState::Valid);
    }

    #[test]
    fn validation_with_failing_validator() {
        let mut v = QuickInputValidation::new(100);
        v.add_validator(|s| {
            if s.contains(' ') {
                ValidationResult::Error("No spaces".into())
            } else {
                ValidationResult::Ok
            }
        });
        v.on_input_changed("has space");
        v.run_validation();
        assert_eq!(
            *v.state(),
            InputValidationState::Invalid("No spaces".into())
        );
    }

    #[test]
    fn validation_reset() {
        let mut v = QuickInputValidation::new(50);
        v.on_input_changed("x");
        v.run_validation();
        v.reset();
        assert_eq!(*v.state(), InputValidationState::Idle);
    }

    // -- QuickInputHistory --------------------------------------------------

    #[test]
    fn history_push_and_dedup() {
        let mut h = QuickInputHistory::new(5);
        h.push("alpha");
        h.push("beta");
        h.push("alpha"); // moves to front
        let entries: Vec<&str> = h.entries().collect();
        assert_eq!(entries, vec!["alpha", "beta"]);
    }

    #[test]
    fn history_max_size() {
        let mut h = QuickInputHistory::new(2);
        h.push("a");
        h.push("b");
        h.push("c");
        assert_eq!(h.len(), 2);
        let entries: Vec<&str> = h.entries().collect();
        assert_eq!(entries, vec!["c", "b"]);
    }

    #[test]
    fn history_search() {
        let mut h = QuickInputHistory::new(10);
        h.push("cargo build");
        h.push("cargo test");
        h.push("git status");
        let results = h.search("cargo");
        assert_eq!(results.len(), 2);
        assert!(results.contains(&"cargo build"));
    }

    #[test]
    fn history_empty_push_ignored() {
        let mut h = QuickInputHistory::new(5);
        h.push("");
        assert!(h.is_empty());
    }

    // -- QuickInputButtonBar ------------------------------------------------

    #[test]
    fn button_bar_add_remove() {
        let mut bar = QuickInputButtonBar::new();
        bar.add(InputButton::new("ok", "OK"));
        bar.add(InputButton::new("cancel", "Cancel"));
        assert_eq!(bar.len(), 2);

        assert!(bar.remove("ok"));
        assert_eq!(bar.len(), 1);
        assert!(!bar.remove("nonexistent"));
    }

    #[test]
    fn button_bar_get_and_mutate() {
        let mut bar = QuickInputButtonBar::new();
        bar.add(InputButton::new("save", "Save").tooltip("Save file"));
        assert_eq!(bar.get("save").unwrap().tooltip.as_deref(), Some("Save file"));

        bar.get_mut("save").unwrap().enabled = false;
        assert!(!bar.get("save").unwrap().enabled);
    }

    #[test]
    fn button_bar_render_label() {
        let mut bar = QuickInputButtonBar::new();
        bar.add(InputButton::new("a", "Apply"));
        bar.add(InputButton::new("d", "Discard").enabled(false));
        bar.add(InputButton::new("c", "Close"));
        assert_eq!(bar.render_label(), "[Apply] [Close]");
    }

    #[test]
    fn button_display() {
        let btn = InputButton::new("x", "Go");
        assert_eq!(format!("{btn}"), "[Go]");
        let disabled = InputButton::new("y", "No").enabled(false);
        assert_eq!(format!("{disabled}"), "(No) ");
    }

    // -- QuickPickGrouper ---------------------------------------------------

    #[test]
    fn grouper_from_label_prefix_groups() {
        let g = QuickPickGrouper::from_label_prefix();
        let items = vec![
            make_item("File: Open"),
            make_item("File: Save"),
            make_item("Edit: Copy"),
        ];
        let groups = g.group(&items);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].category, "File");
        assert_eq!(groups[0].items.len(), 2);
    }

    #[test]
    fn grouper_from_description_groups() {
        let g = QuickPickGrouper::from_description();
        let mut item1 = make_item("Open");
        item1.description = Some("Files".into());
        let mut item2 = make_item("Copy");
        item2.description = Some("Edit".into());
        let groups = g.group(&[item1, item2]);
        assert_eq!(groups.len(), 2);
    }

    #[test]
    fn grouper_into_items_adds_separators_v2() {
        let g = QuickPickGrouper::from_label_prefix();
        let items = vec![
            make_item("A: x"),
            make_item("B: y"),
        ];
        let groups = g.group(&items);
        let flat = QuickPickGrouper::into_items(&groups);
        assert!(flat.len() > 2);
    }

    #[test]
    fn grouper_skips_separator_items() {
        let g = QuickPickGrouper::from_label_prefix();
        let items = vec![
            QuickPickItem::separator("sep"),
            make_item("A: x"),
        ];
        let groups = g.group(&items);
        assert_eq!(groups.len(), 1);
    }

    // -- InputValidator -----------------------------------------------------

    #[test]
    fn validator_non_empty() {
        let v = InputValidator::new().non_empty();
        assert!(v.validate("hello").is_none());
        assert!(v.validate("").is_some());
        assert!(v.validate("   ").is_some());
    }

    #[test]
    fn validator_matches_pattern() {
        let v = InputValidator::new().matches_pattern("alphanum");
        assert!(v.validate("hello_world").is_none());
        assert!(v.validate("hello world!").is_some());
    }

    #[test]
    fn validator_chain() {
        let v = InputValidator::new()
            .non_empty()
            .chain(|s| if s.contains("bad") { Some("no bad".into()) } else { None });
        assert!(v.validate("good").is_none());
        assert!(v.validate("bad").is_some());
        assert!(v.validate("").is_some());
    }

    #[test]
    fn validator_max_length_v2() {
        let v = InputValidator::new().max_length(256);
        assert!(v.validate("short").is_none());
    }

    // -- QuickPickHistory ---------------------------------------------------

    #[test]
    fn history_record_and_entries() {
        let mut h = QuickPickHistory::new(10);
        h.record("Build");
        h.record("Test");
        assert_eq!(h.len(), 2);
        assert_eq!(h.entries()[0], "Test"); // most recent first
    }

    #[test]
    fn history_deduplicates() {
        let mut h = QuickPickHistory::new(10);
        h.record("A");
        h.record("B");
        h.record("A");
        assert_eq!(h.len(), 2);
        assert_eq!(h.entries()[0], "A");
    }

    #[test]
    fn history_clear() {
        let mut h = QuickPickHistory::new(10);
        h.record("X");
        h.clear();
        assert!(h.is_empty());
    }

    #[test]
    fn history_capacity_eviction() {
        let mut h = QuickPickHistory::new(2);
        h.record("A");
        h.record("B");
        h.record("C");
        assert_eq!(h.len(), 2);
        assert!(!h.contains("A")); // evicted
        assert!(h.contains("B"));
        assert!(h.contains("C"));
    }

    // -- quickinput additional tests -------------------------------------------

    #[test]
    fn x_quickinput_panel_state_new() {
        let p = XQuickinputPanelState::new(XQuickinputLayoutRegion::Sidebar, "Explorer");
        assert!(p.visible);
        assert_eq!(p.label, "Explorer");
        assert_eq!(p.region, XQuickinputLayoutRegion::Sidebar);
    }

    #[test]
    fn x_quickinput_panel_area() {
        let p = XQuickinputPanelState::new(XQuickinputLayoutRegion::Editor, "ed");
        assert_eq!(p.area(), 300 * 200);
    }

    #[test]
    fn x_quickinput_panel_toggle() {
        let mut p = XQuickinputPanelState::new(XQuickinputLayoutRegion::Panel, "terminal");
        assert!(p.visible);
        p.toggle();
        assert!(!p.visible);
        p.toggle();
        assert!(p.visible);
    }

    #[test]
    fn x_quickinput_panel_resize() {
        let mut p = XQuickinputPanelState::new(XQuickinputLayoutRegion::Sidebar, "files");
        p.resize(400, 600);
        assert_eq!(p.width, 400);
        assert_eq!(p.height, 600);
        assert_eq!(p.area(), 240_000);
    }

    #[test]
    fn x_quickinput_panel_is_narrow() {
        let mut p = XQuickinputPanelState::new(XQuickinputLayoutRegion::Sidebar, "x");
        assert!(!p.is_narrow());
        p.resize(100, 200);
        assert!(p.is_narrow());
    }

    #[test]
    fn x_quickinput_total_visible_area_basic() {
        let panels = vec![
            XQuickinputPanelState::new(XQuickinputLayoutRegion::Sidebar, "a"),
            XQuickinputPanelState::new(XQuickinputLayoutRegion::Editor, "b"),
        ];
        assert_eq!(x_quickinput_total_visible_area(&panels), 2 * 300 * 200);
    }

    #[test]
    fn x_quickinput_total_visible_area_hidden() {
        let mut panels = vec![
            XQuickinputPanelState::new(XQuickinputLayoutRegion::Sidebar, "a"),
            XQuickinputPanelState::new(XQuickinputLayoutRegion::Panel, "b"),
        ];
        panels[1].visible = false;
        assert_eq!(x_quickinput_total_visible_area(&panels), 300 * 200);
    }

    #[test]
    fn x_quickinput_count_in_region_basic() {
        let panels = vec![
            XQuickinputPanelState::new(XQuickinputLayoutRegion::Sidebar, "a"),
            XQuickinputPanelState::new(XQuickinputLayoutRegion::Sidebar, "b"),
            XQuickinputPanelState::new(XQuickinputLayoutRegion::Editor, "c"),
        ];
        assert_eq!(x_quickinput_count_in_region(&panels, XQuickinputLayoutRegion::Sidebar), 2);
        assert_eq!(x_quickinput_count_in_region(&panels, XQuickinputLayoutRegion::Editor), 1);
        assert_eq!(x_quickinput_count_in_region(&panels, XQuickinputLayoutRegion::Panel), 0);
    }

    #[test]
    fn x_quickinput_widest_panel_basic() {
        let mut panels = vec![
            XQuickinputPanelState::new(XQuickinputLayoutRegion::Sidebar, "narrow"),
            XQuickinputPanelState::new(XQuickinputLayoutRegion::Editor, "wide"),
        ];
        panels[1].resize(800, 600);
        let widest = x_quickinput_widest_panel(&panels).unwrap();
        assert_eq!(widest.label, "wide");
    }

    #[test]
    fn x_quickinput_collapse_region_basic() {
        let mut panels = vec![
            XQuickinputPanelState::new(XQuickinputLayoutRegion::Sidebar, "a"),
            XQuickinputPanelState::new(XQuickinputLayoutRegion::Sidebar, "b"),
            XQuickinputPanelState::new(XQuickinputLayoutRegion::Editor, "c"),
        ];
        x_quickinput_collapse_region(&mut panels, XQuickinputLayoutRegion::Sidebar);
        assert!(!panels[0].visible);
        assert!(!panels[1].visible);
        assert!(panels[2].visible);
    }

    #[test]
    fn x_quickinput_layout_constraint_clamp() {
        let lc = XQuickinputLayoutConstraint::new(100, 800, 50, 600);
        assert_eq!(lc.clamp_width(50), 100);
        assert_eq!(lc.clamp_width(500), 500);
        assert_eq!(lc.clamp_width(1000), 800);
        assert_eq!(lc.clamp_height(10), 50);
    }

    #[test]
    fn x_quickinput_layout_constraint_satisfied() {
        let lc = XQuickinputLayoutConstraint::new(100, 800, 50, 600);
        assert!(lc.is_satisfied(400, 300));
        assert!(!lc.is_satisfied(50, 300));
        assert!(!lc.is_satisfied(400, 700));
    }

    #[test]
    fn x_quickinput_widest_panel_empty() {
        let panels: Vec<XQuickinputPanelState> = vec![];
        assert!(x_quickinput_widest_panel(&panels).is_none());
    }

    #[test]
    fn x_quickinput_layout_region_eq() {
        assert_eq!(XQuickinputLayoutRegion::Sidebar, XQuickinputLayoutRegion::Sidebar);
        assert_ne!(XQuickinputLayoutRegion::Sidebar, XQuickinputLayoutRegion::Panel);
    }


    #[test]
    fn quickinput_config_new() {
        let cfg = QuickinputConfig::new();
        assert!(cfg.is_enabled());
        assert_eq!(cfg.version(), 1);
        assert_eq!(cfg.option_count(), 0);
    }

    #[test]
    fn quickinput_config_set_get() {
        let mut cfg = QuickinputConfig::new();
        cfg.set_option("key", "value");
        assert_eq!(cfg.get_option("key"), Some("value"));
        assert!(cfg.has_option("key"));
    }

    #[test]
    fn quickinput_config_remove() {
        let mut cfg = QuickinputConfig::new();
        cfg.set_option("a", "1");
        assert_eq!(cfg.remove_option("a"), Some("1".into()));
        assert!(!cfg.has_option("a"));
    }

    #[test]
    fn quickinput_config_keys_sorted() {
        let mut cfg = QuickinputConfig::new();
        cfg.set_option("z", "1");
        cfg.set_option("a", "2");
        assert_eq!(cfg.option_keys(), vec!["a", "z"]);
    }

    #[test]
    fn quickinput_config_bump_version() {
        let mut cfg = QuickinputConfig::new();
        cfg.bump_version();
        cfg.bump_version();
        assert_eq!(cfg.version(), 3);
    }

    #[test]
    fn quickinput_config_clear() {
        let mut cfg = QuickinputConfig::new();
        cfg.set_option("x", "y");
        cfg.bump_version();
        cfg.clear();
        assert_eq!(cfg.option_count(), 0);
        assert_eq!(cfg.version(), 1);
    }

    #[test]
    fn quickinput_config_merge() {
        let mut cfg1 = QuickinputConfig::new();
        cfg1.set_option("a", "1");
        let mut cfg2 = QuickinputConfig::new();
        cfg2.set_option("b", "2");
        cfg1.merge(&cfg2);
        assert_eq!(cfg1.option_count(), 2);
    }

    #[test]
    fn quickinput_config_disable() {
        let mut cfg = QuickinputConfig::new();
        cfg.set_enabled(false);
        assert!(!cfg.is_enabled());
    }

    #[test]
    fn quickinput_rate_tracker_empty() {
        let rt = QuickinputRateTracker::new(1000);
        assert_eq!(rt.count(), 0);
        assert_eq!(rt.rate_per_second(), 0.0);
    }

    #[test]
    fn quickinput_rate_tracker_record() {
        let mut rt = QuickinputRateTracker::new(1000);
        rt.record(100);
        rt.record(200);
        rt.record(300);
        assert_eq!(rt.count(), 3);
    }

    #[test]
    fn quickinput_rate_tracker_prune() {
        let mut rt = QuickinputRateTracker::new(100);
        rt.record(10);
        rt.record(200);
        assert_eq!(rt.count(), 1);
    }

    #[test]
    fn quickinput_validator_valid() {
        let v = QuickinputValidator::new();
        assert!(v.is_valid());
        assert_eq!(v.error_count(), 0);
    }

    #[test]
    fn quickinput_validator_errors() {
        let mut v = QuickinputValidator::new();
        v.add_error("bad input");
        v.add_warning("slow");
        assert!(!v.is_valid());
        assert_eq!(v.error_count(), 1);
        assert_eq!(v.warning_count(), 1);
        assert_eq!(v.first_error(), Some("bad input"));
    }

    #[test]
    fn quickinput_validator_clear() {
        let mut v = QuickinputValidator::new();
        v.add_error("err");
        v.clear();
        assert!(v.is_valid());
    }

    #[test]
    fn quickinput_validator_merge() {
        let mut v1 = QuickinputValidator::new();
        v1.add_error("e1");
        let mut v2 = QuickinputValidator::new();
        v2.add_error("e2");
        v2.add_warning("w1");
        v1.merge(&v2);
        assert_eq!(v1.error_count(), 2);
        assert_eq!(v1.warning_count(), 1);
    }

    #[test]
    fn quickinput_rate_tracker_clear() {
        let mut rt = QuickinputRateTracker::new(1000);
        rt.record(100);
        rt.clear();
        assert_eq!(rt.count(), 0);
    }

}
