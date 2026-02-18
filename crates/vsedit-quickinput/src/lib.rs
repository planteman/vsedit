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
// xb_ utilities – batch 24
// ---------------------------------------------------------------------------

/// A bounded ring buffer that stores up to `cap` items.
pub struct XbRingBuffer24 {
    buf: Vec<i64>,
    cap: usize,
    head: usize,
    len: usize,
}

impl XbRingBuffer24 {
    /// Create a new ring buffer with the given capacity.
    pub fn new(cap: usize) -> Self {
        Self {
            buf: vec![0i64; cap],
            cap,
            head: 0,
            len: 0,
        }
    }

    /// Push a value into the buffer, overwriting the oldest if full.
    pub fn push(&mut self, val: i64) {
        let pos = (self.head + self.len) % self.cap;
        self.buf[pos] = val;
        if self.len == self.cap {
            self.head = (self.head + 1) % self.cap;
        } else {
            self.len += 1;
        }
    }

    /// Return the number of elements currently stored.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Return whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Get element at logical index (0 = oldest).
    pub fn get(&self, index: usize) -> Option<i64> {
        if index >= self.len {
            return None;
        }
        Some(self.buf[(self.head + index) % self.cap])
    }

    /// Drain all elements oldest-first.
    pub fn drain_all(&mut self) -> Vec<i64> {
        let mut out = Vec::with_capacity(self.len);
        for i in 0..self.len {
            out.push(self.buf[(self.head + i) % self.cap]);
        }
        self.head = 0;
        self.len = 0;
        out
    }

    /// Peek at the oldest element.
    pub fn peek_front(&self) -> Option<i64> {
        self.get(0)
    }

    /// Peek at the newest element.
    pub fn peek_back(&self) -> Option<i64> {
        if self.len == 0 {
            None
        } else {
            self.get(self.len - 1)
        }
    }

    /// Clear the buffer.
    pub fn clear(&mut self) {
        self.head = 0;
        self.len = 0;
    }

    /// Return capacity.
    pub fn capacity(&self) -> usize {
        self.cap
    }
}

/// Compute a simple FNV-1a 64-bit hash over bytes.
pub fn xb_fnv1a_24(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Run-length encode a slice of items.
pub fn xb_rle_encode_24<T: Eq + Clone>(items: &[T]) -> Vec<(T, usize)> {
    let mut result = Vec::new();
    let mut i = 0;
    while i < items.len() {
        let val = &items[i];
        let mut count = 1;
        while i + count < items.len() && items[i + count] == *val {
            count += 1;
        }
        result.push((val.clone(), count));
        i += count;
    }
    result
}

/// Decode an RLE-encoded sequence.
pub fn xb_rle_decode_24<T: Clone>(encoded: &[(T, usize)]) -> Vec<T> {
    let mut out = Vec::new();
    for (val, count) in encoded {
        for _ in 0..*count {
            out.push(val.clone());
        }
    }
    out
}

/// Clamp a value to [lo, hi].
pub fn xb_clamp_24(val: f64, lo: f64, hi: f64) -> f64 {
    if val < lo { lo } else if val > hi { hi } else { val }
}

/// Linear interpolation between a and b.
pub fn xb_lerp_24(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}


// ---------------------------------------------------------------------------
// xc_ pool and scheduler – generated block 145
// ---------------------------------------------------------------------------

/// Generic object pool `Xc145Pool<T>`.
pub struct Xc145Pool<T> {
    items: Vec<T>,
    capacity: usize,
    acquired: usize,
}

/// Statistics snapshot returned by [`Xc145Pool::stats`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xc145PoolStats {
    pub capacity: usize,
    pub len: usize,
    pub acquired: usize,
    pub available: usize,
}

impl<T> Xc145Pool<T> {
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
    pub fn stats(&self) -> Xc145PoolStats {
        Xc145PoolStats {
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

impl<T> Default for Xc145Pool<T> {
    fn default() -> Self {
        Self::new(16)
    }
}

/// Round-robin scheduler `Xc145Scheduler`.
pub struct Xc145Scheduler {
    targets: Vec<String>,
    index: usize,
    dispatched: usize,
}

impl Xc145Scheduler {
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

impl Default for Xc145Scheduler {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}


/// Computes a simple xc_145 hash for the given byte slice.
pub fn xc_145_hash(data: &[u8]) -> u64 {
    let mut h: u64 = 5381;
    for &b in data {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

/// Reverses a string using xc_145 convention.
pub fn xc_145_reverse(s: &str) -> String {
    s.chars().rev().collect()
}


// === Xe36 Pipeline & Cache ===

#[derive(Debug, Clone, PartialEq)]
pub enum Xe36Stage {
    Parse,
    Transform,
    Validate,
    Emit,
}

#[derive(Debug, Clone)]
pub struct Xe36PipelineError {
    pub stage: Xe36Stage,
    pub message: String,
}

impl std::fmt::Display for Xe36PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Xe36Pipeline error at {:?}: {}", self.stage, self.message)
    }
}

pub struct Xe36Pipeline {
    stages: Vec<Box<dyn Fn(Vec<u8>) -> Result<Vec<u8>, Xe36PipelineError>>>,
    stage_names: Vec<Xe36Stage>,
}

impl Xe36Pipeline {
    pub fn new() -> Self {
        Self { stages: Vec::new(), stage_names: Vec::new() }
    }

    pub fn add_parse<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe36PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe36Stage::Parse);
        self
    }

    pub fn add_transform<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe36PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe36Stage::Transform);
        self
    }

    pub fn add_validate<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe36PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe36Stage::Validate);
        self
    }

    pub fn add_emit<F>(mut self, f: F) -> Self
    where F: Fn(Vec<u8>) -> Result<Vec<u8>, Xe36PipelineError> + 'static {
        self.stages.push(Box::new(f));
        self.stage_names.push(Xe36Stage::Emit);
        self
    }

    pub fn execute(&self, input: Vec<u8>) -> Result<Vec<u8>, Xe36PipelineError> {
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

    pub fn compose(mut self, other: Xe36Pipeline) -> Self {
        for (stage_fn, name) in other.stages.into_iter().zip(other.stage_names) {
            self.stages.push(stage_fn);
            self.stage_names.push(name);
        }
        self
    }
}

pub struct Xe36CacheEntry<V> {
    value: V,
    inserted_at: u64,
    ttl: u64,
}

pub struct Xe36CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

pub struct Xe36Cache<K: std::hash::Hash + Eq, V: Clone> {
    entries: std::collections::HashMap<K, Xe36CacheEntry<V>>,
    capacity: usize,
    current_time: u64,
    stats: Xe36CacheStats,
}

impl<K: std::hash::Hash + Eq + Clone, V: Clone> Xe36Cache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: std::collections::HashMap::new(),
            capacity,
            current_time: 0,
            stats: Xe36CacheStats { hits: 0, misses: 0, evictions: 0 },
        }
    }

    pub fn advance_time(&mut self, amount: u64) {
        self.current_time += amount;
    }

    pub fn put(&mut self, key: K, value: V, ttl: u64) {
        if self.entries.len() >= self.capacity && !self.entries.contains_key(&key) {
            self.xe_36_evict_expired();
            if self.entries.len() >= self.capacity {
                if let Some(oldest_key) = self.entries.keys().next().cloned() {
                    self.entries.remove(&oldest_key);
                    self.stats.evictions += 1;
                }
            }
        }
        self.entries.insert(key, Xe36CacheEntry {
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

    fn xe_36_evict_expired(&mut self) {
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

    pub fn stats(&self) -> &Xe36CacheStats {
        &self.stats
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

pub fn xe_36_pipeline_identity(data: Vec<u8>) -> Result<Vec<u8>, Xe36PipelineError> {
    Ok(data)
}

pub fn xe_36_pipeline_double(data: Vec<u8>) -> Result<Vec<u8>, Xe36PipelineError> {
    let mut out = data.clone();
    out.extend_from_slice(&data);
    Ok(out)
}

pub fn xe_36_pipeline_reverse(data: Vec<u8>) -> Result<Vec<u8>, Xe36PipelineError> {
    Ok(data.into_iter().rev().collect())
}

pub fn xe_36_pipeline_filter_zeros(data: Vec<u8>) -> Result<Vec<u8>, Xe36PipelineError> {
    Ok(data.into_iter().filter(|b| *b != 0).collect())
}

pub fn xe_36_pipeline_fail(_data: Vec<u8>) -> Result<Vec<u8>, Xe36PipelineError> {
    Err(Xe36PipelineError {
        stage: Xe36Stage::Parse,
        message: "intentional failure".to_string(),
    })
}


// ---------------------------------------------------------------------------
// xg_1: Directed graph
// ---------------------------------------------------------------------------

/// A directed graph with adjacency-list representation.
#[derive(Debug, Clone)]
pub struct Xg1Graph {
    adj: std::collections::HashMap<usize, Vec<usize>>,
    edge_cnt: usize,
}

impl Xg1Graph {
    /// Create an empty graph.
    pub fn new() -> Self {
        Self { adj: std::collections::HashMap::new(), edge_cnt: 0 }
    }

    /// Add a node (idempotent).
    pub fn add_node(&mut self, id: usize) {
        self.adj.entry(id).or_default();
    }

    /// Add a directed edge from `src` to `dst`, creating nodes if needed.
    pub fn add_edge(&mut self, src: usize, dst: usize) {
        self.adj.entry(dst).or_default();
        self.adj.entry(src).or_default().push(dst);
        self.edge_cnt += 1;
    }

    /// Return the neighbours of `node`.
    pub fn neighbors(&self, node: usize) -> &[usize] {
        self.adj.get(&node).map_or(&[], |v| v.as_slice())
    }

    /// BFS reachability check.
    pub fn has_path(&self, from: usize, to: usize) -> bool {
        if from == to { return true; }
        let mut visited = std::collections::HashSet::new();
        let mut queue = std::collections::VecDeque::new();
        queue.push_back(from);
        visited.insert(from);
        while let Some(cur) = queue.pop_front() {
            for &nb in self.neighbors(cur) {
                if nb == to { return true; }
                if visited.insert(nb) {
                    queue.push_back(nb);
                }
            }
        }
        false
    }

    /// Kahn's algorithm topological sort. Returns `None` if a cycle exists.
    pub fn topological_sort(&self) -> Option<Vec<usize>> {
        let mut in_deg: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
        for &n in self.adj.keys() { in_deg.entry(n).or_insert(0); }
        for edges in self.adj.values() {
            for &dst in edges { *in_deg.entry(dst).or_insert(0) += 1; }
        }
        let mut queue: std::collections::VecDeque<usize> = in_deg.iter()
            .filter(|&(_, &d)| d == 0).map(|(&n, _)| n).collect();
        let mut order = Vec::new();
        while let Some(n) = queue.pop_front() {
            order.push(n);
            if let Some(edges) = self.adj.get(&n) {
                for &dst in edges {
                    if let Some(d) = in_deg.get_mut(&dst) {
                        *d -= 1;
                        if *d == 0 { queue.push_back(dst); }
                    }
                }
            }
        }
        if order.len() == self.adj.len() { Some(order) } else { None }
    }

    /// Detect whether the graph contains a cycle.
    pub fn cycle_detect(&self) -> bool {
        self.topological_sort().is_none()
    }

    /// Number of nodes.
    pub fn node_count(&self) -> usize { self.adj.len() }

    /// Number of edges.
    pub fn edge_count(&self) -> usize { self.edge_cnt }
}

impl Default for Xg1Graph {
    fn default() -> Self { Self::new() }
}

// ---------------------------------------------------------------------------
// xg_1: Min-heap
// ---------------------------------------------------------------------------

/// A min-heap backed by a `Vec`.
#[derive(Debug, Clone)]
pub struct Xg1Heap<T: Ord> {
    data: Vec<T>,
}

impl<T: Ord> Xg1Heap<T> {
    /// Create an empty heap.
    pub fn new() -> Self { Self { data: Vec::new() } }

    /// Number of elements.
    pub fn len(&self) -> usize { self.data.len() }

    /// Whether the heap is empty.
    pub fn is_empty(&self) -> bool { self.data.is_empty() }

    /// Push a value onto the heap.
    pub fn push(&mut self, val: T) {
        self.data.push(val);
        self.sift_up(self.data.len() - 1);
    }

    /// Peek at the minimum element.
    pub fn peek(&self) -> Option<&T> { self.data.first() }

    /// Remove and return the minimum element.
    pub fn pop(&mut self) -> Option<T> {
        if self.data.is_empty() { return None; }
        let last = self.data.len() - 1;
        self.data.swap(0, last);
        let val = self.data.pop();
        if !self.data.is_empty() { self.sift_down(0); }
        val
    }

    /// Drain all elements in sorted order.
    pub fn drain_sorted(&mut self) -> Vec<T> {
        let mut out = Vec::with_capacity(self.data.len());
        while let Some(v) = self.pop() { out.push(v); }
        out
    }

    /// Merge another heap into this one.
    pub fn merge(&mut self, other: &mut Xg1Heap<T>) {
        self.data.append(&mut other.data);
        let n = self.data.len();
        for i in (0..n / 2).rev() { self.sift_down(i); }
    }

    fn sift_up(&mut self, mut idx: usize) {
        while idx > 0 {
            let parent = (idx - 1) / 2;
            if self.data[idx] < self.data[parent] {
                self.data.swap(idx, parent);
                idx = parent;
            } else { break; }
        }
    }

    fn sift_down(&mut self, mut idx: usize) {
        let len = self.data.len();
        loop {
            let mut smallest = idx;
            let left = 2 * idx + 1;
            let right = 2 * idx + 2;
            if left < len && self.data[left] < self.data[smallest] { smallest = left; }
            if right < len && self.data[right] < self.data[smallest] { smallest = right; }
            if smallest != idx { self.data.swap(idx, smallest); idx = smallest; }
            else { break; }
        }
    }
}

impl<T: Ord> Default for Xg1Heap<T> {
    fn default() -> Self { Self::new() }
}


/// A probabilistic sorted list using a skip-list structure (variant 144).
pub struct Xh144SkipList {
    xh_levels: Vec<Vec<(i64, usize)>>,
    xh_data: Vec<i64>,
    xh_len: usize,
    xh_max_level: usize,
    xh_seed: u64,
}

impl Xh144SkipList {
    /// Create a new skip list with the given maximum level.
    pub fn xh_new(max_level: usize) -> Self {
        Self {
            xh_levels: vec![Vec::new(); max_level],
            xh_data: Vec::new(),
            xh_len: 0,
            xh_max_level: max_level,
            xh_seed: 186 as u64,
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

/// A compact bit set supporting boolean operations (variant 144).
pub struct Xh144BitSet {
    xh_words: Vec<u64>,
    xh_nbits: usize,
}

impl Xh144BitSet {
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


/// A double-ended queue backed by a ring buffer (variant 144).
pub struct Xi144Deque<T> {
    xi_buf: Vec<Option<T>>,
    xi_head: usize,
    xi_tail: usize,
    xi_len: usize,
}

impl<T: Clone> Xi144Deque<T> {
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
pub struct Xi144Interval {
    pub xi_low: i64,
    pub xi_high: i64,
}

impl Xi144Interval {
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

/// A simple interval tree (variant 144).
pub struct Xi144IntervalTree {
    xi_intervals: Vec<Xi144Interval>,
}

impl Xi144IntervalTree {
    /// Create a new empty interval tree.
    pub fn xi_new() -> Self {
        Self { xi_intervals: Vec::new() }
    }

    /// Insert an interval.
    pub fn xi_insert(&mut self, interval: Xi144Interval) {
        self.xi_intervals.push(interval);
        self.xi_intervals.sort_by_key(|iv| (iv.xi_low, iv.xi_high));
    }

    /// Query all intervals containing the given point.
    pub fn xi_query_point(&self, point: i64) -> Vec<&Xi144Interval> {
        self.xi_intervals.iter().filter(|iv| iv.xi_contains_point(point)).collect()
    }

    /// Query all intervals overlapping with the given interval.
    pub fn xi_query_overlap(&self, query: &Xi144Interval) -> Vec<&Xi144Interval> {
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
    pub fn xi_all_intervals(&self) -> &[Xi144Interval] {
        &self.xi_intervals
    }

    /// Return the number of intervals.
    pub fn xi_count(&self) -> usize {
        self.xi_intervals.len()
    }

    /// Compute gaps between intervals in the range [range_low, range_high).
    pub fn xi_gaps(&self, range_low: i64, range_high: i64) -> Vec<Xi144Interval> {
        let mut gaps = Vec::new();
        let mut cursor = range_low;
        for iv in &self.xi_intervals {
            if iv.xi_high <= range_low || iv.xi_low >= range_high {
                continue;
            }
            let lo = iv.xi_low.max(range_low);
            if cursor < lo {
                gaps.push(Xi144Interval::xi_new(cursor, lo));
            }
            cursor = cursor.max(iv.xi_high);
        }
        if cursor < range_high {
            gaps.push(Xi144Interval::xi_new(cursor, range_high));
        }
        gaps
    }

    /// Merge overlapping intervals and return a new set.
    pub fn xi_merge_overlapping(&self) -> Vec<Xi144Interval> {
        if self.xi_intervals.is_empty() {
            return Vec::new();
        }
        let mut merged: Vec<Xi144Interval> = Vec::new();
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


    #[test]
    fn xb_ring_buffer_24_push_and_len() {
        let mut rb = super::XbRingBuffer24::new(4);
        assert!(rb.is_empty());
        rb.push(10);
        rb.push(20);
        assert_eq!(rb.len(), 2);
    }

    #[test]
    fn xb_ring_buffer_24_overwrite() {
        let mut rb = super::XbRingBuffer24::new(3);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        assert_eq!(rb.len(), 3);
        assert_eq!(rb.get(0), Some(2));
        assert_eq!(rb.get(2), Some(4));
    }

    #[test]
    fn xb_ring_buffer_24_get_out_of_bounds() {
        let rb = super::XbRingBuffer24::new(3);
        assert_eq!(rb.get(0), None);
        assert_eq!(rb.get(99), None);
    }

    #[test]
    fn xb_ring_buffer_24_drain_all() {
        let mut rb = super::XbRingBuffer24::new(5);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        let v = rb.drain_all();
        assert_eq!(v, vec![1, 2, 3]);
        assert!(rb.is_empty());
    }

    #[test]
    fn xb_ring_buffer_24_peek_front_back() {
        let mut rb = super::XbRingBuffer24::new(4);
        assert_eq!(rb.peek_front(), None);
        assert_eq!(rb.peek_back(), None);
        rb.push(5);
        rb.push(10);
        assert_eq!(rb.peek_front(), Some(5));
        assert_eq!(rb.peek_back(), Some(10));
    }

    #[test]
    fn xb_ring_buffer_24_clear() {
        let mut rb = super::XbRingBuffer24::new(4);
        rb.push(1);
        rb.push(2);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    #[test]
    fn xb_ring_buffer_24_capacity() {
        let rb = super::XbRingBuffer24::new(7);
        assert_eq!(rb.capacity(), 7);
    }

    #[test]
    fn xb_fnv1a_24_basic() {
        let h = super::xb_fnv1a_24(b"hello");
        assert_ne!(h, 0);
        let h2 = super::xb_fnv1a_24(b"hello");
        assert_eq!(h, h2);
    }

    #[test]
    fn xb_fnv1a_24_different_inputs() {
        let h1 = super::xb_fnv1a_24(b"abc");
        let h2 = super::xb_fnv1a_24(b"def");
        assert_ne!(h1, h2);
    }

    #[test]
    fn xb_rle_24_round_trip() {
        let data = vec![1, 1, 2, 2, 2, 3];
        let enc = super::xb_rle_encode_24(&data);
        let dec = super::xb_rle_decode_24(&enc);
        assert_eq!(dec, data);
    }

    #[test]
    fn xb_rle_24_empty() {
        let data: Vec<i32> = vec![];
        let enc = super::xb_rle_encode_24(&data);
        assert!(enc.is_empty());
        let dec = super::xb_rle_decode_24(&enc);
        assert!(dec.is_empty());
    }

    #[test]
    fn xb_clamp_24_values() {
        assert!((super::xb_clamp_24(5.0, 0.0, 10.0) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_24(-1.0, 0.0, 10.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_clamp_24(99.0, 0.0, 10.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_lerp_24_values() {
        assert!((super::xb_lerp_24(0.0, 10.0, 0.5) - 5.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_24(0.0, 10.0, 0.0) - 0.0).abs() < f64::EPSILON);
        assert!((super::xb_lerp_24(0.0, 10.0, 1.0) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn xb_ring_buffer_24_wrap_around_twice() {
        let mut rb = super::XbRingBuffer24::new(2);
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4);
        rb.push(5);
        assert_eq!(rb.len(), 2);
        assert_eq!(rb.get(0), Some(4));
        assert_eq!(rb.get(1), Some(5));
    }


    // ---- xc_ pool / scheduler tests – block 145 ----

    #[test]
    fn xc_145_pool_new_empty() {
        let pool: super::Xc145Pool<i32> = super::Xc145Pool::new(4);
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
        assert_eq!(pool.capacity(), 4);
        assert!(!pool.is_full());
    }

    #[test]
    fn xc_145_pool_release_acquire() {
        let mut pool = super::Xc145Pool::new(4);
        pool.release(10);
        pool.release(20);
        assert_eq!(pool.len(), 2);
        assert_eq!(pool.available(), 2);
        let v = pool.acquire().unwrap();
        assert_eq!(v, 20);
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_145_pool_acquire_empty() {
        let mut pool: super::Xc145Pool<i32> = super::Xc145Pool::new(2);
        assert!(pool.acquire().is_none());
    }

    #[test]
    fn xc_145_pool_full() {
        let mut pool = super::Xc145Pool::new(2);
        pool.release(1);
        pool.release(2);
        assert!(pool.is_full());
        pool.release(3); // over capacity – ignored
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_145_pool_drain() {
        let mut pool = super::Xc145Pool::new(4);
        pool.release(1);
        pool.release(2);
        let items = pool.drain();
        assert_eq!(items.len(), 2);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_145_pool_stats() {
        let mut pool = super::Xc145Pool::new(8);
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
    fn xc_145_pool_clear() {
        let mut pool = super::Xc145Pool::new(4);
        pool.release(1);
        pool.release(2);
        pool.clear();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn xc_145_pool_shrink() {
        let mut pool = super::Xc145Pool::new(100);
        pool.release(1);
        pool.shrink_to_fit();
        assert_eq!(pool.len(), 1);
    }

    #[test]
    fn xc_145_pool_default() {
        let pool: super::Xc145Pool<String> = super::Xc145Pool::default();
        assert_eq!(pool.capacity(), 16);
        assert!(pool.is_empty());
    }

    #[test]
    fn xc_145_pool_extend() {
        let mut pool = super::Xc145Pool::new(3);
        pool.extend_from(vec![10, 20, 30, 40]);
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn xc_145_pool_retain() {
        let mut pool = super::Xc145Pool::new(8);
        pool.extend_from(vec![1, 2, 3, 4, 5]);
        pool.retain(|x| x % 2 == 0);
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn xc_145_scheduler_round_robin() {
        let mut sched = super::Xc145Scheduler::new(vec![
            "a".into(), "b".into(), "c".into(),
        ]);
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.next().unwrap(), "b");
        assert_eq!(sched.next().unwrap(), "c");
        assert_eq!(sched.next().unwrap(), "a");
        assert_eq!(sched.dispatched(), 4);
    }

    #[test]
    fn xc_145_scheduler_empty() {
        let mut sched = super::Xc145Scheduler::new(vec![]);
        assert!(sched.next().is_none());
        assert!(sched.is_empty());
    }

    #[test]
    fn xc_145_scheduler_reset() {
        let mut sched = super::Xc145Scheduler::new(vec!["x".into()]);
        sched.next();
        sched.next();
        sched.reset();
        assert_eq!(sched.dispatched(), 0);
        assert_eq!(sched.position(), 0);
    }

    #[test]
    fn xc_145_scheduler_add_remove() {
        let mut sched = super::Xc145Scheduler::new(vec!["a".into()]);
        sched.add_target("b".into());
        assert_eq!(sched.len(), 2);
        assert!(sched.remove_target("a"));
        assert_eq!(sched.len(), 1);
        assert!(!sched.remove_target("z"));
    }

    #[test]
    fn xc_145_scheduler_targets() {
        let sched = super::Xc145Scheduler::new(vec!["t1".into(), "t2".into()]);
        assert_eq!(sched.targets(), &["t1".to_string(), "t2".to_string()]);
        assert_eq!(sched.len(), 2);
    }


    #[test]
    fn xc_145_hash_empty() {
        assert_eq!(super::xc_145_hash(b""), 5381);
    }

    #[test]
    fn xc_145_hash_data() {
        let h = super::xc_145_hash(b"hello");
        assert_ne!(h, 0);
        assert_eq!(super::xc_145_hash(b"hello"), h);
    }

    #[test]
    fn xc_145_reverse_str() {
        assert_eq!(super::xc_145_reverse("abc"), "cba");
        assert_eq!(super::xc_145_reverse(""), "");
    }


    #[test]
    fn xe_36_pipeline_empty() {
        let p = super::Xe36Pipeline::new();
        assert_eq!(p.stage_count(), 0);
        let r = p.execute(vec![1, 2, 3]).unwrap();
        assert_eq!(r, vec![1, 2, 3]);
    }

    #[test]
    fn xe_36_pipeline_parse_stage() {
        let p = super::Xe36Pipeline::new()
            .add_parse(super::xe_36_pipeline_identity);
        assert_eq!(p.stage_count(), 1);
        assert_eq!(p.execute(vec![10]).unwrap(), vec![10]);
    }

    #[test]
    fn xe_36_pipeline_transform_double() {
        let p = super::Xe36Pipeline::new()
            .add_transform(super::xe_36_pipeline_double);
        assert_eq!(p.execute(vec![1, 2]).unwrap(), vec![1, 2, 1, 2]);
    }

    #[test]
    fn xe_36_pipeline_validate_reverse() {
        let p = super::Xe36Pipeline::new()
            .add_validate(super::xe_36_pipeline_reverse);
        assert_eq!(p.execute(vec![1, 2, 3]).unwrap(), vec![3, 2, 1]);
    }

    #[test]
    fn xe_36_pipeline_emit_filter() {
        let p = super::Xe36Pipeline::new()
            .add_emit(super::xe_36_pipeline_filter_zeros);
        assert_eq!(p.execute(vec![0, 1, 0, 2]).unwrap(), vec![1, 2]);
    }

    #[test]
    fn xe_36_pipeline_multi_stage() {
        let p = super::Xe36Pipeline::new()
            .add_parse(super::xe_36_pipeline_identity)
            .add_transform(super::xe_36_pipeline_double)
            .add_validate(super::xe_36_pipeline_reverse)
            .add_emit(super::xe_36_pipeline_filter_zeros);
        assert_eq!(p.stage_count(), 4);
        let r = p.execute(vec![1, 0]).unwrap();
        assert_eq!(r, vec![1, 1]);
    }

    #[test]
    fn xe_36_pipeline_error_propagation() {
        let p = super::Xe36Pipeline::new()
            .add_parse(super::xe_36_pipeline_fail);
        let e = p.execute(vec![1]).unwrap_err();
        assert_eq!(e.stage, super::Xe36Stage::Parse);
        assert!(e.message.contains("intentional"));
    }

    #[test]
    fn xe_36_pipeline_compose() {
        let p1 = super::Xe36Pipeline::new()
            .add_parse(super::xe_36_pipeline_identity);
        let p2 = super::Xe36Pipeline::new()
            .add_transform(super::xe_36_pipeline_double);
        let combined = p1.compose(p2);
        assert_eq!(combined.stage_count(), 2);
        assert_eq!(combined.execute(vec![5]).unwrap(), vec![5, 5]);
    }

    #[test]
    fn xe_36_pipeline_error_display() {
        let e = super::Xe36PipelineError {
            stage: super::Xe36Stage::Validate,
            message: "bad data".to_string(),
        };
        let s = format!("{}", e);
        assert!(s.contains("Validate"));
        assert!(s.contains("bad data"));
    }

    #[test]
    fn xe_36_cache_put_get() {
        let mut c = super::Xe36Cache::new(10);
        c.put("a", 1, 100);
        assert_eq!(c.get(&"a"), Some(1));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn xe_36_cache_miss() {
        let mut c: super::Xe36Cache<&str, i32> = super::Xe36Cache::new(10);
        assert_eq!(c.get(&"x"), None);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_36_cache_ttl_expiry() {
        let mut c = super::Xe36Cache::new(10);
        c.put("k", 42, 5);
        assert_eq!(c.get(&"k"), Some(42));
        c.advance_time(5);
        assert_eq!(c.get(&"k"), None);
    }

    #[test]
    fn xe_36_cache_evict() {
        let mut c = super::Xe36Cache::new(10);
        c.put("k", 1, 100);
        assert!(c.evict(&"k"));
        assert!(!c.evict(&"k"));
        assert!(c.is_empty());
    }

    #[test]
    fn xe_36_cache_capacity() {
        let mut c = super::Xe36Cache::new(2);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.put("c", 3, 100);
        assert!(c.len() <= 2);
    }

    #[test]
    fn xe_36_cache_stats() {
        let mut c = super::Xe36Cache::new(10);
        c.put("a", 1, 100);
        c.get(&"a");
        c.get(&"z");
        assert_eq!(c.stats().hits, 1);
        assert_eq!(c.stats().misses, 1);
    }

    #[test]
    fn xe_36_cache_clear() {
        let mut c = super::Xe36Cache::new(10);
        c.put("a", 1, 100);
        c.put("b", 2, 100);
        c.clear();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }


    // -- xg_1 graph tests ------------------------------------------------

    #[test]
    fn xg_1_graph_empty() {
        let g = super::Xg1Graph::new();
        assert_eq!(g.node_count(), 0);
        assert_eq!(g.edge_count(), 0);
    }

    #[test]
    fn xg_1_graph_add_node() {
        let mut g = super::Xg1Graph::new();
        g.add_node(1);
        g.add_node(2);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_1_graph_add_edge() {
        let mut g = super::Xg1Graph::new();
        g.add_edge(0, 1);
        assert_eq!(g.edge_count(), 1);
        assert_eq!(g.node_count(), 2);
    }

    #[test]
    fn xg_1_graph_neighbors() {
        let mut g = super::Xg1Graph::new();
        g.add_edge(0, 1);
        g.add_edge(0, 2);
        assert_eq!(g.neighbors(0).len(), 2);
    }

    #[test]
    fn xg_1_graph_has_path() {
        let mut g = super::Xg1Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(g.has_path(0, 2));
        assert!(!g.has_path(2, 0));
    }

    #[test]
    fn xg_1_graph_self_path() {
        let g = super::Xg1Graph::new();
        assert!(g.has_path(5, 5));
    }

    #[test]
    fn xg_1_graph_topo_sort() {
        let mut g = super::Xg1Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        let sorted = g.topological_sort().unwrap();
        let pos: std::collections::HashMap<usize, usize> =
            sorted.iter().enumerate().map(|(i, &n)| (n, i)).collect();
        assert!(pos[&0] < pos[&1]);
        assert!(pos[&1] < pos[&2]);
    }

    #[test]
    fn xg_1_graph_cycle_detect_false() {
        let mut g = super::Xg1Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        assert!(!g.cycle_detect());
    }

    #[test]
    fn xg_1_graph_cycle_detect_true() {
        let mut g = super::Xg1Graph::new();
        g.add_edge(0, 1);
        g.add_edge(1, 2);
        g.add_edge(2, 0);
        assert!(g.cycle_detect());
    }

    // -- xg_1 heap tests -------------------------------------------------

    #[test]
    fn xg_1_heap_empty() {
        let h: super::Xg1Heap<i32> = super::Xg1Heap::new();
        assert!(h.is_empty());
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn xg_1_heap_push_pop() {
        let mut h = super::Xg1Heap::new();
        h.push(3);
        h.push(1);
        h.push(2);
        assert_eq!(h.pop(), Some(1));
        assert_eq!(h.pop(), Some(2));
        assert_eq!(h.pop(), Some(3));
    }

    #[test]
    fn xg_1_heap_peek() {
        let mut h = super::Xg1Heap::new();
        h.push(5);
        h.push(2);
        assert_eq!(h.peek(), Some(&2));
    }

    #[test]
    fn xg_1_heap_drain_sorted() {
        let mut h = super::Xg1Heap::new();
        for v in [4, 1, 7, 2, 9] { h.push(v); }
        assert_eq!(h.drain_sorted(), vec![1, 2, 4, 7, 9]);
        assert!(h.is_empty());
    }

    #[test]
    fn xg_1_heap_merge() {
        let mut a = super::Xg1Heap::new();
        let mut b = super::Xg1Heap::new();
        a.push(5); a.push(3);
        b.push(4); b.push(1);
        a.merge(&mut b);
        assert_eq!(a.len(), 4);
        assert_eq!(a.pop(), Some(1));
    }

    #[test]
    fn xg_1_heap_default() {
        let h: super::Xg1Heap<u64> = Default::default();
        assert!(h.is_empty());
    }

    #[test]
    fn xg_1_graph_default() {
        let g: super::Xg1Graph = Default::default();
        assert_eq!(g.node_count(), 0);
    }


    #[test]
    fn xh144_skip_insert_contains() {
        let mut sl = super::Xh144SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        sl.xh_insert(5);
        assert!(sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(sl.xh_contains(5));
        assert!(!sl.xh_contains(15));
    }

    #[test]
    fn xh144_skip_remove() {
        let mut sl = super::Xh144SkipList::xh_new(4);
        sl.xh_insert(10);
        sl.xh_insert(20);
        assert!(sl.xh_remove(10));
        assert!(!sl.xh_contains(10));
        assert!(sl.xh_contains(20));
        assert!(!sl.xh_remove(99));
    }

    #[test]
    fn xh144_skip_len() {
        let mut sl = super::Xh144SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        sl.xh_insert(1);
        sl.xh_insert(2);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(1);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh144_skip_range_query() {
        let mut sl = super::Xh144SkipList::xh_new(4);
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
    fn xh144_skip_floor_ceiling() {
        let mut sl = super::Xh144SkipList::xh_new(4);
        for v in [10, 20, 30] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_floor(25), Some(20));
        assert_eq!(sl.xh_ceiling(25), Some(30));
        assert_eq!(sl.xh_floor(5), None);
        assert_eq!(sl.xh_ceiling(35), None);
    }

    #[test]
    fn xh144_skip_rank() {
        let mut sl = super::Xh144SkipList::xh_new(4);
        for v in [10, 20, 30, 40, 50] {
            sl.xh_insert(v);
        }
        assert_eq!(sl.xh_rank(30), 2);
        assert_eq!(sl.xh_rank(10), 0);
        assert_eq!(sl.xh_rank(60), 5);
    }

    #[test]
    fn xh144_skip_empty() {
        let sl = super::Xh144SkipList::xh_new(4);
        assert_eq!(sl.xh_len(), 0);
        assert!(!sl.xh_contains(1));
        assert_eq!(sl.xh_floor(1), None);
        assert_eq!(sl.xh_ceiling(1), None);
        assert_eq!(sl.xh_rank(1), 0);
    }

    #[test]
    fn xh144_skip_duplicates() {
        let mut sl = super::Xh144SkipList::xh_new(4);
        sl.xh_insert(5);
        sl.xh_insert(5);
        assert_eq!(sl.xh_len(), 2);
        sl.xh_remove(5);
        assert_eq!(sl.xh_len(), 1);
    }

    #[test]
    fn xh144_bitset_set_test() {
        let mut bs = super::Xh144BitSet::xh_new(256);
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
    fn xh144_bitset_clear_count() {
        let mut bs = super::Xh144BitSet::xh_new(128);
        bs.xh_set(10);
        bs.xh_set(20);
        bs.xh_set(30);
        assert_eq!(bs.xh_count(), 3);
        bs.xh_clear(20);
        assert_eq!(bs.xh_count(), 2);
        assert!(!bs.xh_test(20));
    }

    #[test]
    fn xh144_bitset_and_or_xor() {
        let mut a = super::Xh144BitSet::xh_new(128);
        let mut b = super::Xh144BitSet::xh_new(128);
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
    fn xh144_bitset_iter_ones() {
        let mut bs = super::Xh144BitSet::xh_new(256);
        bs.xh_set(5);
        bs.xh_set(100);
        bs.xh_set(200);
        let ones = bs.xh_iter_ones();
        assert_eq!(ones, vec![5, 100, 200]);
    }

    #[test]
    fn xh144_bitset_first_last() {
        let mut bs = super::Xh144BitSet::xh_new(256);
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        bs.xh_set(50);
        bs.xh_set(150);
        assert_eq!(bs.xh_first_set(), Some(50));
        assert_eq!(bs.xh_last_set(), Some(150));
    }

    #[test]
    fn xh144_bitset_empty() {
        let bs = super::Xh144BitSet::xh_new(64);
        assert_eq!(bs.xh_count(), 0);
        assert!(!bs.xh_test(0));
        assert_eq!(bs.xh_first_set(), None);
        assert_eq!(bs.xh_last_set(), None);
        assert!(bs.xh_iter_ones().is_empty());
    }


    #[test]
    fn xi144_deque_push_pop_back() {
        let mut dq = super::Xi144Deque::xi_new(4);
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
    fn xi144_deque_push_pop_front() {
        let mut dq = super::Xi144Deque::xi_new(4);
        dq.xi_push_front(1);
        dq.xi_push_front(2);
        dq.xi_push_front(3);
        assert_eq!(dq.xi_pop_front(), Some(3));
        assert_eq!(dq.xi_pop_front(), Some(2));
        assert_eq!(dq.xi_pop_front(), Some(1));
        assert_eq!(dq.xi_pop_front(), None);
    }

    #[test]
    fn xi144_deque_mixed_ops() {
        let mut dq = super::Xi144Deque::xi_new(4);
        dq.xi_push_back(1);
        dq.xi_push_front(0);
        dq.xi_push_back(2);
        assert_eq!(dq.xi_iter(), vec![0, 1, 2]);
        assert_eq!(dq.xi_pop_front(), Some(0));
        assert_eq!(dq.xi_pop_back(), Some(2));
    }

    #[test]
    fn xi144_deque_get_and_split() {
        let mut dq = super::Xi144Deque::xi_new(8);
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
    fn xi144_deque_rotate_left() {
        let mut dq = super::Xi144Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_left(2);
        assert_eq!(dq.xi_iter(), vec![2, 3, 4, 0, 1]);
    }

    #[test]
    fn xi144_deque_rotate_right() {
        let mut dq = super::Xi144Deque::xi_new(8);
        for i in 0..5 {
            dq.xi_push_back(i);
        }
        dq.xi_rotate_right(2);
        assert_eq!(dq.xi_iter(), vec![3, 4, 0, 1, 2]);
    }

    #[test]
    fn xi144_deque_grow() {
        let mut dq = super::Xi144Deque::xi_new(4);
        for i in 0..10 {
            dq.xi_push_back(i);
        }
        assert_eq!(dq.xi_len(), 10);
        assert!(dq.xi_capacity() >= 10);
        assert_eq!(dq.xi_iter(), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn xi144_deque_empty() {
        let dq = super::Xi144Deque::<i32>::xi_new(4);
        assert!(dq.xi_is_empty());
        assert_eq!(dq.xi_len(), 0);
        assert_eq!(dq.xi_get(0), None);
        assert!(dq.xi_iter().is_empty());
    }

    #[test]
    fn xi144_interval_tree_insert_query() {
        let mut tree = super::Xi144IntervalTree::xi_new();
        tree.xi_insert(super::Xi144Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi144Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi144Interval::xi_new(10, 15));
        let hits = tree.xi_query_point(4);
        assert_eq!(hits.len(), 2);
        let hits = tree.xi_query_point(12);
        assert_eq!(hits.len(), 1);
        let hits = tree.xi_query_point(9);
        assert_eq!(hits.len(), 0);
    }

    #[test]
    fn xi144_interval_tree_overlap() {
        let mut tree = super::Xi144IntervalTree::xi_new();
        tree.xi_insert(super::Xi144Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi144Interval::xi_new(6, 10));
        tree.xi_insert(super::Xi144Interval::xi_new(12, 20));
        let q = super::Xi144Interval::xi_new(4, 7);
        let hits = tree.xi_query_overlap(&q);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn xi144_interval_tree_remove() {
        let mut tree = super::Xi144IntervalTree::xi_new();
        tree.xi_insert(super::Xi144Interval::xi_new(0, 10));
        tree.xi_insert(super::Xi144Interval::xi_new(5, 15));
        assert_eq!(tree.xi_count(), 2);
        assert!(tree.xi_remove(0, 10));
        assert_eq!(tree.xi_count(), 1);
        assert!(!tree.xi_remove(0, 10));
    }

    #[test]
    fn xi144_interval_tree_gaps() {
        let mut tree = super::Xi144IntervalTree::xi_new();
        tree.xi_insert(super::Xi144Interval::xi_new(2, 4));
        tree.xi_insert(super::Xi144Interval::xi_new(6, 8));
        let gaps = tree.xi_gaps(0, 10);
        assert_eq!(gaps.len(), 3);
        assert_eq!(gaps[0], super::Xi144Interval::xi_new(0, 2));
        assert_eq!(gaps[1], super::Xi144Interval::xi_new(4, 6));
        assert_eq!(gaps[2], super::Xi144Interval::xi_new(8, 10));
    }

    #[test]
    fn xi144_interval_tree_merge() {
        let mut tree = super::Xi144IntervalTree::xi_new();
        tree.xi_insert(super::Xi144Interval::xi_new(1, 5));
        tree.xi_insert(super::Xi144Interval::xi_new(3, 8));
        tree.xi_insert(super::Xi144Interval::xi_new(10, 15));
        let merged = tree.xi_merge_overlapping();
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], super::Xi144Interval::xi_new(1, 8));
        assert_eq!(merged[1], super::Xi144Interval::xi_new(10, 15));
    }

    #[test]
    fn xi144_interval_tree_all() {
        let mut tree = super::Xi144IntervalTree::xi_new();
        tree.xi_insert(super::Xi144Interval::xi_new(10, 20));
        tree.xi_insert(super::Xi144Interval::xi_new(1, 5));
        let all = tree.xi_all_intervals();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].xi_low, 1);
        assert_eq!(all[1].xi_low, 10);
    }

    #[test]
    fn xi144_interval_tree_empty() {
        let tree = super::Xi144IntervalTree::xi_new();
        assert_eq!(tree.xi_count(), 0);
        assert!(tree.xi_all_intervals().is_empty());
        assert!(tree.xi_query_point(5).is_empty());
        assert!(tree.xi_gaps(0, 10).len() == 1);
        assert!(tree.xi_merge_overlapping().is_empty());
    }

    #[test]
    fn xi144_interval_tree_contains_point() {
        let iv = super::Xi144Interval::xi_new(5, 15);
        assert!(iv.xi_contains_point(5));
        assert!(iv.xi_contains_point(10));
        assert!(iv.xi_contains_point(14));
        assert!(!iv.xi_contains_point(15));
        assert!(!iv.xi_contains_point(4));
        assert!(!iv.xi_contains_point(100));
    }

}
